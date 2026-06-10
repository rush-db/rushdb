---
sidebar_position: 3
---

# SearchResult

The `SearchResult` class is a container for search results that follows Python SDK best practices. It provides both list-like access and iteration support, along with metadata about the search operation including total count and pagination information.

This class is designed to be familiar to Python developers, following patterns used by popular libraries like boto3 (AWS SDK), google-cloud libraries, and requests libraries.

## Class Definition

```python
from typing import Generic, Iterator, List, Optional, TypeVar
from .record import Record
from .search_query import SearchQuery

T = TypeVar("T")

class SearchResult(Generic[T]):
    """Container for search results following Python SDK best practices."""

    def __init__(
        self,
        data: List[T],
        total: Optional[int] = None,
        search_query: Optional[SearchQuery] = None,
        client: Optional["RushDB"] = None,
    )
```

## Type Aliases

```python
# Type alias for record search results
RecordSearchResult = SearchResult[Record]
```

## Properties

The `SearchResult` class provides the following properties:

- **`data`** - List of result items
- **`total`** - Total number of matching records in the database
- **`search_query`** - The search query used to generate this result
- **`has_more`** - Whether there are more records available beyond this result set
- **`skip`** - Number of records that were skipped in the search query
- **`limit`** - Limit that was applied to the search query

### data

Gets the list of result items.

**Type:** `List[T]`

**Example:**

```python
result = client.records.find({"labels": ["USER"]})
records = result.data
print(f"Retrieved {len(records)} records")
```

### total

Gets the total number of records in the database that match your search criteria.

**Type:** `int`

**Important:** This represents the total count of all records matching your search criteria in the entire database, not the number of records in the current page/result set. When pagination is used (`limit` and `skip`), this number will typically be larger than the number of records actually returned in `data`.

**Example:**

```python
# Search for users with a limit of 10 records per page
result = client.records.find({"labels": ["USER"], "limit": 10})

# total = all users in database matching the criteria (e.g., 1,847)
# len(result) = records in this result set (e.g., 10)
print(f"Showing {len(result)} out of {result.total} total matching users")
# Output: "Showing 10 out of 1,847 total matching users"
```

### search_query

Gets the search query used to generate this result.

**Type:** `SearchQuery`

**Example:**

```python
query = {"labels": ["USER"], "where": {"active": True}}
result = client.records.find(query)
original_query = result.search_query
```

### has_more

Checks if there are more records available beyond this result set.

**Type:** `bool`

**Example:**

```python
result = client.records.find({"labels": ["USER"], "limit": 10})
if result.has_more:
    print("There are more records available")
    # Fetch next page
    next_result = client.records.find({
        "labels": ["USER"],
        "limit": 10,
        "skip": result.skip + len(result)
    })
```

### skip

Gets the number of records that were skipped in the search query.

**Type:** `int`

**Example:**

```python
result = client.records.find({
    "labels": ["USER"],
    "limit": 10,
    "skip": 20
})
print(f"Skipped {result.skip} records")  # Will print: "Skipped 20 records"
```

### limit

Gets the limit that was applied to the search query.

**Type:** `Optional[int]`

**Note:** If no limit was specified in the original query, this returns `len(result.data)`.

**Example:**

```python
result = client.records.find({"labels": ["USER"], "limit": 25})
print(f"Limit applied: {result.limit}")  # Will print: "Limit applied: 25"

# If no limit was specified
result = client.records.find({"labels": ["USER"]})
print(f"Effective limit: {result.limit}")  # Will print the actual number of records returned
```

## Methods

### Length

`__len__() -> int`

Returns the number of records in this result set.

**Example:**

```python
result = client.records.find({"labels": ["USER"]})
record_count = len(result)
print(f"Found {record_count} records")
```

### Iteration

`__iter__() -> Iterator[T]`

Allows iteration over the result items.

**Example:**

```python
result = client.records.find({"labels": ["USER"]})
for record in result:
    print(f"User: {record.get('name', 'Unknown')}")
```

### Get Item

`__getitem__(index) -> T`

Gets an item by index or slice.

**Parameters:**

- `index`: Integer index or slice object

**Example:**

```python
result = client.records.find({"labels": ["USER"]})

# Get first record
first_user = result[0] if result else None

# Get last record
last_user = result[-1] if result else None

# Get slice of records
first_five = result[0:5]
```

### Boolean Check

`__bool__() -> bool`

Checks if the result set contains any items.

**Example:**

```python
result = client.records.find({"labels": ["USER"], "where": {"active": False}})
if result:
    print(f"Found {len(result)} inactive users")
else:
    print("No inactive users found")
```

### Convert to Dict

`to_dict() -> dict`

Returns the result in a standardized dictionary format.

**Returns:** Dictionary with keys: `total`, `data`, `search_query`

**Example:**

```python
result = client.records.find({"labels": ["USER"]})
result_dict = result.to_dict()
print(result_dict["total"])  # Total count
print(len(result_dict["data"]))  # Records in this result set
```

### Page Info

`get_page_info() -> dict`

Gets pagination information about the current result set.

**Returns:** Dictionary with pagination metadata

**Example:**

```python
result = client.records.find({
    "labels": ["USER"],
    "limit": 10,
    "skip": 20
})

page_info = result.get_page_info()
print(f"Total records: {page_info['total']}")      # Total matching records in database
print(f"Records loaded: {page_info['loaded']}")    # Records in this result set (len(result))
print(f"Has more: {page_info['has_more']}")        # Whether there are more records available
print(f"Current skip: {page_info['skip']}")        # Number of records skipped
print(f"Current limit: {page_info['limit']}")      # Limit applied to the query
```

### delete_all()

`delete_all(transaction=None) -> dict`

Deletes all records in this result set in a single batched call.

**Parameters:**

- `transaction`: Optional transaction context

**Returns:** Server response dict with operation status

**Example:**

```python
result = client.records.find({"labels": ["TEMP_DATA"]})
result.delete_all()
```

### next()

`next(preserve_data=False) -> SearchResult`

Fetches the next page of results by incrementing `skip` by `limit`.

**Parameters:**

- `preserve_data` (bool): If `True`, appends new records to this instance and returns `self`. Otherwise returns a new `SearchResult`.

**Raises:** `RuntimeError` if no `search_query` was provided at construction.

**Example:**

```python
result = client.records.find({"labels": ["USER"], "limit": 10})

# Get next page as new result
page2 = result.next()

# Or accumulate pages in-place
result.next(preserve_data=True)  # result.data now has 20 records
result.next(preserve_data=True)  # result.data now has 30 records
```

### export_csv()

`export_csv() -> str`

Serializes all records in this result set to a CSV string. System fields (`__id`, `__label`, `__proptypes`) are excluded from the output.

**Returns:** CSV string with a header row followed by one row per record. Empty string if the result set is empty.

**Example:**

```python
result = client.records.find({"labels": ["PRODUCT"]})
csv_data = result.export_csv()
with open("products.csv", "w") as f:
    f.write(csv_data)
```

### set_properties()

`set_properties(patch, transaction=None) -> None`

Updates one or more fields across every record in this result set. Records are updated in batches of 100.

**Parameters:**

- `patch` (dict): Fields to update and their new values
- `transaction`: Optional transaction context

**Example:**

```python
result = client.records.find({"labels": ["USER"], "where": {"status": "pending"}})
result.set_properties({"status": "active"})
```

### to_dataframe()

`to_dataframe(exclude_internal=True) -> pandas.DataFrame`

Converts this result set to a `pandas.DataFrame`. Requires `pandas` to be installed.

**Parameters:**

- `exclude_internal` (bool): If `True` (default), columns starting with `__` are excluded.

**Raises:** `ImportError` if pandas is not installed.

**Example:**

```python
result = client.records.find({"labels": ["USER"]})
df = result.to_dataframe()
print(df.head())

# Include internal fields (__id, __label, etc.)
df_full = result.to_dataframe(exclude_internal=False)
```

## Usage Examples

### Basic Iteration

```python
# Find all active users
result = client.records.find({
    "labels": ["USER"],
    "where": {"active": True}
})

# Iterate over results
for user in result:
    print(f"User: {user.get('name')} ({user.get('email')})")

# Check if results exist
if result:
    print(f"Found {len(result)} active users")
else:
    print("No active users found")
```

### Pagination Handling

```python
def get_all_users():
    """Example of handling pagination to get all users."""
    all_users = []
    skip = 0
    limit = 100

    while True:
        result = client.records.find({
            "labels": ["USER"],
            "limit": limit,
            "skip": skip
        })

        # Add records to our collection
        all_users.extend(result.data)

        # Check if we have more records
        if not result.has_more:
            break

        skip += limit

    return all_users
```

### List-like Operations

```python
result = client.records.find({"labels": ["USER"]})

# Access specific records
first_user = result[0] if result else None
last_user = result[-1] if result else None

# Get a subset
top_five = result[:5]

# Check length
user_count = len(result)

# Convert to regular list if needed
user_list = list(result)
```

### Working with Metadata

```python
result = client.records.find({
    "labels": ["USER"],
    "where": {"department": "Engineering"},
    "limit": 20,
    "skip": 40
})

print(f"Page size: {len(result)}")                    # Records in current page
print(f"Total engineers: {result.total}")             # Total matching records
print(f"Records skipped: {result.skip}")              # Records skipped (40)
print(f"Limit applied: {result.limit}")               # Limit for this query (20)
print(f"Showing results for query: {result.search_query}")

if result.has_more:
    remaining = result.total - (result.skip + len(result))
    print(f"{remaining} more engineers available")

    # Get next page
    next_result = client.records.find({
        "labels": ["USER"],
        "where": {"department": "Engineering"},
        "limit": 20,
        "skip": result.skip + len(result)  # Skip to next page
    })
```

### Error Handling

```python
# SearchResult is designed to be safe - it won't raise exceptions for empty results
result = client.records.find({"labels": ["NONEXISTENT"]})

# These operations are safe even if no results found
print(f"Found {len(result)} records")  # Will print 0
print(f"Total: {result.total}")  # Will print 0

# Iteration is safe
for record in result:
    print("This won't execute if result is empty")

# Boolean check is safe
if result:
    print("This won't execute if result is empty")
```

## Integration with Records API

The `SearchResult` class is returned by the `RecordsAPI.find()` method:

```python
from rushdb.models.search_query import SearchQuery

# Using SearchQuery class
query = SearchQuery(
    labels=["USER"],
    where={"active": True},
    limit=10,
    order_by={"created_at": "desc"}
)

result = client.records.find(query)

# Working with the result
for user in result:
    print(f"User: {user.get('name')}")

print(f"Loaded {len(result)} out of {result.total} total users")
```

## Best Practices

1. **Check for Results**: Always check if results exist before accessing individual records:

   ```python
   result = client.records.find(query)
   if result:
       first_record = result[0]
   ```

2. **Handle Pagination**: Use the `has_more` property to implement proper pagination:

   ```python
   if result.has_more:
       # Load next page
       pass
   ```

3. **Use Iteration**: Prefer iteration over index access when processing all records:

   ```python
   # Good
   for record in result:
       process(record)

   # Less efficient
   for i in range(len(result)):
       process(result[i])
   ```

4. **Monitor Total vs Length**: Understand the difference between `total` (all matching records) and `len()` (records in current page):

   ```python
   print(f"Showing {len(result)} of {result.total} total records")
   ```

5. **Use Skip and Limit for Pagination**: Leverage the `skip` and `limit` properties for implementing pagination:

   ```python
   # Get next page
   next_result = client.records.find({
       "labels": ["USER"],
       "limit": result.limit,
       "skip": result.skip + len(result)
   })
   ```

   Or use the built-in `next()` method:

   ```python
   page2 = result.next()
   ```

6. **pandas integration**: Use `to_dataframe()` for analysis, or use `export_csv()` for file export:
   ```python
   df = result.to_dataframe()            # pandas DataFrame, no __* columns
   csv = result.export_csv()             # raw CSV string
   result.delete_all()                   # clean up after processing
   result.set_properties({"reviewed": True})  # bulk update
   ```
