# Introduction

This page explains what RushDB is, how it models data, which query languages
and wire protocols it speaks, and which concerns it deliberately leaves to
other software. Read it first if you are deciding whether RushDB fits your
application.

## What RushDB is

RushDB is a persistent property-graph database delivered as a single Rust
binary, `rushdb`: `rushdb serve` runs the server and `rushdb up` runs a local
node. One process
gives you:

- **Durable storage** — an embedded ACID store with a write-ahead log,
  snapshots, export/restore and point-in-time recovery.
- **Connected records** — JSON records linked by typed, directed
  relationships, with an emergent schema you never have to declare up front.
- **Search** — structured filters, traversal across relationships,
  aggregations, BM25 full-text search, vector similarity search and hybrid
  search that fuses both.
- **Several query surfaces at once** — a native JSON protocol plus SQL, ISO
  GQL, openCypher, Gremlin, SQL/PGQ and Mongo-style statements, all over the
  same data.
- **Several wire protocols at once** — HTTP, the PostgreSQL wire protocol,
  gRPC and Bolt, all served by the same node.
- **Tenant keyspaces** — an optional multitenant mode that isolates each
  tenant's data in its own keyspace.

RushDB is licensed under Apache-2.0.

## The data model in one paragraph

RushDB stores data in its **Labeled Meta-Property Graph (LMPG)** model, a
graph in which properties are first-class, typed structures rather than
key-value maps inside each record, with a few deliberate rules. Every
**record** has exactly one primary **label** (such as `Person` or `Order`)
and a set of **properties**. A property holds typed
values — boolean, integer, float, string or datetime — and may hold several
values at once (an array) or an explicit `null`, which is different from the
property being absent. Records are connected by **relationships** that have a
type (such as `WORKS_AT`), a direction, and optionally their own properties.
There is no schema to declare: the engine records every label, property and
relationship shape it sees in a **schema catalog** that you can inspect at any
time. Nested JSON you ingest is unfolded into records joined by
relationships. See [Data model](../data-model/README.md) for the details.

## Query surfaces

Every request, in every language, runs against the same records and the same
engine. The native query document is always available; statement languages
are switched on per node.

| Surface | What it is | Access | Status advertised by the engine |
|---|---|---|---|
| Native query document | JSON `labels` / `predicate` / `select` / `order` document | Read and write | Always enabled |
| SQL | PostgreSQL-dialect relational SQL; labels appear as tables | Read and write | Stable |
| ISO GQL | GQL property-graph profile (a subset of ISO/IEC 39075 syntax) | Read and write | Release candidate |
| openCypher | openCypher property-graph profile | Read and write | Release candidate |
| Gremlin | Apache TinkerPop Gremlin text traversals | Read and write | Stable |
| SQL/PGQ | SQL:2023 `GRAPH_TABLE` queries | Read only | Stable |
| Mongo-style | `find` / `aggregate` / CRUD / index statements | Read and write | Stable |

The status column is what the node itself reports through
`describe.queryLanguages`; clients should negotiate features from that
response rather than from a version string. See
[Query languages](../languages/README.md).

## Wire protocols

| Protocol | Default port | Carries | Enabled when |
|---|---|---|---|
| HTTP (`POST /protocol/v1/execute`) | 3000 | The complete native protocol and every enabled language | Always |
| PostgreSQL wire | 5432 | SQL | The `sql` language is enabled |
| gRPC (`rushdb.v1.Database/Execute`, unary) | disabled | The same envelope as HTTP | A gRPC port is set |
| Bolt 5.0 | disabled | openCypher statements | A Bolt port is set and `cypher` is enabled |

HTTP is the complete data-plane API: everything the engine can do is
expressible there. The other protocols expose the subset that maps naturally
onto them. See [Connectivity](../connectivity/README.md).

## Capabilities at a glance

| Area | What the engine provides |
|---|---|
| Durability | A segmented write-ahead log is the durable truth; the embedded store is a checkpointed copy, and crash recovery replays the log tail. WAL retention has hard caps, and idle compaction runs in the background. See [Storage engine](../architecture/storage-engine.md). |
| Transactions | Snapshot isolation, interactive `begin` / `stage` / `commit` / `rollback`, and idempotent outcome discovery on commit. See [Transactions](../query/transactions.md). |
| Querying | Filters, traversal, aggregation, facets, declared ordered indexes, and snapshot-pinned cursors in the native query document; relational SQL; GQL, openCypher, Gremlin and SQL/PGQ; property-overlap joins. See [Querying and writing data](../query/README.md). |
| Tenancy | Single-tenant or multitenant mode. Each tenant is an isolated keyspace; in multitenant mode the scope is mandatory and fail-closed, and records carry no tenant property. See [Tenants](../data-model/tenants.md). |
| Authorization | A bootstrap token, signed capability tokens, and graph-aware data-access policies inside a tenant. See [Authentication](../connectivity/authentication.md). |
| Vector search | Named vector indexes with exact reranking over segmented approximate-nearest-neighbour indexes, and stale-binding detection. See [Vector search](../indexes-and-search/vector-search.md). |
| Full-text search | Declared BM25 text indexes with `immediate` or `eventual` consistency, and hybrid fusion with graph and vector results. See [Full-text search](../indexes-and-search/full-text-search.md). |
| Replication | Streaming WAL replication to read-only followers, attachable from an empty store or a base backup, with promotion. Raft is an experimental Preview. See [Replication](../operations/replication.md). |
| Backup | Base backups plus a WAL archive for point-in-time recovery, and checksummed graph-snapshot export and restore. See [Backup and restore](../operations/backup-and-restore.md). |

## What is not in the engine

RushDB is a database, not an application platform. The engine does **not**
include:

- **A dashboard or admin UI.** You interact with a node through its protocols
  and the CLI.
- **A hosted control plane.** There are no projects, workspaces, users,
  billing or provisioning concepts inside the engine. A node knows about an
  access token, optional signed capability tokens, and tenant scopes — nothing
  more.
- **An embedding model.** RushDB indexes and searches vectors you supply; it
  never calls a model API to generate them. Compute embeddings in your
  application (or your own pipeline) and write them with the record or with
  `upsertVectors`.
- **Agent prompts, AI workflows or approval workflows.**

These capabilities belong to the application layer around the database. RushDB
Cloud, the managed service, provides them on top of the engine; a standalone
node does not need them for SQL, graph, document or vector workloads.

## See also

- [Installation](installation.md)
- [Quickstart](quickstart.md)
- [Concepts](concepts.md)
- [Architecture overview](../architecture/overview.md)
