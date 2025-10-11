---
sidebar_position: 1
---

# Import Data

When importing data into RushDB, choose the method that matches your data shape:

- createMany — arrays of flat rows only (CSV-like). No nested objects or arrays inside items.
- importJson — real JSON: nested, messy, arrays with nested data, or a hash-map-like top-level structure.

This page explains when to use each and shows practical examples. Keep using createMany where you already import flat arrays; use importJson for everything else.

## createMany: arrays of flat rows (CSV-like)
Use createMany when your input is an array (or single object) of flat rows. Nested objects/arrays are not allowed.

```typescript
import RushDB from '@rushdb/javascript-sdk';
const db = new RushDB(process.env.RUSHDB_API_KEY!);

const authors = [
  { name: 'Alice Johnson', email: 'alice@example.com', age: 30 },
  { name: 'Bob Smith', email: 'bob@example.com', age: 25 }
];

const result = await db.records.createMany({
  label: 'AUTHOR',
  data: authors,
  options: { suggestTypes: true }
});

console.log(result.data.map(r => r.data));
```

## Importing Data from CSV

Use `importCsv` when your data source is a CSV string. You can control both import options (type inference etc.) and a subset of CSV parsing configuration.

```typescript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB(process.env.RUSHDB_API_KEY!);

const csv = `name,email,age\nJohn Doe,john@example.com,30\nJane Smith,jane@example.com,25`;

const customers = await db.records.importCsv({
  label: 'CUSTOMER',
  data: csv,
  options: {
    suggestTypes: true,
    convertNumericValuesToNumbers: true,
    returnResult: true
  },
  parseConfig: {
    delimiter: ',',
    header: true,
    skipEmptyLines: true,
    dynamicTyping: true
  }
});

console.log(customers.data.map(c => c.data));
```

### CSV Parse Configuration (parseConfig)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `delimiter` | `string` | `,` | Field delimiter |
| `header` | `boolean` | `true` | Whether first row contains headers |
| `skipEmptyLines` | `boolean` \| `"greedy"` | `true` | Skip empty (or whitespace-only when `greedy`) lines |
| `dynamicTyping` | `boolean` | Mirrors `options.suggestTypes` | PapaParse numeric/boolean autodetection |
| `quoteChar` | `string` | `"` | Quote character |
| `escapeChar` | `string` | `"` | Escape character for quotes |
| `newline` | `string` | auto | Explicit newline sequence override |

If `parseConfig.dynamicTyping` is omitted, it inherits from `options.suggestTypes`.

## importJson: nested or hash-map-like JSON
Use importJson when your data is nested or “messy” (arrays with nested objects, objects-within-objects, etc.), or when your top-level input is a hash map of label -> items.

importJson works in two modes:

1) With label provided — you explicitly set the top-level label.

```typescript
import RushDB from '@rushdb/javascript-sdk';
import fs from 'fs';

const db = new RushDB(process.env.RUSHDB_API_KEY!);
const data = JSON.parse(fs.readFileSync('data.json', 'utf8'));

// data.json can be nested/messy. importJson will BFS through and create
// records and relationships according to structure.
const imported = await db.records.importJson({
  label: 'BLOG',
  data,
  options: { suggestTypes: true }
});

console.log(imported.data.length);
```

2) Without label — pass a single top-level key used as the label:

```typescript
const payload = {
  ITEM: [
    { name: 'Sprocket', specs: { size: 'M', weight: 1.2 }, images: ['a.jpg', 'b.jpg'] },
    { name: 'Cog', specs: { size: 'S', weight: 0.7 } }
  ]
}

// Label is inferred as 'ITEM'
await db.records.importJson({ data: payload });
```

Unlabeled invalid root object — if you don’t provide label and the top level is not a single-key map, importJson throws:

```typescript
await db.records.importJson({
  data: {
    some: 'key',
    data: 1,
    nested: { level: 2 }
  }
});
// Error: importJson requires either an explicit label or a single top-level key to infer the label
```

### Advanced Usage: Import Options

The `importJson` method accepts an optional `options` parameter to customize how your data is processed and stored:

```typescript
const importOptions = {
  suggestTypes: true,
  convertNumericValuesToNumbers: true,
  capitalizeLabels: false,
  relationshipType: 'OWNS',
  returnResult: true,
  castNumberArraysToVectors: false
};

const importedUsers = await db.records.importJson({ label: 'user', data: data.users, options: importOptions })
```

### Available Options (JSON & CSV)

| Option                          | Type    | Default                         | Description                                       |
|---------------------------------|---------|---------------------------------|---------------------------------------------------|
| `suggestTypes`                  | Boolean | `true`                          | Automatically infers data types for properties    |
| `castNumberArraysToVectors`     | Boolean | `false`                         | Converts numeric arrays to vector type            |
| `convertNumericValuesToNumbers` | Boolean | `false`                         | Converts string numbers to number type            |
| `capitalizeLabels`              | Boolean | `false`                         | Converts all labels to uppercase                  |
| `relationshipType`              | String  | `__RUSHDB__RELATION__DEFAULT__` | Default relationship type between Records (nodes) |
| `returnResult`                  | Boolean | `false`                         | Returns imported records in response              |

## Quick rules recap

- createMany: arrays of flat rows only. Nested objects or arrays inside items are not allowed and will cause an error — use importJson instead.
- importJson: nested/mixed JSON. Provide label explicitly, or pass a single-key object like `{ LABEL: [...] }` to infer the label.
- importCsv: CSV string input with parseConfig; dynamicTyping inherits from options.suggestTypes when omitted.

## How RushDB JSON Import Works

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
- CSV imports currently read the full string; for very large files consider splitting client-side

## Related Documentation

- [REST API - Import Data](../../rest-api/records/import-data) - Complete API details for data import
- [Storage Internals](../../concepts/storage) - Technical details about how RushDB stores your data
- [Properties](../../concepts/properties) - Learn about property handling and type inference
- [Transactions](../../concepts/transactions.mdx) - Understand how RushDB ensures data integrity during imports
