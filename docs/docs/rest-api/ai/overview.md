---
sidebar_position: 0
title: Overview
---

# AI & Semantic Search

RushDB is a **self-aware memory layer for agents, humans, and apps**. It continuously understands its own structure — labels, fields, value distributions, relationships — and exposes that knowledge so agents can reason over real data without hallucinating schema details, and apps can retrieve semantically relevant context on demand.

The AI API covers three capabilities:

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
         POST /api/v1/ai/indexes
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
          POST /api/v1/ai/search
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
| [Ontology](#graph-ontology) | Schema discovery with `POST /api/v1/ai/ontology/md` and `POST /api/v1/ai/ontology` |
| [Indexing](./indexing.md) | Create and manage managed embedding indexes |
| [Advanced Indexing — BYOV](./advanced-indexing.md) | Bring Your Own Vectors: external indexes, inline writes |
| [Semantic Search](./search.md) | Query by meaning with `POST /api/v1/ai/search` |
| [Writing with Vectors](./write-with-vectors.md) | Attach vectors at create / upsert / importJson time |

---

## Graph Ontology

The ontology endpoints expose a live snapshot of your database structure — without any manual schema definitions.

### Get Ontology (Markdown)

```http
POST /api/v1/ai/ontology/md
```

Returns the full schema as compact Markdown — the **recommended format for LLM context injection**: token-efficient, human-readable, and ready to paste into a system prompt or tool result.

#### Request Body

| Field    | Type             | Required | Description                                                                 |
|----------|------------------|----------|-----------------------------------------------------------------------------|
| `labels` | array of strings | no       | Restrict output to specific labels. Omit (or pass `[]`) for the full schema. |

#### Example Request

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/ontology/md \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{}'
```

#### Example Response

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

#### Filtered request (single label)

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/ontology/md \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"labels": ["Order"]}'
```

Returns only the `Order` section. The underlying cache still covers the full schema — filtering is applied in-memory.

---

### Get Ontology (JSON)

```http
POST /api/v1/ai/ontology
```

Returns the same ontology as a structured JSON array. Each element describes one label.

#### Request Body

| Field    | Type             | Required | Description                                            |
|----------|------------------|----------|--------------------------------------------------------|
| `labels` | array of strings | no       | Restrict to specific labels. Omit for the full schema. |

#### Response Schema

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

:::tip Agent quickstart
Call `POST /api/v1/ai/ontology/md` first in every AI session. Without it, models will hallucinate label and field names.
:::
