---
'rushdb-dashboard': minor
'rushdb-core': minor
'rushdb-docs': minor
'@rushdb/javascript-sdk': minor
'@rushdb/mcp-server': minor
'@rushdb/skills': minor
---

Bulk relationships overhaul, Zod validation migration, and a dashboard UI refresh.

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
