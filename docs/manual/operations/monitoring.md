# Monitoring

How to check that a RushDB server is alive and ready, collect its metrics
and logs, and what to alert on. Every metric is listed in the
[metrics reference](../reference/metrics.md).

## Health and readiness

| Endpoint | Auth | Purpose |
|---|---|---|
| `GET /health` | none | Liveness: the process is running. |
| `GET /ready` | none | Readiness: the node can serve and persist writes. |

```bash
curl -s http://127.0.0.1:3000/health
# {"status":"ok","engine":"rushdb-lmpg","protocolVersion":1,"queryLanguages":["gremlin","mongo","searchQuery","sql","sqlPgq"]}
curl -s http://127.0.0.1:3000/ready
# {"status":"ready","commitIndex":182344,"dataDir":"/var/lib/rushdb"}
```

`/ready` returns 503 with a `reason` when:

| `reason` | Meaning | Action |
|---|---|---|
| boot phase (`status: starting`) | Still opening storage or starting engines. | Wait. |
| `maintenance mode` | Started with `--maintenance`. | Expected during maintenance. |
| `store is shutting down` | Shutdown in progress. | Expected during a stop. |
| `durable publisher failed` | The write path hit an unrecoverable error (for example the disk filled up). `error` has the detail. | Fix the cause and restart. |
| `data directory is not writable` | The readiness probe file could not be written. | Check permissions, mount, disk. |
| `free disk below threshold` | Free space below `disk-space-threshold-percent`. Writes get `unavailable` (HTTP 503). | Free disk space. |
| `vector indexes are still warming` | Only with `--require-index-warmup`. | Wait. |

Use `/health` for liveness probes (restarting a node that is busy recovering
does not help) and `/ready` for load-balancer membership.

> **Warning:** In maintenance mode `/health` also returns 503
> (`{"status":"maintenance"}`). If your orchestrator restarts on a failed
> liveness probe, treat that body as alive, or disable the liveness probe
> while in maintenance.

## Prometheus metrics

`GET /metrics` needs the boot token or a capability with `read` permission:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: rushdb
    scrape_interval: 15s
    metrics_path: /metrics
    authorization:
      type: Bearer
      credentials_file: /etc/prometheus/rushdb-token
    static_configs:
      - targets: ['127.0.0.1:3000']
```

In multitenant mode, a capability used for scraping must be bound to a
tenant and the scrape must send that tenant in `X-RushDB-Tenant`; the boot
token needs no tenant header.

Scrapes are cheap: the expensive storage byte breakdown is computed in the
background every 15 minutes and served from cache. Use
`--metrics-prefix` to namespace the metrics and `--metrics-filter` to export
a subset.

### Key metrics

| Area | Metric | Watch for |
|---|---|---|
| Traffic | `rate(rushdb_requests_total[5m])` by `route` | Throughput per operation. |
| Latency | `histogram_quantile(0.99, rate(rushdb_request_duration_seconds_bucket[5m]))` | Slow operations. |
| Commits | `rushdb_commit_latency_seconds`, `rushdb_commit_phase_seconds{phase="wal_append_sync"}` | fsync latency of the disk. |
| Write rate | `deriv(rushdb_commits_total[5m])` | Commits per second. |
| Saturation | `rushdb_scheduler_inflight / rushdb_scheduler_capacity` | Near 1 means requests will be shed with 503. |
| Memory | `rushdb_allocator_resident_bytes`, `rushdb_budget_used_bytes{pool="managed"} / rushdb_budget_limit_bytes{pool="managed"}` | Approaching the host or container limit; managed budget exhaustion. |
| Disk | `rushdb_storage_file_bytes`, `rushdb_wal_retained_bytes` | Growth; WAL held by a lagging consumer. |
| Retention | `rushdb_retention_consumers`, `rushdb_retention_protected_floor`, `rushdb_retention_oldest_reader_commit` | A consumer or reader pinning history. |
| Maintenance | `rushdb_gc_tick_seconds`, `rushdb_maintenance_step_seconds{step}`, `rushdb_storage_compactions_total` | Maintenance falling behind; a long tick's `step` names what held the writer (a slow `checkpoint` is a slow fsync). |
| Writer | `rushdb_writer_hold_seconds{holder}` | Long writer holds stall every tenant's commits; `holder` names the work (`publish`, `history_gc`, `checkpoint`, `pause`). |
| Search | `rushdb_text_pending`, `rushdb_ann_exact_fallbacks_total`, `rushdb_vector_searches_total{path,reason}` | Text index backlog; vector cache too small; vector searches answered exactly instead of by the ANN index. |
| Tenants | `rushdb_tenants_resident`, `rushdb_tenant_resident_bytes` | Residency budget pressure. |

## Logs

The server logs to stderr (captured by journald under systemd). Control
verbosity with `RUST_LOG`; the default is
`rushdb_server=info,tower_http=info`.

| Message | Level | Meaning |
|---|---|---|
| `rushdb listening` | info | Startup complete, with the bound ports and tenancy. |
| `data directory has copy-on-write off` | info | The store files and the WAL are on a copy-on-write filesystem (btrfs) with copy-on-write turned off. |
| `data files still copy on write: ...` | warn | Store files created before copy-on-write was turned off keep it; checkpoints can stall commits for seconds. `paths`, `reason`. See [Deployment](deployment.md#data-directory-layout). |
| `slow protocol call` | warn | An HTTP request took at least `logs-query-threshold-ms` (default 5 s). Fields: `op`, `scope`, `elapsed_ms`, `rows`, `ok`. |
| `storage compaction complete` | info | Idle compaction ran; `before_bytes`, `after_bytes`. |
| `compaction deferred` | info | Compaction could not run this time; retried next interval. |
| `resident memory over watchdog limit; reclaiming` | warn | RSS above `--rss-max`. |
| `WAL archiving failed` | warn | The archiver could not write a segment. |
| `WAL archive fell behind the retention cap` | error | The archive now has a gap; take a new base backup. |
| `WAL archive byte cap removed segments inside the retention window` | warn | The PITR window shrank. |
| `durability checkpoint failed` | warn | A periodic checkpoint failed. |
| `incremental GC tick failed` | warn | A maintenance tick failed. |
| `suppressing internal execution error detail` | warn / error | A client got `internal execution error`; this line has the real cause. |
| `replication follower: ...` | warn / error | Follower progress problems; see [Replication](replication.md#monitoring-replication). |
| `shutdown signal received; closing the store cleanly` | info | `SIGTERM`/`SIGINT` received. |
| `raft replication is EXPERIMENTAL ...` | warn | The Raft Preview is enabled. |

## What to alert on

| Alert | Condition (example) | Severity |
|---|---|---|
| Node not ready | `/ready` not 200 for 2 min | page |
| Durable publisher failed | `/ready` reason `durable publisher failed` | page |
| Disk nearly full | free space < 20% on the data filesystem (the write guard trips at 10% by default) | page |
| Writes rejected for disk | `unavailable` "free disk" errors (HTTP 503) | page |
| Shedding | sustained 503 responses, or `rushdb_scheduler_inflight / rushdb_scheduler_capacity > 0.9` | warn |
| Memory | `rushdb_allocator_resident_bytes` > 85% of the service's memory limit | warn |
| WAL growth | `rushdb_wal_retained_bytes` > 75% of `tx-log-retention-bytes` | warn |
| Archive gap | log `WAL archive fell behind the retention cap` | page |
| Replica lag | leader `rushdb_commits_total` − follower `rushdb_commits_total` above your tolerance | warn |
| Slow requests | rate of `slow protocol call` log lines, or p99 latency above target | warn |
| Commit latency | p99 `rushdb_commit_latency_seconds` > 0.5 s | warn |
| Internal errors | HTTP 500 responses or `suppressing internal ... error detail` lines | warn |

## See also

- [Metrics reference](../reference/metrics.md)
- [Troubleshooting](troubleshooting.md)
- [Capacity planning](capacity-planning.md)
- [Deployment](deployment.md#probes)
