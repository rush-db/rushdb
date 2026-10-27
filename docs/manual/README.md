# RushDB manual

RushDB is a standalone, persistent property-graph database in one Rust
process. It stores connected records with typed properties and directed,
typed relationships. It learns its schema from the data, and it answers
native JSON queries, SQL, ISO GQL, openCypher, Gremlin, SQL/PGQ and
Mongo-style statements. Vector, full-text and hybrid search are built in, and
clients connect over HTTP, the PostgreSQL wire protocol, gRPC and Bolt.

This manual describes the engine as it works today. Where a feature is not
yet generally available, the page states the status the engine itself
reports: *stable*, *release candidate*, *coming soon* or *preview*.

## How this manual is organized

| Section | Read it when you want to… |
|---|---|
| [Getting started](getting-started/README.md) | understand what RushDB is, install it, and run your first queries |
| [Data model](data-model/README.md) | learn how records, labels, properties, relationships, the schema catalog and tenants work |
| [Querying and writing data](query/README.md) | find, filter, traverse, sort, page, aggregate and write data with the native API, and use transactions |
| [Query languages](languages/README.md) | use SQL, GQL, openCypher, Gremlin, SQL/PGQ or Mongo-style statements against the same graph |
| [Connectivity](connectivity/README.md) | connect over HTTP, PostgreSQL wire, gRPC or Bolt, use the CLI, and authenticate |
| [Indexes and search](indexes-and-search/README.md) | declare indexes, understand query planning, and run vector, full-text and hybrid search |
| [Architecture](architecture/README.md) | understand the internals: storage engine, write path, MVCC, on-disk layout, execution, multitenancy, replication, resource governance |
| [Operations](operations/README.md) | deploy, configure, secure, back up, replicate, monitor, size and troubleshoot a server |
| [Reference](reference/README.md) | look up a setting, command, endpoint, error code, metric, limit or term |

## Suggested reading paths

- **New to RushDB:** [Introduction](getting-started/introduction.md) →
  [Quickstart](getting-started/quickstart.md) →
  [Concepts](getting-started/concepts.md) →
  [Search queries](query/search-queries.md).
- **Application developer:** [Data model](data-model/README.md) →
  [Filtering](query/filtering.md) →
  [Writing data](query/writing-data.md) →
  [Transactions](query/transactions.md) →
  [Indexes](indexes-and-search/indexes.md).
- **Coming from SQL, Cypher or MongoDB:** [Query languages](languages/README.md),
  then the page for your language, then
  [Connectivity](connectivity/README.md).
- **Operator:** [Deployment](operations/deployment.md) →
  [Configuration](operations/configuration.md) →
  [Security](operations/security.md) →
  [Backup and restore](operations/backup-and-restore.md) →
  [Monitoring](operations/monitoring.md).
- **Understanding the internals:** [Architecture overview](architecture/overview.md) →
  [Storage engine](architecture/storage-engine.md) →
  [Write path](architecture/write-path.md) →
  [MVCC and snapshots](architecture/mvcc-and-snapshots.md).

## Conventions

- HTTP examples use `curl` against `http://127.0.0.1:3000`, with the token in
  `$RUSHDB_TOKEN`. In multitenant mode they also send the tenant scope (see
  [Tenants](data-model/tenants.md)).
- `rushdb` is the binary; `rushdb serve` runs the server and `rushdb up` a local node.
- `> **Note:**`, `> **Tip:**` and `> **Warning:**` blocks mark asides,
  shortcuts and pitfalls.
