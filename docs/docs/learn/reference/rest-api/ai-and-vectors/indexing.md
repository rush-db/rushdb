---
sidebar_position: 1
title: Embedding Indexes
---

# Embedding Indexes

An **embedding index** is a policy that tells RushDB to vectorize a specific string property for a label. Once `status` is `ready`, every record matching that label+property pair is searchable via `POST /api/v1/ai/search`.

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

## List Embedding Indexes

`GET /api/v1/ai/indexes`

Returns all embedding index policies for the project.

### Example Response

```json
{
  "data": [
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
      "enabled": true,
      "status": "ready",
      "createdAt": "2025-01-10T12:00:00.000Z",
      "updatedAt": "2025-01-10T12:05:00.000Z"
    }
  ],
  "success": true
}
```

---

## Create Embedding Index

`POST /api/v1/ai/indexes`

Creates a new managed embedding index policy scoped to a label. The property must exist in the graph and have type `string` (scalar or list).

### Request Body

| Field                | Type   | Required | Description                                                                                                                 |
| -------------------- | ------ | -------- | --------------------------------------------------------------------------------------------------------------------------- |
| `label`              | string | **yes**  | Label to scope this index to (e.g. `"Article"`, `"Product"`)                                                                |
| `propertyName`       | string | **yes**  | Name of the property to embed (e.g. `"description"`)                                                                        |
| `sourceType`         | string | no       | `"managed"` (default) or `"external"`. See [Advanced Indexing](/learn/reference/rest-api/ai-and-vectors/advanced-indexing). |
| `similarityFunction` | string | no       | `"cosine"` (default) or `"euclidean"`                                                                                       |
| `dimensions`         | number | no       | Vector dimensionality. Defaults to server `RUSHDB_EMBEDDING_DIMENSIONS`. **Required** for external indexes.                 |

> **Model config is server-side.** The embedding model is set via `RUSHDB_EMBEDDING_MODEL` / `RUSHDB_EMBEDDING_DIMENSIONS` env vars.

### Example — simplest form

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label": "Article", "propertyName": "description"}'
```

### Example — with explicit parameters

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "label": "Article",
    "propertyName": "description",
    "similarityFunction": "cosine",
    "dimensions": 1536
  }'
```

### Example Response

```json
{
  "data": {
    "id": "idx_abc123",
    "label": "Article",
    "propertyName": "description",
    "sourceType": "managed",
    "similarityFunction": "cosine",
    "dimensions": 1536,
    "status": "pending"
  },
  "success": true
}
```

### Index lifecycle

| Status             | Description                                            |
| ------------------ | ------------------------------------------------------ |
| `pending`          | Policy created, waiting for backfill scheduler         |
| `indexing`         | Backfill in progress                                   |
| `awaiting_vectors` | External index — waiting for client to push vectors    |
| `ready`            | All existing records have vectors; search is available |
| `error`            | Backfill failed; check server logs for the cause       |

### Error cases

| Status | When                                                                                                       |
| ------ | ---------------------------------------------------------------------------------------------------------- |
| `404`  | The property does not exist in the project graph                                                           |
| `422`  | The property exists but is not `string` type                                                               |
| `422`  | Embedding model is not configured on the server                                                            |
| `409`  | An index for this `(label, propertyName, sourceType, similarityFunction, dimensions)` tuple already exists |

---

## Delete Embedding Index

`DELETE /api/v1/ai/indexes/:id`

Deletes an embedding index policy. The underlying Neo4j DDL vector index is only dropped when **zero embeddings remain** across the entire project — this avoids unnecessary rebuilds when multiple policies share the same `(dimensions, similarityFunction)`.

### Example Request

```bash
curl -X DELETE https://api.rushdb.com/api/v1/ai/indexes/idx_abc123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

### Example Response

```json
{ "data": { "deleted": true }, "success": true }
```

---

## Get Embedding Index Stats

`GET /api/v1/ai/indexes/:id/stats`

Returns the current indexing progress — useful for progress monitoring or health checks before running search.

### Example Response

```json
{
  "data": {
    "totalRecords": 1840,
    "indexedRecords": 1234
  },
  "success": true
}
```

---

## Waiting for an index to become ready

For managed indexes, backfill runs asynchronously. Poll `GET /api/v1/ai/indexes` until `status` is `ready`:

```bash
# Shell polling loop
while true; do
  STATUS=$(curl -s https://api.rushdb.com/api/v1/ai/indexes \
    -H "Authorization: Bearer $RUSHDB_API_KEY" | \
    jq -r '.data[] | select(.id == "idx_abc123") | .status')
  echo "Status: $STATUS"
  if [ "$STATUS" = "ready" ]; then break; fi
  if [ "$STATUS" = "error" ]; then echo "Index entered error state" && exit 1; fi
  sleep 3
done
```

Or in JavaScript:

```javascript
async function waitForIndexReady(apiKey, indexId, timeoutMs = 90_000) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const res = await fetch('https://api.rushdb.com/api/v1/ai/indexes', {
      headers: { Authorization: `Bearer ${apiKey}` }
    })
    const { data: indexes } = await res.json()
    const idx = indexes.find((i) => i.id === indexId)
    if (idx?.status === 'ready') return
    if (idx?.status === 'error') throw new Error('Index entered error state')
    await new Promise((r) => setTimeout(r, 3_000))
  }
  throw new Error('Index did not become ready in time')
}
```

---

## Multiple indexes on the same property

You can have more than one index per `(label, propertyName)` pair, provided the signature differs (`sourceType`, `similarityFunction`, or `dimensions`):

```bash
# Cosine index
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label": "Product", "propertyName": "description", "similarityFunction": "cosine", "dimensions": 768}'

# Euclidean index on the same property
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label": "Product", "propertyName": "description", "similarityFunction": "euclidean", "dimensions": 768}'
```

When searching or writing vectors against a property with multiple indexes, specify `similarityFunction` to disambiguate. See [Advanced Indexing](/learn/reference/rest-api/ai-and-vectors/advanced-indexing#disambiguation) for details.

---

## Index response shape

```json
{
  "id": "string",
  "projectId": "string",
  "label": "string",
  "propertyName": "string",
  "modelKey": "string",
  "sourceType": "managed | external",
  "similarityFunction": "cosine | euclidean",
  "dimensions": 1536,
  "vectorPropertyName": "string",
  "enabled": true,
  "status": "string",
  "createdAt": "string",
  "updatedAt": "string"
}
```

---

## String Array Properties

`List<String>`

String array properties are supported. Each item in the array is embedded individually, then mean-pooled into a single vector stored on the relationship.
