#!/usr/bin/env bash
# The release gate, in tiers. Each tier runs the tiers before it and stops
# at the first failure, naming it.
#
#   scripts/gate.sh fast      before each commit (a few minutes, incremental):
#                             fmt, quality bar, clippy, the exhaustiveness
#                             tests (a new key family must be classified)
#   scripts/gate.sh full      before a branch is handed over: the workspace
#                             tests in both resident modes, per test binary,
#                             with memory and time budgets (scripts/run-tests.py)
#   scripts/gate.sh release   before an image is handed over: the release
#                             build, the upgrade matrix and a 3.0.13 vector
#                             index opened as built, the openCypher TCK,
#                             the advisory check, the image and its smoke
#                             (scripts/smoke-image.sh: restart, text, hybrid)
#
# Environment:
#   GATE_WRAP          a prefix for the heavy cargo commands, e.g. a memory-
#                      capped scope: "systemd-run --user --scope -q -p MemoryMax=14G"
#   GATE_LOG_DIR       logs (default target/gate)
#   GATE_IMAGE         the image tag to build (default rushdb/rushdb:gate)
#   GATE_MIN_FREE_GB   free disk the gate needs before it builds (default 40)
#   GATE_MAX_RSS_MB    peak memory budget of one test binary (default 4096)
set -uo pipefail

TIER="${1:-fast}"
case "$TIER" in fast|full|release) ;; *) echo "usage: scripts/gate.sh fast|full|release" >&2; exit 2 ;; esac
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
L="${GATE_LOG_DIR:-$ROOT/target/gate}"
IMAGE="${GATE_IMAGE:-rushdb/rushdb:gate}"
mkdir -p "$L"
read -r -a WRAP <<< "${GATE_WRAP:-}"
heavy() { "${WRAP[@]}" "$@"; }
step() { echo "$(date +%H:%M:%S) $*"; }
fail() { echo "$(date +%H:%M:%S) FAILED: $*"; exit 1; }

# Build output has a budget: stale artifacts go first, and a gate that
# would run out of disk stops before it builds.
step "disk"
python3 scripts/prune-target.py --keep-hours "${GATE_KEEP_HOURS:-6}" > "$L/prune.log" 2>&1 || fail "prune (see $L/prune.log)"
tail -1 "$L/prune.log"
free_gb=$(df -P --block-size=1G "$ROOT" | awk 'NR==2 {print $4}')
[ "$free_gb" -ge "${GATE_MIN_FREE_GB:-40}" ] ||
  fail "disk: ${free_gb} GB free, the gate needs ${GATE_MIN_FREE_GB:-40} GB; prune other target directories (scripts/prune-target.py --target DIR)"

step "fmt"; cargo fmt --all -- --check > "$L/fmt.log" 2>&1 || fail "fmt (see $L/fmt.log)"
step "quality bar"; bash scripts/check-quality-bar.sh > "$L/quality.log" 2>&1 || { cat "$L/quality.log"; fail "quality bar"; }
step "clippy"
heavy cargo clippy --workspace --all-targets --locked > "$L/clippy.log" 2>&1 || { grep -E "^(error|warning)" -A5 "$L/clippy.log" | head -30; fail "clippy"; }
grep -qE "^(warning|error)" "$L/clippy.log" && { grep -E "^(warning|error)" -A5 "$L/clippy.log" | head -30; fail "clippy warnings"; }
step "exhaustiveness"
heavy cargo test --locked -p rushdb-storage --lib resident::tests::every_key_family > "$L/exhaustive.log" 2>&1 ||
  { tail -20 "$L/exhaustive.log"; fail "exhaustiveness"; }
[ "$TIER" = fast ] && { step "GATE GREEN (fast)"; exit 0; }

for mode in on off; do
  step "tests, resident layers $mode"
  RUSHDB_RESIDENT_CACHES=$mode heavy python3 scripts/run-tests.py --max-rss-mb "${GATE_MAX_RSS_MB:-4096}" \
    --log-dir "$L/tests-$mode" > "$L/tests-$mode.log" 2>&1
  rc=$?
  sed -n '/^largest test binaries/,$p' "$L/tests-$mode.log"
  [ $rc -eq 0 ] || fail "tests, resident layers $mode"
done
[ "$TIER" = full ] && { step "GATE GREEN (full)"; exit 0; }

step "release build"
heavy cargo build --release --locked -p rushdb-server --bin rushdb > "$L/release.log" 2>&1 || fail "release build"
heavy cargo build --release --locked -p rushdb-language --example grafeo_reference >> "$L/release.log" 2>&1 || fail "grafeo_reference build"
step "upgrade matrix"
UPGRADE_FIXTURE_FLAGS="--disk-space-threshold-percent 0" ./scripts/upgrade-fixtures.py matrix ./target/release/rushdb > "$L/upgrade.log" 2>&1 || fail "upgrade matrix"
echo "  $(grep -c 'reads match, write and restart work' "$L/upgrade.log") upgrade cases"
step "vector upgrade (3.0.13 segments)"
python3 -I scripts/vector-upgrade-check.py ./target/release/rushdb 3.0.13 > "$L/vector-upgrade.log" 2>&1 ||
  { tail -5 "$L/vector-upgrade.log"; fail "vector upgrade"; }
tail -1 "$L/vector-upgrade.log"

step "openCypher TCK"
TCK_SOURCE="$ROOT/target/gate-opencypher"
if [ ! -d "$TCK_SOURCE" ]; then
  git clone -q --depth 1 --branch 2024.2 https://github.com/opencypher/openCypher.git "$TCK_SOURCE" || fail "TCK clone"
fi
[ "$(git -C "$TCK_SOURCE" rev-parse HEAD)" = "59edf2e1c17b845bf97c334ed06b2eb780950c13" ] || fail "TCK pin"
TCK_DATA="$(mktemp -d)"
./target/release/rushdb serve --port 3217 --pgwire-port 0 --grpc-port 0 --bolt-port 0 --languages cypher \
  --token local-token --request-timeout-ms 20000 --disk-space-threshold-percent 0 --data-dir "$TCK_DATA" \
  > "$L/tck-server.log" 2>&1 &
TCK_PID=$!
curl -s --fail --retry 30 --retry-connrefused --retry-delay 1 http://127.0.0.1:3217/ready > /dev/null ||
  { kill $TCK_PID; fail "TCK server"; }
python3 -I scripts/run-opencypher-tck.py --source "$TCK_SOURCE" --endpoint http://127.0.0.1:3217/protocol/v1/execute \
  --output "$L/opencypher-tck.json" --reference ./target/release/examples/grafeo_reference > "$L/tck.log" 2>&1
rc=$?
kill $TCK_PID; wait $TCK_PID 2>/dev/null; rm -rf "$TCK_DATA"
tail -5 "$L/tck.log"
[ $rc -eq 0 ] || fail "TCK"

step "advisories"
python3 scripts/check-advisories.py > "$L/advisories.log" 2>&1 || { cat "$L/advisories.log"; fail "advisories"; }
tail -1 "$L/advisories.log"

step "image"
docker build -q -t "$IMAGE" . > "$L/image.log" 2>&1 || { tail -20 "$L/image.log"; fail "image"; }
docker image inspect "$IMAGE" --format '  image {{.Id}} {{.Size}}'
step "image smoke"
./scripts/smoke-image.sh "$IMAGE" > "$L/smoke.log" 2>&1 || { tail -20 "$L/smoke.log"; fail "image smoke"; }
tail -3 "$L/smoke.log"
step "GATE GREEN (release)"
