# Structural embeddings

A structural index gives every record of one label a vector computed from
the graph around it, so records in similar positions (the same community,
the same kind of neighbourhood) are near each other in a vector search. You
do not supply the vectors: the engine derives them from the records'
relationships and keeps them current as relationships change.

The vectors are **model output, not facts**: use them to rank and explore
("records shaped like this one"), never as exact answers. Every structural
index names its model (`fastrp-v1`).

## The model

`fastrp-v1` is FastRP (Fast Random Projection, Chen et al. 2019; the
algorithm Neo4j Graph Data Science ships):

1. Every record gets a very sparse random vector (sparsity 3), seeded by the
   record, so it is the same on every run.
2. One hop: each record averages its neighbours' random vectors.
3. Two hops: each record averages its neighbours' one-hop vectors.
4. The embedding is `0.1 · own + one hop + two hops`, normalized to unit
   length.

Relationships of every type count, in both directions. Neighbours of any
label take part; only records of the index's label get a vector. A record
without relationships keeps its own random direction.

The same graph at the same commit always produces the same vectors.

## Create a structural index

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "index",
      "index": {
        "kind": "create",
        "definition": { "name": "customer_shape", "label": "Customer", "property": "*", "kind": "structural", "dimensions": 128 }
      }
    }
  }'
```

| Field | Value |
|---|---|
| `property` | Always `*`: a structural index reads topology, not a property. |
| `dimensions` | 8 to 4,096. More dimensions separate finer structure and cost more memory and time. |

The name must not be taken by a vector index: the structural index registers
a vector index of the same name (cosine similarity), which is how you search
it. Like other declared indexes, it is built in the background: every member
is queued and the index applier embeds them, a batch of 1,024 at a time.

## Search

Search with `query.vector`, naming the index. The query vector is any vector
of the index's dimensions, typically a record's own embedding:

```json
{
  "kind": "query",
  "query": {
    "kind": "vector",
    "query": {
      "labels": ["Customer"], "property": "*", "index": "customer_shape",
      "vector": [0.012, -0.08, "…"], "similarity": "cosine",
      "predicate": {}, "limit": 10, "ef": 64
    }
  }
}
```

Everything in [Vector search](vector-search.md) applies: exact search by
default, ANN with `ef`, predicates evaluated before ranking.

## Freshness

Creating or deleting a relationship changes the embeddings of its two
records and of every record adjacent to them (an embedding reads two hops).
The commit that makes the change marks those records `pending`; the index
applier re-embeds them shortly after (it runs continuously with a linger of
at most 250 ms). `describe.indexes` reports the backlog:

```json
{ "name": "customer_shape", "label": "Customer", "property": "*", "kind": "structural",
  "dimensions": 128, "model": "fastrp-v1", "state": "ready", "pending": 0 }
```

A search between the change and the applier's pass sees the previous
vectors. A record next to a hub (more than 10,000 relationships) does not
mark its neighbours one by one: the index restarts its build instead, and
`state` shows `building` until every member is embedded again.

A record that leaves the label (deleted) has its vector retired. A record
retyped to another label keeps its vector but is no longer returned, since a
vector search only returns records of the requested labels.

## Memory

Each applier batch holds one vector per record it reads (the batch's records
and their neighbours), reserved from the vector cache budget
(`logical-cache-bytes`) before it is computed. A batch that does not fit is
refused: nothing is bound, the records stay pending, and the applier retries
on its next pass.

## Drop

`index.drop` removes the structural index, its vectors and its vector index.

## See also

- [Vector search](vector-search.md)
- [Indexes](indexes.md)
