---
sidebar_position: 8
---

# SearchQuery

`SearchQuery` is a type that defines the structure for querying [records](/learn/records-and-queries/store-records) in RushDB. It provides a flexible way to filter, sort, paginate, and compute metrics using `select` and `groupBy`. The legacy `aggregate` clause is deprecated and should only be used for vector similarity until `select` supports it. For more information on search concepts, see the [search documentation](/learn/search-query).

## Type Definition

```typescript
export type SearchQuery<S extends Schema = any> = SearchQueryLabelsClause &
  PaginationClause &
  OrderClause<S> &
  WhereClause<S> &
  SelectClause &
  GroupByClause
```

## Type Parameters

| Parameter                   | Description                                                                                                                                                                                                                                                                                                                                                                                                           |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `S extends Schema = Schema` | The schema type that defines the structure of the records being queried. `Schema` is `Record<string, SchemaField>` where `SchemaField = { type: 'boolean' \| 'datetime' \| 'null' \| 'number' \| 'string'; required?: boolean; multiple?: boolean; unique?: boolean; default?: ... }`. The default `Schema` (rather than `any`) preserves type safety while remaining permissive when no explicit schema is provided. |

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
| -------- | -------- | ----------------------------------- |
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

### Select Clause

```typescript
export type CollectExpr = {
  // ── alias-based (requires $alias declared in where) ──
  from?: string // "$alias" — the traversal alias to collect from

  // ── label-based (inline traversal, no alias needed) ──
  label?: string // related record label (e.g. 'DEPARTMENT'); mutually exclusive with from
  where?: Record<string, any> // flat property filter on this traversal level (label-based only)

  // ── common options ──
  select?: Record<string, string | { $collect: CollectExpr }> // field projection; nested $collect allowed
  orderBy?: Order
  limit?: number
  skip?: number
  unique?: boolean // default: true
}
// Use $self in select to reference the current level when using label: { name: '$self.name' }
// from and label are mutually exclusive.

export type TimeBucketExprUnit =
  | 'day'
  | 'week'
  | 'month'
  | 'quarter'
  | 'year'
  | 'hour'
  | 'minute'
  | 'second'
  | 'months'
  | 'hours'
  | 'minutes'
  | 'seconds'
  | 'years'

export type TimeBucketExpr = {
  field: string // "$alias.fieldName" — must be a datetime field
  unit: TimeBucketExprUnit
  size?: number // required when unit is plural
}

export type Expr =
  | string // "$alias.field" or "$alias"
  | number // literal
  | boolean // literal
  | { $ref: string } // reference another select output key
  | { $sum: Expr }
  | { $avg: Expr; $precision?: number }
  | { $count: '*' | Expr }
  | { $min: Expr }
  | { $max: Expr }
  | { $divide: [Expr, Expr] }
  | { $multiply: [Expr, Expr] }
  | { $add: [Expr, Expr] }
  | { $subtract: [Expr, Expr] }
  | { $collect: CollectExpr }
  | { $timeBucket: TimeBucketExpr }

export type SelectExprMap = Record<string, Expr>

export type SelectClause = {
  select?: SelectExprMap
}
```

The canonical output-shaping clause. See the [Select Expressions guide](/learn/search-query/select-expressions) for full documentation.

### GroupBy Clause

```typescript
export type GroupByClause = {
  groupBy?: Array<string>
}
```

Shapes how aggregation output is grouped. See [GroupBy](#groupby) for the two supported modes.

## Where Expressions

The `where` property of a search query can include various expressions to filter records:

### Property Expressions

```typescript
export type PropertyExpression =
  | BooleanExpression
  | DatetimeExpression
  | NullExpression
  | NumberExpression
  | StringExpression
  | TypeExpression
```

#### Number Expressions

```typescript
export type NumberExpression =
  | number
  | {
      $gt?: number
      $gte?: number
      $in?: Array<number>
      $lt?: number
      $lte?: number
      $ne?: number
      $nin?: Array<number>
      $exists?: boolean
    }
```

#### String Expressions

```typescript
export type StringExpression =
  | string
  | {
      $in?: Array<string>
      $ne?: string
      $nin?: Array<string>
      $endsWith?: string
      $startsWith?: string
      $contains?: string
      $exists?: boolean
    }
```

#### Datetime Expressions

```typescript
export type DatetimeObject = {
  $year: number
  $month?: number
  $day?: number
  $hour?: number
  $minute?: number
  $second?: number
  $millisecond?: number
  $microsecond?: number
  $nanosecond?: number
}

export type DatetimeExpression =
  | string
  | DatetimeObject
  | {
      $gt?: DatetimeObject | string
      $gte?: DatetimeObject | string
      $lt?: DatetimeObject | string
      $lte?: DatetimeObject | string
      $ne?: DatetimeObject | string
      $in?: Array<DatetimeObject | string>
      $nin?: Array<DatetimeObject | string>
      $exists?: boolean
    }
```

Datetime fields support two matching styles:

**ISO 8601 exact match or set membership:**

```typescript
// Exact ISO match
{
  where: {
    created: '2023-01-01T00:00:00Z'
  }
}

// Match a set of dates
{
  where: {
    created: {
      $in: ['2023-01-01T00:00:00Z', '2023-06-01T00:00:00Z']
    }
  }
}
```

**Component object — match a specific point in time or use for range comparisons:**

```typescript
// Exact point: January 1 2023
{ where: { created: { $year: 2023, $month: 1, $day: 1 } } }
```

:::warning Never use plain ISO strings with `$gt`/`$lt` comparisons
Always use component objects for range comparisons:

```typescript
// Records created in 1994
{ where: { created: { $gte: { $year: 1994 }, $lt: { $year: 1995 } } } }

// Records created in January 1994
{ where: { created: { $gte: { $year: 1994, $month: 1 }, $lt: { $year: 1994, $month: 2 } } } }

// Records created in the 1990s
{ where: { created: { $gte: { $year: 1990 }, $lt: { $year: 2000 } } } }

// Records created on 1994-03-15
{ where: { created: { $gte: { $year: 1994, $month: 3, $day: 15 }, $lt: { $year: 1994, $month: 3, $day: 16 } } } }
```

:::

#### Boolean Expressions

```typescript
export type BooleanExpression =
  | boolean
  | {
      $ne?: boolean
      $exists?: boolean
    }
```

#### Null Expressions

```typescript
export type NullExpression = null | {
  $ne?: null
  $exists?: boolean | null
}
  $ne?: null
}
```

#### Type Expressions

```typescript
export type TypeExpression = {
  $type: 'string' | 'number' | 'boolean' | 'datetime' | 'null'
}
```

The `$type` operator checks whether a field has a specific data type:

```typescript
// Find records where age is actually stored as a number
{
  where: {
    age: {
      $type: 'number'
    }
  }
}
```

#### $id Operator

Filter records by their own ID directly inside the `where` clause:

```typescript
// Find records whose ID is in a known set
{
  where: {
    $id: {
      $in: ['id1', 'id2', 'id3']
    }
  }
}

// Filter by specific ID on a related node
{
  where: {
    EMPLOYEE: {
      $id: 'specific-employee-id'
    }
  }
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

Defines conditions on related records. The key of the nested object **is** the label name (case-sensitive). Use `$alias` to name the traversal for later use in `select`/`groupBy`, and `$relation` to constrain the relationship type or direction:

```typescript
// Constrain by relationship type and direction
{
  where: {
    POST: {
      $relation: { type: 'AUTHORED', direction: 'in' }, // full form
      title: { $contains: 'Graph' }
    }
  }
}

// Shorthand — type only
{
  where: {
    POST: {
      $relation: 'AUTHORED',
      publishedAt: { $gte: { $year: 2024 } }
    }
  }
}
```

Learn more about [relationships in RushDB](/learn/relationships).

#### $xor and $nor operators

```typescript
// $xor — exactly one of the conditions must match
{
  where: {
    $xor: [{ isPremium: true }, { hasFreeTrialAccess: true }]
  }
}

// $nor — none of the conditions may match
{
  where: {
    $nor: [{ status: 'deleted' }, { status: 'archived' }]
  }
}
```

### Aggregation

`select` expressions support all aggregation operations. See the [Select Expressions guide](/learn/search-query/select-expressions) for full documentation.

```typescript
// Count and sum via select expressions
const stats = await db.records.find({
  labels: ['COMPANY'],
  where: { EMPLOYEE: { $alias: '$employee', salary: { $gte: 50000 } } },
  select: {
    companyName: '$record.name',
    headcount: { $count: '*' },
    totalWage: { $sum: '$employee.salary' },
    avgSalary: { $avg: '$employee.salary', $precision: 0 }
  }
})
```

## GroupBy

`groupBy` operates in two modes:

### Mode A — Dimensional (one row per distinct value)

Entries are `'$alias.propertyName'` strings. Each distinct value becomes its own output row.

```typescript
// Count and avg amount per deal stage
const result = await db.records.find({
  labels: ['DEAL'],
  select: {
    count: { $count: '*' },
    avgAmt: { $avg: '$record.amount', $precision: 2 }
  },
  groupBy: ['$record.stage'],
  orderBy: { count: 'desc' }
})
// Output: [{ stage: 'won', count: 42, avgAmt: 15200.00 }, ...]

// Pivot on two keys (category × active)
const pivot = await db.records.find({
  labels: ['PROJECT'],
  select: { count: { $count: '*' } },
  groupBy: ['$record.category', '$record.active'],
  orderBy: { count: 'desc' }
})
```

### Mode B — Self-group (one row with global KPIs)

Put the **select key names** themselves into `groupBy` (not `$alias.field` paths).

```typescript
// Total salary across all employees (single result row)
const kpis = await db.records.find({
  labels: ['EMPLOYEE'],
  select: {
    totalSalary: { $sum: '$record.salary' },
    headcount: { $count: '*' },
    avgSalary: { $avg: '$record.salary', $precision: 0 }
  },
  groupBy: ['totalSalary', 'headcount', 'avgSalary'],
  orderBy: { totalSalary: 'asc' } // ← required for correct full-scan total
})
// Output: [{ totalSalary: 4875000, headcount: 95, avgSalary: 51315 }]
```

:::caution Late-ordering rule
For self-group queries, always add `orderBy` on a `select` output key. Without it the engine applies `LIMIT` before aggregation and produces mathematically wrong totals.
:::

## Critical Rules

:::danger Do not combine `limit` with `select`
Never set `limit` when `select` is present (except to cap root records in a per-record flat query). `limit` restricts the record scan, so metrics like `$sum` or `$avg` operate only on the first N rows and return wrong results. The legacy `aggregate` clause is deprecated and should only be used for vector similarity until select supports it.

```typescript
// ❌ WRONG — limit cuts the scan
const wrong = await db.records.find({
  labels: ['PROJECT'],
  select: { totalBudget: { $sum: '$record.budget' } },
  groupBy: ['totalBudget'],
  limit: 10 // DO NOT include
})

// ✅ CORRECT — no limit; full dataset is summed
const correct = await db.records.find({
  labels: ['PROJECT'],
  select: { totalBudget: { $sum: '$record.budget' } },
  groupBy: ['totalBudget'],
  orderBy: { totalBudget: 'asc' } // triggers late ordering
})
```

:::

## Usage Examples

### Basic Query

```typescript
// Find all users aged 30 or above
const users = await UserModel.find({
  where: {
    age: { $gte: 30 }
  }
})
```

### Complex Filtering

```typescript
// Find active users with specific email domains
const users = await UserModel.find({
  where: {
    $and: [
      { active: true },
      {
        $or: [{ email: { $endsWith: '@gmail.com' } }, { email: { $endsWith: '@outlook.com' } }]
      }
    ]
  }
})
```

### Pagination and Sorting

```typescript
// Get the second page of users sorted by name
const users = await UserModel.find({
  orderBy: { name: 'asc' },
  skip: 10,
  limit: 10
})
```

### Datetime Range Query

```typescript
// Records created in 2024
const recent = await UserModel.find({
  where: {
    createdAt: { $gte: { $year: 2024 }, $lt: { $year: 2025 } }
  }
})

// Records created in Q1 2023
const q1 = await db.records.find({
  labels: ['ORDER'],
  where: {
    issuedAt: { $gte: { $year: 2023, $month: 1 }, $lt: { $year: 2023, $month: 4 } }
  }
})
```

### Filter by Record ID

```typescript
// Find records from a known set of IDs
const records = await db.records.find({
  where: { $id: { $in: ['id1', 'id2', 'id3'] } }
})
```

### Filtering by Related Records

```typescript
// Find users who authored a post (constrain relationship type + direction)
const users = await UserModel.find({
  where: {
    POST: {
      $relation: { type: 'AUTHORED', direction: 'in' },
      title: { $contains: 'Graph' }
    }
  }
})
```

### Aggregation with `select`

```typescript
// One row per company with employee stats
const stats = await db.records.find({
  labels: ['COMPANY'],
  where: { EMPLOYEE: { $alias: '$employee', salary: { $gte: 50000 } } },
  select: {
    companyName: '$record.name',
    headcount: { $count: '$employee' },
    totalWage: { $sum: '$employee.salary' },
    avgSalary: { $avg: '$employee.salary', $precision: 0 },
    employeeNames: {
      $collect: {
        from: '$employee',
        select: { name: '$employee.name' },
        orderBy: { name: 'asc' },
        limit: 10
      }
    }
  }
})
```

### TimeBucket — Time-Series Aggregation (using `select`)

```typescript
// Daily order count for 2024
const daily = await db.records.find({
  labels: ['ORDER'],
  where: { issuedAt: { $gte: { $year: 2024 }, $lt: { $year: 2025 } } },
  select: {
    day: { $timeBucket: { field: '$record.issuedAt', unit: 'day' } },
    count: { $count: '*' }
  },
  groupBy: ['day'],
  orderBy: { day: 'asc' }
})

// Monthly revenue
const monthly = await db.records.find({
  labels: ['ORDER'],
  select: {
    month: { $timeBucket: { field: '$record.issuedAt', unit: 'month' } },
    revenue: { $sum: '$record.amount' }
  },
  groupBy: ['month'],
  orderBy: { month: 'asc' }
})

// Bi-monthly buckets
const biMonthly = await db.records.find({
  labels: ['ORDER'],
  select: {
    period: { $timeBucket: { field: '$record.issuedAt', unit: 'months', size: 2 } },
    count: { $count: '*' }
  },
  groupBy: ['period'],
  orderBy: { period: 'asc' }
})
```

### Nested Collect (Hierarchical Output)

Label-based `$collect` traverses inline — no `$alias` declarations needed. Use `$self` to reference the current level:

```typescript
// COMPANY → DEPARTMENT → PROJECT → EMPLOYEE tree (label-based, preferred)
const tree = await db.records.find({
  labels: ['COMPANY'],
  select: {
    departments: {
      $collect: {
        label: 'DEPARTMENT',
        select: {
          name: '$self.name',
          projects: {
            $collect: {
              label: 'PROJECT',
              select: {
                name: '$self.name',
                employees: {
                  $collect: {
                    label: 'EMPLOYEE',
                    orderBy: { salary: 'desc' },
                    limit: 3
                  }
                }
              }
            }
          }
        }
      }
    }
  }
})
// Output: [{ departments: [{ name: 'Eng', projects: [{ name: 'Platform', employees: [...] }] }] }]
```

Alias-based form (requires `$alias` in `where`, one hop per `$collect`):

```typescript
// COMPANY → DEPARTMENT collect using alias
const tree = await db.records.find({
  labels: ['COMPANY'],
  where: {
    DEPARTMENT: { $alias: '$dept' }
  },
  select: {
    company: '$record.name',
    departments: {
      $collect: {
        from: '$dept',
        select: { name: '$dept.name' }
      }
    }
  }
})
```
