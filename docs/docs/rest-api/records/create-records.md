---
sidebar_position: 1
---

# Create Records

RushDB provides multiple ways to create records via its REST API. You can create single [records](../../concepts/records.md), control how your data is processed, and work with transactions for data consistency.

## Overview

The create records endpoints allow you to:
- Create a single record with properties and a label
- Upsert records (create or update based on matching criteria)
- Control data type inference and other formatting options
- Create records within transactions for data consistency

All create record endpoints require authentication using a token header.

## Create / Upsert a Record

```http
POST /api/v1/records
```

This endpoint creates a record with the provided label and data. If `options.mergeBy` and/or `options.mergeStrategy` are supplied, it performs an upsert (create-or-update) instead of a plain create.

### Request Body

| Field       | Type   | Description |
|-------------|--------|-------------|
| `label`     | String | Label for the new record |
| `data`   | Object | Object containing property name/value pairs |
| `options`   | Object | Optional configuration parameters (including upsert) |

#### Options Object (Create & Upsert)

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `suggestTypes` | Boolean | `true` | **Default is `true`** - Automatically infers data types for properties. To disable type inference and store all values as strings, explicitly set to `false` |
| `castNumberArraysToVectors` | Boolean | `false` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | Boolean | `false` | When true, converts string numbers to number type |
| `mergeBy` | Array of Strings | `[]` / omitted | Upsert match keys. If omitted and `mergeStrategy` present, all incoming keys are used. Empty array means use all keys. |
| `mergeStrategy` | String | `'append'` | Upsert behavior when match found: `'append'` (add/update, keep others) or `'rewrite'` (replace all existing properties). Providing either this or `mergeBy` triggers upsert flow. |

:::info Default Behavior
By default, `suggestTypes` is set to `true` for all write operations (create, upsert, import). This means RushDB automatically infers data types from your values. To store all properties as strings without type inference, you must explicitly set `suggestTypes: false` in the options.
:::

### Example Create Request (no upsert)

```json
{
  "label": "Person",
  "data": {
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

### Response (Create)

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

## Upserting Via POST /api/v1/records

Previously upsert required a dedicated endpoint (`/records/upsert`). Upsert is now unified into `POST /api/v1/records`. The legacy endpoint will continue to function for backward compatibility but new integrations should prefer the unified create/upsert endpoint.

### Upsert Request Body

| Field       | Type   | Description |
|-------------|--------|-------------|
| `label`     | String | Optional label for the record |
| `data`   | Object | Object containing property name/value pairs |
| `options`   | Object | Configuration parameters including merge behavior (see Options table) |

#### Upsert-Specific Options Highlights

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `mergeBy` | Array of Strings | `[]` / omitted | Property names to match on. Empty or omitted with mergeStrategy provided falls back to all incoming keys. |
| `mergeStrategy` | String | `'append'` | `'append'` adds/updates provided properties; `'rewrite'` replaces all properties (unmentioned ones removed). |
| `suggestTypes` | Boolean | `true` | **Default is `true`** - Automatically infers data types for properties. To disable type inference and store all values as strings, explicitly set to `false` |
| `castNumberArraysToVectors` | Boolean | `false` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | Boolean | `false` | When true, converts string numbers to number type |

:::info Default Behavior
By default, `suggestTypes` is set to `true` for all write operations. This means RushDB automatically infers data types from your values during upsert operations. To store all properties as strings without type inference, you must explicitly set `suggestTypes: false` in the options.
:::

### Merge Strategies

#### Append Strategy
When using `mergeStrategy: 'append'`, the upsert operation:
- Adds new properties from the incoming data
- Updates existing properties with new values
- Preserves existing properties not included in the incoming data

#### Rewrite Strategy
When using `mergeStrategy: 'rewrite'`, the upsert operation:
- Replaces all existing properties with the incoming data
- Removes properties not included in the incoming data
- Essentially performs a complete replacement of the record's properties

### Example Upsert Requests

#### Create or Update with Append Strategy (primary key: sku)

```json
{
  "label": "Product",
  "data": {
    "sku": "SKU-001",
    "name": "Laptop Pro",
    "price": 1299.99,
    "category": "Electronics"
  },
  "options": {
    "mergeBy": ["sku"],
    "mergeStrategy": "append",
    "suggestTypes": true
  }
}
```

If a product with `sku: "SKU-001"` exists, this will update its properties while keeping any other existing properties. If it doesn't exist, a new product record will be created.

#### Subsequent Update Preserving Fields (append)

```json
{
  "label": "Product",
  "data": {
    "sku": "SKU-001",
    "price": 1199.99,
    "stock": 50
  },
  "options": {
    "mergeBy": ["sku"],
    "mergeStrategy": "append",
    "suggestTypes": true
  }
}
```

This updates the price and adds a stock field, while preserving the existing `name` and `category` properties.

#### Update with Rewrite Strategy (full replacement)

```json
{
  "label": "Product",
  "data": {
    "sku": "SKU-001",
    "name": "Laptop Pro v2",
    "price": 1399.99
  },
  "options": {
    "mergeBy": ["sku"],
    "mergeStrategy": "rewrite",
    "suggestTypes": true
  }
}
```

This replaces all properties of the product, removing `category` and `stock` fields from the previous example.

#### Upsert with Multiple Match Fields

```json
{
  "label": "User",
  "data": {
    "email": "user@example.com",
    "tenantId": "tenant-123",
    "name": "John Doe",
    "role": "admin"
  },
  "options": {
    "mergeBy": ["email", "tenantId"],
    "mergeStrategy": "append",
    "suggestTypes": true
  }
}
```

This matches on both `email` and `tenantId`, useful for multi-tenant applications.

#### Upsert Without Explicit MergeBy (all keys become match fingerprint)

```json
{
  "label": "Setting",
  "data": {
    "key": "theme",
    "value": "dark",
    "userId": "user-123"
  },
  "options": {
    "mergeStrategy": "append",
    "suggestTypes": true
  }
}
```

When `mergeBy` is empty or omitted, the match is performed on all properties in the incoming data. A record will only be updated if all property values match exactly.

### Response (Upsert)

```json
{
  "__id": "018e4c71-f35a-7000-89cd-850db63a1e77",
  "__label": "Product",
  "__proptypes": {
    "sku": "string",
    "name": "string",
    "price": "number",
    "category": "string"
  },
  "sku": "SKU-001",
  "name": "Laptop Pro",
  "price": 1299.99,
  "category": "Electronics"
}
```

### Use Cases

The upsert operation is particularly useful for:

- **Idempotent data imports**: Safely re-run imports without creating duplicates
- **User profile updates**: Update user information while preserving unmodified fields
- **Inventory management**: Update product stock levels while maintaining product details
- **Configuration management**: Update settings by key while preserving other settings
- **Multi-tenant applications**: Match records by tenant-specific identifiers
- **Data synchronization**: Keep external data sources in sync with your graph database

### Best Practices

- **Choose the right merge strategy**: Use `append` when you want to preserve existing data, `rewrite` when you need a clean slate
- **Use specific mergeBy fields**: Define clear unique identifiers for better performance and predictability (email, sku, externalId, tenantId+userId compound, etc.)
- **Consider multi-field matching**: For multi-tenant or complex scenarios, use multiple fields in `mergeBy`
- **Handle edge cases**: When `mergeBy` is empty, ensure your data structure supports matching on all fields
- **Use with transactions**: Combine upsert with [transactions](../../concepts/transactions.mdx) for atomic multi-record operations

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
Token: $RUSHDB_API_KEY
X-Transaction-Id: $YOUR_TRANSACTION_ID
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

### Automatic Type Inference

**By default, `suggestTypes` is set to `true` for all write operations** (create, upsert, import). This means RushDB automatically infers data types from your values:
- Numeric values become `number` type
- `true`/`false` become `boolean` type
- ISO8601 strings become `datetime` type
- `null` becomes `null` type
- All other values become `string` type

To disable automatic type inference and store all values as strings, you must **explicitly set `suggestTypes: false`** in your request options.

### Additional Type Conversions

When `convertNumericValuesToNumbers` is enabled, string values that represent numbers (e.g., '30') will be converted to their numeric equivalents (e.g., 30).

When `castNumberArraysToVectors` is enabled, numeric arrays will be stored as `vector` type instead of `number` arrays.

## Best Practices

- Use the default approach for typical use cases and when automatic type inference is desired
- Use the property-based approach when precise control over [property](../../concepts/properties.md) types is required
- Use the [Import Data API](./import-data.md) for batch operations and creating multiple records
- Use [transactions](../../concepts/transactions.mdx) when creating related records to ensure data consistency
- Validate data on the client side before sending it to the API
