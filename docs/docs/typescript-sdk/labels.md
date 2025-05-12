---
sidebar_position: 2
---

# Labels

The RushDB TypeScript SDK provides a simple interface for working with [labels](../concepts/labels.md) in your database. Labels in RushDB help categorize and organize [records](../concepts/records.md), functioning similarly to table names in relational databases but with the flexibility of graph databases.

## Labels Overview

Labels in RushDB:
- Provide a way to categorize and organize records
- Enable efficient querying across similar types of records
- Each record has exactly one user-defined label (e.g., `User`, `Product`, `Car`)
- Are case-sensitive (e.g., "User" and "user" are treated as different labels)
- Function similarly to table names in relational databases but with graph database flexibility

## Labels API

The SDK provides label-related methods through the `labels` object:

```typescript
// Access the labels API
const labels = db.labels;
```

The Labels API is built on the powerful [SearchQuery](/concepts/search/introduction.md) interface, which enables you to use the same querying capabilities that are available throughout the RushDB search API. This means you can leverage complex filters, logical operators, and comparison operators when working with labels.

### Find Labels

Searches for labels based on the provided query parameters and returns label names with their record counts:

```typescript
const response = await db.labels.find({
  // Optional: Any search parameters to filter labels
  // Similar to record search queries
  where: {
    // You can filter by record properties that have specific labels
    name:  "John"
  },
  // Other search parameters like skip, limit, etc.
});

// Response contains labels with their counts
console.log(response.data);
/* Example output:
{
  "User": 125
}
*/
```

## Using Labels with Records

When creating or updating records, you need to specify a label:

```typescript
// Create a record with the "User" label
const user = await db.records.create({
  label: "User",
  data: {
    name: "John Doe",
    email: "john.doe@example.com"
  }
});

// Find all records with the "User" label
const users = await db.records.find({
  labels: ["User"]
});
```

## Filtering Labels

The labels API leverages the powerful [`SearchQuery`](/concepts/search/introduction.md) interface, allowing you to use the same advanced querying capabilities that are available throughout the RushDB search API. You can use complex queries to filter which labeled records to include:

### Example with Multiple Conditions

```typescript
const response = await db.labels.find({
  where: {
    age: { $gt: 30 },
    active: true
  }
});
```

This will return labels for records where `age` is greater than 30 AND `active` is true.

### Example with OR Logic

```typescript
const response = await db.labels.find({
  where: {
    $or: [
      { country: "USA" },
      { country: "Canada" }
    ]
  }
});
```

This will return labels for records where `country` is either "USA" OR "Canada".

### Advanced Query Operators

Since the Labels API uses the [`SearchQuery`](/concepts/search/introduction.md) interface, you can use all the query operators available in the [RushDB search API](/concepts/search/introduction.md):

```typescript
const response = await db.labels.find({
  where: {
    // String operators
    name: { $contains: "Smith" },
    email: { $endsWith: "@example.com" },

    // Numeric operators
    age: { $gt: 18, $lt: 65 },
    score: { $gte: 4.5 },

    // Array operators
    tags: { $in: ["premium", "verified"] },

    // Negation
    status: { $ne: "inactive" }
  }
});
```

## Label Requirements and Limitations

- **Single Custom Label**: Each record can have only one custom label at a time
- **Required Field**: A custom label is required for each record
- **Case-Sensitive**: Labels are case-sensitive ("User" ≠ "user")

## Working with Labels

### Best Practices

1. **Consistent naming conventions**: Use a consistent pattern for [label](../concepts/labels.md) names (e.g., singular nouns, PascalCase)
2. **Meaningful labels**: Choose labels that describe what the record represents, not just its attributes
3. **Hierarchical labeling**: Consider using more specific labels for specialized record types (e.g., "Employee" and "Manager" instead of just "Person")

### Common Use Cases

- **Data organization**: Group related records for easier querying and visualization
- **Access control**: Set permissions based on record labels
- **Conditional processing**: Apply different business logic depending on record types
- **Schema validation**: Enforce data structure based on record labels

## Internal Representation

Internally, labels are stored as the `__RUSHDB__KEY__LABEL__` property and exposed to clients as `__label`. This property is essential for organizing records and enabling efficient queries across similar types of data.

## Additional Resources

- [Labels Concept Documentation](../concepts/labels.md) - Learn more about labels and their role in the RushDB data model
- [Search API Documentation](/concepts/search/introduction.md) - Explore the powerful search capabilities available in RushDB

