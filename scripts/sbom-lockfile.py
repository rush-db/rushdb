#!/usr/bin/env python3
"""Write a Cargo.lock with only the packages one release target ships.

    scripts/sbom-lockfile.py <target> <out-dir>

The release SBOM is generated from a lockfile, and the workspace Cargo.lock
lists every package for every platform and purpose (Windows crates in a
macOS SBOM, test and bench dependencies). This keeps the packages reachable
from rushdb-server's `rushdb` binary through normal dependencies, as
`cargo metadata --filter-platform <target>` resolves them, and writes them
to <out-dir>/Cargo.lock for the SBOM step to scan. Build-script and dev
dependencies are left out: they run on the build host or in tests and are
not in the binary. Proc-macros are normal dependencies and stay in.
"""
import json
import os
import subprocess
import sys
import tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROOT_PACKAGE = "rushdb-server"


def shipped_ids(metadata):
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    roots = [p["id"] for p in metadata["packages"] if p["name"] == ROOT_PACKAGE and p["id"] in nodes]
    if len(roots) != 1:
        sys.exit(f"sbom-lockfile: expected one {ROOT_PACKAGE} package, found {len(roots)}")
    seen, stack = set(), roots
    while stack:
        node = nodes[stack.pop()]
        if node["id"] in seen:
            continue
        seen.add(node["id"])
        for dep in node["deps"]:
            if any(kind["kind"] is None for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    return seen


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__.strip().splitlines()[2].strip())
    target, out_dir = sys.argv[1], sys.argv[2]
    run = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    metadata = json.loads(run.stdout)
    by_id = {p["id"]: p for p in metadata["packages"]}
    shipped = {(by_id[i]["name"], by_id[i]["version"]) for i in shipped_ids(metadata)}

    with open(os.path.join(ROOT, "Cargo.lock"), "rb") as lockfile:
        lock = tomllib.load(lockfile)
    kept = [p for p in lock["package"] if (p["name"], p["version"]) in shipped]
    missing = shipped - {(p["name"], p["version"]) for p in kept}
    if missing:
        sys.exit(f"sbom-lockfile: not in Cargo.lock: {sorted(missing)}")

    kept_names = {p["name"] for p in kept}
    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, "Cargo.lock"), "w") as out:
        out.write(f"version = {lock['version']}\n")
        for package in kept:
            out.write("\n[[package]]\n")
            for key in ("name", "version", "source", "checksum"):
                if key in package:
                    out.write(f"{key} = {json.dumps(package[key])}\n")
            # Entries read "name", "name version" or "name version (source)".
            deps = [d for d in package.get("dependencies", []) if d.split(" ")[0] in kept_names]
            if deps:
                out.write("dependencies = [\n" + "".join(f" {json.dumps(d)},\n" for d in deps) + "]\n")
    print(f"sbom-lockfile: {target}: {len(kept)} of {len(lock['package'])} packages")


if __name__ == "__main__":
    main()
