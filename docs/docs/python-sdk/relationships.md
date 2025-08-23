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

db = RushDB("RUSHDB_API_KEY", base_url="https://api.rushdb.com/api/v1")
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

## Bulk Relationship Creation by Key Match

When importing tabular data in separate steps, you can create relationships in bulk by matching a key on the source label to a key on the target label. Use `relationships.create_many` for this.

```python
# Create USER -[:ORDERED]-> ORDER for all pairs where
# USER.id = ORDER.userId and both belong to the same tenant
tenant_id = "ACME"

db.relationships.create_many(
        source={"label": "USER", "key": "id", "where": {"tenantId": tenant_id}},
        target={"label": "ORDER", "key": "userId", "where": {"tenantId": tenant_id}},
        type="ORDERED",
        direction="out"  # (source) -[:ORDERED]-> (target)
)
```

Parameters
- `source`: Dict describing the source side
    - `label` (str): Source record label
    - `key` (str): Property on the source used for equality match
    - `where` (optional, dict): Additional filters for source records; same shape as SearchQuery `where`
- `target`: Dict describing the target side
    - `label` (str): Target record label
    - `key` (str): Property on the target used for equality match
    - `where` (optional, dict): Additional filters for target records; same shape as SearchQuery `where`
- `type` (optional, str): Relationship type. Defaults to the RushDB default type when omitted
- `direction` (optional, str): 'in' or 'out'. Defaults to 'out'
- `transaction` (optional): Include to run the operation atomically

Notes
- The join condition is always `source[key] = target[key]` combined with any additional `where` constraints.
- `where` follows the same operators as record search (e.g., `{"tenantId": "ACME"}` or `{"tenantId": "ACME"}`).
- This is efficient for connecting data created in separate imports (e.g., users and orders).

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
result = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {
        "_in": {  # Use _in to find incoming relationships
            "relation": "WORKS_IN",  # Relationship type
            "source": department.id  # The source record ID
        }
    }
})

employees = result.data

# Find all projects managed by a specific user
result = db.records.find({
    "labels": ["PROJECT"],
    "where": {
        "_in": {
            "relation": "MANAGES",
            "source": user.id
        }
    }
})

projects = result.data
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

## Searching and Querying Relationships with RelationsAPI

The `RelationsAPI` provides dedicated functionality for searching and analyzing relationships directly. This API allows you to query relationships themselves rather than records, giving you insights into the connections within your graph.

**Important**: The RelationsAPI uses a Record-centric approach. When filtering relationships, you specify properties of the records involved in those relationships, not properties of the relationships themselves. This means the `where` clause contains Record properties to find relationships involving records that match those criteria.

### Overview

The RelationsAPI enables you to:
- Search for specific relationships based on criteria
- Analyze relationship patterns across your data
- Discover connections between records
- Perform relationship-based analytics
- Monitor relationship types and their usage

### Accessing the RelationsAPI

You access the RelationsAPI through the main RushDB client:

```python
from rushdb import RushDB

# Initialize the client
db = RushDB("RUSHDB_API_KEY", base_url="https://api.rushdb.com/api/v1")

# Access the relationships API
relationships_api = db.relationships
```

### The find() Method

The `find()` method is the primary way to search for relationships in your database. It supports the same powerful SearchQuery pattern used throughout RushDB.

#### Method Signature

```python
async def find(
    self,
    search_query: Optional[SearchQuery] = None,
    pagination: Optional[PaginationParams] = None,
    transaction: Optional[Union[Transaction, str]] = None,
) -> List[Relationship]
```

#### Parameters

- **search_query** (`Optional[SearchQuery]`): Search criteria to filter relationships. Uses a Record-centric approach where the `where` clause contains Record properties to find relationships involving records that match those criteria:
  - `where`: Filter conditions for record properties (not relationship properties)
  - `labels`: Filter by record labels to find relationships involving specific record types
  - `orderBy`: Sort relationships by various criteria
- **pagination** (`Optional[PaginationParams]`): Control result set size and pagination:
  - `limit` (int): Maximum number of relationships to return (default: 100, max: 1000)
  - `skip` (int): Number of relationships to skip for pagination (default: 0)
- **transaction** (`Optional[Union[Transaction, str]]`): Optional transaction context for the operation

#### Return Value

Returns a `List[Relationship]` containing relationship objects that match the search criteria. Each relationship object contains information about the source record, target record, relationship type, and direction.

### Basic Relationship Searching

#### Find All Relationships

```python
# Get all relationships in the database
all_relationships = await db.relationships.find()

print(f"Total relationships: {len(all_relationships)}")
for rel in all_relationships[:5]:  # Show first 5
    print(f"{rel.sourceId} -> {rel.targetId} ({rel.type})")
```

#### Find Relationships with Pagination

```python
# Get relationships with pagination
pagination_params = {
    "limit": 50,
    "skip": 0
}

first_page = await db.relationships.find(pagination=pagination_params)

# Get next page
next_page_params = {
    "limit": 50,
    "skip": 50
}

second_page = await db.relationships.find(pagination=next_page_params)
```

### Advanced Relationship Queries

#### Filter by Record Properties

The RelationsAPI uses a Record-centric approach. You specify record properties in the `where` clause to find relationships involving records that match those criteria:

```python
# Find relationships involving active users in Engineering department
engineering_relationships = await db.relationships.find({
    "where": {
        "isActive": True,
        "department": "Engineering"
    }
})

# Find relationships involving large technology companies
tech_company_relationships = await db.relationships.find({
    "where": {
        "industry": "Technology",
        "employees": {"$gte": 100}
    }
})

# Find relationships involving records with specific labels
user_relationships = await db.relationships.find({
    "labels": ["USER"]  # Only find relationships involving USER records
})

# Find relationships involving records matching complex criteria
senior_dev_relationships = await db.relationships.find({
    "where": {
        "role": "Developer",
        "experience": {"$gte": 5},
        "isActive": True
    }
})
```

#### Complex Relationship Queries

```python
# Find relationships involving engineering employees who are active
engineering_relationships = await db.relationships.find({
    "where": {
        "$and": [
            {"department": "Engineering"},
            {"isActive": True},
            {"role": {"$in": ["Developer", "QA Engineer", "DevOps"]}}
        ]
    },
    "orderBy": {"name": "asc"}
})

# Find relationships involving recently created records
recent_record_relationships = await db.relationships.find({
    "where": {
        "createdAt": {"$gte": "2024-01-01T00:00:00Z"}
    },
    "orderBy": {"createdAt": "desc"}
}, pagination={"limit": 25, "skip": 0})

# Find relationships involving records with specific labels and properties
manager_relationships = await db.relationships.find({
    "labels": ["MANAGER"],
    "where": {
        "department": "Engineering",
        "teamSize": {"$gte": 5}
    }
})
```

### Relationship Analytics and Insights

#### Count Relationships by Type

```python
# Get all relationships and analyze by type
all_relationships = await db.relationships.find()

# Count by type
type_counts = {}
for rel in all_relationships:
    rel_type = rel.type
    type_counts[rel_type] = type_counts.get(rel_type, 0) + 1

print("Relationship types and counts:")
for rel_type, count in sorted(type_counts.items()):
    print(f"  {rel_type}: {count}")
```

#### Find Highly Connected Records

```python
# Find relationships involving all records first
all_relationships = await db.relationships.find()

# Count outgoing relationships per record
outgoing_counts = {}
for rel in all_relationships:
    source_id = rel.sourceId
    outgoing_counts[source_id] = outgoing_counts.get(source_id, 0) + 1

# Find top 10 most connected records
top_connected = sorted(outgoing_counts.items(), key=lambda x: x[1], reverse=True)[:10]
print("Most connected records (outgoing):")
for record_id, count in top_connected:
    print(f"  {record_id}: {count} relationships")

# Alternative: Find relationships for specific high-activity records
manager_relationships = await db.relationships.find({
    "labels": ["MANAGER"],
    "where": {
        "isActive": True,
        "teamSize": {"$gte": 10}  # Managers with large teams likely have many relationships
    }
})
```

### Using Relationships API with Transactions

The RelationsAPI supports transactions for consistent querying:

```python
# Start a transaction
tx = db.tx.begin()

try:
    # Query relationships involving records in Sales department within the transaction
    relationships = await db.relationships.find({
        "where": {
            "department": "Sales"
        }
    }, transaction=tx)

    # Perform additional operations in the same transaction
    for rel in relationships:
        # Update related records or create new relationships
        pass

    # Commit the transaction
    tx.commit()
except Exception as e:
    # Roll back on error
    tx.rollback()
    print(f"Transaction failed: {e}")
```

### Pagination Best Practices

When working with large numbers of relationships, use pagination effectively:

```python
# Process relationships in batches
async def process_all_relationships(batch_size=100):
    skip = 0
    processed_count = 0

    while True:
        # Get next batch
        relationships = await db.relationships.find(
            pagination={"limit": batch_size, "skip": skip}
        )

        if not relationships:
            break  # No more relationships

        # Process this batch
        for rel in relationships:
            # Process individual relationship
            processed_count += 1
            print(f"Processing relationship {rel.sourceId} -> {rel.targetId}")

        # Move to next batch
        skip += batch_size

        print(f"Processed {processed_count} relationships so far...")

    print(f"Finished processing {processed_count} total relationships")

# Run the batch processor
await process_all_relationships()
```

### Performance Considerations

When using the RelationsAPI:

1. **Use specific filters**: Apply `where` conditions to reduce the result set
2. **Limit result sizes**: Use pagination to avoid loading too many relationships at once
3. **Filter by relationship type**: Use type filters when you know the specific relationship types you need
4. **Index frequently queried properties**: Ensure properties used in filters are indexed
5. **Combine with record queries**: Use RelationsAPI to discover connections, then use RecordsAPI for detailed record data

### Error Handling

```python
try:
    relationships = await db.relationships.find({
        "where": {"department": "InvalidDepartment"}
    })
except Exception as e:
    print(f"Error querying relationships: {e}")
    # Handle the error appropriately
```

### Integration with Record Operations

The RelationsAPI works seamlessly with record operations:

```python
# 1. Discover relationships involving specific types of records
management_rels = await db.relationships.find({
    "labels": ["MANAGER"],
    "where": {
        "department": "Engineering",
        "isActive": True
    }
})

# 2. Extract record IDs from relationships
manager_ids = [rel.sourceId for rel in management_rels]
employee_ids = [rel.targetId for rel in management_rels]

# 3. Query the actual records using RecordsAPI for detailed information
managers = db.records.find_by_id(manager_ids)
employees = db.records.find_by_id(employee_ids)

# 4. Combine data for analysis
for rel in management_rels:
    manager = next(m for m in managers if m.id == rel.sourceId)
    employee = next(e for e in employees if e.id == rel.targetId)
    print(f"{manager.name} manages {employee.name}")
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

8. **Use RelationsAPI for analysis** - Use the dedicated RelationsAPI for relationship analytics and discovery

9. **Optimize relationship queries** - Use appropriate filters and pagination when querying large numbers of relationships

10. **Combine APIs effectively** - Use RelationsAPI to discover connections, then RecordsAPI for detailed record data

## API Reference

### PaginationParams

The `PaginationParams` TypedDict defines the structure for pagination options when querying relationships:

```python
from typing import TypedDict

class PaginationParams(TypedDict, total=False):
    """TypedDict for pagination parameters in relationship queries.

    Defines the structure for pagination options when querying relationships,
    allowing for efficient retrieval of large result sets.
    """
    limit: int  # Maximum number of relationships to return in a single request
    skip: int   # Number of relationships to skip from the beginning of the result set
```

#### Parameters

- **limit** (`int`): Maximum number of relationships to return in a single request
  - Default: 100
  - Maximum: 1000
  - Used for controlling the size of result sets and implementing pagination

- **skip** (`int`): Number of relationships to skip from the beginning of the result set
  - Default: 0
  - Used for implementing pagination by skipping already retrieved items
  - Useful for getting subsequent pages of results

#### Usage Example

```python
# Define pagination parameters
pagination = PaginationParams(
    limit=50,   # Return at most 50 relationships
    skip=100    # Skip the first 100 relationships
)

# Use with the find method
relationships = await db.relationships.find(
    search_query={"where": {"isActive": True}},
    pagination=pagination
)
```

### Relationship Object

When you query relationships using the RelationsAPI, you receive `Relationship` objects with the following structure:

```python
# Example Relationship object attributes
relationship = relationships[0]

print(f"Source ID: {relationship.sourceId}")        # ID of the source record
print(f"Source Label: {relationship.sourceLabel}")  # Label of the source record
print(f"Target ID: {relationship.targetId}")        # ID of the target record
print(f"Target Label: {relationship.targetLabel}")  # Label of the target record
print(f"Type: {relationship.type}")                 # Relationship type (e.g., "MANAGES")
print(f"Direction: {relationship.direction}")       # Relationship direction
```

## Related Documentation

- [Relationships Concept](../concepts/relationships.md) - Learn more about how relationships work in RushDB
- [Transactions](../concepts/transactions.mdx) - Using transactions for relationship consistency
- [Record Creation](./records/create-records.md) - Creating records with relationships
- [Finding Records](./records/get-records.md) - Search techniques including relationship-based queries
