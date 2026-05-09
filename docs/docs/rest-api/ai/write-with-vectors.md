---
sidebar_position: 4
title: Writing Records with Vectors
---

# Writing Records with Vectors

RushDB lets you attach pre-computed embedding vectors to records **at write time**, eliminating the need for a separate `POST /api/v1/ai/indexes/:id/vectors/upsert` call. Any endpoint that creates or modifies records accepts a `vectors` field.

This feature requires at least one [external index](./advanced-indexing.md) to exist for the target `(label, propertyName)`.

---

## `vectors` field format

All write endpoints accept a `vectors` array:

```json
"vectors": [
  {
    "propertyName": "description",
    "vector": [0.1, 0.9, 0.4],
    "similarityFunction": "cosine"
  }
]
```

| Field                | Type             | Required | Description                                               |
|----------------------|------------------|----------|-----------------------------------------------------------|
| `propertyName`       | string           | **yes**  | Property name this vector is associated with              |
| `vector`             | array of numbers | **yes**  | Pre-computed embedding vector                             |
| `similarityFunction` | string           | no       | Required when multiple indexes exist on the same property |

---

## Create a Record with Vectors

`POST /api/v1/records`

The record is created **and** the vector is written atomically:

```bash
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Article",
    "data": {
      "title": "How transformers work",
      "body": "Attention is all you need ..."
    },
    "vectors": [
      { "propertyName": "body", "vector": [0.1, 0.2, 0.3] }
    ]
  }'
```

---

## Upsert with Vectors

`POST /api/v1/records`

Upsert is idempotent on the record's natural key (set via `mergeBy`). Include `vectors` to write or replace the stored vector in the same call:

```bash
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Article",
    "data": { "slug": "transformers-101", "title": "Transformers 101", "body": "..." },
    "options": { "mergeBy": ["slug"], "mergeStrategy": "append" },
    "vectors": [{ "propertyName": "body", "vector": [0.1, 0.2, 0.3] }]
  }'
```

---

## Set with Vectors

`PUT /api/v1/records/:id`

`PUT` replaces all properties of a record with new values. Including `vectors` writes those vectors at the same time:

```bash
curl -X PUT https://api.rushdb.com/api/v1/records/rec_abc123 \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Product",
    "data": { "name": "Widget Pro", "price": 19.99 },
    "vectors": [{ "propertyName": "description", "vector": [0.5, 0.6, 0.7] }]
  }'
```

---

## Import JSON Flat Rows with Vectors

`POST /api/v1/records/import/json`

When using the flat-rows format, provide a top-level `vectors` array indexed by row position:

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/json \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Product",
    "data": [
      { "name": "Alpha", "description": "First product" },
      { "name": "Beta",  "description": "Second product" },
      { "name": "Gamma", "description": "Third product" }
    ],
    "vectors": [
      [{ "propertyName": "description", "vector": [1, 0, 0] }],
      [{ "propertyName": "description", "vector": [0, 1, 0] }],
      [{ "propertyName": "description", "vector": [0, 0, 1] }]
    ]
  }'
```

### Sparse vectors

Leave rows without vectors by providing a shorter `vectors` array — any rows beyond `vectors.length` are skipped:

```bash
# Only row 0 gets a vector; rows 1 and 2 are skipped
curl -X POST https://api.rushdb.com/api/v1/records/import/json \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Product",
    "data": [{ "name": "Alpha" }, { "name": "Beta" }, { "name": "Gamma" }],
    "vectors": [[{ "propertyName": "description", "vector": [1, 0, 0] }]]
  }'
```

---

## Import CSV with Vectors

`POST /api/v1/records/import/csv`

CSV data is a raw string, so per-row vectors are supplied as a separate `vectors` array using the same indexed format. Row indices are 0-based and refer to data rows after the header is consumed:

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/csv \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Product",
    "data": "name,description\nAlpha,First product\nBeta,Second product\nGamma,Third product",
    "vectors": [
      [{ "propertyName": "description", "vector": [1, 0, 0] }],
      [{ "propertyName": "description", "vector": [0, 1, 0] }],
      [{ "propertyName": "description", "vector": [0, 0, 1] }]
    ]
  }'
```

The server returns `400 Bad Request` if `vectors.length` exceeds the number of CSV data rows (validated after CSV parsing).

---

## Specifying `similarityFunction` for disambiguation

When a `(label, propertyName)` has multiple external indexes, include `similarityFunction` in each vector entry to route to the correct index:

```bash
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Product",
    "data": { "name": "Widget" },
    "vectors": [
      { "propertyName": "embedding", "vector": [0.1, 0.9], "similarityFunction": "cosine" }
    ]
  }'
```

Omitting `similarityFunction` when multiple indexes match returns `422 Unprocessable Entity`.

---

## Multiple vectors in one call

Write vectors for multiple properties in a single request:

```bash
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Document",
    "data": { "title": "Multi-modal doc", "abstract": "...", "fullText": "..." },
    "vectors": [
      { "propertyName": "abstract",  "vector": [0.1, 0.2, 0.3] },
      { "propertyName": "fullText",  "vector": [0.7, 0.8, 0.9] }
    ]
  }'
```

Each entry is matched independently against the available external indexes.
