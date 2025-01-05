---
sidebar_position: 4
---

# BooleanValue

The `BooleanValue` type is used to define conditions for boolean fields in a query.

### Type Definition
```typescript
type BooleanValue = RequireAtLeastOne<Record<'$not', boolean>> | boolean;
```

### Properties

#### $not

- **Type:** `boolean`
- **Optional:** Yes

Negates the condition.

### Example Usage

Here is an example of how to define conditions for a boolean field:
```typescript
const booleanCondition: BooleanValue = {
  $not: true
};
```

In this example:
- The query filters records where the `isActive` field is `true`.
