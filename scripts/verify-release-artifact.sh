#!/usr/bin/env bash
set -euo pipefail

# Validate a release archive as a consumer would: checksum, package contents,
# machine-readable capability evidence, and the documented one-command startup
# path. This deliberately uses only the archived binaries, never `target/`.

ARCHIVE="${1:?usage: verify-release-artifact.sh ARCHIVE CHECKSUM CAPABILITIES_JSON}"
CHECKSUM="${2:?usage: verify-release-artifact.sh ARCHIVE CHECKSUM CAPABILITIES_JSON}"
CAPABILITIES="${3:?usage: verify-release-artifact.sh ARCHIVE CHECKSUM CAPABILITIES_JSON}"
TMP_DIR="$(mktemp -d)"
PID=""
PORT="${RUSHDB_VERIFY_PORT:-$((30000 + RANDOM % 20000))}"
BASE_URL="http://127.0.0.1:${PORT}"

cleanup() {
  if [ -n "$PID" ]; then kill "$PID" 2>/dev/null || true; fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

[ -f "$ARCHIVE" ]
[ -f "$CHECKSUM" ]
[ -f "$CAPABILITIES" ]

if command -v shasum >/dev/null 2>&1; then
  (cd "$(dirname "$CHECKSUM")" && shasum -a 256 -c "$(basename "$CHECKSUM")")
else
  (cd "$(dirname "$CHECKSUM")" && sha256sum -c "$(basename "$CHECKSUM")")
fi

tar -xzf "$ARCHIVE" -C "$TMP_DIR"
for file in rushdb README.md LICENSE; do
  [ -e "$TMP_DIR/$file" ] || { echo "release archive is missing $file" >&2; exit 1; }
done
[ -x "$TMP_DIR/rushdb" ]
"$TMP_DIR/rushdb" --help >/dev/null
"$TMP_DIR/rushdb" serve --help >/dev/null

python3 - "$CAPABILITIES" <<'PY'
import json
import sys

payload = json.load(open(sys.argv[1]))
for name in ('build', 'capabilities', 'queryLanguages'):
    assert payload[name]['protocolVersion'] == 1, name
build = payload['build']['response']['build']
assert build['engine'] == 'rushdb-lmpg'
assert build['protocolVersion'] == 1
assert isinstance(payload['capabilities']['response']['capabilities'], list)
assert isinstance(payload['queryLanguages']['response']['languages'], list)
PY

"$TMP_DIR/rushdb" serve --port "$PORT" --pgwire-port 0 --data-dir "$TMP_DIR/data" >"$TMP_DIR/rushdb.log" 2>&1 &
PID="$!"
for _ in $(seq 1 100); do
  if curl --fail --silent "$BASE_URL/ready" >/dev/null; then break; fi
  sleep 0.05
done
curl --fail --silent "$BASE_URL/ready" >/dev/null

request() {
  curl --fail --silent --show-error "$BASE_URL/protocol/v1/execute" \
    -H 'Authorization: Bearer local-token' \
    -H 'Content-Type: application/json' \
    --data "$1"
}

request '{"protocolVersion":1,"context":{},"operation":{"kind":"mutate","mutation":{"kind":"putRecord","mode":"create","label":"RELEASE_SMOKE","properties":{"value":"durable"},"options":{}}}}' >/dev/null
kill "$PID"
wait "$PID" 2>/dev/null || true
PID=""
"$TMP_DIR/rushdb" serve --port "$PORT" --pgwire-port 0 --data-dir "$TMP_DIR/data" >"$TMP_DIR/rushdb-restart.log" 2>&1 &
PID="$!"
for _ in $(seq 1 100); do
  if curl --fail --silent "$BASE_URL/ready" >/dev/null; then break; fi
  sleep 0.05
done
curl --fail --silent "$BASE_URL/ready" >/dev/null

# Provenance: the *extracted* binary must report the same build identity as the
# capabilities evidence generated from the build under test. Compare only stable
# fields; transports/ports differ between the capture and verify runs.
observed_build="$(request '{"protocolVersion":1,"context":{},"operation":{"kind":"describe","describe":{"kind":"buildInfo"}}}')"
python3 - "$CAPABILITIES" "$observed_build" <<'PY'
import json
import sys

expected = json.load(open(sys.argv[1]))['build']['response']['build']
observed = json.loads(sys.argv[2])['response']['build']
for field in ('engine', 'version', 'buildRevision', 'protocolVersion'):
    assert expected.get(field) == observed.get(field), (
        field,
        expected.get(field),
        observed.get(field),
    )
PY

result="$(request '{"protocolVersion":1,"context":{},"operation":{"kind":"query","query":{"kind":"records","query":{"labels":["RELEASE_SMOKE"],"predicate":{},"limit":1}}}}')"
python3 - "$result" <<'PY'
import json
import sys

response = json.loads(sys.argv[1])
records = response['response']['records']
assert len(records) == 1
assert records[0]['properties']['value'] == 'durable'
PY

echo "Verified release archive: $ARCHIVE"
