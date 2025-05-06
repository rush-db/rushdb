---
sidebar_position: 5
---

# Transactions

The `PropertiesAPI` class provides methods for managing and querying properties in RushDB.

## Class Definition

```python
class PropertiesAPI(BaseAPI):
```

## Methods

### find

Retrieves a list of properties based on optional search criteria.

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

### find_by_id

Retrieves a specific property by its ID.

**Signature:**
```python
def find_by_id(
    self,
    property_id: str,
    transaction: Optional[Transaction] = None
) -> Property
```

**Arguments:**
- `property_id` (str): Unique identifier of the property
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Property`: Property details

**Example:**
```python
# Retrieve a specific property by ID
property_details = client.properties.find_by_id("prop_123456")
```

### delete

Deletes a property by its ID.

**Signature:**
```python
def delete(
    self,
    property_id: str,
    transaction: Optional[Transaction] = None
) -> None
```

**Arguments:**
- `property_id` (str): Unique identifier of the property to delete
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `None`

**Example:**
```python
# Delete a property
client.properties.delete("prop_123456")
```

### values

Retrieves values for a specific property with optional sorting and pagination.

**Signature:**
```python
def values(
    self,
    property_id: str,
    sort: Optional[Literal["asc", "desc"]] = None,
    skip: Optional[int] = None,
    limit: Optional[int] = None,
    transaction: Optional[Transaction] = None
) -> PropertyValuesData
```

**Arguments:**
- `property_id` (str): Unique identifier of the property
- `sort` (Optional[Literal["asc", "desc"]]): Sort order of values
- `skip` (Optional[int]): Number of values to skip (for pagination)
- `limit` (Optional[int]): Maximum number of values to return
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `PropertyValuesData`: Property values data, including optional min/max and list of values

**Example:**
```python
# Get property values
values_data = client.properties.values(
    property_id="prop_age",
    sort="desc",  # Sort values in descending order
    skip=0,       # Start from the first value
    limit=100     # Return up to 100 values
)

# Access values
print(values_data.get('values', []))  # List of property values
print(values_data.get('min'))         # Minimum value (for numeric properties)
print(values_data.get('max'))         # Maximum value (for numeric properties)
```

## Comprehensive Usage Example

```python
# Find all properties
all_properties = client.properties.find()
for prop in all_properties:
    print(f"Property ID: {prop['id']}")
    print(f"Name: {prop['name']}")
    print(f"Type: {prop['type']}")
    print(f"Metadata: {prop.get('metadata', 'No metadata')}")
    print("---")

# Detailed property search
query = {
    "where": {
        "type": "number",             # Only numeric properties
        "name": {"$contains": "score"}  # Properties with 'score' in name
    },
    "limit": 5  # Limit to 5 results
}
numeric_score_properties = client.properties.find(query)

# Get values for a specific property
if numeric_score_properties:
    first_prop = numeric_score_properties[0]
    prop_values = client.properties.values(
        property_id=first_prop['id'],
        sort="desc",
        limit=50
    )
    print(f"Values for {first_prop['name']}:")
    print(f"Min: {prop_values.get('min')}")
    print(f"Max: {prop_values.get('max')}")
    
    # Detailed property examination
    detailed_prop = client.properties.find_by_id(first_prop['id'])
    print("Detailed Property Info:", detailed_prop)
```

## Property Types and Structures

RushDB supports the following property types:
- `"boolean"`: True/False values
- `"datetime"`: Date and time values
- `"null"`: Null/empty values
- `"number"`: Numeric values
- `"string"`: Text values

### Property Structure Example
```python
property = {
    "id": "prop_unique_id",
    "name": "user_score",
    "type": "number",
    "metadata": Optional[str]  # Optional additional information
}

property_with_value = {
    "id": "prop_unique_id",
    "name": "user_score",
    "type": "number",
    "value": 95.5  # Actual property value
}
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
