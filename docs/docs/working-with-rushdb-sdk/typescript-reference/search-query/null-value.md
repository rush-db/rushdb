---
sidebar_position: 8
---

# NullValue

The `NullValue` type is used to define conditions for null fields in a query.

### Type Definition
```typescript
type NullValue = RequireAtLeastOne<Record<'$not', null>> | null;
```

### Properties

#### $not

- **Type:** `null`
- **Optional:** Yes

Negates the condition.

### Example Usage

Here is an example of how to define conditions for a null field:
```typescript
const nullCondition: NullValue = {
  $not: null
};
```

In this example:
- The query filters records where the `deletedAt` field is `null`.