---
sidebar_position: 7
---

# Models

A `Model` binds a label to a schema, giving you typed access to all record operations without ever repeating the label name.

## Define a model

```typescript
import RushDB, { Model } from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

const MovieModel = new Model('MOVIE', {
  title:     { type: 'string' },
  rating:    { type: 'number' },
  genre:     { type: 'string' },
  releasedAt:{ type: 'datetime', default: () => new Date().toISOString() }
})
```

### Schema field types

| Type | Notes |
|---|---|
| `boolean` | |
| `datetime` | ISO string or detailed object |
| `null` | |
| `number` | |
| `string` | |

### Schema field options

| Option | Description |
|---|---|
| `type` | **Required.** Field data type |
| `default` | Static value or `() => value` function (sync or async) |
| `multiple` | `true` → field holds an array |
| `required` | `true` → create throws if value missing |
| `unique` | `true` → value must be unique across all records of this label |

## Types from the model

```typescript
export const USER = 'USER' as const

export const UserModel = new Model(USER, {
  name:      { type: 'string' },
  login:     { type: 'string', unique: true },
  password:  { type: 'string' },
  active:    { type: 'boolean', default: true },
  createdAt: { type: 'datetime', default: () => new Date().toISOString() },
  tags:      { type: 'string', multiple: true, required: false },
})

// Export strongly-typed aliases
export type UserRecord          = typeof UserModel.record
export type UserRecordDraft     = typeof UserModel.draft
export type UserSearchQuery     = SearchQuery<typeof UserModel.schema>
```

| Helper | What it represents |
|---|---|
| `.record` | Full DB record including system fields (`__id`, `__label`, `__proptypes`) |
| `.draft` | Your schema fields only — no system fields; use when creating records |
| `.recordInstance` | Record + instance methods (`update`, `delete`, `attach`, …) |
| `.recordsArrayInstance` | Array result with `data` + `total` |

## CRUD operations

### Create

```typescript
const movie = await MovieModel.create({
  title: 'Inception', rating: 8.8, genre: 'sci-fi'
})

const movies = await MovieModel.createMany([
  { title: 'The Dark Knight', rating: 9.0, genre: 'action' },
  { title: 'Interstellar',    rating: 8.6, genre: 'sci-fi' }
])
```

### Read

```typescript
const all        = await MovieModel.find()
const sciFi      = await MovieModel.find({ where: { genre: 'sci-fi' } })
const one        = await MovieModel.findOne({ where: { title: 'Inception' } })
const byId       = await MovieModel.findById('movie-id-123')
const unique     = await MovieModel.findUniq({ where: { title: 'Inception' } })
```

### Update

```typescript
// Partial update — only listed fields change
await MovieModel.update('movie-id-123', { rating: 9.1 })

// Full replace — all other fields are removed
await MovieModel.set('movie-id-123', { title: 'Inception', rating: 9.1, genre: 'sci-fi' })
```

### Delete

```typescript
await MovieModel.delete({ where: { genre: 'temp' } })
await MovieModel.deleteById(['movie-id-123', 'movie-id-456'])
```

### Relationships

```typescript
await MovieModel.attach({
  source: 'movie-id-123',
  target: 'actor-id-456',
  options: { type: 'STARS', direction: 'out' }
})

await MovieModel.detach({
  source: 'movie-id-123',
  target: 'actor-id-456',
  options: { type: 'STARS' }
})
```

## Initialization order

Create the `RushDB` instance before importing models — models call `RushDB.getInstance()` on first use.

```typescript
// db.ts
import RushDB from '@rushdb/javascript-sdk'
export const db = new RushDB('RUSHDB_API_KEY')

// models.ts — import db.ts first in your app entry
import './db'
import { Model } from '@rushdb/javascript-sdk'
export const MovieModel = new Model('MOVIE', { /* … */ })
```

## Transactions

```typescript
const tx = await db.tx.begin()
try {
  const movie  = await MovieModel.create({ title: 'Dune', rating: 8.0, genre: 'sci-fi' }, tx)
  const actor  = await ActorModel.create({ name: 'Timothée Chalamet' }, tx)
  await MovieModel.attach({ source: movie, target: actor, options: { type: 'STARS' } }, tx)
  await tx.commit()
} catch (e) {
  await tx.rollback()
  throw e
}
```

## Advanced TypeScript

For declaration merging, path aliases, and schema-aware intellisense (typed relation queries, aggregate result shapes), see the [Model reference](./typescript-reference/Model#typescript-extend-sdk-types-for-schema-aware-suggestions).
