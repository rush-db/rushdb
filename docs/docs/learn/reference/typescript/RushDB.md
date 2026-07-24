---
sidebar_position: 7
---

# RushDB

The `RushDB` class is the main entry point for interacting with the RushDB database. It manages API connections and model registration. It provides access to [records](/learn/records-and-queries/store-records), [labels](/learn/records-and-queries/labels-and-properties), and [transactions](/learn/records-and-queries/transactions).

## Initialization

```typescript
import RushDB from '@rushdb/javascript-sdk'

// Create an instance with an API token
const db = new RushDB('RUSHDB_API_KEY', {
  // Optionnaly provide API url to your RushDB instance
  url: 'https://api.rushdb.com/api/v1'
})
```

## Constructor

```typescript
constructor(token?: string, config?: SDKConfig)
```

Creates a new `RushDB` instance.

### Parameters

| Parameter | Type        | Description                           |
| --------- | ----------- | ------------------------------------- |
| `token`   | `string`    | Optional API token for authentication |
| `config`  | `SDKConfig` | Optional configuration object         |

### SDKConfig Options

| Option       | Type         | Description                       |
| ------------ | ------------ | --------------------------------- |
| `url`        | `string`     | The base URL of the RushDB API    |
| `timeout`    | `number`     | Request timeout in milliseconds   |
| `httpClient` | `HttpClient` | Custom HTTP client implementation |
| `debug`      | `boolean`    | Enable debug mode                 |

## Namespaces

All database operations are accessed through sub-namespaces on the client instance.

### `db.records`

CRUD, import, export, and relationship operations on records.

| Method                                                                             | Description                                                                                                                                                                            |
| ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `create({ label, data, options, vectors }, transaction)`                           | Create a single record                                                                                                                                                                 |
| `createMany({ label, data, options, vectors }, transaction)`                       | Create multiple flat records; supports upsert via `options.mergeBy`. `vectors` accepts both `VectorEntry[]` (one per row, auto-wrapped) and `VectorEntry[][]` (explicit per-row lists) |
| `importJson({ label, data, options }, transaction)`                                | Import nested or complex JSON payloads; `label` may be omitted for container objects whose top-level values are objects or arrays of nested records                                    |
| `importCsv({ label, data, options, parseConfig, parentId, vectors }, transaction)` | Import records from CSV text; supports upsert via `options.mergeBy`                                                                                                                    |
| `upsert({ label, data, options, vectors }, transaction)`                           | Create or update a record by properties in `options.mergeBy`                                                                                                                           |
| `set({ target, label, data, options, vectors }, transaction)`                      | Replace all fields of a record                                                                                                                                                         |
| `update({ target, label, data, options }, transaction)`                            | Partially update a record                                                                                                                                                              |
| `find(searchQuery, transaction)`                                                   | Search records                                                                                                                                                                         |
| `findOne(searchQuery, transaction)`                                                | Return the first match                                                                                                                                                                 |
| `findUniq(searchQuery, transaction)`                                               | Return the single match; raises if ambiguous                                                                                                                                           |
| `findById(idOrIds, transaction)`                                                   | Fetch record(s) by ID                                                                                                                                                                  |
| `delete(searchQuery, transaction)`                                                 | Delete all records matching a query                                                                                                                                                    |
| `deleteById(idOrIds, transaction)`                                                 | Delete record(s) by ID                                                                                                                                                                 |
| `attach({ source, target, options }, transaction)`                                 | Create relationships between records                                                                                                                                                   |
| `detach({ source, target, options }, transaction)`                                 | Remove relationships between records                                                                                                                                                   |
| `export(searchQuery, transaction)`                                                 | Export matching records as CSV text                                                                                                                                                    |

For bulk imports, pass `options: { mergeBy: ['propertyName'], mergeStrategy: 'append' | 'rewrite' }` to upsert by property instead of creating duplicates. See [Upsert by Property During Import](/learn/records-and-queries/import-data#upsert-by-property-during-import).

### `db.relationships`

Query and bulk-manage relationships.

| Method                           | Description                                                 |
| -------------------------------- | ----------------------------------------------------------- |
| `find(searchQuery, transaction)` | Search relationships                                        |
| `createMany(data, transaction)`  | Bulk-create relationships by key-match or cartesian product |
| `deleteMany(data, transaction)`  | Bulk-delete relationships                                   |

#### `db.relationships.patterns`

Review and manage relationship patterns inferred from the project schema. See [Relationship Patterns](/learn/reference/typescript/relationship-patterns) for the full review flow.

| Method                | Description                                                            |
| --------------------- | ---------------------------------------------------------------------- |
| `list()`              | List saved patterns, schema relationships, and analysis status         |
| `analyze()`           | Queue schema analysis to generate suggestions                          |
| `approve(id)`         | Approve a suggestion and apply its relationships                       |
| `ignore(id)`          | Ignore a suggestion without applying it                                |
| `delete(id, options)` | Delete a saved pattern, optionally removing materialized relationships |

### `db.labels`

Discover record labels in the database.

| Method                           | Description                                            |
| -------------------------------- | ------------------------------------------------------ |
| `find(searchQuery, transaction)` | Return a record of `{ label: count }` matching a query |

### `db.properties`

Inspect property metadata.

| Method                                 | Description                         |
| -------------------------------------- | ----------------------------------- |
| `find(searchQuery, transaction)`       | List properties matching a query    |
| `findById(id, transaction)`            | Retrieve a property by ID           |
| `delete(id, transaction)`              | Delete a property by ID             |
| `values(id, searchQuery, transaction)` | List distinct values for a property |

### `db.tx`

Transaction lifecycle management.

| Method                  | Description                                |
| ----------------------- | ------------------------------------------ |
| `begin(config)`         | Start a transaction; returns `Transaction` |
| `get(transaction)`      | Retrieve a transaction by object or ID     |
| `commit(transaction)`   | Commit a transaction by object or ID       |
| `rollback(transaction)` | Roll back a transaction by object or ID    |

See also: [Transaction](/learn/reference/typescript/Transaction) for the `commit()` and `rollback()` instance methods.

### `db.ai`

Semantic search and schema exploration. Embedding index management is available under `db.ai.indexes`.

| Method                                   | Description                                   |
| ---------------------------------------- | --------------------------------------------- |
| `search(params)`                         | Semantic search over indexed properties       |
| `getSchema(params, transaction)`         | Return the graph schema as structured JSON    |
| `getSchemaMarkdown(params, transaction)` | Return the schema as token-efficient Markdown |

#### `db.ai.indexes`

| Method                      | Description                                           |
| --------------------------- | ----------------------------------------------------- |
| `find()`                    | List all embedding index policies                     |
| `create(params)`            | Create an embedding index for a property              |
| `delete(id)`                | Delete an embedding index by ID                       |
| `stats(id)`                 | Get Neo4j-level statistics for an index               |
| `upsertVectors(id, params)` | Bulk-seed an external index with pre-computed vectors |

### `db.query`

Raw Cypher query execution. **Cloud-only** — not available on self-hosted instances without a managed database.

| Method                                | Description                                                |
| ------------------------------------- | ---------------------------------------------------------- |
| `raw({ query, params }, transaction)` | Execute a raw Cypher query string with optional parameters |

### `db.settings`

Project configuration.

| Method  | Description                           |
| ------- | ------------------------------------- |
| `get()` | Retrieve the current project settings |

## Properties

### state

```typescript
state: State
```

The internal state of the SDK instance.

## Static Methods

### getInstance()

```typescript
public static getInstance(): RushDB
```

Gets the singleton instance of the RushDB class.

**Returns**: The RushDB instance

## Instance Methods

### toInstance()

```typescript
public toInstance<S extends Schema = Schema>(record: DBRecord<S>): DBRecordInstance<S>
```

Converts a database record to a record instance.

**Parameters**:

- `record`: The record to convert

**Returns**: A record instance with additional methods

## Usage Example

```typescript
import RushDB, { Model } from '@rushdb/javascript-sdk'

// Initialize the SDK
const db = new RushDB('RUSHDB_API_KEY', {
  url: 'https://api.rushdb.com/api/v1'
})

// Define schemas
const UserSchema = {
  name: { type: 'string', required: true },
  email: { type: 'string', required: true, unique: true },
  age: { type: 'number' }
}

const PostSchema = {
  title: { type: 'string', required: true },
  content: { type: 'string', required: true },
  published: { type: 'boolean', default: false }
}

// Create models
const UserModel = new Model('User', UserSchema)
const PostModel = new Model('Post', PostSchema)

// Use the models to interact with the database
async function main() {
  // Create a user
  const user = await UserModel.create({
    name: 'John Doe',
    email: 'john@example.com',
    age: 30
  })

  // Create a post
  const post = await PostModel.create({
    title: 'My First Post',
    content: 'This is my first post!',
    published: true
  })

  // Create a relationship
  await UserModel.attach({
    source: user,
    target: post,
    options: { type: 'AUTHORED' }
  })

  // Find all users
  const users = await UserModel.find()
  console.log(users.data?.length)

  // Find a specific user
  const foundUser = await UserModel.findOne({
    where: { email: 'john@example.com' }
  })
  console.log(foundUser?.data?.name)
}

main().catch(console.error)
```
