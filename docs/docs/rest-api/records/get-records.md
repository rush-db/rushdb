---
sidebar_position: 7
---

# Get Records

RushDB provides flexible APIs for retrieving records from your database. This capability allows you to access individual records by ID or retrieve multiple records using powerful search queries.

## Overview

The record retrieval endpoints allow you to:
- Get a single record by its ID
- Search for multiple records using [SearchQuery capabilities](/concepts/search/introduction)
- Filter, sort, and paginate results
- Retrieve records with related data

All record retrieval operations require authentication using a bearer token.

## Get a Single Record

```http
GET /api/v1/records/{entityId}
```

This endpoint retrieves a specific record identified by its unique ID.

### Path Parameters

| Parameter  | Type   | Description |
|------------|--------|-------------|
| `entityId` | String | The unique identifier of the record to retrieve |

### Response

```json
{
  "success": true,
  "data": {
    "id": "018e4c71-5f20-7db2-b0b1-e7e681542af9",
    "label": "PERSON",
    "name": "John Doe",
    "age": 30,
    "email": "john@example.com"
  }
}
```

## Search for Records

```http
POST /api/v1/records/search
```

This endpoint searches for records that match the specified criteria, with support for filtering, pagination, and sorting.

### Request Body

You can use search parameters to filter the data you want to retrieve:

| Field     | Type   | Description |
|-----------|--------|-------------|
| `where`   | Object | Filter conditions for records ([learn more](/concepts/search/where)) |
| `orderBy` | String or Object | Sorting criteria ([learn more](/concepts/search/pagination-order)) |
| `skip`    | Number | Number of records to skip for pagination ([learn more](/concepts/search/pagination-order)) |
| `limit`   | Number | Maximum number of records to return (default: 1000) |
| `labels`  | Array  | Optional array of labels to filter records by ([learn more](/concepts/search/labels)) |

### Example Request

```json
{
  "where": {
    "age": { "$gt": 25 }
  },
  "orderBy": { "name": "asc" },
  "skip": 0,
  "limit": 50,
  "labels": ["PERSON"]
}
```

### Response

```json
{
  "success": true,
  "data": {
    "data": [
      {
        "id": "018e4c71-5f20-7db2-b0b1-e7e681542af9",
        "label": "PERSON",
        "name": "John Doe",
        "age": 30,
        "email": "john@example.com"
      },
      {
        "id": "018e4c71-6a38-7db2-b0b1-e7e681542c13",
        "label": "PERSON",
        "name": "Jane Smith",
        "age": 28,
        "email": "jane@example.com"
      }
      // ... more records
    ],
    "total": 125
  }
}
```

## Search Related Records

```http
POST /api/v1/records/{entityId}/search
```

This endpoint searches for records that are related to a specific record, identified by its ID.

### Path Parameters

| Parameter  | Type   | Description |
|------------|--------|-------------|
| `entityId` | String | The unique identifier of the record to search from |

### Request Body

The request body is the same as for the regular search endpoint, allowing you to filter, paginate, and sort the related records.

### Example Request

```json
{
  "where": {
    "status": "active"
  },
  "orderBy": { "createdAt": "desc" },
  "limit": 20
}
```

### Response

```json
{
  "success": true,
  "data": {
    "data": [
      {
        "id": "018e4c71-7b42-7db2-b0b1-e7e681543d21",
        "label": "DOCUMENT",
        "title": "Project Plan",
        "status": "active",
        "createdAt": "2025-04-12T10:30:15Z"
      },
      // ... more records
    ],
    "total": 8
  }
}
```

## Advanced Filtering

RushDB supports complex filtering through the `where` clause, allowing you to create sophisticated queries:

```json
{
  "where": {
    "$or": [
      { "status": "active", "priority": { "$gte": 2 } },
      { "status": "pending", "deadline": { "$lt": "2025-06-01" } }
    ],
    "assignedTo": { "$ne": null }
  },
  "orderBy": [
    { "priority": "desc" },
    { "deadline": "asc" }
  ],
  "limit": 100
}
```

See the [Where Clause documentation](/concepts/search/where) for a complete reference of available operators.

## Performance Considerations

- Use appropriate `limit` values to control response size and query performance
- When working with large datasets, use pagination (`skip` and `limit`) as described in [pagination documentation](/concepts/search/pagination-order)
- Complex query conditions may increase processing time
- Use [label filtering](/concepts/search/labels) to narrow down the search scope before applying other filters
- For frequently accessed records, consider optimizing query patterns

## Related Documentation

- [Search Introduction](/concepts/search/introduction)
- [Where Clause](/concepts/search/where)
- [Labels](/concepts/search/labels)
- [Pagination and Order](/concepts/search/pagination-order)
- [Record Relationships](/concepts/relationships)
