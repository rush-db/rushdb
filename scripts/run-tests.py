#!/usr/bin/env python3
"""Run the workspace tests one test binary at a time, with budgets.

Builds every test target (`cargo test --no-run`), then runs each binary as
its own process from its package directory, as `cargo test` does, and
reports for each its tests, peak memory and wall time. The run fails, and
says why by name, when:

- a test fails;
- a binary dies on a signal (killed for memory, a crash): its tests count
  as failed, never as "0 failed";
- a binary's peak memory passes --max-rss-mb (default 4096);
- a test runs over 60 seconds (libtest reports it) and is not listed in
  scripts/slow-tests.txt.

Doc tests run afterwards with `cargo test --doc`.

To find which test of a binary holds the memory, run
scripts/test-memory.py BINARY: one process per test, ranked by peak memory.

usage: scripts/run-tests.py [--max-rss-mb N] [--log-dir DIR] [-- CARGO_ARGS...]
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RESULT = re.compile(r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored")
SLOW = re.compile(r"^test (\S+) has been running for over 60 seconds")


def test_binaries(cargo_args):
    """(name, executable, package directory) of every test target."""
    cmd = ["cargo", "test", "--workspace", "--locked", "--no-run", "--message-format=json", *cargo_args]
    build = subprocess.run(cmd, cwd=ROOT, stdout=subprocess.PIPE, text=True)
    if build.returncode != 0:
        sys.exit("run-tests: the test build failed")
    binaries = []
    for line in build.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact" or not message.get("executable"):
            continue
        if not message.get("profile", {}).get("test"):
            continue
        target = message["target"]
        package_dir = os.path.dirname(message["manifest_path"])
        kind = "lib" if "lib" in target["kind"] else target["kind"][0]
        name = f"{os.path.basename(package_dir)}::{target['name']} ({kind})"
        binaries.append((name, message["executable"], package_dir))
    return binaries


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--max-rss-mb", type=int, default=4096)
    parser.add_argument("--log-dir", default=os.path.join(ROOT, "target", "gate"))
    parser.add_argument("cargo_args", nargs="*")
    args = parser.parse_args()
    os.makedirs(args.log_dir, exist_ok=True)
    allowed_slow = set()
    slow_list = os.path.join(ROOT, "scripts", "slow-tests.txt")
    if os.path.exists(slow_list):
        allowed_slow = {line.split("#")[0].strip() for line in open(slow_list)} - {""}
    failures, totals, rows = [], [0, 0, 0], []
    for name, executable, package_dir in test_binaries(args.cargo_args):
        log_path = os.path.join(args.log_dir, re.sub(r"[^A-Za-z0-9_.-]", "_", name) + ".log")
        started = time.monotonic()
        with open(log_path, "w") as log:
            child = subprocess.Popen([executable], cwd=package_dir, stdout=log, stderr=subprocess.STDOUT,
                                     env={**os.environ, "CARGO_MANIFEST_DIR": package_dir})
            _, status, usage = os.wait4(child.pid, 0)
        seconds = time.monotonic() - started
        rss_mb = usage.ru_maxrss // 1024
        output = open(log_path, errors="replace").read().splitlines()
        counts = [0, 0, 0]
        for line in output:
            match = RESULT.match(line)
            if match:
                counts = [a + int(b) for a, b in zip(counts, match.groups())]
        totals = [a + b for a, b in zip(totals, counts)]
        rows.append((rss_mb, seconds, name, counts))
        if os.WIFSIGNALED(status):
            signal = os.WTERMSIG(status)
            hint = " (out of memory?)" if signal == 9 else ""
            failures.append(f"{name}: killed by signal {signal}{hint} after {seconds:.0f} s at {rss_mb} MB; "
                            f"its results are incomplete (log {log_path})")
        elif os.WEXITSTATUS(status) != 0:
            failed = [line for line in output if line.startswith("test ") and line.endswith("FAILED")]
            failures.append(f"{name}: {counts[1]} failed: " + ", ".join(line.split()[1] for line in failed[:10]))
        if rss_mb > args.max_rss_mb:
            failures.append(f"{name}: peak memory {rss_mb} MB over the {args.max_rss_mb} MB budget; "
                            f"rank its tests with scripts/test-memory.py {executable}")
        for line in output:
            slow = SLOW.match(line)
            if slow and slow.group(1) not in allowed_slow:
                failures.append(f"{name}: {slow.group(1)} ran over 60 s (list it in scripts/slow-tests.txt "
                                f"with a reason, or make it faster)")
    doc = subprocess.run(["cargo", "test", "--workspace", "--locked", "--doc", *args.cargo_args], cwd=ROOT,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    open(os.path.join(args.log_dir, "doctests.log"), "w").write(doc.stdout)
    for line in doc.stdout.splitlines():
        match = RESULT.match(line)
        if match:
            totals = [a + int(b) for a, b in zip(totals, match.groups())]
    if doc.returncode != 0:
        failures.append("doc tests failed (log doctests.log)")
    print("largest test binaries by peak memory:")
    for rss_mb, seconds, name, counts in sorted(rows, reverse=True)[:5]:
        print(f"  {rss_mb:6} MB {seconds:7.1f} s  {name} ({counts[0]} passed)")
    print(f"passed {totals[0]} failed {totals[1]} ignored {totals[2]}")
    for failure in failures:
        print("FAILED: " + failure)
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
