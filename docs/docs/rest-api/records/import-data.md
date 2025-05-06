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

All import endpoints require authentication using a token header.

## Import JSON Data

```http
POST /records/import/json
```

### Request Body

| Field     | Type   | Description |
|-----------|--------|-------------|
| `payload` | Object or Array | JSON data to import |
| `label`   | String | Label for the root node(s) |
| `options` | Object | Optional configuration parameters |

#### Options Object

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `suggestTypes` | Boolean | `true` | When true, automatically infers data types for properties |
| `castNumberArraysToVectors` | Boolean | `false` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | Boolean | `false` | When true, converts string numbers to number type |
| `capitalizeLabels` | Boolean | `false` | When true, converts all labels to uppercase |
| `relationshipType` | String | `__RUSHDB__RELATION__DEFAULT__` | Default relationship type between nodes |
| `returnResult` | Boolean | `false` | When true, returns imported records in response |

### Example Request

```json
{
  "label": "Person",
  "payload": {
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
      "id": "018dfc84-d6cb-7000-89cd-850db63a1e77",
      "label": "Person",
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
POST /records/import/csv
```

### Request Body

| Field     | Type   | Description |
|-----------|--------|-------------|
| `payload` | String | CSV data as a string |
| `label`   | String | Label for the nodes |
| `options` | Object | Optional configuration parameters (same as JSON import) |

CSV files must have headers in the first row.

### Example Request

```json
{
  "label": "Customer",
  "payload": "name,email,age\nJohn Doe,john@example.com,30\nJane Smith,jane@example.com,25",
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

When `suggestTypes` is enabled, RushDB will infer the following types:

- `string`: Text values
- `number`: Number values (& numeric values when `convertNumericValuesToNumbers` is true)
- `boolean`: True/false values
- `null`: Null values
- `vector`: Arrays of numbers (when `castNumberArraysToVectors` is true)
- `datetime`: ISO8601 format strings (e.g., "2025-04-23T10:30:00Z") will be automatically cast to Neo4j datetime values

When `convertNumericValuesToNumbers` is enabled, string values that represent numbers (e.g., '123') will be automatically converted to their numeric equivalents (e.g., 123).

Arrays with consistent data types (e.g., all numbers, all strings) will be handled seamlessly according to their type. However, for inconsistent arrays (e.g., `[1, 'two', null, false]`), all values will be automatically converted to strings to mitigate data loss, and the property type will be stored as `string`.

## Error Handling

The import API may return the following error responses:

| Status Code | Description |
|-------------|-------------|
| 400 | Bad Request - Invalid payload format |
| 401 | Unauthorized - Authentication required |
| 402 | Payment Required - Import would exceed plan limits |
| 403 | Forbidden - Insufficient permissions |
| 500 | Server Error - Processing failed |

### Example Error Response

```json
{
  "success": false,
  "message": "The number of items you are trying to send exceeds your limits.",
  "statusCode": 402
}
```

## Performance Considerations

- Imports are processed in chunks of 1000 records for optimal performance
- For large imports (>25MB), consider splitting into multiple requests
- Setting `returnResult: false` is recommended for large imports to improve performance
