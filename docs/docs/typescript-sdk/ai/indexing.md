---
sidebar_position: 1
title: Embedding Indexes
---

# Embedding Indexes

An **embedding index** is a policy that tells RushDB to vectorize a specific string property for a label. Once `status` is `ready`, every record matching that label+property pair is searchable via `db.ai.search()`.

---

## How indexes work

Indexes are scoped to `(label, propertyName)`. "Book:description" and "Article:description" are completely independent — they maintain separate vector stores and never interfere.

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

## List Indexes

`db.ai.indexes.find()`

List all embedding index policies for the current project.

```typescript
const { data: indexes } = await db.ai.indexes.find();
/*
[
  {
    id: "01jb...",
    label: "Book",
    propertyName: "description",
    sourceType: "managed",
    similarityFunction: "cosine",
    dimensions: 1536,
    status: "ready",
    modelKey: "text-embedding-3-small",
    ...
  }
]
*/
```

---

## Create Index

`db.ai.indexes.create()`

Create a new managed embedding index for a string property.

```typescript
db.ai.indexes.create(params: {
  label: string
  propertyName: string
  sourceType?: 'managed' | 'external'
  similarityFunction?: 'cosine' | 'euclidean'  // default: 'cosine'
  dimensions?: number                           // default: server RUSHDB_EMBEDDING_DIMENSIONS
}): Promise<ApiResponse<EmbeddingIndex>>
```

```typescript
// Simplest form — uses server-configured model and dimensions
const { data: index } = await db.ai.indexes.create({
  label: "Book",
  propertyName: "description",
});

console.log(index.status); // 'pending' → backfill starts immediately
```

```typescript
// With explicit parameters
const { data: index } = await db.ai.indexes.create({
  label: "Article",
  propertyName: "body",
  similarityFunction: "cosine",
  dimensions: 1536,
});
```

> Attempting to create a duplicate `(label, propertyName, sourceType, similarityFunction, dimensions)` tuple returns `409 Conflict`.

### Index lifecycle

| Status             | Description                                            |
| ------------------ | ------------------------------------------------------ |
| `pending`          | Policy created, waiting for backfill scheduler         |
| `indexing`         | Backfill in progress                                   |
| `awaiting_vectors` | External index — waiting for client to push vectors    |
| `ready`            | All existing records have vectors, search is available |
| `error`            | Backfill failed; check server logs for the cause       |

---

## Get Index Stats

`db.ai.indexes.stats(id)`

Returns the fill rate for an index — useful for progress monitoring or health checks.

```typescript
db.ai.indexes.stats(id: string): Promise<ApiResponse<EmbeddingIndexStats>>
```

```typescript
const { data: stats } = await db.ai.indexes.stats(index.id);
console.log(`${stats.indexedRecords} / ${stats.totalRecords} records indexed`);
```

```typescript
type EmbeddingIndexStats = {
  totalRecords: number;
  indexedRecords: number;
};
```

---

## Delete Index

`db.ai.indexes.delete(id)`

Remove an embedding index policy and its scoped vector data.

```typescript
await db.ai.indexes.delete(index.id);
```

The underlying Neo4j DDL vector index is only dropped when **zero embeddings remain** across the entire project. This avoids unnecessary index rebuilds when multiple policies share the same `(dimensions, similarityFunction)` combination.

---

## Response type

```typescript
type EmbeddingIndex = {
  id: string;
  projectId: string;
  /** Neo4j label this index is scoped to (e.g. "Book"). */
  label: string;
  propertyName: string;
  modelKey: string;
  sourceType: "managed" | "external";
  similarityFunction: "cosine" | "euclidean";
  dimensions: number;
  vectorPropertyName: string; // internal Neo4j property name for the vector
  enabled: boolean;
  status: string;
  createdAt: string;
  updatedAt: string;
};
```

---

## Waiting for an index to become ready

For managed indexes, backfill runs asynchronously. Poll `db.ai.indexes.find()` until status is `ready`:

```typescript
async function waitForIndexReady(
  db: RushDB,
  indexId: string,
  timeoutMs = 90_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const { data: indexes } = await db.ai.indexes.find();
    const idx = indexes.find((i) => i.id === indexId);
    if (idx?.status === "ready") return;
    if (idx?.status === "error") throw new Error("Index entered error state");
    await new Promise((r) => setTimeout(r, 3_000));
  }
  throw new Error("Index did not become ready in time");
}

const { data: index } = await db.ai.indexes.create({
  label: "Book",
  propertyName: "description",
});
await waitForIndexReady(db, index.id);
// now safe to call db.ai.search(...)
```

---

## Multiple indexes on the same property

You can have more than one index per `(label, propertyName)` pair, provided the signature differs:

```typescript
// Same label + property, different similarity function
await db.ai.indexes.create({
  label: "Product",
  propertyName: "description",
  similarityFunction: "cosine",
  dimensions: 768,
});

await db.ai.indexes.create({
  label: "Product",
  propertyName: "description",
  similarityFunction: "euclidean",
  dimensions: 768,
});
```

When performing a search or writing inline vectors against a property with multiple indexes, specify `similarityFunction` to disambiguate. See [Advanced Indexing — BYOV](./advanced-indexing.md#disambiguation) for details.

---

## String Array Properties

`List<String>`

String array properties are supported. Each item in the array is embedded individually, then mean-pooled into a single vector stored on the relationship.
