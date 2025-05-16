---
sidebar_position: 1
---

# Create Records

RushDB Python SDK provides flexible methods for creating [records](../../concepts/records.md). You can create single records or multiple records at once, with automatic data type inference and relationship handling.

## Overview

The Python SDK offers two main methods for creating records:
- `create()` - Create a single record with a label and data
- `create_many()` - Create multiple records in a batch operation

Both methods support options for controlling data processing and formatting.

## Prerequisites

Before creating records, make sure you have initialized the RushDB client with your API token:

```python
from rushdb import RushDB

db = RushDB("YOUR_API_TOKEN", base_url="https://api.rushdb.com")
```

## Creating a Single Record

The `create()` method creates a single record with the specified label and data.

### Syntax

```python
db.records.create(
    label: str,
    data: Dict[str, Any],
    options: Optional[Dict[str, bool]] = None,
    transaction: Optional[Transaction] = None
) -> Record
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `label` | str | [Label](../../concepts/labels.md) for the new record |
| `data` | Dict[str, Any] | Record data as key-value pairs |
| `options` | Optional[Dict[str, bool]] | Optional configuration parameters |
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

#### Options Dictionary

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `suggestTypes` | bool | `True` | When true, automatically infers data types for [properties](../../concepts/properties.md) |
| `castNumberArraysToVectors` | bool | `False` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | bool | `False` | When true, converts string numbers to number type |
| `capitalizeLabels` | bool | `False` | When true, converts all [labels](../../concepts/labels.md) to uppercase |
| `relationshipType` | str | `None` | Custom [relationship](../../concepts/relationships.md) type for nested objects |
| `returnResult` | bool | `True` | Whether to return the created record |

### Returns

A `Record` object representing the newly created record.

### Examples

#### Basic Record Creation

```python
# Create a simple record
person = db.records.create(
    label="PERSON",
    data={
        "name": "John Doe",
        "age": 30,
        "email": "john@example.com"
    }
)

print(f"Created record with ID: {person.id}")
print(f"Record label: {person.label}")
```

#### Record with Complex Data Types

```python
# Create a record with various data types
product = db.records.create(
    label="PRODUCT",
    data={
        "name": "Smartphone X",
        "price": 899.99,
        "isAvailable": True,
        "tags": ["electronics", "smartphone", "new"],
        "releaseDate": "2025-03-15T00:00:00Z",
        "specifications": {
            "dimensions": "150x70x8mm",
            "weight": "180g",
            "color": "Midnight Blue"
        },
        "ratings": [4.7, 4.8, 4.9, 5.0]  # Could be converted to a vector
    },
    options={
        "returnResult": True,
        "suggestTypes": True,
        "castNumberArraysToVectors": True
    }
)
```

#### With Type Control

When you need precise control over how data types are handled:

```python
# Create a record with explicit options
customer = db.records.create(
    label="customer",  # Will be capitalized to "CUSTOMER"
    data={
        "id": "C-12345",  # Will be stored as string
        "name": "Jane Smith",
        "joinDate": "2025-01-20T09:30:00Z",  # Will be stored as datetime
        "loyalty_points": "250",  # Will be converted to number
        "scores": ["95", "87", "92"]  # Will be converted to numbers
    },
    options={
        "suggestTypes": True,
        "convertNumericValuesToNumbers": True,
        "capitalizeLabels": True
    }
)

# Access the property types
print(customer.proptypes)
```

## Creating Multiple Records

The `create_many()` method allows you to create multiple records in a single operation, which is more efficient for batch operations.

### Syntax

```python
db.records.create_many(
    label: str,
    data: Union[Dict[str, Any], List[Dict[str, Any]]],
    options: Optional[Dict[str, bool]] = None,
    transaction: Optional[Transaction] = None
) -> List[Record]
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `label` | str | [Label](../../concepts/labels.md) for all created records |
| `data` | Union[Dict[str, Any], List[Dict[str, Any]]] | List of record data or a single dictionary |
| `options` | Optional[Dict[str, bool]] | Optional configuration parameters |
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

### Returns

A list of `Record` objects representing the newly created records.

### Examples

#### Creating Multiple Simple Records

```python
# Create multiple employee records
employees = db.records.create_many(
    label="EMPLOYEE",
    data=[
        {
            "name": "John Smith",
            "position": "Developer",
            "department": "Engineering"
        },
        {
            "name": "Sarah Johnson",
            "position": "Product Manager",
            "department": "Product"
        },
        {
            "name": "Michael Chen",
            "position": "Data Scientist",
            "department": "Data"
        }
    ],
    options={
        "returnResult": True
    }
)

# Access the created records
for employee in employees:
    print(f"Created {employee.label} record: {employee.id}")
```

#### Working with Structured Data

```python
# Create records with relationships
products_data = [
    {
        "name": "Laptop Pro",
        "price": "1299.99",  # Will be converted to number
        "category": "Computers",
        "specs": {
            "processor": "i9 13th Gen",
            "memory": "32GB",
            "storage": "1TB SSD"
        },
        "inStock": True,
        "featureVector": [0.23, 0.45, 0.67, 0.89]  # Will be stored as vector
    },
    {
        "name": "Smartphone Ultra",
        "price": "999.99",  # Will be converted to number
        "category": "Phones",
        "specs": {
            "processor": "Snapdragon 8 Gen 3",
            "memory": "12GB",
            "storage": "512GB"
        },
        "inStock": False,
        "featureVector": [0.33, 0.55, 0.77, 0.99]  # Will be stored as vector
    }
]

products = db.records.create_many(
    label="product",  # Will be capitalized to "PRODUCT"
    data=products_data,
    options={
        "returnResult": True,
        "suggestTypes": True,
        "convertNumericValuesToNumbers": True,
        "castNumberArraysToVectors": True,
        "capitalizeLabels": True,
        "relationshipType": "HAS_SPECS"  # Custom relationship for nested objects
    }
)
```

#### With Nested Objects and Arrays

RushDB automatically handles nested objects and arrays by creating proper [relationships](../../concepts/relationships.md) between [records](../../concepts/records.md):

```python
# Create a company with employees as nested objects
company_data = {
    "name": "Tech Innovations Inc.",
    "founded": "2020-01-01T00:00:00Z",
    "location": "San Francisco, CA",
    "employees": [
        {
            "name": "Alice Cooper",
            "role": "CEO",
            "joinDate": "2020-01-01T00:00:00Z"
        },
        {
            "name": "Bob Williams",
            "role": "CTO",
            "joinDate": "2020-02-15T00:00:00Z"
        }
    ]
}

# This will create a COMPANY record connected to two EMPLOYEE records
# with custom relationship type "EMPLOYS"
result = db.records.create_many(
    label="COMPANY",
    data=company_data,
    options={
        "relationshipType": "EMPLOYS",
        "capitalizeLabels": True,
        "returnResult": True
    }
)
```

## Best Practices

1. **Use Type Inference**: Let RushDB automatically infer data types with `suggestTypes: True` for most use cases.

2. **Batch Operations**: Use `create_many()` for better performance when creating multiple [records](../../concepts/records.md).

3. **Nested Data**: Use nested objects and arrays to create related records automatically with proper [relationships](../../concepts/relationships.md).

4. **Transactions**: For operations that need to be atomic, use the optional [transaction](../../concepts/transactions.mdx) parameter.

5. **Data Validation**: Validate your data on the client side before sending it to RushDB.

6. **Label Convention**: Consider using uppercase for [labels](../../concepts/labels.md) (e.g., "PERSON" instead of "Person") for consistency with graph database conventions.
