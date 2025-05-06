---
sidebar_position: 3
---
# Labels

Labels in RushDB are an essential part of the database schema, providing a way to categorize and organize records.

## How it works

Every record in RushDB has two labels:
1. A default system label (`__RUSHDB__LABEL__RECORD__`). This label is never exposed publicly.
2. A user-defined label that is searchable (e.g., `User`, `Car`, `Product`)

Labels help in organizing your data and enable efficient querying across similar types of records. They function similarly to table names in relational databases but with the flexibility of graph databases.

```typescript
// Record with label "User"
{
  "__id": "01968aa4-22c1-781a-8e8c-8fe6be6c3fd4",
  "__label": "User",  // User-defined label (required and limited to one per record)
  "__proptypes": {
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

## Label Requirements and Limitations

Currently, RushDB has the following requirements for labels:

1. **Single Custom Label**: Each record can have only one custom label at a time.
2. **Required Field**: A custom label is required for each record by default.
3. **Case-Sensitive**: Labels are case-sensitive, so "User" and "user" would be considered different labels.

These requirements help maintain a clean and consistent data structure across your database. For more details on how labels interact with other database elements, see [Records](/concepts/records) and [Properties](/concepts/properties).

## Label Assignment

Labels can be:
1. **Explicitly provided** by the user for top-level records
2. **Automatically derived** from parent keys for nested objects during the data import process

When importing nested JSON data, RushDB's breadth-first search algorithm automatically assigns appropriate labels based on the parent keys, making the process intuitive without requiring manual schema design.

For example, when importing this JSON:

```json
{
  "car": {
    "make": "Tesla",
    "model": "Model 3",
    "engine": {
      "power": 283,
      "type": "electric"
    }
  }
}
```

The label "car" is assigned to the top record, and "engine" is assigned to the nested record.

## Internal Representation

Internally, labels are stored as the `__RUSHDB__KEY__LABEL__` property and exposed to clients as `__label`. This property is essential for organizing records and enabling efficient queries across similar types of data.

To learn more about how records are structured and interconnected, see [Records](/concepts/records) and [Relationships](/concepts/relationships).

