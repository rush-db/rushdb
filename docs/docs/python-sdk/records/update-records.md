---
sidebar_position: 6
---

# Update Records

RushDB Python SDK provides two main methods for updating [records](../../concepts/records.md): `update()` for partial updates and `set()` for complete replacement of record data.

## Overview

The update methods allow you to:
- Update specific properties while preserving others (`update()`)
- Completely replace record data (`set()`)
- Apply changes either through the RecordsAPI or directly on Record objects

## Prerequisites

Before updating records, make sure you have initialized the RushDB client with your API token:

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY", base_url="https://api.rushdb.com/api/v1")
```

## Updating Records with `update()`

The `update()` method allows you to modify specific properties of a record while preserving other existing properties.

### Syntax

```python
# Using RecordsAPI
db.records.update(
    record_id: str,
    data: Dict[str, Any],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]

# Using Record object
record.update(
    data: Dict[str, Any],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `record_id` | str | ID of the [record](../../concepts/records.md) to update (when using RecordsAPI) |
| `data` | Dict[str, Any] | Partial record data containing only the properties to update |
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

### Returns

A dictionary with the response data confirming the update.

### Examples

#### Using RecordsAPI

```python
# First, create or retrieve a record
person = db.records.create(
    label="PERSON",
    data={
        "name": "John Doe",
        "age": 30,
        "email": "john@example.com",
        "active": True
    }
)

# Later, update specific properties using the record's ID
response = db.records.update(
    record_id=person.id,
    data={
        "age": 31,
        "title": "Senior Developer",
        "active": False
    }
)

# The record now contains both original and updated properties:
# name: "John Doe" (preserved)
# age: 31 (updated)
# email: "john@example.com" (preserved)
# active: False (updated)
# title: "Senior Developer" (added)
```

#### Using Record Object

```python
# If you have a record object, you can update it directly
person = db.records.create(
    label="PERSON",
    data={
        "name": "Jane Smith",
        "age": 28,
        "department": "Engineering"
    }
)

# Update the record
response = person.update({
    "age": 29,
    "department": "Product",
    "role": "Product Manager"
})

# The record now contains:
# name: "Jane Smith" (preserved)
# age: 29 (updated)
# department: "Product" (updated)
# role: "Product Manager" (added)
```

## Replacing Records with `set()`

The `set()` method completely replaces all properties of a record with new data.

### Syntax

```python
# Using RecordsAPI
db.records.set(
    record_id: str,
    data: Dict[str, Any],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]

# Using Record object
record.set(
    data: Dict[str, Any],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `record_id` | str | ID of the [record](../../concepts/records.md) to replace (when using RecordsAPI) |
| `data` | Dict[str, Any] | New record data that will completely replace existing properties |
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

### Returns

A dictionary with the response data confirming the replacement.

### Examples

#### Using RecordsAPI

```python
# First, create or retrieve a record
product = db.records.create(
    label="PRODUCT",
    data={
        "name": "Smartphone X",
        "price": 899.99,
        "category": "Electronics",
        "features": ["5G", "Water Resistant"]
    }
)

# Later, completely replace the record data
response = db.records.set(
    record_id=product.id,
    data={
        "name": "Smartphone X Pro",
        "price": 1099.99,
        "inStock": True,
        "specifications": {
            "storage": "256GB",
            "color": "Midnight Blue"
        }
    }
)

# The record now ONLY contains the new properties:
# name: "Smartphone X Pro"
# price: 1099.99
# inStock: True
# specifications: { storage: "256GB", color: "Midnight Blue" }
# Note: "category" and "features" properties are removed
```

#### Using Record Object

```python
# If you have a record object, you can replace it directly
product = db.records.create(
    label="PRODUCT",
    data={
        "name": "Laptop Basic",
        "price": 699.99,
        "category": "Computer"
    }
)

# Replace all record data
response = product.set({
    "name": "Laptop Pro",
    "price": 1299.99,
    "memory": "16GB",
    "processor": "i7"
})

# The record now ONLY contains:
# name: "Laptop Pro"
# price: 1299.99
# memory: "16GB"
# processor: "i7"
# Note: "category" property is removed
```

## Working with Transactions

For operations that need to be atomic, you can use transactions:

```python
# Start a transaction
tx = db.tx.begin()

try:
    # Update multiple records in the same transaction
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

    # Update first product
    db.records.update(
        record_id=product1.id,
        data={"price": 11.99, "featured": True},
        transaction=tx
    )

    # Replace second product
    db.records.set(
        record_id=product2.id,
        data={"name": "Item 2 Pro", "price": 29.99, "featured": True},
        transaction=tx
    )

    # Commit all changes
    tx.commit()
except Exception as e:
    # If any operation fails, roll back all changes
    tx.rollback()
    print(f"Transaction failed: {e}")
```

## Best Practices

1. **Use `update()` for partial updates** when you want to preserve existing data.
2. **Use `set()` for complete replacement** when you want to ensure the record only has the properties you specify.
3. **Use Record objects directly** for more concise code when you already have a reference to the record.
4. **Use transactions** when updating multiple related records to ensure data consistency.
5. **Validate data** on the client side before sending update requests.
6. **Handle exceptions** properly to manage failed update operations.
