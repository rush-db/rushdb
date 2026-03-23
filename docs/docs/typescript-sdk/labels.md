---
sidebar_position: 5
---

# Labels

List which labels exist in your project and how many records each has.

## `db.labels.find()`

Returns `{ [label]: count }` for records matching the optional filter.

```typescript
// All labels
const { data } = await db.labels.find()
// { MOVIE: 84, ACTOR: 312, DIRECTOR: 47 }

// Labels for records where rating > 8
const { data } = await db.labels.find({
  where: { rating: { $gt: 8 } }
})
// { MOVIE: 21 }
```

`find()` accepts the same `where`, `skip`, `limit` parameters as `db.records.find()`.

