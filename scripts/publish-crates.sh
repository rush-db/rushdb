#!/usr/bin/env sh
# Publish the crates that make up the standalone RushDB distribution.
#
# Use --dry-run during archive qualification. The package verifier resolves
# interdependent crates from the clean consumer's available registry; a first
# release therefore requires synchronized versioning/publication of the full
# internal crate set before consumer verification can pass. --publish
# intentionally requires a separate workflow_dispatch run with the
# CARGO_REGISTRY_TOKEN secret.
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

for package in $crates; do
  cargo publish --package "$package"
  echo "Published $package. Wait for it to appear on crates.io before continuing."
done
