---
sidebar_position: 1
---

# SearchQuery

The `SearchQuery` type is used to define the criteria for querying records in the RushDB SDK. It allows you to specify filters, sorting, pagination, and related records to be retrieved.

### Type Definition
```typescript
type SearchQuery<T extends FlatObject | Schema = Schema> =
  SearchQueryCommonParams<T> & SearchQueryWhereClause<T> & { includes?: never };

type SearchQueryCommonParams<T extends FlatObject | Schema = Schema> = {
  labels?: string[];
  limit?: number;
  orderBy?: SearchQueryOrderBy<T>;
  skip?: number;
};

type SearchQueryOrderBy<T extends FlatObject | Schema = Schema> =
  | 'asc'
  | 'desc'
  | SearchQueryOrderByMap<T>;

type SearchQueryOrderByMap<T extends FlatObject | Schema = Schema> =
  Partial<Record<keyof T, 'asc' | 'desc'>>;
```

### Properties

#### labels

- **Type:** `string[]`
- **Optional:** Yes

An array of labels to filter the records by their assigned labels.

#### limit

- **Type:** `number`
- **Optional:** Yes

Limits the number of records returned by the query.

#### orderBy

- **Type:** `SearchQueryOrderBy`
- **Optional:** Yes

Defines the order in which the records should be returned. It can be a string (`'asc' | 'desc'`) or an object mapping fields to their respective sort order.

#### skip

- **Type:** `number`
- **Optional:** Yes

Specifies the number of records to skip before starting to return results.

#### where

- **Type:** `SearchQueryWhere`
- **Optional:** Yes

Defines the filtering conditions for the query. It is an object mapping field names to their respective conditions.

### Example Usage

Here is an example of how to define a query using `SearchQuery`:
```typescript
const query: SearchQuery<typeof AuthorSchema> = {
  limit: 10,
  orderBy: { createdAt: 'desc' },
  skip: 5,
  where: {
    $AND: [
      { age: { $gt: 25 } },
      { name: { $startsWith: 'A' } }
    ]
  }
};
```

In this example:
- The query limits the results to 10 records.
- It orders the results by the `createdAt` field in descending order.
- It skips the first 5 records.
- It filters the records where the `age` field is greater than 25 and the `name` field starts with 'A'.