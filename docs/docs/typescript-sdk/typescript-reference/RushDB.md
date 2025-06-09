---
sidebar_position: 7
---

# RushDB

The `RushDB` class is the main entry point for interacting with the RushDB database. It manages API connections and model registration. It provides access to [records](../../concepts/records), [labels](../../concepts/labels), and [transactions](../../concepts/transactions).

## Initialization

```typescript
import RushDB from '@rushdb/javascript-sdk';

// Create an instance with an API token
const db = new RushDB('RUSHDB_API_TOKEN', {
  // Optionnaly provide API url to your RushDB instance
  url: 'https://api.rushdb.com/api/v1'
});
```

## Constructor

```typescript
constructor(token?: string, config?: SDKConfig)
```

Creates a new `RushDB` instance.

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `token` | `string` | Optional API token for authentication |
| `config` | `SDKConfig` | Optional configuration object |

### SDKConfig Options

| Option | Type | Description |
|--------|------|-------------|
| `url` | `string` | The base URL of the RushDB API |
| `timeout` | `number` | Request timeout in milliseconds |
| `httpClient` | `HttpClient` | Custom HTTP client implementation |
| `debug` | `boolean` | Enable debug mode |

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

### init()

```typescript
public static async init(): Promise<RushDB>
```

Initializes and returns the RushDB instance. This method is used internally by other SDK components (like Transaction and DBRecordInstance) to access the API.

**Returns**: A Promise that resolves to the RushDB instance

**Example**:
```typescript
// Internal usage in SDK components
async someMethod() {
  const instance = await RushDB.init()
  return await instance.someApi.someMethod()
}
```

## Instance Methods

### toInstance()

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
import RushDB, { Model } from '@rushdb/javascript-sdk';

// Initialize the SDK
const db = new RushDB('RUSHDB_API_TOKEN', {
  url: 'https://api.rushdb.com/api/v1'
});

// Define schemas
const UserSchema = {
  name: { type: 'string', required: true },
  email: { type: 'string', required: true, uniq: true },
  age: { type: 'number' }
};

const PostSchema = {
  title: { type: 'string', required: true },
  content: { type: 'string', required: true },
  published: { type: 'boolean', default: false }
};

// Create models
const UserModel = new Model('User', UserSchema);
const PostModel = new Model('Post', PostSchema);

// Use the models to interact with the database
async function main() {
  // Create a user
  const user = await UserModel.create({
    name: 'John Doe',
    email: 'john@example.com',
    age: 30
  });

  // Create a post
  const post = await PostModel.create({
    title: 'My First Post',
    content: 'This is my first post!',
    published: true
  });

  // Create a relationship
  await UserModel.attach({
    source: user,
    target: post,
    options: { type: 'AUTHORED' }
  });

  // Find all users
  const users = await UserModel.find();
  console.log(users.data?.length);

  // Find a specific user
  const foundUser = await UserModel.findOne({
    where: { email: 'john@example.com' }
  });
  console.log(foundUser?.data?.name);
}

main().catch(console.error);
```
