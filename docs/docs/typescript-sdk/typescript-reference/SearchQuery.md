---
sidebar_position: 8
---

# SearchQuery

`SearchQuery` is a type that defines the structure for querying [records](../../concepts/records) in RushDB. It provides a flexible way to filter, sort, paginate, and aggregate data. For more information on search concepts, see the [search documentation](../../concepts/search/introduction.md).

## Type Definition

```typescript
export type SearchQuery<S extends Schema = any> = SearchQueryLabelsClause &
  PaginationClause &
  OrderClause<S> &
  WhereClause<S> &
  AggregateClause
```

## Type Parameters

| Parameter                | Description                                                             |
|--------------------------|-------------------------------------------------------------------------|
| `S extends Schema = any` | The schema type that defines the structure of the records being queried |

## Query Components

### Labels Clause

```typescript
export type SearchQueryLabelsClause = {
  labels?: Array<string>
}
```

Specifies the labels (types) of records to search for. If omitted, only records with the model's label will be searched.

### Pagination Clause

```typescript
export type PaginationClause = {
  limit?: number
  skip?: number
}
```

Controls pagination of the query results.

| Property | Type     | Description                         |
|----------|----------|-------------------------------------|
| `limit`  | `number` | Maximum number of records to return |
| `skip`   | `number` | Number of records to skip           |

### Order Clause

```typescript
export type OrderClause<S extends Schema = Schema> = {
  orderBy?: Order<S>
}

export type OrderDirection = 'asc' | 'desc'
export type Order<S extends Schema = Schema> = OrderDirection | Partial<Record<keyof S, OrderDirection>>
```

Specifies how to sort the query results.

### Where Clause

```typescript
export type WhereClause<S extends Schema = Schema> = {
  where?: Where<S>
}
```

Filters records based on property values and relationships.

### Aggregate Clause

```typescript
export type AggregateClause = {
  aggregate?: Aggregate
}
```

Defines aggregation operations to perform on the query results.

## Where Expressions

The `where` property of a search query can include various expressions to filter records:

### Property Expressions

```typescript
export type PropertyExpression =
  BooleanExpression |
  DatetimeExpression |
  NullExpression |
  NumberExpression |
  StringExpression |
  VectorExpression
```

#### Number Expressions

```typescript
export type NumberExpression = number | {
  $gt?: number
  $gte?: number
  $in?: Array<number>
  $lt?: number
  $lte?: number
  $ne?: number
  $nin?: Array<number>
}
```

#### String Expressions

```typescript
export type StringExpression = string | {
  $in?: Array<string>
  $ne?: string
  $nin?: Array<string>
  $endsWith?: string
  $startsWith?: string
  $contains?: string
}
```

#### Boolean Expressions

```typescript
export type BooleanExpression = boolean | {
  $ne?: boolean
}
```

#### Null Expressions

```typescript
export type NullExpression = null | {
  $ne?: null
}
```

### Logical Expressions

```typescript
export type LogicalExpression<T = PropertyExpression & Related> = RequireAtLeastOne<
  AndExpression<T> & OrExpression<T> & NotExpression<T> & XorExpression<T> & NorExpression<T>
>

export type AndExpression<T = PropertyExpression & Related> = {
  $and: MaybeArray<LogicalExpressionValue<T>>
}

export type OrExpression<T = PropertyExpression & Related> = {
  $or: MaybeArray<LogicalExpressionValue<T>>
}

export type NotExpression<T = PropertyExpression & Related> = {
  $not: MaybeArray<LogicalExpressionValue<T>>
}

export type XorExpression<T = PropertyExpression & Related> = {
  $xor: MaybeArray<LogicalExpressionValue<T>>
}

export type NorExpression<T = PropertyExpression & Related> = {
  $nor: MaybeArray<LogicalExpressionValue<T>>
}
```

### Related Records

```typescript
export type Related<M extends Record<string, Model['schema']> = Models> =
  keyof M extends never ? AnyObject
  : {
      [Key in keyof M]?: {
        $alias?: string
        $relation?: RelationOptions | string
      } & Where<M[Key]>
    }
```

Defines conditions on related records. Learn more about [relationships in RushDB](../../concepts/relationships).

### Aggregation

```typescript
export type AggregateFn<S extends Schema = Schema> =
  | { alias: string; field: string; fn: 'avg'; precision?: number }
  | { alias: string; field: string; fn: 'max' }
  | { alias: string; field: string; fn: 'min' }
  | { alias: string; field: string; fn: 'sum' }
  | { alias: string; field?: string; fn: 'count'; uniq?: boolean }
  | { field: string; fn: `gds.similarity.${VectorSearchFn}`; alias: string; vector: number }
  | AggregateCollectFn
```

Defines aggregation functions to apply to the query results.

## Usage Examples

### Basic Query

```typescript
// Find all users aged 30 or above
const users = await UserModel.find({
  where: {
    age: { $gte: 30 }
  }
});
```

### Complex Filtering

```typescript
// Find active users with specific email domains
const users = await UserModel.find({
  where: {
    $and: [
      { active: true },
      {
        $or: [
          { email: { $endsWith: '@gmail.com' } },
          { email: { $endsWith: '@outlook.com' } }
        ]
      }
    ]
  }
});
```

### Pagination and Sorting

```typescript
// Get the second page of users sorted by name
const users = await UserModel.find({
  orderBy: { name: 'asc' },
  skip: 10,
  limit: 10
});
```

### Filtering by Related Records

```typescript
// Find users who authored a post with a specific title
const users = await UserModel.find({
  where: {
    Post: {
      $relation: { type: 'AUTHORED' },
      title: { $eq: 'My First Post' }
    }
  }
});
```

### Aggregation

```typescript
// Calculate average age of users per country
const results = await UserModel.find({
  aggregate: {
    averageAge: { fn: 'avg', field: 'age', alias: 'averageAge' },
    countries: {
      fn: 'collect',
      field: 'country',
      alias: 'countries',
      uniq: true
    }
  }
});
```

### Vector Search

```typescript
// Find records with similar vector embeddings
const similar = await EmbeddingModel.find({
  where: {
    embedding: {
      $similarity: {
        vector: [0.1, 0.2, 0.3, ...],
        limit: 10
      }
    }
  }
});
```
