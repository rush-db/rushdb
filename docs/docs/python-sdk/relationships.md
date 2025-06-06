---
sidebar_position: 4
---

# Relationships

[Relationships](../concepts/relationships.md) in RushDB connect records to form a rich, interconnected network of data. The Python SDK provides powerful methods for creating, managing, and traversing relationships between records.

## Overview

Relationships in RushDB enable you to:
- Connect related records
- Model complex domain relationships
- Query data based on connections
- Build graph-like data structures
- Navigate between connected entities

## Prerequisites

Before working with relationships, make sure you have initialized the RushDB client with your API token:

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_TOKEN", base_url="https://api.rushdb.com/api/v1")
```

## Creating Records with Relationships

When creating records, you can automatically establish relationships through nested objects:

```python
# Create a company with departments and employees
company_data = {
    "name": "Acme Inc.",
    "founded": "2010-01-15T00:00:00Z",
    "departments": [  # This creates relationships to DEPARTMENT records
        {
            "name": "Engineering",
            "employees": [  # This creates relationships to EMPLOYEE records
                {
                    "name": "Alice Chen",
                    "position": "Senior Developer"
                },
                {
                    "name": "Bob Smith",
                    "position": "QA Engineer"
                }
            ]
        },
        {
            "name": "Marketing",
            "employees": [
                {
                    "name": "Carol Davis",
                    "position": "Marketing Director"
                }
            ]
        }
    ]
}

# Create the company with all related records
records = db.records.create_many(
    label="COMPANY",
    data=company_data,
    options={
        "relationshipType": "HAS_DEPARTMENT",  # Custom relationship type for departments
        "returnResult": True
    }
)
```

## Explicitly Creating Relationships with attach()

You can also explicitly create relationships between existing records using the `attach()` method:

### Using RecordsAPI

```python
# Create two separate records
user = db.records.create(
    label="USER",
    data={"name": "John Doe", "email": "john@example.com"}
)

project = db.records.create(
    label="PROJECT",
    data={"name": "Website Redesign", "deadline": "2025-06-30T00:00:00Z"}
)

# Create a relationship between them
response = db.records.attach(
    source=user.id,
    target=project.id,
    options={
        "type": "MANAGES",  # Relationship type
        "direction": "out"  # Relationship direction (user -> project)
    }
)
```

### Using Record Objects Directly

```python
# Create two separate records
team = db.records.create(
    label="TEAM",
    data={"name": "Frontend Team", "size": 5}
)

employee = db.records.create(
    label="EMPLOYEE",
    data={"name": "Alice Johnson", "role": "Developer"}
)

# Create a relationship from the team to the employee
response = team.attach(
    target=employee,
    options={
        "type": "INCLUDES",
        "direction": "out"  # From team to employee
    }
)
```

## Creating Multiple Relationships at Once

```python
# Create a manager record
manager = db.records.create(
    label="MANAGER",
    data={"name": "Sarah Williams", "department": "Engineering"}
)

# Create multiple employee records
employees = db.records.create_many(
    label="EMPLOYEE",
    data=[
        {"name": "John Smith", "skills": ["Python", "JavaScript"]},
        {"name": "Jane Brown", "skills": ["Java", "SQL"]},
        {"name": "Mike Davis", "skills": ["React", "TypeScript"]}
    ]
)

# Create relationships from the manager to all employees at once
response = manager.attach(
    target=[emp.id for emp in employees],
    options={
        "type": "MANAGES",
        "direction": "out"
    }
)
```

## Removing Relationships with detach()

You can remove relationships between records without deleting the records themselves using the `detach()` method:

### Using RecordsAPI

```python
# Remove a specific relationship type
db.records.detach(
    source=user.id,
    target=project.id,
    options={
        "typeOrTypes": "MANAGES",
        "direction": "out"
    }
)

# Remove multiple relationship types at once
db.records.detach(
    source=manager.id,
    target=employee.id,
    options={
        "typeOrTypes": ["MANAGES", "MENTORS"],
        "direction": "out"
    }
)
```

### Using Record Objects Directly

```python
# Remove a relationship directly from a record object
team.detach(
    target=employee,
    options={
        "typeOrTypes": "INCLUDES",
        "direction": "out"
    }
)
```

## Finding Related Records

You can find records based on their relationships:

```python
# Find all employees of a specific department
employees = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {
        "_in": {  # Use _in to find incoming relationships
            "relation": "WORKS_IN",  # Relationship type
            "source": department.id  # The source record ID
        }
    }
})

# Find all projects managed by a specific user
projects = db.records.find({
    "labels": ["PROJECT"],
    "where": {
        "_in": {
            "relation": "MANAGES",
            "source": user.id
        }
    }
})
```

## Using Custom Relationship Types

By default, RushDB uses a standard relationship type, but you can specify custom types:

```python
# When creating records with nested objects
company = db.records.create_many(
    label="COMPANY",
    data={
        "name": "Tech Corp",
        "employees": [
            {"name": "Jane Smith", "position": "CTO"},
            {"name": "John Doe", "position": "Lead Developer"}
        ]
    },
    options={
        "relationshipType": "EMPLOYS"  # Custom relationship type
    }
)

# When explicitly creating relationships
db.records.attach(
    source=mentor.id,
    target=mentee.id,
    options={
        "type": "MENTORS",
        "direction": "out"
    }
)
```

## Relationship Direction

Relationships in RushDB have direction. You can specify the direction when creating or querying relationships:

- `"out"` - Relationship goes from source to target (source → target)
- `"in"` - Relationship goes from target to source (target → source)

```python
# Creating an outgoing relationship (source → target)
user.attach(
    target=group,
    options={
        "type": "BELONGS_TO",
        "direction": "out"
    }
)

# Creating an incoming relationship (target → source)
project.attach(
    target=employee,
    options={
        "type": "WORKS_ON",
        "direction": "in"  # Employee → Project
    }
)
```

## Working with Transactions

For operations that need to be atomic, you can use [transactions](../concepts/transactions.mdx) when creating or modifying relationships:

```python
# Start a transaction
tx = db.tx.begin()

try:
    # Create records in the transaction
    team = db.records.create(
        label="TEAM",
        data={"name": "Product Team"},
        transaction=tx
    )

    member1 = db.records.create(
        label="EMPLOYEE",
        data={"name": "Alice"},
        transaction=tx
    )

    member2 = db.records.create(
        label="EMPLOYEE",
        data={"name": "Bob"},
        transaction=tx
    )

    # Create relationships in the same transaction
    team.attach(
        target=[member1, member2],
        options={"type": "HAS_MEMBER"},
        transaction=tx
    )

    # Commit all changes
    tx.commit()
except Exception as e:
    # If any operation fails, roll back all changes
    tx.rollback()
    print(f"Transaction failed: {e}")
```

## Best Practices for Working with Relationships

1. **Use meaningful relationship types** - Choose relationship types that clearly express the connection's nature (e.g., "MANAGES", "BELONGS_TO")

2. **Consider relationship direction** - Think about which way the relationship should point based on your domain model

3. **Use nested objects for hierarchical data** - When creating hierarchical data, structure your JSON to reflect the relationships

4. **Create relationships in transactions** - Use transactions when creating multiple related records to ensure data consistency

5. **Be consistent with relationship types** - Use the same relationship types for similar connections throughout your application

6. **Think in graphs** - Approach relationships as a graph model, considering paths between records

7. **Balance denormalization and relationships** - In some cases, it may be better to duplicate data rather than create complex relationship chains

## Related Documentation

- [Relationships Concept](../concepts/relationships.md) - Learn more about how relationships work in RushDB
- [Transactions](../concepts/transactions.mdx) - Using transactions for relationship consistency
- [Record Creation](./records/create-records.md) - Creating records with relationships
- [Finding Records](./records/get-records.md) - Search techniques including relationship-based queries
