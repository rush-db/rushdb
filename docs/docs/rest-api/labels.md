---
sidebar_position: 4
---

# Labels API

RushDB provides a Labels API that allows you to retrieve information about the labels used in your records. Labels are a powerful way to categorize and organize records in your database.

## Overview

The Labels API allows you to:
- Retrieve all labels used in your project
- Get the count of records with each label
- Filter labels based on record properties

All labels endpoints require authentication using a token header.

## List Labels

```http
POST /labels
```

Returns a list of all labels in the current project along with the count of records having each label. You can filter the results using the `where` clause.

### Request Body

| Field     | Type   | Description |
|-----------|--------|-------------|
| `where`   | Object | Optional filter criteria to narrow down which labeled records to include |

### Example Request

```json
{
  "where": {
    "country": "USA"
  }
}
```

This will return labels for all records where the `country` property equals "USA".

### Response

```json
{
  "success": true,
  "data": {
    "Person": 35,
    "Company": 12,
    "Customer": 24
  }
}
```

The response is a map where each key is a label name and each value is the count of records with that label.

## Filtering Labels

You can use complex queries to filter which labeled records to include:

### Example with Multiple Conditions

```json
{
  "where": {
    "$and": [
      { "age": { "$gt": 30 } },
      { "active": true }
    ]
  }
}
```

This will return labels for records where `age` is greater than 30 AND `active` is true.

### Example with OR Logic

```json
{
  "where": {
    "$or": [
      { "country": "USA" },
      { "country": "Canada" }
    ]
  }
}
```

This will return labels for records where `country` is either "USA" OR "Canada".

## Working with Labels

### Best Practices

1. **Consistent naming conventions**: Use a consistent pattern for label names (e.g., singular nouns, PascalCase)
2. **Meaningful labels**: Choose labels that describe what the record represents, not just its attributes
3. **Hierarchical labeling**: Consider using more specific labels for specialized record types (e.g., "Employee" and "Manager" instead of just "Person")
4. **Multiple labels**: Remember that records can have multiple labels in RushDB, allowing for flexible classification

### Common Use Cases

- **Data organization**: Group related records for easier querying and visualization
- **Access control**: Set permissions based on record labels
- **Conditional processing**: Apply different business logic depending on record types
- **Schema validation**: Enforce data structure based on record labels

## Error Handling

The Labels API may return the following error responses:

| Status Code | Description |
|-------------|-------------|
| 400 | Bad Request - Invalid query format |
| 401 | Unauthorized - Authentication required |
| 403 | Forbidden - Insufficient permissions |
| 500 | Server Error - Processing failed |

### Example Error Response

```json
{
  "success": false,
  "message": "Invalid query format in 'where' clause",
  "statusCode": 400
}
```
