# Replication

RushDB supports asynchronous streaming replication: one leader accepts
writes, and any number of read-only followers continuously replay its
write-ahead log. This page shows how to set up a follower, seed it from a
base backup, monitor it, and promote it. For how replication works
internally, see [Replication internals](../architecture/replication.md).

## How it works

- A follower polls the leader's `GET /internal/replication/wal` endpoint
  every 200 ms when caught up (immediately while behind), asking for
  commits after its own commit index. Each response carries up to 2,000
  commits.
- The follower applies each commit in order through its own write path and
  recomputes derived state (indexes, counters) locally. Only commits are
  shipped, never derived data.
- Each follower registers on the leader as a retention consumer named after
  its `--replica-id`, so the leader keeps the WAL the follower still needs,
  up to the leader's hard WAL caps.
- Replication is asynchronous: the leader acknowledges a commit without
  waiting for followers. A follower can lag; on leader loss, commits not
  yet fetched by the follower are lost if you promote it.
- A follower rejects writes with `unavailable` ("this node is a
  streaming-replication follower; writes must go to the leader"). Reads,
  `describe`, and transaction `begin`/`rollback` work.
- There is no automatic failover or leader election. Promotion is a manual
  operator action.

## Requirements

- Leader and followers should run the same RushDB version.
- Followers use the same tenancy mode as the leader, and normally the same
  language settings.
- The follower can reach the leader's HTTP port, and presents the leader's
  boot token (`--replica-token`, which defaults to the follower's own
  `--token`).
- Every follower of one leader has its own `--replica-id`. Two followers
  sharing an id overwrite each other's retention claim, and the leader may
  prune WAL one of them still needs.

## Option A: follower from an empty store

A follower that starts empty replays the leader's history from commit 1.
This only works while the leader still has its whole WAL, which by default
it does not: with no consumers, the leader keeps only what open readers
need. To serve empty followers, give the leader a keep window covering its
history, for example on a new leader:

```yaml
# leader config.yml
resources:
  tx-log-keep-commits: 100000
```

Start the follower on a new data directory:

```bash
RUSHDB_TOKEN=$LEADER_TOKEN rushdb serve \
  --data-dir /var/lib/rushdb-follower \
  --replica-of http://leader.internal:3000 \
  --replica-id follower-1
```

If the leader has already pruned the history the follower needs, the
follower logs `replication follower: leader requires resync from a fresh
base; stopping` with the oldest LSN the leader still has. Use option B.

## Option B: follower from a base backup

Seed the follower with a base backup of the leader, so it only needs the WAL
written after the base:

1. Take a base backup of the running leader (see
   [Backup and restore](backup-and-restore.md#2-take-a-base-backup)):

   ```bash
   curl -s -X POST -H "Authorization: Bearer $LEADER_TOKEN" \
     "http://leader.internal:3000/internal/backup/base?out=/srv/base-for-follower"
   rushdb serve --verify-base /srv/base-for-follower
   ```

2. On the follower host, create the data directory from the base's files:

   ```bash
   sudo install -d -o rushdb -g rushdb -m 0750 /var/lib/rushdb-follower
   sudo -u rushdb cp -a /srv/base-for-follower/base/. /var/lib/rushdb-follower/
   ```

3. Remove or change `wal-archive-dir` in `/var/lib/rushdb-follower/config.yml`
   (it is the leader's file) if the leader archives WAL, so the follower does
   not write into the leader's archive.

4. Start the follower with its role and the leader's token:

   ```bash
   RUSHDB_TOKEN=$LEADER_TOKEN rushdb serve --data-dir /var/lib/rushdb-follower \
     --replica-of http://leader.internal:3000 --replica-id follower-1
   ```

The follower resumes from the base's commit index and catches up.

> **Note:** `--replica-of`, `--replica-id` and `--replica-token` are this
> boot's role, so they never conflict with `config.yml`. Setting them in the
> `resources` section of `config.yml` instead works too, and survives
> restarts without the flags.

The leader must still hold the WAL from the base's commit index onward when
the follower first connects. Taking the base registers a `backup` retention
consumer on the leader that protects this range for
`retention-consumer-lease-seconds` (24 h by default), within the hard caps.
Start the follower well inside that window.

## Monitoring replication

| Signal | Where | What it tells you |
|---|---|---|
| Commit index on each node | `/ready` → `commitIndex`, or `rushdb_commits_total` | Lag = leader index − follower index. |
| Leader retention | `rushdb_retention_consumers`, `rushdb_retention_protected_floor`, `rushdb_wal_retained_bytes` on the leader | A stalled follower holds the protected floor back and WAL grows until the hard cap. |
| Follower log | `replication follower: poll failed, retrying` | Network or auth problem; retried every 200 ms. |
| Follower log | `replication follower: apply failed, retrying` | A commit could not be applied; replication is stalled and needs investigation. |
| Follower log | `replication follower: leader requires resync from a fresh base; stopping` | Rebuild the follower from a new base backup. |

A follower that stops polling is dropped from the leader's retention after
`retention-consumer-lease-seconds`, and the leader's hard caps
(`tx-log-retention-bytes`, default 4 GiB; `tx-log-retention-commits`,
default 100,000) always win over a lagging follower. Size them for the
longest follower outage you want to survive without a rebuild.

## Promoting a follower

To make a follower writable (for example after losing the leader):

1. Make sure the old leader is stopped or fenced off from clients. RushDB
   does not prevent two writable nodes; that decision is yours.
2. Promote:

   ```bash
   curl -s -X POST -H "Authorization: Bearer $RUSHDB_TOKEN" \
     http://follower.internal:3000/internal/replication/promote
   ```

   ```json
   {"promoted":true,"persisted":true}
   ```

   The call waits up to 5 s for the replication task to stop. A 503 means
   it has not stopped yet; call again. A node that was not following
   answers `{"promoted":true,"already_writable":true}`.
   Promotion is persisted: the node's `config.yml` no longer names the
   leader, so a restart stays writable. If you start the node with an
   explicit `--replica-of` flag (for example from a service unit), remove the
   flag too, or it follows again.
3. Point clients at the new leader. Rebuild the old leader as a follower of
   the new one from a fresh base backup; do not reuse its old data
   directory, which may contain commits the new leader never received.

## Raft (Preview)

`--raft N --experimental-raft` starts N Raft state machines **inside one
process** and routes every commit through the elected leader before it
returns. This is a Preview for evaluation only:

- the Raft log, votes, membership, and snapshots are kept in memory and lost
  on restart;
- there is no network transport, so all members run in the same process;
- there are no linearizable reads;
- it provides no high availability and no additional durability.

`--raft` without `--experimental-raft` is refused at startup. Use streaming
replication for real deployments.

## See also

- [Backup and restore](backup-and-restore.md)
- [Storage and compaction](storage-and-compaction.md#wal-retention)
- [Replication internals](../architecture/replication.md)
- [HTTP endpoints](../reference/http-endpoints.md#internalreplicationwal)
