---
sidebar_position: 2
title: Advanced Indexing — BYOV
---

# Advanced Indexing — Bring Your Own Vectors

**External indexes** (BYOV — Bring Your Own Vectors) let you supply pre-computed embedding vectors instead of having the server compute them. Use them when you need:

- A custom or private model the server cannot access
- Multimodal embeddings (image, audio, document structure)
- Vectors already produced by your ML pipeline
- Reproducible embeddings not tied to the server's active model

---

## Creating an external index

Pass `"sourceType": "external"` in the params dict. `dimensions` is **required** because the server never calls an embedding model and cannot infer the vector size:

```python
# Explicit sourceType
response = db.ai.indexes.create({
    "label": "Article",
    "propertyName": "body",
    "sourceType": "external",
    "dimensions": 768,
    "similarityFunction": "cosine",
})
print(response.data["status"])  # 'awaiting_vectors'
```

An external index starts with status `awaiting_vectors` and transitions to `ready` once at least one vector has been written.

### External vs managed comparison

|                               | Managed                          | External                                     |
| ----------------------------- | -------------------------------- | -------------------------------------------- |
| `sourceType`                  | `"managed"`                      | `"external"`                                 |
| Initial status                | `"pending"`                      | `"awaiting_vectors"`                         |
| Who computes embeddings       | RushDB server (configured model) | Your application                             |
| `dimensions` required         | No (uses server default)         | **Yes**                                      |
| Backfill for existing records | Automatic                        | Manual via `upsert_vectors` or inline writes |

---

## Upsert Vectors

`db.ai.indexes.upsert_vectors()`

The bulk upload API — ideal for seeding an index from a dataset or syncing after a batch pipeline.

```python
db.ai.indexes.upsert_vectors(
    index_id: str,
    params: dict    # {"items": [{"recordId": str, "vector": list[float]}]}
) -> ApiResponse
```

```python
# Fetch your records and embed them with your own model
records_response = db.records.find({"where": {"__label": "Article"}})

items = []
for record in records_response.data:
    vector = my_embedder.embed(record["body"])  # your embedding model
    items.append({"recordId": record["__id"], "vector": vector})

db.ai.indexes.upsert_vectors(ext_index_id, {"items": items})
```

The request is **idempotent** — calling it again with the same `recordId` replaces the stored vector.

---

## Writing vectors at record creation time

Instead of a two-step create → upsert_vectors flow, you can write vectors inline using the `vectors` parameter on any write operation. See [Write Records with Vectors](./write-with-vectors.md) for the full reference.

```python
# One step: create record AND write its vector
record = db.records.create(
    label="Article",
    data={"title": "Warp drives", "body": "Alcubierre metric..."},
    vectors=[{"propertyName": "body", "vector": my_embedder.embed("Alcubierre metric...")}],
)
```

---

## Disambiguation {#disambiguation}

When the same `(label, propertyName)` pair is covered by more than one external index (different `similarityFunction` or `dimensions`), specify `similarityFunction` to resolve which index to use:

```python
# Two indexes: Article:body/cosine and Article:body/euclidean

# ✅ Explicit — writes to the cosine index only
db.records.create(
    label="Article",
    data={"title": "Widget", "body": "..."},
    vectors=[{
        "propertyName": "body",
        "vector": vec,
        "similarityFunction": "cosine",   # required when ambiguous
    }],
)

# ✅ Explicit — searches the euclidean index only
db.ai.search({
    "labels": ["Article"],
    "propertyName": "body",
    "queryVector": vec,
    "similarityFunction": "euclidean",
})

# ❌ Missing similarityFunction when two indexes exist → 422 Unprocessable Entity
db.records.create(
    label="Article",
    data={"title": "Gadget"},
    vectors=[{"propertyName": "body", "vector": vec}],  # ambiguous!
)
```

### Index signature uniqueness

Two index policies are considered **identical** (and a second `create` returns `409 Conflict`) when all five fields match:

| Field                | Effect on uniqueness |
| -------------------- | -------------------- |
| `label`              | ✅                   |
| `propertyName`       | ✅                   |
| `sourceType`         | ✅                   |
| `similarityFunction` | ✅                   |
| `dimensions`         | ✅                   |

Changing any one field produces a distinct index and both are allowed to coexist.

---

## Complete BYOV worked example

```python
from rushdb import RushDB

db = RushDB("your-api-key")

# 1. Create the external index
idx_response = db.ai.indexes.create({
    "label": "Doc",
    "propertyName": "content",
    "sourceType": "external",
    "dimensions": 3,
    "similarityFunction": "cosine",
})
ext_index_id = idx_response.data["id"]
# status: 'awaiting_vectors'

# 2. Create records + write inline vectors (one round trip per record)
articles = [
    {"title": "Alpha", "content": "First article",  "vector": [1, 0, 0]},
    {"title": "Beta",  "content": "Second article", "vector": [0, 1, 0]},
    {"title": "Gamma", "content": "Third article",  "vector": [0, 0, 1]},
]

for article in articles:
    db.records.create(
        label="Doc",
        data={"title": article["title"], "content": article["content"]},
        vectors=[{"propertyName": "content", "vector": article["vector"]}],
    )

# 3. Search using a pre-computed query vector
results = db.ai.search({
    "labels": ["Doc"],
    "propertyName": "content",
    "queryVector": [1, 0, 0],   # closest to Alpha
    "limit": 3,
})

print(results.data[0]["title"])    # 'Alpha'
print(results.data[0]["__score"])  # ~1.0
```

---

## Batch import with `$vectors`

For bulk seeding, use `db.records.import_json()` with a `$vectors` key on each object:

```python
db.records.import_json({
    "Doc": [
        {"title": "Alpha", "content": "First article",  "$vectors": [{"propertyName": "content", "vector": [1, 0, 0]}]},
        {"title": "Beta",  "content": "Second article", "$vectors": [{"propertyName": "content", "vector": [0, 1, 0]}]},
        {"title": "Gamma", "content": "Third article",  "$vectors": [{"propertyName": "content", "vector": [0, 0, 1]}]},
    ]
})
```

`$vectors` entries are **stripped** from the stored record data — they do not appear as properties or child records.

---

## Mixing managed and external indexes

You can have both a managed index and an external index on the same property simultaneously:

```python
# Managed — server embeds for full-text search
db.ai.indexes.create({"label": "Product", "propertyName": "description"})

# External — your custom multimodal model
db.ai.indexes.create({
    "label": "Product",
    "propertyName": "description",
    "sourceType": "external",
    "dimensions": 512,
    "similarityFunction": "cosine",
})
```

Specify `similarityFunction` in `db.ai.search()` to route the query to the intended index.
