# Configuration reference

Every server setting (`rushdb serve`, `rushdb up`): command-line flag, environment variable,
`config.yml` key, type, default, and meaning. For how the three sources
interact and for common setups, see
[Configuration](../operations/configuration.md).

## Conventions

- **Byte sizes** accept a bare integer (bytes) or an integer with one of the
  suffixes `B`, `KiB`, `MiB`, `GiB` (for example `256MiB`). Other suffixes
  (`KB`, `MB`, `GB`) are rejected. In `config.yml` a byte size can be a YAML
  number or a string.
- **Booleans** given as flags take no value (`--maintenance`). Set the
  matching environment variable to `true` to enable it.
- **`config.yml` keys** are kebab-case and live in one of the sections
  `server`, `tenancy`, `resources`, `transactions`, or `languages`. The
  `resources` and `transactions` sections reject unknown keys, so a typo is a
  startup error rather than a silently ignored setting.
- A setting marked **CLI/env only** is never written to `config.yml` and is
  read from the command line or environment on every start.
- `0` means "disabled" or "unbounded" where the description says so.

## Server and listeners

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--port` | — | `server.port` | u16 | `3000` | HTTP port (Protocol v1, `/health`, `/ready`, `/metrics`). |
| `--listen` | `RUSHDB_LISTEN` | `server.listen-addr` | string | `127.0.0.1` | Bind address for every listener. A non-loopback address requires TLS or `--allow-insecure-listen` (see [Security](../operations/security.md)). |
| `--tls-cert` | `RUSHDB_TLS_CERT` | `server.tls-cert` | path | none | PEM certificate chain for HTTPS. Must be set together with `--tls-key`. |
| `--tls-key` | `RUSHDB_TLS_KEY` | `server.tls-key` | path | none | PEM private key (PKCS#8) for HTTPS. |
| `--pgwire-port` | — | `server.pgwire-port` | u16 | `5432` | PostgreSQL wire-protocol port. The listener is bound only when the `sql` language is enabled. |
| `--grpc-port` | — | `server.grpc-port` | u16 | `0` | gRPC port; `0` disables gRPC. |
| `--bolt-port` | — | `server.bolt-port` | u16 | `0` | Bolt (Neo4j wire) port; `0` disables Bolt. Bound only when `cypher` is enabled. |
| — | — | `server.body-limit` | bytes | `16MiB` | Maximum HTTP request body, gRPC message, and response size. `config.yml` only. |
| `--allow-insecure-listen` | `RUSHDB_ALLOW_INSECURE_LISTEN` | CLI/env only | bool | `false` | Acknowledge binding a non-loopback address without TLS (HTTP) and exposing the cleartext pgwire/gRPC/Bolt listeners. Refused in multitenant mode for pgwire/gRPC/Bolt regardless. |
| `--maintenance` | `RUSHDB_MAINTENANCE` | `server.maintenance` | bool | `false` | Maintenance mode: writes are rejected with `unavailable`, `/health` and `/ready` return 503. Reads keep working. |
| `--require-index-warmup` | `RUSHDB_REQUIRE_INDEX_WARMUP` | `server.require-index-warmup` | bool | `false` | `/ready` returns 503 until every vector index has caught up with its source records. Single-tenant only. |
| `--data-dir` | `RUSHDB_DATA_DIR` | `server.data-dir` (informational) | path | none | Persistent data directory. Omit for an in-memory node. The running server always takes it from the CLI or environment. |
| `--request-timeout-ms` | `RUSHDB_REQUEST_TIMEOUT_MS` | CLI/env only | u64 | `0` | Execution deadline for requests on every transport (a client `deadlineMs` may only shorten it); exceeded requests fail with `cancelled`. `0` = no deadline. |
| `--max-concurrent-requests-per-tenant` | `RUSHDB_MAX_CONCURRENT_PER_TENANT` | CLI/env only | usize | `0` | Per-tenant concurrent request cap in multitenant mode; excess requests fail with `unavailable`. `0` = unlimited. |
| `--threads` | `RUSHDB_THREADS` | `resources.threads` | usize | `0` | Request-scheduler and HTTP runtime worker threads. `0` = the host's available parallelism. Workers are clamped to 2–256; the scheduler queue holds 32 requests per worker, clamped to 64–4096. |
| `--advertised-address` | `RUSHDB_ADVERTISED_ADDRESS` | `resources.advertised-address` | string | bind address | Address reported to clients in build info. |

## Security and tenancy

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--token` | `RUSHDB_TOKEN` | CLI/env only (never stored) | string | `local-token` | Boot token: full-access credential on every transport, and the pgwire password. Must not be empty. A non-loopback `--listen` refuses to start with the default. |
| `--capability-secret` | `RUSHDB_CAPABILITY_SECRET` | CLI/env only (never stored) | hex string | none | HMAC-SHA256 key (hex, at least 16 bytes after decoding) that enables signed capability tokens on HTTP. |
| `--multitenant` | — | `tenancy.mode` (`single` or `multitenant`) | bool | `false` (`single`) | Scope-keyed multitenancy. Fixed for the life of a data directory. |

## Query languages

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--languages` | — | `languages.sql`, `languages.gql`, `languages.cypher`, `languages.gremlin`, `languages.sql-pgq`, `languages.mongodb` | comma list / bools | `sql`, `gremlin`, `sql-pgq`, `mongodb` | Statement languages to enable (a list replaces the default set): `sql`, `gql`, `cypher`, `gremlin`, `sql-pgq`, `mongodb`. The native search query is always enabled; listing `searchquery` is an error. |

## Memory and caches

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--managed-limit` | `RUSHDB_MANAGED_LIMIT` | `resources.managed-limit` | bytes | `512MiB` | Envelope for engine-managed reservations (caches, transaction pool, query phases). Not a process RSS limit. |
| `--storage-cache` | `RUSHDB_STORAGE_CACHE` | `resources.storage-cache` | bytes | `128MiB` | Storage page cache. At least 64 KiB; `storage-cache + transaction-pool` must be less than `managed-limit`. |
| `--transaction-pool` | `RUSHDB_TRANSACTION_POOL` | `resources.transaction-pool` | bytes | `128MiB` | Shared pool for staged transaction payloads. |
| `--deployment-limit` | `RUSHDB_DEPLOYMENT_LIMIT` | `resources.deployment-limit` | bytes | none | Optional declared memory cap of the host or container. When set, `managed-limit + memory-headroom` must fit inside it. Not detected automatically. |
| `--memory-headroom` | `RUSHDB_MEMORY_HEADROOM` | `resources.memory-headroom` | bytes | `128MiB` | Untracked allowance reserved inside `deployment-limit`. Must be positive. |
| `--logical-cache-bytes` | `RUSHDB_LOGICAL_CACHE_BYTES` | `resources.logical-cache-bytes` | bytes | `128MiB` | Byte-accounted caches: ANN vector segments, text indexes, planner statistics. `0` disables them. |
| `--planner-stats-bytes` | `RUSHDB_PLANNER_STATS_BYTES` | `resources.planner-stats-bytes` | bytes | `32MiB` | Evictable planner-statistics cache, charged to the logical cache. `0` disables caching. |
| `--planner-stats-idle-seconds` | `RUSHDB_PLANNER_STATS_IDLE_SECONDS` | `resources.planner-stats-idle-seconds` | u64 | `600` | Seconds an unused planner-statistics entry stays cached. |
| `--planner-stats-max-age-commits` | `RUSHDB_PLANNER_STATS_MAX_AGE_COMMITS` | `resources.planner-stats-max-age-commits` | u64 | `10000` | Commits after which a cached statistics entry is recomputed. |
| `--query-memory-max` | `RUSHDB_QUERY_MEMORY_MAX` | `resources.query-memory-max` | bytes | `256MiB` | Per-query working-set cap; a query that would materialize more fails with `resource_exhausted`. `0` = off. A request may set a smaller cap. |
| `--unindexed-walk-max-owners` | `RUSHDB_UNINDEXED_WALK_MAX_OWNERS` | `resources.unindexed-walk-max-owners` | u64 | `0` | Largest record population a filter without a usable index may check. A larger one fails at planning with `invalid_request` (not retryable), naming the property; declare an ordered index on it. The population is read from counters before any work: the walked property's support, or every record of the labels when no condition can be walked. `0` (default) = no limit: the filter walks the records, as a PostgreSQL sequential scan does, bounded by `--request-timeout-ms` and `--query-memory-max`. A request may set a tighter limit ([request bounds](../architecture/resource-governance.md#request-bounds)). |
| `--rss-max` | `RUSHDB_RSS_MAX` | `resources.rss-max` | bytes | `0` | Resident-memory watchdog threshold. Above it the server logs a warning and runs best-effort reclamation every 10 s. A policy signal, not an allocation cap. `0` = disabled. |
| `--tenant-idle-seconds` | `RUSHDB_TENANT_IDLE_SECONDS` | `resources.tenant-idle-seconds` | u64 | `300` | Seconds an unused tenant's in-memory handles (vector engine, transaction coordinator) stay resident. |
| `--tenant-resident-bytes` | `RUSHDB_TENANT_RESIDENT_BYTES` | `resources.tenant-resident-bytes` | bytes | `512MiB` | Budget for resident tenant handles. Idle tenants are evicted least recently used first; when only active tenants remain, a new tenant gets a retryable 503 (`tenant_capacity`). |
| `--http-inflight-body-bytes` | `RUSHDB_HTTP_INFLIGHT_BODY_BYTES` | `resources.http-inflight-body-bytes` | bytes | `256MiB` | HTTP request-body bytes that may be in flight across all connections. A request whose declared length does not fit is refused with a retryable 503 before its body is read. Never smaller than `server.body-limit`. |

## Write path and durability

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--durability` | `RUSHDB_DURABILITY` | `resources.durability` | `local` \| `async` \| `memory` | `local` | `local`: fsync before acknowledging. `async`: acknowledge after queuing, persisted by a later `local` commit or checkpoint, so an acknowledged commit can be lost in a crash. `memory`: in-memory stores only; rejected with a data directory. |
| `--max-group-count` | `RUSHDB_MAX_GROUP_COUNT` | `resources.max-group-count` | usize | `0` | Maximum commits coalesced into one group commit. `0` = unbounded. |
| `--max-group-bytes` | `RUSHDB_MAX_GROUP_BYTES` | `resources.max-group-bytes` | usize | `0` | Maximum staged bytes coalesced into one group commit. `0` = unbounded. |
| `--max-group-delay-ms` | `RUSHDB_MAX_GROUP_DELAY_MS` | `resources.max-group-delay-ms` | u64 | `0` | How long the group-commit publisher waits for more commits before flushing, so concurrent commits share one fsync. `0` = flush immediately. |
| `--lock-timeout-ms` | `RUSHDB_LOCK_TIMEOUT_MS` | `resources.lock-timeout-ms` | u64 | `0` | Maximum wait for the write-admission lock before a commit fails with a retryable `resource_exhausted`. `0` = wait indefinitely. |
| `--max-expanded-commit-bytes` | `RUSHDB_MAX_EXPANDED_COMMIT_BYTES` | `resources.max-expanded-commit-bytes` | bytes | `512MiB` | Largest single commit, measured as its expanded write set. Larger commits fail with a non-retryable `resource_exhausted`. Commits larger than one 64 MiB WAL record are chained automatically below this limit. Must be positive. |
| `--idempotency-window-seconds` | `RUSHDB_IDEMPOTENCY_WINDOW_SECONDS` | `resources.idempotency-window-seconds` | u64 | `86400` | How long retry receipts are kept: a repeated commit key within the window returns the original outcome. |
| `--shards` | `RUSHDB_SHARDS` | `resources.shards` | usize | `1` | Physical storage files. Values above 1 (maximum 256) are a Preview and require `--experimental-sharding`. |
| `--experimental-sharding` | `RUSHDB_EXPERIMENTAL_SHARDING` | `resources.experimental-sharding` | bool | `false` | Acknowledge that multi-shard commit groups are persisted per shard and are not crash-atomic. |

## Maintenance, retention, and compaction

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--checkpoint-interval-seconds` | `RUSHDB_CHECKPOINT_INTERVAL_SECONDS` | `resources.checkpoint-interval-seconds` | u64 | `0` (= 5 s) | Maintenance tick: MVCC history GC, WAL retention, index builds, durable checkpoint. |
| `--compact-interval-seconds` | `RUSHDB_COMPACT_INTERVAL_SECONDS` | `resources.compact-interval-seconds` | u64 | `300` | Idle compaction period and minimum interval between compactions. `0` = disabled. |
| `--compact-quiesce-seconds` | `RUSHDB_COMPACT_QUIESCE_SECONDS` | `resources.compact-quiesce-seconds` | u64 | `10` | Quiet period after the last commit before compaction may run. |
| `--compact-growth-percent` | `RUSHDB_COMPACT_GROWTH_PERCENT` | `resources.compact-growth-percent` | u32 | `150` | Compact only when the file has grown to at least this percentage of its size after the last compaction (or at least 10,000 keys were purged). |
| `--tx-log-keep-commits` | `RUSHDB_TX_LOG_KEEP_COMMITS` | `resources.tx-log-keep-commits` | u64 | `0` | Commits the WAL keeps even when no consumer or open reader needs them. `0` = keep only what consumers and readers pin. Bounded by the hard caps. |
| `--tx-log-retention-commits` | `RUSHDB_TX_LOG_RETENTION_COMMITS` | `resources.tx-log-retention-commits` | u64 | `100000` | Hard cap: the WAL never holds more than the last N commits for any consumer. |
| `--tx-log-keep-all` | `RUSHDB_TX_LOG_KEEP_ALL` | `resources.tx-log-keep-all` | bool | `false` | Remove the commit-count cap (wins over `tx-log-retention-commits`). |
| `--tx-log-retention-bytes` | `RUSHDB_TX_LOG_RETENTION_BYTES` | `resources.tx-log-retention-bytes` | bytes | `4GiB` | Hard byte cap on the retained WAL; the oldest commits are pruned until it fits. `0` = off. |
| `--retention-consumer-lease-seconds` | `RUSHDB_RETENTION_CONSUMER_LEASE_SECONDS` | `resources.retention-consumer-lease-seconds` | u64 | `86400` | How long a retention consumer (backup, WAL archive, replica, cursor) may go without acknowledging before its claim on the WAL expires. `0` = never expire. |
| `--disk-space-threshold-percent` | `RUSHDB_DISK_SPACE_THRESHOLD_PERCENT` | `resources.disk-space-threshold-percent` | u8 (0–100) | `10` | Below this free-disk percentage on the data directory's filesystem, writes on every transport fail with `unavailable` (HTTP 503) and `/ready` returns 503. `0` = disabled. |

## Backup, archive, and replication

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--wal-archive-dir` | `RUSHDB_WAL_ARCHIVE_DIR` | `resources.wal-archive-dir` | path | none | Enable continuous WAL archiving into this directory (every 5 s). Required for point-in-time restore. |
| `--wal-archive-retention-seconds` | `RUSHDB_WAL_ARCHIVE_RETENTION_SECONDS` | `resources.wal-archive-retention-seconds` | u64 | `604800` (7 days) | Point-in-time restore window. Older segments not needed by the base that covers the window's start are deleted. |
| `--wal-archive-retention-bytes` | `RUSHDB_WAL_ARCHIVE_RETENTION_BYTES` | `resources.wal-archive-retention-bytes` | bytes | `0` | Hard byte cap on the archive. Over it the oldest segments are deleted even inside the window (logged as a warning). `0` = none. |
| `--replica-of` | `RUSHDB_REPLICA_OF` | `resources.replica-of` | URL | none | Leader base URL (for example `http://leader:3000`). The node becomes a read-only follower. |
| `--replica-id` | `RUSHDB_REPLICA_ID` | `resources.replica-id` | string | `default` | Identity this follower reports to the leader. Each follower of one leader needs a distinct id. |
| `--replica-token` | `RUSHDB_REPLICA_TOKEN` | `resources.replica-token` | string | this node's `--token` | Token presented to the leader's replication endpoint; must equal the leader's boot token. |
| `--raft` | — | CLI/env only | usize | none | Preview: run N in-process Raft state machines. Requires `--experimental-raft`. See [Replication](../operations/replication.md#raft-preview). |
| `--experimental-raft` | — | CLI/env only | bool | `false` | Acknowledge the Raft Preview's limitations. |

## Observability

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--logs-query-threshold-ms` | `RUSHDB_LOGS_QUERY_THRESHOLD_MS` | `resources.logs-query-threshold-ms` | u64 | `5000` | HTTP requests at or above this duration log a `slow protocol call` warning. `0` = disabled. |
| `--tenant-bulk-concurrency` | `RUSHDB_TENANT_BULK_CONCURRENCY` | `resources.tenant-bulk-concurrency` | usize | `2` | Bulk (ingest) requests one tenant runs at a time; further ones wait in line before they take a worker, so one tenant's bulk load does not queue every other tenant's commits behind its batches. `0` = unlimited. |
| `--metrics-prefix` | `RUSHDB_METRICS_PREFIX` | `resources.metrics-prefix` | string | none | Prefix prepended to every exported metric name. |
| `--metrics-filter` | `RUSHDB_METRICS_FILTER` | `resources.metrics-filter` | string | none | Export only metric families whose name contains this substring. |
| `--metrics-tenant-limit` | `RUSHDB_METRICS_TENANT_LIMIT` | `resources.metrics-tenant-limit` | usize | `0` | Emit per-tenant `rushdb_records_stored` series for at most this many most recently served tenants. `0` = only the `tenant="all"` total. |
| — | `RUST_LOG` | — | filter | `rushdb_server=info,tower_http=info` | Log filter (standard `tracing` env-filter syntax). |

## Vector indexes

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--require-vector-index-name` | `RUSHDB_REQUIRE_VECTOR_INDEX_NAME` | `resources.require-vector-index-name` | bool | `true` | Vector writes and searches must name their index explicitly. Turn it off with `--require-vector-index-name false` or `require-vector-index-name: false` in `config.yml`. |
| `--vector-exact-max-pool` | `RUSHDB_VECTOR_EXACT_MAX_POOL` | `resources.vector-exact-max-pool` | integer | `5000` | Largest index (vectors) a vector search in `auto` mode scores exactly; above it, `auto` uses the ANN graph once resident and builds it in the background meanwhile. See [Vector search](../indexes-and-search/vector-search.md#exact-versus-approximate-ann). |
| `--vector-default-ef` | `RUSHDB_VECTOR_DEFAULT_EF` | `resources.vector-default-ef` | integer | `64` | ANN gather breadth when a search does not set `ef`. |
| `--introspection-value-max-chars` | `RUSHDB_INTROSPECTION_VALUE_MAX_CHARS` | `resources.introspection-value-max-chars` | integer | `64` | Longest text (characters) catalog introspection shows for a sample, listed value or bound (`describe` schema/properties, `schemaSummary`); longer text is cut and ends with `…`. Value queries and records are never cut. `0` = no cap. |

## Transactions

| Flag | Environment | `config.yml` key | Type | Default | Description |
|---|---|---|---|---|---|
| `--tx-max-active` | `RUSHDB_TX_MAX_ACTIVE` | `transactions.max-active` | usize | `128` | Maximum concurrently active transactions. |
| `--tx-max-operations` | `RUSHDB_TX_MAX_OPERATIONS` | `transactions.max-operations` | usize | `10000` | Maximum staged operations per transaction. |
| `--tx-max-staged-bytes` | `RUSHDB_TX_MAX_STAGED_BYTES` | `transactions.max-staged-bytes` | bytes | `8MiB` | Maximum staged payload bytes per transaction. |
| `--tx-max-expired-ids` | `RUSHDB_TX_MAX_EXPIRED_IDS` | `transactions.max-expired-ids` | usize | `128` | Expired transaction ids remembered per sweep (so a late commit reports `expired`). |
| `--tx-default-ttl-seconds` | `RUSHDB_TX_DEFAULT_TTL_SECONDS` | `transactions.default-ttl-seconds` | u64 | `60` | Lifetime of a transaction whose `begin` omits `ttlMs`. Must be positive and not exceed `max-age-seconds`. |
| `--tx-max-age-seconds` | `RUSHDB_TX_MAX_AGE_SECONDS` | `transactions.max-age-seconds` | u64 | `60` | Upper bound on any transaction lifetime (and on cursor lifetimes). |
| `--tx-max-staged-bytes-per-tenant` | `RUSHDB_TX_MAX_STAGED_BYTES_PER_TENANT` | `transactions.max-staged-bytes-per-tenant` | bytes | `0` | Share of the transaction pool one tenant may claim in multitenant mode. `0` = tenants share the pool directly. Must not exceed `transaction-pool`. |

## One-shot commands

These flags perform a task and exit without starting the server. See
[CLI reference](cli-reference.md#maintenance-commands).

| Flag | Argument | Description |
|---|---|---|
| `--print-config` | — | Print the effective `config.yml` and exit. |
| `--init` | `DATA_DIR` | Write and validate an initial `config.yml` in `DATA_DIR`, then exit. |
| `--export` | `PATH` | Write a logical graph snapshot of `--data-dir` to `PATH`. |
| `--restore` | `PATH` | Restore a graph snapshot into a fresh `--data-dir`. |
| `--compact` | — | Compact the store in place (server must be stopped). |
| `--backup-base` | `PATH` | Offline physical base backup of `--data-dir`. |
| `--restore-pitr` | `BASE_DIR` | Restore a base backup plus archived WAL into a new `--data-dir`. |
| `--to-lsn` | `LSN` | Stop point for `--restore-pitr`. |
| `--verify` | — | Logical integrity check of `--data-dir`. |
| `--verify-base` | `BASE_DIR` | Check a base backup's manifest and file CRCs. |

## Startup validation rules

`rushdb serve` refuses to start when:

| Condition | Message (abridged) |
|---|---|
| `--token` is empty | `server token must not be empty` |
| Capability secret is not hex, or shorter than 16 bytes | `--capability-secret must be hex` / `capability secret must be at least 16 bytes` |
| Only one of `tls-cert` / `tls-key` set | `TLS requires both a certificate and a private key` |
| Non-loopback `--listen` without TLS or `--allow-insecure-listen` | `refusing to bind HTTP (cleartext) to non-loopback listen address` |
| Non-zero pgwire/gRPC/Bolt port on a non-loopback listen address without `--allow-insecure-listen` | `refusing to bind pgwire/gRPC/Bolt (cleartext) ...` |
| Multitenant mode, non-loopback listen address, and any non-zero pgwire/gRPC/Bolt port | `refusing to bind pgwire/gRPC/Bolt on a non-loopback multitenant listener` |
| `storage-cache` < 64 KiB, or `storage-cache + transaction-pool` ≥ `managed-limit` | `cache must be at least 64 KiB; ...` |
| `managed-limit + memory-headroom` > `deployment-limit` | `managed-limit plus memory-headroom exceeds deployment-limit` |
| `memory-headroom` is 0 | `memory-headroom must be positive` |
| `disk-space-threshold-percent` > 100 | `disk-space-threshold-percent must be between 0 and 100` |
| `shards` is 0, above 256, or above 1 without `--experimental-sharding` | `shard-count ...` |
| `durability: memory` with a data directory | `Memory durability is for in-memory stores` |
| `transactions.default-ttl-seconds` is 0 or above `max-age-seconds` | `transaction default TTL must be positive and no greater than transaction maximum TTL` |
| `transaction-pool` too small for transaction metadata | `transaction-pool cannot fit coordinator and active transaction metadata` |
| `--raft` without `--experimental-raft` | explanation of the Preview, exit code 2 |
| `--languages` names `searchquery` or an unknown language | `invalid --languages: ...` |
| Stored `tenancy.mode` differs from `--multitenant` | `data directory tenancy mismatch` |
| Stored `config.yml` `version` is not 1 | `config version mismatch` |
| An explicitly supplied flag or variable disagrees with `config.yml` | `config.yml disagrees with explicitly supplied startup flags` |
| A data directory with committed data has no `config.yml` | `existing data directory has no config.yml` |

## See also

- [Configuration](../operations/configuration.md)
- [CLI reference](cli-reference.md)
- [Limits](limits.md)
- [Security](../operations/security.md)
