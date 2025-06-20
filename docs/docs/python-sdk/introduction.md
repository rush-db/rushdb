---
title: Introduction
sidebar_position: 0
---

# RushDB Python SDK

The RushDB Python SDK provides a powerful, intuitive interface for interacting with RushDB from Python applications. Whether you're building data science pipelines, web applications, or AI-driven services, this SDK offers a clean, Pythonic way to work with your graph data.

## Features

- **Intuitive API Design**: Simple methods that map directly to common database operations
- **Type Hinting Support**: Comprehensive type annotations for better IDE support
- **Transaction Management**: ACID-compliant transactions with context manager support
- **Flexible Query System**: Expressive query capabilities without learning a graph query language
- **Vector Support**: Built-in handling for vector embeddings and similarity search
- **Data Import Tools**: Easy import of structured data from JSON, CSV, and other formats

## Installation

Install the RushDB Python SDK using pip:

```bash
pip install rushdb
```

## Quick Start

### Initialize Client

```python
from rushdb import RushDB

# Connect to RushDB with your API token
db = RushDB("RUSHDB_API_TOKEN")
```

### Basic Operations

```python
# Create a record
user = db.records.create(
    label="USER",
    data={
        "name": "John Doe",
        "email": "john@example.com",
        "age": 30
    },
    options={"suggestTypes": True}
)

# Find records
result = db.records.find({
    "where": {
        "age": {"$gte": 18},
        "name": {"$startsWith": "J"}
    },
    "limit": 10
})

# Iterate over results
for user in result:
    print(f"Found user: {user.get('name')}")

# Check result metadata
print(f"Found {len(result)} users out of {result.total} total")

# Update a record
user.update({
    "last_login": "2025-05-04T12:30:45Z"
})

# Create relationships
company = db.records.create(
    label="COMPANY",
    data={"name": "Acme Inc."}
)

# Attach records with a relationship
user.attach(
    target=company,
    options={"type": "WORKS_AT", "direction": "out"}
)
```

## Using Transactions

Ensure data consistency with transactions:

```python
# Begin a transaction
with db.transactions.begin() as transaction:
    # Create a user
    user = db.records.create(
        label="USER",
        data={"name": "Alice Smith"},
        transaction=transaction
    )

    # Create a product
    product = db.records.create(
        label="PRODUCT",
        data={"name": "Smartphone", "price": 799.99},
        transaction=transaction
    )

    # Create a purchase relationship
    user.attach(
        target=product,
        options={"type": "PURCHASED", "direction": "out"},
        transaction=transaction
    )

    # Everything will be committed if no errors occur
    # If an error occurs, the transaction will be automatically rolled back
```

## Next Steps

Explore the detailed documentation for each component of the SDK:

- [Records](./records/create-records.md) - Create, read, update, and delete record operations
- [Properties](./properties.md) - Manage data properties
- [Labels](./labels.md) - Work with node labels
- [Relationships](./relationships.md) - Handle connections between records
- [Transactions](./transactions.md) - Manage transaction operations for data consistency

For more advanced use cases, check our [Tutorials](../tutorials/reusable-search-query) section.
