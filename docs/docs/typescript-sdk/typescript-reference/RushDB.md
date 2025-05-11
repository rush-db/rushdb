---
sidebar_position: 7
---

# RushDB

The `RushDB` class is the main entry point for interacting with the RushDB database. It manages API connections and model registration. It provides access to [records](/concepts/records), [labels](/concepts/labels), and [transactions](/concepts/transactions).

## Initialization

```typescript
import RushDB from '@rushdb/javascript-sdk';

// Create an instance with an API token
const db = new RushDB('your-api-token', {
  url: 'https://api.rushdb.com'
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

### models

```typescript
public models: Map<string, Model>
```

A map of registered models, keyed by their label.

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

### registerModel()

```typescript
public registerModel(model: Model): Model
```

Registers a model with the RushDB instance.

**Parameters**:
- `model`: The model to register

**Returns**: The registered model

**Example**:
```typescript
const UserModel = new Model('User', UserSchema);
db.registerModel(UserModel);
```

### getModels()

```typescript
public getModels(): Map<string, Model>
```

Gets all registered models.

**Returns**: A map of all registered models

### getModel()

```typescript
public getModel<Label extends keyof Models | string = keyof Models>(
  label: Label
): Label extends keyof Models ? Model<Models[Label]> : Model | undefined
```

Gets a registered model by its label.

**Parameters**:
- `label`: The label of the model to get

**Returns**: The requested model or undefined if not found

**Example**:
```typescript
const UserModel = db.getModel('User');
```

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
const db = new RushDB('your-api-token', {
  url: 'https://api.rushdb.com'
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

// Create and register models
const UserModel = new Model('User', UserSchema, db);
const PostModel = new Model('Post', PostSchema, db);

// Alternatively:
// const UserModel = new Model('User', UserSchema);
// db.registerModel(UserModel);

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
