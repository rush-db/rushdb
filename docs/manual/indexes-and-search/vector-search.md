# Vector search

This page shows how to store embeddings in RushDB and search them by
similarity: creating a vector index, attaching vectors to records, running
exact and approximate (ANN) queries with filters, and keeping vectors in step
with changing data. It is for developers building semantic search or
retrieval-augmented generation (RAG) on top of RushDB.

RushDB stores and searches vectors; it **never calls an embedding model**. You
compute embeddings with your own provider and send them to the database.

## Concepts

| Term | Meaning |
|---|---|
| **Vector index** | A named vector space for one label (or relationship type) and one property, with a fixed number of `dimensions` and a `similarity` metric. |
| **Space** | The immutable contract behind an index name: dimensions and metric never change. To change them, drop the index and create a new one. |
| **Binding** | The association between one record (or relationship) and one stored vector in a space. A record has at most one vector per index. |
| **Generation** | An opaque token issued when the index is declared (an identical re-declaration keeps it), used to reject embedding jobs that started against an older declaration. |

Vectors are stored as 32-bit floats. Each tenant can hold at most **256**
vector spaces.

## Create a vector index

`index.createVector` takes a `VectorIndexDefinition`:

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | string | yes¹ | — | Index name, 1–64 bytes of `[A-Za-z0-9._:-]`, not starting with `emb__`. |
| `label` | string | yes | — | Label of the records to index (the relationship type when `owner` is `relationship`). |
| `property` | string | yes | — | Property the vectors belong to. |
| `dimensions` | integer | yes | — | Vector length; must be positive. Every stored and query vector must have exactly this length. |
| `similarity` | string | no | `cosine` | `cosine`, `dot`, or `euclidean` (alias `l2`). |
| `model` | string | no | — | Your embedding model identifier. Not interpreted, but re-declaring the same `name` with a different `model` is rejected, so vectors from incomparable models cannot be mixed. |
| `owner` | string | no | `record` | `record` (alias `node`) or `relationship` (alias `edge`). |

¹ The server option `resources.require-vector-index-name`
(`RUSHDB_REQUIRE_VECTOR_INDEX_NAME`) is **on by default** and makes `name`
mandatory on create, search and inline vectors. When it is turned off, a
missing name falls back to a derived alias
`emb__<label>__<property>__<similarity>__<dimensions>`, which cannot tell two
same-shape models apart.

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "index",
      "index": {
        "kind": "createVector",
        "definition": {
          "name": "doc-embedding",
          "label": "Doc",
          "property": "embedding",
          "dimensions": 3,
          "similarity": "cosine",
          "model": "my-embedder-v1"
        }
      }
    }
  }'
```

```json
{
  "protocolVersion": 1,
  "response": {
    "kind": "vectorIndex",
    "index": {
      "id": "doc-embedding",
      "name": "doc-embedding",
      "label": "Doc",
      "property": "embedding",
      "similarity": "cosine",
      "dimensions": 3,
      "status": "awaiting_vectors",
      "model": "my-embedder-v1",
      "nameSource": "explicit",
      "owner": "record",
      "generation": "0192f0c4-…"
    }
  }
}
```

Re-declaring an existing name with an identical definition is a no-op: it
returns the index unchanged, with the same `generation`, so embedding jobs in
flight stay valid. Re-declaring it with different `dimensions` or
`similarity` fails (spaces are immutable); with a different `model`, `label`,
`property` or `owner` it fails with `conflict` (`already exists ... different
...`). Drop the index first to change its definition.

### From statement languages

| Language | Create | Drop |
|---|---|---|
| SQL | `CREATE VECTOR INDEX name ON Label(prop) [SIMILARITY cosine\|dot\|euclidean] DIMENSIONS n` | `DROP INDEX name` |
| Mongo | `{"createIndex": {"collection": "Label", "property": "prop", "dimensions": n, "similarity": "cosine", "name": "…"}}` | `{"dropIndex": {"name": "…"}}` |
| Cypher / GQL | — | `DROP INDEX name` |

Mongo `{"reIndex": {"name": "…"}}` runs [`revalidateVector`](#keep-vectors-in-step-with-data).
`DROP INDEX` drops a declared (ordered or text) index of that name first, and
a vector index only if no declared index matches.

## Attach vectors to records

You can bind vectors in three ways. All of them validate the whole batch
(index exists, every vector has the index's dimensions, every component is
finite) before binding anything.

### 1. `upsertVectors` for existing records

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "index",
      "index": {
        "kind": "upsertVectors",
        "name": "doc-embedding",
        "items": [
          { "recordId": "0192f0c4-7a1e-7b8a-9c3d-2f6e5a4b3c21", "vector": [0.12, 0.98, 0.05] },
          { "recordId": "0192f0c47a1e7b8a9c3d2f6e5a4b3c22", "vector": [0.91, 0.10, 0.40] }
        ]
      }
    }
  }'
```

The response is `{"kind": "vectorsUpserted", "updated": 2, "requested": 2}`.
Record ids may be dashed UUIDs or 32-character hex. Upserting again replaces
the record's vector; upserting an identical vector is a no-op.

| Item field | Required | Description |
|---|---|---|
| `recordId` | yes | Record (or relationship, for a relationship index) id. |
| `vector` | yes | Components; length must equal `dimensions`. |
| `indexGeneration` | no | The `generation` you read when your embedding job started. If the index was dropped and recreated (or re-declared) since, the whole batch is rejected with `stale vector index generation`. |
| `sourceHash` | no | Engine value hash of the source property captured before embedding. When present, the bind succeeds only if the property still has that value; the binding is then *source-tracked* (see below). |

Items without `sourceHash` on a record index are bound in a single commit.

### 2. Inline with the record (`putRecord` and ingest)

`putRecord` accepts a `vectors` array, and `ingest.documents` accepts a
`$vectors` array on any document. Each entry names the property, the vector
and the index:

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "ingest",
    "ingest": {
      "kind": "documents",
      "label": "Doc",
      "documents": [
        {
          "title": "Graph basics",
          "$vectors": [
            { "propertyName": "embedding", "vector": [0.12, 0.98, 0.05], "index": "doc-embedding" }
          ]
        }
      ]
    }
  }
}
```

An entry naming an index that does not exist fails the request before any
record is created (`no embedding index named …`). `similarityFunction` is an
optional entry field used only to disambiguate when names are not required.

### 3. Backfill unbound records

`query.vectorIndexCandidates` pages through records of a label that carry the
property but have no binding in the index yet. Use it to drive an embedding
backfill: fetch a page, embed each `value`, then `upsertVectors`.

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "query",
    "query": {
      "kind": "vectorIndexCandidates",
      "query": { "label": "Doc", "property": "body", "index": "doc-embedding", "skip": 0, "limit": 500 }
    }
  }
}
```

The response is `{"kind": "vectorIndexCandidates", "records": [{"id": "…", "value": "…", "sourceHash": …, "indexGeneration": "…"}], "total": n}`,
where `total` counts every unbound candidate. `limit` defaults to 500. Send
each candidate's `sourceHash` and `indexGeneration` back with its vector in
`upsertVectors`: the bind is then rejected if the value or the index changed
while the embedding was computed.

## Search

`query.vector` ranks the records of a label by similarity to a query vector.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `labels` | string[] | yes | — | Label to search (the first entry selects the derived alias when `index` is omitted). `[]` searches the index's own label. |
| `property` | string | yes | — | Vector property. |
| `vector` | number[] | yes | — | Query vector, non-empty, finite, with the index's dimensions. |
| `index` | string | yes¹ | — | Index name. When set, the space's own metric is used. |
| `similarity` | string | no | `cosine` | Only used to derive the alias when `index` is omitted. |
| `predicate` | object | no | `{}` | Filter applied **before** ranking (same syntax as `query.records`). |
| `offset` | integer | no | 0 | Ranked results to skip. |
| `limit` | integer | yes | — | Results to return. |
| `mode` | string | no | `auto` | `auto`, `exact` or `ann`; see [Exact versus approximate](#exact-versus-approximate-ann). `ef > 0` with no `mode` means `ann`. |
| `ef` | integer | no | `resources.vector-default-ef` (64) | ANN gather breadth whenever the search runs on the ANN graph; at least `offset + limit` is gathered. |
| `scoreThreshold` | number | no | — | Keep only hits scoring at least this much. A page may then hold fewer than `limit` records. |

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": {
        "kind": "vector",
        "query": {
          "labels": ["Doc"],
          "property": "embedding",
          "index": "doc-embedding",
          "vector": [0.10, 0.95, 0.00],
          "predicate": { "lang": "en" },
          "limit": 5
        }
      }
    }
  }'
```

Results come back as a `records` response, best first. Each record carries a
`score`; `total` is the number of records on this page. `vectorSearch` reports
how the search ran: `mode` (`exact` or `ann`), `ef` for an ANN search, `pool`
(the index's vectors) and, when a search that would use the ANN graph was
answered exactly, `fallback` (see below).

```json
{
  "kind": "records",
  "total": 1,
  "records": [
    {
      "id": "0192f0c4-7a1e-7b8a-9c3d-2f6e5a4b3c21",
      "label": "Doc",
      "properties": { "title": "Graph basics", "lang": "en" },
      "propertyTypes": { "title": "string", "lang": "string" },
      "score": 0.998
    }
  ],
  "vectorSearch": { "mode": "exact", "pool": 3 }
}
```

### Similarity metrics

All scores are "higher is more similar".

| `similarity` | Score | ANN with `ef` |
|---|---|---|
| `cosine` | dot(a, b) / (‖a‖·‖b‖) | yes |
| `dot` | dot(a, b) | no — always answered exactly |
| `euclidean` / `l2` | 1 / (1 + ‖a − b‖) | no — always answered exactly |

Exact search orders equal scores deterministically by record.

### Exact versus approximate (ANN)

- **Exact:** every bound vector that passes the filter is scored;
  the top `offset + limit` are kept in a bounded heap. Results are exact and
  memory use is proportional to `offset + limit`, not to the index size. Cost
  grows with the index: ~1.7 µs per 768-d vector (5,000 vectors ≈ 9 ms).
- **ANN** (cosine only): an in-memory HNSW graph gathers at least
  `max(ef, offset + limit)` candidates, every candidate is **re-scored exactly**
  against the stored vector, and the best are returned. Larger `ef` trades
  latency for recall. A resident graph answers in about a millisecond, but
  building it takes seconds (5,000 768-d vectors ≈ 2 s).

`mode` chooses between them:

| `mode` | Behaviour |
|---|---|
| `auto` (default) | Exact while the index holds at most `resources.vector-exact-max-pool` vectors (default 5,000) or its metric is not cosine. Above that, ANN once the graph is resident; until then the search is answered exactly while the graph is built in the background (`fallback: annWarming`), so no search waits for a build. |
| `exact` | Exact search. |
| `ann` | ANN, building the graph first if it is not resident. Refused (`invalid_request`) on a `dot` or `euclidean` index. |

ANN degrades to exact search instead of returning a short or wrong answer, and
reports why in `vectorSearch.fallback`:

| Situation | Behaviour | `fallback` |
|---|---|---|
| The gather produced fewer than `min(ef, offset + limit)` candidates that pass the filter | Exact search for this query | `lowConfidence` |
| The ANN graph does not fit the vector cache budget | Exact search; counted in `rushdb_ann_exact_fallbacks_total`. `auto` does not retry the build until the index changes. | `budget` |
| The logical cache budget is disabled (`RUSHDB_LOGICAL_CACHE_BYTES=0`) | No ANN can be resident: exact search, counted in `rushdb_ann_exact_fallbacks_total`; `revalidateVector` reports `annRebuildError` | `budget` |

The ANN graph is a rebuildable cache over the stored vectors: it is checked
against the query's snapshot, reopened from its persisted segments after a
restart or an idle eviction (decoding only what changed since), and kept
current as vectors are bound. Internally it may hold vectors at reduced (16-bit)
precision; results are always reranked with the stored 32-bit vectors. See
[Vector internals](../architecture/vector-internals.md).

### Filters

The `predicate` is resolved by the [query planner](query-planning.md) into a
candidate set **before** ranking, and both exact and ANN search only consider
candidates. A filtered search therefore never returns a record outside the
filter, and never returns fewer results because high-scoring records were
filtered out after ranking. With ANN, the gather is enlarged in proportion to
the filter's selectivity; a very selective filter usually falls back to exact
search, which is cheap because the candidate set is small.

> **Tip:** Declared [ordered indexes](indexes.md) speed up the filter stage of
> vector queries exactly as they do for `query.records`.

Records deleted after their vector was bound are never returned.

### Relationship vectors

An index created with `"owner": "relationship"` binds vectors to
relationships. `label` is the relationship type, `upsertVectors` items use the
relationship id as `recordId`, and `query.vector` takes the relationship type
in `labels`. The `predicate` filters on relationship properties. Results are
relationship objects (`__id`, `__type`, `__source_id`, `__target_id` and the
properties) with a `score`.

### Inspect a vector query

`describe.explainVector` takes the same document as `query.vector` and reports
the candidate count without ranking:

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "describe",
    "describe": {
      "kind": "explainVector",
      "query": {
        "labels": ["Doc"], "property": "embedding", "index": "doc-embedding",
        "vector": [0.1, 0.95, 0.0], "predicate": { "lang": "en" }, "limit": 5
      }
    }
  }
}
```

The `explainVector` plan contains `accessPath` (`vector_index`), `alias`,
`label`, `candidateCount` (records matching the predicate), `k`
(`offset + limit`) and `predicate`.

## Keep vectors in step with data

### Index status

`describe.vectorIndexes` lists every vector index with live coverage counts;
`describe.vectorIndex` (`{"kind": "vectorIndex", "name": "…"}`) returns one
index's statistics as a `vectorIndexStats` response.

| Field | Meaning |
|---|---|
| `status` | `awaiting_vectors` (no records carry the property yet), `ready` (every record carrying the property has a current binding), or `behind` (some are unbound or stale). |
| `totalRecords` | Records of the label carrying the property (or the number of bound vectors, if larger). |
| `indexedRecords` | Bindings eligible for search. |
| `unindexedRecords` | `totalRecords − indexedRecords`. |
| `staleBindings` | Bindings whose source value changed since they were bound. |
| `retiredBindings` | Bindings whose owner was deleted. |
| `sourceTrackedBindings` / `externalBindings` | Bindings with and without a source hash. |
| `annLifecycle` | `manifestState` and `providerState` (`memoryCached` or `rebuildRequired`; `rebuildRequired` after every restart is expected). |

Status is computed from the bindings and the source records at the read
snapshot; it is never a stored flag.

### Staleness

A binding is only searchable while its status is *ready*. Two kinds of
change make a binding ineligible:

- **Owner deleted** — the record is never returned by search, and
  `revalidateVector` marks the binding *retired*.
- **Source value changed** (source-tracked bindings only, i.e. those bound
  with `sourceHash`) — `revalidateVector` marks the binding *stale*; it stops
  being searched and is counted in `staleBindings` until you upsert a fresh
  vector.

Vectors bound without `sourceHash` (plain `upsertVectors`, inline vectors)
are treated as data you own: changing other properties of the record does not
make them stale. Re-upsert them when your source text changes.

### `revalidateVector`

```json
{ "protocolVersion": 1, "operation": { "kind": "index", "index": { "kind": "revalidateVector", "name": "doc-embedding" } } }
```

It reconciles every binding's status against live data in one commit, then
rebuilds the ANN graph. The `vectorIndexStats` response reports `stale`,
`retired`, `annEntries`, `annLifecycle` and `annRebuildError` (an ANN rebuild
failure does not undo the status reconciliation).

### Drop

`{"kind": "dropVector", "name": "doc-embedding"}` removes the index's bindings,
its space and its metadata. The name can then be reused with a different
shape; the new declaration gets a new `generation`.

## Consistency

Vector bindings are committed through the same commit path as records. A search
reads one snapshot: it sees every binding committed before it started, and the
records it returns are reconstructed from that same snapshot. The ANN graph is
validated against the snapshot's version of the space before use.

## Sizing notes

- Each stored vector costs `4 × dimensions` bytes plus a binding row.
- Exact search reads every bound vector that passes the filter; its cost grows
  with the candidate count. `auto` switches large indexes to ANN; set
  `resources.vector-exact-max-pool` to move that point.
- The ANN graph lives in memory and is charged to the logical cache budget
  (`RUSHDB_LOGICAL_CACHE_BYTES`, default 128 MiB). When it does not fit,
  queries fall back to exact search rather than fail. Watch
  `rushdb_ann_exact_fallbacks_total`.
- When the server setting `require-index-warmup` is on, `/ready` reports not-ready while
  any vector index is `behind` (single-tenant mode).

See [Capacity planning](../operations/capacity-planning.md) for measured
figures.

## See also

- [Hybrid search](hybrid-search.md)
- [Query planning](query-planning.md)
- [Vector internals](../architecture/vector-internals.md)
- [Writing data](../query/writing-data.md)
- [Nested import](../data-model/nested-import.md)
- [Metrics](../reference/metrics.md)
