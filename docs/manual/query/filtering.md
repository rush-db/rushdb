# Filtering

The `predicate` field of a [search query](search-queries.md) selects which
records match. This page is the complete reference for predicate syntax:
field conditions, comparison and string operators, logical combinators,
identity filters, and the exact semantics for types, missing values, nulls,
arrays, and datetimes. Filtering by related records is covered separately in
[Relationships and traversal](relationships-and-traversal.md).

The same predicate syntax is used everywhere a query document appears:
listings, aggregations, facets, `describe.explain`, `deleteRecords`, the
`filter` of a join side, the selectors of `linkRecords`, and the
`predicate` of vector and hybrid searches.

## Field conditions

Each key of the predicate object names a property. Several keys are combined
with an implicit AND.

```json
{
  "labels": ["USER"],
  "predicate": {
    "status": "active",
    "score": { "$gte": 3, "$lte": 5 }
  }
}
```

The value of a key decides how it is read:

| Value form | Meaning | Example |
|---|---|---|
| Scalar (string, number, boolean) | Equality | `{ "status": "active" }` |
| `null` | The property is present and explicitly `null` | `{ "nick": null }` |
| Array of scalars | Membership, same as `$in` | `{ "status": ["active", "pending"] }` |
| Object of `$` operators | Every operator must hold (implicit AND) | `{ "score": { "$gte": 3, "$lte": 5 } }` |
| Any other object | A related-record block keyed by **label** | `{ "DEPARTMENT": { "name": "Sales" } }` (see [Relationships and traversal](relationships-and-traversal.md)) |

A `$`-prefixed key that is not an operator from the table below, such as
the misspelled `{ "name": { "$regx": "a" } }`, is refused with
`invalid_request`.

## Operator reference

| Operator | Value | Matches when | Missing property |
|---|---|---|---|
| `$eq` | scalar, `null`, ISO datetime string, or datetime component object | The value (or any array element) equals the operand. Strings compare case-sensitively; numbers compare numerically (`1` equals `1.0`). | No match |
| `$ne` | as `$eq` | The property is present and neither the value nor any array element equals the operand. | No match |
| `$gt`, `$gte`, `$lt`, `$lte` | number, ISO datetime string, or datetime component object | The numeric (or datetime) value, or any numeric array element, satisfies the bound. Strings and booleans never satisfy a range. | No match |
| `$in` | array of scalars | The value, or any array element, equals one of the operands. | No match |
| `$nin` | array of scalars | The property is present and neither the value nor any array element equals an operand. | No match |
| `$contains` | string | A string value (or string element) contains the operand. **Case-insensitive.** | No match |
| `$startsWith` | string | A string value (or element) starts with the operand. **Case-insensitive.** | No match |
| `$endsWith` | string | A string value (or element) ends with the operand. **Case-insensitive.** | No match |
| `$exists` | boolean | `true`: the property is present (even if `null`). `false`: it is absent. | `$exists: false` matches |
| `$type` | type name | The property has that type (see [Type tests](#type-tests)). | See [Type tests](#type-tests) |
| `$year`, `$month`, `$day`, `$hour`, `$minute`, `$second`, `$millisecond`, `$microsecond`, `$nanosecond` | integer | The datetime value's UTC calendar component equals the operand (see [Datetimes](#datetimes)). | No match |
| `$not` | operator object | The inner operators do not all hold. `{ "age": { "$not": { "$gte": 4 } } }` | Matches (the inner condition is false) |
| `$and`, `$or`, `$nor` | array of operator objects | Field-scoped logic, see [Field-scoped logic](#field-scoped-logic). | — |
| `$xor` | array of exactly two | Exactly one of the two holds. | — |

A predicate on a property that no record has is valid and simply matches
nothing.

## Logical combinators

At the top level of `predicate` (and inside related-record blocks) the
combinators take full condition objects:

| Combinator | Value | Matches when |
|---|---|---|
| `$and` | array of predicates | All hold. |
| `$or` | array of predicates | At least one holds. |
| `$nor` | array of predicates | None holds. |
| `$xor` | array of exactly two predicates | Exactly one holds. Any other length is `invalid_request`. |
| `$not` | one predicate object | The predicate does not hold. |

```json
{
  "labels": ["USER"],
  "predicate": {
    "$or": [
      { "status": "active" },
      { "$and": [{ "status": "pending" }, { "score": { "$gt": 4 } }] }
    ],
    "$not": { "banned": true }
  }
}
```

```json
{ "labels": ["USER"], "predicate": { "$xor": [{ "premium": true }, { "freeAccess": true }] } }
```

### Field-scoped logic

Inside one field's operator object, `$and`, `$or`, `$nor` and `$xor` take an
array whose elements are operator objects applied to that same field:

```json
{
  "labels": ["POST"],
  "predicate": {
    "created": { "$year": 2011, "$month": 11, "$day": 11 },
    "rating": {
      "$or": [
        { "$and": [{ "$gt": 4.5 }, { "$lt": 6 }] },
        { "$ne": 3 },
        { "$not": { "$gte": 4 } }
      ]
    }
  }
}
```

An element that names other properties (has non-`$` keys) is evaluated as a
full predicate instead.

## Identity filters

`$id` matches the record id. It accepts dashed UUIDs and 32-hex-digit ids,
case-insensitively:

```json
{ "predicate": { "$id": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77" } }
```

```json
{ "predicate": { "$id": { "$in": ["0192f1d26c1e7a409d1b0d6f2c1e4a77", "0192f1d2-6c1f-7b11-8e2a-5a6b7c8d9e0f"] } } }
```

`$id` may also be used inside a related-record block to pin the related
record.

## Type semantics

### Numbers

Integers and floating-point numbers form one numeric domain: `{ "n": 1 }`
matches a stored `1.0`, and range bounds compare across both. Integer range
bounds must be exactly representable as a 64-bit float; a bound such as
`9007199254740993` is rejected with `invalid_request` rather than silently
rounded.

### Strings

Equality (`$eq`, `$in`, `$ne`, `$nin`, flat equality) is exact and
case-sensitive. `$contains`, `$startsWith` and `$endsWith` lower-case both
sides first. Range operators never match strings; to range over text,
filter on a datetime or number property instead.

### Booleans

Booleans match only booleans: `{ "active": true }` does not match the string
`"true"` or the number `1`.

### Missing values and `null`

RushDB distinguishes an **absent** property from one that is **present with
the value `null`**:

| Condition | Absent property | Present, `null` | Present, `"x"` |
|---|---|---|---|
| `{ "p": null }` or `{ "p": { "$eq": null } }` | no | **yes** | no |
| `{ "p": { "$exists": true } }` | no | **yes** | **yes** |
| `{ "p": { "$exists": false } }` | **yes** | no | no |
| `{ "p": { "$ne": "x" } }` | no | **yes** | no |
| `{ "p": { "$nin": ["x"] } }` | no | **yes** | no |
| `{ "$not": { "p": "x" } }` | **yes** | **yes** | no |

`$ne` and `$nin` require the property to be present. To include records
that lack the property, combine with `$exists`:

```json
{ "$or": [{ "status": { "$ne": "deleted" } }, { "status": { "$exists": false } }] }
```

### Arrays

A condition on an array-valued property matches when **any element**
satisfies it. With `tags: ["x", "y"]`:

| Condition | Matches |
|---|---|
| `{ "tags": "x" }` | yes |
| `{ "tags": ["y", "z"] }` | yes (any element is in the list) |
| `{ "tags": { "$ne": "x" } }` | no (an element equals `x`) |
| `{ "tags": { "$nin": ["z"] } }` | yes |
| `{ "tags": { "$contains": "X" } }` | yes (case-insensitive) |

Range operators apply to numeric elements the same way.

### Type tests

`$type` accepts these names (case-insensitive; `bool` is an alias for
`boolean`):

| `$type` value | Matches |
|---|---|
| `"string"`, `"number"`, `"boolean"`, `"datetime"` | The property's stored type. A property stored as an array of that type also matches. |
| `"string[]"`, `"number[]"`, `"boolean[]"`, `"datetime[]"` | An array whose elements have that type. |
| `"array"` | Any array value. |
| `"object"` | An object value. |
| `"null"` | A `null` value or an absent property. |

> **Note:** For the scalar type names, `$type` consults the type the schema
> catalog records for the property name, so a value stored as a string in a
> `datetime` property matches `"datetime"`. A record that does not carry the
> property matches only `"null"`.

## Datetimes

RushDB stores datetimes as UTC instants with millisecond precision. When a
record is written with default options, string values that parse as one of
the following are stored as datetimes (see
[Properties and types](../data-model/properties-and-types.md)):

- RFC 3339 timestamps, for example `2023-06-15T12:00:00Z` or
  `2023-06-15T14:00:00+02:00` (a lower-case `z` is accepted);
- local timestamps without an offset, `2023-06-15T12:00:00` or
  `2023-06-15T12:00:00.250`, interpreted as UTC;
- dates, `2023-06-15`, interpreted as midnight UTC.

In a predicate you can write a datetime operand three ways:

| Operand form | Example | Where accepted |
|---|---|---|
| ISO string (any of the formats above) | `"2023-06-15T00:00:00Z"` | `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte` |
| Component object (`$year` required; `$month`, `$day` default to 1; `$hour`, `$minute`, `$second`, `$millisecond` default to 0) | `{ "$year": 2024, "$month": 1, "$day": 1 }` | `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte` |
| Epoch milliseconds | `1686787200000` | every numeric operator |

```json
{ "labels": ["EVENT"], "predicate": { "created": { "$gte": "2023-06-15T00:00:00Z" } } }
```

```json
{ "labels": ["EVENT"], "predicate": { "created": { "$lt": { "$year": 2024, "$month": 1, "$day": 1 } } } }
```

> **Note:** Only the explicit operators convert ISO strings. A flat equality
> such as `{ "created": "2023-06-15" }` compares against the literal string
> and does not match a datetime property; write
> `{ "created": { "$eq": "2023-06-15" } }` instead. Values inside `$in` and
> `$nin` are not converted either.

### Component operators

The component operators test one UTC calendar field of a datetime. Several
components in one operator object must all match:

```json
{ "labels": ["EVENT"], "predicate": { "created": { "$year": 2023, "$month": 6 } } }
```

| Operator | Component |
|---|---|
| `$year` | Calendar year |
| `$month` | Month, 1–12 |
| `$day` | Day of month, 1–31 |
| `$hour` | Hour, 0–23 |
| `$minute` | Minute, 0–59 |
| `$second` | Second, 0–59 |
| `$millisecond` | Sub-second milliseconds, 0–999 |
| `$microsecond` | Sub-second microseconds (a multiple of 1,000, since storage is millisecond-precise) |
| `$nanosecond` | Sub-second nanoseconds (a multiple of 1,000,000) |

Component values must be integers; anything else is `invalid_request`.

## Errors

| Situation | Error |
|---|---|
| Unknown top-level document key (for example `where`) | `invalid_request` |
| Operator with the wrong operand type (`$in` without an array, `$exists` without a boolean, a non-string `$contains`) | `invalid_request` |
| `$xor` without exactly two operands | `invalid_request` |
| A `$` operator used as a top-level key where a property is expected (for example `{ "$gt": 5 }`) | `invalid_request` |
| A filter needs more working memory than the per-query budget | `resource_exhausted` |

## Performance notes

A property filter is answered from a declared ordered index when one exists
on that label and property (equality, `$in`, ranges and `$startsWith`);
otherwise RushDB walks the records of the requested labels that carry the
property. Always include `labels` when you know them. See
[Indexes](../indexes-and-search/indexes.md) and
[Query planning](../indexes-and-search/query-planning.md).

## See also

- [Search queries](search-queries.md)
- [Relationships and traversal](relationships-and-traversal.md)
- [Properties and types](../data-model/properties-and-types.md)
- [Indexes](../indexes-and-search/indexes.md)
