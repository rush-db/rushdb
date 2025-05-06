---
sidebar_position: 1
---
# Records

In RushDB, Records are fundamental data structures that store meaningful key-value data. Each Record consists of individual properties (key-value pairs) and can be connected to other Records through relationships.

## How it works

Records in RushDB can be thought of as nodes in a graph database or rows in a traditional database. While the underlying implementation utilizes complex graph structures, from a user perspective, a Record is simply a key-value object containing properties.

Each record in RushDB consists of:

- User-defined properties (key-value pairs)
- System properties (prefixed with `__`)

Below is an example of a record with the label "User":

```typescript
{
  "__id": "01968aa4-22c1-781a-8e8c-8fe6be6c3fd4",  // Unique identifier
  "__label": "User",  // User-defined label (required and limited to one per record)
  "__proptypes": {    // Property types
    "name": "string",
    "emailConfirmed": "boolean",
    "registeredAt": "datetime",
    "rating": "number",
    "currency": "string",
    "email": "string"
  },
  "name": "John Galt",
  "emailConfirmed": true,
  "registeredAt": "2022-07-19T08:30:28.000Z",
  "rating": 4.98,
  "currency": "USD",
  "email": "john.galt@example.com"
}
```

Or this example with the label "Coffee":
```typescript
{
  "__id": "01968aa4-88c1-781a-8e8c-8fc6be7c3fd4",
  "__label": "Coffee",
  "__proptypes": {
    "origin":  "string",
    "process": "string",
    "cupping": "number",
    "inStock": "boolean",
    "roasted": "datetime",
    "notes": "string"
  },
  "origin": "Guatemala",
  "process": "washed",
  "cupping": 86,
  "inStock": true,
  "roasted": "2023-07-20T14:50:00Z",
  "notes": ["Nuts", "Caramel", "Lime"]
}
```

## Internal Structure

Internally, each Record in RushDB is represented as a node with two labels:
1. The system label `__RUSHDB__LABEL__RECORD__`
2. A user-defined label (exposed as `__label`)

In addition to user-defined properties, each Record contains several internal properties that enable advanced functionality:

| Internal Key | Client Representation | Description |
|--------------|----------------------|-------------|
| `__RUSHDB__KEY__ID__` | `__id` | UUIDv7 that enables lexicographic ordering |
| `__RUSHDB__KEY__PROPERTIES__META__` | `__proptypes` | Metadata about property types |
| `__RUSHDB__KEY__LABEL__` | `__label` | User-defined Record Label |
| `__RUSHDB__KEY__PROJECT__ID__` | (not exposed) | Project identifier for multitenancy isolation |

## Creating Records

Records can be created through:

1. Direct creation via the API or SDK
2. Automatic creation during data import

During record creation:
- A unique `__id` is automatically generated if not provided
- Property types are inferred from values when not specified
- Relationships are established based on data structure

Learn more at [REST API - Create Records](/rest-api/records/create-records) or through the language-specific SDKs:
- [TypeScript SDK](/typescript-sdk/records/create-records)
- [Python SDK](/python-sdk/records/create-records)


## Complex Data Structure

RushDB's architecture allows for nested data structures where Records can contain other Records. When importing hierarchical data like JSON objects, RushDB automatically:

1. Assigns unique IDs to all records
2. Applies appropriate labels based on parent keys or user specifications
3. Creates relationships between parent and child records
4. Stores property metadata for type inference

This process enables you to structure your data in a natural, intuitive way while maintaining the graph-based relationships that power efficient queries and traversals.

RushDB automatically manages parent-child relationships between records. When importing nested JSON:

```json
{
  "user": {
    "name": "Jane Doe",
    "address": {
      "city": "San Francisco",
      "country": "USA"
    }
  }
}
```

RushDB creates:
1. A "user" record containing the name property
2. An "address" record containing city and country properties
3. A relationship from "user" to "address"

Each nested object becomes its own record with:
- A label derived from its key in the parent object
- A unique ID
- Default relationships connecting it to its parent

For details on how to import complex data structures, see [REST API - Import Data](/rest-api/records/import-data)
For details on how properties are stored and managed, see the [Properties](/concepts/properties) section.
For information about the underlying storage structure, visit the [Storage](/concepts/storage) section.
