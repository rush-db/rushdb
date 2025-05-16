---
sidebar_position: 1
---

# Create Records

RushDB provides multiple ways to create records via its REST API. You can create single [records](../../concepts/records.md), control how your data is processed, and work with transactions for data consistency.

## Overview

The create records endpoints allow you to:
- Create a single record with properties and a label
- Control data type inference and other formatting options
- Create records within transactions for data consistency

All create record endpoints require authentication using a token header.

## Create a Record

```http
POST /api/v1/records
```

This endpoint creates a record with the provided label and payload.

### Request Body

| Field       | Type   | Description |
|-------------|--------|-------------|
| `label`     | String | Label for the new record |
| `payload`   | Object | Object containing property name/value pairs |
| `options`   | Object | Optional configuration parameters |

#### Options Object

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `suggestTypes` | Boolean | `true` | When true, automatically infers data types for properties |
| `castNumberArraysToVectors` | Boolean | `false` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | Boolean | `false` | When true, converts string numbers to number type |
| `capitalizeLabels` | Boolean | `false` | When true, converts all labels to uppercase |

### Example Request

```json
{
  "label": "Person",
  "payload": {
    "name": "John Doe",
    "age": "30",
    "isActive": true,
    "skills": ["JavaScript", "Python", "SQL"],
    "joinDate": "2025-04-23T10:30:00Z",
    "score": 92.5
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
    "skills": "string",
    "joinDate": "datetime",
    "score": "number"
  },
  "name": "John Doe",
  "age": 30,
  "isActive": true,
  "skills": ["JavaScript", "Python", "SQL"],
  "joinDate": "2025-04-23T10:30:00Z",
  "score": 92.5
}
```

## Property-Based Approach

If you need precise control over property types and values, you can use the property-based approach:

```http
POST /api/v1/records
```

### Request Body

| Field       | Type   | Description |
|-------------|--------|-------------|
| `label`     | String | Label for the new record |
| `properties` | Array  | Array of property objects defining record data with explicit types |

#### Property Object

| Field     | Type   | Description |
|-----------|--------|-------------|
| `name`    | String | The property name |
| `type`    | String | The data type for the property ('string', 'number', 'boolean', 'datetime', etc.) |
| `value`   | Any    | The value of the property |
| `valueSeparator` | String | Optional separator to split string values into arrays |

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
    },
    {
      "name": "skills",
      "type": "string",
      "value": "JavaScript,Python,SQL",
      "valueSeparator": ","
    },
    {
      "name": "joinDate",
      "type": "datetime",
      "value": "2025-04-23T10:30:00Z"
    },
    {
      "name": "scores",
      "type": "number",
      "value": "85,90,95",
      "valueSeparator": ","
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
    "isActive": "boolean",
    "skills": "string",
    "joinDate": "datetime",
    "scores": "number"
  },
  "name": "John Doe",
  "age": 30,
  "isActive": true,
  "skills": ["JavaScript", "Python", "SQL"],
  "joinDate": "2025-04-23T10:30:00Z",
  "scores": [85, 90, 95]
}
```

## Working with Multiple Records and Complex Data

For batch operations and working with multiple records or complex data structures, please refer to the [Import Data documentation](./import-data.md). The Import Data API provides dedicated endpoints for:

- Batch creation of multiple records in a single request
- Importing JSON or CSV data
- Creating nested record hierarchies
- Handling arrays of objects as linked records
- Setting relationship types between records
- Processing complex object graphs with automatic type inference

The Import Data API is optimized for performance when working with large datasets or complex structures. It offers additional configuration options and better throughput for batch operations.

## Creating Records in Transactions

To ensure data consistency when creating multiple related [records](../../concepts/records.md), you can use [transactions](../../concepts/transactions.mdx):

1. Create a transaction:
```http
POST /api/v1/tx
```

2. Use the returned transaction ID in your create record requests:
```http
POST /api/v1/records
Authorization: Bearer YOUR_TOKEN
X-Transaction-Id: YOUR_TRANSACTION_ID
```

3. Commit the transaction when all operations are successful:
```http
POST /api/v1/tx/YOUR_TRANSACTION_ID/commit
```

Or roll back if there's an error:
```http
POST /api/v1/tx/YOUR_TRANSACTION_ID/rollback
```

## Data Type Handling

RushDB supports the following [property](../../concepts/properties.md) types:

- `string`: Text values
- `number`: Numeric values
- `boolean`: True/false values
- `null`: Null values
- `datetime`: ISO8601 format strings (e.g., "2025-04-23T10:30:00Z")
- `vector`: Arrays of numbers (when `castNumberArraysToVectors` is true)

When `suggestTypes` is enabled (default in the simplified approach), RushDB automatically infers these types from your data.

When `convertNumericValuesToNumbers` is enabled, string values that represent numbers (e.g., '30') will be converted to their numeric equivalents (e.g., 30).

## Best Practices

- Use the default approach for typical use cases and when automatic type inference is desired
- Use the property-based approach when precise control over [property](../../concepts/properties.md) types is required
- Use the [Import Data API](./import-data.md) for batch operations and creating multiple records
- Use [transactions](../../concepts/transactions.mdx) when creating related records to ensure data consistency
- Validate data on the client side before sending it to the API
