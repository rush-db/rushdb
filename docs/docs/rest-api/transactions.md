---
sidebar_position: 5
---

# Transactions API

RushDB provides a powerful Transactions API that allows you to perform multiple database operations atomically. This ensures data consistency by either committing all operations or rolling back all changes if an error occurs.

## Overview

Transactions in RushDB:
- Allow multiple operations to be executed as a single atomic unit
- Provide ACID (Atomicity, Consistency, Isolation, Durability) guarantees
- Automatically rollback after timeout to prevent hanging transactions
- Can be explicitly committed or rolled back

## Transaction Lifecycle

1. **Create** a transaction to get a transaction ID
2. **Use** the transaction ID in subsequent API requests
3. **Commit** the transaction to make changes permanent, or **Rollback** to discard changes
4. If neither committed nor rolled back within the TTL (Time To Live), the transaction will automatically rollback

## API Endpoints

### Create Transaction

Creates a new transaction and returns a transaction ID.

```http
POST /tx
```

#### Request Body

| Field | Type   | Description |
|-------|--------|-------------|
| `ttl` | Number | Optional. Time to live in milliseconds. Default: 5000ms. Maximum: 30000ms (30 seconds). |

#### Example Request

```json
{
  "ttl": 10000
}
```

#### Response

```json
{
  "success": true,
  "data": {
    "id": "018e5c31-f35a-7000-89cd-850db63a1e77"
  }
}
```

### Get Transaction

Check if a transaction exists.

```http
GET /tx/:txId
```

#### Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `txId`    | String | The transaction ID |

#### Response

```json
{
  "success": true,
  "data": {
    "id": "018e5c31-f35a-7000-89cd-850db63a1e77"
  }
}
```

### Commit Transaction

Commits all changes made within the transaction, making them permanent in the database.

```http
POST /tx/:txId/commit
```

#### Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `txId`    | String | The transaction ID |

#### Response

```json
{
  "success": true,
  "data": {
    "message": "Transaction (018e5c31-f35a-7000-89cd-850db63a1e77) has been successfully committed."
  }
}
```

### Rollback Transaction

Discards all changes made within the transaction.

```http
POST /tx/:txId/rollback
```

#### Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `txId`    | String | The transaction ID |

#### Response

```json
{
  "success": true,
  "data": {
    "message": "Transaction (018e5c31-f35a-7000-89cd-850db63a1e77) has been rolled back."
  }
}
```

## Using Transactions with Other APIs

To use a transaction with other API endpoints, include the transaction ID in the `X-Transaction-Id` header.

### Example

```http
POST /records
Content-Type: application/json
token: YOUR_API_TOKEN
X-Transaction-Id: 018e5c31-f35a-7000-89cd-850db63a1e77

{
  "label": "Person",
  "properties": [
    {
      "name": "name",
      "type": "string",
      "value": "John Doe"
    }
  ]
}
```

## Transaction Timeout

Transactions have a timeout mechanism to prevent hanging transactions:

- Default timeout: 5 seconds (5000ms)
- Maximum timeout: 30 seconds (30000ms)
- If a transaction isn't committed or rolled back within its TTL, it will be automatically rolled back

## Error Handling

The Transaction API may return the following error responses:

| Status Code | Description |
|-------------|-------------|
| 401 | Unauthorized - Authentication required |
| 403 | Forbidden - Insufficient permissions |
| 404 | Not Found - Transaction with the specified ID doesn't exist |
| 500 | Server Error - Processing failed |

### Example Error Response

```json
{
  "success": false,
  "message": "Transaction with ID 018e5c31-f35a-7000-89cd-850db63a1e77 not found",
  "statusCode": 404
}
```

## Best Practices

1. **Keep transactions short**: Long-running transactions can lead to resource contention.
2. **Set appropriate TTL**: Choose a TTL that gives your operations enough time to complete, but not so long that resources are unnecessarily tied up.
3. **Always commit or rollback**: Explicitly commit or rollback transactions rather than relying on automatic timeout.
4. **Error handling**: Implement proper error handling in your client code to rollback transactions if operations fail.
5. **Avoid unnecessary transactions**: For single operations, you don't need to use transactions.

## Transaction Example Workflow

```javascript
// 1. Create a transaction
const createTxResponse = await fetch('https://api.rushdb.com/api/v1/tx', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'token': 'YOUR_API_TOKEN'
  },
  body: JSON.stringify({ ttl: 10000 })
});

const { data: { id: txId } } = await createTxResponse.json();

try {
  // 2. Perform operations within the transaction
  await fetch('https://api.rushdb.com/api/v1/records', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'token': 'YOUR_API_TOKEN',
      'X-Transaction-Id': txId
    },
    body: JSON.stringify({
      label: 'Person',
      properties: [
        { name: 'name', type: 'string', value: 'John Doe' }
      ]
    })
  });

  // 3. Commit the transaction if all operations succeeded
  await fetch(`https://api.rushdb.com/api/v1/tx/${txId}/commit`, {
    method: 'POST',
    headers: {
      'token': 'YOUR_API_TOKEN'
    }
  });
} catch (error) {
  // 4. Rollback the transaction if any operation failed
  await fetch(`https://api.rushdb.com/api/v1/tx/${txId}/rollback`, {
    method: 'POST',
    headers: {
      'token': 'YOUR_API_TOKEN'
    }
  });
  throw error;
}
```
