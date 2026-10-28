# Quality gate for RushDB. Run `make gate` before every pull request.
#
# The workspace lint levels live in the root `Cargo.toml` under
# `[workspace.lints]` (`missing_docs` + `clippy::pedantic` are denied) and are
# opted into per crate via `[lints] workspace = true`. `make lint` therefore
# enforces them without extra flags; the `-D warnings` here additionally denies
# default `clippy::all` warnings.

CARGO ?= cargo

.PHONY: gate fmt fmt-check lint test test-doc clean

## Format, lint, and test the whole workspace.
gate: fmt-check lint test

## Apply rustfmt.
fmt:
	$(CARGO) fmt --all

## Verify formatting without writing.
fmt-check:
	$(CARGO) fmt --all --check

## Deny every warning, including the workspace clone/allocation lints.
lint:
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings

## Full test suite (unit, integration, and doc tests).
test:
	$(CARGO) test --workspace --no-fail-fast --locked

## Doc tests only (nextest users must run this separately).
test-doc:
	$(CARGO) test --workspace --doc --locked

clean:
	$(CARGO) clean
