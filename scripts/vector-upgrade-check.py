#!/usr/bin/env python3
"""A vector index written by a published release, opened by the build under
test: its persisted ANN segments open as they were built, are not rewritten,
and answer every search as the release did.

  vector-upgrade-check.py BINARY [VERSION]   (default VERSION 3.0.13)

The release runs as the image rushdb/rushdb:VERSION on a fresh data
directory: a cosine vector index of 32 dimensions over 2,000 records (past
the freeze threshold, so frozen segments are persisted), then ANN searches at
several breadths, exact searches and a filtered search, whose answers are
recorded; it stops cleanly. BINARY then starts on a copy, answers the same
searches (the same records in the same order), leaves the segment manifests
byte for byte as they were, and restarts.
"""

import glob
import json
import os
import random
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

TOKEN = "vector-upgrade-token-0123456789abcdef"
DIMS, RECORDS = 32, 2_000


def call(port, operation):
    body = json.dumps({"protocolVersion": 1, "context": {}, "operation": operation}).encode()
    request = urllib.request.Request(f"http://127.0.0.1:{port}/protocol/v1/execute", data=body, headers={
        "authorization": f"Bearer {TOKEN}", "content-type": "application/json"})
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            return json.loads(response.read())["response"]
    except urllib.error.HTTPError as error:
        raise SystemExit(f"{operation['kind']} failed with {error.code}: {error.read()[:300]!r}")


def wait_ready(port, alive):
    deadline = time.time() + 120
    while time.time() < deadline:
        if not alive():
            raise SystemExit("the server exited before it was ready")
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/ready", timeout=2):
                return
        except (OSError, urllib.error.URLError):
            time.sleep(0.2)
    raise SystemExit("the server was not ready within 120 s")


def free_port():
    import socket
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def searches():
    rng = random.Random(0x18)
    out = []
    for q in range(12):
        vector = [rng.uniform(-1, 1) for _ in range(DIMS)]
        for mode, ef in (("ann", 16), ("ann", 64), ("ann", 128), ("ann", 256), ("exact", None)):
            query = {"labels": ["Doc"], "property": "embedding", "index": "doc-embedding", "vector": vector,
                     "limit": 10, "mode": mode}
            if ef:
                query["ef"] = ef
            out.append((f"q{q}-{mode}-{ef}", {"kind": "query", "query": {"kind": "vector", "query": query}}))
        out.append((f"q{q}-filtered", {"kind": "query", "query": {"kind": "vector", "query": {
            "labels": ["Doc"], "property": "embedding", "index": "doc-embedding", "vector": vector, "limit": 10,
            "predicate": {"bucket": q % 4}}}}))
    return out


def answers(port):
    return {name: [r["properties"]["n"] for r in call(port, op)["records"]] for name, op in searches()}


def manifests(data):
    return {os.path.relpath(path, data): open(path, "rb").read()
            for path in sorted(glob.glob(os.path.join(data, "**", "*.manifest.json"), recursive=True))}


def generate(version, data, port):
    name = f"rushdb-vector-upgrade-{os.getpid()}"
    subprocess.run(["docker", "run", "-d", "--name", name, "--network", "host",
                    "--user", f"{os.getuid()}:{os.getgid()}", "-e", f"RUSHDB_TOKEN={TOKEN}",
                    "-v", f"{data}:/data", f"rushdb/rushdb:{version}", "--listen", "127.0.0.1", "--port", str(port),
                    "--pgwire-port", "0", "--grpc-port", "0", "--bolt-port", "0",
                    "--disk-space-threshold-percent", "0"], check=True, stdout=subprocess.DEVNULL)
    try:
        wait_ready(port, lambda: subprocess.run(["docker", "inspect", "-f", "{{.State.Running}}", name],
                                                capture_output=True, text=True).stdout.strip() == "true")
        call(port, {"kind": "index", "index": {"kind": "createVector", "definition": {
            "name": "doc-embedding", "label": "Doc", "property": "embedding", "dimensions": DIMS,
            "similarity": "cosine"}}})
        rng = random.Random(0x3013)
        for start in range(0, RECORDS, 500):
            call(port, {"kind": "ingest", "ingest": {"kind": "documents", "label": "Doc", "documents": [
                {"n": n, "bucket": n % 4, "$vectors": [{"propertyName": "embedding", "index": "doc-embedding",
                                                        "vector": [rng.uniform(-1, 1) for _ in range(DIMS)]}]}
                for n in range(start, start + 500)]}})
        # Searches build and persist the segments; wait until they are there.
        deadline = time.time() + 120
        while not manifests(data):
            answers(port)
            if time.time() > deadline:
                raise SystemExit(f"{version} persisted no vector segment")
            time.sleep(1)
        recorded = answers(port)
        subprocess.run(["docker", "stop", "-t", "60", name], check=True, stdout=subprocess.DEVNULL)
        return recorded
    finally:
        subprocess.run(["docker", "rm", "-f", "-v", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def main():
    binary = os.path.abspath(sys.argv[1])
    version = sys.argv[2] if len(sys.argv) > 2 else "3.0.13"
    with tempfile.TemporaryDirectory() as scratch:
        original = os.path.join(scratch, "release")
        os.makedirs(original)
        # The data directory keeps the settings it was created with (its
        # port among them): the build under test starts with the same ones.
        port = free_port()
        expected = generate(version, original, port)
        before = manifests(original)
        if any(b'"scalar"' in content for content in before.values()):
            raise SystemExit(f"{version}'s segment manifests already name a scalar")
        data = os.path.join(scratch, "under-test")
        shutil.copytree(original, data)
        for start in (1, 2):
            log = open(os.path.join(scratch, f"start-{start}.log"), "w+")
            server = subprocess.Popen([binary, "serve", "--listen", "127.0.0.1", "--port", str(port),
                                       "--pgwire-port", "0", "--grpc-port", "0", "--bolt-port", "0",
                                       "--disk-space-threshold-percent", "0"],
                                      env=dict(os.environ, RUSHDB_TOKEN=TOKEN, RUSHDB_DATA_DIR=data),
                                      stdout=log, stderr=log)
            try:
                try:
                    wait_ready(port, lambda: server.poll() is None)
                except SystemExit as error:
                    log.seek(0)
                    raise SystemExit(f"start {start}: {error}\n{log.read()[-2000:]}")
                got = answers(port)
                differ = [name for name in expected if got.get(name) != expected[name]]
                if differ:
                    raise SystemExit(f"start {start}: {len(differ)} searches answer differently from {version}, "
                                     f"e.g. {differ[0]}: {expected[differ[0]]} vs {got.get(differ[0])}")
                if manifests(data) != before:
                    raise SystemExit(f"start {start}: the {version} segment manifests changed")
            finally:
                server.send_signal(signal.SIGTERM)
                server.wait(60)
        print(f"{version} vector segments ({len(before)}): opened as built, "
              f"{len(expected)} searches answered as {version} did, across a restart")


if __name__ == "__main__":
    main()
