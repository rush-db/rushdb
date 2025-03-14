---
sidebar_position: 3
---

# SearchQueryLogicalGrouping

The `SearchQueryLogicalGrouping` type is used to define logical groupings for combining multiple conditions in a query.

### Type Definition
```typescript
type SearchQueryLogicalGrouping<T extends FlatObject | Schema = Schema> =
  Record<'$and' | '$or' | "$not" | "$nor" |'$xor', Enumerable<SearchQueryCondition<T>>>;
```

### Properties

#### $and

- **Type:** `SearchQueryCondition[]`
- **Optional:** Yes

Combines multiple conditions using a logical AND.

#### $or

- **Type:** `SearchQueryCondition[]`
- **Optional:** Yes

Combines multiple conditions using a logical OR.

#### $not

- **Type:** `SearchQueryCondition[]`
- **Optional:** Yes

Negates the combined conditions.

#### $xor

- **Type:** `SearchQueryCondition[]`
- **Optional:** Yes

Combines multiple conditions using a logical XOR.

#### $not

- **Type**: `SearchQueryCondition[]`
- **Optional**: Yes
- 
Combines multiple conditions using a logical NOR, meaning the results must not match any of the provided conditions.

### Example Usage

Here is an example of how to use logical groupings in a query:
```typescript
const queryWhere: SearchQueryWhere<typeof AuthorSchema> = {
  $or: [
    { age: { $gt: 25 } },
    { name: { $startsWith: 'A' } }
  ]
};
```

In this example:
- The query filters records where the `age` field is greater than 25 or the `name` field starts with 'A'.
