---
sidebar_position: 2
---

# Labels

The `labels` property in SearchQuery allows you to filter records by their label types. Labels in RushDB are categories or classifications assigned to records that help organize and identify different types of data.

#### Labels Placement in SearchQuery DTO

The `labels` array is defined at the top level of the SearchQuery DTO, alongside other query parameters:

```typescript
// SearchQuery
{
  labels: ['COMPANY'],       // Record labels to search (this is what we focus on)
  where: { /* conditions */ },  // Filtering conditions
  limit: 10,                 // Results limit
  skip: 0,                   // Results offset
  orderBy: { name: 'asc' },  // Sorting
  aggregate: { /* aggregations */ } // Aggregation definitions
}
```

## Basic Usage

### Filtering by a Single Label

The simplest way to use labels is to specify a single label to filter the records:

```typescript
{
  labels: ['PERSON']  // Only search for records with the PERSON label
}
```

This query will only return records that have the PERSON label.

### Implicit Filtering

When no labels are provided in the query, RushDB will search across all record labels:

```typescript
{
  // No 'labels' property - will search across all record types
  where: {
    name: "John"
  }
}
```

## Label Behavior

There are a few important things to understand about how labels work in SearchQuery:

1. **Case sensitivity**: Labels are case-sensitive. 'PERSON' and 'Person' are treated as different labels.

2. **Single label optimization**: When you specify exactly one label in the array, RushDB can optimize the query execution significantly. This is because it can use label-specific indexes in the graph database.

3. **Multiple labels behavior**: When you specify multiple labels, the search will return records that match ANY of the provided labels (OR condition).

## Examples

### Single Label Search

```typescript
{
  labels: ['COMPANY'],
  where: {
    founded: { $gte: 2010 }
  }
}
```

This query searches for COMPANY records founded in or after 2010. This is the most efficient way to query when you know exactly what type of record you're looking for.

### Multiple Labels Search

```typescript
{
  labels: ['PERSON', 'EMPLOYEE'],
  where: {
    age: { $gte: 18 }
  }
}
```

This query searches for records that have either the PERSON label OR the EMPLOYEE label, and have an age greater than or equal to 18.

### Combining Labels with Relationship Queries

You can combine label filtering with relationship traversal in the where clause:

```typescript
{
  labels: ['COMPANY'],
  where: {
    founded: { $gte: 2010 },
    EMPLOYEE: {
      position: "Software Engineer",
      SKILL: {
        name: "TypeScript"
      }
    }
  }
}
```

This query finds companies founded in or after 2010 that have employees with the position "Software Engineer" who possess the "TypeScript" skill.

### Using Labels with Aggregations

Labels are particularly useful when you want to perform aggregations on specific types of records:

```typescript
{
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee'
    }
  },
  aggregate: {
    employeeCount: {
      fn: 'count',
      alias: '$employee'
    }
  }
}
```

This query counts the number of employees for each company.

## Performance Considerations

1. **Always specify a label when possible**: Queries with a single label are significantly faster than queries without label constraints. This is especially important for large databases.

2. **Be specific**: The more specific you can be with your label, the better the performance of your query will be.

3. **Label-based indexes**: RushDB uses label-based indexes to quickly locate records of a specific type. Without labels, the system must scan a much larger set of records.

## Additional Notes

- **System labels**: Some labels in RushDB are prefixed with `__RUSHDB__` and are used for internal purposes. These are not typically used in user queries.

- **Default behavior**: If you don't specify any labels, the search will include all non-system labeled records.

- **Labels vs properties**: While you can also filter records based on their properties, using labels is generally more efficient when you know the type of record you're looking for.

- **Label inheritance**: RushDB does not have a concept of label hierarchy or inheritance. Each record can have its own set of independent labels.

## Complete Examples

<details>
<summary>Using labels with complex where conditions</summary>

```typescript
{
  labels: ['PRODUCT'],
  where: {
    price: { $gte: 100, $lte: 500 },
    inStock: true,
    CATEGORY: {
      name: { $in: ["Electronics", "Computers"] }
    },
    REVIEW: {
      $alias: '$review',
      rating: { $gte: 4 }
    }
  },
  aggregate: {
    avgRating: {
      fn: 'avg',
      field: 'rating',
      alias: '$review',
      precision: 1
    }
  },
  orderBy: { price: 'desc' },
  limit: 20
}
```

This example searches for PRODUCT records with a price between 100 and 500 that are in stock, belong to either the "Electronics" or "Computers" category, and have reviews with ratings of at least 4. It also calculates the average rating for each product, sorts the results by price in descending order, and limits the results to 20 records.
</details>

<details>
<summary>Using multiple labels to search across record types</summary>

```typescript
{
  labels: ['ARTICLE', 'BLOG_POST', 'NEWS'],
  where: {
    published: true,
    $or: [
      { title: { $contains: "AI" } },
      { content: { $contains: "artificial intelligence" } }
    ],
    AUTHOR: {
      $alias: '$author',
      reputation: { $gte: 100 }
    }
  },
  orderBy: { publishedAt: 'desc' },
  limit: 50,
  aggregate: {
    authorName: '$author.name'
  }
}
```

This example searches across three types of content records (ARTICLE, BLOG_POST, and NEWS) that are published and contain either "AI" in the title or "artificial intelligence" in the content, written by authors with a reputation of at least 100. It retrieves the author's name for each record, sorts the results by publication date in descending order, and limits the results to 50 records.
</details>