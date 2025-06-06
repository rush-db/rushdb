---
sidebar_position: 7
---

# Delete Records

RushDB Python SDK provides methods for deleting [records](../../concepts/records.md) from your database. You can delete individual records by ID or delete multiple records matching specific criteria.

## Overview

The delete methods allow you to:
- Delete a single record by ID
- Delete multiple records using search query filters
- Delete records directly from Record objects
- Perform conditional bulk deletions

## Prerequisites

Before deleting records, make sure you have initialized the RushDB client with your API token:

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_TOKEN", base_url="https://api.rushdb.com/api/v1")
```

## Deleting a Single Record by ID

The `delete_by_id()` method allows you to delete a record using its unique identifier.

### Syntax

```python
db.records.delete_by_id(
    id_or_ids: Union[str, List[str]],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `id_or_ids` | Union[str, List[str]] | Single ID or list of IDs to delete |
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

### Returns

A dictionary with the response data confirming the deletion.

### Examples

#### Deleting a Single Record

```python
# First, create or retrieve a record
product = db.records.create(
    label="PRODUCT",
    data={
        "name": "Discontinued Item",
        "price": 19.99
    }
)

# Delete the record by its ID
response = db.records.delete_by_id(product.id)

print(f"Deletion response: {response}")
# Example output: {'message': 'Record deleted successfully'}
```

#### Deleting Multiple Records by ID

```python
# Delete multiple records by their IDs
response = db.records.delete_by_id([
    "018e4c71-f35a-7000-89cd-850db63a1e77",
    "018e4c75-a2b3-7000-89cd-850db63a1e77",
    "018e4c79-c4d5-7000-89cd-850db63a1e77"
])

print(f"Deletion response: {response}")
# Example output: {'message': '3 record(s) deleted successfully'}
```

## Deleting Records with Query Filters

The `delete()` method allows you to delete multiple records that match specific criteria. The search query parameters are consistent across all RushDB APIs and follow the same structure as used in [search operations](../../concepts/search/introduction.md).

### Syntax

```python
db.records.delete(
    query: SearchQuery,
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | SearchQuery | Query to match records for deletion. See [Search Introduction](../../concepts/search/introduction.md) |
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

### Returns

A dictionary with the response data confirming the deletion.

### Examples

#### Basic Query Deletion

```python
# Delete records matching specific criteria
response = db.records.delete({
    "labels": ["PRODUCT"],  # See Labels in search: https://docs.rushdb.com/concepts/search/labels
    "where": {              # See Where clause: https://docs.rushdb.com/concepts/search/where
        "price": {"$lt": 10},
        "discontinued": True
    }
})

print(f"Deletion response: {response}")
# Example output: {'message': '5 record(s) deleted successfully'}
```

#### Advanced Query Deletion

```python
# Delete records with complex conditions using $or operator
response = db.records.delete({
    "where": {
        "$or": [  # Logical operators as described in Where clause documentation
            {
                "status": "archived",
                "lastModified": {"$lt": "2024-01-01T00:00:00Z"}
            },
            {
                "status": "inactive",
                "isTemporary": True
            }
        ]
    },
    "labels": ["DOCUMENT", "ATTACHMENT"]  # Records with either DOCUMENT or ATTACHMENT label
})

print(f"Deletion response: {response}")
# Example output: {'message': '12 record(s) deleted successfully'}
```

## Deleting Records from Record Objects

You can also delete records directly from Record objects.

### Syntax

```python
record.delete(
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

### Returns

A dictionary with the response data confirming the deletion.

### Example

```python
# Create a record
user = db.records.create(
    label="USER",
    data={
        "name": "John Doe",
        "email": "john@example.com"
    }
)

# Perform operations with the record
# ...

# Delete the record when no longer needed
response = user.delete()

print(f"Deletion response: {response}")
# Example output: {'message': 'Record deleted successfully'}
```

## Working with Transactions

For operations that need to be atomic, you can use transactions:

```python
# Start a transaction
tx = db.tx.begin()

try:
    # Create records in the transaction
    product1 = db.records.create(
        label="PRODUCT",
        data={"name": "Item 1", "price": 10.99},
        transaction=tx
    )

    product2 = db.records.create(
        label="PRODUCT",
        data={"name": "Item 2", "price": 20.99},
        transaction=tx
    )

    # Delete the first product
    db.records.delete_by_id(
        id_or_ids=product1.id,
        transaction=tx
    )

    # Delete other records matching a query
    db.records.delete(
        query={"labels": ["PRODUCT"], "where": {"discontinued": True}},
        transaction=tx
    )

    # Commit all changes
    tx.commit()
except Exception as e:
    # If any operation fails, roll back all changes
    tx.rollback()
    print(f"Transaction failed: {e}")
```

## Handling Relationships

When deleting records, all [relationships](../../concepts/relationships.md) associated with those records are automatically deleted. This ensures database integrity and prevents orphaned relationships.

## Best Practices

1. **Use IDs for specific deletions** when you know exactly which records to remove.
2. **Use queries for conditional deletions** when you need to delete records based on specific criteria.
3. **Use transactions** when deleting multiple related records to ensure data consistency.
4. **Consider performance** for large-scale deletions by using appropriate filters.
5. **Handle exceptions** properly to manage failed delete operations.
6. **Verify deletions** after bulk operations to ensure the expected records were removed.
7. **Use [label filtering](../../concepts/search/labels.md)** to narrow down the scope of deletion operations.
8. **Leverage search operators** from the [Where clause documentation](../../concepts/search/where.md) for precise targeting of records to delete.
9. **Remember that search parameters** are consistent across all RushDB operations, including [find()](../../concepts/search/introduction.md), delete(), and other methods.
