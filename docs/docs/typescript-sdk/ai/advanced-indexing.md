---
sidebar_position: 2
title: Advanced Indexing — BYOV
---

# Advanced Indexing — Bring Your Own Vectors

**External indexes** (BYOV — Bring Your Own Vectors) let you supply pre-computed embedding vectors instead of having the server compute them. Use them when you need:

- A custom or private model the server cannot access
- Multimodal embeddings (image, audio, document structure)
- Vectors already produced by your ML pipeline
- Reproducible embeddings not tied to the server's active model

---

## Creating an external index

Pass `external: true` (shorthand) **or** `sourceType: 'external'` (explicit). Both are equivalent:

```typescript
// ── shorthand ────────────────────────────────────────────────
const { data: extIndex } = await db.ai.indexes.create({
  label: 'Article',
  propertyName: 'body',
  external: true,
  dimensions: 768,
  similarityFunction: 'cosine',
})
// extIndex.sourceType === 'external'
// extIndex.status    === 'awaiting_vectors'

// ── explicit ─────────────────────────────────────────────────
const { data: extIndex } = await db.ai.indexes.create({
  label: 'Article',
  propertyName: 'body',
  sourceType: 'external',
  dimensions: 768,
  similarityFunction: 'cosine',
})
```

An external index starts with status `awaiting_vectors` and transitions to `ready` once at least one vector has been written.

> Because the server never calls an embedding model , `dimensions` is **required** for external indexes.

### External vs managed comparison

| | Managed | External |
|---|---|---|
| `sourceType` | `'managed'` | `'external'` |
| Initial status | `'pending'` | `'awaiting_vectors'` |
| Who computes embeddings | RushDB server (via configured model) | Your application |
| `dimensions` | Optional (uses server default) | **Required** |
| Backfill for existing records | Automatic | Manual via `upsertVectors` or inline writes |

---

## Pushing vectors with `upsertVectors`

`db.ai.indexes.upsertVectors()` is the bulk upload API — ideal for seeding an index from a dataset or syncing after a batch pipeline.

```typescript
db.ai.indexes.upsertVectors(
  indexId: string,
  payload: { items: Array<{ recordId: string; vector: number[] }> }
): Promise<ApiResponse<void>>
```

```typescript
const { data: records } = await db.records.find(
  { where: { __label: 'Article' } }
)

const myEmbedder = new MyEmbeddingModel()
const items = await Promise.all(
  records.map(async record => ({
    recordId: record.__id,
    vector: await myEmbedder.embed(record.body)
  }))
)

await db.ai.indexes.upsertVectors(extIndex.id, { items })
```

The request is idempotent — calling it again with the same `recordId` **replaces** the stored vector.

---

## Writing vectors at record creation time

Instead of a two-step create → upsertVectors flow, you can write vectors inline using the `vectors` parameter on any write operation. The server resolves the correct external index automatically.

See [Write Operations with Vectors](./write-with-vectors.md) for the full reference.

```typescript
// One-step: create record AND write its vector
const { data: record } = await db.records.create({
  label: 'Article',
  data: { title: 'Warp drives', body: 'Alcubierre metric...' },
  vectors: [{ propertyName: 'body', vector: myVec }]
})
```

---

## Disambiguation {#disambiguation}

When the same `(label, propertyName)` pair is covered by more than one external index (different `similarityFunction` or `dimensions`), RushDB cannot determine which index to use without extra information.

Specify `similarityFunction` to resolve the ambiguity:

```typescript
// Two indexes on Product:embedding — cosine and euclidean
await db.ai.indexes.create({
  label: 'Product', propertyName: 'embedding', external: true,
  similarityFunction: 'cosine',   dimensions: 768,
})
await db.ai.indexes.create({
  label: 'Product', propertyName: 'embedding', external: true,
  similarityFunction: 'euclidean', dimensions: 768,
})

// ✅ explicit — writes to the cosine index only
await db.records.create({
  label: 'Product',
  data: { name: 'Widget' },
  vectors: [{
    propertyName: 'embedding',
    vector: vec,
    similarityFunction: 'cosine',   // <-- required when ambiguous
  }]
})

// ✅ explicit — searches the euclidean index only
await db.ai.search({
  label: 'Product',
  propertyName: 'embedding',
  queryVector: vec,
  similarityFunction: 'euclidean',  // <-- required when ambiguous
})

// ❌ omitting similarityFunction when two indexes exist → 422 Unprocessable Entity
await db.records.create({
  label: 'Product',
  data: { name: 'Gadget' },
  vectors: [{ propertyName: 'embedding', vector: vec }],
})
```

### Index signature uniqueness

Two index policies are considered **identical** (and a second `create` returns `409 Conflict`) when all five fields match:

| Field | Effect on uniqueness |
|---|---|
| `label` | ✅ |
| `propertyName` | ✅ |
| `sourceType` | ✅ |
| `similarityFunction` | ✅ |
| `dimensions` | ✅ |

Changing any one field produces a distinct index and both are allowed to coexist.

---

## Complete BYOV worked example

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('your-api-key')

// 1. Create the external index
const { data: idx } = await db.ai.indexes.create({
  label: 'Doc',
  propertyName: 'content',
  external: true,
  dimensions: 3,
  similarityFunction: 'cosine',
})

// 2. Create records + write inline vectors (one round trip per record)
const articles = [
  { title: 'Alpha', content: 'First article',  vector: [1, 0, 0] },
  { title: 'Beta',  content: 'Second article', vector: [0, 1, 0] },
  { title: 'Gamma', content: 'Third article',  vector: [0, 0, 1] },
]

for (const { title, content, vector } of articles) {
  await db.records.create({
    label: 'Doc',
    data: { title, content },
    vectors: [{ propertyName: 'content', vector }],
  })
}

// 3. Search using a pre-computed query vector
const { data: results } = await db.ai.search({
  label: 'Doc',
  propertyName: 'content',
  queryVector: [1, 0, 0],   // closest to Alpha
  limit: 3,
})

console.log(results[0].title)   // 'Alpha'
console.log(results[0].__score) // ~1.0
```

---

## Batch import with `$vectors`

For bulk seeding, use `records.importJson()` with a `$vectors` key on each object:

```typescript
await db.records.importJson({
  "Doc": [
    { title: "Alpha", content: "First article",  "$vectors": [{ propertyName: "content", vector: [1, 0, 0] }] },
    { title: "Beta",  content: "Second article", "$vectors": [{ propertyName: "content", vector: [0, 1, 0] }] },
    { title: "Gamma", content: "Third article",  "$vectors": [{ propertyName: "content", vector: [0, 0, 1] }] },
  ]
})
```

`$vectors` entries are **stripped** from the stored record data — they only drive the vector write and do **not** appear as child records or extra properties.

---

## Mixing managed and external indexes

You can have both a managed index and an external index on the same property simultaneously:

```typescript
// Managed — server embeds for full-text search
await db.ai.indexes.create({ label: 'Product', propertyName: 'description' })

// External — your custom multimodal model
await db.ai.indexes.create({
  label: 'Product', propertyName: 'description',
  external: true, dimensions: 512, similarityFunction: 'cosine',
})
```

Specifying `similarityFunction` in `db.ai.search()` routes the query to the intended index.
