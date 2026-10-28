## Change

Describe the observable engine behavior and compatibility impact.

## Validation

- [ ] Focused regression or feature test
- [ ] `make gate` (fmt-check + clippy + tests) passes
- [ ] Workspace lint policy respected (no new global `#[allow]`; use a local
      `#[expect(clippy::lint, reason = "…")]` if a lint is genuinely wrong)
- [ ] Benchmark artifact attached, if storage/planner/index behavior changed

## Release impact

Describe protocol, persisted-format, migration, documentation, or security impact.
