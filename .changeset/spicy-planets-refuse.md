---
'rushdb-core': patch
---

Fix side effects (project stats recount, schema cache recompute, relationship suggestions) computing against pre-commit state for writes made inside a user-defined transaction.

Writes wrapped in a client-managed transaction (`x-transaction-id`, e.g. every SDK Model write and any `db.tx.begin()` flow) triggered the post-response side-effect runner while the transaction was still open. The runner read the graph from a fresh transaction, saw none of the uncommitted data, and persisted wrong results — a fresh tx-wrapped import left project stats at `{"records":0}` and relationship analysis saw an empty schema, so no suggestions ever appeared.

Side effects now defer while the user-defined transaction is open and run once on `POST /tx/:txId/commit`, when the writes are actually visible. Non-transactional writes are unaffected.
