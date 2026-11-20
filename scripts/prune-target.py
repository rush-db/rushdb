#!/usr/bin/env python3
"""Prune stale build output from a cargo target directory.

Every change to a crate builds it (and its test binaries) into a new hash
directory under `target/<profile>/build/<crate>/`, and the old ones stay:
a workspace's target grows by gigabytes a day and once filled the disk.
This keeps, for each crate, the newest hash directory and any touched in
the last `--keep-hours`, and removes the rest, together with incremental
compilation state older than that. A removed artifact is rebuilt when
needed; nothing in use is lost.

usage: prune-target.py [--target DIR] [--keep-hours 24] [--free-below-gb N]
                       [--dry-run]

With --free-below-gb, nothing is removed while the target's file system has
at least N GB free (a guard to run before builds).
"""
import argparse
import os
import shutil
import sys
import time


def newest_mtime(path):
    newest = os.lstat(path).st_mtime
    for root, dirs, files in os.walk(path):
        for name in dirs + files:
            try:
                newest = max(newest, os.lstat(os.path.join(root, name)).st_mtime)
            except FileNotFoundError:
                pass
    return newest


def size_of(path):
    total = 0
    for root, _, files in os.walk(path):
        for name in files:
            try:
                total += os.lstat(os.path.join(root, name)).st_size
            except FileNotFoundError:
                pass
    return total


def stale_entries(target, keep_seconds, now):
    """(path, bytes) of every hash directory and incremental entry to remove."""
    for profile in sorted(os.listdir(target)):
        profile_dir = os.path.join(target, profile)
        if not os.path.isdir(profile_dir):
            continue
        build = os.path.join(profile_dir, "build")
        if os.path.isdir(build):
            for crate in sorted(os.listdir(build)):
                crate_dir = os.path.join(build, crate)
                if not os.path.isdir(crate_dir):
                    continue
                hashes = [os.path.join(crate_dir, h) for h in os.listdir(crate_dir)]
                hashes = [h for h in hashes if os.path.isdir(h)]
                if len(hashes) < 2:
                    continue
                dated = sorted(((newest_mtime(h), h) for h in hashes), reverse=True)
                for mtime, path in dated[1:]:
                    if now - mtime > keep_seconds:
                        yield path
        incremental = os.path.join(profile_dir, "incremental")
        if os.path.isdir(incremental):
            for entry in sorted(os.listdir(incremental)):
                path = os.path.join(incremental, entry)
                if now - newest_mtime(path) > keep_seconds:
                    yield path


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--target", default=os.environ.get("CARGO_TARGET_DIR", "target"))
    parser.add_argument("--keep-hours", type=float, default=24)
    parser.add_argument("--free-below-gb", type=float)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if not os.path.isdir(args.target):
        print(f"prune-target: no target directory at {args.target}", file=sys.stderr)
        return 0
    if args.free_below_gb is not None:
        free_gb = shutil.disk_usage(args.target).free / 1e9
        if free_gb >= args.free_below_gb:
            print(f"prune-target: {free_gb:.1f} GB free, nothing to do")
            return 0
    freed = 0
    for path in stale_entries(args.target, args.keep_hours * 3600, time.time()):
        size = size_of(path)
        freed += size
        print(f"{'would remove' if args.dry_run else 'remove'} {path} ({size / 1e9:.2f} GB)")
        if not args.dry_run:
            shutil.rmtree(path, ignore_errors=True)
    print(f"prune-target: {'would free' if args.dry_run else 'freed'} {freed / 1e9:.1f} GB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
