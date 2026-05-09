---
sidebar_position: 7
---

# Delete Records

## Delete by ID

`db.records.deleteById()`

```typescript
// Single record
await db.records.deleteById("movie-id-123");

// Multiple records
await db.records.deleteById(["id-1", "id-2", "id-3"]);
```

All relationships attached to deleted records are removed automatically.

## Bulk Delete

`db.records.delete()`

Delete all records matching a search query.

```typescript
// Delete all sci-fi movies with low ratings
await db.records.delete({
  labels: ["MOVIE"],
  where: { genre: "sci-fi", rating: { $lt: 5 } },
});
```

:::warning
An empty `where` without `allowForceDelete: true` in the SDK config throws `EmptyTargetError`.
:::

## In a transaction

```typescript
const tx = await db.tx.begin();
try {
  await db.records.deleteById("movie-id-123", tx);
  await db.records.delete(
    { labels: ["ACTOR"], where: { country: "temp" } },
    tx,
  );
  await tx.commit();
} catch (e) {
  await tx.rollback();
  throw e;
}
```

## Via Model

```typescript
const MovieModel = new Model("MOVIE", { title: { type: "string" } });

await MovieModel.deleteById(["id-1", "id-2"]);
await MovieModel.delete({ where: { genre: "temp" } });
```
