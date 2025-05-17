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

db = RushDB("YOUR_API_TOKEN", base_url="https://api.rushdb.com")
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

## Best Practices for Working with Labels

1. **Use consistent naming conventions** - Consider using uppercase for labels (e.g., "PERSON" instead of "Person") for consistency with graph database conventions.

2. **Leverage the `capitalizeLabels` option** - Use this option to ensure consistent capitalization across your database.

3. **Use specific labels** - More specific labels make searching and filtering more efficient.

4. **Consider label hierarchies** - Use label inheritance to model "is-a" relationships between entities.

5. **Combine labels with where clauses** - For precise filtering, combine label filtering with property conditions in the where clause.

6. **Be mindful of performance** - Searching with very common labels might return large result sets. Use additional filters to narrow down results.

## Related Documentation

- [Labels Concept](../concepts/labels.md) - Learn more about how labels work in RushDB
- [Search by Labels](../concepts/search/labels.md) - Advanced techniques for searching by labels
- [Record Creation](./records/create-records.md) - Creating records with labels
- [Finding Records](./records/get-records.md) - Search techniques including label filtering
