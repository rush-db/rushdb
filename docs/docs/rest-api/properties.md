---
sidebar_position: 3
---

# Properties API

RushDB provides a powerful Properties API that enables you to manage the properties associated with your records. This API allows you to find, retrieve, create, update, and delete properties, as well as manage property values.

## Overview

The Properties API allows you to:
- List all properties in your project
- Get details about a specific property
- Get distinct values for a property
- Delete properties

All properties endpoints require authentication using a token header.

## Property Types

RushDB supports the following property types:

| Type | Description |
|------|-------------|
| `string` | Text values |
| `number` | Numeric values |
| `boolean` | True/false values |
| `null` | Null values |
| `datetime` | ISO8601 format datetime values |
| `vector` | Arrays of numbers (for embeddings/vector search) |

## List Properties

```http
POST /api/v1/properties/search
```

Returns a find of all properties in the current project, with filtering options.

### Request Body

| Field     | Type   | Description |
|-----------|--------|-------------|
| `where`   | Object | Optional filter criteria ([learn more](../../concepts/search/where)) |
| `labels`  | Array  | Optional array of labels to filter records by ([learn more](../../concepts/search/labels)) |

### Example Request

```json
{
  "where": {
    "type": "string"
  }
}
```

### Response

```json
{
  "success": true,
  "data": [
    {
      "id": "018dfc84-d6cb-7000-89cd-850db63a1e78",
      "name": "name",
      "type": "string",
      "projectId": "018dfc84-d6cb-7000-89cd-850db63a1e76",
      "metadata": ""
    },
    {
      "id": "018dfc84-d6cb-7000-89cd-850db63a1e79",
      "name": "email",
      "type": "string",
      "projectId": "018dfc84-d6cb-7000-89cd-850db63a1e76",
      "metadata": ""
    }
  ]
}
```

## Get Property

```http
GET /api/v1/properties/:propertyId
```

Retrieve detailed information about a specific property by its ID.

### Parameters

| Parameter   | Type   | Description |
|-------------|--------|-------------|
| `propertyId` | String | The ID of the property to retrieve |

### Response

```json
{
  "success": true,
  "data": {
    "id": "018dfc84-d6cb-7000-89cd-850db63a1e78",
    "name": "name",
    "type": "string",
    "projectId": "018dfc84-d6cb-7000-89cd-850db63a1e76",
    "metadata": ""
  }
}
```

## Get Property Values

```http
POST /api/v1/properties/:propertyId/values
```

Retrieves distinct values for a specific property across all records using SearchQuery filtering.

### Parameters

| Parameter   | Type   | Description |
|-------------|--------|-------------|
| `propertyId` | String | The ID of the property |

### Request Body

The request body supports SearchQuery parameters along with value-specific filtering:

| Field     | Type   | Description |
|-----------|--------|-------------|
| `where`   | Object | Optional. SearchQuery filter criteria ([learn more](../../concepts/search/where)) |
| `labels`  | Array  | Optional array of labels to filter records by ([learn more](../../concepts/search/labels)) |
| `skip`    | Number | Optional. Number of values to skip (default: 0) |
| `limit`   | Number | Optional. Maximum number of values to return (default: 100) |
| `query`   | String | Optional. Filter values by this text string |
| `orderBy` | String | Optional. Sort direction (`asc` or `desc`) |

### Example Request

```http
POST /api/v1/properties/018dfc84-d6cb-7000-89cd-850db63a1e78/values
Content-Type: application/json

{
  "where": {
    "status": "active"
  },
  "query": "jo",
  "orderBy": "asc",
  "skip": 0,
  "limit": 10
}
```

### Response

```json
{
  "success": true,
  "data": {
    "values": ["John", "Johnny", "Jon"],
    "min": null,
    "max": null,
    "type": "string"
  }
}
```

For numeric properties, the response includes minimum and maximum values:

```json
{
  "success": true,
  "data": {
    "values": [18, 19, 20, 21],
    "min": 18,
    "max": 21,
    "type": "number"
  }
}
```

## Delete Property

```http
DELETE /api/v1/properties/:propertyId
```

Deletes a property from all records.

### Parameters

| Parameter   | Type   | Description |
|-------------|--------|-------------|
| `propertyId` | String | The ID of the property to delete |

### Response

```json
{
  "success": true,
  "data": {
    "message": "Property (018dfc84-d6cb-7000-89cd-850db63a1e78) has been successfully deleted."
  }
}
```

## Value Handling

### Single Values

Single values are stored directly:

```json
{
  "name": "John Doe",
  "age": 30,
  "active": true
}
```

### Multiple Values

Arrays can store multiple values of the same type:

```json
{
  "tags": ["important", "urgent", "follow-up"],
  "scores": [85, 90, 95]
}
```

### Value Separators

When updating properties, you can use value separators to split a string into multiple values:

```json
{
  "name": "tags",
  "type": "string",
  "value": "important,urgent,follow-up",
  "valueSeparator": ","
}
```

This will result in an array of values: `["important", "urgent", "follow-up"]`.

## Property Metadata

Properties can have optional metadata, which can be used to store additional information about the property. This is useful for storing things like property descriptions, validation rules, or display preferences.

```json
{
  "name": "email",
  "type": "string",
  "metadata": "{\"description\":\"User's email address\",\"required\":true,\"unique\":true}"
}
```

Metadata is stored as a JSON string and can contain any valid JSON data.

## Best Practices

1. **Use consistent naming**: Follow a consistent naming convention for property names
2. **Set appropriate types**: Use the correct type for each property to facilitate operations like sorting and filtering
3. **Use metadata**: Leverage the metadata field to add useful information about your properties
4. **Batch updates**: When updating property values across many records, use the batch update endpoint
5. **Consider relationships**: For complex data models, consider using relationships between records instead of deeply nested property structures
