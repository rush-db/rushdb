---
sidebar_position: 3
title: Semantic Search
---

# Semantic Search

`db.ai.search()` performs semantic vector search across records that have an associated embedding index.

---

## Signature

```typescript
db.ai.search(params: {
  /** Neo4j label to search within (e.g. "Book"). */
  label: string | string[]

  /** Property the target embedding index is scoped to. */
  propertyName: string

  /** Natural-language query — used by managed indexes. */
  query?: string

  /** Pre-computed query vector — used by external indexes. */
  queryVector?: number[]

  /** Override the similarity function when multiple indexes match. */
  similarityFunction?: 'cosine' | 'euclidean'

  /** Override dimensions when multiple indexes match. */
  dimensions?: number

  /** Prefilter: only return records that also satisfy this where clause. */
  where?: WhereClause

  /** Maximum results to return. */
  limit?: number

  /** Results to skip (for pagination). */
  skip?: number
}): Promise<ApiResponse<SemanticSearchResult[]>>
```

---

## Result type

```typescript
type SemanticSearchResult = {
  /** Similarity score: higher is better. */
  __score: number
  /** Record ID. */
  __id: string
  /** Record label. */
  __label: string
  /** All record properties are returned at the top level. */
  [key: string]: unknown
}
```

Results are always ordered by `__score` descending — closest match first.

---

## Managed search (query text)

For a **managed** index, pass `query` (a natural-language string). The server embeds it using the same model that was used when building the index, then ranks the prefiltered candidates by similarity.

```typescript
const { data: results } = await db.ai.search({
  label: 'Book',
  propertyName: 'description',
  query: 'space exploration and interstellar travel',
  limit: 5,
})

results.forEach(r => {
  console.log(`[${r.__score.toFixed(4)}] ${r.title}`)
})
```

---

## External search (query vector)

For an **external** index, pass `queryVector` — a pre-computed embedding produced by your own model. No text is sent to the server.

```typescript
const myEmbedder = new MyEmbeddingModel()
const vec = await myEmbedder.embed('space exploration')

const { data: results } = await db.ai.search({
  label: 'Article',
  propertyName: 'body',
  queryVector: vec,
  limit: 10,
})
```

- `query` is **not allowed** with external indexes — the server has no model to embed it.
- `queryVector` is **not required** for managed indexes but is accepted (bypasses server embedding).

### Dimension inference

When `queryVector` is supplied you can omit `dimensions` — the server infers it from `queryVector.length`:

```typescript
// dimensions is optional when queryVector is given
await db.ai.search({
  label: 'Product',
  propertyName: 'embedding',
  queryVector: [0.1, 0.9, 0.4],   // length 3 → dimensions inferred as 3
})
```

---

## Filtering with `where`

The `where` clause acts as a **prefilter** — only records that satisfy the filter are candidates for similarity ranking. RushDB already scopes search to the current project, and `where` adds your application-level constraints before scoring.

```typescript
const { data: results } = await db.ai.search({
  label: 'Product',
  propertyName: 'description',
  query: 'wireless headphones',
  where: {
    category: { $eq: 'electronics' },
    inStock: { $eq: true },
    price: { $lt: 100 },
  },
  limit: 20,
})
```

All `WhereClause` operators supported by `db.records.find()` are available here.

---

## Multi-label search

Pass an array of labels to search across multiple entity types simultaneously:

```typescript
const { data: results } = await db.ai.search({
  label: ['Article', 'Post', 'Comment'],
  propertyName: 'body',
  query: 'machine learning trends',
  limit: 10,
})

// Each result carries __label so you can tell them apart
results.forEach(r => console.log(r.__label, r.__score, r.title ?? r.text))
```

All listed labels must have an embedding index on the same `propertyName`, or the request will return `404` for the missing labels.

---

## Disambiguation

When two indexes exist for the same `(label, propertyName)`, you must specify `similarityFunction` (and optionally `dimensions`) to select the target index:

```typescript
// Two indexes: Product:embedding/cosine and Product:embedding/euclidean
await db.ai.search({
  label: 'Product',
  propertyName: 'embedding',
  queryVector: vec,
  similarityFunction: 'cosine',   // required — otherwise 422 Unprocessable Entity
})
```

---

## Pagination

```typescript
const PAGE = 20

// Page 1
const { data: page1 } = await db.ai.search({ ..., limit: PAGE, skip: 0 })
// Page 2
const { data: page2 } = await db.ai.search({ ..., limit: PAGE, skip: PAGE })
```

---

## Error reference

| HTTP | Cause |
|---|---|
| `404 Not Found` | No enabled embedding index found for `(label, propertyName)` |
| `422 Unprocessable Entity` | Multiple indexes match and `similarityFunction` was not specified |
| `422 Unprocessable Entity` | `query` text supplied for an external index (server cannot embed it) |
| `422 Unprocessable Entity` | Vector length does not match index `dimensions` |
| `503 Service Unavailable` | Embedding model unavailable (managed indexes only) |
