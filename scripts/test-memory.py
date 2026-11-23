#!/usr/bin/env python3
"""Rank the tests of one test binary by peak memory: each test runs in a
process of its own, one after another. Use it when scripts/run-tests.py
reports a binary over its memory budget.

usage: scripts/test-memory.py TEST_BINARY [--top N] [--filter SUBSTRING]
"""
import argparse
import os
import subprocess
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary")
    parser.add_argument("--top", type=int, default=15)
    parser.add_argument("--filter", default="")
    args = parser.parse_args()
    listing = subprocess.run([args.binary, "--list"], capture_output=True, text=True, check=True).stdout
    names = [line[: -len(": test")] for line in listing.splitlines()
             if line.endswith(": test") and args.filter in line]
    ranked = []
    for name in names:
        started = time.monotonic()
        child = subprocess.Popen([args.binary, name, "--exact", "--test-threads=1", "-q"],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        _, status, usage = os.wait4(child.pid, 0)
        ranked.append((usage.ru_maxrss // 1024, time.monotonic() - started, name, status))
    ranked.sort(reverse=True)
    for rss_mb, seconds, name, status in ranked[: args.top]:
        state = "" if status == 0 else f"  (exit status {status})"
        print(f"{rss_mb:7} MB {seconds:7.1f} s  {name}{state}")
    print(f"{len(ranked)} tests")


if __name__ == "__main__":
    main()
