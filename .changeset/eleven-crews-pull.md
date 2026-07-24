---
'@rushdb/javascript-sdk': minor
'rushdb-core': patch
'rushdb-dashboard': patch
'@rushdb/docs': patch
---

**Embedding index cold-start** (`rushdb-core`): `POST /ai/indexes` no longer requires the indexed property to exist in Neo4j before creating the index policy. When no property node exists (no records with that property have been created yet), type validation is skipped — the property will be created naturally when the first record carrying it is written. Previously the server threw `NotFoundException`.

**`DBRecordInstance.score` getter** (`@rushdb/javascript-sdk`): Records returned by `records.vectorSearch()` now expose a typed `score` getter (`number | undefined`) that mirrors `data.__score`. Regular `find()`/`search()` results return `undefined`.

**`records.createMany` vectors overload** (`@rushdb/javascript-sdk`): The `vectors` parameter now accepts both `VectorEntry[]` and `VectorEntry[][]`. A flat `VectorEntry[]` list is auto-wrapped into per-record entries, so single-record batches can omit the outer array nesting.

**Import form label reset** (`rushdb-dashboard`): The label field in the import data page is now cleared whenever the user goes back to the method-selection step, uploads a new file, or switches to the CSV editor — preventing stale labels from persisting across import iterations.

**Docs updated** (`docs`): `indexing.md` revised to reflect that index creation no longer requires the property to already exist. `write-with-vectors.mdx` and `bring-your-own-vectors.mdx` document the flat `VectorEntry[]` overload. `semantic-search.mdx` mentions the `record.score` convenience getter. TS reference docs (`RushDB.md`, `DBRecordInstance.md`) describe the flat `vectors` form and the `score` getter. Python reference docs (`RushDB.md`) mention the flat `vectors` form. All tutorial code examples (15 `.mdx` files) now prefer the `.score` getter over raw `__score` access. `manage-indexes.mdx` error reference updated to remove the stale 404 row (property no longer needs to exist). All incorrect `where: { __id: ... }` filter patterns replaced with `where: { $id: ... }` across 12 tutorial files (TS, Python, and Shell code blocks).
