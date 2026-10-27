# HTTP endpoints and protocol operations

Every HTTP route the server serves, and every Protocol v1 operation that can
be sent to `POST /protocol/v1/execute` (or carried by gRPC and Bolt). For
request and response shapes with examples, see the
[HTTP API guide](../connectivity/http-api.md).

## Routes

| Method | Path | Auth | Purpose | Success |
|---|---|---|---|---|
| `GET` | `/health` | none | Liveness. Reports engine name, protocol version, and the enabled query languages. | 200 `{"status":"ok",...}`; 503 `{"status":"maintenance"}` in maintenance mode |
| `GET` | `/ready` | none | Readiness: store open, data directory writable, free disk above threshold, optional vector-index warmup. | 200 `{"status":"ready","commitIndex":N,"dataDir":...}` |
| `GET` | `/metrics` | token or capability with `read` | Prometheus text exposition (`text/plain; version=0.0.4`). | 200 |
| `POST` | `/protocol/v1/execute` | token or capability (`read` or `write` by operation) | Execute one Protocol v1 request envelope. | 200 with a response envelope |
| `GET` | `/internal/replication/wal` | boot token only | Node-to-node: a follower fetches WAL records. Not a client API. | 200 `{"records":[{"lsn":N,"command_b64":"..."}],"caught_up":bool}` |
| `POST` | `/internal/replication/promote` | boot token only | Stop following the leader and accept writes. | 200 `{"promoted":true,"persisted":true}` |
| `POST` | `/internal/backup/base?out=PATH` | boot token only | Online base backup into `PATH` on the server's filesystem. | 200 `{"commitIndex":N,"files":N}` |
| `POST` | `/internal/usage` | boot token only | Usage figures of a list of tenants, and their total. | 200 `{"commitIndex":N,"tenants":[...],"total":{...}}` |
| any | any other path | — | — | 404 `{"error":"route not found"}` |

### Authentication headers

| Header | Value |
|---|---|
| `Authorization` | `Bearer <token>` |
| `X-RushDB-Token` | `<token>` (checked first; either header works) |
| `X-RushDB-Tenant` | Tenant scope, required on `/protocol/v1/execute` in multitenant mode (at most 1024 bytes). |

`<token>` is the boot token (`--token`) or a signed capability token when
`--capability-secret` is configured. See
[Security](../operations/security.md#capability-tokens).

### `/health`

```bash
curl -s http://127.0.0.1:3000/health
```

```json
{"status":"ok","engine":"rushdb-lmpg","protocolVersion":1,"queryLanguages":["searchQuery","sql"]}
```

While the node is still opening its store, `/health` returns 200 with
`{"status":"starting","phase":"..."}` (plain-HTTP listeners only).

### `/ready`

| Status | Body `status` / `reason` | Cause |
|---|---|---|
| 200 | `ready` | Serving. |
| 503 | `starting`, with the boot phase | The node is still booting (`starting`, `opening-storage`, `starting-engines`). |
| 503 | `maintenance` / `maintenance mode` | Maintenance mode is on. |
| 503 | `not_ready` / `store is shutting down` | Shutdown in progress. |
| 503 | `not_ready` / `durable publisher failed` (+ `error`) | The write path failed (for example the disk filled up mid-commit); restart required. |
| 503 | `not_ready` / `data directory is not writable` | A probe file could not be written to the data directory. |
| 503 | `not_ready` / `free disk below threshold` (+ `diskSpaceThresholdPercent`) | Free space below `disk-space-threshold-percent`. |
| 503 | `not_ready` / `vector indexes are still warming` | `--require-index-warmup` is on and a vector index is behind. |

### `/internal/replication/wal`

Query parameters: `follower_id` (required) and `since` (first LSN wanted,
default 1). Returns at most 2,000 records per call. Status 409 with
`{"error":{"code":"resync_required","oldestAvailableLsn":N}}` means the
leader no longer retains the requested history. See
[Replication](../operations/replication.md).

### `/internal/replication/promote`

Waits up to 5 seconds for the follower task to stop, then removes the
replica role from `config.yml` so a restart stays writable (`"persisted"`).
Returns `{"promoted":true,"already_writable":true}` if the node was not
following, or 503 if the follower did not stop in time.

### `/internal/backup/base`

Takes a physical base backup of the running server into `out` (which must
not exist; 409 otherwise). The store files are copied while quiescent at a
durable commit: reads continue and commits wait for the copy. The base is
registered as a `backup` retention consumer at its commit index. See
[Backup and restore](../operations/backup-and-restore.md#2-take-a-base-backup).

### `/internal/usage`

Reports usage per tenant for the tenant scopes you list, plus their sum.
The engine has no workspace, plan or billing concept: the caller decides
which tenants to group. This is a cross-tenant read, so it takes the boot
token and never an `X-RushDB-Tenant` scope.

```bash
curl -s -X POST http://127.0.0.1:3000/internal/usage \
  -H "X-RushDB-Token: $TOKEN" -H "Content-Type: application/json" \
  -d '{"tenants":["project-1","project-2"]}'
```

```json
{
  "commitIndex": 1842,
  "tenants": [
    {"tenant": "project-1", "exists": true, "records": 3, "relationships": 2,
     "labels": 1, "properties": 2, "logicalBytes": 1673, "vectors": 3,
     "vectorIndexes": [{"index": "doc_embedding", "vectors": 3}]},
    {"tenant": "project-2", "exists": true, "records": 2, "relationships": 1,
     "labels": 1, "properties": 2, "logicalBytes": 802, "vectors": 0,
     "vectorIndexes": []}
  ],
  "total": {"records": 5, "relationships": 3, "labels": 2, "properties": 4,
            "logicalBytes": 2475, "vectors": 3}
}
```

| Field | Meaning |
|---|---|
| `records`, `relationships` | Live records and relationships. |
| `labels` | Labels with at least one live record. |
| `properties` | Property ids (name and type) carried by at least one live record. A name stored with two types counts twice, as `describe` lists it. |
| `logicalBytes` | Key + value bytes of every row the tenant stores: data, derived rows (counters, presence, value dictionary, declared index entries), stored vectors and security rows. It does not include ANN index segment files on disk. Not the file size: redb pages run about 40–50% full, so the file is roughly twice this (see [capacity planning](../operations/capacity-planning.md)). |
| `vectors` | Ready vectors summed over the tenant's vector indexes; `vectorIndexes` lists them per index. |
| `exists` | `false` for a scope the tenant directory does not know (never written, or purged). Its figures are zero. The read never registers a scope. |

All figures come from one snapshot (`commitIndex`). Each is a counter that
the commit changing it updates, so a read costs one lookup per figure and
vector index for each tenant, whatever the tenants' sizes. Single-tenant
mode takes `{}` and reports the one tenant with `"tenant": null`. A request
lists at most 10,000 tenants. Errors: 400 for an empty list in multitenant
mode, a list in single-tenant mode, or an unknown body field; 401 without
the boot token.

## Request envelope

```json
{
  "protocolVersion": 1,
  "requestId": "optional-correlation-id",
  "context": { "scope": "tenant-a", "deadlineMs": 5000 },
  "operation": { "kind": "query", "query": { "kind": "record", "id": "..." } }
}
```

| Field | Notes |
|---|---|
| `protocolVersion` | Must be `1`; anything else fails with `unsupported_version`. |
| `requestId` | Optional; echoed on the response. |
| `context.scope` | Single-tenant: must be omitted. Multitenant: required and must equal the transport scope (the `X-RushDB-Tenant` header on HTTP). |
| `context.deadlineMs` | Optional execution deadline in milliseconds; it may only shorten the server's `--request-timeout-ms`. |
| `context.queryMemoryMaxBytes` | Optional working-memory cap of the request's queries (positive); it may only lower `query-memory-max`. |
| `context.unindexedWalkMaxOwners` | Optional limit on the records a filter without a usable index may check; `0` refuses every such filter. It may only lower `unindexed-walk-max-owners`. See [request bounds](../architecture/resource-governance.md#request-bounds). |

Successful responses carry `protocolVersion`, `requestId`, `serverTimingMs`
and `response` (tagged by `kind`). Errors carry
`{"protocolVersion":1,"error":{"code":...,"message":...,"retryable":...}}`;
see [Error codes](error-codes.md).

## Protocol v1 operations

`operation.kind` selects the family; the nested object's `kind` selects the
operation. The **Metric route** column is the `route` label value used in
`rushdb_requests_total` and related metrics. **Permission** is the
capability scope needed.

### `query` (permission `read`)

| `query.kind` | Purpose | Metric route |
|---|---|---|
| `statement` | Run a statement in an enabled language (`sql`, `gql`, `cypher`, `gremlin`, `sqlPgq`, `mongo`). | `query.statement` |
| `records` | Native search query document. | `query.records` |
| `cursor` | Continue a snapshot-pinned records cursor (`cursorId`, `limit`). | `query.cursor` |
| `record` | Fetch one record by `id`. | `query.record` |
| `relationships` | Query relationships. | `query.relationships` |
| `propertyValues` | Distinct values of a property (`mode`: `values`, `count`, `range`, `topK`; optional `search`). | `query.propertyValues` |
| `searchValues` | A value search across every property, with the label and property holding each match. | `query.searchValues` |
| `vector` | Vector similarity search. | `query.vector` |
| `joinPairs` | Pairwise join of two record sets. | `query.joinPairs` |
| `vectorIndexCandidates` | Records not yet bound to a vector index (default `limit` 500). | `query.vectorIndexCandidates` |
| `text` | Full-text search against a declared text index (default `limit` 10). | `query.text` |
| `hybrid` | Graph + vector + full-text query fused by reciprocal-rank fusion. | `query.hybrid` |

### `mutate` (permission `write`)

| `mutation.kind` | Purpose | Metric route |
|---|---|---|
| `statement` | Write statement in an enabled language. | `mutate.statement` |
| `putRecord` | Create, merge or replace one record (`mode`: `create`, `merge`, `replace`). | `mutate.putRecord` |
| `deleteRecord` | Delete a record by id. | `mutate.deleteRecord` |
| `retypeRecord` | Change a record's primary label. | `mutate.retypeRecord` |
| `deleteRecords` | Delete every record matching a query. | `mutate.deleteRecords` |
| `deleteProperty` | Remove a property from the whole tenant. | `mutate.deleteProperty` |
| `linkRecords` / `unlinkRecords` | Link or unlink records matched by label, key and predicate. | `mutate.linkRecords` / `mutate.unlinkRecords` |
| `retypeRelations` | Rename a relationship type between two labels. | `mutate.retypeRelations` |
| `linkRecordsById` / `unlinkRecordsById` | Link or unlink explicit record ids. | `mutate.linkRecordsById` / `mutate.unlinkRecordsById` |
| `linkRecordsPairs` | Batch-link explicit (source, target) pairs. | `mutate.linkRecordsPairs` |

### `describe` (permission `read`)

| `describe.kind` | Purpose | Metric route |
|---|---|---|
| `schema` | Labels, properties and relationship shapes for a query. | `describe.schema` |
| `labels` | Labels present on the records matching a query. | `describe.labels` |
| `properties` | Properties of the records matching a query (`detail`: `full` or `catalog`; `skip`, `limit`). | `describe.properties` |
| `queryLanguages` | Enabled languages and their capability descriptors. | `describe.queryLanguages` |
| `capabilities` | Feature flags for client negotiation (the catalog reads below advertise `valueSearch`, `searchValues`, `describeNames`, `schemaSummary`, `valueOverlap`). | `describe.capabilities` |
| `buildInfo` | Engine build information. | `describe.buildInfo` |
| `explain` | Query plan for a search query. | `describe.explain` |
| `explainVector` | Plan for a vector query. | `describe.explainVector` |
| `indexes` | Declared property and text indexes. | `describe.indexes` |
| `vectorIndexes` | Vector indexes. | `describe.vectorIndexes` |
| `vectorIndex` | Statistics for one vector index (`name`). | `describe.vectorIndex` |
| `schemaSummary` | Every label with its properties, distinct counts, ranges and few values in one read. | `describe.schemaSummary` |
| `names` | Labels and properties spelled like a name (`text`). | `describe.names` |
| `valueOverlap` | For property pairs, how much of the source's values the target holds (evidence for join suggestions). | `describe.valueOverlap` |

### `ingest` (permission `write`)

| `ingest.kind` | Purpose | Metric route |
|---|---|---|
| `documents` | Import nested JSON documents as records and relationships. | `ingest.documents` |

### `index` (permission `write`)

| `index.kind` | Purpose | Metric route |
|---|---|---|
| `create` / `drop` | Declare or drop an `ordered` (default) or `text` index on a label property. | `index.create` / `index.drop` |
| `createVector` / `dropVector` | Declare or drop a vector index. | `index.createVector` / `index.dropVector` |
| `upsertVectors` | Bind vectors to records in a vector index. | `index.upsertVectors` |
| `revalidateVector` | Revalidate a vector index's bindings. | `index.revalidateVector` |

### `transaction` (permission `write`)

| `transaction.kind` | Purpose | Metric route |
|---|---|---|
| `begin` | Start a transaction (`ttlMs`, `autocommit`). | `transaction.begin` |
| `stage` | Stage one mutation (`createRecord`, `updateRecord`, `retypeRecord`, `deleteRecord`, `createRelationship`, `getRecord`, `statement`). | `transaction.stage` |
| `commit` | Commit; returns `status` `committed`, `aborted`, `expired` or `unknown`, with `retryable`. | `transaction.commit` |
| `rollback` | Discard. Always allowed, even after the deadline or under memory pressure. | `transaction.rollback` |

### Tenant and backup operations

| `operation.kind` | Permission | Purpose | Metric route |
|---|---|---|---|
| `purgeTenant` | `write` | Delete every key of the request's tenant. | `purgeTenant` |
| `backup` | `read` | Logical backup of the request's scope as JSONL inside the response (`request.scopes`; in multitenant mode only the caller's own scope). | `backup` |
| `policy` | `operator` (boot token only; never a capability token) | Read (`{"kind":"get"}`) or install (`{"kind":"put","policy":{…},"enforced":true}`) the request tenant's data-access policy. | `policy.get` / `policy.put` |

### Operations refused by mode

| Situation | Refused operations | Error |
|---|---|---|
| Maintenance mode | Writes (`mutate`, `ingest`, `index`, `purgeTenant`, policy `put`, transaction `stage`/`commit`) | `unavailable` |
| Streaming-replication follower | `mutate`, `ingest`, `index`, `purgeTenant`, policy `put`, transaction `stage`/`commit` | `unavailable` ("writes must go to the leader") |
| Free disk below threshold | Every write operation, on every transport | HTTP 503, code `unavailable` |
| Language not enabled | `statement` in that language | `language_disabled` |

## Other transports

The same envelopes travel over:

| Transport | Endpoint | Auth | Tenant scope |
|---|---|---|---|
| gRPC | service `rushdb.v1.Database` on `--grpc-port` | metadata `authorization: Bearer <token>` (boot token) | metadata `x-rushdb-tenant` |
| Bolt | `--bolt-port` (graph statements only) | HELLO `scheme: basic`, `credentials: <token>` | HELLO `db` field |
| PostgreSQL wire | `--pgwire-port` (SQL only) | password = boot token; user name ignored | `database` connection parameter |

Their request metrics use the routes `transport.execute` (gRPC and Bolt) and
`pgwire.execute`.

## See also

- [HTTP API guide](../connectivity/http-api.md)
- [Error codes](error-codes.md)
- [Monitoring](../operations/monitoring.md)
- [Limits](limits.md)
