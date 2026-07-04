# SDK e2e suites

End-to-end tests for `@rushdb/javascript-sdk`, importing the SDK from source
(`packages/javascript-sdk/src`) and running against a live platform.

## Run

From the repo root:

```bash
pnpm test:e2e
```

No configuration needed: the harness (see `e2e/setup/`) boots Neo4j + Postgres +
platform/core, provisions a project and API key, and injects them as
`RUSHDB_API_KEY` / `RUSHDB_API_URL` before each suite.

To target an already-running stack instead:

```bash
E2E_BASE_URL=http://localhost:3390 pnpm test:e2e
```

The harness will try to provision credentials against that stack via admin login
(`E2E_LOGIN` / `E2E_PASSWORD`). If that isn't possible (e.g. a managed instance),
set `RUSHDB_API_KEY` / `RUSHDB_API_URL` in `packages/javascript-sdk/.env` — suites
skip themselves when no API key is available.

To run a single suite:

```bash
pnpm test:e2e -- e2e/sdk/relationships.createMany.e2e.test.ts
```

## Notes

- Each suite isolates its data with a unique per-run `tenantId` and cleans up after
  itself where possible, so they can be re-run against a persistent instance.
- `ai.search.e2e.test.ts` exercises server-side embedding generation and expects the
  server to be configured with `RUSHDB_EMBEDDING_MODEL` / `RUSHDB_EMBEDDING_DIMENSIONS` /
  `RUSHDB_EMBEDDING_API_KEY`; without them those tests fail/skip depending on the flow.
