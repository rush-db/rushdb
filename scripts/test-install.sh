#!/usr/bin/env bash
# Run install.sh the way users do (`curl ... | sh`) inside clean Linux
# containers against a locally served release directory, then start the
# installed database and write a record through it.
#
#   scripts/test-install.sh RELEASE_DIR [IMAGE...]
#
# RELEASE_DIR holds rushdb-<target>.tar.gz and its .sha256 for this machine's
# architecture. Images default to supported glibc distributions plus two that
# must be refused with a clear message (musl, and a glibc older than 2.28).
set -euo pipefail

RELEASE_DIR="$(cd "${1:?usage: test-install.sh RELEASE_DIR [IMAGE...]}" && pwd)"
shift
IMAGES=("$@")
if [ ${#IMAGES[@]} -eq 0 ]; then
  IMAGES=(debian:12 ubuntu:20.04 rockylinux:8 amazonlinux:2023 alpine:3 centos:7)
fi
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SERVE="$(mktemp -d)"
PORT="$((40000 + RANDOM % 10000))"
SERVER=""
# shellcheck disable=SC2329 # called by the EXIT trap
cleanup() {
  if [ -n "$SERVER" ]; then kill "$SERVER" 2>/dev/null || true; fi
  rm -rf "$SERVE"
}
trap cleanup EXIT

cp "$ROOT/install.sh" "$RELEASE_DIR"/rushdb-*.tar.gz "$RELEASE_DIR"/rushdb-*.tar.gz.sha256 "$SERVE/"
python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$SERVE" >/dev/null 2>&1 &
SERVER="$!"
for _ in $(seq 1 50); do
  curl -fs "http://127.0.0.1:$PORT/install.sh" >/dev/null && break
  sleep 0.1
done

# The script each container runs. $1 = served base URL, $2 = expected outcome.
read -r -d '' PROBE <<'EOF' || true
set -eu
base="$1"
expect="$2"
# Minimal images lack what every real host has; install curl and tar.
if ! command -v curl >/dev/null 2>&1 || ! command -v tar >/dev/null 2>&1; then
  if command -v apt-get >/dev/null; then apt-get update -qq && apt-get install -y -qq curl ca-certificates tar gzip >/dev/null
  elif command -v apk >/dev/null; then apk add -q curl tar
  elif command -v dnf >/dev/null; then dnf install -y -q tar gzip >/dev/null
  elif command -v yum >/dev/null; then yum install -y -q curl tar gzip >/dev/null
  fi
fi
export HOME=/root SHELL=/bin/bash
if [ "$expect" != "ok" ]; then
  if out="$(curl -fsSL "$base/install.sh" | RUSHDB_DOWNLOAD_URL="$base" sh 2>&1)"; then
    echo "expected the installer to refuse this system, but it succeeded"; exit 1
  fi
  echo "$out" | grep -q "$expect" || { echo "unexpected refusal: $out"; exit 1; }
  echo "refused as expected: $out"
  exit 0
fi
curl -fsSL "$base/install.sh" | RUSHDB_DOWNLOAD_URL="$base" sh
grep -q '# rushdb' /root/.bashrc
PATH="/root/.rushdb/bin:$PATH"
rushdb --version
port=$((20000 + $$ % 10000))
# The container shares the CI runner's disk, where the build tree can leave
# less than the production 10% free. This probe tests installation, not the
# disk guard.
export RUSHDB_DISK_SPACE_THRESHOLD_PERCENT=0
rushdb up --data-dir /tmp/db --port "$port" --pgwire-port 0 >/tmp/up.log 2>&1 &
for _ in $(seq 1 100); do rushdb health --url "http://127.0.0.1:$port" >/dev/null 2>&1 && break; sleep 0.1; done
rushdb health --url "http://127.0.0.1:$port"
curl -fsS "http://127.0.0.1:$port/protocol/v1/execute" \
  -H 'Authorization: Bearer local-token' -H 'Content-Type: application/json' \
  --data '{"protocolVersion":1,"context":{},"operation":{"kind":"mutate","mutation":{"kind":"putRecord","mode":"create","label":"BOOK","properties":{"title":"Install check"},"options":{}}}}' >/dev/null
# Re-running the installer upgrades in place and leaves the profile alone.
curl -fsSL "$base/install.sh" | RUSHDB_DOWNLOAD_URL="$base" sh >/dev/null
[ "$(grep -c '# rushdb' /root/.bashrc)" = "1" ]
echo "installed and serving"
EOF

failed=0
for image in "${IMAGES[@]}"; do
  case "$image" in
    alpine*) expect="musl" ;;
    centos:7) expect="older than the required 2.28" ;;
    *) expect="ok" ;;
  esac
  printf '== %s ' "$image"
  if output="$(docker run --rm --network host "$image" sh -c "$PROBE" probe "http://127.0.0.1:$PORT" "$expect" 2>&1)"; then
    echo "ok"
  else
    echo "FAILED"
    echo "$output" | tail -20
    failed=1
  fi
done
exit "$failed"
