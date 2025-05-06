---
sidebar_position: 6
---

# Update Records

RushDB offers powerful methods to update existing records in your database. You can update record properties and labels through the REST API.

## Overview

The update endpoints allow you to:
- Update specific properties while preserving others (PATCH)
- Completely replace record data (PUT)

All update endpoints require authentication using a token header.

## Update Record (PATCH)

The PATCH method allows you to update specific properties of a record while preserving other existing properties.

```http
PATCH /records/{entityId}
```

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `entityId` | String | The unique identifier of the record to update |

### Request Body

| Field | Type | Description |
|-------|------|-------------|
| `label` | String | (Optional) New label for the record |
| `properties` | Array | Array of property objects to update or add |

#### Property Object

| Field | Type | Description |
|-------|------|-------------|
| `key` | String | Property name |
| `value` | Any | Property value |
| `type` | String | (Optional) Data type of the property |

### Example Request

```json
{
  "label": "Person",
  "properties": [
    {
      "key": "name",
      "value": "John Smith"
    },
    {
      "key": "age",
      "value": 32,
      "type": "number"
    },
    {
      "key": "active",
      "value": true,
      "type": "boolean"
    }
  ]
}
```

### Response

```json
{
  "id": "018dfc84-d6cb-7000-89cd-850db63a1e77",
  "label": "Person",
  "name": "John Smith",
  "age": 32,
  "email": "john@example.com",  // Preserved from existing record
  "active": true,
  "_rushdb_properties_meta": {
    // Metadata about properties
  }
}
```

### How PATCH Works

When you use PATCH to update a record:
1. The system first retrieves the current record data
2. Merges your new properties with the existing properties
3. Updates only the specified properties while preserving any properties not included in your request
4. Returns the complete updated record

This makes PATCH ideal for updating specific fields without having to resend all record data.

## Replace Record (PUT)

The PUT method allows you to completely replace a record's data.

```http
PUT /records/{entityId}
```

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `entityId` | String | The unique identifier of the record to update |

### Request Body

Same as PATCH method, but all existing properties not included in the request will be removed.

### Example Request

```json
{
  "label": "Customer",
  "properties": [
    {
      "key": "name",
      "value": "John Smith"
    },
    {
      "key": "age",
      "value": 32
    }
  ]
}
```

### Response

```json
{
  "id": "018dfc84-d6cb-7000-89cd-850db63a1e77",
  "label": "Customer",
  "name": "John Smith",
  "age": 32,
  "_rushdb_properties_meta": {
    // Metadata about properties
  }
}
```

### How PUT Works

When you use PUT to update a record:
1. The specified properties completely replace the existing record properties
2. Any properties not included in your request will be removed
3. The operation returns the new state of the record

This makes PUT ideal when you want to ensure the record only has the exact properties you specify.

## Get Record Properties

You can retrieve all properties of a specific record:

```http
GET /records/{entityId}/properties
```

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `entityId` | String | The unique identifier of the record |

### Response

```json
{
  "data": [
    {
      "key": "name",
      "value": "John Smith",
      "type": "string"
    },
    {
      "key": "age",
      "value": 32,
      "type": "number"
    },
    // Additional properties...
  ]
}
```

## Error Handling

Update operations may return the following error responses:

| Status Code | Description |
|-------------|-------------|
| 400 | Bad Request - Invalid input format |
| 401 | Unauthorized - Authentication required |
| 403 | Forbidden - Insufficient permissions |
| 404 | Not Found - Record does not exist |
| 500 | Server Error - Processing failed |

### Example Error Response

```json
{
  "success": false,
  "message": "Record with id '018dfc84-d6cb-7000-89cd-850db63a1e77' not found",
  "statusCode": 404
}
```

## Best Practices

1. **Use PATCH for partial updates** when you want to preserve existing data
2. **Use PUT for complete replacement** when you want to ensure the record only has the properties you specify
3. **Include property types** when you want to ensure proper data type conversion
4. **Check for 404 errors** when updating records that might not exist
5. **Retrieve current properties** with GET before updating to understand the record's current state
