# HTTP API

The HTTP API is RushDB's primary interface. Every operation the engine
supports is available as a JSON request to a single endpoint,
`POST /protocol/v1/execute`, using the versioned Protocol v1 envelope. This
page describes the envelope, authentication headers, the operation families,
and how errors are reported. It is for anyone writing a client, an SDK, or a
proxy in front of RushDB.

## Quick example

```bash
export RUSHDB_TOKEN=local-token   # the server's --token (default: local-token)

curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "protocolVersion": 1,
    "requestId": "create-1",
    "operation": {
      "kind": "mutate",
      "mutation": {
        "kind": "putRecord",
        "mode": "create",
        "label": "NOTE",
        "properties": { "name": "alpha", "position": 7, "active": true },
        "options": {}
      }
    }
  }'
```

Read it back:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": {
        "kind": "records",
        "query": { "labels": ["NOTE"], "predicate": { "position": { "$gt": 5 } }, "limit": 10 }
      }
    }
  }'
```

## Endpoints

| Method and path | Auth | Purpose |
|---|---|---|
| `POST /protocol/v1/execute` | required | Execute one Protocol v1 request envelope. |
| `GET /health` | none | Liveness: status, engine, protocol version, enabled languages. Returns 503 with `"status": "maintenance"` in maintenance mode. |
| `GET /ready` | none | Readiness: 200 with `commitIndex` when the store can accept writes; 503 with a `reason` otherwise. |
| `GET /metrics` | `read` | Prometheus text exposition. |

The full list, including internal replication endpoints, is in the
[HTTP endpoint reference](../reference/http-endpoints.md).

## Headers

| Header | Purpose |
|---|---|
| `Authorization: Bearer <token>` | Credential: the bootstrap token or a capability token. |
| `X-RushDB-Token: <token>` | Alternative to `Authorization`. If both are sent, `X-RushDB-Token` wins. |
| `X-RushDB-Tenant: <scope>` | Tenant selector. Required on a multitenant server (at most 1024 bytes). Not used on a single-tenant server. |
| `Content-Type: application/json` | Request body format. |

Credentials are checked from the headers **before** the request body is read.
See [Authentication](authentication.md).

## Request envelope

```json
{
  "protocolVersion": 1,
  "requestId": "optional-correlation-id",
  "context": { "scope": "tenant-a" },
  "operation": { "kind": "query", "query": { "kind": "records", "query": {} } }
}
```

| Field | Required | Meaning |
|---|---|---|
| `protocolVersion` | yes | Must be `1`. Any other value fails with `unsupported_version` (HTTP 426). |
| `requestId` | no | Opaque string echoed in the response for correlation. |
| `context.scope` | multitenant only | The tenant scope. On a multitenant server it is required and must equal the `X-RushDB-Tenant` header. On a single-tenant server it must be omitted (or `null`); a scope sent to a single-tenant server is refused with 403 `scope_mismatch`. |
| `context.deadlineMs` | no | Execution deadline in milliseconds. It may only shorten the server's `--request-timeout-ms`. |
| `context.queryMemoryMaxBytes` | no | Working-memory cap of this request's queries, in bytes (positive). It may only lower `query-memory-max`. |
| `context.unindexedWalkMaxOwners` | no | Refuse a filter without a usable index that would check more records than this; `0` refuses every such filter. It may only lower `unindexed-walk-max-owners`. See [request bounds](../architecture/resource-governance.md#request-bounds). |
| `operation` | yes | The operation, tagged by `kind`. |

Envelopes are strict where it matters: the native query document rejects
unknown keys (a typo such as `where` instead of `predicate` is an
`invalid_request` naming the key), so a mistake cannot silently widen a query.

## Operations

`operation.kind` selects the family; each family has its own `kind` for the
specific action. The permission column is what a
[capability token](authentication.md#capability-tokens) must grant.

| `operation.kind` | Actions (`kind` of the inner object) | Permission |
|---|---|---|
| `query` | `records`, `cursor`, `record`, `relationships`, `propertyValues`, `vector`, `text`, `hybrid`, `joinPairs`, `vectorIndexCandidates`, `statement`, `exportCsv` | `read` |
| `describe` | `schema`, `labels`, `properties`, `queryLanguages`, `capabilities`, `buildInfo`, `explain`, `explainVector`, `indexes`, `vectorIndexes`, `vectorIndex` | `read` |
| `backup` | logical backup of the request's tenant (`{"kind": "backup", "request": {"scopes": []}}`) | `read` |
| `mutate` | `putRecord`, `deleteRecord`, `deleteRecords`, `retypeRecord`, `deleteProperty`, `linkRecords`, `unlinkRecords`, `linkRecordsById`, `unlinkRecordsById`, `linkRecordsPairs`, `retypeRelations`, `statement` | `write` |
| `ingest` | `documents` (nested JSON import), `csv` (CSV rows) | `write` |
| `index` | `create`, `drop`, `createVector`, `dropVector`, `upsertVectors`, `revalidateVector` | `write` |
| `transaction` | `begin`, `stage`, `commit`, `rollback` | `write` |
| `purgeTenant` | erase the request's tenant (`{"kind": "purgeTenant"}`) | `write` |

Where each family is documented:

- Queries: [Search queries](../query/search-queries.md),
  [Filtering](../query/filtering.md), [Aggregations](../query/aggregations.md),
  [Facets](../query/facets.md),
  [Sorting and pagination](../query/sorting-and-pagination.md).
- Writes and ingest: [Writing data](../query/writing-data.md),
  [Nested import](../data-model/nested-import.md).
- Transactions: [Transactions](../query/transactions.md).
- Statements: [Query languages](../languages/README.md).
- Indexes and search: [Indexes](../indexes-and-search/indexes.md),
  [Vector search](../indexes-and-search/vector-search.md),
  [Full-text search](../indexes-and-search/full-text-search.md),
  [Hybrid search](../indexes-and-search/hybrid-search.md),
  [Query planning](../indexes-and-search/query-planning.md).
- Backup and purge: [Backup and restore](../operations/backup-and-restore.md),
  [Tenants](../data-model/tenants.md).

## Response envelope

A successful request returns HTTP 200:

```json
{
  "protocolVersion": 1,
  "requestId": "create-1",
  "serverTimingMs": 2,
  "response": { "kind": "records", "records": [], "total": 0 }
}
```

| Field | Meaning |
|---|---|
| `protocolVersion` | The protocol version the server speaks. |
| `requestId` | Echoed from the request when supplied. |
| `serverTimingMs` | Server-side execution time. Informational only. |
| `response` | The result, tagged by `kind`. |

Common `response.kind` values:

| `kind` | Returned by | Shape |
|---|---|---|
| `records` | `query.records`, `query.cursor` | `records`, `total` (matches before pagination), `cursorId` while a cursor has more pages |
| `rows` | statements, aggregates, `$count` selections | `rows`, `total` |
| `record` | `query.record`, `mutate.putRecord`, `mutate.retypeRecord` | `record` |
| `relationships` | `query.relationships` | `relationships`, `total` |
| `propertyValues` | `query.propertyValues` | `property`, `values`, `distinctCount`, and mode-specific fields |
| `count` | `query.joinPairs` | `value` |
| `mutation` | delete, link, unlink, and retype-relations mutations | `affected` |
| `statementMutation` | `mutate.statement` | `rows`, `total`, `affected`, `commitIndex`, optional `status` |
| `ingested` | `ingest.documents`, `ingest.csv` | `count`, `relationships`, `records` |
| `csv` | `query.exportCsv` | `content` (the CSV text), `count` |
| `transactionBegun`, `transactionStaged`, `transactionOutcome` | `transaction.*` | `transactionId`; the outcome adds `status`, `commitIndex`, `reason`, `retryable` |
| `schema`, `labels`, `properties` | `describe.*` | catalog descriptions |
| `queryLanguages`, `capabilities`, `buildInfo` | `describe.*` | see [Discovering capabilities](../languages/README.md#discovering-capabilities-at-runtime) |
| `explain`, `explainVector` | `describe.explain*` | the plan |
| `backup`, `purgedTenant` | `backup`, `purgeTenant` | `commitIndex` and payload |

Compatibility rules for Protocol v1:

- Clients must ignore unknown fields: new fields and new variants are added
  without a protocol version change.
- Error codes are append-only; an existing code never changes meaning.
- A breaking change gets a new `protocolVersion`; a request for a version the
  server does not speak fails with `unsupported_version` (HTTP 426).

### Build info

`describe.buildInfo` reports the server identity, its listeners, and its
configured limits. Use it to negotiate instead of parsing version strings:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"describe","describe":{"kind":"buildInfo"}}}'
```

The `build` object contains `engine` (`rushdb-lmpg`), `version`,
`buildRevision`, `protocolVersion`, `transports` (each with `transport`,
`port`, and `tls`), `limits` (for example `maxRequestBytes`), and
`advertisedAddress` when one is configured.

## Errors

A failed operation returns a non-200 status and an error envelope:

```json
{
  "protocolVersion": 1,
  "error": {
    "code": "language_disabled",
    "message": "query language `Cypher` is not enabled",
    "retryable": false
  }
}
```

`code` is a stable, snake_case identifier; `retryable` tells you whether the
same request may succeed later. HTTP statuses:

| Status | Error codes |
|---|---|
| 400 | `invalid_request`, `protocol_violation`; `missing_scope` when the `X-RushDB-Tenant` header is missing on a multitenant server |
| 401 | `unauthorized` (bad or missing credential, insufficient permission, or a data-access policy denial) |
| 403 | `missing_scope`, `scope_mismatch` (envelope scope missing or not equal to the header) |
| 404 | `not_found` |
| 409 | `conflict`, `write_conflict` |
| 413 | request body over the limit (`invalid_request`), or response over the output budget (`execution_failed`) |
| 422 | `execution_failed`, `language_disabled`, `unsupported` |
| 426 | `unsupported_version` |
| 429 | `resource_exhausted` (a per-request budget, such as the query working set, or the in-flight request-body budget) |
| 499 | `cancelled` (the request deadline elapsed) |
| 500 | `execution_failed` with the message `internal execution error` (details are logged server-side, not returned) |
| 503 | `unavailable` (request capacity, maintenance mode, follower node, shutting down, a write while free disk is below `--disk-space-threshold-percent`) |

> **Note:** When authentication fails, the body is the short form
> `{"error":"unauthorized"}`, not the full envelope.

`retryable` is `true` for `resource_exhausted`, `unavailable`, and
`write_conflict` (except for fixed limits that a retry cannot fix, such as a
transaction that exceeds its staging limit). Retry those with jittered
exponential backoff. Do not retry 400, 401, 403, or 413.

The complete code list, with gRPC, SQLSTATE, and Bolt equivalents, is in
[Error codes](../reference/error-codes.md).

## Limits and admission

| Limit | Default | Behavior |
|---|---|---|
| Request body | 16 MiB (`server.body-limit` in `config.yml`) | Larger bodies are refused with 413 before they are read. |
| Response size | same as the body limit | A larger response is refused with 413; results are never silently truncated. |
| In-flight request bytes | 256 MiB (`--http-inflight-body-bytes`) | Over budget: immediate 429 `resource_exhausted`. |
| Concurrent requests | scheduler workers plus queue (`--threads`) | Over capacity: immediate 503 `unavailable`. |
| Per-tenant concurrency | unlimited (`--max-concurrent-requests-per-tenant`) | Over the cap: 503 `unavailable`. |
| Request deadline | none (`--request-timeout-ms`) | Long requests stop with 499 `cancelled`. |

Requests that cannot be admitted are refused immediately rather than queued,
so a client sees back-pressure at once.

### Guidelines for clients and proxies

An SDK or an application proxy in front of the server stays inside the
server's capacity by following these rules:

- **Connection pool.** Keep a bounded HTTP/1.1 keep-alive pool, sized to
  about twice the server's scheduler workers (`threads`, default: the host's
  CPU count). Every admitted request holds a worker or a queue slot; beyond
  workers plus queue the server sheds load with 503 instead of queueing.
  HTTP/1.1 keep-alive is the recommended bulk-load path.
- **Retries.** Retry 503 and 429 (`unavailable`, `resource_exhausted`) and
  409 `write_conflict` with jittered exponential backoff, and only when
  `retryable` is `true`. Do not retry 400, 401, 403, or 413.
- **Writes after a lost response.** Native `mutate` operations carry no
  client idempotency key, so a blind retry can apply a write twice. Use an
  explicit id, the link operations that skip existing relationships, or a
  transaction (re-sending `commit` with the same transaction id returns the
  recorded outcome). See
  [Writing data](../query/writing-data.md#commit-order-and-retries).
- **Payloads.** Stay under the 16 MiB body limit. Ingest in batches of
  about 500 documents and link in batches of about 500 ids: larger chunks
  risk 413 rejections and timeouts without loading faster. Enforce your own
  per-user rate and payload limits in the proxy before forwarding.
- **Writers.** All writes on a node share one commit pipeline, so scale bulk
  loads with bigger batches rather than more concurrent writers (see
  [Capacity planning](../operations/capacity-planning.md#sizing-write-throughput)).
- **Vectors.** Send `ef` (for example 64) for approximate search; without it
  the search is exact. Backfill embeddings with `upsertVectors` or inline
  vectors in batches; one batch is one commit.
- **Counts and large reads.** `$count`, and `total` on an unfiltered label,
  are answered from a stored counter in constant time. Page large reads with
  [cursors](../query/sorting-and-pagination.md), not growing offsets.
- **Application data.** Keep the proxy's own entities (users, accounts,
  billing) in the proxy's store, not in tenant keyspaces.

## HTTPS

Start the server with a PEM certificate chain and a PKCS#8 private key to
serve HTTPS on the same port:

```bash
rushdb serve --listen 0.0.0.0 --tls-cert /etc/rushdb/cert.pem --tls-key /etc/rushdb/key.pem
```

`describe.buildInfo` then reports `"tls": true` for the `http` transport. See
[Security](../operations/security.md).

## See also

- [Authentication](authentication.md)
- [HTTP endpoint reference](../reference/http-endpoints.md)
- [Error codes](../reference/error-codes.md)
- [gRPC](grpc.md), which carries the same envelope
- [Limits](../reference/limits.md)
