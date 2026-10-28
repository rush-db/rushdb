# Search queries

A **search query** is the JSON document RushDB's native query language is
built on. The same document selects records for listings, aggregations,
facets, explain plans, and bulk deletes, so learning it once covers most of
the native API. This page describes the document's fields, how RushDB
decides what kind of result to return, and the response shapes.

## A first query

Find up to ten `Person` records aged 18 or older, oldest first:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": {
        "kind": "records",
        "query": {
          "labels": ["Person"],
          "predicate": { "age": { "$gte": 18 } },
          "order": { "age": "desc" },
          "limit": 10
        }
      }
    }
  }'
```

Response:

```json
{
  "protocolVersion": 1,
  "serverTimingMs": 2,
  "response": {
    "kind": "records",
    "records": [
      {
        "id": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77",
        "label": "Person",
        "properties": { "name": "Ada", "age": 36, "born": "1989-12-10" },
        "propertyTypes": { "name": "string", "age": "number", "born": "datetime" }
      }
    ],
    "total": 1
  }
}
```

`total` is the number of records that match the query before `offset` and
`limit` are applied; `records` is the requested page.

## Document fields

The query document is an exact contract: an unknown top-level key is
rejected with `invalid_request` (HTTP 400) naming the key, never silently
ignored. A typo such as `where` instead of `predicate` therefore fails loudly
instead of widening the query.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `labels` | array of strings | `[]` (any label) | The record's label must be one of these. Labels are matched exactly (case-sensitive). |
| `predicate` | object | `{}` (no filter) | Filter on the record's properties, identity and relationships. See [Filtering](filtering.md) and [Relationships and traversal](relationships-and-traversal.md). `{}` and `null` both mean "no filter". |
| `select` | object | absent | Output shaping: projections, aggregate metrics, derived arithmetic, `$timeBucket`, `$collect`. See [Aggregations](aggregations.md). |
| `groupBy` | array of strings | `[]` | Group keys for aggregate output. See [Aggregations](aggregations.md#grouping). |
| `order` | `"asc"`, `"desc"`, or object | absent | Sort order. See [Sorting and pagination](sorting-and-pagination.md). |
| `offset` | non-negative integer | `0` | Number of matches (or groups) to skip. |
| `limit` | non-negative integer | absent (all matches) | Maximum number of records (or rows) to return. |
| `cursorTtlMs` | positive integer | absent | Start a snapshot-pinned cursor. Requires `limit`. See [Cursors](sorting-and-pagination.md#cursors). |

> **Note:** Inside a `$collect` expression the nested keys are spelled
> `where`, `orderBy`, `skip`, and `limit` (see
> [Aggregations](aggregations.md#collecting-related-records)). Only the top-level
> document uses `predicate`, `order`, and `offset`.

### Property names

Property names in `predicate`, `select`, `groupBy`, and `order` are matched
after normalization: ASCII letters are lower-cased and every character that
is not a letter or digit becomes `_`. `FirstName`, `firstname`, and
`FIRSTNAME` all address the same property; `first-name` and `first_name`
address another one. Responses spell a property as it was first written in the
tenant (see [Property names](../data-model/properties-and-types.md#property-names)).

## What kind of result you get

RushDB picks the result shape from the document:

| Document contains | Execution | Response `kind` |
|---|---|---|
| No `select`, no `groupBy` | Listing | `records` (full records, plus `total`) |
| `select` made only of `$collect` entries | Listing with nested collections attached to each record | `records` |
| Any other `select`, or a non-empty `groupBy` | Projection or aggregation | `rows` (plain JSON objects, plus `total`) |

`total` on a `rows` response is the number of matched root records, not the
number of rows.

### Plain projection

A `select` whose values are field references returns one row per matching
record containing just those fields:

```json
{
  "labels": ["STARSHIP"],
  "predicate": { "name": { "$contains": "falcon" } },
  "select": {
    "starship": "$record.name",
    "pilot_character_ids": "$record.pilot_character_ids"
  },
  "limit": 100
}
```

```json
{
  "kind": "rows",
  "rows": [
    {
      "starship": "Millennium Falcon",
      "pilot_character_ids": ["character_han_solo", "character_chewbacca"]
    }
  ],
  "total": 1
}
```

A field reference is a string: `"$record.field"` or `"$self.field"` (the
current record), or `"$alias.field"` for a related record bound with
`$alias` (see [Relationships and traversal](relationships-and-traversal.md#aliases)).

> **Warning:** Projections and per-record aggregations are computed as
> groups, and a single aggregation may create at most **1,000** distinct
> groups. A plain `select` projection that matches more than 1,000 records
> fails with `resource_exhausted` even when `limit` is smaller, because
> `offset`/`limit` are applied after grouping. Narrow the `predicate`, or use
> a listing (no `select`) with a [cursor](sorting-and-pagination.md#cursors)
> for large result sets.

## Counting

The fastest way to count one label is a `$count` aggregation with no
predicate; it is answered from the label catalog without touching records:

```json
{
  "labels": ["Person"],
  "select": { "n": { "$count": "*" } }
}
```

```json
{ "kind": "rows", "rows": [{ "n": 1204 }], "total": 1204 }
```

When nothing matches, `rows` is empty and `total` is `0`. A listing with
`"limit": 1` returns the same `total` together with one record.

## Fetching one record by id

Use the `record` query when you already know the id. Ids are accepted in
dashed UUID form or as 32 hex digits:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": { "kind": "record", "id": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77" }
    }
  }'
```

The response kind is `record`. A missing id returns `not_found` (HTTP 404).

## The record shape

Every record in a `records`, `record`, or write response has the same shape:

| Field | Meaning |
|---|---|
| `id` | The record id, dashed UUID form. Ids are time-ordered (UUIDv7), so ordering by id is creation order. |
| `label` | The record's label. |
| `properties` | The record's properties with their original spelling. Datetimes are rendered as ISO-8601 strings: `YYYY-MM-DD` when the instant is midnight UTC, otherwise `YYYY-MM-DDTHH:MM:SS.sssZ`. |
| `propertyTypes` | Map from property name to its type: `string`, `number`, `boolean`, or `datetime`. |

Properties hidden by an active data-access policy are removed from
`properties` before the record is returned.

## Exporting records as CSV

`query.exportCsv` takes the same query document as `query.records` and
returns the records it selects as CSV text:

```json
{ "kind": "query", "query": { "kind": "exportCsv", "query": { "labels": ["Book"], "limit": 100 } } }
```

The response is `{ "kind": "csv", "content": "...", "count": 2 }`. The first
column is `__id`, the second `__label`, then every property name of the
selected records in sorted order; rows are joined by `\r\n`. A missing value
is an empty cell, an array is its elements joined by `,`, and a cell holding
a comma, a quote, a line break, or a leading or trailing space is quoted.
`select`, `groupBy` and `cursorTtlMs` are rejected: the export is of records.
No match gives empty `content`.

## Limits that apply to every query

| Limit | Default | Effect |
|---|---|---|
| Per-query working set (`resources.query-memory-max`, `RUSHDB_QUERY_MEMORY_MAX`) | 256 MiB | A query that would materialize more fails with `resource_exhausted` instead of returning a partial result. |
| Response size (`server.body-limit`) | 16 MiB for `rushdb serve` and `rushdb up` | A larger response fails rather than being truncated. Page large results with `limit` or a cursor. |
| Distinct aggregation groups | 1,000 | See [Aggregations](aggregations.md#limits-and-budgets). |
| Request deadline (`--request-timeout-ms`) | off | When set, a query running past it is cancelled with `cancelled` (HTTP 499). |
| Records a filter without a usable index checks (`unindexed-walk-max-owners`) | no limit | A filter on a property with no ordered index walks every record that has it, which on millions of records takes minutes. When a limit is set, a larger walk is refused with `invalid_request` naming the index to declare. |

A request can make each of these bounds stricter with `context.deadlineMs`,
`context.queryMemoryMaxBytes` and `context.unindexedWalkMaxOwners` (see
[request bounds](../architecture/resource-governance.md#request-bounds)).

A query without `limit` returns every match, so always set `limit` on
listings whose size you do not control.

## Explaining a query

`describe.explain` takes the same document and reports the access path the
planner chose (for example `index:<name>` when a declared index answers the
predicate) together with its estimates:

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "describe",
    "describe": {
      "kind": "explain",
      "query": { "labels": ["Item"], "predicate": { "price": { "$lt": 10 } } }
    }
  }
}
```

See [Query planning](../indexes-and-search/query-planning.md).

## See also

- [Filtering](filtering.md)
- [Sorting and pagination](sorting-and-pagination.md)
- [Aggregations](aggregations.md)
- [Indexes](../indexes-and-search/indexes.md)
- [HTTP API](../connectivity/http-api.md)
- [Limits](../reference/limits.md)
