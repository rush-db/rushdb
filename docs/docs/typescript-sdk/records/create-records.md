---
sidebar_position: 1
---

# Create Records

Three methods for writing flat records. For nested/graph data see [Import Data](./import-data.md).

## Create a Record

`db.records.create()`

```typescript
const movie = await db.records.create({
  label: "MOVIE",
  data: { title: "Inception", rating: 8.8, genre: "sci-fi" },
});
// → DBRecordInstance  { __id, __label, title, rating, genre }
```

### Precise type control (PropertyDraft)

```typescript
await db.records.create({
  label: "MOVIE",
  data: [
    { name: "title", type: "string", value: "Inception" },
    { name: "rating", type: "number", value: 8.8 },
    {
      name: "genres",
      type: "string",
      value: "sci-fi,thriller",
      valueSeparator: ",",
    },
    { name: "releasedAt", type: "datetime", value: "2010-07-16T00:00:00Z" },
  ],
});
```

| PropertyDraft field | Type     | Description                                                      |
| ------------------- | -------- | ---------------------------------------------------------------- |
| `name`              | `string` | Property name                                                    |
| `type`              | `string` | `string` · `number` · `boolean` · `datetime` · `null` · `vector` |
| `value`             | any      | The value                                                        |
| `valueSeparator`    | `string` | Split `value` string into an array on this separator             |

## Create Multiple Records

`db.records.createMany()`

Flat rows only — no nested objects. For nested data use [`importJson`](./import-data.md).

```typescript
const result = await db.records.createMany({
  label: "ACTOR",
  data: [
    { name: "Leonardo DiCaprio", country: "USA" },
    { name: "Ken Watanabe", country: "Japan" },
  ],
});
// → DBRecordsArrayInstance  { data: [...], total: 2 }
```

## Upsert

`db.records.upsert()`

Create-or-update based on matching criteria.

```typescript
// Match on 'title'; update rating if found, create if not
const movie = await db.records.upsert({
  label: "MOVIE",
  data: { title: "Inception", rating: 9.0, genre: "sci-fi" },
  options: { mergeBy: ["title"], mergeStrategy: "append" },
});
```

### Merge strategies

| Strategy           | Behaviour                                                             |
| ------------------ | --------------------------------------------------------------------- |
| `append` (default) | Add / update incoming fields; preserve all other existing fields      |
| `rewrite`          | Replace all fields with incoming data; unmentioned fields are removed |

### `mergeBy` behaviour

| `mergeBy` value | Match behaviour                     |
| --------------- | ----------------------------------- |
| `['field']`     | Match only on listed fields         |
| `[]` or omitted | Match on ALL incoming property keys |

## Options

All three methods accept the same `options` object:

| Option                          | Default                         | Description                             |
| ------------------------------- | ------------------------------- | --------------------------------------- |
| `suggestTypes`                  | `true`                          | Infer types automatically               |
| `convertNumericValuesToNumbers` | `false`                         | Convert string numbers to number type   |
| `capitalizeLabels`              | `false`                         | Uppercase all inferred label names      |
| `relationshipType`              | `__RUSHDB__RELATION__DEFAULT__` | Relationship type used for nested links |
| `returnResult`                  | `false`                         | Return created records in the response  |
| `mergeBy`                       | —                               | Fields to match on for upsert           |
| `mergeStrategy`                 | `append`                        | `append` or `rewrite`                   |

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
const MovieModel = new Model("MOVIE", {
  title: { type: "string" },
  rating: { type: "number" },
});

const movie = await MovieModel.create({ title: "Inception", rating: 8.8 });
const movies = await MovieModel.createMany([
  { title: "Dune" },
  { title: "Arrival" },
]);
```
