---
sidebar_position: 1
---

# Import Data

The RushDB Python SDK provides powerful methods for importing data into your database. You can import data from various formats including JSON and CSV, with options to customize how the data is processed and stored.

## Overview

The import functionality in the Python SDK allows you to:
- Import JSON data structures
- Import CSV data from files or strings
- Control data type inference and handling
- Set default relationship types
- Configure property value handling

## Importing CSV Data

### import_csv()

Imports records from CSV data into RushDB.

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
- `label` (str): Label for all imported records
- `csv_data` (Union[str, bytes]): CSV data to import as a string or bytes
- `options` (Optional[Dict[str, bool]]): Import options
  - `suggestTypes` (bool): When true, automatically infers data types for properties
  - `castNumberArraysToVectors` (bool): When true, converts numeric arrays to vector type
  - `convertNumericValuesToNumbers` (bool): When true, converts string numbers to number type
  - `capitalizeLabels` (bool): When true, converts all labels to uppercase
  - `relationshipType` (str): Default relationship type between nodes
  - `returnResult` (bool): When true, returns imported records in response
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `List[Dict[str, Any]]`: Imported records data (if returnResult is True)

**Example:**
```python
# Import records from CSV string
csv_data = """name,email,age
John Doe,john@example.com,30
Jane Smith,jane@example.com,25
Bob Wilson,bob@example.com,45"""

records = client.records.import_csv(
    label="CUSTOMER",
    csv_data=csv_data,
    options={
        "returnResult": True,
        "suggestTypes": True,
        "convertNumericValuesToNumbers": True
    }
)

# Import records from CSV file
with open('employees.csv', 'r') as file:
    csv_content = file.read()

records = client.records.import_csv(
    label="EMPLOYEE",
    csv_data=csv_content,
    options={"returnResult": True, "suggestTypes": True}
)
```

## Importing JSON Data

### import_json()

Imports records from JSON data into RushDB.

**Signature:**
```python
def import_json(
    self,
    label: str,
    json_data: Union[Dict[str, Any], List[Dict[str, Any]]],
    options: Optional[Dict[str, Any]] = None,
    transaction: Optional[Transaction] = None
) -> List[Dict[str, Any]]
```

**Arguments:**
- `label` (str): Label for the root node(s)
- `json_data` (Union[Dict[str, Any], List[Dict[str, Any]]]): JSON data to import as dict or list of dicts
- `options` (Optional[Dict[str, Any]]): Import options
  - `suggestTypes` (bool): When true, automatically infers data types for properties
  - `castNumberArraysToVectors` (bool): When true, converts numeric arrays to vector type
  - `convertNumericValuesToNumbers` (bool): When true, converts string numbers to number type
  - `capitalizeLabels` (bool): When true, converts all labels to uppercase
  - `relationshipType` (str): Default relationship type between nodes
  - `returnResult` (bool): When true, returns imported records in response
- `transaction` (Optional[Transaction]): Optional transaction object

**Returns:**
- `List[Dict[str, Any]]`: Imported records data (if returnResult is True)

**Example:**
```python
# Import a single JSON object
person_data = {
    "name": "John Doe",
    "age": "30",
    "addresses": [
        {
            "type": "home",
            "street": "123 Main St",
            "city": "Anytown"
        },
        {
            "type": "work",
            "street": "456 Business Rd",
            "city": "Workville"
        }
    ],
    "scores": [85, 90, 95],
    "active": True
}

records = client.records.import_json(
    label="PERSON",
    json_data=person_data,
    options={
        "returnResult": True,
        "suggestTypes": True,
        "convertNumericValuesToNumbers": True,
        "relationshipType": "OWNS"
    }
)

# Import multiple JSON objects
employees_data = [
    {
        "name": "Alice Johnson",
        "department": "Engineering",
        "skills": ["Python", "JavaScript", "AWS"]
    },
    {
        "name": "Bob Smith",
        "department": "Marketing",
        "skills": ["SEO", "Content Writing", "Analytics"]
    }
]

records = client.records.import_json(
    label="EMPLOYEE",
    json_data=employees_data,
    options={"returnResult": True, "suggestTypes": True}
)
```

## Data Type Handling

When the `suggestTypes` option is enabled, RushDB will infer the following types:

- `string`: Text values
- `number`: Number values (& numeric values when `convertNumericValuesToNumbers` is true)
- `boolean`: True/false values
- `null`: Null values
- `vector`: Arrays of numbers (when `castNumberArraysToVectors` is true)
- `datetime`: ISO8601 format strings (e.g., "2025-04-23T10:30:00Z") will be automatically cast to datetime values

When `convertNumericValuesToNumbers` is enabled, string values that represent numbers (e.g., '123') will be automatically converted to their numeric equivalents (e.g., 123).

Arrays with consistent data types (e.g., all numbers, all strings) will be handled seamlessly according to their type. However, for inconsistent arrays (e.g., `[1, 'two', None, False]`), all values will be automatically converted to strings to mitigate data loss, and the property type will be stored as `string`.

## Graph Construction

When importing nested JSON data, RushDB automatically creates relationships between parent and child nodes. For example, if you import a person with addresses, RushDB will create:

1. A node with the "PERSON" label for the person data
2. Nodes with the "ADDRESS" label for each address
3. Relationships from the person to each address (using the default relationship type or the one specified)

This allows you to maintain complex data structures in a graph database format without manually creating the relationships.

## Performance Considerations

- For large imports, consider setting `returnResult: False` to improve performance
- Imports are processed in batches for optimal database performance
- Consider using transactions for large imports to ensure data consistency
- For very large datasets (millions of records), consider breaking the import into multiple smaller operations
