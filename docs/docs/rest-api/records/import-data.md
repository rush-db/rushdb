---
sidebar_position: 1
---

# Import Data

RushDB provides powerful and flexible APIs for importing data into your database. You can import data in various formats including JSON and CSV, with options to customize how the data is processed and stored.

## Overview

The import endpoints allow you to:
- Import JSON data
- Import CSV data
- Control data type inference and handling
- Set default relationship types
- Configure property value handling
- Perform batch upsert (create-or-update) using `mergeBy` / `mergeStrategy` on import options

All import endpoints require authentication using a token header.

## Nested Data Processing

When importing nested JSON data structures, RushDB automatically processes and organizes your data using a breadth-first search (BFS) algorithm. This approach efficiently:

1. **Traverses hierarchical structures**: Processes your JSON tree level by level, ensuring proper parent-child relationships
2. **Optimizes object normalization**: Converts nested objects into separate records with appropriate relationships
3. **Preserves data integrity**: Maintains the original structure and relationships between your data elements

For example, when importing a nested object like a person with embedded address information, the BFS algorithm will:
- Create a separate record for the person
- Create separate records for embedded objects (addresses)
- Establish relationships between parent and child records
- Apply proper labels derived from the JSON structure
- Set up property nodes with appropriate type inference

For more details on how RushDB manages data storage and the underlying data import mechanism, see [Storage - Data Import Mechanism](../../concepts/storage#data-import-mechanism).

## Import JSON Data

```http
POST /api/v1/records/import/json
```

### Request Body

| Field     | Type   | Description |
|-----------|--------|-------------|
| `data` | Object or Array | JSON data to import |
| `label`   | String | Label for the root node(s) |
| `options` | Object | Optional configuration parameters |

#### Options Object

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `suggestTypes` | Boolean | `true` | **Default is `true`** - Automatically infers data types for properties. To disable type inference and store all values as strings, explicitly set to `false` |
| `castNumberArraysToVectors` | Boolean | `false` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | Boolean | `false` | When true, converts string numbers to number type |
| `capitalizeLabels` | Boolean | `false` | When true, converts all labels to uppercase |
| `relationshipType` | String | `__RUSHDB__RELATION__DEFAULT__` | Default relationship type between nodes |
| `returnResult` | Boolean | `false` | When true, returns imported records in response |
| `mergeBy` | Array of Strings | `[]` / omitted | Upsert match keys for batch import. Empty or omitted (with mergeStrategy present) means all incoming property keys per record. |
| `mergeStrategy` | String | `'append'` | Upsert behavior: `'append'` adds/updates provided properties; `'rewrite'` replaces all existing properties. Providing either this or `mergeBy` triggers upsert path. |

:::info Default Behavior
By default, `suggestTypes` is set to `true` for all import operations (JSON and CSV). This means RushDB automatically infers data types from your values. To store all properties as strings without type inference, you must explicitly set `suggestTypes: false` in the options.
:::

### Example Request (Batch Upsert Import)

```json
{
  "label": "Product",
  "data": [
    { "sku": "SKU-001", "name": "Gadget", "price": 99.99 },
    { "sku": "SKU-002", "name": "Widget", "price": 149.99 }
  ],
  "options": {
    "suggestTypes": true,
    "mergeBy": ["sku"],
    "mergeStrategy": "append",
    "returnResult": true
  }
}
```

If later you send:

```json
{
  "label": "Product",
  "data": [
    { "sku": "SKU-001", "price": 89.99 },
    { "sku": "SKU-002", "price": 139.99, "category": "Tools" }
  ],
  "options": {
    "mergeBy": ["sku"],
    "mergeStrategy": "append"
  }
}
```

SKU-001 price updates; SKU-002 price updates and category is added; all other properties preserved.

Using `"mergeStrategy": "rewrite"` would replace properties entirely for each matched record (unmentioned fields removed).

```json
{
  "label": "Person",
  "data": {
    "name": "John Doe",
    "age": "30",
    "addresses": [
      {
        "type": "home",
        "street": "123 Main St",
        "city": "Anytown"
      },
      {
        "type": "work",
        "street": "456 Business Rd",
        "city": "Workville"
      }
    ],
    "scores": [85, 90, 95],
    "active": true
  },
  "options": {
    "suggestTypes": true,
    "convertNumericValuesToNumbers": true,
    "relationshipType": "OWNS"
  }
}
```

### Response

```json
{
  "success": true,
  "data": true
}
```

If `returnResult: true` is specified in options, the response will include the imported records:

```json
{
  "success": true,
  "data": [
    {
      "__id": "018dfc84-d6cb-7000-89cd-850db63a1e77",
      "__label": "Person",
      "__proptypes": { ... },
      "name": "John Doe",
      "age": 30,
      // Additional properties...
    }
    // Additional records...
  ]
}
```

## Import CSV Data

```http
POST /api/v1/records/import/csv
```

### Request Body

| Field     | Type   | Description |
|-----------|--------|-------------|
| `data` | String | CSV data as a string |
| `label`   | String | Label for the nodes |
| `options` | Object | Optional configuration parameters (same as JSON import) |

CSV files must have headers in the first row.

### Example Request

```json
{
  "label": "Customer",
  "data": "name,email,age\nJohn Doe,john@example.com,30\nJane Smith,jane@example.com,25",
  "options": {
    "suggestTypes": true,
    "convertNumericValuesToNumbers": true
  }
}
```

### Response

Same as JSON import.

## Data Transformation Process

When importing data, RushDB processes your data through the following steps:

1. **Parsing**: Converts your input format (JSON/CSV) into internal structures
2. **Type Inference**: If `suggestTypes` is enabled, analyzes values to determine appropriate data types
3. **Graph Construction**: Creates nodes and relationships based on your data structure
4. **Validation**: Checks against workspace limits
5. **Storage**: Inserts data into the database in optimized batches

## Data Type Handling

### Automatic Type Inference

**By default, `suggestTypes` is set to `true` for all import operations** (JSON and CSV). This means RushDB automatically infers the following data types from your values:

- `string`: Text values
- `number`: Numeric values
- `boolean`: `true`/`false` values
- `null`: Null values
- `datetime`: ISO8601 format strings (e.g., "2025-04-23T10:30:00Z")
- `vector`: Arrays of numbers (when `castNumberArraysToVectors` is true)

To disable automatic type inference and store all values as strings, you must **explicitly set `suggestTypes: false`** in your request options.

### Additional Type Conversions

When `convertNumericValuesToNumbers` is enabled, string values that represent numbers (e.g., '123') will be automatically converted to their numeric equivalents (e.g., 123).

### Array Handling

Arrays with consistent data types (e.g., all numbers, all strings) will be handled seamlessly according to their type. However, for inconsistent arrays (e.g., `[1, 'two', null, false]`), all values will be automatically converted to strings to mitigate data loss, and the property type will be stored as `string`.

## Performance Considerations

- Imports are processed in chunks of 1000 records for optimal performance
- For large imports (>25MB), consider splitting into multiple requests
- Setting `returnResult: false` is recommended for large imports to improve performance
- Batch upsert performance depends on match selectivity; prefer stable unique or near-unique keys in `mergeBy`.
