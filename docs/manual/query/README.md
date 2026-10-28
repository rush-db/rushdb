# Querying and writing data

This section is the reference for RushDB's native data API: the JSON **search
query** document (`searchQuery`) you use to find, shape, aggregate, and count
records, and the native write operations you use to create, change, link, and
delete them. It is written for application developers who talk to RushDB over
HTTP (or any transport that carries the same protocol envelope, such as gRPC).

The native search query language is always enabled and cannot be switched off.
The other query languages run against the same data and are documented under
[Query languages](../languages/README.md): SQL, Gremlin, SQL/PGQ and MongoDB
are enabled by default; GQL and openCypher are opt-in.

## How requests look

Every operation is one JSON envelope sent with `POST /protocol/v1/execute`:

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
        "query": { "labels": ["Person"], "predicate": { "age": { "$gte": 18 } }, "limit": 10 }
      }
    }
  }'
```

- `protocolVersion` must be `1`.
- `operation.kind` selects the family: `query`, `mutate`, `describe`,
  `ingest`, `index`, `transaction`, `purgeTenant`, or `backup`.
- In single-tenant mode, omit `context.scope`. In multitenant mode, send the
  `X-RushDB-Tenant` header **and** the same value in `context.scope`; a
  missing scope is `missing_scope` and a mismatch is `scope_mismatch`. See
  [Tenants](../data-model/tenants.md).

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "X-RushDB-Tenant: acme" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "context": { "scope": "acme" },
    "operation": { "kind": "query", "query": { "kind": "records", "query": { "labels": ["Person"], "limit": 10 } } }
  }'
```

Examples in this section use single-tenant mode.

### Responses and errors

A successful call returns HTTP 200 with
`{ "protocolVersion": 1, "requestId": …, "serverTimingMs": …, "response": { "kind": …, … } }`;
`requestId` echoes the optional `requestId` you sent. A failed call returns
an error status with
`{ "protocolVersion": 1, "error": { "code": "invalid_request", "message": "…", "retryable": false } }`.
Error codes are snake_case; the common ones for queries and writes are
`invalid_request` (400), `not_found` (404), `conflict` and `write_conflict`
(409), `resource_exhausted` (429), `cancelled` (499), `unavailable` (503),
and `unsupported`/`execution_failed` (422). Retry only when `retryable` is
`true`.

Authentication (`Authorization: Bearer …` or `X-RushDB-Token`) is covered in
[Authentication](../connectivity/authentication.md); the full endpoint and
envelope reference is in [HTTP API](../connectivity/http-api.md).

## Pages in this section

1. [Search queries](search-queries.md) — anatomy of the query document:
   `labels`, `predicate`, `select`, `groupBy`, `order`, `offset`, `limit`,
   `cursorTtlMs`, and the response shapes.
2. [Filtering](filtering.md) — every predicate operator, logical
   combinators, type, null, array and datetime semantics.
3. [Relationships and traversal](relationships-and-traversal.md) — filtering
   by related records, direction, relationship type, variable-length hops,
   cycles, the relationship listing, and property-overlap joins.
4. [Sorting and pagination](sorting-and-pagination.md) — `order`,
   `offset`/`limit`, and snapshot-pinned cursors.
5. [Aggregations](aggregations.md) — `$count`, `$sum`, `$avg`, `$min`,
   `$max`, arithmetic, `$timeBucket`, `$collect`, grouping, and budgets.
6. [Facets](facets.md) — labels, properties, property values and
   relationship shapes narrowed by the same query document.
7. [Writing data](writing-data.md) — create, merge, replace, delete,
   retype, link and unlink, upserts, and bulk deletes.
8. [Transactions](transactions.md) — begin, stage, read-your-writes,
   commit, roll back, TTLs, conflicts, and retries.

## See also

- [Records and labels](../data-model/records-and-labels.md)
- [Properties and types](../data-model/properties-and-types.md)
- [Nested import](../data-model/nested-import.md)
- [Indexes](../indexes-and-search/indexes.md)
- [Error codes](../reference/error-codes.md)
