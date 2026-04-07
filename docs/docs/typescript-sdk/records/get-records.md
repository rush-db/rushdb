---
sidebar_position: 5
---

# Get Records

RushDB provides four read methods: look up by ID, find one, find unique, or run a full search query.

## Find by ID

`db.records.findById()`

```typescript
const movie = await db.records.findById("movie-id-123");
const movies = await db.records.findById(["id-1", "id-2", "id-3"]);
```

Returns `DBRecordInstance` (single) or `DBRecordsArrayInstance` (array).

## Find One

`db.records.findOne()`

Returns the first matching record, or `null` if none found.

```typescript
const movie = await db.records.findOne({
  labels: ["MOVIE"],
  where: { title: "Inception" },
});
```

## Find Unique

`db.records.findUniq()`

Like `findOne` but throws `NonUniqueResultError` if more than one record matches.

```typescript
import { NonUniqueResultError } from "@rushdb/javascript-sdk";

try {
  const movie = await db.records.findUniq({
    labels: ["MOVIE"],
    where: { title: "Inception" },
  });
} catch (e) {
  if (e instanceof NonUniqueResultError)
    console.error(`found ${e.count} matches`);
}
```

## Find Records

`db.records.find()`

Full search with filtering, sorting, and pagination.

```typescript
const { data: movies, total } = await db.records.find({
  labels: ["MOVIE"],
  where: { rating: { $gte: 8 }, genre: "sci-fi" },
  orderBy: { rating: "desc" },
  limit: 20,
  skip: 0,
});
```

### SearchQuery parameters

| Field       | Type               | Description                                                    |
| ----------- | ------------------ | -------------------------------------------------------------- |
| `labels`    | `string[]`         | Filter by one or more labels                                   |
| `where`     | `object`           | Filter conditions ([docs](../../concepts/search/where))        |
| `orderBy`   | `string \| object` | Sort criteria ([docs](../../concepts/search/pagination-order)) |
| `limit`     | `number`           | Max records to return (default: 1000)                          |
| `skip`      | `number`           | Records to skip for pagination                                 |
| `aggregate` | `object`           | Aggregation map ([docs](../../concepts/search/aggregations))   |
| `groupBy`   | `string[]`         | Grouping keys, e.g. `['$record.genre']`                        |

## Relationship traversal

Filter across graph edges inline with `where`:

```typescript
// Movies where at least one actor is from the USA
const { data } = await db.records.find({
  labels: ["MOVIE"],
  where: {
    ACTOR: { country: "USA" },
  },
});

// With explicit relation type and direction
const { data: films } = await db.records.find({
  labels: ["MOVIE"],
  where: {
    DIRECTOR: {
      $relation: { type: "DIRECTED_BY", direction: "out" },
      name: { $contains: "Nolan" },
    },
  },
});
```

See [Where clause docs](../../concepts/search/where#relationship-queries) for full syntax.

## Field operators

```typescript
// Numeric range
where: { rating: { $gte: 8, $lte: 9.5 } }

// Set membership
where: { genre: { $in: ['sci-fi', 'thriller'] } }

// Text
where: { title: { $contains: 'dark' } }

// Existence / type checks
where: { $and: [{ poster: { $exists: true } }, { rating: { $type: 'number' } }] }

// Logical
where: { $or: [{ genre: 'sci-fi' }, { rating: { $gte: 9 } }] }
```

Full operator reference: [Where clause docs](../../concepts/search/where).

## Aggregations

```typescript
const stats = await db.records.find({
  labels: ["MOVIE"],
  where: { ACTOR: { $alias: "$actor", country: "USA" } },
  aggregate: {
    title: "$record.title",
    actorCount: { fn: "count", unique: true, alias: "$actor" },
    avgRating: { fn: "avg", field: "rating", alias: "$record", precision: 1 },
    actorNames: { fn: "collect", field: "name", alias: "$actor" },
  },
});
```

:::danger Do not set `limit` when using `aggregate` — it cuts the scan and returns mathematically incorrect totals. Use `orderBy` on an aggregated key instead.
:::

### GroupBy

```typescript
const byGenre = await db.records.find({
  labels: ["MOVIE"],
  aggregate: {
    count: { fn: "count", alias: "$record" },
    avgRating: { fn: "avg", field: "rating", alias: "$record", precision: 1 },
  },
  groupBy: ["$record.genre"],
  orderBy: { count: "desc" },
});
// [{ genre: 'sci-fi', count: 42, avgRating: 7.9 }, ...]
```

Full reference: [Aggregations](../../concepts/search/aggregations) · [Grouping](../../concepts/search/group-by)

## TimeBucket (time-series)

```typescript
const daily = await db.records.find({
  labels: ["ORDER"],
  aggregate: {
    day: {
      fn: "timeBucket",
      field: "createdAt",
      granularity: "day",
      alias: "$record",
    },
    count: { fn: "count", alias: "$record" },
  },
  groupBy: ["day"],
  orderBy: { day: "asc" },
});
```

`granularity` values: `day` · `week` · `month` · `quarter` · `year` · `hours` · `minutes` · `seconds` (use plural + `size` for custom window widths).

## Nested collect (hierarchical results)

```typescript
const tree = await db.records.find({
  labels: ["MOVIE"],
  where: { ACTOR: { $alias: "$actor" } },
  aggregate: {
    title: "$record.title",
    actors: {
      fn: "collect",
      alias: "$actor",
      aggregate: {
        name: "$actor.name",
        country: "$actor.country",
      },
    },
  },
});
```

:::note Only `fn: 'collect'` is valid inside a nested `aggregate` block.
:::

## In a transaction

```typescript
const tx = await db.tx.begin();
try {
  const { data } = await db.records.find({ labels: ["MOVIE"] }, tx);
  // … do more work …
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

const all = await MovieModel.find();
const sciFi = await MovieModel.find({ where: { genre: "sci-fi" } });
const one = await MovieModel.findOne({ where: { title: "Inception" } });
const byId = await MovieModel.findById("movie-id-123");
const unique = await MovieModel.findUniq({ where: { title: "Inception" } });
```

Model search methods auto-fill `labels` from the model definition.
