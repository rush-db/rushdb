---
sidebar_position: 2
---

# Labels

The `PropertiesAPI` class provides methods for managing and querying properties in RushDB.

## Class Definition

```python
class LabelsAPI(BaseAPI):
```

## Methods

### find()

Retrieves a find of properties based on optional search criteria.

**Signature:**
```python
def find(
    self,
    query: Optional[SearchQuery] = None,
    transaction: Optional[Transaction] = None
) -> List[Property]
```

**Arguments:**
- `query` (Optional[SearchQuery]): Search query parameters for filtering properties
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `List[Property]`: List of properties matching the search criteria

**Example:**
```python
# Find all properties
properties = client.properties.find()

# Find properties with specific criteria
query = {
    "where": {
        "name": {"$startsWith": "user_"},  # Properties starting with 'user_'
        "type": "string"  # Only string type properties
    },
    "limit": 10  # Limit to 10 results
}
filtered_properties = client.properties.find(query)
```


## Transactions

Properties API methods support optional transactions for atomic operations:

```python
# Using a transaction
with client.transactions.begin() as transaction:
    # Perform multiple property-related operations
    property_to_delete = client.properties.find(
        {"where": {"name": "temp_property"}},
        transaction=transaction
    )[0]
    
    client.properties.delete(
        property_id=property_to_delete['id'],
        transaction=transaction
    )
    # Transaction will automatically commit if no errors occur
```

## Error Handling

When working with the PropertiesAPI, be prepared to handle potential errors:

```python
try:
    # Attempt to find or delete a property
    property_details = client.properties.find_by_id("non_existent_prop")
except RushDBError as e:
    print(f"Error: {e}")
    print(f"Error Details: {e.details}")
```
