---
sidebar_position: 2
---

# Export Data

RushDB provides efficient APIs for exporting your database records in different formats. This capability allows you to retrieve and analyze your data externally or integrate it with other systems.

## Overview

The export endpoints allow you to:
- Export data in CSV format
- Filter and query the data to be exported
- Order results as needed
- Handle large exports efficiently through pagination

All export endpoints require authentication using a bearer token.

## Export CSV Data

```http
POST /records/export/csv
```

This endpoint exports data in CSV format with headers in the first row.

### Request Body

You can send search parameters to filter the data you want to export:

| Field     | Type   | Description |
|-----------|--------|-------------|
| `where`   | Object | Filter conditions for records |
| `orderBy` | String or Object | Sorting criteria (`'asc'`, `'desc'` or `{field: 'asc'}`) |
| `skip`    | Number | Number of records to skip (for pagination) |
| `limit`   | Number | Maximum number of records to return (up to 1000) |
| `labels`  | Array  | Optional array of labels to filter records by |

### Example Request

```json
{
  "where": {
    "age": { "$gt": 25 }
  },
  "orderBy": { "name": "asc" },
  "limit": 1000
}
```

### Response

```json
{
  "success": true,
  "data": {
    "fileContent": "id,label,name,age,email\n018dfc84-d6cb-7000-89cd-850db63a1e77,PERSON,John Doe,30,john@example.com\n018dfc84-d78c-7000-89cd-85db63d6a120,PERSON,Jane Smith,28,jane@example.com",
    "dateTime": "2025-04-23T10:15:32.123Z"
  }
}
```

The `fileContent` field contains the CSV data string that can be saved directly to a file.

## Data Processing

When exporting data, RushDB:

1. **Filters**: Applies any specified filters to select records
2. **Sorts**: Orders records based on the `orderBy` parameter
3. **Paginates**: Processes data in efficient batches
4. **Transforms**: Converts internal data structures to CSV format
5. **Cleans**: Removes internal system properties before returning data

## Performance Considerations

- Exports process data in batches of 1000 records for optimal performance
- For large datasets, consider using pagination parameters (`skip` and `limit`)
- Complex queries may increase processing time
- RushDB automatically handles large exports by chunking the data retrieval

## Working with Exported Data

The exported CSV can be:
- Imported into spreadsheet software
- Processed by data analysis tools
- Used for backups and data archiving
- Imported into other databases

## Error Handling

| Status Code | Description |
|-------------|-------------|
| 400 | Bad Request - Invalid search parameters |
| 401 | Unauthorized - Authentication required |
| 403 | Forbidden - Insufficient permissions |
| 500 | Server Error - Export processing failed |

### Example Error Response

```json
{
  "success": false,
  "message": "Invalid search parameters",
  "statusCode": 400
}
```