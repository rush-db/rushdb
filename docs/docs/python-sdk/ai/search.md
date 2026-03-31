---
sidebar_position: 3
title: Semantic Search
---

# Semantic Search

`db.ai.search()` performs semantic vector search across records that have an associated embedding index.

---

## Signature

```python
db.ai.search(params: dict) -> ApiResponse[list[dict]]
```

| `params` key         | Type                       | Required     | Description                                                                                         |
|----------------------|----------------------------|--------------|-----------------------------------------------------------------------------------------------------|
| `propertyName`       | string                     | **yes**      | The indexed property to search against (e.g. `"description"`)                                      |
| `labels`             | string or list of strings  | **yes**      | Label(s) to search within (min 1)                                                                   |
| `query`              | string                     | conditionally | Free-text query to embed. Required for managed indexes; **not allowed** for external indexes.      |
| `queryVector`        | list of floats             | conditionally | Pre-computed query vector. Required for external indexes. Also accepted for managed indexes (bypasses server embedding). |
| `similarityFunction` | string                     | no           | `"cosine"` or `"euclidean"`. Required when multiple indexes target the same `(label, propertyName)`. |
| `dimensions`         | number                     | no           | Disambiguates when multiple indexes match. Inferred from `len(queryVector)` when `queryVector` is supplied. |
| `where`              | dict                       | no           | Standard RushDB filter expression applied **before** similarity scoring.                            |
| `skip`               | number                     | no           | Pagination offset (default `0`)                                                                     |
| `limit`              | number                     | no           | Maximum results to return (default `20`)                                                            |

---

## Result shape

Results are flat dicts with `__score` injected alongside your record fields, ordered by `__score` descending (closest match first):

```python
{
    "__id": str,      # RushDB record ID
    "__label": str,   # Record label
    "__score": float, # Similarity score, 0–1 (higher = more similar)
    # ... your fields
    "title": str,
    "description": str,
}
```

---

## Managed search (query text)

For a **managed** index, pass `query` — a natural-language string. The server embeds it using the same model that built the index.

```python
response = db.ai.search({
    "propertyName": "description",
    "query": "machine learning for beginners",
    "labels": ["Article"],
    "limit": 5,
})

for result in response.data:
    print(f"[{result['__score']:.3f}] {result['title']}")
```

---

## External search (query vector)

For an **external** index, pass `queryVector` — a pre-computed embedding produced by your own model. No text is sent to an embedding model.

```python
vec = my_embedder.embed("machine learning for beginners")

response = db.ai.search({
    "propertyName": "body",
    "queryVector": vec,
    "labels": ["Article"],
    "limit": 10,
})
```

- `query` is **not allowed** with external indexes.
- `queryVector` is **not required** for managed indexes but is accepted (bypasses server embedding).
- When `queryVector` is supplied, `dimensions` can be omitted — the server infers it from `len(queryVector)`.

---

## Filtering with `where`

The `where` clause acts as a **prefilter** — only records satisfying the filter are candidates for similarity ranking. All `where` operators supported by `db.records.find()` are available here.

```python
response = db.ai.search({
    "propertyName": "description",
    "query": "wireless headphones",
    "labels": ["Product"],
    "where": {
        "category": {"$eq": "electronics"},
        "inStock": {"$eq": True},
        "price": {"$lt": 100},
    },
    "limit": 20,
})
```

---

## Multi-label search

Pass a list of labels to search across multiple entity types simultaneously:

```python
response = db.ai.search({
    "propertyName": "body",
    "query": "machine learning trends",
    "labels": ["Article", "Post", "Comment"],
    "limit": 10,
})

# Each result carries __label so you can tell them apart
for result in response.data:
    print(result["__label"], f"{result['__score']:.3f}", result.get("title") or result.get("text"))
```

All listed labels must have an embedding index on the same `propertyName`, or the request returns `404` for the missing labels.

---

## Disambiguation

When two indexes exist for the same `(label, propertyName)`, specify `similarityFunction` to select the target index:

```python
# Two indexes: Product:embedding/cosine and Product:embedding/euclidean
response = db.ai.search({
    "labels": ["Product"],
    "propertyName": "embedding",
    "queryVector": vec,
    "similarityFunction": "cosine",   # required — otherwise 422
})
```

---

## Pagination

```python
PAGE = 20

# Page 1
page1 = db.ai.search({
    "propertyName": "description",
    "query": "sustainable packaging",
    "labels": ["Product"],
    "limit": PAGE,
    "skip": 0,
})

# Page 2
page2 = db.ai.search({
    "propertyName": "description",
    "query": "sustainable packaging",
    "labels": ["Product"],
    "limit": PAGE,
    "skip": PAGE,
})
```

---

## Full example: AI agent with semantic search

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

def build_agent_system_prompt() -> str:
    schema = db.ai.get_ontology_markdown().data
    return f"You are a data assistant for RushDB.\n\n{schema}"

def semantic_search(query: str, label: str, limit: int = 5) -> list[dict]:
    response = db.ai.search({
        "propertyName": "description",
        "query": query,
        "labels": [label],
        "limit": limit,
    })
    return response.data

# Retrieve context then pass to LLM
results = semantic_search("climate change research", "Article")
for r in results:
    print(f"[{r['__score']:.3f}] {r['title']}")
```

---

## Error reference

| HTTP | Cause |
|------|-------|
| `404 Not Found` | No enabled embedding index found for `(label, propertyName)` |
| `422 Unprocessable Entity` | Multiple indexes match and `similarityFunction` was not specified |
| `422 Unprocessable Entity` | `query` text supplied for an external index (server cannot embed it) |
| `422 Unprocessable Entity` | `queryVector` length does not match index `dimensions` |
| `503 Service Unavailable` | Embedding model unavailable (managed indexes only) |
