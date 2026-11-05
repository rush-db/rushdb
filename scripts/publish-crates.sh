#!/usr/bin/env sh
# Publish the crates that make up the standalone RushDB distribution.
#
# Use --dry-run during archive qualification. The package verifier resolves
# interdependent crates from the clean consumer's available registry; a first
# release therefore requires synchronized versioning/publication of the full
# internal crate set before consumer verification can pass. --publish
# runs from the crates.yml workflow, which has the CARGO_REGISTRY_TOKEN secret.
set -eu

mode="${1:---dry-run}"
case "$mode" in
  --dry-run|--publish) ;;
  *)
    echo "usage: $0 [--dry-run|--publish]" >&2
    exit 2
    ;;
esac

# Publishable workspace crates in dependency order, read from the manifests:
# a crate is published only after every workspace crate it depends on.
crates=$(cargo metadata --format-version 1 --no-deps --locked | python3 -c '
import json, sys
packages = {p["name"]: p for p in json.load(sys.stdin)["packages"] if p.get("publish") is None or p["publish"]}
deps = {name: {d["name"] for d in p["dependencies"] if d["name"] in packages} for name, p in packages.items()}
order = []
while deps:
    ready = sorted(name for name, needs in deps.items() if not needs)
    if not ready:
        sys.exit("dependency cycle among " + ", ".join(sorted(deps)))
    order += ready
    for name in ready:
        del deps[name]
    for needs in deps.values():
        needs.difference_update(ready)
print("\n".join(order))
')

if [ "$mode" = "--dry-run" ]; then
  cargo package --workspace --allow-dirty --no-verify
  exit 0
fi

# crates.io rate-limits new crates, so a first release hits 429 part way. On a
# 429 this waits until the time crates.io names and retries; versions already
# on crates.io are skipped, so a re-run resumes where a run stopped. cargo
# publish waits until each crate is available before returning.
version=$(cargo metadata --format-version 1 --no-deps |
  sed -n 's/.*"name":"rushdb-server","version":"\([^"]*\)".*/\1/p')
[ -n "$version" ] || { echo "cannot read the rushdb-server version" >&2; exit 1; }

log=$(mktemp)
trap 'rm -f "$log"' EXIT

publish() {
  while :; do
    if cargo publish --package "$1" 2>"$log"; then
      cat "$log" >&2
      return 0
    fi
    cat "$log" >&2
    grep -q '429 Too Many Requests' "$log" || return 1
    until=$(sed -n 's/.*try again after \(.* GMT\).*/\1/p' "$log" | head -n 1)
    wait=$(( $(date -d "${until:-now}" +%s) - $(date +%s) + 15 ))
    [ "$wait" -gt 0 ] || wait=60
    echo "crates.io rate limit: retrying $1 in ${wait}s"
    sleep "$wait"
  done
}

for package in $crates; do
  status=$(curl -s -o /dev/null -w '%{http_code}' -A 'rushdb-release (https://github.com/rush-db/rushdb)' \
    "https://crates.io/api/v1/crates/$package/$version")
  if [ "$status" = 200 ]; then
    echo "$package $version is already on crates.io; skipping."
    continue
  fi
  publish "$package"
done
