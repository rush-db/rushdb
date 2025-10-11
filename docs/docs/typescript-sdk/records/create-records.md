---
sidebar_position: 1
---

# Create Records

Creating [records](../../concepts/records.md) is a fundamental operation when working with any data-driven application. RushDB provides multiple ways to create records, from direct API calls to Model-based abstractions.

This guide covers different approaches to creating records, from the most basic to more advanced patterns.

## Overview

The create record methods in the SDK enable you to:
- Create a single [record](../../concepts/records.md) with [properties](../../concepts/properties.md) and a [label](../../concepts/labels.md)
- Create multiple records in one operation
- Control data type inference and other formatting options
- Create records with precise type control
- Create records within [transactions](../../concepts/transactions.mdx) for data consistency
- Create records using Model abstractions for type safety

## Creating Single Records

There are multiple ways to create records in RushDB. Let's start with the most basic approach using the direct API methods.

### Using RushDB's `create()` Method

The most direct way to create a record is using the API client's `records.create` method:

```typescript
const newAuthor = await db.records.create({
  label: 'AUTHOR',
  data: {
    name: 'John Doe',
    email: 'john.doe@example.com'
  },
  options: {
    suggestTypes: true
  }
});

console.log(newAuthor);
/*
{
  __id: 'generated_id',
  __label: 'AUTHOR',
  name: 'John Doe',
  email: 'john.doe@example.com'
}
*/
```

#### Parameters

- `label`: The [label](../../concepts/labels.md)/type for the record
- `data`: The data for the record as a flat object
- `options` (optional): Configuration options for record creation:
  - `suggestTypes` (boolean, default: `true`): When true, automatically infers data types for [properties](../../concepts/properties.md)
  - `castNumberArraysToVectors` (boolean, default: `false`): When true, converts numeric arrays to vector type
  - `convertNumericValuesToNumbers` (boolean, default: `false`): When true, converts string numbers to number type
- `transaction` (optional): A [transaction](../../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a `DBRecordInstance` containing the created [record](../../concepts/records.md)

#### Creating Records in Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const newAuthor = await db.records.create({
    label: 'AUTHOR',
    data: {
      name: 'Jane Smith',
      email: 'jane.smith@example.com'
    }
  }, transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(newAuthor);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

### Property-Based Approach for Precise Type Control

When you need precise control over property types, you can use the property-based approach by passing an array of `PropertyDraft` objects instead of a flat data object:

```typescript
const newAuthor = await db.records.create({
  label: 'AUTHOR',
  data: [
    {
      name: 'name',
      type: 'string',
      value: 'John Doe'
    },
    {
      name: 'age',
      type: 'number',
      value: 42
    },
    {
      name: 'isActive',
      type: 'boolean',
      value: true
    },
    {
      name: 'tags',
      type: 'string',
      value: 'fiction,sci-fi,bestseller',
      valueSeparator: ','
    },
    {
      name: 'scores',
      type: 'number',
      value: '85,90,95',
      valueSeparator: ','
    },
    {
      name: 'joinDate',
      type: 'datetime',
      value: '2025-04-23T10:30:00Z'
    }
  ]
});

console.log(newAuthor);
/*
{
  __id: 'generated_id',
  __label: 'AUTHOR',
  __proptypes: {
    name: 'string',
    age: 'number',
    isActive: 'boolean',
    tags: 'string',
    scores: 'number',
    joinDate: 'datetime'
  },
  name: 'John Doe',
  age: 42,
  isActive: true,
  tags: ['fiction', 'sci-fi', 'bestseller'],
  scores: [85, 90, 95],
  joinDate: '2025-04-23T10:30:00Z'
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

## Creating Multiple Records

When you need to create multiple flat records (CSV-like rows) in a single operation, use the `records.createMany` method. For nested or complex JSON, use `records.importJson`.

### Using RushDB's `createMany()` Method (flat rows only)

```typescript
const authors = await db.records.createMany({
  label: 'AUTHOR',
  data: [
    { name: 'Alice Johnson', email: 'alice.johnson@example.com' },
    { name: 'Bob Brown', email: 'bob.brown@example.com' }
  ],
  options: {
    suggestTypes: true
  }
});

console.log(authors);
/*
{
  data: [
    {
      __id: 'generated_id_1',
      __label: 'AUTHOR',
      name: 'Alice Johnson',
      email: 'alice.johnson@example.com'
    },
    {
      __id: 'generated_id_2',
      __label: 'AUTHOR',
      name: 'Bob Brown',
      email: 'bob.brown@example.com'
    }
  ],
  total: 2
}
*/
```

#### Parameters

- `label`: The [label](../../concepts/labels.md)/type for all records
- `data`: An object or array of objects, each a flat record (no nested objects/arrays)
- `options` (optional): Configuration options for record creation:
  - `suggestTypes` (boolean, default: `true`): When true, automatically infers data types for [properties](../../concepts/properties.md)
  - `castNumberArraysToVectors` (boolean, default: `false`): When true, converts numeric arrays to vector type
  - `convertNumericValuesToNumbers` (boolean, default: `false`): When true, converts string numbers to number type
  - `capitalizeLabels` (bool): When true, converts all labels to uppercase
  - `relationshipType` (str): Default relationship type between nodes
  - `returnResult` (bool, default: `false`): When true, returns imported records in response
  - Throws if any record contains nested objects/arrays. Use `records.importJson` for that.

  ### Using RushDB's `importJson()` Method (nested JSON)

  Use `importJson` for nested objects, arrays of nested objects, or hash-map like payloads.

  Signature:

  ```ts
  db.records.importJson({ data, label?: string, options?: ImportOptions }, tx?)
  ```

  Behavior:
  - If `label` is provided, it's used for the import.
  - If `label` is omitted, the input must be an object with a single top-level key whose name becomes the label, e.g. `{ ITEM: [ {...}, {...} ] }`.
  - If `label` is omitted and the object has multiple top-level keys (e.g. `{ some: 'key', data: 1, nested: { level: 2 } }`), an error is thrown.

  Multiple top-level keys:
  - Without `label`: not allowed — importJson requires a single top-level key to infer the label and will throw.
  - With `label`: allowed — the provided `label` becomes the root label; the multiple keys are treated as nested structure under that root.
  - If you want each top-level key to become its own label root, call `importJson` separately per key or pass single-key objects per call.

  Examples:
  - OK (label inferred):
    ```json
    { "ITEM": [ { /*...*/ }, { /*...*/ } ] }
    ```
  - OK (label inferred with object):
    ```json
    { "ITEM": { /*...*/ } }
    ```
  - OK with explicit label (multiple top-level keys):
    ```json
    { "ITEM": { /*...*/ }, "PRODUCT": { /*...*/ } }
    ```
    Call as: `db.records.importJson({ label: 'INVENTORY', data: { ITEM: {...}, PRODUCT: {...} } })`
  - Will throw without label (multiple top-level keys):
    ```json
    { "ITEM": { /*...*/ }, "PRODUCT": { /*...*/ } }
    ```
  - Will throw without label (mixed keys):
    ```json
    { "ITEM": { /*...*/ }, "notNestedProp": "12" }
    ```
- `transaction` (optional): A [transaction](../../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a `DBRecordsArrayInstance` containing the created [records](../../concepts/records.md)

#### Creating Multiple Records in Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const authors = await db.records.createMany({
    label: 'AUTHOR',
    data: [
      { name: 'Charlie Green', email: 'charlie.green@example.com' },
      { name: 'David Blue', email: 'david.blue@example.com' }
    ]
  }, transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(authors);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

## Creating Records with Models

The recommended approach for structured applications is to use RushDB's [Models](../models.md). Models provide type safety, validation, and a more intuitive API for working with records.

We'll use the following model definitions for these examples:

```typescript
const AuthorRepo = new Model('author', {
  name: { type: 'string' },
  email: { type: 'string', unique: true }
});
```

### Using Model's `create` Method

The `create` method on a model creates a single record.

#### Signature
```typescript
create(
  record: InferSchemaTypesWrite<S>,
  transaction?: Transaction | string
): Promise<DBRecordInstance<S>>;
```

#### Parameters

- `record`: An object that adheres to the schema defined for the model
- `transaction` (optional): A [transaction](../../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a `DBRecordInstance` containing the created [record](../../concepts/records.md)

#### Example

```typescript
const newAuthor = await AuthorRepo.create({
  name: 'John Doe',
  email: 'john.doe@example.com'
});

console.log(newAuthor);
/*
{
  data: {
    __id: 'generated_id',
    __label: 'author',
    name: 'John Doe',
    email: 'john.doe@example.com'
  }
}
*/
```

#### Using with Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const newAuthor = await AuthorRepo.create({
    name: 'Jane Smith',
    email: 'jane.smith@example.com'
  }, transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(newAuthor);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

### Using Model's `createMany` Method

The `createMany` method on a model creates multiple records in a single operation.

#### Signature
```typescript
createMany(
  records: Array<InferSchemaTypesWrite<S>>,
  transaction?: Transaction | string
): Promise<DBRecordsArrayInstance<S>>;
```

#### Parameters

- `records`: An array of objects, each adhering to the schema defined for the model
- `transaction` (optional): A transaction object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a `DBRecordsArrayInstance` containing the created records

#### Example

```typescript
const authors = await AuthorRepo.createMany([
  { name: 'Alice Johnson', email: 'alice.johnson@example.com' },
  { name: 'Bob Brown', email: 'bob.brown@example.com' }
]);

console.log(authors);
/*
{
  data: [
    {
      __id: 'generated_id_1',
      __label: 'author',
      name: 'Alice Johnson',
      email: 'alice.johnson@example.com'
    },
    {
      __id: 'generated_id_2',
      __label: 'author',
      name: 'Bob Brown',
      email: 'bob.brown@example.com'
    }
  ],
  total: 2
}
*/
```

#### Using with Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const authors = await AuthorRepo.createMany([
    { name: 'Charlie Green', email: 'charlie.green@example.com' },
    { name: 'David Blue', email: 'david.blue@example.com' }
  ], transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(authors);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

## Best Practices for Creating Records

1. **Use Models for Structured Applications**
   - Models provide type safety, validation, and better organization
   - They enforce schema consistency across your application

2. **Use Transactions for Related Operations**
   - When creating multiple records that are related, use [transactions](../../concepts/transactions.mdx)
   - Transactions ensure data consistency and allow rollback if operations fail

3. **Handle Uniqueness Constraints**
   - Models automatically check uniqueness before creating records
   - Handle `UniquenessError` exceptions appropriately

4. **Leverage Batch Operations**
   - Use `createMany` for better performance when creating multiple records
   - It minimizes network requests and database overhead

5. **Consider Default Values**
   - Define default values in your schema to reduce repetitive code
   - Default values can be static or derived from functions (like timestamps)

6. **Choose the Right Data Type Control Approach**
   - Use the flat object approach for most cases where automatic type inference is sufficient
   - Use the property-based approach with `PropertyDraft` objects when you need precise control over types

## Data Type Handling

RushDB supports the following property types:

- `string`: Text values
- `number`: Numeric values
- `boolean`: True/false values
- `null`: Null values
- `datetime`: ISO8601 format strings (e.g., "2025-04-23T10:30:00Z")
- `vector`: Arrays of numbers (when `castNumberArraysToVectors` is true)

When `suggestTypes` is enabled (default), RushDB automatically infers these types from your data.

When `convertNumericValuesToNumbers` is enabled, string values that represent numbers (e.g., '30') will be converted to their numeric equivalents (e.g., 30).

For more complex data import operations, refer to the [Import Data](./import-data.md) documentation.

## Conclusion

Creating records in RushDB can be done through direct API calls or through the Model abstraction. While direct API calls offer flexibility for dynamic or ad-hoc operations, using Models is recommended for most applications due to their type safety, validation capabilities, and more intuitive API.

For more advanced record operations, see the other guides in this section:
- [Get Records](./get-records.md) - Retrieve records from the database
- [Update Records](./update-records.md) - Modify existing records
- [Delete Records](./delete-records.md) - Remove records from the database
- [Import Data](./import-data.md) - Import data in bulk

