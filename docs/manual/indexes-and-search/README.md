# Indexes and search

This section covers the structures you declare to make RushDB answer
questions faster, and the three ranked-retrieval features built on top of
them: vector similarity search, full-text (BM25) search, and hybrid search
that fuses both. It is written for application developers and operators who
already know how to read and write records (see [Query](../query/README.md)).

RushDB creates **no value indexes automatically**. Every record is stored in
the LMPG occurrence layout, and the engine maintains catalogs (label
membership, property presence, value dictionaries) that let the planner narrow
most filters without help. When a filter on one label's property needs to be
answered directly from sorted values, you declare an index on it. You decide
what is fast; you pay the maintenance cost only for what you declared.

| Structure | Declared with | Answers | Page |
|---|---|---|---|
| Ordered index | `index.create` (`kind: "ordered"`), `CREATE INDEX` | equality, `$in`, numeric/date ranges, `$startsWith` on one label's property | [Indexes](indexes.md) |
| Text index | `index.create` (`kind: "text"`), `CREATE FULLTEXT INDEX` | BM25 keyword search over one property or every string property | [Full-text search](full-text-search.md) |
| Vector index | `index.createVector`, SQL `CREATE VECTOR INDEX`, Mongo `createIndex` with `dimensions` | exact or approximate nearest-neighbour search | [Vector search](vector-search.md) |
| Structural index | `index.create` (`kind: "structural"`) | records in similar positions in the graph, through vector search | [Structural embeddings](structural-embeddings.md) |

## Pages in this section

1. [Indexes](indexes.md) — declare, list and drop ordered indexes; lifecycle
   states; online builds; what queries can use them; maintenance cost.
2. [Query planning](query-planning.md) — how the planner chooses between
   index access and scans, statistics on demand, and how to read
   `describe.explain`.
3. [Vector search](vector-search.md) — vector indexes (immutable spaces),
   binding vectors to records, exact versus ANN search, metrics, filters and
   staleness.
4. [Full-text search](full-text-search.md) — text indexes, consistency modes,
   query syntax and scoring.
5. [Hybrid search](hybrid-search.md) — combine vector and lexical ranking with
   reciprocal-rank fusion inside one filtered candidate set.
6. [Structural embeddings](structural-embeddings.md) — vectors the engine
   derives from the graph around each record (FastRP), kept current as
   relationships change and searched like any vector index.

## Conventions used in this section

Examples call the HTTP protocol endpoint `POST /protocol/v1/execute` on the
default port `3000` with a bearer token. Every request is a protocol envelope:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"protocolVersion": 1, "operation": { ... }}'
```

In multitenant mode, also send `X-RushDB-Tenant: <scope>`. The same envelopes
travel unchanged over gRPC. See [HTTP API](../connectivity/http-api.md) and
[Authentication](../connectivity/authentication.md).

## See also

- [Search queries](../query/search-queries.md)
- [Filtering](../query/filtering.md)
- [LMPG layout](../architecture/lmpg-layout.md)
- [Vector internals](../architecture/vector-internals.md)
- [Text internals](../architecture/text-internals.md)
