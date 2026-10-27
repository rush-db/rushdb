# Storage and compaction

How a RushDB data directory grows, what the background maintenance does
about it, which settings control WAL retention and compaction, and how the
disk guard protects the server. For capacity numbers see
[Capacity planning](capacity-planning.md); for the storage design see
[Storage engine](../architecture/storage-engine.md).

## Where the bytes go

A data directory holds the store file (`rushdb.redb`), its write-ahead log
(`rushdb.redb.wal/`), and small metadata files. Inside the store:

| Category | Contents | Metric |
|---|---|---|
| Live data | Records, relationships, properties, derived counters and dictionaries, declared indexes | `rushdb_storage_data_bytes` |
| MVCC history | Older versions of overwritten keys that a snapshot may still read | `rushdb_storage_history_bytes` |
| Retry receipts | The stored outcome of each recent commit, for idempotent retries | `rushdb_storage_receipt_bytes` |
| Free and partly filled pages | Space left behind by updates, deletes, and page splits | `rushdb_storage_file_bytes` minus the above |

And beside it:

| File | Contents | Metric |
|---|---|---|
| `rushdb.redb.wal/` | Committed command log in 64 MiB segments | `rushdb_wal_retained_bytes` |

The byte categories inside the store are refreshed every 15 minutes; the
file size and WAL size are live.

### Growth model

- **Live data** grows with your records: about 700 bytes per 10-field record
  in the measured fixture, plus ~26 bytes per record per ordered index.
- **History** stays near zero: the maintenance tick (every 5 s) reclaims
  superseded versions as soon as no open snapshot needs them. A long-lived
  reader (an open cursor, transaction, or export) holds history back; an
  abandoned read view is released after 60 s, a cursor after its lifetime.
- **Receipts** expire by time: one per commit for
  `idempotency-window-seconds` (24 h by default). Their total tracks your
  commit rate over that window.
- **The WAL** is pruned at each checkpoint down to what someone still needs
  (see [WAL retention](#wal-retention)).
- **Page slack.** The store is copy-on-write: freed pages are reused, but the
  file itself is a high-water mark and only shrinks when compacted. When
  many tenants grow side by side, B-tree pages end up about 40–50% full;
  this is why the measured total is ~2.5 KB per record while live data is
  ~700 bytes.

## Compaction

Compaction rewrites the store file to release free pages and reclaim
retained history. Without it, a store that once grew large stays large, and
commit and scan costs creep up with file size.

### Automatic idle compaction

On by default. Every `compact-interval-seconds` (300 s) the server checks
three gates, and compacts only if all pass:

1. **Quiet:** no commit in the last `compact-quiesce-seconds` (10 s).
2. **Interval:** at least one interval since the previous check (the check
   itself runs once per interval).
3. **Worth it:** the file has grown to at least `compact-growth-percent`
   (150%) of its size after the previous compaction, **or** at least 10,000
   keys were purged since then (a tenant purge frees space without growing
   the file). The first check after startup counts as "worth it".

If a reader holds the store, the server retries for up to 2 s, then defers
to the next interval. Outcomes are logged: `storage compaction complete`
with `before_bytes` and `after_bytes` (info), or `compaction deferred`.

| Setting | Default | Effect |
|---|---|---|
| `compact-interval-seconds` | 300 | Check period and minimum spacing; `0` disables automatic compaction. |
| `compact-quiesce-seconds` | 10 | Quiet time required after the last commit. |
| `compact-growth-percent` | 150 | File growth needed to trigger. |

Because of the quiet gate, compaction does not run during sustained write
load. In measurements, gated compaction at a 1 s interval left ingest speed
unchanged, while ungated compaction during writes slowed ingest about 70×.
Compaction brings the file down toward the live size but not below it:
expect the file to remain larger than live data.

After `purgeTenant`, the freed space is returned at the first compaction
after the store goes quiet (in the measured run, half the tenants purged,
the store shrank to 0.58× at the next 300 s tick).

### Offline compaction

To reclaim space deterministically, for example after a large purge or
bulk delete, stop the server and run:

```bash
sudo systemctl stop rushdb
sudo -u rushdb rushdb serve --data-dir /var/lib/rushdb --compact
sudo systemctl start rushdb
```

```text
compact: store compacted at commit index 182344
```

It needs exclusive access and fails if the server is still running. Pass the
same tenancy flag the directory was created with.

## WAL retention

The WAL keeps only what someone still needs, like PostgreSQL with
`wal_keep_size = 0`. At each checkpoint it is truncated (a whole segment at
a time) up to the lowest of:

- the oldest commit an open reader is using;
- the oldest commit a **retention consumer** still needs: a follower
  (`replica-<id>`), the WAL archiver (`wal-archive`), a recent base backup
  (`backup`), or an open cursor;
- the newest commit minus `tx-log-keep-commits`.

Two hard caps override every consumer:

| Setting | Default | Effect |
|---|---|---|
| `tx-log-retention-bytes` | 4 GiB | Oldest commits are pruned until the WAL fits. `0` = no byte cap. |
| `tx-log-retention-commits` | 100,000 | The WAL never holds more than the last N commits. |
| `tx-log-keep-all` | false | Removes the commit cap (the byte cap still applies). |
| `tx-log-keep-commits` | 0 | Commits kept even when nobody needs them (for followers that start empty). |
| `retention-consumer-lease-seconds` | 86,400 | A consumer that stops acknowledging is dropped after this long. |

A consumer that falls behind a hard cap is dropped: a follower must be
rebuilt from a new base backup, and the WAL archive has a gap until the next
base backup. Peak WAL size is the byte cap plus one 64 MiB segment.

Watch `rushdb_wal_retained_bytes`, `rushdb_retention_consumers`, and
`rushdb_retention_protected_floor`.

## Disk guard

When free space on the data directory's filesystem drops below
`disk-space-threshold-percent` (default 10%):

- write operations on every transport are refused with `unavailable`
  ("free disk below the configured threshold; writes rejected"; HTTP 503);
- `/ready` returns 503 with reason `free disk below threshold`;
- reads keep working.

Writes resume automatically once space is freed. Set the threshold to `0`
to disable the guard (not recommended).

> **Warning:** The guard is enforced on the HTTP transport. Writes arriving
> over PostgreSQL wire, gRPC, or Bolt are not checked against it. If the
> disk does fill up mid-commit, the write path stops, `/ready` reports
> `durable publisher failed`, and the server must be restarted after space
> is freed.

## Diagnosing growth

| Observation | Likely cause | Action |
|---|---|---|
| `rushdb_storage_file_bytes` much larger than data + history + receipts | Page slack after deletes, purges, or rewrites | Let idle compaction run, or `--compact` offline. |
| `rushdb_storage_history_bytes` growing | A long-lived reader | Check `rushdb_retention_oldest_reader_commit`; close long cursors and transactions. |
| `rushdb_wal_retained_bytes` growing | A follower, the archiver, or a base backup consumer is behind | Check `rushdb_retention_protected_floor` and follower logs. |
| `rushdb_storage_receipt_bytes` large | High commit rate × idempotency window | Lower `idempotency-window-seconds` if you do not need 24 h retries. |
| `rushdb_storage_data_bytes` growing | Real data | Plan capacity. |

## Guidance

- **Bulk loads:** leave compaction on; the quiet gate keeps it out of the
  way. Larger batches reduce churn per record.
- **Between large campaigns:** purge the tenants you no longer need, then run
  `--compact` with the server stopped.
- **Measuring load times:** record `rushdb_storage_file_bytes` next to each
  measurement; a cold page cache and a large uncompacted file both slow the
  first pass.

## See also

- [Capacity planning](capacity-planning.md)
- [Backup and restore](backup-and-restore.md)
- [Monitoring](monitoring.md)
- [Configuration reference](../reference/configuration-reference.md#maintenance-retention-and-compaction)
