---
sidebar_position: 3
---

# SearchQueryLogicalGrouping

The `SearchQueryLogicalGrouping` type is used to define logical groupings for combining multiple conditions in a query.

### Type Definition
```typescript
type SearchQueryLogicalGrouping<T extends FlatObject | Schema = Schema> =
  Record<'$AND' | '$NOT' | '$OR' | '$XOR', Enumerable<SearchQueryCondition<T>>>;
```

### Properties

#### $AND

- **Type:** `SearchQueryCondition[]`
- **Optional:** Yes

Combines multiple conditions using a logical AND.

#### $OR

- **Type:** `SearchQueryCondition[]`
- **Optional:** Yes

Combines multiple conditions using a logical OR.

#### $NOT

- **Type:** `SearchQueryCondition[]`
- **Optional:** Yes

Negates the combined conditions.

#### $XOR

- **Type:** `SearchQueryCondition[]`
- **Optional:** Yes

Combines multiple conditions using a logical XOR.

### Example Usage

Here is an example of how to use logical groupings in a query:
```typescript
const queryWhere: SearchQueryWhere<typeof AuthorSchema> = {
  $AND: [
    { age: { $gt: 25 } },
    { name: { $startsWith: 'A' } }
  ]
};
```

In this example:
- The query filters records where the `age` field is greater than 25 or the `name` field starts with 'A'.