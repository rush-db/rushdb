---
sidebar_position: 2
---

# Labels

In RushDB, [labels](../concepts/labels.md) are used to categorize records and define their types. The Python SDK provides methods for managing labels, finding records by labels, and working with label hierarchies.

## Overview

Labels in RushDB serve several important purposes:
- Categorizing records into logical groups
- Defining the type or class of a record
- Enabling efficient filtering and searching
- Supporting hierarchical data modeling

## Prerequisites

Before working with labels, make sure you have initialized the RushDB client with your API token:

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_TOKEN", base_url="https://api.rushdb.com/api/v1")
```

## Creating Records with Labels

When creating records, you specify labels to categorize them:

```python
# Create a record with a single label
person = db.records.create(
    label="PERSON",
    data={
        "name": "John Doe",
        "age": 30
    }
)

# The record now has the label "PERSON"
print(person.label)  # Output: "PERSON"
```

## Working with Label Case

By default, labels are stored as provided. However, you can use the `capitalizeLabels` option to automatically capitalize labels:

```python
# Create a record with automatic label capitalization
product = db.records.create(
    label="product",  # Will be automatically capitalized to "PRODUCT"
    data={
        "name": "Smartphone",
        "price": 999.99
    },
    options={
        "capitalizeLabels": True
    }
)

print(product.label)  # Output: "PRODUCT"
```

## Finding Records by Label

You can search for records by their labels using the `find()` method with the `labels` parameter:

```python
# Find all records with the "PERSON" label
people = db.records.find({
    "labels": ["PERSON"]
})

# Find records with either "EMPLOYEE" or "CONTRACTOR" labels
workers = db.records.find({
    "labels": ["EMPLOYEE", "CONTRACTOR"]
})

# Combine label filtering with other search criteria
senior_engineers = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {
        "position": "Senior Engineer",
        "yearsOfExperience": {"$gte": 5}
    }
})
```

## Label Hierarchy and Inheritance

RushDB supports label inheritance, allowing you to model hierarchical relationships between labels. For example, an "EMPLOYEE" can also be a "PERSON":

```python
# Create a record with multiple labels
employee = db.records.create_many(
    label="EMPLOYEE",
    data={
        "name": "Jane Smith",
        "email": "jane@example.com",
        "department": "Engineering",
        "PERSON": {  # Nested object creates a relationship with the label PERSON
            "age": 28,
            "address": "123 Main St"
        }
    },
    options={
        "relationshipType": "IS_A"  # Establishes an inheritance relationship
    }
)

# Finding the employee will also include PERSON properties
found_employee = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {
        "name": "Jane Smith"
    }
})
```

## Discovering and Searching Labels with LabelsAPI

The `LabelsAPI` provides dedicated functionality for discovering and working with record labels in the database. This API allows you to find what types of records exist in your database and search for labels based on the properties of records that have those labels.

**Important**: The LabelsAPI uses a Record-centric approach. When searching for labels, you specify properties of the records that have those labels, not properties of the labels themselves. This means the `where` clause contains Record properties to find labels from records that match those criteria.

### Overview

The LabelsAPI enables you to:
- Discover all labels (record types) in the database
- Search for labels based on record properties
- Understand the data structure and schema of your database
- Monitor label usage and distribution
- Work with labels in transaction contexts

### Accessing the LabelsAPI

You access the LabelsAPI through the main RushDB client:

```python
from rushdb import RushDB

# Initialize the client
db = RushDB("RUSHDB_API_TOKEN", base_url="https://api.rushdb.com/api/v1")

# Access the labels API
labels_api = db.labels
```

### The find() Method

The `find()` method is the primary way to discover and search for labels in your database. It uses a Record-centric approach where you can filter labels based on the properties of records that have those labels.

#### Method Signature

```python
def find(
    self,
    search_query: Optional[SearchQuery] = None,
    transaction: Optional[Transaction] = None,
) -> List[str]
```

#### Parameters

- **search_query** (`Optional[SearchQuery]`): Search criteria to filter labels. Uses a Record-centric approach where the `where` clause contains Record properties to find labels from records that match those criteria:
  - `where`: Filter conditions for record properties (not label properties)
  - `labels`: Not typically used in LabelsAPI as you're discovering labels
  - `orderBy`: Not applicable for label discovery
- **transaction** (`Optional[Transaction]`): Optional transaction context for the operation

#### Return Value

Returns a `List[str]` containing label names (strings) that exist in the database. Each string represents a unique label/type used in the database.

### Basic Label Discovery

#### Find All Labels

```python
# Get all labels in the database
all_labels = db.labels.find()

print("Available record types:", all_labels)
# Output: ['USER', 'COMPANY', 'PROJECT', 'EMPLOYEE', 'DEPARTMENT']

# Check how many different types of records exist
print(f"Database contains {len(all_labels)} different record types")
```

#### Discover Labels Based on Record Properties

```python
# Find labels from records that are active
active_record_labels = db.labels.find({
    "where": {
        "isActive": True
    }
})

print("Labels from active records:", active_record_labels)
# Output: ['USER', 'PROJECT', 'EMPLOYEE']

# Find labels from records in Engineering department
engineering_labels = db.labels.find({
    "where": {
        "department": "Engineering"
    }
})

print("Labels used in Engineering:", engineering_labels)
# Output: ['EMPLOYEE', 'MANAGER', 'PROJECT']
```

### Advanced Label Searching

#### Filter by Record Creation Date

```python
# Find labels from recently created records
recent_labels = db.labels.find({
    "where": {
        "createdAt": {"$gte": "2024-01-01T00:00:00Z"}
    }
})

print("Labels from records created this year:", recent_labels)
```

#### Filter by Complex Record Properties

```python
# Find labels from high-value records
valuable_record_labels = db.labels.find({
    "where": {
        "$or": [
            {"revenue": {"$gte": 1000000}},  # High revenue companies
            {"salary": {"$gte": 150000}},    # High salary employees
            {"budget": {"$gte": 500000}}     # High budget projects
        ]
    }
})

print("Labels from high-value records:", valuable_record_labels)
```

#### Find Labels by Record Status

```python
# Find labels from records matching specific status
published_labels = db.labels.find({
    "where": {
        "status": {"$in": ["published", "active", "approved"]}
    }
})

# Find labels from records with specific properties
tech_labels = db.labels.find({
    "where": {
        "$and": [
            {"industry": "Technology"},
            {"employees": {"$gte": 50}},
            {"isPublic": True}
        ]
    }
})
```

### Label Analytics and Insights

#### Analyze Database Schema

```python
# Get comprehensive view of your database schema
all_labels = db.labels.find()

print("Database Schema Overview:")
print(f"Total record types: {len(all_labels)}")
for label in sorted(all_labels):
    print(f"  - {label}")

# Find labels by different criteria to understand data distribution
active_labels = db.labels.find({"where": {"isActive": True}})
inactive_labels = db.labels.find({"where": {"isActive": False}})

print(f"\nActive record types: {len(active_labels)}")
print(f"Inactive record types: {len(inactive_labels)}")
```

#### Monitor Label Usage Patterns

```python
# Discover which types of records exist in different departments
departments = ["Engineering", "Sales", "Marketing", "HR"]
label_distribution = {}

for dept in departments:
    dept_labels = db.labels.find({
        "where": {"department": dept}
    })
    label_distribution[dept] = dept_labels
    print(f"{dept} department uses labels: {dept_labels}")

# Find common labels across departments
common_labels = set(label_distribution["Engineering"])
for dept_labels in label_distribution.values():
    common_labels &= set(dept_labels)

print(f"Labels common across all departments: {list(common_labels)}")
```

### Using Labels API with Transactions

The LabelsAPI supports transactions for consistent discovery:

```python
# Start a transaction
tx = db.tx.begin()

try:
    # Discover labels within the transaction context
    labels = db.labels.find({
        "where": {
            "department": "Sales",
            "isActive": True
        }
    }, transaction=tx)

    print(f"Found labels in Sales: {labels}")

    # Perform additional operations in the same transaction
    for label in labels:
        # Query records of each discovered type
        records = db.records.find({
            "labels": [label],
            "where": {"department": "Sales"}
        }, transaction=tx)
        print(f"Found {len(records)} {label} records in Sales")

    # Commit the transaction
    tx.commit()
except Exception as e:
    # Roll back on error
    tx.rollback()
    print(f"Transaction failed: {e}")
```

### Practical Use Cases

#### Database Migration and Schema Discovery

```python
# Discover existing schema before migration
def analyze_database_schema():
    """Analyze the current database schema and structure."""

    # Get all labels
    all_labels = db.labels.find()

    schema_info = {}
    for label in all_labels:
        # Find sample records for each label to understand structure
        sample_records = db.records.find({
            "labels": [label]
        }, limit=5)

        # Analyze properties
        properties = set()
        for record in sample_records:
            properties.update(record.data.keys())

        schema_info[label] = {
            "sample_properties": list(properties),
            "sample_count": len(sample_records)
        }

    return schema_info

# Run schema analysis
schema = analyze_database_schema()
for label, info in schema.items():
    print(f"\n{label}:")
    print(f"  Sample properties: {info['sample_properties']}")
    print(f"  Sample records found: {info['sample_count']}")
```

#### Data Quality Assessment

```python
# Find labels from incomplete or problematic records
def assess_data_quality():
    """Assess data quality by finding labels from problematic records."""

    # Find labels from records missing critical fields
    incomplete_labels = db.labels.find({
        "where": {
            "$or": [
                {"name": None},
                {"createdAt": None},
                {"id": None}
            ]
        }
    })

    # Find labels from very old records that might need updating
    old_labels = db.labels.find({
        "where": {
            "updatedAt": {"$lt": "2023-01-01T00:00:00Z"}
        }
    })

    return {
        "incomplete_data_labels": incomplete_labels,
        "outdated_labels": old_labels
    }

# Run data quality assessment
quality_report = assess_data_quality()
print("Data Quality Report:")
print(f"Labels with incomplete data: {quality_report['incomplete_data_labels']}")
print(f"Labels with outdated records: {quality_report['outdated_labels']}")
```

### Performance Considerations

When using the LabelsAPI:

1. **Use specific filters**: Apply `where` conditions to reduce the scope of label discovery
2. **Cache results**: Label discovery results can be cached as they don't change frequently
3. **Combine with record queries**: Use LabelsAPI to discover types, then use RecordsAPI for detailed data
4. **Monitor database growth**: Regular label discovery helps track database schema evolution
5. **Use in schema validation**: Incorporate label discovery in data validation pipelines

### Error Handling

```python
try:
    labels = db.labels.find({
        "where": {"department": "NonexistentDepartment"}
    })
    print(f"Found labels: {labels}")  # Will return empty list if no matches
except Exception as e:
    print(f"Error discovering labels: {e}")
    # Handle the error appropriately
```

### Integration with Record Operations

The LabelsAPI works seamlessly with record operations for comprehensive data management:

```python
# 1. Discover available labels
available_labels = db.labels.find()
print(f"Available record types: {available_labels}")

# 2. Find labels from specific types of data
user_related_labels = db.labels.find({
    "where": {
        "$or": [
            {"email": {"$ne": None}},
            {"username": {"$ne": None}},
            {"role": {"$ne": None}}
        ]
    }
})

# 3. Query records for each discovered label
for label in user_related_labels:
    records = db.records.find({
        "labels": [label]
    })
    print(f"Found {len(records)} records with label '{label}'")

# 4. Create new records based on discovered patterns
if "USER" in available_labels:
    # Safe to create USER records
    new_user = db.records.create(
        label="USER",
        data={"name": "New User", "email": "new@example.com"}
    )
```

## API Reference

### LabelsAPI.find()

The `find()` method discovers and retrieves labels (record types) from the database based on record properties.

#### Method Signature

```python
def find(
    self,
    search_query: Optional[SearchQuery] = None,
    transaction: Optional[Transaction] = None,
) -> List[str]
```

#### Parameters

- **search_query** (`Optional[SearchQuery]`): Search criteria to filter labels using a Record-centric approach
  - **where** (dict): Filter conditions for record properties. The API finds labels from records that match these conditions
  - **labels**: Not typically used in LabelsAPI since you're discovering labels
  - **orderBy**: Not applicable for label discovery
- **transaction** (`Optional[Transaction]`): Transaction context for the operation

#### Return Value

- **List[str]**: List of unique label names (strings) found in the database

#### Examples

```python
# Get all labels
all_labels = db.labels.find()

# Get labels from active records
active_labels = db.labels.find({
    "where": {"isActive": True}
})

# Get labels from records in specific department
dept_labels = db.labels.find({
    "where": {"department": "Engineering"}
}, transaction=tx)
```

### SearchQuery Structure for Labels

When using the LabelsAPI, the SearchQuery follows this structure:

```python
from rushdb.models.search_query import SearchQuery

# Example SearchQuery for label discovery
query = SearchQuery(
    where={
        # Record properties to filter by
        "isActive": True,
        "department": "Engineering",
        "createdAt": {"$gte": "2024-01-01T00:00:00Z"}
    }
)

labels = db.labels.find(query)
```

## Best Practices for Working with Labels

1. **Use consistent naming conventions** - Consider using uppercase for labels (e.g., "PERSON" instead of "Person") for consistency with graph database conventions.

2. **Leverage the `capitalizeLabels` option** - Use this option to ensure consistent capitalization across your database.

3. **Use specific labels** - More specific labels make searching and filtering more efficient.

4. **Consider label hierarchies** - Use label inheritance to model "is-a" relationships between entities.

5. **Combine labels with where clauses** - For precise filtering, combine label filtering with property conditions in the where clause.

6. **Be mindful of performance** - Searching with very common labels might return large result sets. Use additional filters to narrow down results.

7. **Use LabelsAPI for schema discovery** - Regularly use the LabelsAPI to understand your database structure and monitor schema evolution.

8. **Cache label discovery results** - Since labels don't change frequently, consider caching the results of label discovery operations.

9. **Filter by record properties for targeted discovery** - Use the Record-centric approach to discover labels from specific subsets of your data.

10. **Integrate with data validation** - Use label discovery to validate that expected record types exist before performing operations.

11. **Monitor label distribution** - Use LabelsAPI to understand how different types of data are distributed across your database.

12. **Combine with record operations** - Use LabelsAPI to discover types, then use RecordsAPI for detailed record manipulation.

## Related Documentation

- [Labels Concept](../concepts/labels.md) - Learn more about how labels work in RushDB
- [Search by Labels](../concepts/search/labels.md) - Advanced techniques for searching by labels
- [Record Creation](./records/create-records.md) - Creating records with labels
- [Finding Records](./records/get-records.md) - Search techniques including label filtering
