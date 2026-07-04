---
'@rushdb/javascript-sdk': minor
'@rushdb/mcp-server': minor
'rushdb-dashboard': minor
'rushdb-core': minor
'rushdb-docs': minor
'@rushdb/skills': minor
---

### Read-only API keys

API keys now carry a permission level: **full access** (read & write, the default) or **read-only**. Read-only keys can query everything — search, labels, properties, relationships, aggregations, semantic search, exports — while every write endpoint is rejected with `403`, and the underlying database session is additionally opened in read-only mode as defense in depth. That makes read-only keys safe to embed in client-side code: public demos, dashboards, and prototypes can now query live data straight from the browser.

- Dashboard: pick the permission level when creating a key; key lists show each key's level.
- MCP server: automatically detects the key's access level and hides write tools for read-only keys, so agents only see what they can actually call.

### Bulk import: dramatically faster and more reliable

`createMany` / `importJson` / `importCsv` previously paid a fixed multi-second overhead per request that grew with project size — on larger projects imports could hit the 30s transaction ceiling and fail outright. This release removes that overhead entirely (read-model refresh now runs after the response instead of inside the request transaction) and rewrites the `mergeBy` upsert match to use index-capable property lookups instead of a full per-row scan.

In practice: batches that previously timed out now complete in seconds, concurrent bulk imports work, and upserts stay idempotent — verified end-to-end at tens-of-thousands-of-records scale. On top of that, the default server-side transaction budget was doubled from 30s to 60s (including the TTL cap for client-managed transactions), giving heavy operations twice the headroom.

### Clearer errors across the stack

- Server-side transaction timeouts now return `408` with an actionable message instead of an opaque `400` with no body.
- SDK errors now include the server's error message plus `status` and `body` fields on the thrown `Error` — no more bare `Error("400")`.
- `POST /records` with a malformed body (e.g. a mis-named `data` field) now fails fast with a descriptive `400` validation error instead of a raw database `500`.
- Fixed a crash where a dropped idle Postgres connection could take down a self-hosted instance; the connection pool now recovers automatically.

### Standalone e2e suite

New self-provisioning end-to-end harness at the repo root: `pnpm test:e2e` spins up Neo4j + Postgres in Docker, builds and boots the platform, provisions credentials, runs the full platform + SDK test suites (including vector search and raw-query flows), and tears everything down. Point it at an existing stack with `E2E_BASE_URL`. All JavaScript SDK e2e suites moved from `packages/javascript-sdk/tests` into `e2e/sdk` and run against the SDK source.
