---
sidebar_position: 0
title: Overview
---

# AI & Semantic Search

RushDB is a **self-aware memory layer for agents, humans, and apps**. It continuously understands its own structure — labels, fields, value distributions, relationships — and exposes that knowledge so agents can reason over real data without hallucinating schema details, and apps can retrieve semantically relevant context on demand.

The `db.ai` namespace covers three capabilities:

| Capability | Description |
|---|---|
| **Graph Ontology** | Self-describing schema discovery: label names, field types, value ranges, and the relationship map — always up to date |
| **Embedding Indexes** | Per-label vector policies that turn string properties into long-term semantic memory |
| **Semantic Search** | Cosine/euclidean similarity retrieval over indexed properties, for agents and apps alike |

---

## How it fits together

```
┌─────────────────────────────────────────────────────┐
│  Your data (records + relationships)                │
│                                                     │
│  BOOK { title: "...", description: "..." }          │
└────────────────────┬────────────────────────────────┘
                     │
         db.ai.indexes.create()
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│  Embedding index policy                             │
│  label: BOOK  property: description  dims: 1536    │
│  sourceType: managed | external                     │
└────────────────────┬────────────────────────────────┘
                     │
      Backfill (managed) / inline vectors (external)
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│  Vector stored on VALUE relationship                │
│  rel._emb_managed_cosine_1536 = [0.1, 0.2, ...]    │
└────────────────────┬────────────────────────────────┘
                     │
          db.ai.search({ query / queryVector })
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│  Records ranked by similarity score                 │
│  result.__score = 0.94  (cosine similarity)        │
└─────────────────────────────────────────────────────┘
```

---

## Quick links

| Topic | Description |
|---|---|
| [Ontology](#graph-ontology) | Schema discovery with `get_ontology_markdown` / `get_ontology` |
| [Indexing](./indexing.md) | Create and manage managed embedding indexes |
| [Advanced Indexing — BYOV](./advanced-indexing.md) | Bring Your Own Vectors: external indexes, inline writes |
| [Semantic Search](./search.md) | Query by meaning with `db.ai.search()` |
| [Writing with Vectors](./write-with-vectors.md) | Attach vectors at create / upsert / import_json time |

---

## Graph Ontology

The ontology methods expose a live snapshot of your database structure — without any manual schema definitions.

### `db.ai.get_ontology_markdown()`

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

### `db.ai.get_ontology()`

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

# Look up property ID for value enumeration
response = db.ai.get_ontology({"labels": ["Order"]})
order_schema = response.data[0]
status_prop = next(p for p in order_schema["properties"] if p["name"] == "status")

values_response = db.properties.values({"id": status_prop["id"]})
# ['pending', 'paid', 'shipped', 'cancelled', 'refunded']
```

Each item in `response.data`:

```python
{
    "label": str,
    "count": int,
    "properties": [
        {
            "id": str,                       # use with db.properties.values()
            "name": str,
            "type": str,                     # 'string' | 'number' | 'boolean' | 'datetime'
            "values": list,                  # up to 10 samples (string/boolean only)
            "min": str | float | None,       # number/datetime only
            "max": str | float | None,
        }
    ],
    "relationships": [
        {
            "label": str,
            "type": str,
            "direction": str,                # 'in' | 'out'
        }
    ]
}
```

:::note Caching
Both methods share a **1-hour cache** per project. The first call after TTL expiry triggers a full graph scan; all subsequent calls within the hour are instant.
:::

:::tip Agent quickstart
Call `db.ai.get_ontology_markdown()` first in every AI session. Without it, models will hallucinate field and label names.
:::
