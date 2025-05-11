---
sidebar_position: 1
---

# Records

The `RecordsAPI` class provides methods for managing records in RushDB. It handles record creation, updates, deletion, searching, and relationship management.

## Class Definition

```python
class RecordsAPI(BaseAPI):
```

## Methods

### create()

Creates a new record in RushDB.

**Signature:**
```python
def create(
    self,
    label: str,
    data: Dict[str, Any],
    options: Optional[Dict[str, bool]] = None,
    transaction: Optional[Transaction] = None
) -> Record
```

**Arguments:**
- `label` (str): Label for the record
- `data` (Dict[str, Any]): Record data
- `options` (Optional[Dict[str, bool]]): Optional parsing and response options
  - `returnResult` (bool): Whether to return the created record
  - `suggestTypes` (bool): Whether to suggest property types
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Record`: Created record object

**Example:**
```python
# Create a new company record
data = {
    "name": "Google LLC",
    "address": "1600 Amphitheatre Parkway",
    "foundedAt": "1998-09-04T00:00:00.000Z",
    "rating": 4.9
}

record = client.records.create(
    label="COMPANY",
    data=data,
    options={"returnResult": True, "suggestTypes": True}
)
```

### create_many()

Creates multiple records in a single operation.

**Signature:**
```python
def create_many(
    self,
    label: str,
    data: Union[Dict[str, Any], List[Dict[str, Any]]],
    options: Optional[Dict[str, bool]] = None,
    transaction: Optional[Transaction] = None
) -> List[Record]
```

**Arguments:**
- `label` (str): Label for all records
- `data` (Union[Dict[str, Any], List[Dict[str, Any]]]): List or Dict of record data
- `options` (Optional[Dict[str, bool]]): Optional parsing and response options
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `List[Record]`: List of created record objects

**Example:**
```python
# Create multiple company records
data = [
    {
        "name": "Apple Inc",
        "address": "One Apple Park Way",
        "foundedAt": "1976-04-01T00:00:00.000Z",
        "rating": 4.8
    },
    {
        "name": "Microsoft Corporation",
        "address": "One Microsoft Way",
        "foundedAt": "1975-04-04T00:00:00.000Z",
        "rating": 4.7
    }
]

records = client.records.create_many(
    label="COMPANY",
    data=data,
    options={"returnResult": True, "suggestTypes": True}
)
```

### set()

Updates a record by ID, replacing all data.

**Signature:**
```python
def set(
    self,
    record_id: str,
    data: Dict[str, Any],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `record_id` (str): ID of the record to update
- `data` (Dict[str, Any]): New record data
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
# Update entire record data
new_data = {
    "name": "Updated Company Name",
    "rating": 5.0
}

response = client.records.set(
    record_id="record-123",
    data=new_data
)
```

### update()

Updates specific fields of a record by ID.

**Signature:**
```python
def update(
    self,
    record_id: str,
    data: Dict[str, Any],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `record_id` (str): ID of the record to update
- `data` (Dict[str, Any]): Partial record data to update
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
# Update specific fields
updates = {
    "rating": 4.8,
    "status": "active"
}

response = client.records.update(
    record_id="record-123",
    data=updates
)
```

### find()

Searches for records matching specified criteria.

**Signature:**
```python
def find(
    self,
    query: Optional[SearchQuery] = None,
    record_id: Optional[str] = None,
    transaction: Optional[Transaction] = None
) -> List[Record]
```

**Arguments:**
- `query` (Optional[SearchQuery]): Search query parameters
- `record_id` (Optional[str]): Optional record ID to search from
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `List[Record]`: List of matching records

**Example:**
```python
# Search for records with complex criteria
query = {
    "where": {
        "$and": [
            {"age": {"$gte": 18}},
            {"status": "active"},
            {"department": "Engineering"}
        ]
    },
    "orderBy": {"created_at": "desc"},
    "limit": 10
}

records = client.records.find(query=query)
```

### delete()

Deletes records matching a query.

**Signature:**
```python
def delete(
    self,
    query: SearchQuery,
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `query` (SearchQuery): Query to match records for deletion
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
# Delete records matching criteria
query = {
    "where": {
        "status": "inactive",
        "lastActive": {"$lt": "2023-01-01"}
    }
}

response = client.records.delete(query)
```

### delete_by_id()

Deletes one or more records by ID.

**Signature:**
```python
def delete_by_id(
    self,
    id_or_ids: Union[str, List[str]],
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `id_or_ids` (Union[str, List[str]]): Single ID or find of IDs to delete
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
# Delete single record
response = client.records.delete_by_id("record-123")

# Delete multiple records
response = client.records.delete_by_id([
    "record-123",
    "record-456",
    "record-789"
])
```

### attach()

Creates relationships between records.

**Signature:**
```python
def attach(
    self,
    source: Union[str, Dict[str, Any]],
    target: Union[str, List[str], Dict[str, Any], List[Dict[str, Any]], Record, List[Record]],
    options: Optional[RelationshipOptions] = None,
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `source` (Union[str, Dict[str, Any]]): Source record ID or data
- `target` (Union[str, List[str], Dict[str, Any], List[Dict[str, Any]], Record, List[Record]]): Target record(s)
- `options` (Optional[RelationshipOptions]): Relationship options
  - `direction` (Optional[Literal["in", "out"]]): Relationship direction
  - `type` (Optional[str]): Relationship type
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
# Create relationship between records
options = RelationshipOptions(
    type="HAS_EMPLOYEE",
    direction="out"
)

response = client.records.attach(
    source="company-123",
    target=["employee-456", "employee-789"],
    options=options
)
```

### detach()

Removes relationships between records.

**Signature:**
```python
def detach(
    self,
    source: Union[str, Dict[str, Any]],
    target: Union[str, List[str], Dict[str, Any], List[Dict[str, Any]], Record, List[Record]],
    options: Optional[RelationshipDetachOptions] = None,
    transaction: Optional[Transaction] = None
) -> Dict[str, str]
```

**Arguments:**
- `source` (Union[str, Dict[str, Any]]): Source record ID or data
- `target` (Union[str, List[str], Dict[str, Any], List[Dict[str, Any]], Record, List[Record]]): Target record(s)
- `options` (Optional[RelationshipDetachOptions]): Detach options
  - `direction` (Optional[Literal["in", "out"]]): Relationship direction
  - `typeOrTypes` (Optional[Union[str, List[str]]]): Relationship type(s)
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `Dict[str, str]`: Response data

**Example:**
```python
# Remove relationships between records
options = RelationshipDetachOptions(
    typeOrTypes=["HAS_EMPLOYEE", "MANAGES"],
    direction="out"
)

response = client.records.detach(
    source="company-123",
    target="employee-456",
    options=options
)
```

### import_csv()

Imports records from CSV data.

**Signature:**
```python
def import_csv(
    self,
    label: str,
    csv_data: Union[str, bytes],
    options: Optional[Dict[str, bool]] = None,
    transaction: Optional[Transaction] = None
) -> List[Dict[str, Any]]
```

**Arguments:**
- `label` (str): Label for imported records
- `csv_data` (Union[str, bytes]): CSV data to import
- `options` (Optional[Dict[str, bool]]): Import options
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `List[Dict[str, Any]]`: Imported records data

**Example:**
```python
# Import records from CSV
csv_data = """name,age,department,role
John Doe,30,Engineering,Senior Engineer
Jane Smith,28,Product,Product Manager
Bob Wilson,35,Engineering,Tech Lead"""

records = client.records.import_csv(
    label="EMPLOYEE",
    csv_data=csv_data,
    options={"returnResult": True, "suggestTypes": True}
)
```
