---
sidebar_position: 7
---

# Delete Records

RushDB provides flexible APIs for deleting records from your database. This capability lets you remove individual records by ID or delete multiple records at once using search query filters.

## Overview

The delete endpoints allow you to:
- Delete a single record or multiple records by ID using `deleteById`
- Delete records using search queries with the `delete` method
- Delete records directly from record instances
- Perform atomic deletions using transactions
- Safely remove records with proper authentication

All delete operations require authentication using a bearer token and handle relationships appropriately. Deletion operations can also be performed within transactions for atomic operations.

## Delete a Single Record by ID

```typescript
// Delete a single record by ID
await db.records.deleteById('record-id-here');
```

This method deletes a single record identified by its unique ID.

### Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `idOrIds` | `String` or `Array<String>` | The unique identifier of the record to delete, or an array of IDs |
| `transaction` | `Transaction` or `String` | Optional transaction for atomic operations |

### Example

```typescript
// Delete a specific record
try {
  const response = await db.records.deleteById('018e4c71-5f20-7db2-b0b1-e7e681542af9');
  if (response.success) {
    console.log('Record deleted successfully');
  }
} catch (error) {
  console.error('Failed to delete record:', error);
}

// Delete multiple records by their IDs
try {
  const response = await db.records.deleteById([
    '018e4c71-5f20-7db2-b0b1-e7e681542af9',
    '018e4c71-5f20-7db2-b0b1-e7e681542af8'
  ]);
  if (response.success) {
    console.log('Records deleted successfully');
  }
} catch (error) {
  console.error('Failed to delete records:', error);
}

// Delete within a transaction
const tx = await db.tx.begin();
try {
  await db.records.deleteById('018e4c71-5f20-7db2-b0b1-e7e681542af9', tx);
  await db.tx.commit(tx);
  console.log('Record deleted successfully in transaction');
} catch (error) {
  await db.tx.rollback(tx);
  console.error('Transaction failed:', error);
}
```

## Delete Records Using a Search Query

```typescript
// Delete records using search query
await db.records.delete(
  {
    where: { /* search conditions */ }
  },
  transaction // optional
);
```

This method deletes records that match the specified search criteria.

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `searchQuery` | `SearchQuery<S>` | Query to identify records to delete |
| `transaction` | `Transaction` or `String` | Optional transaction for atomic operations |

Note: Using an empty `where` clause without allowing force delete will throw an `EmptyTargetError`.

You can use search parameters to define which records to delete:

| SearchQuery Field | Type     | Description                                                                                |
|-------------------|----------|--------------------------------------------------------------------------------------------|
| `where`           | `Object` | Filter conditions for records to delete ([learn more](../../concepts/search/where))        |
| `labels`          | `Array`  | Optional array of labels to filter records by ([learn more](../../concepts/search/labels)) |
| `limit`           | `Number` | Maximum number of records to delete (optional)                                             |

### Example

```typescript
// Delete all users with age under 18
try {
  const response = await db.records.delete({
    where: {
      label: 'USER',
      age: { $lt: 18 }
    }
  });
  if (response.success) {
    console.log(response.data.message); // Displays success message with deletion count
  }
} catch (error) {
  console.error('Failed to delete records:', error);
}

// Delete inactive products in a specific category
try {
  const response = await db.records.delete({
    where: {
      label: 'PRODUCT',
      category: 'electronics',
      isActive: false
    }
  });
  if (response.success) {
    console.log(response.data.message);
  }
} catch (error) {
  console.error('Failed to delete records:', error);
}
```

## Bulk Deletion with Complex Queries

For more advanced deletion scenarios, you can use the full power of RushDB's search query system:

```typescript
// Delete records with complex criteria
try {
  const response = await db.records.delete({
    where: {
      $or: [
        { status: 'archived', lastModified: { $lt: '2024-01-01' } },
        { status: 'deleted', isTemporary: true }
    ]
    },
    labels: ['DOCUMENT', 'ATTACHMENT'],
    limit: 1000 // Optional: limit the number of records deleted
  });
  console.log(`${response.data.message}`);
} catch (error) {
  console.error('Bulk deletion failed:', error);
}
```

## Deleting Records from a Record Instance

If you already have a record instance, you can delete it directly:

```typescript
// Find a record first
const record = await db.records.findById('018e4c71-5f20-7db2-b0b1-e7e681542af9');

// Then delete it
try {
  const response = await record.delete();
  if (response.success) {
    console.log('Record deleted successfully');
  }
} catch (error) {
  console.error('Failed to delete record:', error);
}

// With a transaction
const tx = await db.tx.begin();
try {
  await record.delete(tx);
  await db.tx.commit(tx);
  console.log('Record deleted successfully within transaction');
} catch (error) {
  await db.tx.rollback(tx);
  console.error('Transaction failed:', error);
}
```

## Handling Relationships

When deleting records, all relationships associated with those records are automatically deleted. This ensures database integrity and prevents orphaned relationships.

## Safety Features and Transactions

RushDB implements several safeguards for delete operations:

1. **Authentication**: All delete operations require a valid authentication token
2. **Authorization**: Users can only delete records in projects they have access to
3. **Validation**: Input data is validated before processing
4. **Transactions**: Delete operations can be wrapped in transactions for data consistency
5. **Partial Failure Handling**: If a deletion affects multiple records and some operations fail, all changes are rolled back when using transactions
6. **Empty Query Protection**: The API prevents accidental deletion of all records by requiring explicit configuration to allow force deletion with empty `where` clauses

## Performance Considerations

- For large-scale deletions, RushDB processes operations in batches
- Complex query conditions may increase processing time
- Consider using [label filtering](../../concepts/search/labels) to narrow down records before deletion
- For very large datasets, use pagination in combination with delete operations

## Related Documentation

- [Search Introduction](../../concepts/search/introduction)
- [Where Clause](../../concepts/search/where)
- [Labels](../../concepts/search/labels)
- [Record Relationships](../../concepts/relationships)
- [Transactions](../../concepts/transactions.mdx)
