# Feature status

How mature each surface of RushDB is, in one table. Every row links to the
page that defines its exact behavior and limits; those pages are the
reference, and this table follows them.

| Area | Status |
|---|---|
| Native JSON query language, [SQL](../languages/sql.md), [Gremlin text traversals](../languages/gremlin.md), [Mongo-style documents](../languages/mongo.md) | Stable |
| [SQL/PGQ `GRAPH_TABLE`](../languages/sql-pgq.md) | Stable, read-only |
| [openCypher](../languages/cypher.md), [ISO GQL](../languages/gql.md), [Bolt](../connectivity/bolt.md) | Release candidate, opt-in: the external conformance suites do not pass in full |
| [PostgreSQL wire](../connectivity/postgresql-wire.md) | SQL reads and single-statement writes; checked with `psql`, node-postgres, psycopg, `tokio-postgres` and Metabase, not every client or ORM |
| [Replication](../operations/replication.md) | Asynchronous streaming to read-only followers; promotion is manual, with no automatic failover |
| [Raft](../operations/replication.md#raft-preview), [storage sharding](../architecture/storage-engine.md#experimental-multi-shard-layout) | Preview, not for production: Raft runs in one process with in-memory state; multi-shard commits are not crash-atomic |

The query-language statuses are the ones the server reports through
`describe.queryLanguages`; see
[Discovering capabilities](../languages/README.md#discovering-capabilities-at-runtime).
