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

Pass `"sourceType": "external"` in the create request. `dimensions` is **required** because the server never calls an embedding model and cannot infer the vector size:

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Article",
    "propertyName": "body",
    "sourceType": "external",
    "dimensions": 768,
    "similarityFunction": "cosine"
  }'
```

### Example Response

```json
{
  "data": {
    "id": "idx_ext_abc123",
    "label": "Article",
    "propertyName": "body",
    "sourceType": "external",
    "similarityFunction": "cosine",
    "dimensions": 768,
    "status": "awaiting_vectors"
  },
  "success": true
}
```

An external index starts with status `awaiting_vectors` and transitions to `ready` once at least one vector has been written.

### External vs managed comparison

|                               | Managed                          | External                                    |
| ----------------------------- | -------------------------------- | ------------------------------------------- |
| `sourceType`                  | `"managed"`                      | `"external"`                                |
| Initial status                | `"pending"`                      | `"awaiting_vectors"`                        |
| Who computes embeddings       | RushDB server (configured model) | Your application                            |
| `dimensions` required         | No (uses server default)         | **Yes**                                     |
| Backfill for existing records | Automatic                        | Manual via `upsertVectors` or inline writes |

---

## Upsert Vectors

`POST /api/v1/ai/indexes/:id/vectors/upsert`

The bulk upload API — ideal for seeding an index from a dataset or syncing after a batch pipeline.

### Request Body

| Field   | Type  | Required | Description                                                   |
| ------- | ----- | -------- | ------------------------------------------------------------- |
| `items` | array | **yes**  | Array of `{ "recordId": string, "vector": number[] }` objects |

### Example Request

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/indexes/idx_ext_abc123/vectors/upsert \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "items": [
      { "recordId": "rec_001", "vector": [0.1, 0.2, 0.3] },
      { "recordId": "rec_002", "vector": [0.7, 0.8, 0.9] }
    ]
  }'
```

The request is **idempotent** — calling it again with the same `recordId` replaces the stored vector.

---

## Writing vectors at record creation time

Instead of a two-step create → upsertVectors flow, you can write vectors inline using the `vectors` field on any write endpoint. See [Write Records with Vectors](/learn/reference/rest-api/ai-and-vectors/write-with-vectors) for the full reference.

```bash
# One step: create record AND write its vector
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Article",
    "data": { "title": "Warp drives", "body": "Alcubierre metric..." },
    "vectors": [{ "propertyName": "body", "vector": [0.1, 0.2, 0.3] }]
  }'
```

---

## Disambiguation {#disambiguation}

When the same `(label, propertyName)` pair is covered by more than one external index (different `similarityFunction` or `dimensions`), specify `similarityFunction` to resolve which index to use:

```bash
# Two indexes: Article:body/cosine and Article:body/euclidean

# ✅ Explicit — writes to the cosine index only
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Article",
    "data": { "title": "Widget", "body": "..." },
    "vectors": [{ "propertyName": "body", "vector": [0.1, 0.9, 0.4], "similarityFunction": "cosine" }]
  }'

# ❌ Missing similarityFunction when two indexes exist → 422 Unprocessable Entity
```

### Index signature uniqueness

Two index policies are considered **identical** (and a second create returns `409 Conflict`) when all five fields match:

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

```bash
# 1. Create the external index
INDEX_ID=$(curl -s -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label":"Doc","propertyName":"content","sourceType":"external","dimensions":3,"similarityFunction":"cosine"}' | \
  jq -r '.data.id')

# 2. Create records with inline vectors
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label":"Doc","data":{"title":"Alpha","content":"First article"},"vectors":[{"propertyName":"content","vector":[1,0,0]}]}'

curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label":"Doc","data":{"title":"Beta","content":"Second article"},"vectors":[{"propertyName":"content","vector":[0,1,0]}]}'

curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label":"Doc","data":{"title":"Gamma","content":"Third article"},"vectors":[{"propertyName":"content","vector":[0,0,1]}]}'

# 3. Search using a pre-computed query vector (closest to Alpha)
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"labels":["Doc"],"propertyName":"content","queryVector":[1,0,0],"limit":3}'
```

---

## Batch import with flat-rows format

For bulk seeding with flat rows, use `POST /api/v1/records/import/json` with the flat-rows format and a top-level `vectors` array:

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/json \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Doc",
    "data": [
      { "title": "Alpha", "content": "First article" },
      { "title": "Beta",  "content": "Second article" },
      { "title": "Gamma", "content": "Third article" }
    ],
    "vectors": [
      [{ "propertyName": "content", "vector": [1, 0, 0] }],
      [{ "propertyName": "content", "vector": [0, 1, 0] }],
      [{ "propertyName": "content", "vector": [0, 0, 1] }]
    ]
  }'
```

For nested JSON payloads, use `importJson` to create records and then call `POST /api/v1/ai/indexes/:id/vectors/upsert` to seed the vectors separately.

---

## Mixing managed and external indexes

You can have both a managed index and an external index on the same property simultaneously:

```bash
# Managed — server embeds for full-text search
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label": "Product", "propertyName": "description"}'

# External — your custom multimodal model
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Product",
    "propertyName": "description",
    "sourceType": "external",
    "dimensions": 512,
    "similarityFunction": "cosine"
  }'
```

Specify `similarityFunction` in `POST /api/v1/ai/search` to route the query to the intended index.
