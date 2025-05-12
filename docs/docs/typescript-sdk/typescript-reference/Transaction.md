# Transaction

The `Transaction` class represents an active database transaction in RushDB. It provides methods for committing or rolling back the transaction and includes the unique transaction identifier.

## Overview

In RushDB, a transaction allows you to execute multiple database operations atomically, ensuring that either all operations succeed or all operations are rolled back if an error occurs. The `Transaction` class encapsulates the logic for managing these database transactions.

## Properties

| Property | Type   | Description |
|----------|--------|-------------|
| `id`     | string | The unique identifier for this transaction. This ID is used internally when making API calls within the transaction context. |

## Methods

### `commit()`

Commits all operations performed within this transaction, making the changes permanent in the database.

```typescript
async commit(): Promise<ApiResponse<{ message: string }>>
```

#### Returns
- `Promise<ApiResponse<{ message: string }>>`: A promise resolving to the API response with a success message.

#### Example
```typescript
const transaction = await db.tx.begin();
// Perform database operations...
const result = await transaction.commit();
console.log(result.data.message); // "Transaction (id) has been successfully committed."
```

### `rollback()`

Rolls back all operations performed within this transaction, discarding all changes.

```typescript
async rollback(): Promise<ApiResponse<{ message: string }>>
```

#### Returns
- `Promise<ApiResponse<{ message: string }>>`: A promise resolving to the API response with a rollback confirmation message.

#### Example
```typescript
const transaction = await db.tx.begin();
// Perform database operations...
try {
  // Some operation failed
  const result = await transaction.rollback();
  console.log(result.data.message); // "Transaction (id) has been rolled back."
} catch (error) {
  console.error("Error rolling back transaction:", error);
}
```

## Usage with SDK Methods

Many SDK methods accept a `Transaction` instance as their last parameter, allowing you to include the operation within the transaction:

```typescript
// Start a transaction
const transaction = await db.tx.begin({ ttl: 10000 });

try {
  // Create a record within the transaction
  const person = await db.records.create({
    label: "Person",
    data: { name: "Jane Smith", age: 28 }
  }, transaction);

  // Update a record within the same transaction
  await db.records.update({
    target: person,
    label: "Person",
    data: { age: 29 }
  }, transaction);

  // Commit the transaction to make changes permanent
  await transaction.commit();
} catch (error) {
  // If any operation fails, roll back the entire transaction
  await transaction.rollback();
  throw error;
}
```

## Inheritance

The `Transaction` class extends `RestApiProxy`, which provides access to the underlying API methods through the `apiProxy` property.

## See Also

- [Transactions API Documentation](/typescript-sdk/transactions)
- [RushDB Class Reference](/typescript-sdk/typescript-reference/RushDB)
