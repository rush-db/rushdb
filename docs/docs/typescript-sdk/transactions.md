---
sidebar_position: 5
---

# Transactions

The RushDB TypeScript SDK provides a simple but powerful interface for working with database transactions. Transactions allow you to perform multiple database operations atomically, ensuring that either all operations succeed or none do, which helps maintain data consistency.

## Transaction Overview

Transactions in RushDB TypeScript SDK:
- Enable multiple database operations to be executed as a single atomic unit
- Provide ACID (Atomicity, Consistency, Isolation, Durability) guarantees
- Automatically roll back after a timeout to prevent hanging transactions
- Can be explicitly committed or rolled back

## Transaction API

The SDK provides transaction-related methods through the `tx` object:

```typescript
// Access the transaction API
const tx = db.tx;
```

### Begin a Transaction

Creates a new transaction and returns a transaction object:

```typescript
const transaction = await db.tx.begin({
  ttl: 10000 // Optional: Time to live in milliseconds (default: 5000ms, max: 30000ms)
});

// transaction object contains the transaction ID
console.log(transaction.id); // e.g., "018e5c31-f35a-7000-89cd-850db63a1e77"
```

### Get a Transaction

Checks if a transaction exists and retrieves its information:

```typescript
// You can pass either a transaction object or a transaction ID string
const txInfo = await db.tx.get(transaction);
// or
const txInfo = await db.tx.get("018e5c31-f35a-7000-89cd-850db63a1e77");
```

### Commit a Transaction

Commits all changes made within the transaction, making them permanent in the database:

```typescript
// You can pass either a transaction object or a transaction ID string
await transaction.commit()
// or
await db.tx.commit(transaction);
// or
await db.tx.commit("018e5c31-f35a-7000-89cd-850db63a1e77");
```

### Rollback a Transaction

Discards all changes made within the transaction:

```typescript
// You can pass either a transaction object or a transaction ID string
await transaction.rollback()
// or
await db.tx.rollback(transaction);
// or
await db.tx.rollback("018e5c31-f35a-7000-89cd-850db63a1e77");
```

## Using Transactions with API Methods

Most API methods in the RushDB TypeScript SDK accept an optional transaction parameter that allows you to include the operation in a transaction:

```typescript
// Create a transaction
const transaction = await db.tx.begin({ ttl: 10000 });

try {
  // Perform operations as part of the transaction
  const person = await db.records.create({
    label: "Person",
    data: { name: "John Doe", age: 30 }
  }, transaction); // Pass the transaction as the second parameter

  const address = await db.records.create({
    label: "Address",
    data: { street: "123 Main St", city: "New York" }
  }, transaction);

  // Create a relationship between the person and address
  await db.records.attach({
    source: person,
    target: address,
    options: {
      type: "LIVES_AT",
      direction: "out"
    }
  }, transaction);

  // Commit the transaction if all operations succeeded
  await transaction.commit()
  // or
  // await db.tx.commit(transaction);

  console.log("All operations completed successfully!");
} catch (error) {
  // Rollback the transaction if any operation failed
  await transaction.rollback()
  // or
  // await db.tx.rollback(transaction);
  console.error("Transaction failed:", error);
}
```

## Transaction Timeout

Transactions in RushDB have a timeout mechanism to prevent hanging transactions:

- Default timeout: 5 seconds (5000ms)
- Maximum timeout: 30 seconds (30000ms)
- If a transaction is not committed or rolled back within its TTL, it will be automatically rolled back

## Best Practices

1. **Keep transactions short and focused**

   Long-running transactions can lead to resource contention and reduce overall system performance.

2. **Set appropriate TTL**

   Choose a TTL that gives your operations enough time to complete, but not so long that resources are unnecessarily tied up.

3. **Always commit or rollback explicitly**

   Explicitly commit or rollback transactions rather than relying on automatic timeout.

4. **Implement proper error handling**

   Always use try/catch blocks when working with transactions to ensure proper rollback in case of errors.

5. **Use transactions only when necessary**

   For single operations, you don't need to use transactions. Only use transactions when multiple operations need to be atomic.

6. **Be aware of transaction scope**

   Transactions in RushDB are tied to your API token and will affect only the operations performed with that token.

## Example: Complete Transaction Workflow

Here's a complete example showing a transaction workflow for creating a user profile with multiple related records:

```typescript
import RushDB from '@rushdb/javascript-sdk';

// Initialize SDK
const db = new RushDB('YOUR_API_TOKEN');

async function createUserProfile(userData) {
  // Begin a transaction with 15-second TTL
  const transaction = await db.tx.begin({ ttl: 15000 });

  try {
    // Create user record
    const user = await db.records.create({
      label: "User",
      data: {
        username: userData.username,
        email: userData.email
      }
    }, transaction);

    // Create profile record
    const profile = await db.records.create({
      label: "Profile",
      data: {
        firstName: userData.firstName,
        lastName: userData.lastName,
        birthDate: userData.birthDate
      }
    }, transaction);

    // Create address record
    const address = await db.records.create({
      label: "Address",
      data: {
        street: userData.street,
        city: userData.city,
        postalCode: userData.postalCode,
        country: userData.country
      }
    }, transaction);

    // Create relationships
    await db.records.attach({
      source: user,
      target: profile,
      options: {
        type: "HAS_PROFILE",
        direction: "out"
      }
    }, transaction);

    await db.records.attach({
      source: profile,
      target: address,
      options: {
        type: "HAS_ADDRESS",
        direction: "out"
      }
    }, transaction);

    // Commit the transaction
    await transaction.commit()
    // or
    // await db.tx.commit(transaction);

    return {
      success: true,
      user
    };

  } catch (error) {
    // Rollback the transaction on any error
    await transaction.rollback()
    // or
    // await db.tx.rollback(transaction);

    return {
      success: false,
      error: error.message
    };
  }
}

// Usage
createUserProfile({
  username: "johndoe",
  email: "john@example.com",
  firstName: "John",
  lastName: "Doe",
  birthDate: "1990-01-01",
  street: "123 Main St",
  city: "New York",
  postalCode: "10001",
  country: "USA"
}).then(result => {
  if (result.success) {
    console.log("User profile created successfully:", result.user);
  } else {
    console.error("Failed to create user profile:", result.error);
  }
});
```

