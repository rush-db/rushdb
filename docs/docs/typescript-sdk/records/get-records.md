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
| `select`    | `object`           | Output-shaping expressions ([docs](../../concepts/search/select))             |
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

## Select Expressions

```typescript
const stats = await db.records.find({
  labels: ["MOVIE"],
  where: { ACTOR: { $alias: "$actor", country: "USA" } },
  select: {
    title:      "$record.title",
    actorCount: { $count: "$actor" },
    avgRating:  { $avg: "$record.rating", $precision: 1 },
    actorNames: { $collect: { from: "$actor", select: { name: "$actor.name" } } },
  },
});
```

:::danger Do not set `limit` when using `select` — it cuts the scan and returns mathematically incorrect totals. Use `orderBy` on a `select` output key instead.
:::

### GroupBy

```typescript
const byGenre = await db.records.find({
  labels: ["MOVIE"],
  select: {
    count:     { $count: "*" },
    avgRating: { $avg: "$record.rating", $precision: 1 },
  },
  groupBy: ["$record.genre"],
  orderBy: { count: "desc" },
});
// [{ genre: 'sci-fi', count: 42, avgRating: 7.9 }, ...]
```

Full reference: [Select Expressions](../../concepts/search/select) · [Grouping](../../concepts/search/group-by)

## TimeBucket (time-series)

```typescript
const daily = await db.records.find({
  labels: ["ORDER"],
  select: {
    day:   { $timeBucket: { field: "$record.createdAt", unit: "day" } },
    count: { $count: "*" },
  },
  groupBy: ["day"],
  orderBy: { day: "asc" },
});
```

`unit` values: `day` · `week` · `month` · `quarter` · `year` · `hours` · `minutes` · `seconds` (use plural + `size` for custom window widths).

## Collect related records

Label-based (no alias needed — preferred for nesting):

```typescript
const companies = await db.records.find({
  labels: ["COMPANY"],
  select: {
    name: "$record.name",
    departments: {
      $collect: {
        label: "DEPARTMENT",
        select: {
          name: "$self.name",
          projects: {
            $collect: {
              label: "PROJECT",
              select: { name: "$self.name" }
            }
          }
        }
      }
    }
  }
});
```

Alias-based (requires `$alias` in `where`):

```typescript
const movies = await db.records.find({
  labels: ["MOVIE"],
  where: { ACTOR: { $alias: "$actor" } },
  select: {
    title:  "$record.title",
    actors: {
      $collect: {
        from: "$actor",
        select: { name: "$actor.name", country: "$actor.country" },
      },
    },
  },
});
```

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
