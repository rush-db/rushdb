# Mongo-style documents

RushDB accepts MongoDB-style query and command documents as a statement
language. A collection is a label, a document is a record, and filters use the
same operators as the native query document. This page describes the document
shapes RushDB understands and where they differ from MongoDB. It is for
developers who think in MongoDB queries; it is not a MongoDB server.

| Property | Value |
|---|---|
| Status | Stable |
| Enabled | By default (`languages.mongodb: true`); a `--languages` list must name `mongodb` to keep it |
| Wire id | `mongo` |
| Profile | MongoDB query language (find/aggregate + CRUD + index) |
| Implementation | Translated onto the native query and write operations |
| Access | read and write |
| Transports | Protocol v1 (HTTP, gRPC) |

> **Warning:** RushDB does not speak the MongoDB wire protocol. MongoDB drivers
> and `mongosh` cannot connect. Send Mongo-style documents as the `statement`
> string of a Protocol v1 statement operation.

## Data model mapping

| MongoDB | RushDB |
|---|---|
| Collection | Label |
| Document | Record |
| Field | Property |
| `_id` | The record id (assigned by RushDB; `_id`, `id`, `__id`, and `__label` in inserted documents are ignored). A top-level `_id` condition in a filter matches the record id. Because `_id` is the document id here, a record property named `_id` is not visible through this language. |
| Database | The request's tenant |

Relationships are not visible through this language.

## Sending a statement

The statement is a JSON document serialized into the `statement` string. Reads
go through `query.statement`, writes and index commands through
`mutate.statement`:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": {
        "kind": "statement",
        "language": "mongo",
        "statement": "{\"collection\":\"FACTION\",\"filter\":{\"name\":{\"$ne\":\"Sith Order\"}},\"sort\":{\"name\":1}}"
      }
    }
  }'
```

## Find

```json
{
  "collection": "FACTION",
  "filter": { "name": { "$ne": "Sith Order" } },
  "projection": { "name": 1 },
  "sort": { "name": 1 },
  "skip": 0,
  "limit": 20
}
```

| Field | Meaning |
|---|---|
| `collection` | Label to read. Required (or `labels`, an array of labels). |
| `filter` | Filter document. Uses the native filter operators (`$eq`, `$ne`, `$gt`, `$in`, ...); see [Filtering](../query/filtering.md). |
| `projection` | Inclusion projection: each field `1` or `true`. Exclusion (`0`/`false`) is rejected. |
| `sort` | Field to direction: `1` ascending, `-1` descending. Other values are rejected. |
| `skip`, `limit` | Pagination. |

The response is a `rows` result. `total` is the number of matching documents
before `skip`/`limit` are applied, so you can page without a separate count.

To fetch one document, use `"limit": 1`; there is no separate `findOne`
command.

`$text` searches the collection's text index; `{"$meta": "textScore"}`
projects or sorts by its BM25 score:

```json
{
  "collection": "Doc",
  "filter": { "$text": { "$search": "quick fox" }, "year": { "$gt": 2020 } },
  "projection": { "score": { "$meta": "textScore" } },
  "sort": { "score": { "$meta": "textScore" } }
}
```

See [Full-text search](../indexes-and-search/full-text-search.md#from-statement-languages).

## Aggregate

```json
{
  "collection": "FACTION",
  "aggregate": [
    { "$match": {} },
    { "$group": { "_id": "$name", "count": { "$sum": 1 } } },
    { "$sort": { "count": -1 } },
    { "$limit": 2 }
  ]
}
```

| Stage | Support |
|---|---|
| `$match` | Filter, as in `find`. |
| `$group` | `_id` names the group key field (`"$field"`). Accumulators: `$sum` (`1` counts), `$avg`, `$min`, `$max`, `$count`, `$first`. |
| `$sort` | `1` or `-1` per field. |
| `$skip`, `$limit` | Pagination of the output. |
| `$count` | `{ "$count": "n" }` returns the match count as `n`. |
| `$project`, `$lookup`, `$geoNear`, `$unwind`, others | Rejected with `unsupported`. The output columns come from `$group`/`$count`. |

> **Note:** The pipeline is compiled into one native aggregate query, not run
> stage by stage, so it follows the `$match` → `$group` → `$sort` →
> `$skip`/`$limit` shape. Several `$match` stages are combined with AND; a
> `$match` after `$group`, `$skip` or `$limit` is rejected with `unsupported`. For
> an aggregate, `total` is the number of output rows.

## Writes

| Command | Shape |
|---|---|
| `insertOne` | `{"insertOne": {"collection": "C", "document": {...}}}` |
| `insertMany` | `{"insertMany": {"collection": "C", "documents": [{...}, ...]}}` |
| `updateOne`, `updateMany` | `{"updateOne": {"collection": "C", "filter": {...}, "update": {...}, "upsert": false}}` |
| `deleteOne`, `deleteMany` | `{"deleteOne": {"collection": "C", "filter": {...}}}` |

Update operators:

| Operator | Behavior |
|---|---|
| `$set` | Set fields. |
| `$inc` | Add to a numeric field; a missing field counts as `0`. Overflow, non-numeric targets, and mixes that would lose integer precision are rejected. |
| `$unset` | Remove fields. |

Other update operators, and replacement documents, are rejected. With
`"upsert": true` and no match, a new document is created from the filter's
equality fields plus the `$set` values.

Example sequence:

```json
{"insertOne": {"collection": "FACTION", "document": {"name": "Rebel Alliance"}}}
```

```json
{"updateOne": {"collection": "FACTION", "filter": {"name": "Rebel Alliance"},
               "update": {"$set": {"name": "Alliance"}}}}
```

```json
{"deleteOne": {"collection": "FACTION", "filter": {"name": "Alliance"}}}
```

Each returns a `statementMutation` response whose `affected` is the number of
documents inserted, updated, or deleted.

Multi-document commands are atomic: `insertMany`, `updateMany` and
`deleteMany` write every document in one commit, so a command that fails,
is cancelled, or exceeds `max-expanded-commit-bytes` changes nothing.
`updateMany` computes every update (including `$inc`) from one snapshot and
is refused with a conflict error when another write changed the tenant
in the meantime; retry it.

Mongo-style commands cannot be staged inside a multi-request transaction.

## Index commands

Send index commands with `mutate.statement`:

| Command | Effect |
|---|---|
| `{"createIndex": {"collection": "C", "property": "p"}}` | Ordered index named `C_p` (or `name`). |
| `{"createIndex": {"collection": "C", "property": "p", "kind": "text", "consistency": "immediate"}}` | Full-text index. `consistency` defaults to eventual. |
| `{"createIndex": {"collection": "C", "property": "p", "dimensions": 768, "similarity": "cosine", "name": "c-p"}}` | Vector index (any document with `dimensions`). |
| `{"dropIndex": {"name": "..."}}` | Drops a declared index of that name, otherwise a vector index. |
| `{"reIndex": {"name": "..."}}` | Revalidates a vector index against current data. |

See [Indexes](../indexes-and-search/indexes.md) and
[Vector search](../indexes-and-search/vector-search.md).

## Limitations

The capability profile lists `noLookup`, `noGeoNear`, `noUnwind`, and
`noProject`. In practice:

- `$lookup`, `$geoNear`, `$unwind`, and `$project` stages fail with `unsupported`. Use a
  graph language or SQL joins for cross-label queries.
- `$text` needs exactly one ready text index on the collection, belongs in
  the filter of a `find` or the first `$match`, and cannot sort a pipeline.
- Exclusion projections, `findOne`, `replaceOne`, and update operators other
  than `$set`/`$inc`/`$unset` are rejected.
- Update and delete commands consider at most 1,000,000 matching documents.
- Results are materialized; page with `skip`/`limit`.

## Errors

| Situation | Error code |
|---|---|
| Mongo-style statements not enabled | `language_disabled` |
| Unsupported stage or command | `unsupported` |
| Missing `collection`, `property`, `dimensions`, or `name` in an index command; unknown index `kind` | `invalid_request` |

## See also

- [Filtering](../query/filtering.md)
- [Aggregations](../query/aggregations.md)
- [Writing data](../query/writing-data.md)
- [Query languages overview](README.md)
