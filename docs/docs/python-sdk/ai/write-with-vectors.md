---
sidebar_position: 4
title: Writing Records with Vectors
---

# Writing Records with Vectors

RushDB lets you attach pre-computed embedding vectors to records **at write time**, eliminating the need for a separate `db.ai.indexes.upsert_vectors()` call. Any operation that creates or modifies records supports this through the `vectors` parameter.

This feature requires at least one [external index](./advanced-indexing.md) to exist for the target `(label, propertyName)`.

---

## `vectors` parameter format

All write methods accept a `vectors` list of dicts:

```python
vectors = [
    {
        "propertyName": "description",   # required
        "vector": [0.1, 0.9, 0.4, ...], # required
        "similarityFunction": "cosine",  # required only when multiple indexes share (label, propertyName)
    }
]
```

---

## Create a Record with Vectors

`db.records.create()`

```python
record = db.records.create(
    label="Article",
    data={
        "title": "How transformers work",
        "body": "Attention is all you need ...",
    },
    vectors=[
        {"propertyName": "body", "vector": my_embedder.embed("Attention is all you need ...")}
    ],
)

print(record.id)  # record created AND vector written atomically
```

---

## Upsert with Vectors

`db.records.upsert()`

`upsert` is idempotent on the record's natural key (`mergeBy`). Passing `vectors` writes or replaces the stored vector for each `propertyName` in the same call:

```python
# First call — creates the record + writes vector
r1 = db.records.upsert(
    label="Article",
    data={"slug": "transformers-101", "title": "Transformers 101", "body": "..."},
    options={"mergeBy": ["slug"], "mergeStrategy": "append"},
    vectors=[{"propertyName": "body", "vector": v1}],
)

# Second call — same slug → updates data + replaces the vector
r2 = db.records.upsert(
    label="Article",
    data={"slug": "transformers-101", "title": "Transformers 101 (revised)", "body": "Updated ..."},
    options={"mergeBy": ["slug"], "mergeStrategy": "append"},
    vectors=[{"propertyName": "body", "vector": v2}],
)

# r1.__id == r2.__id  — same record
```

---

## Set with Vectors

`db.records.set()`

`set` replaces all properties of a record with new values. Including `vectors` writes those vectors at the same time:

```python
# Full replace — data AND vector updated together
db.records.set(
    target=record,
    label="Product",
    data={"name": "Widget Pro", "price": 19.99},
    vectors=[{"propertyName": "description", "vector": new_vec}],
)
```

---

## Create Multiple Records with Vectors

`db.records.create_many()`

`create_many` is optimised for flat rows. Use the top-level `vectors` parameter — a list indexed by row position — to attach a vector to each record without nesting inside your flat data:

```python
db.records.create_many(
    label="Product",
    data=[
        {"name": "Alpha", "description": "First product"},
        {"name": "Beta",  "description": "Second product"},
        {"name": "Gamma", "description": "Third product"},
    ],
    vectors=[
        [{"propertyName": "description", "vector": [1, 0, 0]}],  # row 0
        [{"propertyName": "description", "vector": [0, 1, 0]}],  # row 1
        [{"propertyName": "description", "vector": [0, 0, 1]}],  # row 2
    ],
)
```

### Sparse vectors

Leave rows without vectors by providing a shorter `vectors` list — any unspecified trailing rows are skipped:

```python
db.records.create_many(
    label="Product",
    data=[{"name": "Alpha"}, {"name": "Beta"}, {"name": "Gamma"}],
    # only row 0 gets a vector; rows 1 and 2 are skipped
    vectors=[[{"propertyName": "description", "vector": my_vec}]],
)
```

---

## Import CSV with Vectors

`db.records.import_csv()`

CSV data is a raw string, so per-row vectors are supplied as a separate `vectors` parameter using the same indexed-list format. Row indices are 0-based and refer to data rows after the header is consumed:

```python
csv_data = """name,description
Alpha,First product
Beta,Second product
Gamma,Third product"""

db.records.import_csv(
    label="Product",
    data=csv_data,
    vectors=[
        [{"propertyName": "description", "vector": [1, 0, 0]}],  # csv row 0
        [{"propertyName": "description", "vector": [0, 1, 0]}],  # csv row 1
        [{"propertyName": "description", "vector": [0, 0, 1]}],  # csv row 2
    ],
)
```

The server returns `400 Bad Request` if `vectors` length exceeds the number of CSV data rows (validated after CSV parsing).

---

## Specifying `similarityFunction` for disambiguation

When a `(label, propertyName)` has multiple external indexes registered (e.g. one cosine and one euclidean), include `similarityFunction` in each vector entry so the server routes the write to the correct index:

```python
# Write to the cosine index
db.records.create(
    label="Product",
    data={"name": "Widget"},
    vectors=[
        {"propertyName": "embedding", "vector": vec, "similarityFunction": "cosine"}
    ],
)
```

Omitting `similarityFunction` when multiple indexes match returns `422 Unprocessable Entity`.

---

## Multiple vectors in one call

Write vectors for multiple properties or indexes in a single operation:

```python
db.records.create(
    label="Document",
    data={"title": "Multi-modal doc", "abstract": "...", "fullText": "..."},
    vectors=[
        {"propertyName": "abstract",  "vector": abstract_vec},
        {"propertyName": "fullText",  "vector": full_text_vec},
    ],
)
```

Each entry is matched independently against the available external indexes.

---

## Complete worked example

```python
from rushdb import RushDB

db  = RushDB("your-api-key")
emb = YourEmbeddingModel()

# 1. Create an external index (safe to call multiple times — 409 on duplicate)
try:
    idx_response = db.ai.indexes.create({
        "label": "Article",
        "propertyName": "body",
        "sourceType": "external",
        "dimensions": 768,
        "similarityFunction": "cosine",
    })
    index_id = idx_response.data["id"]
except Exception:
    index_id = next(
        i["id"] for i in db.ai.indexes.find().data
        if i["label"] == "Article" and i["propertyName"] == "body"
    )

# 2. Create records from your pipeline, embedding as you go
docs = [
    {"title": "Alpha", "body": "First doc"},
    {"title": "Beta",  "body": "Second doc"},
]

for doc in docs:
    db.records.create(
        label="Article",
        data=doc,
        vectors=[{"propertyName": "body", "vector": emb.embed(doc["body"])}],
    )

# 3. Search
query_vec = emb.embed("first document")
results = db.ai.search({
    "labels": ["Article"],
    "propertyName": "body",
    "queryVector": query_vec,
    "limit": 3,
})

for r in results.data:
    print(f"[{r.score:.3f}] {r['title']}")
```
