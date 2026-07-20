---
'rushdb-core': patch
---

Make schema recalculation scale with project size and stop AI endpoints from timing out on large datasets

- **Schema recalculation is now labels-first and incremental in shape**: instead of one monolithic full-graph scan inside a single transaction, the rebuild fans out small per-label statements — property inventory via pure VALUE-edge counts (no value reads), exact streaming min/max for number/datetime properties only, and split relationship topology/property queries — each running as its own short transaction on dedicated read sessions. No single unit of work can approach the server-side transaction time budget regardless of project size; on a 40k-record / 320k-value dataset a full forced rebuild completes in ~0.5s.
- **`POST /ai/schema` and `/ai/schema/md` no longer block or 408 on large projects**: a stale cache is served immediately while a single-flight background refresh recomputes it; concurrent requests share one rebuild instead of stacking full-graph scans. `force: true` still waits and now guarantees read-your-writes — a force call issued after a write never returns a schema computed before that write committed.
- **Smart Search (`POST /ai/search-query`) no longer fails with “transaction time budget” errors**: the endpoint releases the idle request-scoped transaction before the LLM round-trip (previously the transaction expired while waiting on the LLM and the request failed at commit), and query generation uses the cached schema instead of triggering a synchronous recompute.
- Behavior note: `isArray` detection for string/boolean properties is now based on the bounded sample window (first 100 records per property) rather than a full column scan; number/datetime `isArray`, all min/max ranges, record counts, and relationship counts remain exact.
