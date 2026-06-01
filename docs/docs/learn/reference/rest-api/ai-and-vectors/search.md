---
slug: /rest-api/ai/search
sidebar_position: 3
title: Semantic Search
---

# Semantic Search

`POST /api/v1/ai/search`

Embeds the supplied query text (or uses a pre-computed vector) and returns the most relevant records by similarity score. The property referenced by `propertyName` must have a `ready` embedding index.

RushDB performs exact semantic search: candidates are narrowed with label and `where` filters first, then ranked by cosine or euclidean similarity.

---

## Request Body

| Field                | Type                       | Required      | Description                                                                                                              |
| -------------------- | -------------------------- | ------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `propertyName`       | string                     | **yes**       | The indexed property to search against (e.g. `"description"`)                                                            |
| `labels`             | string or array of strings | **yes**       | Label(s) to search within (min 1)                                                                                        |
| `query`              | string                     | conditionally | Free-text query to embed. Required for managed indexes; **not allowed** for external indexes.                            |
| `queryVector`        | array of numbers           | conditionally | Pre-computed query vector. Required for external indexes. Also accepted for managed indexes (bypasses server embedding). |
| `similarityFunction` | string                     | no            | `"cosine"` or `"euclidean"`. Required when multiple indexes target the same `(label, propertyName)`.                     |
| `dimensions`         | number                     | no            | Disambiguates when multiple indexes match. Inferred from `queryVector.length` when `queryVector` is supplied.            |
| `where`              | object                     | no            | Standard RushDB filter expression applied **before** similarity scoring.                                                 |
| `skip`               | number                     | no            | Pagination offset (default `0`)                                                                                          |
| `limit`              | number                     | no            | Maximum results to return (default `20`)                                                                                 |

---

## Result shape

Results are flat records with `__score` injected alongside your fields, ordered by `__score` descending (closest match first):

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

---

## Managed search (query text)

For a **managed** index, pass `query` — a natural-language string. The server embeds it using the same model that was used when building the index, then ranks candidates by similarity.

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

---

## External search (query vector)

For an **external** index, pass `queryVector` — a pre-computed embedding produced by your own model. No text is sent to an embedding model.

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "propertyName": "body",
    "queryVector": [0.1, 0.2, 0.3],
    "labels": ["Article"],
    "limit": 10
  }'
```

- `query` is **not allowed** with external indexes — the server has no model to embed it.
- `queryVector` is **not required** for managed indexes but is accepted (bypasses server embedding).
- When `queryVector` is supplied, `dimensions` can be omitted — the server infers it from `queryVector.length`.

---

## Filtering with `where`

The `where` clause acts as a **prefilter** — only records satisfying the filter are candidates for similarity ranking. All filter operators supported by `POST /api/v1/records/search` are available here.

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "propertyName": "description",
    "query": "wireless headphones",
    "labels": ["Product"],
    "where": {
      "category": { "$eq": "electronics" },
      "inStock": { "$eq": true },
      "price": { "$lt": 100 }
    },
    "limit": 20
  }'
```

---

## Multi-label search

Pass an array of labels to search across multiple entity types simultaneously:

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "propertyName": "body",
    "query": "machine learning trends",
    "labels": ["Article", "Post", "Comment"],
    "limit": 10
  }'
```

Each result carries `__label` so you can tell the entity types apart:

```json
{
  "data": [
    { "__id": "rec_1", "__label": "Article", "__score": 0.93, "title": "...", "body": "..." },
    { "__id": "rec_2", "__label": "Post", "__score": 0.87, "body": "..." },
    { "__id": "rec_3", "__label": "Comment", "__score": 0.82, "body": "..." }
  ],
  "success": true
}
```

All listed labels must have an embedding index on the same `propertyName`, or the request returns `404` for the missing labels.

---

## Disambiguation

When two indexes exist for the same `(label, propertyName)`, specify `similarityFunction` to select the target index:

```bash
# Two indexes: Product:embedding/cosine and Product:embedding/euclidean
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "labels": ["Product"],
    "propertyName": "embedding",
    "queryVector": [0.1, 0.9, 0.4],
    "similarityFunction": "cosine"
  }'
```

Omitting `similarityFunction` when multiple indexes match returns `422 Unprocessable Entity`.

---

## Pagination

```bash
# Page 1
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"propertyName":"description","query":"...", "labels":["Product"],"limit":20,"skip":0}'

# Page 2
curl -X POST https://api.rushdb.com/api/v1/ai/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"propertyName":"description","query":"...", "labels":["Product"],"limit":20,"skip":20}'
```

---

## Error reference

| HTTP                       | Cause                                                                |
| -------------------------- | -------------------------------------------------------------------- |
| `404 Not Found`            | No enabled embedding index found for `(label, propertyName)`         |
| `422 Unprocessable Entity` | Multiple indexes match and `similarityFunction` was not specified    |
| `422 Unprocessable Entity` | `query` text supplied for an external index (server cannot embed it) |
| `422 Unprocessable Entity` | `queryVector` length does not match index `dimensions`               |
| `503 Service Unavailable`  | Embedding model unavailable (managed indexes only)                   |
