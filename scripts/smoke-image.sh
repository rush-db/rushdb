#!/usr/bin/env bash
# Smoke-test a RushDB container image: it turns healthy, accepts a durable
# write that survives a restart, and refuses to start without a token.
#
#   scripts/smoke-image.sh IMAGE [PLATFORM]
set -euo pipefail

IMAGE="${1:?usage: smoke-image.sh IMAGE [PLATFORM]}"
PLATFORM="${2:-}"
NAME="rushdb-smoke-$$"
PORT="$((40000 + RANDOM % 10000))"
PLATFORM_ARGS=()
[ -n "$PLATFORM" ] && PLATFORM_ARGS=(--platform "$PLATFORM")
# shellcheck disable=SC2329 # called by the EXIT trap
cleanup() { docker rm -f "$NAME" >/dev/null 2>&1 || true; }
trap cleanup EXIT

wait_healthy() {
  for _ in $(seq 1 90); do
    [ "$(docker inspect -f '{{.State.Health.Status}}' "$NAME")" = healthy ] && return 0
    sleep 1
  done
  docker logs "$NAME" >&2
  echo "the container never became healthy" >&2
  exit 1
}

execute() {
  curl -fsS "http://127.0.0.1:$PORT/protocol/v1/execute" -H 'Authorization: Bearer smoke' \
    -H 'Content-Type: application/json' --data "$1"
}

# The container shares the CI runner's disk; this smoke tests the image, not
# the production free-disk guard.
docker run -d --name "$NAME" "${PLATFORM_ARGS[@]}" -p "127.0.0.1:$PORT:3000" \
  -e RUSHDB_TOKEN=smoke -e RUSHDB_DISK_SPACE_THRESHOLD_PERCENT=0 "$IMAGE" >/dev/null
wait_healthy
execute '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{"kind":"putRecord","mode":"create","label":"SMOKE","properties":{"ok":true}}}}' >/dev/null
docker restart -t 60 "$NAME" >/dev/null
wait_healthy
execute '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{"labels":["SMOKE"]}}}}' |
  grep -q '"total":1' || { echo "the write did not survive a restart" >&2; exit 1; }

if docker run --rm "${PLATFORM_ARGS[@]}" "$IMAGE" >/dev/null 2>&1; then
  echo "the image started with the default token" >&2
  exit 1
fi
echo "Smoke-tested $IMAGE"
