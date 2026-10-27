#!/usr/bin/env bash
set -euo pipefail

BIN="${1:-./target/release/rushdb}"
OUT="${2:-dist/capabilities.json}"
PORT="${RUSHDB_CAPABILITIES_PORT:-39117}"
DATA_DIR="$(mktemp -d)"
TOKEN="capability-snapshot-token"
PID=""

cleanup() {
  if [ -n "$PID" ]; then kill "$PID" 2>/dev/null || true; fi
  rm -rf "$DATA_DIR"
}
trap cleanup EXIT

mkdir -p "$(dirname "$OUT")"
"$BIN" serve --port "$PORT" --pgwire-port 0 --data-dir "$DATA_DIR" --token "$TOKEN" >"$DATA_DIR/node.log" 2>&1 &
PID="$!"
for _ in $(seq 1 100); do
  if curl --fail --silent "http://127.0.0.1:$PORT/ready" >/dev/null; then break; fi
  sleep 0.05
done
curl --fail --silent "http://127.0.0.1:$PORT/ready" >/dev/null

request() {
  local body="$1"
  curl --fail --silent --show-error "http://127.0.0.1:$PORT/protocol/v1/execute" \
    -H "X-RushDB-Token: $TOKEN" -H 'Content-Type: application/json' --data "$body"
}

build="$(request '{"protocolVersion":1,"context":{},"operation":{"kind":"describe","describe":{"kind":"buildInfo"}}}')"
capabilities="$(request '{"protocolVersion":1,"context":{},"operation":{"kind":"describe","describe":{"kind":"capabilities"}}}')"
languages="$(request '{"protocolVersion":1,"context":{},"operation":{"kind":"describe","describe":{"kind":"queryLanguages"}}}')"
printf '{"build":%s,"capabilities":%s,"queryLanguages":%s}\n' "$build" "$capabilities" "$languages" > "$OUT"

# Keep the documentation artifact shape stable: these fields are consumed by
# release tooling and the 3.x capability/transport matrix.
python3 - "$OUT" <<'PY'
import json, sys
payload = json.load(open(sys.argv[1]))
for name in ('build', 'capabilities', 'queryLanguages'):
    assert payload[name]['protocolVersion'] == 1, name
    assert payload[name]['response']['kind'], name
build = payload['build']['response']['build']
for name in ('engine', 'version', 'protocolVersion', 'transports', 'limits'):
    assert name in build, name
assert isinstance(payload['capabilities']['response']['capabilities'], list)
assert isinstance(payload['queryLanguages']['response']['languages'], list)
PY
