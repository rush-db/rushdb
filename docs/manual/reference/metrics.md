# Metrics

Every Prometheus metric the server exports at `GET /metrics`. For scraping
setup and alerting guidance see [Monitoring](../operations/monitoring.md).

## Scraping

```bash
curl -s -H "Authorization: Bearer $RUSHDB_TOKEN" http://127.0.0.1:3000/metrics
```

- The endpoint requires the boot token or a capability with `read`
  permission.
- Format: Prometheus text exposition, version 0.0.4.
- `--metrics-prefix` prepends a string to every name;
  `--metrics-filter` keeps only families whose name contains a substring.
- Values are refreshed when the endpoint is scraped, except the storage byte
  breakdown (`rushdb_storage_data_bytes`, `_history_bytes`, `_log_bytes`,
  `_receipt_bytes`), which a background task refreshes every 15 minutes,
  starting 15 minutes after boot.
- Every family ending in `_total` is a counter; it resets when the process
  restarts.

Histogram buckets (seconds) for every histogram below:
`0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10`.

## Requests

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `rushdb_requests_total` | counter | `route` | Requests executed, by route. |
| `rushdb_request_duration_seconds` | histogram | `route` | Request latency, measured around execution. |
| `rushdb_rows_returned_total` | counter | `route` | Rows/records returned by queries. |
| `rushdb_operation_work_total` | counter | `operation`, `counter` | Bounded work units. `operation` is the protocol family (`query`, `mutate`, `describe`, `ingest`, `index`, `transaction`, `purge_tenant`, `backup`) with `counter` `response_rows` or `response_bytes`; or `operation="sql_scan"` with `counter` `candidate_rows`, `reconstructed_rows`, `filtered_rows`, `posting_bytes`. |
| `rushdb_scheduler_inflight` | gauge | — | Jobs admitted to the bounded request scheduler. |
| `rushdb_scheduler_capacity` | gauge | — | Scheduler admission capacity (workers + queue). |
| `rushdb_http_inflight_body_bytes` | gauge | — | HTTP request-body bytes currently reserved. |

`route` values: one per Protocol v1 operation (for example `query.records`,
`mutate.putRecord`, `transaction.commit`; the full list is in
[HTTP endpoints](http-endpoints.md#protocol-v1-operations)), `invalid` for an
HTTP body that is not a valid envelope, `transport.execute` for gRPC and
Bolt, and `pgwire.execute` for PostgreSQL wire.

## Commits and write path

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `rushdb_commits_total` | counter | — | Durable commit index: commits since the store was created. |
| `rushdb_commit_latency_seconds` | histogram | — | Time from enqueue to durable acknowledgement per commit. |
| `rushdb_commit_phase_seconds` | histogram | `phase` | Commit pipeline time by phase. Per commit: `admission_wait`, `build`, `expand`, `durable`. Per publish group: `wal_encode`, `wal_append_sync`, `backend_apply`, `derive`, `publish`. |
| `rushdb_storage_group_commits_total` | counter | — | Group-commit write transactions persisted. |
| `rushdb_sequencer_inflight_keys` | gauge | — | Keys assigned to commits that are not yet published. |
| `rushdb_conflict_window_bytes` | gauge | — | Memory held by the conflict-validation window. |
| `rushdb_storage_admission_waits_total` | counter | — | Waits for the storage admission lock. |
| `rushdb_storage_admission_wait_ms_total` | counter | — | Milliseconds spent waiting for storage admission. |
| `rushdb_storage_backend_mutex_wait_ms_total` | counter | — | Milliseconds spent waiting for the storage backend lock. |
| `rushdb_storage_backend_mutex_hold_ms_total` | counter | — | Milliseconds the storage backend lock was held. |
| `rushdb_storage_backend_mutex_hold_max_ms` | gauge | — | Longest single hold of the storage backend lock. |
| `rushdb_writer_hold_seconds` | histogram | `holder` | Time one write transaction held the storage writer, from the start of its work to its commit: `publish` (commit groups), `history_gc` (one GC batch), `checkpoint` (fsync), `pause` (backup copy). Every other commit waits meanwhile, so the longest holds explain commit stalls. |

## Storage, WAL, and retention

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `rushdb_storage_file_bytes` | gauge | — | Size of the primary store file (0 for in-memory). |
| `rushdb_storage_data_bytes` | gauge | — | Live data row bytes (key + value), excluding history and log. 15-minute refresh. |
| `rushdb_storage_history_bytes` | gauge | — | Retained MVCC history bytes. 15-minute refresh. |
| `rushdb_storage_log_bytes` | gauge | — | Command-log bytes inside the store; normally 0 for a durable store because the log is the WAL. 15-minute refresh. |
| `rushdb_storage_receipt_bytes` | gauge | — | Retry-receipt bytes. 15-minute refresh. |
| `rushdb_wal_retained_bytes` | gauge | — | WAL bytes retained on disk. |
| `rushdb_bytes_stored` | gauge | `tenant` | Approximate stored bytes; `tenant="all"` is the node total. |
| `rushdb_records_stored` | gauge | `tenant` | Records stored. `tenant="all"` always; per-tenant series (internal tenant id) only with `--metrics-tenant-limit`. |
| `rushdb_storage_checkpoints_total` | counter | — | Durability checkpoints taken. |
| `rushdb_storage_checkpoint_wait_ms_total` | counter | — | Milliseconds spent in checkpoints. |
| `rushdb_storage_compactions_total` | counter | — | Completed storage compactions. |
| `rushdb_commit_log_rows_pruned` | gauge | — | Command-log rows reclaimed by retention. |
| `rushdb_receipts_swept` | gauge | — | Retry receipts removed after the idempotency window. |
| `rushdb_gc_tick_seconds` | histogram | — | Duration of one maintenance tick (GC, retention, checkpoint). |
| `rushdb_maintenance_step_seconds` | histogram | `step` | Duration of one step of a maintenance tick: `read_pins`, `vector_segments`, `housekeeping`, `history_gc`, `retention`, `receipts`, `vector_slots`, `checkpoint`. |
| `rushdb_retention_consumers` | gauge | — | Active retention consumers (backups, WAL archive, replicas, cursors). |
| `rushdb_retention_protected_floor` | gauge | — | Lowest commit protected by a retention consumer. |
| `rushdb_retention_oldest_reader_commit` | gauge | — | Oldest commit pinned by a live read view (0 = none). |

## Memory

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `rushdb_budget_used_bytes` | gauge | `pool` | Bytes currently reserved in a managed pool. |
| `rushdb_budget_peak_bytes` | gauge | `pool` | Peak reservation of a pool since start. |
| `rushdb_budget_limit_bytes` | gauge | `pool` | Configured limit of a pool. |
| `rushdb_allocator_allocated_bytes` | gauge | — | Bytes allocated by the application (jemalloc `stats.allocated`). |
| `rushdb_allocator_active_bytes` | gauge | — | Bytes in active pages (`stats.active`). |
| `rushdb_allocator_metadata_bytes` | gauge | — | Allocator metadata (`stats.metadata`). |
| `rushdb_allocator_resident_bytes` | gauge | — | Physically resident bytes (`stats.resident`); the closest in-process view of RSS. |
| `rushdb_allocator_retained_bytes` | gauge | — | Bytes retained by the allocator but not active (`stats.retained`). |
| `rushdb_allocator_bytes_allocated_total` | counter | — | Cumulative bytes allocated; only populated in special measurement builds, otherwise 0. |
| `rushdb_planner_stats_bytes` | gauge | — | Bytes in the planner-statistics cache. |
| `rushdb_planner_stats_entries` | gauge | — | Value distributions in the planner-statistics cache. |
| `rushdb_tenants_resident` | gauge | — | Tenants whose in-memory handles are resident. |
| `rushdb_tenant_resident_bytes` | gauge | — | Bytes charged by resident tenant handles. |

`pool` values: `managed` (the whole managed envelope), `transaction`
(transaction staging pool), and, when the logical cache is enabled,
`vector`, `sql`, `graph`. Per-query pools (`sql`, `graph`) are reserved only
while a query runs, so their `used` is usually 0 between scrapes; read their
`peak`. The `rushdb_budget_*` gauges track
reservations, not mapped memory; compare them with the `rushdb_allocator_*`
gauges. The allocator gauges stay 0 in builds without jemalloc.

## Indexes and search

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `rushdb_text_pending` | gauge | `index` | Changes not yet applied to each text index. In multitenant mode, summed by index name over the resident tenants. |
| `rushdb_ann_exact_fallbacks_total` | counter | — | Vector searches answered exactly because the cache budget could not admit the ANN index. |
| `rushdb_vector_searches_total` | counter | `path`, `reason` | Vector searches by the path that answered (`ann`, `exact`) and why (`none`, or the fallback: `annWarming`, `budget`, `metric`, `lowConfidence`). An `exact` answer with `annWarming` scanned every vector while the ANN index was built. |
| `rushdb_unindexed_walks_total` | counter | — | Queries whose unindexed predicate leaf walked more than 10,000 owners; `describe.explain` names the ordered index to declare. |

## Startup

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `rushdb_boot_phase_duration_seconds` | histogram | `phase` | Startup phase durations: `config`, `storage`, `raft` (Preview only), `total`. |

## See also

- [Monitoring](../operations/monitoring.md)
- [Storage and compaction](../operations/storage-and-compaction.md)
- [Configuration reference](configuration-reference.md#observability)
