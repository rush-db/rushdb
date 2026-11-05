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

# This order follows the workspace dependency graph. Cargo checks each package
# again, so a newly introduced edge cannot silently publish out of order.
crates='
rushdb-types
rushdb-storage
rushdb-lmpg
rushdb-query
rushdb-authz
rushdb-tx
rushdb-protocol
rushdb-bolt
rushdb-grpc
rushdb-vector
rushdb-textsearch
rushdb-raft
rushdb-backup
rushdb-sql
rushdb-language
rushdb-mongodb
rushdb-engine
rushdb-server
'

if [ "$mode" = "--dry-run" ]; then
  cargo package --workspace --allow-dirty --no-verify
  exit 0
fi

# crates.io rate-limits new crates, so a first release can stop part way.
# Versions already on crates.io are skipped, so a re-run resumes where it
# stopped. cargo publish waits until each crate is available before returning.
version=$(cargo metadata --format-version 1 --no-deps |
  sed -n 's/.*"name":"rushdb-server","version":"\([^"]*\)".*/\1/p')
[ -n "$version" ] || { echo "cannot read the rushdb-server version" >&2; exit 1; }

for package in $crates; do
  status=$(curl -s -o /dev/null -w '%{http_code}' -A 'rushdb-release (https://github.com/rush-db/rushdb)' \
    "https://crates.io/api/v1/crates/$package/$version")
  if [ "$status" = 200 ]; then
    echo "$package $version is already on crates.io; skipping."
    continue
  fi
  cargo publish --package "$package"
done
