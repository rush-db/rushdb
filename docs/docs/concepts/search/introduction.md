---
title: Introduction
sidebar_position: 0
---

# Search

RushDB provides a powerful and flexible search system that allows you to efficiently query and traverse your graph data. The Search API is a cornerstone of RushDB, enabling you to find records, filter by conditions, navigate relationships, aggregate results, and format the returned data exactly as needed.

## Core Capabilities

RushDB's Search API offers a comprehensive set of features:

- **Powerful Filtering**: Use the [`where` clause](./where.md) with a wide range of operators to precisely filter records
- **Graph Traversal**: Navigate through connected records with relationship queries
- **Aggregation**: Perform calculations and transform data using [aggregation functions](./aggregations.md)
- **Pagination and Sorting**: Control result volume and order with [pagination and sorting options](./pagination-order.md)
- **Label-Based Filtering**: Target specific types of records using [label filtering](./labels.md)
- **Vector Search**: Find records based on vector similarity for AI and machine learning applications

## SearchQuery Structure

All search operations in RushDB use a consistent SearchQuery data structure:

```typescript
interface SearchQuery {
  labels?: string[];           // Filter by record labels
  where?: WhereCondition;      // Filter by property values and relationships
  limit?: number;              // Maximum number of records to return (default: 100)
  skip?: number;               // Number of records to skip (for pagination)
  orderBy?: OrderByClause;     // Sorting configuration
  aggregate?: AggregateClause; // Data aggregation and transformation
}
```

## Basic Example

Here's a simple example of searching for products:

```typescript
db.records.find("PRODUCT", {
    where: {
        title: { $contains: "Sneakers" },
        SIZE: {
            uk: { $gte: 8, $lte: 9 },
            qty: { $gt: 0 }
        }
    },
    limit: 20,
    orderBy: { price: 'asc' }
})
```

This query:
1. Searches for records with the `PRODUCT` label
2. Filters for products with "Sneakers" in the title
3. Finds only those with UK sizes 8-9 that are in stock
4. Limits results to 20 records
5. Sorts results by price in ascending order

## Advanced Search Features

### Filtering with Where Clauses

The [`where` clause](./where.md) is the primary mechanism for filtering records. It supports:

- **Property Matching**: Filter by exact values, string patterns, numeric ranges, etc.
- **Logical Operators**: Combine conditions with AND, OR, NOT, etc.
- **Relationship Traversal**: Filter based on properties of related records

```typescript
{
  where: {
    category: "Electronics",
    price: { $gte: 100, $lte: 500 },
    $or: [
      { inStock: true },
      { preorderAvailable: true }
    ]
  }
}
```

### Aggregating Results

[Aggregations](./aggregations.md) allow you to perform calculations and transform the structure of your results:

```typescript
{
  labels: ["ORDER"],
  where: {
    PRODUCT: {
      $alias: "$product",
      category: "Electronics"
    }
  },
  aggregate: {
    totalSpent: {
      fn: "sum",
      field: "amount",
      alias: "$record"
    },
    products: {
      fn: "collect",
      alias: "$product"
    }
  }
}
```

### Pagination and Sorting

Control the volume and order of results using [pagination and sorting options](./pagination-order.md):

```typescript
{
  labels: ["CUSTOMER"],
  limit: 50,         // Return up to 50 records
  skip: 100,         // Skip the first 100 records
  orderBy: {
    lastName: "asc", // Sort by lastName ascending
    firstName: "asc" // Then by firstName ascending
  }
}
```

## Performance Best Practices

For optimal performance when using the Search API:

1. **Be Specific**: Filter by labels when possible to narrow the search scope
2. **Use Indexed Properties**: Prioritize filtering on properties that have indexes
3. **Limit Results**: Use pagination to retrieve only the records you need
4. **Optimize Queries**: Avoid deep relationship traversals when possible
5. **Use Aliases Efficiently**: Define aliases only for records you need to reference in aggregations

## Next Steps

- Learn more about [filtering with where clauses](./where.md)
- Explore [data aggregation capabilities](./aggregations.md)
- Understand [pagination and sorting options](./pagination-order.md)
- Discover how to filter by [record labels](./labels.md)