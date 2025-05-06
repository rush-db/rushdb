---
sidebar_position: 7
---

#  Relationships

RushDB provides dedicated endpoints to create, read, update, and delete relationships between records. These endpoints allow you to build complex graph structures and model real-world relationships in your data.

## Overview

The relationship management endpoints enable you to:
- Create relationships between records
- List relationships for a record
- Remove specific relationships
- Search across all relationships
- Manage relationship types and directions

All relationship endpoints require authentication using a bearer token.

## Create Relationship

Create one or more relationships between records.

```http
POST /records/{entityId}/relations
```

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `entityId` | String | Source record identifier (UUIDv7) |

### Request Body

| Field | Type | Description |
|-------|------|-------------|
| `targetIds` | String or Array | Target record identifier(s). Cannot be empty or contain empty strings |
| `type` | String | (Optional) Relationship type. Cannot be an empty string |
| `direction` | String | (Optional) Relationship direction. Must be either "in" or "out". Defaults to "out" |

### Example Request

```json
{
  "targetIds": ["018dfc84-d6cb-7000-89cd-850db63a1e78"],
  "type": "FOLLOWS",
  "direction": "out"
}
```

#### Creating Multiple Relationships

You can create multiple relationships in a single request by passing an array of target IDs:

```json
{
  "targetIds": [
    "018dfc84-d6cb-7000-89cd-850db63a1e78",
    "018dfc84-d6cb-7000-89cd-850db63a1e79"
  ],
  "type": "FOLLOWS",
  "direction": "out"
}
```

### Response

```json
{
  "message": "Relations created successfully"
}
```

## List Relationships

Retrieve relationships for a specific record.

```http
GET /records/{entityId}/relations
```

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `entityId` | String | Record identifier (UUIDv7) |

### Query Parameters

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `skip` | Number | (Optional) Number of relationships to skip | 0 |
| `limit` | Number | (Optional) Maximum number of relationships to return | 1000 |

### Example Response

```json
{
  "data": [
    {
      "sourceId": "018dfc84-d6cb-7000-89cd-850db63a1e77",
      "sourceLabel": "Person",
      "targetId": "018dfc84-d6cb-7000-89cd-850db63a1e78",
      "targetLabel": "Person",
      "type": "FOLLOWS"
    }
  ],
  "total": 1
}
```

## Remove Relationship

Remove one or more relationships between records.

```http
PUT /records/{entityId}/relations
```

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `entityId` | String | Source record identifier (UUIDv7) |

### Request Body

| Field | Type | Description |
|-------|------|-------------|
| `targetIds` | String or Array | Target record identifier(s). Cannot be empty or contain empty strings |
| `typeOrTypes` | String or Array | (Optional) One or more relationship type(s) to remove. Cannot be empty strings |
| `direction` | String | (Optional) Filter relationships by direction: "in" or "out" |

### Example Request - Single Type

```json
{
  "targetIds": ["018dfc84-d6cb-7000-89cd-850db63a1e78"],
  "typeOrTypes": "FOLLOWS",
  "direction": "out"
}
```

### Example Request - Multiple Types

```json
{
  "targetIds": ["018dfc84-d6cb-7000-89cd-850db63a1e78"],
  "typeOrTypes": ["FOLLOWS", "LIKES"],
  "direction": "out"
}
```

### Response

```json
{
  "message": "Relations removed successfully"
}
```

## Search Relationships

Search across all relationships in the project.

```http
POST /records/relations/search
```

### Query Parameters

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `skip` | Number | (Optional) Number of relationships to skip | 0 |
| `limit` | Number | (Optional) Maximum number of relationships to return | 1000 |

### Request Body

The search endpoint accepts a SearchDto object with the following fields:

| Field | Type | Description |
|-------|------|-------------|
| `where` | Object | (Optional) Filter criteria for the search |
| `orderBy` | Object | (Optional) Sorting criteria |
| `labels` | Array | (Optional) Filter by record labels |

### Example Request - With Filters

```json
{
  "where": {
    "sourceLabel": "Person",
    "type": "FOLLOWS"
  },
  "orderBy": {
    "type": "ASC"
  },
  "limit": 10
}
```

### Response

```json
{
  "data": [
    {
      "sourceId": "018dfc84-d6cb-7000-89cd-850db63a1e77",
      "sourceLabel": "Person",
      "targetId": "018dfc84-d6cb-7000-89cd-850db63a1e78",
      "targetLabel": "Person",
      "type": "FOLLOWS"
    }
  ],
  "total": 1
}
```

## Relationship Types

RushDB supports several relationship configurations:

### Default Relationship
If no type is specified when creating a relationship, it uses the default type `__RUSHDB__RELATION__DEFAULT__`. This relationship type is useful for simple connections where semantic meaning isn't required.

### Custom Types
You can define custom relationship types to represent specific semantic meanings in your data model. For example:
- `FOLLOWS` for social connections
- `BELONGS_TO` for hierarchical relationships
- `WORKS_FOR` for organizational relationships

### Bidirectional Relationships
While relationships have a direction, you can create bidirectional relationships by:
1. Creating two relationships with opposite directions
2. Querying relationships without specifying direction

### Relationship Properties
Relationships can have properties attached to them, which is useful for storing metadata about the connection, such as:
- Timestamps (when the relationship was established)
- Weights or strengths
- Additional context

## Validation

The API enforces the following validation rules:

1. `targetIds` cannot be empty or contain empty strings
2. `type` and `typeOrTypes` cannot be empty strings when provided
3. `direction` must be either "in" or "out" when provided
4. Record IDs must be valid UUIDv7 strings
5. Source and target records must exist in the database

## Error Handling

Relationship operations may return the following error responses:

| Status Code | Description |
|-------------|-------------|
| 400 | Bad Request - Invalid input format or validation failure |
| 401 | Unauthorized - Authentication required |
| 403 | Forbidden - Insufficient permissions |
| 404 | Not Found - Record does not exist |
| 500 | Server Error - Processing failed |

### Example Error Response

```json
{
  "success": false,
  "message": "Target record with id '018dfc84-d6cb-7000-89cd-850db63a1e78' not found",
  "statusCode": 404
}
```

## Best Practices

1. **Use meaningful relationship types** that describe the semantic connection between records
2. **Consider directionality** when designing your data model - choose directions that make semantic sense
3. **Batch relationship operations** when creating or modifying many relationships at once
4. **Use pagination** when retrieving large sets of relationships to improve performance
5. **Validate record existence** before creating relationships
6. **Index important relationship types** that are frequently queried
7. **Use consistent naming conventions** for relationship types (e.g., uppercase with underscores)
8. **Document relationship types** and their meanings in your application