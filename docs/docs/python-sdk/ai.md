---
sidebar_position: 1
---

# AI & Semantic Search

The `db.ai` namespace covers three things: **graph ontology** (schema discovery for LLM agents), **embedding indexes** (per-label vector policies), and **semantic search** (cosine-similarity queries over indexed properties).

:::tip Agent quickstart
Call `db.ai.get_ontology_markdown()` first in every AI session — it returns all label names, field names, value ranges, and the relationship map as one token-efficient Markdown string. Without it, the model will hallucinate label and field names.
:::

---

## `db.ai.get_ontology_markdown()`

Returns the full schema as compact Markdown — the **recommended format for LLM context injection**.

```python
db.ai.get_ontology_markdown(
    params: dict | None = None,   # {"labels": ["Order"]} to scope output
    transaction=None
) -> ApiResponse[str]
```

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

# Inject into LLM at session start
response = db.ai.get_ontology_markdown()
schema = response.data

messages = [
    {"role": "system", "content": f"You are a data assistant.\n\n{schema}"},
    {"role": "user",   "content": "How many paid orders are there?"}
]

# Scope to specific labels
order_response = db.ai.get_ontology_markdown({"labels": ["Order"]})
```

<details>
<summary>Example output</summary>

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
```

</details>

---

## `db.ai.get_ontology()`

Returns the same ontology as a structured list of dicts — useful for schema UIs, auto-complete, or looking up property IDs for `db.properties.values()`.

```python
db.ai.get_ontology(
    params: dict | None = None,
    transaction=None
) -> ApiResponse[list[dict]]
```

```python
# List all labels with counts
response = db.ai.get_ontology()
for item in response.data:
    print(f"{item['label']}: {item['count']} records")

# Get property ID for value enumeration
response = db.ai.get_ontology({"labels": ["Order"]})
order_schema = response.data[0]
status_prop = next(p for p in order_schema["properties"] if p["name"] == "status")

values_response = db.properties.values({"id": status_prop["id"]})
# ['pending', 'paid', 'shipped', 'cancelled', 'refunded']
```

:::note Caching
Both methods share a **1-hour cache** on the ProjectNode. The first call after TTL expiry triggers a full graph scan; all subsequent calls within the hour are instant.
:::

---

## Embedding Indexes

An embedding index policy tells RushDB to vectorize a specific string property, scoped to a label. Once `status` is `ready`, that property is searchable via `db.ai.search()`.

`List[str]` properties are supported — each item is embedded and mean-pooled into one vector.

> **Model config is server-side** — the embedding model and dimensions come from `RUSHDB_EMBEDDING_MODEL` / `RUSHDB_EMBEDDING_DIMENSIONS` env vars.

### `db.ai.indexes.find()`

```python
response = db.ai.indexes.find()
for index in response.data:
    print(f"{index['label']}.{index['propertyName']} — {index['status']}")
```

### `db.ai.indexes.create()`

```python
db.ai.indexes.create(params: dict) -> ApiResponse[dict]
# params: { "label": str, "propertyName": str }
```

```python
response = db.ai.indexes.create({
    "label": "Article",
    "propertyName": "description"
})
print(response.data["status"])  # 'pending' → backfill starts immediately
```

> Duplicate `(label, propertyName)` pairs return `409 Conflict`.

### `db.ai.indexes.stats(index_id)`

```python
response = db.ai.indexes.stats(index_id)
stats = response.data
print(f"{stats['indexedRecords']} / {stats['totalRecords']} embedded")
```

### `db.ai.indexes.delete(index_id)`

```python
db.ai.indexes.delete(index_id)
```

### Index shape

```python
{
    "id": str,
    "projectId": str,
    "label": str,         # Neo4j label this index is scoped to
    "propertyName": str,
    "modelKey": str,
    "dimensions": int,
    "enabled": bool,
    "status": str,        # 'pending' | 'indexing' | 'ready' | 'error'
    "createdAt": str,
    "updatedAt": str,
}
```

---

## `db.ai.search()`

Embeds the query text and returns relevant records by cosine similarity. Requires a `ready` embedding index on `property_name` scoped to the target `label`.

RushDB performs exact semantic search: candidates are narrowed via labels and optional `where`, then ranked by cosine similarity.

```python
db.ai.search(params: dict) -> ApiResponse[list[dict]]
# params: {
#   "propertyName": str,    # required
#   "query": str,           # required
#   "labels": list[str],    # required, min 1
#   "where": dict,          # optional filter applied before cosine scoring
#   "skip": int,            # optional
#   "limit": int,           # optional
# }
```

```python
# Semantic search
response = db.ai.search({
    "propertyName": "description",
    "query": "machine learning for beginners",
    "labels": ["Article"],
    "limit": 5
})

for result in response.data:
    print(f"[{result['__score']:.3f}] {result['title']}")

# Semantic search with filter
response = db.ai.search({
    "propertyName": "description",
    "query": "sustainable packaging",
    "labels": ["Product"],
    "where": {"status": {"$in": ["active", "featured"]}},
    "limit": 10
})
```

### Response shape

Results are flat dicts with `__score` injected alongside your record fields:

```python
{
    "__id": str,      # RushDB record ID
    "__label": str,   # Neo4j label
    "__score": float, # cosine similarity, 0–1 (higher = more similar)
    # ... your fields
    "title": str,
    "description": str,
}
```

---

## Full example: AI agent with schema context

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

# Example
results = semantic_search("climate change research", "Article")
for r in results:
    print(f"[{r['__score']:.3f}] {r['title']}")
```
