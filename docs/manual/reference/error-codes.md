# Error codes

RushDB reports every failure with one stable error code, whatever the
transport. This page lists the codes, how each transport renders them, and
the HTTP statuses that occur outside the protocol taxonomy.

## Error body

On HTTP a protocol error has this shape:

```json
{
  "protocolVersion": 1,
  "error": {
    "code": "write_conflict",
    "message": "serializable_conflict on write at ci 4812",
    "retryable": true
  }
}
```

- `code` is one of the snake_case values below.
- `retryable` is `true` when the same request may succeed if retried later.
  It defaults from the code (`resource_exhausted`, `unavailable`,
  `write_conflict` are retryable) and is turned off for fixed limits that a
  retry cannot fix (for example a transaction over its operation limit).
- For an internal fault the message is replaced with
  `internal execution error`; the detail is written to the server log.

## Protocol error codes

| Code | Meaning | Retryable | HTTP | pgwire SQLSTATE | gRPC status | Bolt code |
|---|---|---|---|---|---|---|
| `unsupported_version` | `protocolVersion` is not 1. | no | 426 | `0A000` | `INVALID_ARGUMENT` | `Neo.ClientError.Request.Invalid` |
| `protocol_violation` | The request broke the protocol contract. | no | 400 | `08P01` | `INVALID_ARGUMENT` | `Neo.ClientError.Statement.SyntaxError` |
| `invalid_request` | Malformed request: bad JSON, unknown query key, invalid argument, oversized body. | no | 400 | `42601` | `INVALID_ARGUMENT` | `Neo.ClientError.Statement.SyntaxError` |
| `unauthorized` | Credential missing or wrong, capability lacks the permission, or a data-access policy denies the read. | no | 401 | `42501` | `UNAUTHENTICATED` | `Neo.ClientError.Security.Unauthorized` |
| `missing_scope` | Multitenant mode and no tenant scope supplied. | no | 403 | `3D000` | `UNAUTHENTICATED` | `Neo.ClientError.Security.Unauthorized` |
| `scope_mismatch` | `context.scope` differs from the transport scope, or a scope was sent to a single-tenant server. | no | 403 | `3D000` | `UNAUTHENTICATED` | `Neo.ClientError.Security.Unauthorized` |
| `not_found` | Record, index, property, or other target does not exist. | no | 404 | `42P01` | `NOT_FOUND` | `Neo.ClientError.Statement.EntityNotFound` |
| `conflict` | Constraint or state conflict (already exists, label changed, stale reference). Retrying the same request will fail again. | no | 409 | `23505` | `ALREADY_EXISTS` | `Neo.ClientError.Constraint.UniqueConstraintViolation` |
| `write_conflict` | Snapshot-isolation write conflict (first committer wins). The transaction was aborted; retry it. | yes | 409 | `40001` | `ABORTED` | `Neo.TransientError.Transaction.DeadlockDetected` |
| `cancelled` | The request deadline passed or the request was cancelled. | no | 499 | `57014` | `CANCELLED` | `Neo.ClientError.Transaction.TransactionTerminated` |
| `resource_exhausted` | A budget was exhausted: query memory, transaction staging, commit size, write-lock wait. | yes (no for fixed per-request limits) | 429 | `53200` | `RESOURCE_EXHAUSTED` | `Neo.TransientError.General.DatabaseUnavailable` |
| `unavailable` | Temporarily unable to serve: overload, tenant capacity, maintenance mode, follower read-only, shutting down. | yes | 503 | `57P03` | `UNAVAILABLE` | `Neo.TransientError.General.DatabaseUnavailable` |
| `execution_failed` | Execution failed for another reason, including internal faults. | no | 422 | `XX000` | `INVALID_ARGUMENT` | `Neo.DatabaseError.Execution.Failed` |
| `language_disabled` | The statement's language is not enabled on this server. | no | 422 | `0A000` | `INVALID_ARGUMENT` | `Neo.ClientError.Statement.SyntaxError` |
| `unsupported` | Recognized but not implemented by this engine (for example an unsupported clause or operator). | no | 422 | `0A000` | `INVALID_ARGUMENT` | `Neo.ClientError.Statement.SyntaxError` |

## HTTP statuses outside the taxonomy

Some responses are produced by the HTTP layer before or after protocol
execution. Their status does not always follow the table above.

| Status | Body | Cause | What to do |
|---|---|---|---|
| 401 | `{"error":"unauthorized"}` | No valid token or capability, or the capability lacks the needed permission. | Check the `Authorization` / `X-RushDB-Token` header. |
| 400 | protocol error `missing_scope` | Multitenant server and no `X-RushDB-Tenant` header. | Send the header. |
| 400 | protocol error `invalid_request` | `X-RushDB-Tenant` longer than 1024 bytes. | Shorten the scope. |
| 404 | `{"error":"route not found"}` | Unknown path. | — |
| 413 | protocol error `invalid_request` | Request body larger than `server.body-limit`. | Split the request. |
| 413 | protocol error `execution_failed` | The response would exceed the `server.body-limit` output budget. | Page the query (smaller `limit`, cursors). |
| 503 | protocol error `unavailable` | Scheduler at capacity (`server at request capacity; retry shortly`), node still booting, or a write while free disk is below `disk-space-threshold-percent`. | Retry with backoff; for disk, free space. |
| 429 | protocol error `resource_exhausted` | HTTP in-flight body budget full (`too many request bytes in flight`). | Retry with backoff. |
| 500 | protocol error `execution_failed`, `internal execution error` | Internal fault; detail in the server log. | Check the log. |

During boot, any request other than `/health` and `/ready` gets 503 with
`{"code":"unavailable","message":"node is starting (<phase>)","retryable":true}`
and a `Retry-After: 1` header.

## Replication endpoint

| Status | Body | Meaning |
|---|---|---|
| 400 | `{"error":"follower_id is required"}` | Missing query parameter. |
| 401 | `{"error":"unauthorized"}` | Not the leader's boot token. |
| 409 | `{"error":{"code":"resync_required","oldestAvailableLsn":N,...}}` | The requested history was pruned; rebuild the follower from a base backup. |
| 503 | `{"error":"promotion requested but the replication task has not stopped yet"}` | Promotion did not finish within 5 s; call again. |

## pgwire-specific SQLSTATEs

Besides the mapping above, the PostgreSQL wire adapter raises:

| SQLSTATE | Cause |
|---|---|
| `3D000` | Multitenant server and the connection has no `database` parameter. |
| `42601` | The SQL could not be parsed or described. |
| `22P02` | A result value cannot be encoded as the column's declared type. |
| `0A000` | A binary-format parameter of an unsupported type. |

The server answers `N` to an SSL request (no TLS on pgwire), so clients must
connect with `sslmode=disable` or `prefer`.

## Common messages

| Message (prefix) | Code | Meaning |
|---|---|---|
| `maintenance mode: writes are rejected until maintenance is cleared` | `unavailable` | Server started with `--maintenance`. |
| `this node is a streaming-replication follower; writes must go to the leader` | `unavailable` | Write sent to a follower. |
| `tenant concurrent request limit exceeded` | `unavailable` | Per-tenant cap or scheduler queue full. |
| `tenant_capacity` | `unavailable` | Resident-tenant budget full of active tenants. |
| `query language ... is not enabled` | `language_disabled` | Enable it with `--languages`. |
| `resource exhausted: transaction operation limit` | `resource_exhausted` (not retryable) | Over `tx-max-operations`. |
| `resource exhausted: transaction staging byte limit` | `resource_exhausted` (not retryable) | Over `tx-max-staged-bytes`. |
| `storage resource exhausted: expanded mutation batch exceeds` | `resource_exhausted` (not retryable) | Over `max-expanded-commit-bytes`. |
| `query working set exceeded` | `resource_exhausted` | Over `query-memory-max`. |
| `serializable_conflict` / `transaction_conflict` | `write_conflict` | Concurrent write to the same data; retry. |
| `policy-enforced reads are unavailable for this adapter` | `unauthorized` | A data-access policy is enforced and this read path cannot apply it. |
| `index DDL is a mutation; send it as mutate.statement` | `invalid_request` | Index DDL sent as a query. |
| `SQL parse failed` / `SQL statement is not read-only` | `invalid_request` | A SQL syntax error, or a write sent through `query.statement`. |
| `SQL write conflict` / `query-language write conflict` | `write_conflict` | Another write to the same tenant committed after the statement's snapshot; retry. |
| `query-language mutation graph exceeds the bounded budget` | `resource_exhausted` (not retryable) | A Cypher/GQL write over too large a tenant graph. |
| `invalid predicate` (for example `divide by zero`) | `invalid_request` | The query's own expression cannot be evaluated. |
| `cursorTtlMs must be greater than zero` / `ttlMs must be greater than zero` | `invalid_request` | Zero lifetimes are refused. |
| `unknown transaction` | `not_found` | Stage or rollback of a transaction that does not exist (or was settled). |
| `transaction ... expired` | `conflict` | Stage or rollback after the transaction's TTL lapsed. |
| `import options` / `invalid import option` | `invalid_request` | Wrongly typed ingest or put options. |
| `catalog detail does not support a predicate` | `invalid_request` | `detail: "catalog"` with a predicate. |
| `property ... does not exist` | `not_found` | `deleteProperty` of an unknown name. |
| `record ... already exists` | `conflict` | `putRecord` create with the id of an existing record. |
| `aggregate exceeds 1000 distinct groups` | `resource_exhausted` (not retryable) | Narrow the grouping or add filters. |

## See also

- [Troubleshooting](../operations/troubleshooting.md)
- [HTTP endpoints](http-endpoints.md)
- [Limits](limits.md)
- [Transactions](../query/transactions.md)
