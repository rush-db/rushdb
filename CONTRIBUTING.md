# Contributing to RushDB

RushDB 3 is a database engine. Changes must preserve deterministic replay,
tenant isolation, durability, and transport-independent behavior.

## Local setup

Install Rust with `rustup`; `rust-toolchain.toml` selects the pinned toolchain.
You also need a C/C++ compiler and Python 3.11 or later.

The gate is `scripts/gate.sh`, in three tiers; each runs the ones before it
and stops at the first failure, naming it:

```bash
scripts/gate.sh fast      # before each commit: fmt, quality bar, clippy, exhaustiveness tests
scripts/gate.sh full      # before a pull request (`make gate`): the tests in both resident modes
scripts/gate.sh release   # before an image: release build, upgrade matrix, openCypher TCK,
                          # advisories, the image and its smoke test
```

CI runs the same checks, and its test job the same test runner.
`scripts/check-quality-bar.sh` caps the size of
known hotspot files. The tests run through `scripts/run-tests.py`, one
process per test binary: a binary killed by a signal (for memory, say) fails
by name instead of reading as passed, and a binary whose peak memory passes
its budget (4 GB by default) or a test that runs over 60 seconds fails the
run. `scripts/test-memory.py BINARY` ranks one binary's tests by peak memory.
A test that needs more than 60 seconds is listed in `scripts/slow-tests.txt`
with the reason. Set `GATE_WRAP` to run the heavy steps in a memory-capped
scope, for example `GATE_WRAP="systemd-run --user --scope -q -p MemoryMax=14G"`.

The individual `make` targets are:

```bash
make gate-fast   # scripts/gate.sh fast
make test        # scripts/run-tests.py
make fmt-check   # cargo fmt --all --check
make lint        # cargo clippy --workspace --all-targets --locked -- -D warnings
```

Build both local binaries with `cargo build -p rushdb-server --bins`, then use
`./target/debug/rushdb up --data-dir ./.rushdb-dev`. Do not commit database
directories, benchmark output, or credentials.

The target directory grows with every rebuild: each change to a crate builds
it and its test binaries into a new directory, and old ones stay.
`scripts/prune-target.py` removes those not used in the last day (they are
rebuilt if needed); with `--free-below-gb 20` it acts only when the disk is
that full, as a guard before a build. `CARGO_PROFILE_DEV_DEBUG=0` builds
tests without debug info, which keeps them several times smaller. The gate
prunes first (artifacts unused for `GATE_KEEP_HOURS`, default 6) and stops
before building when less than `GATE_MIN_FREE_GB` (default 40) is free: a
full disk fails builds, and a server below its free-disk threshold refuses
to become ready.

Changes to `install.sh`, `Dockerfile` or the release workflows can be checked
locally: `scripts/test-install.sh <release-dir>` runs the installer in clean
distribution containers against archives you built, and
`docker build -t rushdb .` builds the image from source.

## Toolchain policy

`rust-toolchain.toml` pins the toolchain used for development, linting, and
tests to the current stable (`1.100.0`). CI runs `quality`/`test`/`release` on
that same pinned toolchain so the `missing_docs`/`pedantic` gate is
deterministic and matches local runs. The declared MSRV
(`[workspace.package] rust-version = "1.100"`, the floor Grafeo 0.6 sets) is guaranteed separately by the
`msrv` CI job, which runs `cargo check --workspace --all-targets` on 1.100 —
clippy and rustfmt are deliberately **not** run on the MSRV toolchain. Build
the full `make gate` on the pinned toolchain; use the MSRV check only to verify
the compilation floor.


## Lint policy

The workspace denies `missing_docs` and `clippy::pedantic` (plus `perf`,
`redundant_clone`, and `needless_collect`) in `[workspace.lints]` (root
`Cargo.toml`); every crate opts in with `[lints] workspace = true`. Two
consequences:

- Every public module, type, field, variant, function, and constant needs a
  doc comment; public `Result`/panicking functions need `# Errors`/`# Panics`.
- Pedantic warnings are build failures. If a lint fires and the code is
  genuinely correct (for example an audited integer cast, or a cohesive
  long function that is the single source of truth for an ordered protocol),
  add a **local** `#[expect(clippy::lint, reason = "…")]` on the smallest
  enclosing item rather than a global `#[allow]`, so the suppression warns
  again if it becomes unnecessary.


## Change expectations

- Add a focused regression test for a bug and an observable behavior test for
  a feature.
- Treat a persisted command, WAL/replay rule, or protocol variant as a
  compatibility contract. Update its tests and documentation together: the
  [manual](docs/manual/README.md) is the reference (including the Protocol v1
  wire contract), so change the page that describes the behavior.
- Keep query-language claims precise. A parser accepting syntax is not a
  conformance guarantee.
- Add benchmark evidence when changing storage, planner, index, or hot-path
  behavior; include the methodology, machine details, and raw results in the
  pull request.
- Keep user/workspace, billing, prompts, and hosted product behavior outside
  the engine unless it is required database semantics.

## Pull requests

Explain the concrete user-visible behavior, migration or compatibility impact,
and validation performed. Keep changes focused. Maintainers may request a design note
for changes to durable format, authorization, query semantics, or public wire
contracts.

By submitting a contribution, you agree to license it under Apache-2.0.
