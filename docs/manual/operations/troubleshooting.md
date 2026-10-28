# Troubleshooting

Common problems, what causes them, and how to fix them. Error codes and
statuses are listed in [Error codes](../reference/error-codes.md).

## Server does not start

### `data directory tenancy mismatch`

```text
server bootstrap: data directory tenancy mismatch: stored multitenant, requested single; migrate into a new data directory
```

The data directory was created in the other tenancy mode. The mode is
fixed per directory. Start with the original mode (add or remove
`--multitenant`), or export the data and restore it into a new directory
created with the mode you want.

### `config.yml disagrees with explicitly supplied startup flags`

A flag or `RUSHDB_*` environment variable you passed has a different value
than the stored `config.yml`. The message names the flag and both values.
Either edit `config.yml` and drop the flag, or pass the stored value. Check
the environment too: a leftover variable in a systemd `EnvironmentFile`
counts as explicitly supplied.

### `refusing to bind ... to non-loopback listen address`

| Message | Fix |
|---|---|
| `refusing to bind HTTP (cleartext) to non-loopback listen address` | Add `--tls-cert`/`--tls-key`, or acknowledge with `--allow-insecure-listen`. |
| `refusing to bind pgwire/gRPC/Bolt (cleartext) ...` | Turn off the listed listeners that are enabled (`--pgwire-port 0` when `sql` is on; gRPC and Bolt ports 0), or pass `--allow-insecure-listen`. Only listeners that will start count. |
| `refusing to bind pgwire/gRPC/Bolt on a non-loopback multitenant listener` | Keep `--listen` on loopback and use a local proxy, or set those ports to 0. This cannot be overridden. |

See [Security](security.md#network-exposure).

### `cannot bind 127.0.0.1:3000: Address already in use`

Another process (often another RushDB node) uses the port. Stop it, or
change `server.port` in `config.yml`.

### Store is locked / maintenance command fails

One-shot commands (`--compact`, `--export`, `--backup-base`, `--verify`)
open the store themselves and fail while the server is running. Stop the
server first.

### `unsupported storage format version`

The data directory was written by a release with a different storage
format. See
[Upgrading between 3.x releases](upgrading-from-2x.md#upgrading-between-3x-releases).

### Other validation errors

`storage-cache + transaction-pool` must be below `managed-limit`, the token
must not be empty, TLS needs both files, and so on. The full list is in
[Startup validation rules](../reference/configuration-reference.md#startup-validation-rules).

## Requests fail

### 401 `unauthorized`

- The token header is missing or wrong. Use `Authorization: Bearer <token>`
  or `X-RushDB-Token: <token>`.
- A capability token is expired, signed with a different secret, lacks the
  permission (`write` does not include `read`), or is bound to a different
  tenant than `X-RushDB-Tenant`.
- pgwire: the password is the boot token; the user name is ignored.

### `missing_scope` / `scope_mismatch`

| Symptom | Cause | Fix |
|---|---|---|
| HTTP 400 `X-RushDB-Tenant is required in multitenant mode` | No tenant header. | Send `X-RushDB-Tenant`. |
| 403 `scope is required in multitenant mode` | Header present but `context.scope` missing from the envelope. | Set `context.scope` to the same value. |
| 403 `protocol scope does not match the authenticated transport scope` | Header and `context.scope` differ. | Make them equal. |
| 403 `scope must be omitted in single-tenant mode` | `context.scope` sent to a single-tenant server. | Remove it. |
| pgwire `3D000 database is required as tenant scope` | No database name on a multitenant server. | Connect with `dbname=<tenant>`. |

### "free disk below the configured threshold"

Free space on the data filesystem is below `disk-space-threshold-percent`
(10%). Writes are refused and `/ready` fails; reads continue.

1. Free space: remove old exports and backups from the data disk, move the
   WAL archive elsewhere, or grow the volume.
2. If the store file is much larger than live data, run compaction (see
   [Storage and compaction](storage-and-compaction.md#compaction)).
3. Check that the WAL is not held by a stalled consumer
   (`rushdb_wal_retained_bytes`, `rushdb_retention_protected_floor`).

Writes resume automatically once space is available. If the disk filled up
completely and `/ready` reports `durable publisher failed`, free space and
restart the server.

### 503 `server at request capacity` or 429 `too many request bytes in flight`

The server is shedding load instead of queueing without bound:

| Message | Limit |
|---|---|
| `server at request capacity; retry shortly` (`unavailable`) | Scheduler workers plus queue are full (`threads`). |
| `too many request bytes in flight; retry shortly` (`resource_exhausted`) | `http-inflight-body-bytes` is full. |
| `tenant concurrent request limit exceeded` (`unavailable`) | `--max-concurrent-requests-per-tenant`. |
| `tenant_capacity` (`unavailable`) | `tenant-resident-bytes` full of active tenants. |

Clients should retry 503 responses with jittered exponential backoff. If it
is sustained, add CPU (`threads` defaults to all cores), reduce request
size, batch writes, or add indexes so requests finish faster. Check
`rushdb_scheduler_inflight` against `rushdb_scheduler_capacity`.

### 413 request or response too large

- `request body exceeds the N-byte limit`: split the request (for ingest, use
  batches of about 500 documents), or raise `server.body-limit`.
- `response exceeds the N byte output budget`: request fewer records per
  page, use cursors, or project fewer fields.

### 429 `resource_exhausted`

| Message | Meaning | Fix |
|---|---|---|
| `query working set exceeded` | The query would materialize more than `query-memory-max`. | Add filters, indexes, or paging; or raise the limit. |
| `resource exhausted: transaction operation limit` (not retryable) | Over `tx-max-operations`. | Split the transaction. |
| `resource exhausted: transaction staging byte limit` (not retryable) | Over `tx-max-staged-bytes`. | Split the transaction, or raise the limit. |
| `resource exhausted: active transaction limit` | `tx-max-active` transactions are open. | Commit or roll back promptly; retry. |
| `storage resource exhausted: expanded mutation batch exceeds` (not retryable) | One commit is larger than `max-expanded-commit-bytes`. | Split the write. |
| write-lock wait timeout | `lock-timeout-ms` elapsed. | Retry; reduce concurrent writers. |

### Transaction expired

Transactions live `tx-default-ttl-seconds` (60 s) unless `begin` requests a
`ttlMs`, and never longer than `tx-max-age-seconds` (60 s).

- `commit` of an expired transaction returns `status: "expired"` with
  `retryable: false`; nothing was written. Start a new transaction.
- Staging into an expired or unknown transaction fails; the error may
  surface as `execution_failed` with the generic message
  `internal execution error`.
- `rollback` always works, even after expiry.

Keep transactions short. If you need longer ones, raise
`tx-max-age-seconds` and pass `ttlMs` on `begin`.

### `write_conflict` (409, retryable)

Two transactions wrote the same data concurrently; the later committer was
aborted (first committer wins). Retry the whole transaction with backoff.
Persistent conflicts mean many clients update the same records: reduce
contention or serialize those updates in the application.

### `conflict` (409, not retryable)

The request conflicts with current state: a record already exists, a label
changed, or a reference is stale. Re-read the current state; retrying the
same request will fail again.

### `language_disabled` (422)

The statement's language is not enabled. Add it to `languages` in
`config.yml` (or `--languages` on a new directory) and restart. pgwire
listens only when `sql` is enabled; Bolt only when `cypher` is.

### `unavailable`: writes rejected

| Message | Cause |
|---|---|
| `maintenance mode: writes are rejected until maintenance is cleared` | The server runs with `--maintenance` or `server.maintenance: true`. |
| `this node is a streaming-replication follower; writes must go to the leader` | Send writes to the leader, or promote this node. |

### 500 `internal execution error`

The server hid an internal fault's details from the client. Find the real
cause in the server log next to `suppressing internal execution error
detail` or `suppressing internal engine error detail`.

## Performance

### Slow filtered queries

A filter on a property without a declared index walks all records of the
first condition and checks the rest one by one. In measurements an ordered
index made two-condition filters about 5.8× faster.

1. Find slow calls in the log (`slow protocol call`, with `op` and
   `elapsed_ms`).
2. Run `describe` → `explain` for the query. An `accessPath` of
   `property_walk`, `label_scan` or `full_scan` means no index is used;
   `index:<name>` means one is. A `property_walk` lists the index to declare
   in `unindexedWalk.suggestedIndexes`. The `rushdb_unindexed_walks_total`
   metric counts queries that walked more than 10,000 owners.
3. Declare an ordered index on the filtered property:

   ```bash
   curl -s -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
     -d '{"protocolVersion":1,"operation":{"kind":"index","index":{"kind":"create",
          "definition":{"name":"user_status","label":"User","property":"status","kind":"ordered"}}}}' \
     http://127.0.0.1:3000/protocol/v1/execute
   ```

4. Index builds run in the background; check progress with `describe` →
   `indexes`. Queries stay correct while it builds.

See [Indexes](../indexes-and-search/indexes.md) and
[Query planning](../indexes-and-search/query-planning.md).

### Slow writes

- Batch: one ingest of many documents is far cheaper than many single-record
  writes, because every commit pays one WAL fsync.
- Check fsync latency: `rushdb_commit_phase_seconds{phase="wal_append_sync"}`.
- Check `rushdb_commit_phase_seconds{phase="admission_wait"}` for lock
  contention.
- See [Capacity planning](capacity-planning.md#sizing-write-throughput).

### High memory

- Compare `rushdb_allocator_resident_bytes` with your limit, and
  `rushdb_budget_used_bytes` per `pool`.
- Lower `query-memory-max`, `logical-cache-bytes`, or `storage-cache`;
  set `--rss-max` to have the server reclaim memory when it grows past a
  threshold.

### Vector search is slow or exact

`rushdb_vector_searches_total{path="exact",reason="budget"}` (and
`rushdb_ann_exact_fallbacks_total`) increasing means the vector cache could
not admit the ANN index, so searches ran exactly over every vector. The
index holds about 150 bytes per vector of the vector cache (a 768-dimension
space, measured 2026-10-19), and the vector cache shares
`logical-cache-bytes` (default 128 MiB) with text indexes and planner
statistics: one space of ~800K vectors fills it. Raise `logical-cache-bytes`
(1M vectors answered in ~60 ms with 384 MiB, against 4.5 s exactly).
`reason="annWarming"` means the index was being built in the background.
Search is also slower right after a large bulk bind until background
maintenance merges segments.

## Replication

| Symptom | Fix |
|---|---|
| Follower log: `leader requires resync from a fresh base; stopping` | Rebuild the follower from a new base backup. |
| Follower log: `poll failed, retrying` | Check network and that the follower presents the leader's boot token (`--replica-token`). |
| Follower restarted after promotion and is following again | Remove `replica-of` from its `config.yml`. |

See [Replication](replication.md).

## Collecting information for a bug report

- `rushdb --version`
- The server log around the failure (with `RUST_LOG=rushdb_server=debug` if
  you can reproduce it)
- `config.yml` (contains no secrets)
- A `/metrics` scrape
- `curl -s http://127.0.0.1:3000/ready`

## See also

- [Error codes](../reference/error-codes.md)
- [Monitoring](monitoring.md)
- [Configuration](configuration.md)
- [Limits](../reference/limits.md)
