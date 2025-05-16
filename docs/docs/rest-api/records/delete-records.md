---
sidebar_position: 3
---

# Delete Records

RushDB provides efficient APIs for deleting records from your database. This capability allows you to remove individual records by ID or delete multiple records at once using search query filters.

## Overview

The delete endpoints allow you to:
- Delete a single record by ID
- Delete multiple records using [SearchQuery capabilities](../../concepts/search/introduction)
- Perform conditional bulk deletions
- Safely remove records with proper authentication

All delete operations require authentication using a bearer token and handle relationships appropriately.

## Delete a Single Record

```http
DELETE /api/v1/records/{entityId}
```

This endpoint deletes a specific record identified by its unique ID.

### Path Parameters

| Parameter  | Type   | Description |
|------------|--------|-------------|
| `entityId` | String | The unique identifier of the record to delete |

### Response

```json
{
  "success": true,
  "data": {
    "message": "Record deleted successfully"
  }
}
```

## Delete Multiple Records

```http
PUT /api/v1/records/delete
```

This endpoint deletes multiple records that match the specified search criteria.

### Request Body

You can use search parameters to filter the data you want to delete:

| Field     | Type   | Description |
|-----------|--------|-------------|
| `where`   | Object | Filter conditions for records ([learn more](../../concepts/search/where)) |
| `labels`  | Array  | Optional array of labels to filter records by ([learn more](../../concepts/search/labels)) |

### Example Request

```json
{
  "where": {
    "age": { "$lt": 18 },
    "status": "inactive"
  },
  "labels": ["USER"]
}
```

### Response

```json
{
  "success": true,
  "data": {
    "message": "25 record(s) deleted successfully"
  }
}
```

## Bulk Deletion with Complex Queries

For more advanced deletion scenarios, you can use the full power of RushDB's search query system:

```json
{
  "where": {
    "$or": [
      { "status": "archived", "lastModified": { "$lt": "2024-01-01" } },
      { "status": "deleted", "isTemporary": true }
    ]
  },
  "labels": ["DOCUMENT", "ATTACHMENT"]
}
```

## Handling Relationships

When deleting records, all relationships associated with those records are automatically deleted. This ensures database integrity and prevents orphaned relationships.

## Delete Operation Safety

RushDB implements several safeguards for delete operations:

1. **Authentication**: All delete operations require a valid authentication token
2. **Authorization**: Users can only delete records in projects they have access to
3. **Validation**: Input data is validated before processing
4. **Transactions**: Delete operations are performed within transactions for data consistency
5. **Partial Failure Handling**: If a deletion affects multiple records and some operations fail, all changes are rolled back

## Performance Considerations

- For large-scale deletions, RushDB processes operations in batches
- Complex query conditions may increase processing time
- Consider using [label filtering](../../concepts/search/labels) to narrow down records before deletion
- For very large datasets, consider multiple smaller delete operations

## Related Documentation

- [Search Introduction](../../concepts/search/introduction)
- [Where Clause](../../concepts/search/where)
- [Labels](../../concepts/search/labels)
- [Record Relationships](../../concepts/relationships)
