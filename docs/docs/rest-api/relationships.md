---
sidebar_position: 5
---

# Relationships API

RushDB provides a powerful Relationships API that enables you to manage connections between records. This API allows you to create, retrieve, update, and delete relationships between any records in your database.

## Overview

The Relationships API allows you to:
- Create relationships between records
- Retrieve relationships for a specific record
- Search relationships across your entire database
- Delete specific or all relationships between records
- Specify relationship types and directions

All relationships endpoints require authentication using a token header.

## Create Relationship

```http
POST /records/:entityId/relations
```

Creates one or more relationships from a source record to one or more target records.

### Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `entityId` | String | The ID of the source record |

### Request Body

| Field | Type | Description |
|-------|------|-------------|
| `targetIds` | String or Array | ID(s) of target record(s) to create relationship(s) with |
| `type` | String | Optional. The type of relationship to create. Defaults to `__RUSHDB__RELATION__DEFAULT__` |
| `direction` | String | Optional. Direction of the relationship: `in` or `out`. Defaults to `out` |

### Example Request - Single Target

```json
{
  "targetIds": "018e4c71-f35a-7000-89cd-850db63a1e78",
  "type": "WORKS_FOR"
}
```

### Example Request - Multiple Targets

```json
{
  "targetIds": [
    "018e4c71-f35a-7000-89cd-850db63a1e78",
    "018e4c71-f35a-7000-89cd-850db63a1e79"
  ],
  "type": "KNOWS",
  "direction": "out"
}
```

### Response

```json
{
  "success": true,
  "data": {
    "message": "Relations to Record 018e4c71-f35a-7000-89cd-850db63a1e77 have been successfully created"
  }
}
```

## Get Record Relationships

```http
GET /records/:entityId/relations
```

Retrieves all relationships for a specific record.

### Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `entityId` | String | The ID of the record |
| `skip` | Number | Optional. Number of relationships to skip (default: 0) |
| `limit` | Number | Optional. Maximum number of relationships to return (default: 1000) |

### Response

```json
{
  "success": true,
  "data": {
    "total": 3,
    "data": [
      {
        "sourceId": "018e4c71-f35a-7000-89cd-850db63a1e77",
        "sourceLabel": "Person",
        "targetId": "018e4c71-f35a-7000-89cd-850db63a1e78",
        "targetLabel": "Company",
        "type": "WORKS_FOR"
      },
      {
        "sourceId": "018e4c71-f35a-7000-89cd-850db63a1e77",
        "sourceLabel": "Person",
        "targetId": "018e4c71-f35a-7000-89cd-850db63a1e79",
        "targetLabel": "Person",
        "type": "KNOWS"
      },
      {
        "sourceId": "018e4c71-f35a-7000-89cd-850db63a1e80",
        "sourceLabel": "Department",
        "targetId": "018e4c71-f35a-7000-89cd-850db63a1e77",
        "targetLabel": "Person",
        "type": "HAS_MEMBER"
      }
    ]
  }
}
```

## Delete Relationships

```http
PUT /records/:entityId/relations
```

Deletes one or more relationships from a source record to one or more target records.

### Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `entityId` | String | The ID of the source record |

### Request Body

| Field | Type | Description |
|-------|------|-------------|
| `targetIds` | String or Array | ID(s) of target record(s) to delete relationship(s) with |
| `typeOrTypes` | String or Array | Optional. Type(s) of relationships to delete. If omitted, deletes relationships of any type |
| `direction` | String | Optional. Direction of the relationship: `in` or `out`. If omitted, deletes relationships in both directions |

### Example Request - Delete All Relationship Types

```json
{
  "targetIds": "018e4c71-f35a-7000-89cd-850db63a1e78"
}
```

### Example Request - Delete Specific Relationship Types

```json
{
  "targetIds": [
    "018e4c71-f35a-7000-89cd-850db63a1e78",
    "018e4c71-f35a-7000-89cd-850db63a1e79"
  ],
  "typeOrTypes": ["KNOWS", "WORKS_FOR"],
  "direction": "out"
}
```

### Response

```json
{
  "success": true,
  "data": {
    "message": "Relations to Record 018e4c71-f35a-7000-89cd-850db63a1e77 have been successfully deleted"
  }
}
```

## Search Relationships

```http
POST /records/relations/search
```

Searches for relationships across your database with optional filtering.

### Request Body

| Field | Type | Description |
|-------|------|-------------|
| `where` | Object | Optional filter criteria to search for specific relationships |

### Query Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `skip` | Number | Optional. Number of relationships to skip (default: 0) |
| `limit` | Number | Optional. Maximum number of relationships to return (default: 1000) |

### Example Request - Filter by Record Properties

```json
{
  "where": {
    "sourceRecord": {
      "name": "John Doe"
    },
    "targetRecord": {
      "name": "Acme Inc"
    }
  }
}
```

### Response

```json
{
  "success": true,
  "data": {
    "total": 1,
    "data": [
      {
        "sourceId": "018e4c71-f35a-7000-89cd-850db63a1e77",
        "sourceLabel": "Person",
        "targetId": "018e4c71-f35a-7000-89cd-850db63a1e78",
        "targetLabel": "Company",
        "type": "WORKS_FOR"
      }
    ]
  }
}
```

## Relationship Directionality

RushDB supports three types of relationship directionality:

1. **Outgoing relationships (`direction: "out"`)**:
   The source record points to the target record: `(source)-[relationship]->(target)`

2. **Incoming relationships (`direction: "in"`)**:
   The target record points to the source record: `(source)<-[relationship]-(target)`

3. **Undirected relationships (no direction specified)**:
   The relationship has no specific direction: `(source)-[relationship]-(target)`

## Best Practices

1. **Use meaningful relationship types**: Choose relationship types that clearly describe the connection between records
2. **Consider directionality**: Choose the right direction for your relationships based on your domain model
3. **Use relationship metadata**: When your use case requires it, store additional information about relationships
4. **Use consistent naming**: Establish naming conventions for relationship types (e.g., uppercase with underscores)
5. **Mind performance**: For highly connected records, paginate relationships with the `skip` and `limit` parameters

## Error Handling

The Relationships API may return the following error responses:

| Status Code | Description |
|-------------|-------------|
| 400 | Bad Request - Invalid parameters |
| 401 | Unauthorized - Authentication required |
| 403 | Forbidden - Insufficient permissions |
| 404 | Not Found - Record with the specified ID doesn't exist |
| 500 | Server Error - Processing failed |

### Example Error Response

```json
{
  "success": false,
  "message": "Record with ID 018e4c71-f35a-7000-89cd-850db63a1e99 not found",
  "statusCode": 404
}
