# Quality gate for RushDB. Run `make gate` before every pull request.
#
# The workspace lint levels live in the root `Cargo.toml` under
# `[workspace.lints]` (`missing_docs` + `clippy::pedantic` are denied) and are
# opted into per crate via `[lints] workspace = true`. `make lint` therefore
# enforces them without extra flags; the `-D warnings` here additionally denies
# default `clippy::all` warnings.

CARGO ?= cargo

.PHONY: gate gate-fast gate-release fmt fmt-check lint test test-doc clean

## The gate in tiers (scripts/gate.sh): fast before each commit, full (the
## default) before a pull request, release before an image is handed over.
gate:
	scripts/gate.sh full

gate-fast:
	scripts/gate.sh fast

gate-release:
	scripts/gate.sh release

## Apply rustfmt.
fmt:
	$(CARGO) fmt --all

## Verify formatting without writing.
fmt-check:
	$(CARGO) fmt --all --check

## Deny every warning, including the workspace clone/allocation lints.
lint:
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings

## Full test suite (unit, integration, and doc tests), one process per test
## binary with memory and time budgets (scripts/run-tests.py).
test:
	python3 scripts/run-tests.py

## Doc tests only (nextest users must run this separately).
test-doc:
	$(CARGO) test --workspace --doc --locked

clean:
	$(CARGO) clean
