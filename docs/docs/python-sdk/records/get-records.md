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
print(f"Found {len(result)} records out of {result.total} total users")

# Iterate over results
for user in result:
    print(f"User: {user.get('name', 'Unknown')}")

# Access specific records
first_user = result[0] if result else None
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
result = db.records.find({
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

posts = result.data
total = result.total
```

For more complex relationship queries, you can specify relationship types and directions:

```python
# Find users who follow specific topics
result = db.records.find({
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

users = result.data
total = result.total
```

See the [Where clause documentation](../../concepts/search/where#relationship-queries) for more details on relationship queries.

### Vector Search

RushDB supports vector similarity searches for AI and machine learning applications:

```python
# Find documents similar to a query embedding
result = db.records.find({
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

documents = result.data
total = result.total
```

See the [Vector operators documentation](../../concepts/search/where#vector-operators) for more details on vector search capabilities.

### Pagination and Sorting

Control the order and volume of results:

```python
# Get the second page of results (20 items per page)
result = db.records.find({
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

products = result.data
total_products = result.total
```

For more details on pagination and sorting options, see the [Pagination and ordering documentation](../../concepts/search/pagination-order).

### Aggregations

Transform and aggregate your search results:

```python
# Calculate sales statistics
result = db.records.find({
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

stats = result.data
total = result.total
```

For comprehensive details on available aggregation functions and usage, see the [Aggregations documentation](../../concepts/search/aggregations).

### Searching Within a Record's Context

You can search for records within the context of a specific record's relationships using the `record_id` parameter:

```python
# Find all records related to a specific user
result = db.records.find(
    search_query={
        "labels": ["POST", "COMMENT"],
        "where": {
            "isPublished": True
        }
    },
    record_id="user_123"  # Search within this user's context
)

related_records = result.data
total = result.total

# Find only posts created by a specific user
result = db.records.find(
    search_query={
        "labels": ["POST"],
        "orderBy": {"createdAt": "desc"}
    },
    record_id="user_123"
)

user_posts = result.data

# Search for documents shared with a specific team
result = db.records.find(
    search_query={
        "labels": ["DOCUMENT"],
        "where": {
            "status": "shared",
            "category": {"$in": ["proposal", "contract"]}
        }
    },
    record_id="team_456"
)

team_documents = result.data
```

This is particularly useful when you want to:
- Find all records that have relationships with a specific record
- Search within the scope of a particular entity's connected data
- Implement features like "user's posts", "team's documents", or "company's projects"

## Return Format and Error Handling

The `find()` method returns a [`SearchResult`](../python-reference/search-result.md) object that provides list-like access and comprehensive metadata:

```python
# The method returns a SearchResult object
result = db.records.find({
    "labels": ["USER"],
    "limit": 10
})

# len(result) = records in this result set (affected by limit)
print(f"Retrieved {len(result)} records in this page")

# total = all records matching criteria in the entire database
print(f"Total matching records in database: {result.total}")

# Example: if you have 1,000 users total but limit to 10:
# len(result) = 10 (records returned in this request)
# result.total = 1000 (total users matching your criteria)

# Iterate over results
for record in result:
    print(f"User: {record.get('name')}")

# Access records by index
first_user = result[0] if result else None

# Check if there are more records beyond this page
if result.has_more:
    print("There are more records available")

# Handle cases where no records are found
if not result:
    print("No records found matching the criteria")

# Use total count for pagination calculations
pages = (result.total + 9) // 10  # Calculate number of pages (10 per page)
```

### Understanding Total vs Length

It's important to understand the difference between these two key concepts:

- **`result.total`** - The total number of records in your database that match your search criteria
- **`len(result)`** - The number of records actually returned in this specific request (limited by `limit` parameter)

```python
# Example: searching users in a database with 10,000 total users
result = db.records.find({
    "labels": ["USER"],
    "where": {"active": True},  # Let's say 8,500 users are active
    "limit": 25                 # But we only want 25 per page
})

print(f"Records in this page: {len(result)}")      # Will show: 25
print(f"Total active users: {result.total}")        # Will show: 8,500
print(f"Has more pages: {result.has_more}")         # Will show: True

# This is useful for building pagination UIs:
current_page = 1
per_page = 25
total_pages = (result.total + per_page - 1) // per_page  # = 340 pages
print(f"Page {current_page} of {total_pages}")
```

### Error Handling

The `find()` method includes built-in error handling that returns an empty SearchResult on exceptions:

```python
# If an error occurs, the method returns an empty SearchResult instead of raising an exception
result = db.records.find({
    "labels": ["INVALID_LABEL"],
    "where": {
        "nonexistent_field": "some_value"
    }
})

# Always returns a SearchResult, even on errors
print(f"Found {len(result)} records")  # Will print "Found 0 records"
print(f"Total: {result.total}")        # Will print "Total: 0"

# Safe iteration
for record in result:
    print("This won't execute if result is empty")

# Boolean check is safe
if result:
    print("This won't execute if result is empty")

# For more explicit error handling in production code, you may want to validate
# your queries before calling find() or implement additional error checking
```

## Search Within Transactions

All search operations can be performed within transactions for consistency:

```python
# Begin a transaction
tx = db.tx.begin()

try:
    # Perform search within the transaction
    result = db.records.find({
        "labels": ["USER"],
        "where": {"is_active": True}
    }, transaction=tx)

    # Use the results to make changes
    for user in result:
        if user.last_login < older_than_3_months:
            db.records.update({
                "target": user,
                "data": {"is_active": False}
            }, transaction=tx)

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
- Learn about the [`SearchResult](../python-reference/search-result.md) class returned by find operations
- See how to use [Records API](../../python-sdk/records/create-records.md) for other operations
