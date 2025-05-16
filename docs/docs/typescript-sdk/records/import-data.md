---
sidebar_position: 1
---

# Import Data

When working with RushDB SDK, creating models like Author, Post, and Blog repositories allows us to define clear TypeScript contracts, ensuring type safety and better development experience. However, in many scenarios, you might need to quickly import data from external sources, such as JSON files, without going through the process of registering models.

Using the `createMany` method, you can efficiently import large datasets into RushDB by directly specifying the labels and payloads. This approach is ideal for batch imports, data migrations, and integrating external data sources.

## Example: Importing Data from JSON
In this example, we will demonstrate how to import user, post, and blog data from a JSON file into RushDB SDK using the `createMany` method:

```typescript
import RushDB from '@rushdb/javascript-sdk';
import fs from 'fs';

// Initialize the SDK
const db = new RushDB('API_TOKEN');

// Load data from a JSON file
const data = JSON.parse(fs.readFileSync('data.json', 'utf8'));

// Example JSON structure
/*
{
  "users": [
    { "name": "Alice Johnson", "email": "alice@example.com", "age": 30 },
    { "name": "Bob Smith", "email": "bob@example.com", "age": 25 }
  ],
  "posts": [
    { "title": "Introduction to RushDB SDK", "content": "This is a post about RushDB SDK...", "authorEmail": "alice@example.com" },
    { "title": "Advanced RushDB SDK Usage", "content": "This post covers advanced usage of RushDB SDK...", "authorEmail": "bob@example.com" }
  ],
  "blogs": [
    { "title": "Alice's Tech Blog", "description": "A blog about tech by Alice.", "ownerEmail": "alice@example.com" },
    { "title": "Bob's Coding Adventures", "description": "Bob shares his coding journey.", "ownerEmail": "bob@example.com" }
  ]
}
*/

// Function to import data
async function importData() {
  try {
    // Import users
    const importedUsers = await db.records.createMany({label: 'user', payload: data.users});
    console.log('Imported Users:', importedUsers.data);

    // Import posts
    const importedPosts = await db.records.createMany({label: 'post', payload: data.posts});
    console.log('Imported Posts:', importedPosts.data);

    // Import blogs
    const importedBlogs = await db.records.createMany({label: 'blog', payload: data.blogs});
    console.log('Imported Blogs:', importedBlogs.data);
  } catch (error) {
    console.error('Error importing data:', error);
  }
}

// Run the import function
importData();
```

## Advanced Usage: Import Options

The `createMany` method accepts an optional third parameter to customize how your data is processed and stored:

```typescript
const importOptions = {
  suggestTypes: true,
  convertNumericValuesToNumbers: true,
  capitalizeLabels: false,
  relationshipType: 'OWNS',
  returnResult: true,
  castNumberArraysToVectors: false
};

const importedUsers = await db.records.createMany({
  labels: 'user',
  payload: data.users, 
  options: importOptions
});
```

### Available Options

| Option                          | Type    | Default                         | Description                                       |
|---------------------------------|---------|---------------------------------|---------------------------------------------------|
| `suggestTypes`                  | Boolean | `true`                          | Automatically infers data types for properties    |
| `castNumberArraysToVectors`     | Boolean | `false`                         | Converts numeric arrays to vector type            |
| `convertNumericValuesToNumbers` | Boolean | `false`                         | Converts string numbers to number type            |
| `capitalizeLabels`              | Boolean | `false`                         | Converts all labels to uppercase                  |
| `relationshipType`              | String  | `__RUSHDB__RELATION__DEFAULT__` | Default relationship type between Records (nodes) |
| `returnResult`                  | Boolean | `false`                         | Returns imported records in response              |

## How RushDB Import Works

When you import data through the TypeScript SDK, RushDB applies a breadth-first search (BFS) algorithm to parse and transform your data:

1. **Data Preparation**: Each record is assigned a unique UUIDv7 `__id` (unless provided)
2. **Type Inference**: If `suggestTypes` is enabled, RushDB analyzes values to determine appropriate data types
3. **Graph Construction**: Records become nodes in the graph database with properties and relationships
4. **Metadata Generation**: Type information is stored in `__proptypes` for each record
5. **Storage**: Data is efficiently inserted into the underlying Neo4j database

### Data Structure Example

For example, importing this JSON:

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

Creates this graph structure in RushDB:

- A `car` node with properties `make: "Tesla"` and `model: "Model 3"`
- An `engine` node with properties `power: 283` and `type: "electric"`
- A relationship connecting the car to its engine
- Property metadata nodes tracking property names and types

The TypeScript SDK abstracts this complexity, allowing you to focus on your data models.

## Performance Considerations

- For large data imports (>1,000 records), consider batching your requests in chunks
- Setting `returnResult: false` is recommended for large imports to improve performance
- For time-critical imports, pre-process your data to ensure type consistency

## Related Documentation

- [REST API - Import Data](../../rest-api/records/import-data) - Complete API details for data import
- [Storage Internals](../../concepts/storage) - Technical details about how RushDB stores your data
- [Properties](../../concepts/properties) - Learn about property handling and type inference
- [Transactions](../../concepts/transactions.mdx) - Understand how RushDB ensures data integrity during imports
