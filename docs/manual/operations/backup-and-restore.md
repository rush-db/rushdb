# Backup and restore

RushDB offers three backup mechanisms with different trade-offs. This page
explains what each one captures, how to run it, how to restore, and how to
verify the result.

## Choosing a method

| Method | Server state | Captures | Restore | Use it for |
|---|---|---|---|---|
| **Base backup + WAL archive** (`--backup-base` or `POST /internal/backup/base`, `--wal-archive-dir`, `--restore-pitr`) | base: stopped or running; archive: running | The complete store: data, declared indexes, vector and text state, policies, transaction outcomes, configuration | Any commit inside the archive window | Disaster recovery, point-in-time recovery, seeding followers |
| **Graph snapshot** (`--export`, `--restore`) | stopped | Records and relationships of every tenant | Into a fresh data directory | Moving data between servers or storage formats, interchange |
| **Protocol `backup` operation** | running | Records and relationships of one scope, as JSONL in the response | No built-in restore command | Small per-tenant logical exports from an application |

> **Note:** The graph snapshot and the `backup` operation do **not** include
> vector spaces and bindings, ANN index state, text index profiles, declared
> indexes, data-access policies, transaction outcomes, or MVCC history.
> After restoring one, re-declare your indexes. Use a base backup for
> complete recovery.

## Base backups and point-in-time recovery

A complete recovery setup has two parts:

1. **Continuous WAL archiving** on the running server: every 5 seconds the
   server appends the newly committed range of the write-ahead log to an
   archive directory.
2. **Periodic base backups**: a physical copy of the store pinned at one
   commit index.

A base plus the archived WAL after it reconstructs any commit inside the
archive's retention window.

### 1. Enable WAL archiving

Add the archive directory to `config.yml` (or pass `--wal-archive-dir` on
the first start) and restart:

```yaml
resources:
  wal-archive-dir: /srv/rushdb-archive
  wal-archive-retention-seconds: 604800   # 7-day PITR window (default)
  wal-archive-retention-bytes: 0          # optional hard cap; 0 = none
```

The archive directory contains:

| File | Content |
|---|---|
| `wal-<from>-<to>.seg` | One append-only segment per archive tick; `<from>` and `<to>` are 20-digit commit indexes. |
| `archive.state` | The last archived commit index (the watermark). |
| `bases.json` | Base backups registered with this archive. |

Put the archive on a different disk or host than the data directory (for
example a mounted network volume) so that losing the data disk does not
lose the archive.

How archive retention works:

- Segments older than the window are deleted once the newest base taken at or
  before the window's start no longer needs them, so the window always
  starts at a restorable base.
- `wal-archive-retention-bytes` deletes the oldest segments even inside the
  window when the archive exceeds it, and logs a warning (`WAL archive byte
  cap removed segments inside the retention window`). The restorable window
  shrinks accordingly.
- Segments are removed oldest first, so the archive never has a hole in the
  middle.
- The archiver is a retention consumer of the live WAL. If it falls so far
  behind that the WAL's hard caps (`tx-log-retention-bytes`,
  `tx-log-retention-commits`) prune past it, the log shows `WAL archive
  fell behind the retention cap` and the archive has a gap until the next
  base backup.

### 2. Take a base backup

Take a base from the running server with the operator endpoint (boot
token). The server writes the base to `out` on its own filesystem:

```bash
curl -s -X POST -H "Authorization: Bearer $RUSHDB_TOKEN" \
  "http://127.0.0.1:3000/internal/backup/base?out=/srv/rushdb-backups/base-2026-10-10"
```

```json
{"commitIndex":182344,"files":3}
```

The store files are copied while quiescent at a durable commit: reads
continue, and commits wait until the copy finishes (roughly the time to copy
the store files). A target that already exists is refused with 409.

Or take it offline: stop the server, run the backup with the same data
directory and archive settings, and start the server again:

```bash
sudo systemctl stop rushdb
sudo -u rushdb rushdb serve --data-dir /var/lib/rushdb \
  --wal-archive-dir /srv/rushdb-archive \
  --backup-base /srv/rushdb-backups/base-2026-10-10
sudo systemctl start rushdb
```

```text
backup-base: 3 file(s) at commit index 182344 -> `/srv/rushdb-backups/base-2026-10-10`
```

- The target directory must not exist. The backup is staged in a temporary
  directory and renamed into place only when complete.
- It contains `base/` (the store files, layout manifest, and `config.yml`)
  and `manifest.json` (format version, commit index, and each file's size
  and CRC32).
- Passing `--wal-archive-dir` registers the base in the archive's
  `bases.json`, which archive retention needs to keep the right segments.
- Taking a base registers a `backup` retention consumer at its commit index,
  so the live WAL from that point is protected until the consumer's lease
  (`retention-consumer-lease-seconds`, default 24 h) expires, within the
  hard caps.

Opening the store replays the WAL into the store file, so an offline base
holds every commit even after an unclean stop.

### 3. Verify a base backup

```bash
rushdb serve --verify-base /srv/rushdb-backups/base-2026-10-10
```

```text
verify-base: OK at commit index 182344
```

This recomputes every file's size and CRC32. Run it after copying a base to
other storage, and before relying on it.

### 4. Restore to a point in time

Restore into a **new** data directory (it must not exist yet). The command
verifies the base, copies it into place, and replays archived WAL from the
base's commit index up to `--to-lsn`, or to the newest archived commit if you
omit it:

```bash
sudo -u rushdb rushdb serve \
  --data-dir /var/lib/rushdb-restored \
  --wal-archive-dir /srv/rushdb-archive \
  --restore-pitr /srv/rushdb-backups/base-2026-10-10 \
  --to-lsn 190000
```

```text
restore-pitr: replayed archived WAL to commit index 190000
```

Then point the server at the restored directory:

1. Review `/var/lib/rushdb-restored/config.yml`. It comes from the source
   server, but the restore removes the source's `wal-archive-dir` and
   replica role, so the restored server never writes into the original
   archive or follows the source's leader. Set a new `wal-archive-dir` if
   it should archive, and adjust ports if both servers run on one host.
2. Start `rushdb serve --data-dir /var/lib/rushdb-restored`.
3. Check `/ready` and spot-check data (see [Verification](#verification)).

Things to know:

- `--wal-archive-dir` here is the archive to replay from; it can be any
  path holding the archived segments. The command checks it before it
  copies the base.
- The commit index is the LSN. Find the index you want from the server log,
  the `commitIndex` field of `/ready`, `rushdb_commits_total`, or the
  `commitIndex` of a transaction commit response.
- The archive is written every 5 seconds, so the newest few seconds of
  commits before a disk loss may not be in it.
- A target outside the archive's reach fails with an error about a gap;
  restore never leaves a partially restored store behind.

## Graph snapshot export and restore

`--export` writes a consistent, checksummed snapshot of every tenant's
records and relationships, pinned at one commit index. The store can be
opened by only one process, so stop the server first:

```bash
sudo systemctl stop rushdb
sudo -u rushdb rushdb serve --data-dir /var/lib/rushdb --export /srv/exports/graph-2026-10-10.snap
sudo systemctl start rushdb
```

```text
export: wrote snapshot at commit index 182344 to `/srv/exports/graph-2026-10-10.snap`
```

The file is published atomically and never overwrites an existing file.

Restore it into a fresh data directory, created with the same tenancy mode
and languages you want the new server to have:

```bash
sudo -u rushdb rushdb serve --data-dir /var/lib/rushdb-new \
  --restore /srv/exports/graph-2026-10-10.snap
```

```text
restore: chunk 0 -> 51234 records, 80211 relationships applied
restore: applied 51234 records and 80211 relationships across 1 chunks into `/var/lib/rushdb-new`
```

- The whole file is validated (format, version, per-chunk CRC) before
  anything is applied; a truncated or corrupted file is rejected with a
  non-zero exit and the target is left untouched.
- Restore replays the data as a series of commits, so it is bounded by the
  usual transaction and commit limits. Very large tenants may need raised
  limits.
- Record identities are preserved.
- Afterwards, re-declare ordered, text, and vector indexes and re-upload
  vectors.

## The `backup` protocol operation

Applications can request a logical backup of their own scope from a running
server. It needs `read` permission.

```bash
curl -s -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"backup","request":{"scopes":[]}}}' \
  http://127.0.0.1:3000/protocol/v1/execute
```

The response contains `commitIndex` and, per scope, `jsonl` (one line per
record or relationship), `records`, and `relationships`.

- In multitenant mode the only scope you may request is your own (an empty
  list means your scope).
- The whole backup is in one response, so it is limited by
  `server.body-limit` (16 MiB by default). It suits small tenants only.
- There is no CLI command to restore this JSONL; use base backups or graph
  snapshots for operator-driven recovery.

## Verification

| Check | Command |
|---|---|
| Base backup integrity | `rushdb serve --verify-base <BASE_DIR>` |
| Logical integrity of a data directory (server stopped) | `rushdb serve --data-dir <DIR> --verify` → `verify: no violations` |
| Restored commit index | `curl -s http://127.0.0.1:3000/ready` → `commitIndex` |
| Record counts | `describe` operations, or `$count` queries, compared with the source |

Schedule regular restore drills: restore the latest base plus archive into a
scratch directory, run `--verify`, start a server on it, and compare counts.

## Example schedule

| Task | Frequency |
|---|---|
| WAL archiving | continuous (every 5 s) |
| Base backup (short maintenance window) | daily or weekly; at least once per archive window |
| `--verify-base` on the copied base | after every base |
| Restore drill | monthly |
| Copy base and archive off the host | continuously or daily |

Keep at least one base older than the start of the archive window, or the
archive cannot reach the window's beginning.

## See also

- [Replication](replication.md) — seeding a follower from a base backup
- [Storage and compaction](storage-and-compaction.md) — WAL retention
- [CLI reference](../reference/cli-reference.md#maintenance-commands)
- [Security](security.md#backups-and-secrets)
