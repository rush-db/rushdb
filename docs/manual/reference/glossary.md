# Glossary

Terms used throughout the RushDB manual.

| Term | Definition |
|---|---|
| **Base backup** | A physical copy of the store files plus a `manifest.json` with sizes and CRC32 checksums, pinned at one commit index. Taken with `rushdb serve --backup-base`. The starting point for point-in-time restore and for attaching a follower. See [Backup and restore](../operations/backup-and-restore.md). |
| **Boot token** | The secret given with `--token` / `RUSHDB_TOKEN`. Grants full access on every transport and is the pgwire password. Not bound to a tenant. |
| **Capability token** | A signed, expiring, permission-scoped (and optionally tenant-bound) token accepted on HTTP when `--capability-secret` is set. See [Security](../operations/security.md#capability-tokens). |
| **Checkpoint** | The point at which applied commits become durable in the store file, after which WAL segments can be reclaimed. Runs on every maintenance tick and at clean shutdown. |
| **Commit index (CI), LSN** | The sequence number of a durable commit. RushDB uses the same number as the WAL log sequence number, so `--to-lsn` takes a commit index. |
| **Compaction** | Rewriting the store file to release free pages and retained history. Runs automatically when the store is idle and has grown, or on demand with `rushdb serve --compact`. See [Storage and compaction](../operations/storage-and-compaction.md). |
| **`config.yml`** | The configuration record inside a data directory. Written on first start and authoritative for that directory. Never contains secrets. |
| **Cursor** | A server-side, snapshot-pinned position in a records query, continued with `query.cursor`. Lives only in the server process and expires. |
| **Data directory** | The directory given with `--data-dir`, holding the store file(s), their WAL directory, and `config.yml`. |
| **Declared index** | An ordered, text, or vector index you create explicitly. RushDB does not create property indexes automatically. See [Indexes](../indexes-and-search/indexes.md). |
| **Derived state** | Counters, presence lists, and dictionaries the engine maintains from records (for example O(1) label counts). Recomputed from data, never replicated. |
| **Durability mode** | `local` (fsync before acknowledging, default), `async` (acknowledge before fsync), or `memory` (in-memory nodes). |
| **Follower / leader** | In streaming replication, the follower polls the leader's WAL and replays it; the follower is read-only until promoted. See [Replication](../operations/replication.md). |
| **Group commit** | Several concurrent commits written to the WAL with one fsync by the single publisher. |
| **History floor (GC watermark)** | The oldest commit whose snapshot can still be read. Older MVCC versions are reclaimed. |
| **Label** | The type name of a record (for example `User`). A record has one primary label. |
| **LMPG** | Labeled Meta-Property Graph, RushDB's data model and its storage layout: properties are first-class typed definitions, and records, property values, and relationships are stored as key families in an ordered key-value store. See [LMPG layout](../architecture/lmpg-layout.md). |
| **Maintenance mode** | A startup switch (`--maintenance`) that rejects writes and fails readiness while reads continue. |
| **Maintenance tick** | The background cycle (every 5 s by default) that runs GC, WAL retention, index builds, vector segment maintenance, and the durability checkpoint. |
| **Managed envelope** | The memory budget (`managed-limit`) from which caches, the transaction pool, and query phases reserve bytes. Not the process RSS. |
| **MVCC** | Multi-version concurrency control: every read sees one commit's snapshot while writers continue. |
| **Point-in-time restore (PITR)** | Restoring a base backup and replaying archived WAL up to a chosen commit index. |
| **Protocol v1** | RushDB's transport-neutral request/response envelope, served at `POST /protocol/v1/execute` and over gRPC and Bolt. |
| **Publisher** | The single component that appends each group of commits to the WAL, fsyncs, and applies them to the store. All tenants share it. |
| **Purge (tenant)** | `purgeTenant`: delete every key of one tenant. The freed space is returned by the next compaction. |
| **Read view** | A pinned snapshot at one commit index used by a query, cursor, transaction, or export. |
| **Record** | A node in the graph: an id, a label, and properties. |
| **Relationship** | A typed, directed edge between two records. |
| **Retention consumer** | Something that still needs WAL history: a backup, the WAL archiver, a follower, or a cursor. Consumers protect the WAL from pruning up to the hard caps and expire after their lease. |
| **Retry receipt** | The stored outcome of a commit, kept for the idempotency window so a retried commit returns the original result. |
| **Scope (tenant scope)** | The string a request presents to select its tenant in multitenant mode: the `X-RushDB-Tenant` header, pgwire `database`, gRPC `x-rushdb-tenant`, or Bolt `db`. |
| **Search query** | RushDB's native JSON query document (`query.records`), always enabled. |
| **Shedding** | Refusing a request immediately with a retryable 503 when the scheduler or the in-flight body budget is full, instead of queueing without bound. |
| **Statement language** | An optional text query language: SQL, GQL, Cypher, Gremlin, SQL/PGQ, or MongoDB query documents. Enabled with `--languages`. |
| **Tenant** | An isolated keyspace inside one server. In multitenant mode every request selects one by scope; nothing identifying the tenant is stored on records. See [Tenants](../data-model/tenants.md). |
| **WAL** | Write-ahead log: the durable, ordered log of commits, stored beside the store file in `rushdb.redb.wal/`. |
| **WAL archive** | A directory of WAL segment files (`wal-<from>-<to>.seg`) written continuously when `--wal-archive-dir` is set, used for point-in-time restore. |

## See also

- [Concepts](../getting-started/concepts.md)
- [Architecture overview](../architecture/overview.md)
