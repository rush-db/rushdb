---
sidebar_position: 6
---

# Update Records

Two methods for updating records: `update()` patches fields, `set()` replaces them all.

## `db.records.update()`

Partial update — only the specified fields change; all other fields are preserved.

```typescript
await db.records.update({
  target: 'movie-id-123',
  label: 'MOVIE',
  data: { rating: 9.1 }
})
// → DBRecordInstance  (title, genre, etc. unchanged)
```

## `db.records.set()`

Full replace — all fields not in `data` are removed.

```typescript
await db.records.set({
  target: 'movie-id-123',
  label: 'MOVIE',
  data: { title: 'Inception', rating: 9.1, genre: 'sci-fi' }
})
// → DBRecordInstance  (only these three fields remain)
```

### Parameters (both methods)

| Parameter | Description |
|---|---|
| `target` | Record ID string, record instance, or record object |
| `label` | Label for the record |
| `data` | Flat object or `PropertyDraft[]` for precise type control |
| `options` | `suggestTypes`, `convertNumericValuesToNumbers` |
| `transaction` | Optional `Transaction` or ID string |

### PropertyDraft approach

```typescript
await db.records.update({
  target: 'movie-id-123',
  label: 'MOVIE',
  data: [
    { name: 'rating',     type: 'number',   value: 9.1 },
    { name: 'releasedAt', type: 'datetime', value: '2010-07-16T00:00:00Z' }
  ]
})
```

## In a transaction

```typescript
const tx = await db.tx.begin()
try {
  await db.records.update({ target: 'movie-id-123', label: 'MOVIE', data: { rating: 9.1 } }, tx)
  await db.records.update({ target: 'actor-id-456', label: 'ACTOR', data: { country: 'USA' } }, tx)
  await tx.commit()
} catch (e) {
  await tx.rollback(); throw e
}
```

## Via Model

```typescript
const MovieModel = new Model('MOVIE', { title: { type: 'string' }, rating: { type: 'number' } })

// Partial update
await MovieModel.update('movie-id-123', { rating: 9.1 })

// Full replace
await MovieModel.set('movie-id-123', { title: 'Inception', rating: 9.1, genre: 'sci-fi' })
```

