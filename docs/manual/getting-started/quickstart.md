# Quickstart

This page walks you through a first session with RushDB: start a node, write
records (including a nested JSON document), query them, connect two records
with a relationship, run a vector search, and read the same data from `psql`.
It takes about ten minutes. Every request goes to the native HTTP protocol
with `curl`.

## 1. Install and start a node

Install RushDB (other methods are in [Installation](installation.md)):

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
```

Start a persistent node in a fresh directory. Leave it running in its own
terminal.

```bash
export RUSHDB_TOKEN=local-token          # the default; pick your own for real data
rushdb up --data-dir ./quickstart-data
```

If port 3000 or 5432 is taken, add `--port 3100 --pgwire-port 5433` and use
those ports below. In a second terminal, confirm the node is healthy:

```bash
rushdb health
```

```json
{"status":"ok","engine":"rushdb-lmpg","protocolVersion":1,"queryLanguages":["gremlin","mongo","searchQuery","sql","sqlPgq"]}
```

Every data request is a JSON **envelope** posted to
`/protocol/v1/execute`. To keep the examples short, define a helper in the
second terminal:

```bash
export RUSHDB_TOKEN=local-token
rdb() {
  curl -s http://127.0.0.1:3000/protocol/v1/execute \
    -H "Authorization: Bearer $RUSHDB_TOKEN" \
    -H 'Content-Type: application/json' \
    --data "$1"
}
```

An envelope always has `protocolVersion` (currently `1`) and one
`operation`, whose `kind` is `query`, `mutate`, `describe`, `ingest`, `index`,
`transaction`, `purgeTenant` or `backup`. Successful responses echo
`protocolVersion`, usually add `serverTimingMs`, and carry the result in
`response`; the responses below are abbreviated.

## 2. Create records

Create a `Person` record. Supplying an `id` (any UUID) is optional; it lets
the later steps refer to the record without copying ids around.

```bash
rdb '{
  "protocolVersion": 1,
  "operation": {
    "kind": "mutate",
    "mutation": {
      "kind": "putRecord",
      "mode": "create",
      "id": "019a0000-0000-7000-8000-000000000001",
      "label": "Person",
      "properties": {
        "name": "Ada Lovelace",
        "born": 1815,
        "birthdate": "1815-12-10",
        "languages": ["en", "fr"],
        "nickname": null
      }
    }
  }
}'
```

The response carries the stored record (abbreviated):

```json
{
  "protocolVersion": 1,
  "serverTimingMs": 1,
  "response": {
    "kind": "record",
    "record": {
      "id": "019a0000-0000-7000-8000-000000000001",
      "label": "Person",
      "properties": {
        "name": "Ada Lovelace",
        "born": 1815,
        "birthdate": "1815-12-10",
        "languages": ["en", "fr"],
        "nickname": null
      },
      "propertyTypes": {
        "name": "string",
        "born": "number",
        "birthdate": "datetime",
        "languages": "string"
      }
    }
  }
}
```

Notice what happened without any schema:

- `birthdate` was recognised as a date and stored as a **datetime**.
- `languages` is one property holding two string values.
- `nickname` is stored as an explicit `null` — present, but empty — which is
  different from not having the property at all.

Add two more people:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{"kind":"putRecord","mode":"create",
  "id":"019a0000-0000-7000-8000-000000000002","label":"Person",
  "properties":{"name":"Grace Hopper","born":1906,"languages":["en"]}}}}'

rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{"kind":"putRecord","mode":"create",
  "id":"019a0000-0000-7000-8000-000000000003","label":"Person",
  "properties":{"name":"Alan Turing","born":1912,"languages":["en","de"]}}}}'
```

## 3. Ingest a nested JSON document

`putRecord` writes one flat record. To store a nested document, use
`ingest`: every nested object (and every object inside an array) becomes its
own record, linked to its parent by a relationship.

```bash
rdb '{
  "protocolVersion": 1,
  "operation": {
    "kind": "ingest",
    "ingest": {
      "kind": "documents",
      "label": "Company",
      "documents": {
        "name": "Analytical Engines Ltd",
        "founded": 1834,
        "address": { "city": "London", "street": "Dorset Street" },
        "department": [
          { "name": "Engineering", "headcount": 12 },
          { "name": "Research", "headcount": 4 }
        ]
      },
      "options": {
        "returnResult": true,
        "relationshipTypes": { "address": "HAS_ADDRESS", "department": "HAS_DEPARTMENT" }
      }
    }
  }
}'
```

The response reports four records and three relationships:

```json
{
  "protocolVersion": 1,
  "response": {
    "kind": "ingested",
    "count": 4,
    "relationships": 3,
    "records": [
      { "id": "…", "label": "Company", "properties": { "founded": 1834, "name": "Analytical Engines Ltd" }, "propertyTypes": { "…": "…" } },
      { "id": "…", "label": "address", "properties": { "city": "London", "street": "Dorset Street" }, "propertyTypes": { "…": "…" } },
      { "id": "…", "label": "department", "properties": { "headcount": 12, "name": "Engineering" }, "propertyTypes": { "…": "…" } },
      { "id": "…", "label": "department", "properties": { "headcount": 4, "name": "Research" }, "propertyTypes": { "…": "…" } }
    ]
  }
}
```

Child records take their label from the JSON key (`address`, `department`).
The [Nested import](../data-model/nested-import.md) page explains every option,
including upserts.

## 4. Query records

Find people born in 1900 or later, sorted by name:

```bash
rdb '{
  "protocolVersion": 1,
  "operation": {
    "kind": "query",
    "query": {
      "kind": "records",
      "query": {
        "labels": ["Person"],
        "predicate": { "born": { "$gte": 1900 } },
        "order": { "name": "asc" },
        "limit": 10
      }
    }
  }
}'
```

```json
{
  "protocolVersion": 1,
  "response": {
    "kind": "records",
    "total": 2,
    "records": [
      { "id": "019a0000-0000-7000-8000-000000000003", "label": "Person", "properties": { "name": "Alan Turing", "…": "…" }, "propertyTypes": { "…": "…" } },
      { "id": "019a0000-0000-7000-8000-000000000002", "label": "Person", "properties": { "name": "Grace Hopper", "…": "…" }, "propertyTypes": { "…": "…" } }
    ]
  }
}
```

`total` counts every match before `limit` is applied. A filter can also reach
across relationships: put a related label as a key in the predicate. This
finds companies that have a department with at least ten people:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
  "labels": ["Company"],
  "predicate": { "department": { "headcount": { "$gte": 10 } } }
}}}}'
```

Multi-value properties match when **any** value matches, so
`{"languages": "de"}` finds Alan Turing. See
[Filtering](../query/filtering.md) for every operator.

## 5. Add a relationship

Link Ada to the other two people with an `INFLUENCED` relationship that
carries a property of its own:

```bash
rdb '{
  "protocolVersion": 1,
  "operation": {
    "kind": "mutate",
    "mutation": {
      "kind": "linkRecordsById",
      "command": {
        "relationType": "INFLUENCED",
        "sourceIds": ["019a0000-0000-7000-8000-000000000001"],
        "targetIds": ["019a0000-0000-7000-8000-000000000002",
                      "019a0000-0000-7000-8000-000000000003"],
        "properties": { "via": "the Analytical Engine notes" }
      }
    }
  }
}'
```

```json
{"protocolVersion":1,"response":{"kind":"linked","created":2,"skippedExisting":0}}
```

Linking by id is idempotent: repeating the request creates no duplicate
edges and reports `"created": 0, "skippedExisting": 2`. Now traverse the relationship — find every
person that Ada influenced:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
  "labels": ["Person"],
  "predicate": {
    "Person": {
      "$relation": { "type": "INFLUENCED", "direction": "in" },
      "name": "Ada Lovelace"
    }
  }
}}}}'
```

The direction is seen from the records you are looking for: Grace and Alan
have an **incoming** `INFLUENCED` edge from a `Person` named Ada. List the
edges themselves with a relationship query:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"relationships","query":{
  "relationType": "INFLUENCED"
}}}}'
```

Each row has the relationship's `type`, its `sourceId` and `targetId`, the
endpoint labels, and its `properties`.

## 6. A vector search teaser

RushDB stores and searches vectors you compute; it does not generate
embeddings. Create a three-dimensional vector index, write two notes with
vectors attached, and search:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"index","index":{"kind":"createVector","definition":{
  "name": "note-embedding", "label": "Note", "property": "embedding",
  "dimensions": 3, "similarity": "cosine"
}}}}'

rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{"kind":"putRecord","mode":"create",
  "label":"Note","properties":{"text":"engines and looms"},
  "vectors":[{"propertyName":"embedding","vector":[0.9,0.1,0.0],"index":"note-embedding"}]}}}'

rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{"kind":"putRecord","mode":"create",
  "label":"Note","properties":{"text":"compilers and ships"},
  "vectors":[{"propertyName":"embedding","vector":[0.0,0.2,0.9],"index":"note-embedding"}]}}}'

rdb '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"vector","query":{
  "labels": ["Note"], "property": "embedding", "index": "note-embedding",
  "vector": [1.0, 0.0, 0.0], "limit": 2
}}}}'
```

The response is a `records` list ordered by similarity; each record carries a
`score`, and "engines and looms" comes first. Vector search can also be
filtered with a `predicate` and combined with full-text search; see
[Vector search](../indexes-and-search/vector-search.md) and
[Hybrid search](../indexes-and-search/hybrid-search.md).

## 7. Connect with psql

The PostgreSQL wire listener is bound whenever the `sql` language is
enabled, which it is by default, on port 5432. Connect with `psql`. The password is the token; the user name is ignored, and in
single-tenant mode so is the database name.

```bash
PGPASSWORD="$RUSHDB_TOKEN" psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable"
```

```sql
SELECT name, born FROM Person ORDER BY born;
```

```text
     name     | born
--------------+------
 Ada Lovelace | 1815
 Grace Hopper | 1906
 Alan Turing  | 1912
(3 rows)
```

Labels appear as tables and properties as columns.

> **Note:** The PostgreSQL wire protocol is qualified against the
> `tokio-postgres` Rust driver. Other clients, `psql` included, are outside the
> qualified surface: stick to plain `SELECT` / `INSERT` / `UPDATE` / `DELETE`
> statements, and do not expect catalog-introspection commands (such as `\d`)
> or ORMs to work. See
> [PostgreSQL wire protocol](../connectivity/postgresql-wire.md).

## Clean up

Stop the node and delete `./quickstart-data` when you are done.

## See also

- [Concepts](concepts.md)
- [Data model](../data-model/README.md)
- [Search queries](../query/search-queries.md)
- [HTTP API](../connectivity/http-api.md)
- [SQL](../languages/sql.md)
