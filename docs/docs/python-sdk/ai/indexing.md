---
sidebar_position: 1
title: Embedding Indexes
---

# Embedding Indexes

An **embedding index** is a policy that tells RushDB to vectorize a specific string property for a label. Once `status` is `ready`, every record matching that label+property pair is searchable via `db.ai.search()`.

---

## How indexes work

Indexes are scoped to `(label, propertyName)`. `Book:description` and `Article:description` are completely independent — they maintain separate vector stores and never interfere.

```
Index policy
  label:          "Book"
  propertyName:   "description"
  sourceType:     "managed"
  dimensions:     1536
  status:         "ready"

↓  backfill runs automatically

Book records get vectors stored on their VALUE relationships:
  rel._emb_managed_cosine_1536 = [0.1, 0.2, ...]
```

When new records are created or existing records are updated, the index transitions back to `pending` and vectors are recomputed on the next backfill cycle.

---

## List Indexes

`db.ai.indexes.find()`

List all embedding index policies for the current project.

```python
response = db.ai.indexes.find()
for index in response.data:
    print(f"{index['label']}.{index['propertyName']} — {index['status']}")
```

### Example response data

```python
[
    {
        "id": "idx_abc123",
        "projectId": "proj_xyz",
        "label": "Article",
        "propertyName": "description",
        "sourceType": "managed",
        "similarityFunction": "cosine",
        "modelKey": "text-embedding-3-small",
        "dimensions": 1536,
        "vectorPropertyName": "_emb_managed_cosine_1536",
        "enabled": True,
        "status": "ready",
        "createdAt": "2025-01-10T12:00:00.000Z",
        "updatedAt": "2025-01-10T12:05:00.000Z",
    }
]
```

---

## Create Index

`db.ai.indexes.create()`

Create a new embedding index policy for a string property.

```python
db.ai.indexes.create(params: dict) -> ApiResponse[dict]
```

| `params` key         | Type   | Required | Description                                                                                                 |
| -------------------- | ------ | -------- | ----------------------------------------------------------------------------------------------------------- |
| `label`              | string | **yes**  | Label to scope this index to (e.g. `"Article"`)                                                             |
| `propertyName`       | string | **yes**  | Name of the property to embed (e.g. `"description"`)                                                        |
| `sourceType`         | string | no       | `"managed"` (default) or `"external"`. See [Advanced Indexing](./advanced-indexing.md).                     |
| `similarityFunction` | string | no       | `"cosine"` (default) or `"euclidean"`                                                                       |
| `dimensions`         | number | no       | Vector dimensionality. Defaults to server `RUSHDB_EMBEDDING_DIMENSIONS`. **Required** for external indexes. |

```python
# Simplest form — uses server-configured model and dimensions
response = db.ai.indexes.create({
    "label": "Article",
    "propertyName": "description"
})
print(response.data["status"])  # 'pending' → backfill starts immediately

# With explicit parameters
response = db.ai.indexes.create({
    "label": "Article",
    "propertyName": "description",
    "similarityFunction": "cosine",
    "dimensions": 1536
})
```

> Attempting to create a duplicate `(label, propertyName, sourceType, similarityFunction, dimensions)` tuple returns `409 Conflict`.

### Index lifecycle

| Status             | Description                                            |
| ------------------ | ------------------------------------------------------ |
| `pending`          | Policy created, waiting for backfill scheduler         |
| `indexing`         | Backfill in progress                                   |
| `awaiting_vectors` | External index — waiting for client to push vectors    |
| `ready`            | All existing records have vectors; search is available |
| `error`            | Backfill failed; check server logs for the cause       |

---

## Get Index Stats

`db.ai.indexes.stats(index_id)`

Returns the fill rate for an index — useful for progress monitoring or health checks.

```python
response = db.ai.indexes.stats(index_id)
stats = response.data
print(f"{stats['indexedRecords']} / {stats['totalRecords']} records indexed")
```

---

## Delete Index

`db.ai.indexes.delete(index_id)`

Remove an embedding index policy. The underlying Neo4j DDL vector index is only dropped when **zero embeddings remain** across the entire project.

```python
db.ai.indexes.delete(index_id)
```

---

## Waiting for an index to become ready

For managed indexes, backfill runs asynchronously. Poll `db.ai.indexes.find()` until `status` is `ready`:

```python
import time

def wait_for_index_ready(db, index_id, timeout_s=90):
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        response = db.ai.indexes.find()
        idx = next((i for i in response.data if i["id"] == index_id), None)
        if idx and idx["status"] == "ready":
            return
        if idx and idx["status"] == "error":
            raise RuntimeError("Index entered error state")
        time.sleep(3)
    raise TimeoutError("Index did not become ready in time")

response = db.ai.indexes.create({"label": "Book", "propertyName": "description"})
wait_for_index_ready(db, response.data["id"])
# now safe to call db.ai.search(...)
```

---

## Multiple indexes on the same property

You can have more than one index per `(label, propertyName)` pair, provided the signature differs:

```python
# Cosine index
db.ai.indexes.create({
    "label": "Product",
    "propertyName": "description",
    "similarityFunction": "cosine",
    "dimensions": 768,
})

# Euclidean index on the same property
db.ai.indexes.create({
    "label": "Product",
    "propertyName": "description",
    "similarityFunction": "euclidean",
    "dimensions": 768,
})
```

When searching or writing vectors against a property with multiple indexes, specify `similarityFunction` to disambiguate. See [Advanced Indexing](./advanced-indexing.md#disambiguation) for details.

---

## Index shape

```python
{
    "id": str,
    "projectId": str,
    "label": str,
    "propertyName": str,
    "modelKey": str,
    "sourceType": str,            # 'managed' | 'external'
    "similarityFunction": str,    # 'cosine' | 'euclidean'
    "dimensions": int,
    "vectorPropertyName": str,    # internal Neo4j property name for the vector
    "enabled": bool,
    "status": str,                # 'pending' | 'indexing' | 'awaiting_vectors' | 'ready' | 'error'
    "createdAt": str,
    "updatedAt": str,
}
```

---

## String List Properties

`List[str]`

String list properties are supported. Each item in the list is embedded individually, then mean-pooled into a single vector stored on the relationship.
