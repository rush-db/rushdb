# @rushdb/mcp-server

## 2.9.1

### Patch Changes

- 0ac46fd: Rework `$cycle` and stabilize relationship/query intelligence

  - **`$cycle` is now a record-level operator**: `where: { $cycle: { type, direction, hops } }` — the value IS the traversal spec. Replaces the previous `KEY: { $cycle: true, $relation: {...} }` block form entirely (breaking: the old form now throws). Compiles to an `EXISTS` subquery so the engine stops at the first cycle found per record instead of enumerating every path — avoids exponential blowup on densely connected graphs. Default traversal hop cap (`RUSHDB_MAX_TRAVERSAL_HOPS`) lowered from 25 to 10 to keep worst-case queries within the transaction budget.
  - **Relationship pattern suggestions are now verified against live data**: candidates are proposed from schema names/types and LLM semantic judgment, then confirmed with a real graph probe before being surfaced — sampled schema values (which carry no signal on high-cardinality data) no longer gate or reject a suggestion.
  - SmartSearch prompts and specs (core, MCP server, skills, docs) made more data-agnostic and kept in sync across all four DSL mirrors.
  - Fix: CSV import no longer fails when a column parses to a JS `Date` (folded back to ISO string).
  - Fix: dashboard record search box text no longer leaks between project switches.

- Updated dependencies [0ac46fd]
  - @rushdb/javascript-sdk@2.9.1

## 2.9.0

### Minor Changes

- d05e56c: Add variable-length (multihop) traversal and cycle detection to SearchQuery

  **`$relation.hops`** — traverse a relationship pattern up to N hops in a single `where` block, without naming intermediate records:

  ```ts
  db.records.find({
    labels: ['EMPLOYEE'],
    where: {
      EMPLOYEE: {
        $alias: '$manager',
        $relation: { type: 'REPORTS_TO', direction: 'out', hops: { min: 1, max: 4 } },
        name: { $contains: 'Alice' }
      }
    }
  })
  ```

  `hops` accepts an exact count (`hops: 3`) or a range (`{ min?, max? }`, `min` defaults to 1). `type` and `direction` apply to every hop; the nested label and its criteria constrain only the endpoint record. Omitting `type` traverses any relationship — RushDB's internal property metadata edges are automatically excluded, so untyped traversal never leaves the user's data model.

  **`$cycle`** — find records sitting on a closed path back to themselves (fraud rings, circular ownership, dependency cycles):

  ```ts
  db.records.find({
    labels: ['ACCOUNT'],
    where: {
      $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } }
    }
  })
  ```

  `$cycle` is a record-level predicate: its value is the traversal spec itself (`type`, `direction`, `hops` — `hops` mandatory, `min` ≥ 2, defaulting to 2). A cycle has no separate endpoint, so it accepts no `$alias`, no property criteria, and no nested labels. Combine with `$not` to select records not on a cycle.

  **Traversal depth policy** — `hops.max` is capped per deployment via `RUSHDB_MAX_TRAVERSAL_HOPS` (default 10) on the shared cloud connection. Self-hosted deployments (`RUSHDB_SELF_HOSTED=true`) and projects with a custom external Neo4j allow unbounded traversal (`max` omitted), guarded by the existing transaction timeout.

  Also in this release:

  - New SDK types: `TraversalHops` and `TraversalRelationOptions` (TypeScript); matching `TypedDict`s in the Python SDK.
  - NL→query (smart search), the MCP server spec/tools, and the query-builder/data-modeling skills all understand `hops` and `$cycle`.
  - Docs: full operator reference in Where Operators, updated hierarchy-modeling tutorial, and a new "Detecting Fraud Rings" tutorial.

### Patch Changes

- Updated dependencies [d05e56c]
  - @rushdb/javascript-sdk@2.9.0

## 2.8.1

### Patch Changes

- 8fece4f: Remove redundant api guard
- Updated dependencies [8fece4f]
  - @rushdb/javascript-sdk@2.8.1

## 2.8.0

### Minor Changes

- cb1db5e: Bulk relationships overhaul, Zod validation migration, and a dashboard UI refresh.

  ### Bulk relationships API

  - **Single-pass hash join for `relationships.createMany` / `deleteMany`.** The key-join path used to re-match the target label once per source record, so an unscoped source side went quadratic — ~2,100 source records timed out at the gateway even when only a handful of pairs were written. The join now enumerates each side once and emits distinct pairs, so cost tracks `O(|source| + |target| + pairs)` instead of label size. Covered by a new scale regression e2e suite (`relationships.createMany.unscoped-scale`).
  - **`manyToMany` with join keys no longer requires `where` scoping.** Only the pure cross-product form (no `source.key`/`target.key`) still demands non-empty `where` filters on both sides. Shape errors now surface as structured 400s with guidance instead of raw 500s.
  - **Structured 408 on transaction timeouts.** Neo4j transactions run under a server-side time budget (default 55s, just under the 60s managed-gateway limit; configurable via `NEO4J_TRANSACTION_TIMEOUT_MS`). When the budget is exceeded the API answers 408 with actionable guidance instead of a generic 500 — or the gateway's body-less 504.
  - Relationship writes (attach, detach, create-many, delete-many) on managed instances are now metered consistently with the import path; external-DB and self-hosted projects are unaffected.
  - Bulk-relationships guide and REST reference updated to match.

  ### Validation migrated to Zod

  - **Core:** all REST request validation moved from Joi to Zod with byte-compatible behavior — unknown body keys are still tolerated and preserved, the `Request validation of body failed, because: ...` error format is unchanged, and acceptance/rejection semantics are locked in by a new black-box e2e suite (`validation.zod-migration`) plus unit smoke tests.
  - **Dashboard:** all forms moved from yup to Zod (`zodResolver`); `yup` and `joi` are dropped from the dependency tree.

  ### Dashboard UI

  - **Tailwind v4 migration** with a CSS-first generated theme (`theme:generate`), `@tailwindcss/vite`, and `tw-animate-css`; every element and component restyled against the new theme. The docs site moved to Tailwind v4 as well.
  - Record and property sheets refactored into docked side panels.
  - Colorful property type icons.

  ### Fixes and DX

  - Dev-mode startup now probes the actual configured bolt connection for readiness instead of guessing the Neo4j HTTP port, so remapped or multiple local instances no longer confuse the health check.
  - `.env.example` documents the new `NEO4J_TRANSACTION_TIMEOUT_MS` knob.

### Patch Changes

- Updated dependencies [cb1db5e]
  - @rushdb/javascript-sdk@2.8.0

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
