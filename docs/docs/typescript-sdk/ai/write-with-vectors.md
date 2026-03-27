---
sidebar_position: 4
title: Writing Records with Vectors
---

# Writing Records with Vectors

RushDB lets you attach pre-computed embedding vectors to records **at write time**, eliminating the need for a separate `upsertVectors` call. Any operation that creates or modifies records supports this through the `vectors` parameter (or the `$vectors` key in batch imports).

This feature requires at least one [external index](./advanced-indexing.md) to exist for the target `(label, propertyName)`.

---

## `vectors` parameter

All write operations accept a `vectors` array:

```typescript
type VectorEntry = {
  /** Property name this vector is associated with. */
  propertyName: string
  /** Pre-computed embedding vector. */
  vector: number[]
  /** Required when multiple indexes exist on the same property. */
  similarityFunction?: 'cosine' | 'euclidean'
}
```

---

## `records.create()` with vectors

```typescript
const { data: record } = await db.records.create({
  label: 'Article',
  data: {
    title: 'How transformers work',
    body: 'Attention is all you need ...',
  },
  vectors: [
    { propertyName: 'body', vector: myEmbed('Attention is all you need ...') }
  ],
})

console.log(record.__id) // record is created AND vector is written atomically
```

---

## `records.upsert()` with vectors

`upsert` is idempotent on the record's slug (natural key). Passing `vectors` writes (or replaces) the stored vector for each `propertyName` in the same call:

```typescript
// First call — creates the record + writes vector
const { data: r1 } = await db.records.upsert({
  label: 'Article',
  data: { slug: 'transformers-101', title: 'Transformers 101', body: '...' },
  vectors: [{ propertyName: 'body', vector: v1 }],
})

// Second call — same slug → updates the title/body + replaces the vector
const { data: r2 } = await db.records.upsert({
  label: 'Article',
  data: { slug: 'transformers-101', title: 'Transformers 101 (revised)', body: 'Updated ...' },
  vectors: [{ propertyName: 'body', vector: v2 }],
})

console.log(r1.__id === r2.__id) // true — same record
```

---

## `records.set()` with vectors

`set` replaces all properties of a record with new values. Including `vectors` writes those vectors at the same time:

```typescript
// Find or create the record first
const { data: rec } = await db.records.create({
  label: 'Product',
  data: { name: 'Widget', price: 9.99 },
})

// Full replace — data AND vector updated together
await db.records.set(rec.__id, {
  data: { name: 'Widget Pro', price: 19.99 },
  vectors: [{ propertyName: 'description', vector: newVec }],
})
```

---

## `records.importJson()` with `$vectors`

For bulk ingestion, add a `$vectors` key alongside properties in each JSON object. The format is the same as the `VectorEntry` array:

```typescript
await db.records.importJson({
  "Article": [
    {
      title: "Alpha",
      body: "First article about AI",
      "$vectors": [{ propertyName: "body", vector: [1, 0, 0] }]
    },
    {
      title: "Beta",
      body: "Second article about ML",
      "$vectors": [{ propertyName: "body", vector: [0, 1, 0] }]
    },
    {
      title: "Gamma",
      body: "Third article about DL",
      "$vectors": [{ propertyName: "body", vector: [0, 0, 1] }]
    },
  ]
})
```

Important: `$vectors` entries are stripped before the record is persisted. They:
- **Do not** appear as record properties
- **Do not** create child records
- **Do not** appear in query results

---

## `records.createMany()` with vectors

`createMany` is optimised for flat (CSV-like) rows. Use the top-level `vectors` parameter — an array indexed by row position — to attach a vector to each record without nesting arrays inside your flat data:

```typescript
await db.records.createMany({
  label: 'Product',
  data: [
    { name: 'Alpha', description: 'First product' },
    { name: 'Beta',  description: 'Second product' },
    { name: 'Gamma', description: 'Third product' },
  ],
  vectors: [
    [{ propertyName: 'description', vector: [1, 0, 0] }], // row 0
    [{ propertyName: 'description', vector: [0, 1, 0] }], // row 1
    [{ propertyName: 'description', vector: [0, 0, 1] }], // row 2
  ],
  options: { returnResult: true },
})
```

### Sparse vectors

Leave rows without vectors by providing a shorter `vectors` array (any unspecified trailing rows are skipped):

```typescript
await db.records.createMany({
  label: 'Product',
  data: [{ name: 'Alpha' }, { name: 'Beta' }, { name: 'Gamma' }],
  // only row 0 gets a vector; rows 1 and 2 are skipped
  vectors: [[{ propertyName: 'description', vector: myVec }]],
})
```

### Validation

The SDK throws synchronously if `vectors.length > data.length`:

```typescript
// ❌ Throws: "vectors length (3) exceeds the number of data rows (2)"
db.records.createMany({
  label: 'Product',
  data: [{ name: 'A' }, { name: 'B' }],
  vectors: [
    [{ propertyName: 'description', vector: [1, 0, 0] }],
    [{ propertyName: 'description', vector: [0, 1, 0] }],
    [{ propertyName: 'description', vector: [0, 0, 1] }], // no row 2
  ],
})
```

---

## `records.importCsv()` with vectors

CSV data is a raw string, so per-row vectors are supplied as a separate `vectors` parameter using the same indexed-array format as `createMany`. Row indices are 0-based and refer to data rows after the header is consumed.

```typescript
const csv = `name,description
Alpha,First product
Beta,Second product
Gamma,Third product`

await db.records.importCsv({
  label: 'Product',
  data: csv,
  vectors: [
    [{ propertyName: 'description', vector: [1, 0, 0] }], // csv row 0
    [{ propertyName: 'description', vector: [0, 1, 0] }], // csv row 1
    [{ propertyName: 'description', vector: [0, 0, 1] }], // csv row 2
  ],
  options: { returnResult: true },
})
```

### Sparse vectors

Same sparse pattern as `createMany` — any rows beyond `vectors.length` get no vector:

```typescript
await db.records.importCsv({
  label: 'Product',
  data: csv,
  // only the first row gets a vector
  vectors: [[{ propertyName: 'description', vector: myVec }]],
})
```

### Validation

The server returns `400 Bad Request` if `vectors.length` exceeds the number of data rows (validated after CSV parsing). The client does not know the row count before sending since CSV is a raw string.

```
400 Bad Request: vectors length (5) exceeds the number of CSV data rows (3)
```

---

## Specifying `similarityFunction` for disambiguation

When a single `(label, propertyName)` has multiple external indexes registered (e.g. one cosine and one euclidean), you must include `similarityFunction` in each `VectorEntry` so the server can route the write to the correct index:

```typescript
// Write to the cosine index
await db.records.create({
  label: 'Product',
  data: { name: 'Widget' },
  vectors: [
    { propertyName: 'embedding', vector: vec, similarityFunction: 'cosine' }
  ],
})
```

Omitting `similarityFunction` when multiple indexes match returns `422 Unprocessable Entity`.

---

## Multiple vectors in one call

You can write vectors for multiple properties or indexes in a single operation:

```typescript
await db.records.create({
  label: 'Document',
  data: { title: 'Multi-modal doc', abstract: '...', fullText: '...' },
  vectors: [
    { propertyName: 'abstract',  vector: abstractVec  },
    { propertyName: 'fullText',  vector: fullTextVec  },
  ],
})
```

Each entry is matched independently against the available external indexes.

---

## Complete worked example

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db  = new RushDB('your-api-key')
const emb = new YourEmbeddingModel()

// 1. Create an external index once (idempotent via 409 Conflict)
const { data: idx } = await db.ai.indexes.create({
  label: 'Article',
  propertyName: 'body',
  external: true,
  dimensions: 768,
  similarityFunction: 'cosine',
}).catch(e => e.status === 409 ? db.ai.indexes.find() : Promise.reject(e))

// 2. Create records from your pipeline, embedding as you go
const docs = [
  { title: 'Alpha', body: 'First doc' },
  { title: 'Beta',  body: 'Second doc' },
]

for (const doc of docs) {
  await db.records.create({
    label: 'Article',
    data: doc,
    vectors: [{ propertyName: 'body', vector: await emb.embed(doc.body) }],
  })
}

// 3. Search
const queryVec = await emb.embed('first document')
const { data } = await db.ai.search({
  label: 'Article',
  propertyName: 'body',
  queryVector: queryVec,
  limit: 3,
})
console.log(data[0].title)  // 'Alpha'
```

---

## Inline vectors vs. `upsertVectors`

| | Inline `vectors` | `db.ai.indexes.upsertVectors()` |
|---|---|---|
| **Round trips** | 1 (write + vector together) | 2+ (write, then upload) |
| **Use case** | Streaming ingestion, real-time pipeline | Batch backfill, dataset migration |
| **Idempotency** | Depends on the write operation used | Always idempotent per `recordId` |
| **Availability** | `create`, `upsert`, `set`, `createMany`, `importCsv`, `importJson` | Standalone call on any existing records |
| **Multi-record** | `createMany` or `importCsv` with indexed `vectors[][]`, `importJson` with `$vectors` per item | Single bulk payload |

For streaming pipelines that produce records one-by-one or in small batches, inline vectors are simpler and more efficient. For seeding an index from a large existing dataset, `upsertVectors` is the right choice.

---

## Per-row vs. per-item vector formats

| Method | Vector syntax | Notes |
|---|---|---|
| `create` | `vectors: VectorEntry[]` | single record |
| `upsert` | `vectors: VectorEntry[]` | single record, idempotent |
| `set` | `vectors: VectorEntry[]` | single record, full replace |
| `importJson` | `"$vectors": VectorEntry[]` inside each item | nested in data object |
| `createMany` | `vectors: VectorEntry[][]` (indexed) | `vectors[i]` → `data[i]` |
| `importCsv` | `vectors: VectorEntry[][]` (indexed) | `vectors[i]` → CSV row `i` |

`importJson` uses the `$vectors` in-item style because JSON items can themselves be nested objects with their own structure. `createMany` and `importCsv` use the external indexed array style because the data they carry is flat — no room for nested arrays inside a flat record.

---

## Error conditions

| Error | Cause | Method |
|---|---|---|
| `404 Not Found` | No external index exists for `(label, propertyName)` | all |
| `422 Unprocessable Entity` | `vector.length` does not match `index.dimensions` | all |
| `422 Unprocessable Entity` | Multiple indexes match and `similarityFunction` was not specified | all |
| `400 Bad Request` | `vectors.length` exceeds number of CSV data rows | `importCsv` |
| Client `Error` | `vectors.length` exceeds `data.length` | `createMany` (thrown synchronously) |
