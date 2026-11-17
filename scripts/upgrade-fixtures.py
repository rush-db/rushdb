#!/usr/bin/env python3
"""Data directories written by published releases, for upgrade tests.

  upgrade-fixtures.py generate VERSION OUT [--kill]
      Run the published image rushdb/rushdb:VERSION on a fresh data directory
      in each tenancy mode (OUT/single, OUT/multitenant), apply the workload of
      crates/rushdb-server/tests/fixtures/upgrade/workload.json, record the
      answers to its reads in answers.json, then stop the server cleanly
      (SIGTERM) or, with --kill, with SIGKILL.

  upgrade-fixtures.py verify DIR BINARY
      Start BINARY (the build under test) on a copy of each mode directory of
      DIR with the flags the directory was created with, wait for /ready,
      compare every read with the recorded answer, write, restart, and read
      the write back.

  upgrade-fixtures.py regenerate
      Rewrite crates/rushdb-server/tests/fixtures/upgrade/<version>/ for every
      published 3.x release tag.

  upgrade-fixtures.py matrix BINARY
      CI's upgrade-matrix: generate and verify a fresh directory for every
      published 3.x release, plus the newest one killed with SIGKILL.

The server runs on the host network on a free port, with the other
transports off, as `rushdb serve --listen 127.0.0.1 --port P --pgwire-port 0
--grpc-port 0 --bolt-port 0 [--multitenant]`, followed by the flags in
UPGRADE_FIXTURE_FLAGS (CI turns the free-disk guard off with it).
"""

import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURES = os.path.join(ROOT, "crates/rushdb-server/tests/fixtures/upgrade")
TOKEN = "upgrade-fixture-token"
MODES = {"single": [], "multitenant": ["--multitenant"]}
# The tenants written in multitenant mode: the first gets the workload, the
# second a single record, so reads also check that tenants stay apart.
TENANTS = ["alpha", "beta"]


def workload():
    with open(os.path.join(FIXTURES, "workload.json")) as f:
        return json.load(f)


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def flags(mode, port):
    return [
        "serve",
        "--listen", "127.0.0.1",
        "--port", str(port),
        "--pgwire-port", "0",
        "--grpc-port", "0",
        "--bolt-port", "0",
        *MODES[mode],
        *os.environ.get("UPGRADE_FIXTURE_FLAGS", "").split(),
    ]


def call(port, method, path, body=None, tenant=None):
    headers = {"authorization": f"Bearer {TOKEN}", "content-type": "application/json"}
    if tenant:
        headers["x-rushdb-tenant"] = tenant
    data = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}", data=data, headers=headers, method=method
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status, response.read().decode()
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode()


def execute(port, operation, tenant=None):
    status, text = call(
        port, "POST", "/protocol/v1/execute",
        {"protocolVersion": 1, "context": {"scope": tenant} if tenant else {},
         "operation": operation},
        tenant,
    )
    if status != 200:
        raise SystemExit(f"{operation['kind']} failed with {status}: {text}")
    return json.loads(text)["response"]


def wait_ready(port, alive):
    deadline = time.time() + 120
    while time.time() < deadline:
        if not alive():
            raise SystemExit("the server exited before it was ready")
        try:
            if call(port, "GET", "/ready")[0] == 200:
                return
        except OSError:
            pass
        time.sleep(0.2)
    raise SystemExit("the server was not ready within 120 s")


def contains(got, expected):
    """Whether `got` has every field of `expected` with the same value. A
    release may add response fields, never drop or change one."""
    if isinstance(expected, dict):
        return isinstance(got, dict) and all(
            k in got and contains(got[k], v) for k, v in expected.items()
        )
    if isinstance(expected, list):
        return (isinstance(got, list) and len(got) == len(expected)
                and all(contains(g, e) for g, e in zip(got, expected)))
    return got == expected


def tenants(mode):
    return TENANTS if mode == "multitenant" else [None]


def apply_workload(port, mode):
    w = workload()
    for operation in w["writes"]:
        execute(port, operation, tenants(mode)[0])
    if mode == "multitenant":
        execute(port, w["write"], TENANTS[1])


def read_all(port, mode):
    """Every read for the workload's tenant; `employees` for the other one."""
    w = workload()
    return {
        tenant or "": {
            name: execute(port, operation, tenant)
            for name, operation in w["reads"].items()
            if tenant in (None, TENANTS[0]) or name == "employees"
        }
        for tenant in tenants(mode)
    }


def generate(version, out, kill=False):
    for mode in MODES:
        data = os.path.join(out, mode)
        if os.path.exists(data):
            shutil.rmtree(data)
        os.makedirs(data)
        port = free_port()
        name = f"rushdb-upgrade-{version}-{mode}-{os.getpid()}"
        subprocess.run(
            [
                "docker", "run", "-d", "--name", name, "--network", "host",
                "--user", f"{os.getuid()}:{os.getgid()}",
                "-e", f"RUSHDB_TOKEN={TOKEN}", "-v", f"{os.path.abspath(data)}:/data",
                f"rushdb/rushdb:{version}", *flags(mode, port)[1:],
            ],
            check=True, stdout=subprocess.DEVNULL,
        )

        def alive():
            state = subprocess.run(
                ["docker", "inspect", "-f", "{{.State.Running}}", name],
                capture_output=True, text=True,
            )
            return state.stdout.strip() == "true"

        try:
            wait_ready(port, alive)
            apply_workload(port, mode)
            answers = read_all(port, mode)
            if kill:
                subprocess.run(["docker", "kill", "-s", "KILL", name], check=True,
                               stdout=subprocess.DEVNULL)
            else:
                subprocess.run(["docker", "stop", "-t", "60", name], check=True,
                               stdout=subprocess.DEVNULL)
        finally:
            subprocess.run(["docker", "rm", "-f", "-v", name], stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL)
        with open(os.path.join(out, mode + ".answers.json"), "w") as f:
            json.dump(answers, f, indent=1, sort_keys=True)
            f.write("\n")
        with open(os.path.join(out, mode + ".flags"), "w") as f:
            f.write(" ".join(flags(mode, port)) + "\n")
        print(f"{version} {mode}: generated{' (killed)' if kill else ''}")


def verify(directory, binary):
    w = workload()
    for mode in MODES:
        with open(os.path.join(directory, mode + ".answers.json")) as f:
            expected = json.load(f)
        with open(os.path.join(directory, mode + ".flags")) as f:
            args = f.read().split()
        port = int(args[args.index("--port") + 1])
        with tempfile.TemporaryDirectory() as scratch:
            data = os.path.join(scratch, "data")
            shutil.copytree(os.path.join(directory, mode), data)
            env = dict(os.environ, RUSHDB_TOKEN=TOKEN, RUSHDB_DATA_DIR=data)
            for start in (1, 2):
                log = open(os.path.join(scratch, f"start-{start}.log"), "w+")
                server = subprocess.Popen([binary, *args], env=env, stdout=log, stderr=log)
                try:
                    try:
                        wait_ready(port, lambda: server.poll() is None)
                    except SystemExit as error:
                        log.seek(0)
                        raise SystemExit(f"{directory} {mode} start {start}: {error}\n{log.read()}")
                    got = read_all(port, mode)
                    tenant = tenants(mode)[0]
                    if start == 1:
                        if not contains(got, expected):
                            raise SystemExit(
                                f"{directory} {mode}: reads differ\nexpected {json.dumps(expected, sort_keys=True)}"
                                f"\ngot      {json.dumps(got, sort_keys=True)}"
                            )
                        execute(port, w["write"], tenant)
                    else:
                        names = [r["properties"].get("name") for r in
                                 execute(port, w["reads"]["employees"], tenant)["records"]]
                        if names.count("Dana") != 1:
                            raise SystemExit(f"{directory} {mode}: the write before the restart is gone: {names}")
                finally:
                    server.send_signal(signal.SIGTERM)
                    try:
                        code = server.wait(timeout=60)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        raise SystemExit(f"{directory} {mode}: no clean shutdown within 60 s")
                    log.seek(0)
                    output = log.read()
                    log.close()
                if code != 0:
                    raise SystemExit(f"{directory} {mode}: exit {code} on shutdown\n{output}")
        print(f"{directory} {mode}: started, reads match, write and restart work")


def released():
    tags = subprocess.run(
        ["git", "-C", ROOT, "tag", "-l", "v3.*", "--merged", "HEAD"],
        capture_output=True, text=True, check=True,
    ).stdout.split()
    versions = [t[1:] for t in tags]
    return sorted(versions, key=lambda v: [int(p) for p in v.split(".")])


def main(argv):
    if len(argv) >= 3 and argv[0] == "generate":
        generate(argv[1], argv[2], kill="--kill" in argv[3:])
    elif len(argv) == 3 and argv[0] == "verify":
        verify(argv[1], os.path.abspath(argv[2]))
    elif len(argv) == 2 and argv[0] == "matrix":
        versions = released()
        with tempfile.TemporaryDirectory() as scratch:
            cases = [(v, False) for v in versions] + [(versions[-1], True)]
            for version, kill in cases:
                out = os.path.join(scratch, version + ("-killed" if kill else ""))
                generate(version, out, kill)
                verify(out, os.path.abspath(argv[1]))
                shutil.rmtree(out)
    elif argv == ["regenerate"]:
        for version in released():
            generate(version, os.path.join(FIXTURES, version))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
