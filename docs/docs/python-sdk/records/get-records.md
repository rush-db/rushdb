---
sidebar_position: 4
---

# Get Records

The Search API is one of the most powerful features of RushDB, allowing you to find records, navigate relationships, and transform results to exactly match your application's needs. This guide demonstrates how to effectively use the Python SDK to search and query data in your RushDB database.

## Direct Record Search

The RushDB Python SDK provides several ways to search for records, from simple lookups to complex queries with filtering, sorting, and pagination.

### Basic Searching with `find()`

The most versatile search method is `find()`, which accepts a search query dictionary to filter, sort, and paginate results.

```python
# Basic search for records with the "USER" label
result = db.records.find({
    "labels": ["USER"],
    "where": {
        "isActive": True
    },
    "limit": 10,
    "orderBy": {"createdAt": "desc"}
})

# Access the returned records
users = result.data
print(f"Found {result.total} total users")
```

Search queries support a powerful and flexible syntax for filtering records. For a detailed explanation of all the available operators and capabilities, see the [Where clause documentation](../../concepts/search/where).

### Finding Records by ID with `find_by_id()`

When you already know the ID of the record(s) you need:

```python
# Find a single record by ID
user = db.records.find_by_id("user-123")

# Find multiple records by ID
users = db.records.find_by_id(["user-123", "user-456", "user-789"])
```

### Relationship Traversal

One of RushDB's most powerful features is the ability to search across relationships between records:

```python
# Find all blog posts by users who work at tech companies
tech_bloggers = db.records.find({
    "labels": ["POST"],
    "where": {
        "USER": {                            # Traverse to related USER records
            "COMPANY": {                     # Traverse to related COMPANY records
                "industry": "Technology"
            }
        },
        "publishedAt": {"$lte": datetime.now()}  # Only published posts
    },
    "orderBy": {"publishedAt": "desc"},
    "limit": 20
})
```

For more complex relationship queries, you can specify relationship types and directions:

```python
# Find users who follow specific topics
users = db.records.find({
    "labels": ["USER"],
    "where": {
        "TOPIC": {
            "$relation": {
                "type": "FOLLOWS",
                "direction": "out"       # User -> FOLLOWS -> Topic
            },
            "name": {"$in": ["Python", "GraphDB", "RushDB"]}
        }
    }
})
```

See the [Where clause documentation](../../concepts/search/where#relationship-queries) for more details on relationship queries.

### Vector Search

RushDB supports vector similarity searches for AI and machine learning applications:

```python
# Find documents similar to a query embedding
similar_documents = db.records.find({
    "labels": ["DOCUMENT"],
    "where": {
        "embedding": {
            "$vector": {
                "fn": "gds.similarity.cosine",   # Similarity function
                "query": query_embedding,        # Your vector embedding
                "threshold": {"$gte": 0.75}      # Minimum similarity threshold
            }
        }
    },
    "limit": 10
})
```

See the [Vector operators documentation](../../concepts/search/where#vector-operators) for more details on vector search capabilities.

### Pagination and Sorting

Control the order and volume of results:

```python
# Get the second page of results (20 items per page)
page2 = db.records.find({
    "labels": ["PRODUCT"],
    "where": {
        "category": "Electronics"
    },
    "skip": 20,      # Skip the first 20 results
    "limit": 20,     # Return 20 results
    "orderBy": {
        "price": "asc"  # Sort by price ascending
    }
})

# Get total number of results for pagination UI
total_products = page2.total
```

For more details on pagination and sorting options, see the [Pagination and ordering documentation](../../concepts/search/pagination-order).

### Aggregations

Transform and aggregate your search results:

```python
# Calculate sales statistics
sales_by_category = db.records.find({
    "labels": ["ORDER"],
    "where": {
        "status": "completed",
        "createdAt": {"$gte": "2023-01-01T00:00:00Z"}
    },
    "aggregate": {
        "totalSales": {
            "fn": "sum",
            "alias": "$record",
            "field": "amount"
        },
        "orderCount": {
            "fn": "count",
            "alias": "$record"
        },
        "avgOrderValue": {
            "fn": "avg",
            "alias": "$record",
            "field": "amount"
        }
    }
})
```

For comprehensive details on available aggregation functions and usage, see the [Aggregations documentation](../../concepts/search/aggregations).

## Search Within Transactions

All search operations can be performed within transactions for consistency:

```python
# Begin a transaction
tx = db.tx.begin()

try:
    # Perform search within the transaction
    users = db.records.find({
        "labels": ["USER"],
        "where": {"is_active": True}
    }, tx)

    # Use the results to make changes
    for user in users.data:
        if user.last_login < older_than_3_months:
            db.records.update({
                "target": user,
                "data": {"is_active": False}
            }, tx)

    # Commit the transaction when done
    tx.commit()
except Exception as error:
    # Roll back the transaction on error
    tx.rollback()
    raise error
```

For more details on transactions, see the [Transactions documentation](../../python-sdk/transactions).

## Performance Best Practices

When working with the Search API, follow these best practices for optimal performance:

1. **Be Specific with Labels**: Always specify labels to narrow the search scope.
2. **Use Indexed Properties**: Prioritize filtering on properties that have indexes.
3. **Limit Results**: Use pagination to retrieve only the records you need.
4. **Optimize Relationship Traversal**: Avoid deep relationship traversals when possible.
5. **Use Aliases Efficiently**: Define aliases only for records you need to reference in aggregations.
6. **Filter Early**: Apply filters as early as possible in relationship traversals to reduce the amount of data processed.

## Next Steps

- Explore [filtering with where clauses](../../concepts/search/where) in depth
- Learn about [data aggregation capabilities](../../concepts/search/aggregations)
- Understand [pagination and sorting options](../../concepts/search/pagination-order)
- Discover how to filter by [record labels](../../concepts/search/labels)
- See how to use [Records API](../../python-sdk/records/create-records.md) for other operations
