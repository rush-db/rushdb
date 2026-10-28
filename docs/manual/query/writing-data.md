# Writing data

This page covers the native write operations: creating, updating, replacing,
retyping and deleting records; removing a property everywhere; upserts;
bulk deletes by query; and creating, removing and retyping relationships. It
also explains which writes are atomic and how to retry writes safely.

All writes are `mutate` operations sent to `POST /protocol/v1/execute`, except
document ingest (`ingest`) and multi-step transactions (`transaction`). Writes
need a token with write permission (see
[Authentication](../connectivity/authentication.md)).

## Operations at a glance

| `mutation.kind` | What it does | Response | Atomicity |
|---|---|---|---|
| `putRecord` (`mode: "create"`) | Create a record, or upsert when `options.mergeBy`/`mergeStrategy` is set | `record` | One commit |
| `putRecord` (`mode: "merge"`) | Update the given properties of a record by id; create it if missing and `label` is given | `record` | One commit |
| `putRecord` (`mode: "replace"`) | Replace all properties of a record by id | `record` | One commit (see [Replace](#replace-a-record)) |
| `deleteRecord` | Delete one record and its relationships | `mutation` | One commit |
| `deleteRecords` | Delete records matching a query | `mutation` | One commit per 500 records |
| `retypeRecord` | Change a record's label, keeping id, properties and relationships | `record` | One commit |
| `deleteProperty` | Remove a property from every record and relationship | `mutation` | One commit |
| `linkRecordsById` | Create relationships between records named by id (cross product) | `linked` | One commit |
| `linkRecordsPairs` | Create relationships for explicit source/target pairs | `linked` | One commit |
| `linkRecords` | Create relationships between records matched by label, property and predicate | `linked` | One commit |
| `unlinkRecordsById` | Delete relationships between records named by id | `mutation` | One commit |
| `unlinkRecords` | Delete relationships between records matched by selectors | `mutation` | One commit |
| `retypeRelations` | Rename the type of relationships between two labels | `mutation` | One commit |
| `statement` | A write in SQL, GQL, Cypher, Gremlin or MongoDB syntax | `statementMutation` | One commit per statement |

The `mutation` response is `{ "kind": "mutation", "affected": <n> }`, where
`affected` is the number of records, relationships, or property occurrences
the operation changed. The link operations answer
`{ "kind": "linked", "created": <n>, "skippedExisting": <m> }`: relationships
created, and requested pairs that were already linked by the same type.

Writes are rejected with `unavailable` while the server is in maintenance
mode, on a read-only replica, or when free disk space is below the configured
threshold (HTTP 503), on every transport.

## Create a record

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "mutate",
      "mutation": {
        "kind": "putRecord",
        "mode": "create",
        "label": "Person",
        "properties": { "name": "Ada", "born": "1815-12-10", "tags": ["math", "poetry"] },
        "options": {}
      }
    }
  }'
```

```json
{
  "protocolVersion": 1,
  "response": {
    "kind": "record",
    "record": {
      "id": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77",
      "label": "Person",
      "properties": { "name": "Ada", "born": "1815-12-10", "tags": ["math", "poetry"] },
      "propertyTypes": { "name": "string", "born": "datetime", "tags": "string" }
    }
  }
}
```

| Field | Required | Meaning |
|---|---|---|
| `mode` | yes | `create`, `merge`, or `replace`. |
| `label` | for `create` | The record's label. |
| `id` | no | An explicit id (dashed UUID or 32 hex digits). A malformed id is `invalid_request`; an id that already exists is `conflict`. Without it, RushDB assigns a time-ordered id. |
| `properties` | yes | A JSON object of property values. Nested objects are not properties; use [nested import](../data-model/nested-import.md) to turn them into related records. Names starting with `__` (`__id`, `__label`, `__proptypes`, `__score`, …) are reserved for the engine: a write that uses one is `invalid_request`. Names are compared after normalization, so `--x` is reserved too. `_id` is an ordinary property. |
| `vectors` | no | Inline vectors to store with the record, see [Vector search](../indexes-and-search/vector-search.md). |
| `options` | no | Write options, below. |

### Write options

| Option | Default | Effect |
|---|---|---|
| `suggestTypes` | `true` | Infer types from string values: ISO dates and timestamps become datetimes (see [Datetimes](filtering.md#datetimes)). |
| `convertNumericValuesToNumbers` | `false` | With `suggestTypes`, numeric strings such as `"42"` become numbers. |
| `skipEmptyValues` | `false` | Drop empty strings, and arrays that contain only empty strings. |
| `mergeBy` | absent | Upsert: the properties that identify an existing record. See [Upserts](#upserts). |
| `mergeStrategy` | absent | Upsert strategy: `append` or `rewrite`. |
| `merge` | `false` | Shorthand for `mergeBy: []` with `mergeStrategy: "append"`. |

Known options with the wrong JSON type are rejected (for example a string
`suggestTypes`). Arrays whose elements are not all of the same type are
stored as arrays of strings. An empty array is stored as `[]`, and `null` is
stored as an explicit null (see [Missing values and null](filtering.md#missing-values-and-null)).

## Upserts

Set `options.mergeBy` on a `create` to update an existing record of the same
label instead of creating a duplicate. A record matches when it has the same
values for every `mergeBy` property:

```json
{
  "kind": "putRecord",
  "mode": "create",
  "label": "Customer",
  "properties": { "email": "ada@example.com", "plan": "pro" },
  "options": { "mergeBy": ["email"], "mergeStrategy": "append" }
}
```

| `mergeBy` | `mergeStrategy` | Behavior |
|---|---|---|
| absent | absent | Always create. |
| `["email"]` | `append` or absent | Match on `email`; update the given properties and keep the others. Create when nothing matches. |
| `["email"]` | `rewrite` | Match on `email`; replace all properties of the matched record with the given ones. |
| `[]` (or `merge: true`) | `append` | Match on **all** given properties. |
| absent | `append` / `rewrite` | Match on all given properties. |

`mergeBy: ["__id"]` matches by record id: the record's `id` (or, in
[document ingest](../data-model/nested-import.md), the document's `__id`) is
looked up directly, and a miss creates the record with that id. With `[]`,
a document's `__id` is one of the matched keys. Any other `mergeBy` key
starting with `__` is `invalid_request`; `_id` is matched like any other
property.

Declare an [ordered index](../indexes-and-search/indexes.md) on a `mergeBy`
property of a large label: the upsert then reads the index. Without one it
reads the property's value on every record of the label that has it and
holds them in memory while the request runs. Those values count against
`query-memory-max` (default 256 MiB, or the request's smaller cap); over it
the upsert fails with `resource_exhausted` naming the index to declare. When
the server or the request sets an unindexed-walk limit
([request bounds](../architecture/resource-governance.md#request-bounds)),
an upsert over more records than that is refused up front with
`invalid_request`, naming the property and the index to declare.

The same options apply per record to [document ingest](../data-model/nested-import.md),
which is the recommended path for bulk upserts.

## Merge a record by id

`mode: "merge"` updates only the properties you send and leaves the others
unchanged. Sending `null` stores an explicit null; it does not remove the
property.

```json
{
  "kind": "putRecord",
  "mode": "merge",
  "id": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77",
  "label": "Person",
  "properties": { "city": "London" }
}
```

If no record has that id, `merge` creates one with that id under `label`.
Without `label`, a missing record is an error. This makes `merge` with an
explicit id an idempotent create-or-update.

## Replace a record

`mode: "replace"` removes every property of the record and writes the given
set. The label and relationships are kept; `id` is required.

```json
{
  "kind": "putRecord",
  "mode": "replace",
  "id": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77",
  "properties": { "name": "Ada Lovelace" }
}
```

Replace writes the new properties and removes every other property in one
commit: a reader sees the old record or the new one, and a refused replace
(for example one over `max-expanded-commit-bytes`) leaves the record as it
was. Inline `vectors` are bound in a second commit after it; if that
binding fails, the properties are already replaced and the request can be
retried.

## Delete a record

```json
{ "kind": "deleteRecord", "id": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77" }
```

The record's relationships are deleted with it, in the same commit. A missing
id returns `not_found`.

## Delete records by query

`deleteRecords` takes a [search query](search-queries.md) and deletes its
matches:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "mutate",
      "mutation": {
        "kind": "deleteRecords",
        "query": { "labels": ["Session"], "predicate": { "expiresAt": { "$lt": "2026-01-01T00:00:00Z" } } }
      }
    }
  }'
```

```json
{ "kind": "mutation", "affected": 1832 }
```

- `offset` matches are left in place, and `limit` caps how many records are
  deleted; without `limit`, every match is deleted. With `order`, the
  selection follows that order, so
  `{ "order": { "createdAt": "asc" }, "limit": 100 }` deletes the 100 oldest.
- The match set is determined once, at the start. Records inserted afterwards
  are not deleted; records that disappear before their turn are skipped.
- Records are deleted in pages of 500, each page one atomic commit. The
  operation as a whole is **not** one transaction: a concurrent reader can see
  a partially completed delete, and a cancelled or timed-out request leaves
  the already committed pages deleted.

> **Warning:** `{ "predicate": {} }` with no `labels` matches every record of
> the tenant. To erase a whole tenant, use `purgeTenant` (see
> [Tenants](../data-model/tenants.md)).

## Retype a record

`retypeRecord` changes a record's label atomically. The id, properties,
vectors and relationships are kept. `fromLabel` must equal the current label;
otherwise the request fails with `conflict`, which protects against
overwriting a concurrent reclassification.

```json
{ "kind": "retypeRecord", "id": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77", "fromLabel": "Lead", "toLabel": "Customer" }
```

## Delete a property everywhere

`deleteProperty` removes a property from every record and relationship of
the tenant, together with its schema entry, in one commit. `name` is matched
after [normalization](search-queries.md#property-names). `affected` is the
number of values removed.

```json
{ "kind": "deleteProperty", "name": "legacyScore" }
```

## Relationships

Relationships are typed and directed. The operations below take a
`direction`: `out` (default) creates `source → target`; `in` creates
`target → source`. Linking operations never create a second relationship of
the same type between the same two records in the same direction; existing
ones are skipped and not counted in `affected`.

### Linking by id

`linkRecordsById` links every source to every target:

```json
{
  "kind": "linkRecordsById",
  "command": {
    "relationType": "EMPLOYS",
    "direction": "out",
    "sourceIds": ["0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77"],
    "targetIds": ["0192f1d2-6c20-7c02-a1b3-77e1f0d2c3b4", "0192f1d2-6c21-7e4d-b5c6-d7e8f9a0b1c2"],
    "properties": { "since": 2021 }
  }
}
```

`linkRecordsPairs` creates one relationship per explicit pair, and a pair may
override the relationship type:

```json
{
  "kind": "linkRecordsPairs",
  "command": {
    "relationType": "KNOWS",
    "pairs": [
      { "sourceId": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77", "targetId": "0192f1d2-6c20-7c02-a1b3-77e1f0d2c3b4" },
      { "sourceId": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77", "targetId": "0192f1d2-6c21-7e4d-b5c6-d7e8f9a0b1c2", "relationType": "MENTORS" }
    ]
  }
}
```

Both operations resolve every id first: if any id does not exist, the call
fails with `not_found` and nothing is written. All new relationships are then
committed in one atomic commit. For large batches, send chunks of about 500
ids or pairs per call.

### Linking by matching properties

`linkRecords` selects the source and target records with a label, an
optional predicate and an optional join property:

```json
{
  "kind": "linkRecords",
  "command": {
    "relationType": "PLACED_BY",
    "source": { "label": "Order", "property": "customerEmail", "predicate": {} },
    "target": { "label": "Customer", "property": "email", "predicate": {} },
    "direction": "out",
    "properties": {}
  }
}
```

| Field | Meaning |
|---|---|
| `source`, `target` | `label` (required), `property` (join key), `predicate` ([Filtering](filtering.md) syntax). |
| `property` on both sides | A source and a target are linked when any value of the source property equals any value of the target property (type-strict; arrays compared element by element). Property names are matched exactly as stored. |
| `allowCartesian: true` without join properties | Link every selected source to every selected target. Both predicates must then be non-empty, and a record is never linked to itself. |
| `properties` | Properties stored on each new relationship. |

Without join properties and without `allowCartesian`, the request is
rejected. Every new relationship is written in one commit, so the call
creates all of them or none; a call whose relationships together exceed
`max-expanded-commit-bytes` is refused whole (narrow the selectors and
repeat it; existing relationships are skipped). Use
[`joinPairs`](relationships-and-traversal.md#property-overlap-joins) to count
the pairs before linking.

### Unlinking

`unlinkRecords` takes the same shape as `linkRecords` and deletes
relationships of `relationType` between each selected source and target, in
the same `direction` as linking: `out` (default) deletes `source → target`
edges, `in` deletes `target → source` edges.

`unlinkRecordsById` names the endpoints by id. Its `relationTypes` lists the
types to delete (omitted: every type), and its `direction` is `out` or `in`
(omitted: both directions); see
[Remove relationships](../data-model/relationships.md#remove-relationships).

Every deletion is in one commit: all of them happen, or none.

### Retype relationships

`retypeRelations` renames the type of every relationship whose endpoints are
a `sourceLabel` record and a `targetLabel` record:

```json
{
  "kind": "retypeRelations",
  "command": { "sourceLabel": "Order", "targetLabel": "Customer", "sourceType": "__RUSHDB__RELATION__DEFAULT__", "targetType": "PLACED_BY", "direction": "out" }
}
```

`sourceType` defaults to `__RUSHDB__RELATION__DEFAULT__`, the type nested
import uses when no relationship type is configured. Each matching
relationship is re-created with the new type, endpoints and properties
preserved, and the old one is removed. `affected` is the number of
relationships retyped.

The whole retype is one commit: a reader sees every relationship under its
old type or every one under its new type, and a refused retype changes
nothing.

## Bulk import

For loading many records, and nested JSON documents that become records
linked by relationships, use `ingest`:

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "ingest",
    "ingest": { "kind": "documents", "label": "Person", "documents": [{ "name": "Ada" }, { "name": "Alan" }], "options": {} }
  }
}
```

The response is `{ "kind": "ingested", "count": …, "relationships": …, "records": […] }`;
`records` is filled only when `options.returnResult` is `true`. Document
shapes, relationship-type options, upserts, and chunk sizing are described in
[Nested import](../data-model/nested-import.md).

### CSV import

`ingest.csv` imports CSV text: every data row becomes one record of `label`,
and the rows then go through the same path as document ingest, so the same
`options` apply.

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "ingest",
    "ingest": {
      "kind": "csv",
      "label": "Book",
      "data": "title,pages\nDune,412\n",
      "parseConfig": { "dynamicTyping": true },
      "options": {}
    }
  }
}
```

The response is the same `ingested` response. `parseConfig` is optional;
every field has a default:

| Field | Default | Meaning |
|---|---|---|
| `delimiter` | `","` | Field separator; any non-empty string without a line break or the quote character. |
| `header` | `true` | The first row names the fields. Without it, fields are named `column_1`, `column_2`, ... |
| `skipEmptyLines` | `true` | `true` skips lines with no characters, `"greedy"` also lines of whitespace-only fields, `false` keeps every line. |
| `dynamicTyping` | `options.suggestTypes` | Turn `true`/`TRUE`/`false`/`FALSE`, numbers below 2^53 in magnitude, and ISO datetimes with a time zone into typed values; an empty cell is then left out. |
| `quoteChar` | `"` | One character opening and closing a quoted field. |
| `escapeChar` | the quote character | One character that, before the quote character, makes it literal. |
| `newline` | detected | `\n`, `\r` or `\r\n`; any other value is ignored and the line ending is detected. |

Header names become property names: null bytes are removed, the name is
trimmed, whitespace runs become `_`, a name that does not start with an ASCII
letter or `_` gets a `_` prefix, a blank header cell's column is not
imported, and a repeated name gets a suffix (`a`, `a_1`, ...). A row whose
field count differs from the header's, or a malformed quoted field, fails the
whole import with `invalidRequest` and a `CSV parse error: ...` message;
nothing is written.

`vectors` (optional) carries inline vectors per row: entry `i` becomes data
row `i`'s `$vectors`, as in a [nested import](../data-model/nested-import.md).
It may not be longer than the number of data rows. A header with no data rows
imports nothing (`count: 0`).

Decoding reproduces Papa Parse 5.4 followed by the header clean-up above,
which is what CSV imports meant in RushDB 2.x, before CSV moved into the
engine. The differences: `quoteChar` and `escapeChar` must be one
character, and an unusable delimiter is rejected rather than guessed.

## Writes in other languages

SQL `INSERT`/`UPDATE`/`DELETE`, Cypher and GQL `CREATE`/`MERGE`/`SET`/`DELETE`,
Gremlin mutation steps, and MongoDB `insert`/`update`/`delete` commands are
sent as `mutate.statement`; each statement commits atomically and the
response reports `affected` and the `commitIndex` it landed at. See
[Query languages](../languages/README.md).

## Commit order and retries

Every successful write is assigned a position in the tenant's commit
sequence, and writes to a tenant are applied one at a time in that order.
Transactions and statement writes report this position as `commitIndex`.
Because commits are serialized per tenant, throughput comes from putting more
work in each call (ingest batches, `linkRecordsById` chunks), not from many
concurrent single-record writers.

The native `mutate` operations do not carry a client-supplied idempotency
key, so a blind retry of a write whose response was lost can apply it twice.
Make retries safe by construction:

| Goal | Safe pattern |
|---|---|
| Create exactly once | `create` with an explicit `id`: a retry after success returns `conflict`, which you can treat as success. Or `merge` with an explicit `id` and `label`. |
| Upsert by a business key | `create` with `options.mergeBy`. |
| Link exactly once | `linkRecordsById` / `linkRecordsPairs`: existing relationships are skipped. |
| Several writes as one unit, with retry discovery | A [transaction](transactions.md): committing again with the same transaction id returns the recorded outcome instead of executing twice. |

Retry `unavailable` and `resource_exhausted` errors (HTTP 503 and 429) with
jittered backoff. A response that is too large to send after a write has
committed is never reported as retryable: a single created record falls back
to `{ "kind": "mutation", "affected": 1 }`, and other committed writes fail
with a non-retryable `resource_exhausted`.

## Errors

| Error | Typical cause |
|---|---|
| `invalid_request` | Malformed envelope, missing `label` on create, missing `id` on merge/replace, malformed id, a reserved property name or `mergeBy` key, an upsert on a key without an index over the unindexed-walk limit. |
| `not_found` | The record or an endpoint id does not exist. |
| `conflict` | Create with an id that exists; `retypeRecord` with a stale `fromLabel`. |
| `unauthorized` | The token cannot write, or a data-access policy denies the label. |
| `resource_exhausted` | The write exceeds a server limit (request body, commit size) or the server is shedding load. |
| `unavailable` | Maintenance mode, read-only replica, low disk space, or capacity limits; retry later or against the leader. |

See [Error codes](../reference/error-codes.md) for the full taxonomy.

## See also

- [Transactions](transactions.md)
- [Nested import](../data-model/nested-import.md)
- [Records and labels](../data-model/records-and-labels.md)
- [Relationships](../data-model/relationships.md)
- [Write path](../architecture/write-path.md)
