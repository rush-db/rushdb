---
sidebar_position: 6
---

# NumberValue

The `NumberValue` type is used to define conditions for number fields in a query.

### Type Definition
```typescript
type NumberValue =
  | RequireAtLeastOne<
      Record<'$gt' | '$gte' | '$lt' | '$lte' | '$ne', number> & Record<'$in' | '$nin', Array<number>>
    >
  | number;
```

### Properties

#### $gt, $gte, $lt, $lte, $ne

- **Type:** `number`
- **Optional:** Yes

Defines greater than, greater than or equal to, less than, less than or equal to, and not conditions for number fields.

#### $in, $nin

- **Type:** `number[]`
- **Optional:** Yes

Defines inclusion and exclusion conditions for number fields.

### Example Usage

Here is an example of how to define conditions for a number field:
```typescript
const numberCondition: NumberValue = {
  $gt: 25
};
```

In this example:
- The query filters records where the `age` field is greater than 25.
