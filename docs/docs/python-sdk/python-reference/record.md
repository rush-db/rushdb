---
sidebar_position: 2
---

# Record

The `Record` class represents a record in RushDB and provides methods for manipulating individual records, including updates, relationships, and deletions.

## Class Definition

```python
class Record:
    """Represents a record in RushDB with methods for manipulation."""
    def __init__(self, client: "RushDB", data: Union[Dict[str, Any], None] = None)
```

## Properties

### id

Gets the record's unique identifier.

**Type:** `str`

**Example:**
```python
record = client.records.create("USER", {"name": "John"})
print(record.id)  # e.g., "1234abcd-5678-..."
```

### proptypes

Gets the record's property types.

**Type:** `str`

**Example:**
```python
record = client.records.create("USER", {"name": "John", "age": 25})
print(record.proptypes)  # Returns property type definitions
```

### label

Gets the record's label.

**Type:** `str`

**Example:**
```python
record = client.records.create("USER", {"name": "John"})
print(record.label)  # "USER"
```

### timestamp

Gets the record's creation timestamp from its ID.

**Type:** `int`

**Example:**
```python
record = client.records.create("USER", {"name": "John"})
print(record.timestamp)  # Unix timestamp in milliseconds
```

### date

Gets the record's creation date.

**Type:** `datetime`

**Example:**
```python
record = client.records.create("USER", {"name": "John"})
print(record.date)  # datetime object
```

## Methods

### set()

Updates all data for the record.

**Signature:**
```python
def set(
    self,
    data: Dict[str, Any],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `data` (Dict[str, Any]): New record data
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
record = client.records.create("USER", {"name": "John"})
response = record.set({
    "name": "John Doe",
    "email": "john@example.com",
    "age": 30
})
```

### update()

Updates specific fields of the record.

**Signature:**
```python
def update(
    self,
    data: Dict[str, Any],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `data` (Dict[str, Any]): Partial record data to update
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
record = client.records.create("USER", {
    "name": "John",
    "email": "john@example.com"
})
response = record.update({
    "email": "john.doe@example.com"
})
```

### attach()

Creates relationships with other records.

**Signature:**
```python
def attach(
    self,
    target: Union[str, List[str], Dict[str, Any], List[Dict[str, Any]], "Record", List["Record"]],
    options: Optional[RelationshipOptions] = None,
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `target` (Union[str, List[str], Dict[str, Any], List[Dict[str, Any]], Record, List[Record]]): Target record(s)
- `options` (Optional[RelationshipOptions]): Relationship options
  - `direction` (Optional[Literal["in", "out"]]): Relationship direction
  - `type` (Optional[str]): Relationship type
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
# Create two records
user = client.records.create("USER", {"name": "John"})
group = client.records.create("GROUP", {"name": "Admins"})

# Attach user to group
response = user.attach(
    target=group,
    options=RelationshipOptions(
        type="BELONGS_TO",
        direction="out"
    )
)
```

### detach()

Removes relationships with other records.

**Signature:**
```python
def detach(
    self,
    target: Union[str, List[str], Dict[str, Any], List[Dict[str, Any]], "Record", List["Record"]],
    options: Optional[RelationshipDetachOptions] = None,
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `target` (Union[str, List[str], Dict[str, Any], List[Dict[str, Any]], Record, List[Record]]): Target record(s)
- `options` (Optional[RelationshipDetachOptions]): Detach options
  - `direction` (Optional[Literal["in", "out"]]): Relationship direction
  - `typeOrTypes` (Optional[Union[str, List[str]]]): Relationship type(s)
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
# Detach user from group
response = user.detach(
    target=group,
    options=RelationshipDetachOptions(
        typeOrTypes="BELONGS_TO",
        direction="out"
    )
)
```

### delete()

Deletes the record.

**Signature:**
```python
def delete(
    self,
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
user = client.records.create("USER", {"name": "John"})
response = user.delete()
```

## Complete Usage Example

Here's a comprehensive example demonstrating various Record operations:

```python
# Create a new record
user = client.records.create("USER", {
    "name": "John Doe",
    "email": "john@example.com",
    "age": 30
})

# Access properties
print(f"Record ID: {user.id}")
print(f"Label: {user.label}")
print(f"Created at: {user.date}")

# Update record data
user.update({
    "age": 31,
    "title": "Senior Developer"
})

# Create related records
department = client.records.create("DEPARTMENT", {
    "name": "Engineering"
})

project = client.records.create("PROJECT", {
    "name": "Secret Project"
})

# Create relationships
user.attach(
    target=department,
    options=RelationshipOptions(
        type="BELONGS_TO",
        direction="out"
    )
)

user.attach(
    target=project,
    options=RelationshipOptions(
        type="WORKS_ON",
        direction="out"
    )
)

# Remove relationship
user.detach(
    target=project,
    options=RelationshipDetachOptions(
        typeOrTypes="WORKS_ON",
        direction="out"
    )
)

# Delete record
user.delete()
```

## Working with Transactions

Records can be manipulated within transactions for atomic operations:

```python
# Start a transaction
with client.transactions.begin() as transaction:
    # Create user
    user = client.records.create(
        "USER",
        {"name": "John Doe"},
        transaction=transaction
    )
    
    # Update user
    user.update(
        {"status": "active"},
        transaction=transaction
    )
    
    # Create and attach department
    dept = client.records.create(
        "DEPARTMENT",
        {"name": "Engineering"},
        transaction=transaction
    )
    
    user.attach(
        target=dept,
        options=RelationshipOptions(type="BELONGS_TO"),
        transaction=transaction
    )
    
    # Transaction will automatically commit if no errors occur
    # If an error occurs, it will automatically rollback
```
