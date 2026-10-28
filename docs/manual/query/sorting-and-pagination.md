# Sorting and pagination

This page explains how to order query results, page through them with
`offset` and `limit`, and walk large result sets consistently with
snapshot-pinned cursors. It applies to the native
[search query](search-queries.md); statement languages have their own
`ORDER BY`/`LIMIT` clauses (see [Query languages](../languages/README.md)).

## Ordering: `order`

| `order` value | Sorts by |
|---|---|
| `"asc"` / `"desc"` | Record id. Ids are time-ordered, so this is creation order (oldest first / newest first). |
| `{ "age": "desc" }` | One property. |
| `{ "lastName": "asc", "age": "desc" }` | Several properties; earlier keys take priority, later keys break ties. |
| `{ "id": "asc" }` | The key `id` means the record id. |

Any direction value other than `"desc"` sorts ascending. Property names are
normalized the same way as in predicates (see
[Search queries](search-queries.md#property-names)).

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
        "query": { "labels": ["ITEM"], "order": { "rank": "asc" }, "offset": 0, "limit": 20 }
      }
    }
  }'
```

### Comparison rules

Ordering is a total order, so mixed or missing values never make a sort
fail:

| Values compared | Rule |
|---|---|
| Numbers | Numerically; integers and floats compare as one domain. |
| Datetimes | Chronologically (they are stored as instants). |
| Strings | Byte-wise, which is case-sensitive (`"B"` sorts before `"a"`). |
| A non-number and a number | The non-number sorts first (ascending). |
| Other values (booleans, arrays) | By their JSON text. |
| Missing property | Sorts first ascending, last descending. |

Records that compare equal keep a stable relative order, so repeated
requests page consistently.

Without `order`, the result order is unspecified. Always set `order` when
you page with `offset`.

## Offset pagination

`offset` skips matches and `limit` caps the page. `total` in the response is
always the exact number of matches, independent of `offset` and `limit`.

```json
{ "labels": ["ITEM"], "order": { "rank": "asc" }, "offset": 40, "limit": 20 }
```

How the engine executes a page:

- Without `order`, it stops collecting once the page is filled; the remaining
  matches are only counted for `total`.
- With `order`, it keeps the best `offset + limit` records seen so far, so
  memory grows with the page depth, not with the number of matches.
- Every request evaluates the query again from the beginning. Deep offsets
  cost proportionally more, and writes that land between requests can shift
  records across page boundaries.

For [aggregations](aggregations.md), `offset` and `limit` apply to the
output rows after grouping and ordering.

> **Tip:** Use offset pagination for small, user-facing pages. For exports,
> re-indexing, or any multi-page walk that must be consistent, use a cursor.

## Cursors

A cursor pages through **one snapshot** of the data. Every page sees the
database exactly as it was when the cursor was created: records written,
changed, or deleted afterwards do not appear in, shift, or disappear from
later pages, and every page reports the same `total`.

### Starting a cursor

Add `cursorTtlMs` and a finite `limit` to a records query:

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
          "labels": ["CURSOR_ITEM"],
          "order": { "rank": "asc" },
          "limit": 1000,
          "cursorTtlMs": 30000
        }
      }
    }
  }'
```

```json
{
  "kind": "records",
  "records": [ "… 1000 records …" ],
  "total": 4210,
  "cursorId": "0192f1e0-3b7d-7d55-9f0e-2a4c6b8d0e1f"
}
```

`cursorId` is present only when more records remain. If the first page
already contains everything, no cursor is created.

### Continuing a cursor

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": { "kind": "cursor", "cursorId": "0192f1e0-3b7d-7d55-9f0e-2a4c6b8d0e1f", "limit": 1000 }
    }
  }'
```

Each continuation may use a different `limit`. Keep requesting pages while
the response carries `cursorId`; when it is absent, the walk is complete and
the cursor is gone.

### Cursor rules

| Rule | Detail |
|---|---|
| Page size | `limit` must be between 1 and 10,000 when starting and when continuing. |
| Lifetime | `cursorTtlMs` must be greater than 0 and is capped by the server's maximum transaction age (`transactions.max-age-seconds`, default 60 s). The lifetime counts from creation; continuing a cursor does not extend it. |
| Scope | Only native `records` queries without `select` or `groupBy` can start a cursor. A cursor on an aggregation is rejected with `invalid_request`. |
| Tenant binding | A cursor belongs to the tenant that created it. Using it from another tenant behaves exactly like an unknown id. |
| Not found | Unknown, expired, exhausted, and foreign-tenant ids all return `not_found` (HTTP 404); the error does not reveal which. |
| Policy changes | If the tenant's data-access policy changes, open cursors stop working (`not_found`). |
| Server-local | A cursor lives in one server process. It does not survive a restart and is not shared between nodes. |
| Sequential use | Request pages one at a time; do not continue the same cursor concurrently. |
| Memory | Open cursors share a 64 MiB resident budget per server; when it is full, starting or continuing a cursor fails until others finish or expire. |

> **Note:** An open cursor keeps its snapshot's superseded row versions from
> being reclaimed until it is exhausted or expires. Use short TTLs and finish
> walks promptly. See [MVCC and snapshots](../architecture/mvcc-and-snapshots.md).

### Recommended pattern: scanning a whole label

To read every record of a label (exports, rebuilding an external index),
page a cursor with pages of about 1,000 records:

```text
1. records query: labels=[L], order={"id":"asc"}, limit=1000, cursorTtlMs=60000
2. while response.cursorId is present:
       cursor query: cursorId=<id>, limit=1000
```

Each continuation reuses the pinned snapshot and the `total` computed for
the first page, so it does not recount the matches.

Cursors are a feature of the native protocol only. SQL, Cypher, GQL,
Gremlin and MongoDB statements return their results in one response.

## See also

- [Search queries](search-queries.md)
- [Aggregations](aggregations.md)
- [MVCC and snapshots](../architecture/mvcc-and-snapshots.md)
- [Limits](../reference/limits.md)
