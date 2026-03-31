---
sidebar_position: 0
title: Overview
---

# AI & Semantic Search

RushDB is a **self-aware memory layer for agents, humans, and apps**. It continuously understands its own structure — labels, fields, value distributions, relationships — and exposes that knowledge so that agents can reason over real data without hallucinating schema details, and apps can retrieve semantically relevant context on demand.

The `db.ai` namespace covers three capabilities:

| Capability            | Description                                                                                                            |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Graph Ontology**    | Self-describing schema discovery: label names, field types, value ranges, and the relationship map — always up to date |
| **Embedding Indexes** | Per-label vector policies that turn string properties into long-term semantic memory                                   |
| **Semantic Search**   | Cosine/euclidean similarity retrieval over indexed properties, for agents and apps alike                               |

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
│  SemanticSearchResult[] — records ranked by score  │
│  result.__score = 0.94  (cosine similarity)        │
└─────────────────────────────────────────────────────┘
```

---

## Quick links

| Topic                                              | Description                                                 |
| -------------------------------------------------- | ----------------------------------------------------------- |
| [Ontology](./overview.md#graph-ontology)           | Schema discovery with `getOntology` / `getOntologyMarkdown` |
| [Indexing](./indexing.md)                          | Create and manage managed embedding indexes                 |
| [Advanced indexing — BYOV](./advanced-indexing.md) | Bring Your Own Vectors: external indexes, inline writes     |
| [Semantic search](./search.md)                     | Query by meaning with `db.ai.search()`                      |
| [Writing with vectors](./write-with-vectors.md)    | Attach vectors at create / upsert / importJson time         |

---

## Graph Ontology

The ontology endpoints expose a live snapshot of your database structure — without any manual schema definitions.

### Get Ontology as Markdown

`db.ai.getOntologyMarkdown()`

Returns the full schema as compact Markdown — the **recommended format for LLM context injection**.

```typescript
db.ai.getOntologyMarkdown(
  params?: { labels?: string[] },
  transaction?: Transaction | string
): Promise<ApiResponse<string>>
```

```typescript
// Inject into LLM at session start
const { data: schema } = await db.ai.getOntologyMarkdown();
const messages = [
  { role: "system", content: `You are a data assistant.\n\n${schema}` },
  { role: "user", content: "How many paid orders are there?" },
];

// Scope to specific labels
const { data: orderSchema } = await db.ai.getOntologyMarkdown({
  labels: ["Order"],
});
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

### Get Ontology (raw)

`db.ai.getOntology()`

Returns the same ontology as a structured JSON array — useful for schema UIs, auto-complete, or looking up property IDs for `db.properties.values()`.

```typescript
db.ai.getOntology(
  params?: { labels?: string[] },
  transaction?: Transaction | string
): Promise<ApiResponse<OntologyItem[]>>
```

```typescript
// List all labels with counts
const { data: ontology } = await db.ai.getOntology();
for (const item of ontology) {
  console.log(`${item.label}: ${item.count} records`);
}

// Get property ID for value enumeration
const {
  data: [bookSchema],
} = await db.ai.getOntology({ labels: ["Book"] });
const genreProp = bookSchema.properties.find((p) => p.name === "genre");
const { data: genres } = await db.properties.values({ id: genreProp.id });
```

```typescript
type OntologyItem = {
  label: string;
  count: number;
  properties: OntologyProperty[];
  relationships: OntologyRelationship[];
};

type OntologyProperty = {
  id: string; // use with db.properties.values()
  name: string;
  type: string; // 'string' | 'number' | 'boolean' | 'datetime'
  values?: Array<string | number>; // up to 10 samples (string/boolean only)
  min?: number | string; // number/datetime only
  max?: number | string;
};

type OntologyRelationship = {
  label: string;
  type: string;
  direction: "in" | "out";
};
```

:::note Caching
Both methods share a **1-hour cache** per project. The first call after TTL expiry triggers a full graph scan; all subsequent calls within the hour are instant.
:::

:::tip Agent quickstart
Call `db.ai.getOntologyMarkdown()` first in every AI session. Without it, models will hallucinate field and label names.
:::
