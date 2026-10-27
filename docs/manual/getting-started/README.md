# Getting started

This section takes you from "what is RushDB?" to a running node with data in
it. Read it in order if you are new to RushDB; each page is short and ends with
links to the deeper material.

RushDB is a standalone, persistent property-graph database that runs as one
process. You write JSON records, connect them with typed relationships, and
query them through a native JSON protocol or through SQL, GQL, openCypher,
Gremlin, SQL/PGQ and Mongo-style statements.

## Pages

1. [Introduction](introduction.md) — what RushDB is, the data model in one
   paragraph, the query surfaces, and what is deliberately not in the engine.
2. [Installation](installation.md) — install with one command, Homebrew,
   Docker, Cargo or a release archive; start a node with `rushdb up`; choose a
   data directory; and set your own access token.
3. [Quickstart](quickstart.md) — a ten-minute tour: create records, ingest
   nested JSON, query, link records, run a vector search, and connect with
   `psql`.
4. [Concepts](concepts.md) — short definitions of record, label, property,
   relationship, tenant, transaction, snapshot, index and capability profile,
   each linking to its full page.

## Where to go next

- [Data model](../data-model/README.md) — how RushDB stores records,
  properties, relationships and tenants.
- [Querying](../query/README.md) — the native query document in depth.
- [Connectivity](../connectivity/README.md) — HTTP, PostgreSQL wire, gRPC,
  Bolt and the CLI.
- [Operations](../operations/README.md) — deploying and running RushDB in
  production.
