---
sidebar_position: 1
---

# Create Records

RushDB Python SDK provides flexible methods for creating [records](../../concepts/records.md). You can create single records or multiple records at once, with automatic data type inference and relationship handling.

## Overview

The Python SDK offers three main methods for creating records:
- `create()` - Create a single record with a label and data
- `create_many()` - Create multiple records in a batch operation
- `upsert()` - Create a new record or update an existing one based on matching criteria

Batch Upsert: You can also trigger upsert behavior directly in `create_many()` by providing `mergeBy` and/or `mergeStrategy` inside `options` (same semantics as single-record `upsert`). If either is present, each item in the batch will attempt to match and update existing records instead of always creating new ones.

All methods support options for controlling data processing and formatting.

## Prerequisites

Before creating records, make sure you have initialized the RushDB client with your API token:

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY", base_url="https://api.rushdb.com/api/v1")
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
| `options` | Optional[Dict[str, Any]] | Optional configuration parameters (including batch upsert) |
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

#### Options Dictionary

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `suggestTypes` | bool | `True` | **Default is `True`** - Automatically infers data types for [properties](../../concepts/properties.md). To disable type inference and store all values as strings, explicitly set to `False` |
| `castNumberArraysToVectors` | bool | `False` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | bool | `False` | When true, converts string numbers to number type |
| `capitalizeLabels` | bool | `False` | When true, converts all [labels](../../concepts/labels.md) to uppercase |
| `relationshipType` | str | `None` | Custom [relationship](../../concepts/relationships.md) type for nested objects |
| `returnResult` | bool | `True` | Whether to return the created records |
| `mergeBy` | List[str] | `[]` / omitted | Batch upsert match keys. Empty or omitted with `mergeStrategy` means use all incoming property keys. |
| `mergeStrategy` | str | `'append'` | `'append'` adds/updates; `'rewrite'` replaces all properties for matched records. |

:::info Default Behavior
By default, `suggestTypes` is set to `True` for all write operations (create, upsert, import). This means RushDB automatically infers data types from your values. To store all properties as strings without type inference, you must explicitly set `suggestTypes=False` in the options.
:::

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
        "returnResult": True,
        "mergeBy": ["name", "department"],  # enables batch upsert
        "mergeStrategy": "append"
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
        "relationshipType": "HAS_SPECS",  # Custom relationship for nested objects
        "mergeBy": ["name"],              # Upsert by product name
        "mergeStrategy": "rewrite"        # Replace product properties fully
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
        "returnResult": True,
        "mergeBy": ["name"],          # Company name uniqueness for idempotent import
        "mergeStrategy": "append"
    }
)
```

## Upserting Records

The `upsert()` method provides a powerful way to create or update records in a single operation. It attempts to find an existing record based on specified properties and either creates a new one or updates the existing record according to your chosen strategy.

### Syntax

```python
db.records.upsert(
    label: Optional[str],
    data: Dict[str, Any],
    options: Optional[Dict[str, Any]] = None,
    transaction: Optional[Transaction] = None
) -> Record
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `label` | Optional[str] | Optional [label](../../concepts/labels.md) for the record |
| `data` | Dict[str, Any] | Record data as key-value pairs |
| `options` | Optional[Dict[str, Any]] | Optional configuration parameters |
| `transaction` | Optional[Transaction] | Optional [transaction](../../concepts/transactions.mdx) object |

#### Options Dictionary

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `mergeBy` | List[str] | `[]` | Property names to match on. If empty/undefined, matches on all incoming properties |
| `mergeStrategy` | str | `'append'` | Strategy for updating: `'append'` (add/update properties, keep others) or `'rewrite'` (replace all properties) |
| `suggestTypes` | bool | `True` | **Default is `True`** - Automatically infers data types for [properties](../../concepts/properties.md). To disable type inference and store all values as strings, explicitly set to `False` |
| `castNumberArraysToVectors` | bool | `False` | When true, converts numeric arrays to vector type |
| `convertNumericValuesToNumbers` | bool | `False` | When true, converts string numbers to number type |

:::info Default Behavior
By default, `suggestTypes` is set to `True` for all write operations including upsert. This means RushDB automatically infers data types from your values. To store all properties as strings without type inference, you must explicitly set `suggestTypes=False` in the options.
:::

### Returns

A `Record` object representing the created or updated record.

### Merge Strategies

#### Append Strategy

The `append` strategy (default) updates or adds properties while preserving existing ones:

```python
# Initial create
product = db.records.upsert(
    label="Product",
    data={
        "sku": "SKU-001",
        "name": "Widget",
        "price": 10.0,
        "category": "Tools"
    },
    options={
        "mergeBy": ["sku"],
        "mergeStrategy": "append",
        "suggestTypes": True
    }
)

# Update price and add stock - name and category are preserved
updated = db.records.upsert(
    label="Product",
    data={
        "sku": "SKU-001",
        "price": 15.0,
        "stock": 100
    },
    options={
        "mergeBy": ["sku"],
        "mergeStrategy": "append",
        "suggestTypes": True
    }
)

print(updated.data)
# {
#   "sku": "SKU-001",
#   "name": "Widget",      # Preserved
#   "category": "Tools",   # Preserved
#   "price": 15.0,         # Updated
#   "stock": 100           # Added
# }
```

#### Rewrite Strategy

The `rewrite` strategy replaces all properties with the incoming data:

```python
# Rewrite - removes unspecified fields
rewritten = db.records.upsert(
    label="Product",
    data={
        "sku": "SKU-001",
        "name": "New Widget",
        "price": 20.0
    },
    options={
        "mergeBy": ["sku"],
        "mergeStrategy": "rewrite",
        "suggestTypes": True
    }
)

print(rewritten.data)
# {
#   "sku": "SKU-001",
#   "name": "New Widget",
#   "price": 20.0
#   # category and stock are removed
# }
```

### Common Use Cases

#### Idempotent Data Imports

```python
from datetime import datetime

# Can be safely run multiple times without creating duplicates
user = db.records.upsert(
    label="User",
    data={
        "email": "john@example.com",
        "name": "John Doe",
        "lastLogin": datetime.now().isoformat()
    },
    options={
        "mergeBy": ["email"],
        "mergeStrategy": "append",
        "suggestTypes": True
    }
)
```

#### Multi-Tenant Applications

```python
# Match on both tenant and entity identifiers
setting = db.records.upsert(
    label="Setting",
    data={
        "tenantId": "tenant-123",
        "userId": "user-456",
        "theme": "dark",
        "notifications": True
    },
    options={
        "mergeBy": ["tenantId", "userId"],
        "mergeStrategy": "append",
        "suggestTypes": True
    }
)
```

#### Configuration Management

```python
from datetime import datetime

# Update configuration by key
config = db.records.upsert(
    label="Config",
    data={
        "key": "api_timeout",
        "value": 30000,
        "updatedAt": datetime.now().isoformat()
    },
    options={
        "mergeBy": ["key"],
        "mergeStrategy": "append",
        "suggestTypes": True
    }
)
```

#### Inventory Updates

```python
from datetime import datetime

# Update stock while preserving product details
inventory = db.records.upsert(
    label="Product",
    data={
        "productCode": "PROD-789",
        "stock": 50,
        "lastRestocked": datetime.now().isoformat()
    },
    options={
        "mergeBy": ["productCode"],
        "mergeStrategy": "append",
        "suggestTypes": True
    }
)
```

#### Switching Between Strategies

```python
# Start with append to build up data
doc = db.records.upsert(
    label="Document",
    data={
        "docId": "DOC-1",
        "title": "My Document",
        "content": "Initial content",
        "version": 1
    },
    options={
        "mergeBy": ["docId"],
        "mergeStrategy": "append"
    }
)

# Add more fields with append
doc = db.records.upsert(
    label="Document",
    data={
        "docId": "DOC-1",
        "author": "John Doe",
        "tags": ["important", "draft"]
    },
    options={
        "mergeBy": ["docId"],
        "mergeStrategy": "append"
    }
)

# Clean slate with rewrite
doc = db.records.upsert(
    label="Document",
    data={
        "docId": "DOC-1",
        "title": "Final Document",
        "version": 2
    },
    options={
        "mergeBy": ["docId"],
        "mergeStrategy": "rewrite"
    }
)
# Now only docId, title, and version remain
```

### Matching Behavior

#### With Specific MergeBy Fields

When `mergeBy` contains specific field names, only those fields are used for matching:

```python
# Matches only on 'email'
user = db.records.upsert(
    label="User",
    data={
        "email": "user@example.com",
        "name": "John",
        "age": 30
    },
    options={
        "mergeBy": ["email"],
        "mergeStrategy": "append"
    }
)
```

#### Without MergeBy (All Properties Match)

When `mergeBy` is empty or undefined, matching is performed on all incoming properties:

```python
# Matches only if ALL properties (email, name, age) match exactly
user = db.records.upsert(
    label="User",
    data={
        "email": "user@example.com",
        "name": "John",
        "age": 30
    },
    options={
        "mergeStrategy": "append"
    }
)

# This would create a new record (age doesn't match)
different = db.records.upsert(
    label="User",
    data={
        "email": "user@example.com",
        "name": "John",
        "age": 31
    },
    options={
        "mergeStrategy": "append"
    }
)
```

### Using with Transactions

```python
# Begin transaction
tx = db.tx.begin()

try:
    # Upsert product
    product = db.records.upsert(
        label="Product",
        data={
            "sku": "SKU-001",
            "name": "Widget",
            "price": 10.0
        },
        options={
            "mergeBy": ["sku"],
            "mergeStrategy": "append"
        },
        transaction=tx
    )

    # Upsert inventory
    inventory = db.records.upsert(
        label="Inventory",
        data={
            "productSku": "SKU-001",
            "quantity": 100,
            "warehouse": "A"
        },
        options={
            "mergeBy": ["productSku", "warehouse"],
            "mergeStrategy": "append"
        },
        transaction=tx
    )

    # Commit transaction
    tx.commit()
except Exception as e:
    # Rollback on error
    tx.rollback()
    raise e
```

### Working with Complex Data Types

```python
# Upsert with various data types
config = db.records.upsert(
    label="Config",
    data={
        "configId": "config-1",
        "enabled": True,
        "maxRetries": 3,
        "timeout": 30.5,
        "tags": ["production", "critical"],
        "vector": [0.1, 0.2, 0.3, 0.4]
    },
    options={
        "mergeBy": ["configId"],
        "mergeStrategy": "append",
        "suggestTypes": True,
        "castNumberArraysToVectors": True
    }
)

# Update some values
updated = db.records.upsert(
    label="Config",
    data={
        "configId": "config-1",
        "enabled": False,
        "maxRetries": 5
    },
    options={
        "mergeBy": ["configId"],
        "mergeStrategy": "append"
    }
)
# timeout, tags, and vector are preserved
```

## Best Practices

1. **Automatic Type Inference is Enabled by Default**: RushDB automatically infers data types with `suggestTypes=True` for all write operations (create, create_many, upsert). This is the recommended approach for most use cases. Only set `suggestTypes=False` if you explicitly need all values stored as strings.

2. **Batch Operations**: Use `create_many()` for better performance when creating multiple [records](../../concepts/records.md).

3. **Nested Data**: Use nested objects and arrays to create related records automatically with proper [relationships](../../concepts/relationships.md).

4. **Transactions**: For operations that need to be atomic, use the optional [transaction](../../concepts/transactions.mdx) parameter.

5. **Data Validation**: Validate your data on the client side before sending it to RushDB.

6. **Label Convention**: Consider using uppercase for [labels](../../concepts/labels.md) (e.g., "PERSON" instead of "Person") for consistency with graph database conventions.

7. **Choose Appropriate MergeBy Fields for Upsert**: Use fields that uniquely identify your records (like `email`, `sku`, `userId`) to ensure proper matching.

8. **Select the Right Merge Strategy**: Use `append` to preserve existing data and update specific fields; use `rewrite` for complete record replacement.

9. **Use Upsert for Idempotent Operations**: Upsert is ideal for data synchronization and import operations where you want to safely re-run operations without creating duplicates.
