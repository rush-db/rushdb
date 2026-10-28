#!/usr/bin/env bash
set -euo pipefail

# R59/R60 packaged-consumer qualification: a published crate can only resolve
# dependencies from a registry. Local `path`/`git` overrides and `[patch.*]`
# sections do NOT travel with a published crate, so a consumer would silently
# build against different code than the workspace. This preflight fails the
# release when such a source is present.
#
# Run before `publish-crates.sh` / a release build. It is intentionally NOT
# part of the always-on quality bar: a future local `path`/`git`/`[patch.*]`
# override may be required for local correctness until it is published under
# a registry name (the workspace's one historical example, a patched
# `hnsw_rs` fork, was retired — see RELEASING.md).

python3 - <<'PY'
import glob
import re
import sys

blockers = []

# 1. `[patch.*]` sections are workspace-local and never reach consumers.
root = open("Cargo.toml").read().splitlines()
section = None
for i, line in enumerate(root, 1):
    stripped = line.strip()
    if stripped.startswith("[") and stripped.endswith("]"):
        section = stripped
        continue
    if section and section.startswith("[patch.") and stripped and not stripped.startswith("#"):
        blockers.append(
            f"Cargo.toml:{i}: {section} `{stripped}` is local-only; a packaged "
            f"consumer resolves the unpatched registry crate instead"
        )

# 2. Path/git dependencies in publishable crates must carry a version (Cargo
#    normalizes a path dep with a version to a registry dep; without a version
#    it cannot be resolved by a consumer). Dev-dependencies are not packaged.
def in_scope(section: str) -> bool:
    return (
        section is not None
        and "dependencies" in section
        and "dev-dependencies" not in section
        and "workspace.dependencies" not in section
        and "build-dependencies" not in section
    )

for path in sorted(glob.glob("crates/*/Cargo.toml")):
    section = None
    for i, line in enumerate(open(path), 1):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            section = stripped
            continue
        if not in_scope(section):
            continue
        if stripped.startswith("#") or "=" not in stripped:
            continue
        if "git =" in stripped:
            blockers.append(f"{path}:{i}: git dependency is not publishable: {stripped}")
        elif "path =" in stripped and "version =" not in stripped:
            blockers.append(
                f"{path}:{i}: path dependency without a version is not "
                f"resolvable by a packaged consumer: {stripped}"
            )

if blockers:
    print("package-source check FAILED:", file=sys.stderr)
    for blocker in blockers:
        print(f"  - {blocker}", file=sys.stderr)
    sys.exit(1)

print("package-source check OK: all publishable dependencies resolve from a registry")
PY
