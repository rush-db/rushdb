---
sidebar_position: 5
---

# Relationships API

RushDB provides a powerful Relationships API that enables you to manage connections between [records](../concepts/records.md). This API allows you to create, retrieve, update, and delete [relationships](../concepts/relationships.md) between any records in your database.

## Overview

The Relationships API allows you to:
- Create relationships between records
- Retrieve relationships for a specific record
- Search relationships across your entire database
- Delete specific or all relationships between records
- Specify relationship types and directions

All relationships endpoints require authentication using a token header.

## Create Many Relationships (by key match)

```http
POST /api/v1/relationships/create-many
```

Creates relationships in bulk by matching a property from source-labeled records to a property from target-labeled records.

### Request Body

| Field           | Type   | Description                                                                                       |
|-----------------|--------|---------------------------------------------------------------------------------------------------|
| `source`        | Object | Source selector: `{ label: string; key: string; where?: object }`                                  |
| `target`        | Object | Target selector: `{ label: string; key: string; where?: object }`                                  |
| `type`          | String | Optional. Relationship type to create. Defaults to `__RUSHDB__RELATION__DEFAULT__`                |
| `direction`     | String | Optional. Relationship direction: `in` or `out`. Defaults to `out`                                |

The matching condition is always `source[key] = target[key]`, combined with optional `where` filters on each side.
The `where` objects follow the standard SearchQuery `where` syntax used across the platform.

### Example Request

```json
{
  "source": { "label": "USER", "key": "id", "where": { "tenantId": "ACME" } },
  "target": { "label": "ORDER", "key": "userId", "where": { "tenantId": "ACME" } },
  "type": "ORDERED",
  "direction": "out"
}
```

### Response

```json
{
  "success": true,
  "data": {
    "message": "Relations have been successfully created"
  }
}
```

## Create Relationship

```http
POST /api/v1/records/:entityId/relationships
```

Creates one or more [relationships](../concepts/relationships.md) from a source record to one or more target records.

### Parameters

| Parameter  | Type   | Description                 |
|------------|--------|-----------------------------|
| `entityId` | String | The ID of the source record |

### Request Body

| Field       | Type            | Description                                                                               |
|-------------|-----------------|-------------------------------------------------------------------------------------------|
| `targetIds` | String or Array | ID(s) of target record(s) to create relationship(s) with                                  |
| `type`      | String          | Optional. The type of relationship to create. Defaults to `__RUSHDB__RELATION__DEFAULT__` |
| `direction` | String          | Optional. Direction of the relationship: `in` or `out`. Defaults to `out`                 |

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
GET /api/v1/records/:entityId/relationships
```

Retrieves all relationships for a specific [record](../concepts/records.md).

### Parameters

| Parameter  | Type   | Description                                                         |
|------------|--------|---------------------------------------------------------------------|
| `entityId` | String | The ID of the record                                                |
| `skip`     | Number | Optional. Number of relationships to skip (default: 0)              |
| `limit`    | Number | Optional. Maximum number of relationships to return (default: 1000) |

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
PUT /api/v1/records/:entityId/relationships
```

Deletes one or more relationships from a source record to one or more target records.

### Parameters

| Parameter  | Type   | Description                 |
|------------|--------|-----------------------------|
| `entityId` | String | The ID of the source record |

### Request Body

| Field         | Type            | Description                                                                                                  |
|---------------|-----------------|--------------------------------------------------------------------------------------------------------------|
| `targetIds`   | String or Array | ID(s) of target record(s) to delete relationship(s) with                                                     |
| `typeOrTypes` | String or Array | Optional. Type(s) of relationships to delete. If omitted, deletes relationships of any type                  |
| `direction`   | String          | Optional. Direction of the relationship: `in` or `out`. If omitted, deletes relationships in both directions |

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
POST /api/v1/relationships/search
```

Searches for [relationships](../concepts/relationships.md) across your database with optional filtering.

### Request Body

| Field   | Type   | Description                                                                                  |
|---------|--------|----------------------------------------------------------------------------------------------|
| `where` | Object | Optional [filter criteria](../concepts/search/where.md) to search for specific relationships |

### Query Parameters

| Parameter | Type   | Description                                                                                                     |
|-----------|--------|-----------------------------------------------------------------------------------------------------------------|
| `skip`    | Number | Optional. Number of relationships to skip for [pagination](../concepts/search/pagination-order.md) (default: 0) |
| `limit`   | Number | Optional. Maximum number of relationships to return (default: 1000)                                             |

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

RushDB supports three types of [relationship](../concepts/relationships.md) directionality:

1. **Outgoing relationships (`direction: "out"`)**:
   The source record points to the target record: `(source)-[relationship]->(target)`

2. **Incoming relationships (`direction: "in"`)**:
   The target record points to the source record: `(source)<-[relationship]-(target)`

3. **Undirected relationships (no direction specified)**:
   The relationship has no specific direction: `(source)-[relationship]-(target)`

## Best Practices

1. **Use meaningful relationship types**: Choose relationship types that clearly describe the connection between [records](../concepts/records.md)
2. **Consider directionality**: Choose the right direction for your relationships based on your domain model
3. **Use relationship metadata**: When your use case requires it, store additional information about relationships
4. **Use consistent naming**: Establish naming conventions for relationship types (e.g., uppercase with underscores)
5. **Mind performance**: For highly connected records, paginate relationships with the `skip` and `limit` parameters



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
POST /api/v1/records/{entityId}/relationships
```

### Path Parameters

| Parameter  | Type   | Description                       |
|------------|--------|-----------------------------------|
| `entityId` | String | Source record identifier (UUIDv7) |

### Request Body

| Field       | Type            | Description                                                                        |
|-------------|-----------------|------------------------------------------------------------------------------------|
| `targetIds` | String or Array | Target record identifier(s). Cannot be empty or contain empty strings              |
| `type`      | String          | (Optional) Relationship type. Cannot be an empty string                            |
| `direction` | String          | (Optional) Relationship direction. Must be either "in" or "out". Defaults to "out" |

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
GET /api/v1/records/{entityId}/relationships
```

### Path Parameters

| Parameter  | Type   | Description                |
|------------|--------|----------------------------|
| `entityId` | String | Record identifier (UUIDv7) |

### Query Parameters

| Parameter | Type   | Description                                          | Default |
|-----------|--------|------------------------------------------------------|---------|
| `skip`    | Number | (Optional) Number of relationships to skip           | 0       |
| `limit`   | Number | (Optional) Maximum number of relationships to return | 1000    |

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
PUT /api/v1/records/{entityId}/relationships
```

### Path Parameters

| Parameter  | Type   | Description                       |
|------------|--------|-----------------------------------|
| `entityId` | String | Source record identifier (UUIDv7) |

### Request Body

| Field         | Type            | Description                                                                    |
|---------------|-----------------|--------------------------------------------------------------------------------|
| `targetIds`   | String or Array | Target record identifier(s). Cannot be empty or contain empty strings          |
| `typeOrTypes` | String or Array | (Optional) One or more relationship type(s) to remove. Cannot be empty strings |
| `direction`   | String          | (Optional) Filter relationships by direction: "in" or "out"                    |

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

## Search Relations

```http
POST /api/v1/relationships/search
```

This endpoint searches for [relationships](../concepts/relationships.md) between records based on specified criteria.

### Request Body

The request body follows the standard [search parameters](../concepts/search/introduction.md) format.

### Query Parameters

| Parameter | Type   | Description                                                                                           |
|-----------|--------|-------------------------------------------------------------------------------------------------------|
| `skip`    | Number | Number of relationships to skip for [pagination](../concepts/search/pagination-order.md) (default: 0) |
| `limit`   | Number | Maximum number of relationships to return (default: 1000)                                             |

### Response

```json
{
  "success": true,
  "data": {
    "data": [
      // relationships matching the search criteria
    ],
    "total": 42
  }
}
```

## Search Relationships

Search across all [relationships](../concepts/relationships.md) in the project. This endpoint allows you to query relationships with powerful filtering options.

```http
POST /api/v1/relationships/search
```

### Query Parameters

| Parameter | Type   | Description                                                                                         | Default |
|-----------|--------|-----------------------------------------------------------------------------------------------------|---------|
| `skip`    | Number | (Optional) Number of relationships to skip for [pagination](../concepts/search/pagination-order.md) | 0       |
| `limit`   | Number | (Optional) Maximum number of relationships to return                                                | 1000    |

### Request Body

The search endpoint accepts a SearchDto object with the following fields:

| Field     | Type   | Description                                                                                        |
|-----------|--------|----------------------------------------------------------------------------------------------------|
| `where`   | Object | (Optional) [Filter criteria](../concepts/search/where.md) for the search                           |
| `orderBy` | Object | (Optional) [Sorting criteria](../concepts/search/pagination-order.md#sorting-records-with-orderby) |
| `labels`  | Array  | (Optional) Filter by [record labels](../concepts/search/labels.md)                                 |

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

## Best Practices

1. **Use meaningful relationship types** that describe the semantic connection between records
2. **Consider directionality** when designing your data model - choose directions that make semantic sense
3. **Batch relationship operations** when creating or modifying many relationships at once
4. **Use pagination** when retrieving large sets of relationships to improve performance
5. **Validate record existence** before creating relationships
6. **Index important relationship types** that are frequently queried
7. **Use consistent naming conventions** for relationship types (e.g., uppercase with underscores)
8. **Document relationship types** and their meanings in your application
