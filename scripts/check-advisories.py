#!/usr/bin/env python3
"""The RustSec advisory check CI runs (rustsec/audit-check), locally.

Fails on a vulnerability advisory against Cargo.lock that .cargo/audit.toml
does not ignore; unmaintained and unsound notices are listed, as CI lists
them, without failing. Uses `cargo audit` when it is installed (it reads
.cargo/audit.toml itself), else `cargo deny check advisories` with the
same ignore list applied here.
"""
import os
import re
import shutil
import subprocess
import sys
import tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    if shutil.which("cargo-audit"):
        sys.exit(subprocess.run(["cargo", "audit"], cwd=ROOT).returncode)
    if not shutil.which("cargo-deny"):
        sys.exit("check-advisories: install cargo-audit or cargo-deny")
    with open(os.path.join(ROOT, ".cargo", "audit.toml"), "rb") as config:
        ignored = set(tomllib.load(config).get("advisories", {}).get("ignore", []))
    run = subprocess.run(["cargo", "deny", "check", "advisories"], cwd=ROOT, capture_output=True, text=True)
    vulnerabilities, notices = set(), set()
    kind = None
    for line in run.stderr.splitlines():
        heading = re.match(r"^(?:error|warning)\[(\w+)\]: (.*)", line)
        if heading:
            kind = heading.group(1)
            continue
        found = re.search(r"ID: (RUSTSEC-\d+-\d+)", line)
        if found and kind:
            (vulnerabilities if kind == "vulnerability" else notices).add(found.group(1))
            kind = None
    for advisory in sorted(notices):
        print(f"notice {advisory}")
    unignored = sorted(vulnerabilities - ignored)
    for advisory in unignored:
        print(f"VULNERABILITY {advisory} (not in .cargo/audit.toml)")
    print(f"advisories: {len(vulnerabilities)} vulnerabilities ({len(vulnerabilities & ignored)} ignored with a "
          f"reason in .cargo/audit.toml), {len(notices)} notices")
    sys.exit(1 if unignored else 0)


if __name__ == "__main__":
    main()
