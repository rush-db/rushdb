---
sidebar_position: 2
---

# AI & Semantic Search

The `db.ai` namespace covers three things: **graph ontology** (schema discovery for LLM agents), **embedding indexes** (per-label vector policies), and **semantic search** (cosine-similarity queries over indexed properties).

:::tip Agent quickstart
Call `db.ai.getOntologyMarkdown()` first in every AI session — it returns all label names, field names, value ranges, and the relationship map in a single token-efficient Markdown string. Without it, the model will hallucinate label and field names.
:::

---

## `db.ai.getOntologyMarkdown()`

Returns the full schema as compact Markdown — the **recommended format for LLM context injection**.

```typescript
db.ai.getOntologyMarkdown(
  params?: { labels?: string[] },
  transaction?: Transaction | string
): Promise<ApiResponse<string>>
```

```typescript
// Inject into LLM at session start
const { data: schema } = await db.ai.getOntologyMarkdown()
const messages = [
  { role: 'system', content: `You are a data assistant.\n\n${schema}` },
  { role: 'user',   content: 'How many paid orders are there?' }
]

// Scope to specific labels
const { data: orderSchema } = await db.ai.getOntologyMarkdown({ labels: ['Order'] })
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

## `db.ai.getOntology()`

Returns the same ontology as a structured JSON array — useful for schema UIs, auto-complete, or looking up property IDs for `db.properties.values()`.

```typescript
db.ai.getOntology(
  params?: { labels?: string[] },
  transaction?: Transaction | string
): Promise<ApiResponse<OntologyItem[]>>
```

```typescript
// List all labels with counts
const { data: ontology } = await db.ai.getOntology()
for (const item of ontology) {
  console.log(`${item.label}: ${item.count} records`)
}

// Get property ID for value enumeration
const { data: [orderSchema] } = await db.ai.getOntology({ labels: ['Order'] })
const statusProp = orderSchema.properties.find(p => p.name === 'status')
const { data: values } = await db.properties.values({ id: statusProp.id })
// ['pending', 'paid', 'shipped', 'cancelled', 'refunded']
```

```typescript
type OntologyItem = {
  label: string
  count: number
  properties: OntologyProperty[]
  relationships: OntologyRelationship[]
}

type OntologyProperty = {
  id: string                       // use with db.properties.values()
  name: string
  type: string                     // 'string' | 'number' | 'boolean' | 'datetime'
  values?: Array<string | number>  // up to 10 samples (string/boolean only)
  min?: number | string            // number/datetime only
  max?: number | string
}

type OntologyRelationship = {
  label: string
  type: string
  direction: 'in' | 'out'
}
```

:::note Caching
Both methods share a **1-hour cache** on the ProjectNode. The first call after TTL expiry triggers a full graph scan; all subsequent calls within the hour are instant.
:::

---

## Embedding Indexes

An embedding index policy tells RushDB to vectorize a specific string property, scoped to a label. Once `status` is `ready`, that property is searchable via `db.ai.search()`.

`List<String>` properties are supported — each item is embedded and mean-pooled into one vector.

> **Model config is server-side** — the embedding model and dimensions come from `RUSHDB_EMBEDDING_MODEL` / `RUSHDB_EMBEDDING_DIMENSIONS` env vars.

### `db.ai.indexes.find()`

```typescript
const { data: indexes } = await db.ai.indexes.find()
// [{ id, label, propertyName, status, dimensions, modelKey, ... }]
```

### `db.ai.indexes.create()`

```typescript
db.ai.indexes.create(params: {
  label: string       // Neo4j label to scope this index to (e.g. 'Article', 'Product')
  propertyName: string
}): Promise<ApiResponse<EmbeddingIndex>>
```

```typescript
const { data: index } = await db.ai.indexes.create({
  label: 'Article',
  propertyName: 'description'
})
console.log(index.status) // 'pending' → backfill starts immediately
```

> Duplicate `(label, propertyName)` pairs return `409 Conflict`.

### `db.ai.indexes.stats(id)`

```typescript
const { data: stats } = await db.ai.indexes.stats(index.id)
console.log(`${stats.indexedRecords} / ${stats.totalRecords} embedded`)
```

### `db.ai.indexes.delete(id)`

```typescript
await db.ai.indexes.delete(index.id)
// Strips policy + scoped embeddings. Global Neo4j DDL index only dropped when zero embeddings remain project-wide.
```

### Response type

```typescript
type EmbeddingIndex = {
  id: string
  projectId: string
  label: string       // Neo4j label this index is scoped to
  propertyName: string
  modelKey: string
  dimensions: number
  enabled: boolean
  status: string      // 'pending' | 'indexing' | 'ready' | 'error'
  createdAt: string
  updatedAt: string
}

type EmbeddingIndexStats = {
  totalRecords: number
  indexedRecords: number
}
```

---

## `db.ai.search()`

Embeds the query text and returns relevant records by cosine similarity. Requires a `ready` embedding index on `propertyName` scoped to the target `label`.

**Execution mode is automatic:**
- **ANN** (fast): single label in `labels`, no `where` → queries the global vector index
- **ENN prefilter** (exact): `where` present or 2+ labels → MATCH/WHERE first, then `vector.similarity.cosine()`

```typescript
db.ai.search(params: {
  propertyName: string
  query: string
  labels: string[]   // required, min 1
  where?: object
  topK?: number      // ANN candidate count (default 20)
  skip?: number
  limit?: number
}): Promise<ApiResponse<SemanticSearchResult[]>>
```

```typescript
// ANN — single label, no where
const { data: results } = await db.ai.search({
  propertyName: 'description',
  query: 'machine learning for beginners',
  labels: ['Article'],
  limit: 5
})

for (const result of results) {
  console.log(`[${result.__score.toFixed(3)}] ${result.title}`)
}

// ENN prefilter — with where clause
const { data: filtered } = await db.ai.search({
  propertyName: 'description',
  query: 'sustainable packaging',
  labels: ['Product'],
  where: { status: { $in: ['active', 'featured'] } },
  limit: 10
})
```

### Response type

```typescript
// SemanticSearchResult is a flat DBRecord with __score injected
type SemanticSearchResult<S extends Schema = Schema> = DBRecord<S> & {
  readonly __score: number  // cosine similarity score, 0–1 (higher = more similar)
}

// Access fields directly — no .record unwrap needed
result.__id       // RushDB record ID
result.__label    // Neo4j label
result.__score    // cosine similarity
result.title      // your field
```
