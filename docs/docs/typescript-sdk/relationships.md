---
sidebar_position: 3
---

# Relationships

Connect records into a graph. Relationships have a type and a direction.

## Attach

`db.records.attach()`

```typescript
await db.records.attach({
  source: "movie-id-123",
  target: "actor-id-456",
  options: { type: "STARS", direction: "out" },
  // (MOVIE) -[:STARS]-> (ACTOR)
});
```

`target` can be a single ID, an array of IDs, or a record instance.

## Detach

`db.records.detach()`

```typescript
await db.records.detach({
  source: "movie-id-123",
  target: "actor-id-456",
  options: { typeOrTypes: "STARS" }, // omit to detach all types
});
```

## Bulk create by key match

Connect records by matching a property on the source to a property on the target:

```typescript
await db.relationships.createMany({
  source: { label: "MOVIE", key: "directorId" },
  target: { label: "DIRECTOR", key: "id" },
  type: "DIRECTED_BY",
  direction: "out",
});
// Creates MOVIE -[:DIRECTED_BY]-> DIRECTOR where MOVIE.directorId = DIRECTOR.id
```

## Bulk delete by key match

```typescript
await db.relationships.deleteMany({
  source: { label: "MOVIE", key: "directorId" },
  target: { label: "DIRECTOR", key: "id" },
  type: "DIRECTED_BY",
  direction: "out",
});
```

### Many-to-many deletion (cartesian)

```typescript
// Deletes HAS_TAG relationships between ALL matching pairs
await db.relationships.deleteMany({
  source: { label: "MOVIE", where: { genre: "sci-fi" } },
  target: { label: "TAG", where: { category: "genre" } },
  type: "HAS_TAG",
  manyToMany: true, // Must be explicit — requires non-empty where on both sides
});
```

:::warning Both `source.where` and `target.where` must be non-empty when `manyToMany: true`.
:::

## Find relationships

```typescript
const { data, total } = await db.relationships.find({
  labels: ["MOVIE"],
  where: {
    ACTOR: { $relation: "STARS", country: "USA" },
  },
});
```

## In a transaction

```typescript
const tx = await db.tx.begin();
try {
  const movie = await db.records.create(
    { label: "MOVIE", data: { title: "Dune" } },
    tx,
  );
  const actor = await db.records.create(
    { label: "ACTOR", data: { name: "Timothée Chalamet" } },
    tx,
  );
  await db.records.attach(
    { source: movie, target: actor, options: { type: "STARS" } },
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
await MovieModel.attach({
  source: "movie-id-123",
  target: "actor-id-456",
  options: { type: "STARS", direction: "out" },
});

await MovieModel.detach({
  source: "movie-id-123",
  target: "actor-id-456",
  options: { typeOrTypes: "STARS" },
});
```

## Direction reference

| `direction` | Graph pattern                  |
| ----------- | ------------------------------ |
| `out`       | `(source) -[:TYPE]-> (target)` |
| `in`        | `(source) <-[:TYPE]- (target)` |
