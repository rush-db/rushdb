---
sidebar_position: 7
---

# StringValue

The `StringValue` type is used to define conditions for string fields in a query.

### Type Definition
```typescript
type StringValue =
  | RequireAtLeastOne<
      Record<'$contains' | '$endsWith' | '$ne' | '$startsWith', string> &
        Record<'$in' | '$nin', Array<string>>
    >
  | string;
```

### Properties

#### $contains, $endsWith, $ne, $startsWith

- **Type:** `string`
- **Optional:** Yes

Defines contains, ends with, not, and starts with conditions for string fields.

#### $in, $nin

- **Type:** `string[]`
- **Optional:** Yes

Defines inclusion and exclusion conditions for string fields.

### Example Usage

Here is an example of how to define conditions for a string field:
```typescript
const stringCondition: StringValue = {
  $startsWith: 'A'
};
```

In this example:
- The query filters records where the `name` field starts with 'A'.
