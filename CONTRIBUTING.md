# Contributing to RushDB

RushDB 3 is a database engine. Changes must preserve deterministic replay,
tenant isolation, durability, and transport-independent behavior.

## Local setup

Install Rust with `rustup`; `rust-toolchain.toml` selects the pinned toolchain.
You also need a C/C++ compiler. Then run:

```bash
make gate   # fmt-check + clippy + test
```

`make gate` is the same check CI runs. The individual targets are:

```bash
cargo build --workspace
make test        # cargo test --workspace --no-fail-fast --locked
make fmt-check   # cargo fmt --all --check
make lint        # cargo clippy --workspace --all-targets --locked -- -D warnings
```

Build both local binaries with `cargo build -p rushdb-server --bins`, then use
`./target/debug/rushdb up --data-dir ./.rushdb-dev`. Do not commit database
directories, benchmark output, or credentials.

Changes to `install.sh`, `Dockerfile` or the release workflows can be checked
locally: `scripts/test-install.sh <release-dir>` runs the installer in clean
distribution containers against archives you built, and
`docker build -t rushdb .` builds the image from source.

## Toolchain policy

`rust-toolchain.toml` pins the toolchain used for development, linting, and
tests to the current stable (`1.98.1`). CI runs `quality`/`test`/`release` on
that same pinned toolchain so the `missing_docs`/`pedantic` gate is
deterministic and matches local runs. The declared MSRV
(`[workspace.package] rust-version = "1.91"`) is guaranteed separately by the
`msrv` CI job, which runs `cargo check --workspace --all-targets` on 1.91 —
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
