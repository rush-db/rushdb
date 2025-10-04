---
sidebar_position: 6
---

# Update Records

Updating [records](../../concepts/records.md) is a crucial operation for maintaining and modifying data within your application. RushDB provides multiple ways to update records, from direct API calls to Model-based abstractions.

This guide covers different approaches to updating records, from the most basic to more advanced patterns.

## Overview

The update record methods in the SDK enable you to:
- Update a single [record](../../concepts/records.md) with new [properties](../../concepts/properties.md)
- Update multiple records in one operation
- Control data type inference and other formatting options
- Update records with precise type control
- Update records within [transactions](../../concepts/transactions.mdx) for data consistency
- Update records using Model abstractions for type safety

## Updating Single Records

There are multiple ways to update records in RushDB. Let's start with the most basic approach using the direct API methods.

### Using RushDB's `update()` Method

The most direct way to update a record is using the API client's `records.update` method:

```typescript
const updatedAuthor = await db.records.update({
  target: 'author_id',
  label: 'AUTHOR',
  data: {
    name: 'John Doe Updated',
    email: 'john.doe.updated@example.com'
  },
  options: {
    suggestTypes: true
  }
});

console.log(updatedAuthor);
/*
{
  __id: 'author_id',
  __label: 'AUTHOR',
  name: 'John Doe Updated',
  email: 'john.doe.updated@example.com'
}
*/
```

#### Parameters

- `target`: The target record to modify (record ID, record instance, or record object)
- `label`: The [label](../../concepts/labels.md)/type for the record
- `data`: The updated data for the record as a flat object
- `options` (optional): Configuration options for record update:
  - `suggestTypes` (boolean, default: `true`): When true, automatically infers data types for [properties](../../concepts/properties.md)
  - `castNumberArraysToVectors` (boolean, default: `false`): When true, converts numeric arrays to vector type
  - `convertNumericValuesToNumbers` (boolean, default: `false`): When true, converts string numbers to number type
- `transaction` (optional): A [transaction](../../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a `DBRecordInstance` containing the updated [record](../../concepts/records.md)

#### Updating Records in Transactions

#### Updating Records in Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const updatedAuthor = await db.records.update({
    target: 'author_id',
    label: 'AUTHOR',
    data: {
      name: 'Jane Smith Updated',
      email: 'jane.smith.updated@example.com'
    }
  }, transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(updatedAuthor);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

### Using RushDB's `set()` Method

While the `update()` method only modifies the specified fields while preserving other existing fields, the `set()` method replaces all fields of a record with the provided values. This is useful when you want to completely reset a record's state.

```typescript
const updatedAuthor = await db.records.set({
  target: 'author_id',
  label: 'AUTHOR',
  data: {
    name: 'John Doe Reset',
    email: 'john.reset@example.com'
    // All other fields will be removed
  },
  options: {
    suggestTypes: true
  }
});

console.log(updatedAuthor);
/*
{
  __id: 'author_id',
  __label: 'AUTHOR',
  name: 'John Doe Reset',
  email: 'john.reset@example.com'
  // Previous fields that were not specified are now gone
}
*/
```

#### Parameters

The parameters for `set()` are identical to those for `update()`:

- `target`: The target record to modify (record ID, record instance, or record object)
- `label`: The [label](../../concepts/labels.md)/type for the record
- `data`: The complete new data for the record as a flat object
- `options` (optional): Configuration options identical to those for `update()`
- `transaction` (optional): A [transaction](../../concepts/transactions.mdx) object or string

#### Returns

- A promise that resolves to a `DBRecordInstance` containing the updated [record](../../concepts/records.md)

#### Difference between `update()` and `set()`

The key difference between these methods:

- `update()`: Performs a partial update, only modifying the specified fields
- `set()`: Performs a complete replacement, removing any fields not specified in the data

#### Setting Records in Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const resetAuthor = await db.records.set({
    target: 'author_id',
    label: 'AUTHOR',
    data: {
      name: 'Reset Author',
      email: 'reset@example.com'
    }
  }, transaction);

  await transaction.commit();
  console.log(resetAuthor);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

### Property-Based Approach for Precise Type Control

When you need precise control over property types, you can use the property-based approach by passing an array of `PropertyDraft` objects instead of a flat data object:

```typescript
const updatedAuthor = await db.records.update({
  target: 'author_id',
  label: 'AUTHOR',
  data: [
    {
      name: 'name',
      type: 'string',
      value: 'John Doe Updated'
    },
    {
      name: 'joinDate',
      type: 'datetime',
      value: '2025-05-15T14:30:00Z'
    }
  ]
});

console.log(updatedAuthor);
/*
{
  __id: 'author_id',
  __label: 'AUTHOR',
  name: 'John Doe Updated',
  joinDate: '2025-05-15T14:30:00Z'
}
*/
```

#### Property Draft Object Properties

Each property draft object supports the following properties:

| Property | Type | Description |
|----------|------|-------------|
| `name` | `string` | The property name |
| `type` | `string` | The data type ('string', 'number', 'boolean', 'datetime', etc.) |
| `value` | `any` | The property value |
| `valueSeparator` | `string` (optional) | Separator to split string values into arrays |

## Updating Multiple Records

## Updating Multiple Records

When you need to update multiple records in a single operation, you can use a combination of `find` and `update` methods.

### Using Find and Update Pattern

```typescript
// Find all authors with a specific name
const authorsToUpdate = await db.records.find({
  labels: ['AUTHOR'],
  where: { name: 'John Doe' }
});

// Update each record individually
for (const author of authorsToUpdate.data) {
  await db.records.update({
    target: author.__id,
    label: 'AUTHOR',
    data: { name: 'John Doe Updated' }
  });
}

console.log(authorsToUpdate);
/*
{
  data: [
    {
      __id: 'author_id_1',
      __label: 'AUTHOR',
      name: 'John Doe Updated',
      email: 'john.doe@example.com'
    },
    {
      __id: 'author_id_2',
      __label: 'AUTHOR',
      name: 'John Doe Updated',
      email: 'john.doe@example.com'
    }
  ],
  total: 2
}
*/
```

#### Updating Multiple Records in Transactions

```typescript
// Find records matching criteria
const postsToUpdate = await db.records.find({
  labels: ['POST'],
  where: { rating: { $lt: 5 } }
});

const transaction = await db.tx.begin();
try {
  // Update each record within the transaction
  for (const post of postsToUpdate.data) {
    await db.records.update({
      target: post.__id,
      label: 'POST',
      data: { rating: 5 }
    }, transaction);
  }
  await transaction.commit();
  console.log(postsToUpdate);
  /*
  {
    data: [
      {
        __id: 'post_id_1',
        __label: 'POST',
        created: '2023-01-02T00:00:00Z',
        title: 'Blog Post Title 1',
        content: 'This is a blog post content.',
        rating: 5
      },
      {
        __id: 'post_id_2',
        __label: 'POST',
        created: '2023-01-03T00:00:00Z',
        title: 'Blog Post Title 2',
        content: 'This is another blog post content.',
        rating: 5
      }
    ],
    total: 2
  }
  */
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

## Updating Records with Models

## Updating Records with Models

The recommended approach for structured applications is to use RushDB's [Models](../models.md). Models provide type safety, validation, and a more intuitive API for working with records.

We'll use the following model definitions for these examples:

```typescript
const AuthorRepo = new Model('author', {
  name: { type: 'string' },
  email: { type: 'string', unique: true }
});
```

### Using Model's `update` Method

The `update` method on a model updates a single record.

#### Signature
```typescript
update(
  target: DBRecordTarget,
  record: Partial<InferSchemaTypesWrite<S>>,
  transaction?: Transaction | string
): Promise<DBRecordInstance<S>>;
```

#### Parameters

- `target`: The target record to update (ID string, record instance, or record object)
- `record`: An object containing the fields to update and their new values
- `transaction` (optional): A [transaction](../../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a `DBRecordInstance` containing the updated [record](../../concepts/records.md)

#### Example

```typescript
const updatedAuthor = await AuthorRepo.update('author_id', {
  name: 'John Doe Updated'
});

console.log(updatedAuthor);
/*
{
  __id: 'author_id',
  __label: 'author',
  name: 'John Doe Updated',
  email: 'john.doe@example.com'
}
*/
```

#### Using with Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const updatedAuthor = await AuthorRepo.update('author_id', {
    name: 'Jane Smith Updated'
  }, transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(updatedAuthor);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

### Using Model's `set` Method

The `set` method on a model completely replaces a record's data with the new values.

#### Signature
```typescript
set(
  target: DBRecordTarget,
  record: InferSchemaTypesWrite<S>,
  transaction?: Transaction | string
): Promise<DBRecordInstance<S>>;
```

#### Parameters

- `target`: The target record to modify (ID string, record instance, or record object)
- `record`: An object containing all the fields to set for the record (fields not included will be removed)
- `transaction` (optional): A [transaction](../../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a `DBRecordInstance` containing the modified [record](../../concepts/records.md)

#### Example

```typescript
const resetAuthor = await AuthorRepo.set('author_id', {
  name: 'John Doe Reset',
  email: 'john.reset@example.com'
  // All fields not specified will be removed
});

console.log(resetAuthor);
/*
{
  __id: 'author_id',
  __label: 'author',
  name: 'John Doe Reset',
  email: 'john.reset@example.com'
}
*/
```

#### Difference between `update` and `set`

- `update`: Performs a partial update, preserving fields not specified in the update data
- `set`: Completely replaces all record data with the new values, removing any fields not specified

#### Using with Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const resetAuthor = await AuthorRepo.set('author_id', {
    name: 'Complete Reset',
    email: 'reset@example.com'
  }, transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(resetAuthor);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

## Complex Example with Multiple Updates in a Transaction

## Complex Example with Multiple Updates in a Transaction

In this example, we'll update an `Author` and a `Post` within the same transaction. This ensures that either both updates succeed, or both are rolled back in case of an error.

```typescript
const transaction = await db.tx.begin();
try {
  // Update the author
  const updatedAuthor = await AuthorRepo.update('author_id', {
    name: 'Updated Author Name'
  }, transaction);

  // Update the post
  const updatedPost = await PostRepo.update('post_id', {
    title: 'Updated Post Title',
    content: 'Updated content for the post.',
    rating: 4.8
  }, transaction);

  await transaction.commit();
  console.log(updatedAuthor);
  console.log(updatedPost);
  /*
  {
    __id: 'author_id',
    __label: 'author',
    name: 'Updated Author Name',
    email: 'john.doe@example.com'
  }

  {
    __id: 'post_id',
    __label: 'post',
    created: '2023-01-02T00:00:00Z',
    title: 'Updated Post Title',
    content: 'Updated content for the post.',
    rating: 4.8
  }
  */
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

## Best Practices for Updating Records

1. **Use Models for Structured Applications**
   - Models provide type safety, validation, and better organization
   - They enforce schema consistency across your application

2. **Use Transactions for Related Operations**
   - When updating multiple records that are related, use [transactions](../../concepts/transactions.mdx)
   - Transactions ensure data consistency and allow rollback if operations fail

3. **Handle Uniqueness Constraints**
   - Models automatically check uniqueness before updating records
   - Handle `UniquenessError` exceptions appropriately

4. **Partial Updates vs. Complete Replacement**
   - Use the `update` method for partial updates when you only need to change specific fields
   - Use the `set` method when you want to completely replace a record's data
   - This minimizes network traffic and avoids unintended side effects

5. **Consider Validation**
   - Validate your data on the client side before sending updates
   - This improves performance and provides a better user experience

6. **Choose the Right Data Type Control Approach**
   - Use the flat object approach for most cases where automatic type inference is sufficient
   - Use the property-based approach with `PropertyDraft` objects when you need precise control over types

## Data Type Handling

RushDB supports the same property types for updates as it does for creating records:

- `string`: Text values
- `number`: Numeric values
- `boolean`: True/false values
- `null`: Null values
- `datetime`: ISO8601 format strings (e.g., "2025-04-23T10:30:00Z")
- `vector`: Arrays of numbers (when `castNumberArraysToVectors` is true)

When `suggestTypes` is enabled (default), RushDB automatically infers these types from your data.

When `convertNumericValuesToNumbers` is enabled, string values that represent numbers (e.g., '30') will be converted to their numeric equivalents (e.g., 30).

## Conclusion

Updating records in RushDB can be done through direct API calls or through the Model abstraction. While direct API calls offer flexibility for dynamic or ad-hoc operations, using Models is recommended for most applications due to their type safety, validation capabilities, and more intuitive API.

For more advanced record operations, see the other guides in this section:
- [Get Records](./get-records.md) - Retrieve records from the database
- [Create Records](./create-records.md) - Create new records
- [Delete Records](./delete-records.md) - Remove records from the database
- [Import Data](./import-data.md) - Import data in bulk
