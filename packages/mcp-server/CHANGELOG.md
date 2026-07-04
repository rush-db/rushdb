# @rushdb/mcp-server

## 2.7.0

### Minor Changes

- 772d881: ### Read-only API keys

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

### Patch Changes

- Updated dependencies [772d881]
  - @rushdb/javascript-sdk@2.7.0

## 2.6.1

### Patch Changes

- 54a0767: Update docs
- Updated dependencies [54a0767]
  - @rushdb/javascript-sdk@2.6.1

## 2.6.0

### Minor Changes

- 1c9a994: Refactor Dashboard UI, add query lab, saved queries, optimize performance

### Patch Changes

- Updated dependencies [1c9a994]
  - @rushdb/javascript-sdk@2.6.0

## 2.5.1

### Patch Changes

- 1736715: Update login and onboarding
- e04be41: Fix docker image build issues
- Updated dependencies [1736715]
- Updated dependencies [e04be41]
  - @rushdb/javascript-sdk@2.5.1

## 2.5.0

### Minor Changes

- 58c6a45: Add SSO auth path

### Patch Changes

- Updated dependencies [58c6a45]
  - @rushdb/javascript-sdk@2.5.0

## 2.4.1

### Patch Changes

- ffd47f2: Dashboard UI fixes
- Updated dependencies [ffd47f2]
  - @rushdb/javascript-sdk@2.4.1

## 2.4.0

### Minor Changes

- 5de0ef8: Stability improvements

### Patch Changes

- Updated dependencies [5de0ef8]
  - @rushdb/javascript-sdk@2.4.0

## Unreleased

### Patch Changes

- Sync query-builder prompt and SearchQuery spec with production AI SearchQuery tuning: root-label selection, related-label traversal, alias-backed related counts, comparative related-count ordering, alias-safe `groupBy`, ontology array metadata, and fuzzy named-reference matching.

## 2.3.3

### Patch Changes

- 913c5cb: Minor fixes
- Updated dependencies [913c5cb]
  - @rushdb/javascript-sdk@2.3.3

## 2.3.2

### Patch Changes

- d8f63d7: Add docs search and indexing flow speed up
- Updated dependencies [d8f63d7]
  - @rushdb/javascript-sdk@2.3.2

## 2.3.1

### Patch Changes

- 1db6db1: Docs update
- Updated dependencies [1db6db1]
  - @rushdb/javascript-sdk@2.3.1

## 2.3.0

### Minor Changes

- 7a519f2: Add edge properties support, indexes suggestions, and minor fixes

### Patch Changes

- Updated dependencies [7a519f2]
  - @rushdb/javascript-sdk@2.3.0

## 2.2.1

### Patch Changes

- ec32832: Fix titles of dashboard pages shown on browser tabs
- Updated dependencies [ec32832]
  - @rushdb/javascript-sdk@2.2.1

## 2.2.0

### Minor Changes

- 2e9a82a: Docs update and SDK DX improvements

### Patch Changes

- Updated dependencies [2e9a82a]
  - @rushdb/javascript-sdk@2.2.0

## 2.1.1

### Patch Changes

- 5ee52f8: SQL migrations sync fix
- 149a2da: Minor fixes
- Updated dependencies [5ee52f8]
- Updated dependencies [149a2da]
  - @rushdb/javascript-sdk@2.1.1

## 2.1.0

### Minor Changes

- 3000fa6: Add relationship patterns suggestions

### Patch Changes

- 5d65783: OpenAI MCP domain verification
- Updated dependencies [3000fa6]
- Updated dependencies [5d65783]
  - @rushdb/javascript-sdk@2.1.0

## 2.0.7

### Patch Changes

- 6e712a7: MCP refresh tokens fix
- Updated dependencies [6e712a7]
  - @rushdb/javascript-sdk@2.0.7

## 2.0.6

### Patch Changes

- c555b56: Minor docs update
- 72ee13f: Dashboard help panel improvements
- Updated dependencies [c555b56]
- Updated dependencies [72ee13f]
  - @rushdb/javascript-sdk@2.0.6

## 2.0.5

### Patch Changes

- eb50f23: Fix OAuth MCP Flow
- Updated dependencies [eb50f23]
  - @rushdb/javascript-sdk@2.0.5

## 2.0.4

### Patch Changes

- bc52537: MCP OAuth API improvements
- Updated dependencies [bc52537]
  - @rushdb/javascript-sdk@2.0.4

## 2.0.3

### Patch Changes

- 7fdb94a: Fix MCP auth flow
- Updated dependencies [7fdb94a]
  - @rushdb/javascript-sdk@2.0.3

## 2.0.2

### Patch Changes

- 52b5f5c: Fix mcp image
- fc652b5: Fix docker image build
- Updated dependencies [52b5f5c]
- Updated dependencies [fc652b5]
  - @rushdb/javascript-sdk@2.0.2

## 2.0.1

### Patch Changes

- b1a491a: Fix chunks size in dashboard distro
- Updated dependencies [b1a491a]
  - @rushdb/javascript-sdk@2.0.1

## 2.0.0

### Major Changes

- 07920fb: Move website out of monorepo
- 3b25fad: Major update: native sematic search, ontology api, agentic skills

### Minor Changes

- 1e0acac: Decoupling billing from a platform
- d3156cb: Add native vector support and docs update
- bba13b1: Add editing functionality in dashboard
- 5043f13: Add skills package
- 6631e4a: Introducing select clause to SearchQuery
- b351ce4: Refactor dashboard
- 7bfea19: Add tutorials and BYOV feature
- edd8598: Improve separation between os and cloud versions
- f1ac305: Add oauth and mcp improvements
- 0786d74: Update docs portal
- 1275daf: Update docs portal & minor dx improvements

### Patch Changes

- Updated dependencies [1e0acac]
- Updated dependencies [d3156cb]
- Updated dependencies [07920fb]
- Updated dependencies [bba13b1]
- Updated dependencies [5043f13]
- Updated dependencies [3b25fad]
- Updated dependencies [6631e4a]
- Updated dependencies [b351ce4]
- Updated dependencies [7bfea19]
- Updated dependencies [edd8598]
- Updated dependencies [f1ac305]
- Updated dependencies [0786d74]
- Updated dependencies [1275daf]
  - @rushdb/javascript-sdk@2.0.0

## 1.19.1

### Patch Changes

- 78e6672: Fix deduplication issue for nested upsert
- Updated dependencies [78e6672]
  - @rushdb/javascript-sdk@1.19.1

## 1.19.0

### Minor Changes

- 865ba18: Add merge/upsert for bulk importing or single record creation

### Patch Changes

- Updated dependencies [865ba18]
  - @rushdb/javascript-sdk@1.19.0

## 1.18.0

### Minor Changes

- 488c1d1: createMany & importJson separation plus minor export and import improvements

### Patch Changes

- Updated dependencies [488c1d1]
  - @rushdb/javascript-sdk@1.18.0

## 1.17.0

### Minor Changes

- 71ca63f: Add MCP Server package

### Patch Changes

- Updated dependencies [71ca63f]
  - @rushdb/javascript-sdk@1.17.0
