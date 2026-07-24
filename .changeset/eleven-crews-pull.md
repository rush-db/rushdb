---
'@rushdb/javascript-sdk': minor
'rushdb-core': patch
'rushdb-dashboard': patch
---

**Embedding index cold-start** (`rushdb-core`): `POST /ai/indexes` no longer requires the indexed property to exist in Neo4j before creating the index policy. When no property node exists (no records with that property have been created yet), type validation is skipped — the property will be created naturally when the first record carrying it is written. Previously the server threw `NotFoundException`.

**`DBRecordInstance.score` getter** (`@rushdb/javascript-sdk`): Records returned by `records.vectorSearch()` now expose a typed `score` getter (`number | undefined`) that mirrors `data.__score`. Regular `find()`/`search()` results return `undefined`.

**`records.createMany` vectors overload** (`@rushdb/javascript-sdk`): The `vectors` parameter now accepts both `VectorEntry[]` and `VectorEntry[][]`. A flat `VectorEntry[]` list is auto-wrapped into per-record entries, so single-record batches can omit the outer array nesting.

**Import form label reset** (`rushdb-dashboard`): The label field in the import data page is now cleared whenever the user goes back to the method-selection step, uploads a new file, or switches to the CSV editor — preventing stale labels from persisting across import iterations.
