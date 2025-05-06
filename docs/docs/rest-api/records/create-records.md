---
sidebar_position: 1
---

# Create Records

RushDB provides multiple ways to create records via its REST API. You can create single records, batch create multiple records, and even control how your data is processed and stored.

## Overview

The create records endpoints allow you to:
- Create a single record with properties and a label
- Create multiple records in a batch operation
- Control data type inference and other formatting options
- Create records within transactions for data consistency

All create record endpoints require authentication using a token header.

## Create a Single Record

```http
POST /records
```

This endpoint creates a single record with the provided label and properties.

### Request Body

| Field       | Type   | Description |
|-------------|--------|-------------|
| `label`     | String | Label for the new record |
| `properties` | Array  | Array of property objects defining record data |

#### Property Object

| Field     | Type   | Description |
|-----------|--------|-------------|
| `name`    | String | The property name |
| `type`    | String | The data type for the property ('string', 'number', 'boolean', etc.) |
| `value`   | Any    | The value of the property |

### Example Request

```json
{
  "label": "Person",
  "properties": [
    {
      "name": "name",
      "type": "string",
      "value": "John Doe"
    },
    {
      "name": "age",
      "type": "number",
      "value": 30
    },
    {
      "name": "isActive",
      "type": "boolean",
      "value": true
    }
  ]
}
```

### Response

```json
{
  "__id": "018e4c71-f35a-7000-89cd-850db63a1e77",
  "__label": "Person",
  "__proptypes": {
    "name": "string",
    "age": "number",
    "isActive": "boolean"
  },
  "name": "John Doe",
  "age": 30,
  "isActive": true
}
```

## Create Records with Simplified Payload

You can also create records using a simplified approach with a flat object.

### Request Body

| Field       | Type   | Description |
|-------------|--------|-------------|
| `label`     | String | Label for the new record |
| `payload`   | Object | Flat object containing property name/value pairs |
| `options`   | Object | Optional configuration parameters |

#### Options Object

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `suggestTypes` | Boolean | `true` | When true, automatically infers data types for properties |
| `castNumberArraysToVectors` | Boolean | `false` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | Boolean | `false` | When true, converts string numbers to number type |
| `capitalizeLabels` | Boolean | `false` | When true, converts all labels to uppercase |
| `returnResult` | Boolean | `true` | When true, returns created record in response |

### Example Request

```json
{
  "label": "Person",
  "payload": {
    "name": "John Doe",
    "age": "30",
    "isActive": true,
    "skills": ["JavaScript", "Python", "SQL"]
  },
  "options": {
    "suggestTypes": true,
    "convertNumericValuesToNumbers": true
  }
}
```

### Response

```json
{
  "__id": "018e4c71-f35a-7000-89cd-850db63a1e77",
  "__label": "Person",
  "__proptypes": {
    "name": "string",
    "age": "number",
    "isActive": "boolean",
    "skills": "string[]"
  },
  "name": "John Doe",
  "age": 30,
  "isActive": true,
  "skills": ["JavaScript", "Python", "SQL"]
}
```

## Create Multiple Records (Batch)

```http
POST /records/import/json
```

This endpoint allows you to create multiple records in a single request, which is more efficient than making separate requests.

### Request Body

| Field     | Type   | Description |
|-----------|--------|-------------|
| `label`   | String | Base label for all records |
| `payload` | Array  | Array of objects, each representing a record to create |
| `options` | Object | Optional configuration parameters (same as single record creation) |

### Example Request

```json
{
  "label": "Product",
  "payload": [
    {
      "name": "Laptop",
      "price": 1200,
      "inStock": true
    },
    {
      "name": "Smartphone",
      "price": 800,
      "inStock": false
    },
    {
      "name": "Headphones",
      "price": 150,
      "inStock": true
    }
  ],
  "options": {
    "suggestTypes": true,
    "returnResult": true
  }
}
```

### Response

```json
[
  {
    "__id": "018e4c71-f35a-7000-89cd-850db63a1e77",
    "__label": "Product",
    "__proptypes": {
      "name": "string",
      "price": "number",
      "inStock": "boolean"
    },
    "name": "Laptop",
    "price": 1200,
    "inStock": true
  },
  {
    "__id": "018e4c71-f35a-7000-89cd-850db63a1e78",
    "__label": "Product",
    "__proptypes": {
      "name": "string",
      "price": "number",
      "inStock": "boolean"
    },
    "name": "Smartphone",
    "price": 800,
    "inStock": false
  },
  {
    "__id": "018e4c71-f35a-7000-89cd-850db63a1e79",
    "__label": "Product",
    "__proptypes": {
      "name": "string",
      "price": "number",
      "inStock": "boolean"
    },
    "name": "Headphones",
    "price": 150,
    "inStock": true
  }
]
```

## Create Nested Records

The batch creation API also supports nested records. Records will be created for each nested object with appropriate relationships.

### Example Request with Nested Records

```json
{
  "label": "Order",
  "payload": {
    "orderNumber": "ORD-12345",
    "total": 1350,
    "customer": {
      "name": "Jane Smith",
      "email": "jane@example.com",
      "address": {
        "street": "123 Main St",
        "city": "New York",
        "zipcode": "10001"
      }
    },
    "items": [
      {
        "productName": "Laptop",
        "quantity": 1,
        "price": 1200
      },
      {
        "productName": "Mouse",
        "quantity": 1,
        "price": 150
      }
    ]
  },
  "options": {
    "suggestTypes": true,
    "relationshipType": "HAS"
  }
}
```

In this example:
- A main "Order" record will be created
- A "customer" record with nested "address" record will be created
- Two "items" records will be created
- All will be linked with the relationship type "HAS"

## Upsert Records

```http
PUT /records
```

This endpoint creates a new record if it doesn't exist or updates an existing one if it matches specified criteria.

### Request Body

| Field     | Type   | Description |
|-----------|--------|-------------|
| `label`   | String | Label for the record |
| `properties` | Array  | Array of property objects defining record data |
| `matchBy` | Array  | Array of property names to match for upserting |

### Example Request

```json
{
  "label": "User",
  "properties": [
    {
      "name": "email",
      "type": "string",
      "value": "john@example.com"
    },
    {
      "name": "name",
      "type": "string",
      "value": "John Doe"
    },
    {
      "name": "lastLogin",
      "type": "datetime",
      "value": "2025-04-23T10:30:00Z"
    }
  ],
  "matchBy": ["email"]
}
```

This creates a User record if no record with email "john@example.com" exists, or updates the matching record.

## Creating Records in Transactions

To ensure data consistency when creating multiple related records, you can use transactions:

1. Create a transaction:
```http
POST /tx
```

2. Use the returned transaction ID in your create record requests:
```http
POST /records
Authorization: Bearer YOUR_TOKEN
X-Transaction-Id: YOUR_TRANSACTION_ID
```

3. Commit the transaction when all operations are successful:
```http
POST /tx/YOUR_TRANSACTION_ID/commit
```

Or roll back if there's an error:
```http
POST /tx/YOUR_TRANSACTION_ID/rollback
```

## Data Type Handling

When `suggestTypes` is enabled, RushDB will infer the following types:

- `string`: Text values
- `number`: Number values (& numeric values when `convertNumericValuesToNumbers` is true)
- `boolean`: True/false values
- `null`: Null values
- `vector`: Arrays of numbers (when `castNumberArraysToVectors` is true)
- `datetime`: ISO8601 format strings (e.g., "2025-04-23T10:30:00Z")
- Arrays: Supported as `string[]`, `number[]`, etc.

## Error Handling

The create API may return the following error responses:

| Status Code | Description |
|-------------|-------------|
| 400 | Bad Request - Invalid payload structure |
| 401 | Unauthorized - Authentication required |
| 402 | Payment Required - Create would exceed plan limits |
| 403 | Forbidden - Insufficient permissions |
| 500 | Server Error - Processing failed |

### Example Error Response

```json
{
  "success": false,
  "message": "Invalid property type provided for field 'age'",
  "statusCode": 400
}
```

## Best Practices

- Use batch operations when creating multiple records for better performance
- Set appropriate labels to help organize and query records
- Use transactions when creating related records to ensure data consistency
- Validate data on the client side before sending it to the API
- Handle errors gracefully in your application
