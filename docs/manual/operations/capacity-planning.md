# Capacity planning

Measured capacity of a single RushDB node, and how to size disk, memory,
and write throughput for your workload. All figures are measurements on the
hardware and fixtures described here, not guarantees; your results depend on
your data shape, hardware, and query mix.

## Measurement context

Unless noted, the figures come from one benchmark run on an idle host:

| Item | Value |
|---|---|
| CPU | Intel i9-12900H (20 threads) |
| Storage | NVMe SSD with LUKS encryption |
| OS | Linux |
| Page cache | warm unless noted |
| Server | one `rushdb serve` node, multitenant, durable data directory, default `local` durability |
| Fixture | 200,000 records in 50 tenants; 10 fields per record, one of them a 2-element array |
| Load | 500 concurrent closed-loop clients for mixed workloads |

## Measured envelope (200K records, 50 tenants)

| Area | Measured |
|---|---|
| Bulk ingest (8 writers, 500-document batches) | 12,600 records/s; batch p50 0.27 s, p99 1.3 s |
| Single-record commits, 1 tenant | 114 / 476 / 1,015 / 1,022 commits/s at 1 / 8 / 32 / 128 writers |
| Single-record commits, 50 tenants | 117 / 452 / 893 / 823 commits/s at 1 / 8 / 32 / 128 writers (p50 8 / 17 / 34 / 140 ms) |
| Mixed workload (80% filtered read, 15% put, 5% count), 500 clients | 875 successful requests/s without indexes on the filtered fields, 0% shed; 4,373 requests/s with ordered indexes |
| Filtered read (two equality conditions), 32 clients | 1,810 requests/s, p50 17 ms, p99 33 ms without an index; 10,600 requests/s, p50 2.9 ms with ordered indexes |
| `$count` and unfiltered totals | constant time (one stored counter read) |
| Declared ordered index storage | about 26 bytes per record per index |
| Building 3 ordered indexes over 200K records | ready after 193 s with the default 5 s maintenance tick |
| Text index (20K documents) | search 0.16 ms; a change applied in 5 ms; ingest 4% slower with an index declared |
| ANN vector search (768 dimensions, `ef` 64, 3K vectors) | 1.5 ms single client; p50 2.5 ms, p99 5.3 ms at 32 clients |
| Vector backfill (768 dimensions, 200 per batch) | 0.063 s per batch |
| RSS | ~0.52 GB settled after ingest; ~1.0 GB peak during bulk ingest; ~0.76 GB in the 500-client mix |
| Idle background I/O | 0.03 MB/s |
| `/health` after process start | 26 ms |
| `/ready` after restart | 0.07 s after a clean stop, 0.09 s after `kill -9` (0.05 / 0.07 s with 1M records in one tenant) |
| Oversized-body flood (64 × 8 MiB declared) | 32 requests shed with 503 before buffering; RSS flat |

### Larger single tenant (1M records)

| Area | Measured |
|---|---|
| Ingest into one tenant | 3,360 records/s (batch p99 14 s); one tenant's commits are serialized |
| Disk | ~2.7 KB per record |
| Peak RSS | 1.1 GB |
| Filtered facet requests without an index | ~1–1.7 s for narrow filters |
| The same with ordered indexes on the filtered fields | 0.2–0.6 s |

### Vector search scaling (one 768-dimension space)

| Vectors | Search p50 (settled) | p50 right after bulk bind | p50 at 32 clients | Server RSS |
|---:|---:|---:|---:|---:|
| 10K | 1.8 ms | 1.6 ms | 2.9 ms | 471 MiB |
| 50K | 7.2 ms | 14.4 ms | 17 ms | 706 MiB |
| 150K | 21.6 ms | 61 ms | 90 ms | 1,567 MiB |

Vector segments take about 1.6 KB per vector on disk. Search is slower right
after a large bulk bind while background maintenance merges segments.

## Sizing disk

Measured on the fixture above:

| Component | Size |
|---|---|
| Live data | ~700 bytes per record (28 storage rows per record) |
| Store file including page slack, plus WAL | **~2.5 KB per record** at 200K records (store 465 MiB, WAL 18 MiB); ~2.7 KB per record at 1M records in one tenant |
| Each ordered index | ~26 bytes per record |
| Vectors | ~1.6 KB per 768-dimension vector |

Rules of thumb:

- Start from ~2.5 KB per 10-field record, add indexes and vectors, then add
  the free-space headroom the disk guard needs (writes stop below 10% free
  by default) and room for compaction and backups.
- The WAL stays small unless a follower, the WAL archiver, a base backup, or
  `tx-log-keep-commits` holds it. Its hard cap is
  `tx-log-retention-bytes` (4 GiB by default) plus one 64 MiB segment.
- The store file is a high-water mark: deletes free pages for reuse but do
  not shrink the file until compaction runs. See
  [Storage and compaction](storage-and-compaction.md).
- Records per node are bounded by disk, not memory: no per-record state is
  kept resident.

## Sizing memory

- Set the service memory limit with headroom over the measured RSS (about
  1 GB peak in the 200K bulk-ingest run, 1.1 GB at 1M records in one tenant,
  more with large vector spaces; see the table above).
- Leave the rest of the host's RAM to the kernel page cache, which caches the
  store file, vector segments, and text indexes.
- `managed-limit` and the cache settings bound engine reservations, not
  total RSS. `--deployment-limit` lets the server check that the managed
  envelope plus `memory-headroom` fits your declared container size.
- The [8 GB VPS profile](deployment.md#8-gb-vps-profile) is a measured
  starting point.

## Sizing write throughput

All tenants share one commit pipeline: commits are grouped, the group is
appended to the WAL and fsynced once, then applied. Measured per publish
group of ~10 commits: 6.6 ms WAL append and fsync, 2.7–5.0 ms applying, and
~0.5 ms deriving, serially. Consequences:

- Plan for roughly 450 single-record commits/s at 8 concurrent writers, or
  about 900/s at 32 writers spread over many tenants, on comparable hardware.
- **Batch writes.** Bulk ingest in 500-document batches reached
  12,600 records/s, versus about 1,000 single-record commits/s: each commit
  pays the pipeline cost once, however many records it carries.
- More concurrent writers stop helping beyond ~32; larger batches help more.
- One tenant's commits are serialized; spreading load across tenants helps
  only up to the shared pipeline's limit.
- Fast `fsync` matters most: watch
  `rushdb_commit_phase_seconds{phase="wal_append_sync"}`.

## Sizing reads

- Reads scale with CPU cores; `threads` defaults to all cores.
- **Declare indexes on filtered properties.** A filter without an index
  walks the records of the first condition and checks the rest one by one;
  in the measurements an ordered index made two-condition filters about 5.8×
  faster and the mixed workload about 5× faster.
- Index builds run in the background during the maintenance tick; queries
  stay correct (unindexed) until the index is ready.
- Facets and counts over broad filters stay linear in the number of matching
  records, even with indexes, because the counts are exact.

## Known limits

| Limit | Guidance |
|---|---|
| One commit pipeline per node | Batch writes; measure fsync latency on your disk. |
| Store files are ~40–50% full when many tenants grow side by side (B-tree page splits) | Budget ~2.5 KB per record; compaction bounds but does not remove this slack. |
| Filters without indexes scan the first condition's population | Declare ordered indexes on filtered properties. |
| Index builds use a small share of each maintenance tick | Expect minutes for several indexes over hundreds of thousands of records; plan index creation before peak load. |
| ANN latency grows with space size and right after bulk binds | Keep spaces moderately sized; allow time after bulk loads. |
| SQL reads a label as a table of at most 100,000 records | Use the native query or graph languages for larger labels. |

## See also

- [Deployment](deployment.md#8-gb-vps-profile)
- [Storage and compaction](storage-and-compaction.md)
- [Indexes](../indexes-and-search/indexes.md)
- [Limits](../reference/limits.md)
