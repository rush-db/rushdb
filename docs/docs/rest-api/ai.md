---
sidebar_position: 1
---

# AI & Semantic Search

The AI API covers three areas: **graph ontology** (schema discovery for LLM agents), **embedding indexes** (per-label vector policies), and **semantic search** (cosine-similarity queries over indexed properties).

:::tip Agent quickstart
Call `POST /api/v1/ai/ontology/md` first in every AI session — it returns all label names, field names, value ranges, and the relationship map in one token-efficient Markdown string.
:::

---

## Get Ontology (Markdown)

```http
POST /api/v1/ai/ontology/md
```

Returns the full graph schema as compact Markdown tables. This is the **recommended format for LLM agents**: token-efficient, human-readable, and ready to paste into a system prompt or tool result.

### Request Body

| Field    | Type             | Required | Description                                                                 |
|----------|------------------|----------|-----------------------------------------------------------------------------|
| `labels` | array of strings | no       | Restrict output to specific labels. Omit (or pass `[]`) for the full schema. |

### Example Request

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/ontology/md \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{}'
```

### Example Response

```text
# Graph Ontology

## Labels

| Label     | Count |
|-----------|------:|
| `Order`   |  1840 |
| `User`    |   312 |
| `Product` |    95 |

---

## `Order` (1840 records)

### Properties

| Property    | Type     | Values / Range                           |
|-------------|----------|------------------------------------------|
| `status`    | string   | `pending`, `paid`, `shipped` (+2 more)   |
| `total`     | number   | `4.99`..`2499.00`                        |
| `createdAt` | datetime | `2024-01-03`..`2026-02-27`               |

### Relationships

| Type        | Direction | Other Label |
|-------------|-----------|-------------|
| `PLACED_BY` | out       | `User`      |
| `CONTAINS`  | out       | `Product`   |

---

## `User` (312 records)

### Properties

| Property | Type   | Values / Range                 |
|----------|--------|--------------------------------|
| `email`  | string | `alice@…`, `bob@…` (+310 more) |
| `plan`   | string | `free`, `pro`, `enterprise`    |

### Relationships

| Type        | Direction | Other Label |
|-------------|-----------|-------------|
| `PLACED_BY` | in        | `Order`     |
```

### Filtered request (single label)

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/ontology/md \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"labels": ["Order"]}'
```

Returns only the `Order` section. The underlying cache still covers the full schema — filtering is applied in-memory.

---

## Get Ontology (JSON)

```http
POST /api/v1/ai/ontology
```

Returns the same ontology as a structured JSON array. Each element describes one label.

### Request Body

| Field    | Type             | Required | Description                                          |
|----------|------------------|----------|------------------------------------------------------|
| `labels` | array of strings | no       | Restrict to specific labels. Omit for the full schema. |

### Response Schema

```json
[
  {
    "label": "Order",
    "count": 1840,
    "properties": [
      { "id": "prop_abc123", "name": "status", "type": "string", "values": ["pending", "paid", "shipped"] },
      { "id": "prop_def456", "name": "total",  "type": "number", "min": 4.99, "max": 2499.00 }
    ],
    "relationships": [
      { "label": "User",    "type": "PLACED_BY", "direction": "out" },
      { "label": "Product", "type": "CONTAINS",  "direction": "out" }
    ]
  }
]
```

- `properties[].id` — pass to `GET /api/v1/properties/:id/values` to enumerate all distinct values
- `properties[].values` — up to 10 samples (string/boolean only)
- `properties[].min` / `.max` — range info (number/datetime only)
- `relationships[].direction` — `out` = this label is source; `in` = this label is target

---

:::note Caching
Both endpoints share a **1-hour cache** on the ProjectNode. First call after TTL expiry triggers a full graph scan; all subsequent calls within the hour are instant.
:::

---

## Embedding Indexes

Embedding index policies tell RushDB which string properties to vectorize. Once a policy exists, vectors are stored on the `VALUE` relationship edges and become available for semantic (cosine-similarity) search.

`List<String>` properties are supported — each item is embedded individually and the vectors are mean-pooled into a single representation.

---

### List Embedding Indexes

```http
GET /api/v1/ai/indexes
```

Returns all embedding index policies for the project.

#### Example Response

```json
{
  "data": [
    {
      "id": "idx_abc123",
      "projectId": "proj_xyz",
      "label": "Article",
      "propertyName": "description",
      "modelKey": "text-embedding-3-small",
      "dimensions": 1536,
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

### Create Embedding Index

```http
POST /api/v1/ai/indexes
```

Creates a new embedding index policy scoped to a label. The property must exist in the graph and have type `string` (scalar or list).

#### Request Body

| Field          | Type     | Required | Description                                                                       |
|----------------|----------|----------|-----------------------------------------------------------------------------------|
| `label`        | string   | **yes**  | Neo4j label to scope this index to (e.g. `"Article"`, `"Product"`)               |
| `propertyName` | string   | **yes**  | Name of the property to embed (e.g. `"description"`)                             |

> **Model config is server-side.** The embedding model and vector dimensions are set via `RUSHDB_EMBEDDING_MODEL` / `RUSHDB_EMBEDDING_DIMENSIONS` env vars.

#### Example Request

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"label": "Article", "propertyName": "description"}'
```

#### Error cases

| Status | When                                                                          |
|--------|-------------------------------------------------------------------------------|
| `404`  | The property does not exist in the project graph                              |
| `422`  | The property exists but is not a `string` type                                |
| `422`  | Embedding model is not configured on the server                               |
| `409`  | An index for this `(label, propertyName)` pair already exists                 |

---

### Delete Embedding Index

```http
DELETE /api/v1/ai/indexes/:id
```

Deletes an embedding index policy. This removes the policy record but does not delete vectors already written to the graph.

#### Example Request

```bash
curl -X DELETE https://api.rushdb.com/api/v1/ai/indexes/idx_abc123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

#### Example Response

```json
{ "data": { "deleted": true }, "success": true }
```

---

### Get Embedding Index Stats

```http
GET /api/v1/ai/indexes/:id/stats
```

Returns the current indexing progress for an embedding index — how many records have the property and how many have already had vectors generated.

#### Example Response

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

## Semantic Search

```http
POST /api/v1/ai/search
```

Embeds the supplied query text and returns the most relevant records by vector similarity. The property referenced by `propertyName` must have a `ready` embedding index.

Two execution modes are selected automatically:

| Mode | When | How |
|------|------|-----|
| **ANN** (fast, approximate) | No `where` filter; `labels` has 0 or 1 entries | Queries the shared global vector index directly |
| **ENN prefilter** (exact, slower) | `where` filter present **or** `labels` has 2+ entries | Narrows candidates via MATCH/WHERE first, then scores with cosine similarity |

### Request Body

| Field          | Type             | Required | Description                                                                              |
|----------------|------------------|----------|------------------------------------------------------------------------------------------|
| `propertyName` | string           | **yes**  | The indexed property to search against                                                   |
| `query`        | string           | **yes**  | Free-text query to embed and compare                                                     |
| `labels`       | array of strings | **yes**  | Labels to search within (min 1). Single label = ANN mode; 2+ or with `where` = ENN.    |
| `where`        | object           | no       | Standard RushDB filter expression. Activates ENN prefilter mode when provided.          |
| `topK`         | number           | no       | Candidate count for ANN index scan (default `20`, ignored in prefilter mode)             |
| `skip`         | number           | no       | Pagination offset (default `0`)                                                          |
| `limit`        | number           | no       | Maximum results to return (default `20`)                                                 |

### Example — ANN (single label, no filter)

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "propertyName": "description",
    "query": "fast delivery and easy returns",
    "labels": ["Product"],
    "limit": 5
  }'
```

### Example — ENN with prefilter

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "propertyName": "description",
    "query": "fast delivery and easy returns",
    "labels": ["Product"],
    "where": {
      "status": {"$in": ["active", "featured"]}
    },
    "limit": 10
  }'
```

### Example Response

Results are flat records with `__score` injected alongside other fields:

```json
{
  "data": [
    {
      "__id": "rec_abc123",
      "__label": "Product",
      "__score": 0.921,
      "description": "Same-day shipping with hassle-free returns policy",
      "status": "active"
    },
    {
      "__id": "rec_def456",
      "__label": "Product",
      "__score": 0.887,
      "description": "Free returns within 30 days, express shipping available",
      "status": "featured"
    }
  ],
  "success": true
}
```

### Error cases

| Status | When                                                               |
|--------|--------------------------------------------------------------------|
| `404`  | No embedding index found for the specified property                |
| `422`  | The embedding index exists but is not yet in `ready` status        |
