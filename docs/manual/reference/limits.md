# Limits

Size caps, budgets, lifetimes, and fixed maximums in RushDB, with their
defaults. Limits marked **configurable** can be changed (see the
[configuration reference](configuration-reference.md)); the others are fixed
in this release.

## Requests and responses

| Limit | Default | Configurable | Behavior when exceeded |
|---|---|---|---|
| HTTP request body | 16 MiB | `server.body-limit` | 413, `invalid_request` |
| Response size (all transports) | 16 MiB | `server.body-limit` | HTTP 413, `execution_failed`; elsewhere `resource_exhausted` |
| gRPC message size (in and out) | 16 MiB | `server.body-limit` | gRPC error |
| HTTP request bodies in flight (all connections) | 256 MiB (never below the body limit) | `http-inflight-body-bytes` | 503, `resource_exhausted`, before the body is read |
| Admitted concurrent requests (scheduler) | workers + queue; workers = CPU count clamped to 2–256, queue = 32 × workers clamped to 64–4096 | `threads` | 503, `unavailable` |
| Concurrent requests per tenant (multitenant) | unlimited | `--max-concurrent-requests-per-tenant` | `unavailable` |
| Request execution deadline (every transport) | none | `--request-timeout-ms`; a request may set a shorter one | `cancelled` |
| `X-RushDB-Tenant` header length | 1024 bytes | no | 400, `invalid_request` |
| pgwire connections | 1024 | no | connection refused |
| Bolt connections | 1024 | no | connection refused |
| Bolt message size | 64 MiB | no | connection error |
| Bolt buffered rows per result | 100,000 | no | error |
| Bolt idle I/O timeout | 60 s | no | connection closed |
| Bolt authentication timeout | 10 s | no | connection closed |
| gRPC concurrent requests per connection | 64 | no | queued by HTTP/2 |
| gRPC concurrent streams per connection | 64 | no | queued by HTTP/2 |

## Queries

| Limit | Default | Configurable | Behavior when exceeded |
|---|---|---|---|
| Query working set | 256 MiB | `query-memory-max` (0 = off); a request may set a smaller cap | `resource_exhausted` |
| Records a filter without a usable index checks | none | `unindexed-walk-max-owners`; a request may set a limit ([request bounds](../architecture/resource-governance.md#request-bounds)) | `invalid_request` naming the index to declare |
| Query phase reservation | 8 MiB per request, drawn from the managed envelope | no | `resource_exhausted` when the envelope is full |
| Records cursor page size | 1–10,000 | no | refused |
| Retained cursor state (node-wide) | 64 MiB | no | new cursor refused |
| Cursor lifetime | as requested, capped at `tx-max-age-seconds` | `tx-max-age-seconds` | cursor expires |
| Aggregation buckets per query | 1,000 | no | refused |
| SQL: records per label (table) | 100,000 | no | query refused |
| Graph-language mutation: records created or changed | 100,000 | no | mutation refused |
| Graph-language mutation staging | 64 MiB | no | `resource_exhausted` (not retryable) |
| Graph-language statement clauses | 8,192 | no | `invalid_request` |
| `text` query default `limit` | 10 | per request | — |
| `vectorIndexCandidates` default `limit` | 500 | per request | — |
| Vector spaces per tenant | 256 | no | refused |
| Records per transport response | bounded by the response size | `server.body-limit` | see above |

## Writes and transactions

| Limit | Default | Configurable | Behavior when exceeded |
|---|---|---|---|
| Expanded size of one commit | 512 MiB | `max-expanded-commit-bytes` | `resource_exhausted` (not retryable) |
| WAL record size | 64 MiB (larger commits are chained automatically) | no | — |
| Active transactions | 128 | `tx-max-active` | `resource_exhausted` (retryable) |
| Operations per transaction | 10,000 | `tx-max-operations` | `resource_exhausted` (not retryable) |
| Staged bytes per transaction | 8 MiB | `tx-max-staged-bytes` | `resource_exhausted` (not retryable) |
| Staged JSON nesting depth | 64 | no | `resource_exhausted` (not retryable) |
| Transaction pool (all transactions) | 128 MiB | `transaction-pool` | `resource_exhausted` (retryable) |
| Staged bytes per tenant (multitenant) | shared pool | `tx-max-staged-bytes-per-tenant` | `resource_exhausted` |
| Default transaction lifetime | 60 s | `tx-default-ttl-seconds` | transaction expires |
| Maximum transaction lifetime | 60 s | `tx-max-age-seconds` | requested TTL is capped |
| Write-lock wait | unlimited | `lock-timeout-ms` | `resource_exhausted` (retryable) |
| Idempotent retry window | 24 h | `idempotency-window-seconds` | a retry becomes a new commit |
| Free disk for writes (every transport) | 10% | `disk-space-threshold-percent` | `unavailable` (HTTP 503) |

## Memory budgets

| Budget | Default | Configurable |
|---|---|---|
| Managed envelope | 512 MiB | `managed-limit` |
| Storage page cache | 128 MiB (minimum 64 KiB) | `storage-cache` |
| Logical caches (vector, text, planner statistics) | 128 MiB | `logical-cache-bytes` |
| Planner statistics | 32 MiB (child of the logical caches) | `planner-stats-bytes` |
| ANN segment cache | 256 MiB, and at most 12 cached generations | no |
| Resident tenant handles | 512 MiB (16 KiB minimum charge per tenant) | `tenant-resident-bytes` |
| Tenant handle idle time | 300 s | `tenant-idle-seconds` |
| Memory headroom inside a declared deployment limit | 128 MiB | `memory-headroom` |

## Retention and storage

| Limit | Default | Configurable |
|---|---|---|
| WAL commit cap | 100,000 commits | `tx-log-retention-commits`, `tx-log-keep-all` |
| WAL byte cap | 4 GiB | `tx-log-retention-bytes` |
| WAL keep window without consumers | 0 commits | `tx-log-keep-commits` |
| WAL segment size | 64 MiB | no |
| Retention consumer lease | 24 h | `retention-consumer-lease-seconds` |
| Abandoned read view lease | 60 s | no |
| WAL archive window | 7 days | `wal-archive-retention-seconds` |
| WAL archive byte cap | none | `wal-archive-retention-bytes` |
| Physical storage shards | 1 (maximum 256, Preview above 1) | `shards` |
| Compaction trigger | file grew to 150% or ≥ 10,000 keys purged | `compact-growth-percent` |
| Replication page | 2,000 WAL records per poll | no |

## Background intervals

| Task | Interval | Configurable |
|---|---|---|
| Maintenance tick (GC, retention, index builds, checkpoint) | 5 s, up to 512 keys per step | `checkpoint-interval-seconds` |
| Index build budget per tick | 250 ms | no |
| Idle compaction check | 300 s | `compact-interval-seconds` |
| Compaction quiet period | 10 s | `compact-quiesce-seconds` |
| Expired-transaction reaper | 1 s | no |
| Text index applier | 250 ms | no |
| WAL archiver | 5 s | no |
| Storage byte statistics refresh | 15 min | no |
| RSS watchdog (when enabled) | 10 s | `rss-max` |
| Replication follower poll when caught up | 200 ms | no |

## Shutdown

| Phase | Deadline |
|---|---|
| HTTP connection drain | 30 s |
| Background maintenance drain | 30 s |
| Final durability checkpoint | 5 s |
| pgwire / gRPC / Bolt drain | 5 s each, inside a 7 s transport budget |

## See also

- [Configuration reference](configuration-reference.md)
- [Capacity planning](../operations/capacity-planning.md)
- [Error codes](error-codes.md)
