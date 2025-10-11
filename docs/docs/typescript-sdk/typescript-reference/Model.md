---
sidebar_position: 1
---

# Model

The `Model` class represents a schema-defined entity in RushDB. It provides methods to perform CRUD operations and manage [relationships](../../concepts/relationships) between [records](../../concepts/records). Models are identified by [labels](../../concepts/labels) in the database.

## Usage

```typescript
import { Model } from '@rushdb/javascript-sdk';

// Define a schema
const UserSchema = {
  name: { type: 'string', required: true },
  email: { type: 'string', required: true, unique: true },
  age: { type: 'number' },
  active: { type: 'boolean', default: true }
};

// Create a model
const UserModel = new Model('User', UserSchema);
```

## Type Parameters

| Parameter                | Description                                                       |
|--------------------------|-------------------------------------------------------------------|
| `S extends Schema = any` | The schema type that defines the structure of the model's records |

## Constructor

```typescript
constructor(modelName: string, schema: S)
```

Creates a new `Model` instance.

### Parameters

| Parameter        | Type             | Description                                                                |
|------------------|------------------|----------------------------------------------------------------------------|
| `modelName`      | `string`         | The name/label of the model in the database                                |
| `schema`         | `S`              | The schema definition that describes the model's structure                 |

## Properties

### label

```typescript
public readonly label: string
```

The name/label of the model in the database.

### schema

```typescript
public readonly schema: S
```

The schema definition that describes the model's structure.

### draft

```typescript
readonly draft!: InferType<Model<S>>
```

Type helper for a draft version of the schema. Represents a flat object containing only the record's own properties (defined by the schema), excluding system fields such as `__id`, `__label`, and `__proptypes`. This type does not yet have a representation on the database side.

### record

```typescript
readonly record!: DBRecord<S>
```

Type helper for a fully-defined record with database representation. Similar to the draft, but includes all fields that come with the record's database-side representation, such as `__id`, `__label`, and `__proptypes`.

### searchQuery
```typescript
readonly searchQuery!: SearchQuery<S>
```

Type helper for a SearchQuery of the schema. Represents a structured query input that enables filtering, sorting, pagination, and aggregation of records based on schema-defined fields. Useful for composing reusable, type-safe search expressions.


### recordInstance

```typescript
readonly recordInstance!: DBRecordInstance<S>
```

Type helper for a single record instance. Extends the record by providing additional methods to operate on this specific record, such as saving, updating, or deleting it.

### recordsArrayInstance

```typescript
readonly recordsArrayInstance!: DBRecordsArrayInstance<S>
```

Type helper for an array of record instances. Similar to a single record instance but supports batch or bulk operations, allowing efficient management of multiple records simultaneously.

### Type-only properties on Model

These helpers are available for typing only and are not populated at runtime. They exist to let you derive precise types from a model without constructing values in code.

Properties that are type-only:
- `draft`
- `record`
- `recordInstance`
- `recordsArrayInstance`
- `searchQuery`

Practical usage examples:

```ts
// DO: derive types using typeof
export type UserRecord = typeof UserModel.record;
export type UserRecordResult<T extends Record<PropertyKey, any> = never> =
  typeof UserModel.recordInstance & { data: T };
export type UserRecordsArrayResult = typeof UserModel.recordsArrayInstance;
export type UserRecordDraft = typeof UserModel.draft;
export type UserSearchQuery = SearchQuery<typeof UserModel.schema>;

// DON'T: read them at runtime — they aren’t real values
// console.log(UserModel.record) // undefined at runtime
```

If you need an actual instance at runtime, work with query results (e.g., `find`, `findOne`, `create`) or convert a raw record via `toDBRecordInstance`.

## Methods

### getLabel()

```typescript
public getLabel(): string
```

Retrieves the model's label.

**Returns**: The label/name of the model

### toInstance()

```typescript
public toInstance(record: DBRecord<S>): DBRecordInstance<S>
```

Converts a database record to a record instance with additional methods.

**Parameters**:
- `record`: The database record to convert

**Returns**: A record instance with additional methods

**Throws**: Error if no RushDB instance was provided during initialization

### find()

```typescript
async find<Q extends SearchQuery<S> = SearchQuery<S>>(
  searchQuery?: Q & { labels?: never },
  transaction?: Transaction | string
): Promise<DBRecordsArrayInstance<S, Q>>
```

Finds records that match the given search criteria.

**Parameters**:
- `searchQuery`: Optional search criteria
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to matching records

### findOne()

```typescript
async findOne<Q extends SearchQuery<S> = SearchQuery<S>>(
  searchQuery?: Q & {
    labels?: never
    limit?: never
    skip?: never
  },
  transaction?: Transaction | string
): Promise<DBRecordInstance<S, Q> | null>
```

Finds a single record that matches the given search criteria.

**Parameters**:
- `searchQuery`: Optional search criteria
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the matching record or null if not found

### findById()

```typescript
async findById(
  id: string,
  transaction?: Transaction | string
): Promise<DBRecordInstance<S> | null>
```

Finds a record by its ID.

**Parameters**:
- `id`: The ID of the record to find
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the matching record or null if not found

### findUniq()

```typescript
async findUniq<Q extends SearchQuery<S> = SearchQuery<S>>(
  searchQuery?: Q & {
    labels?: never
    limit?: never
    skip?: never
  },
  transaction?: Transaction | string
): Promise<DBRecordInstance<S, Q> | null>
```

Finds a unique record that matches the given search criteria.

**Parameters**:
- `searchQuery`: Optional search criteria
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the unique matching record or null if not found

### create()

```typescript
async create(
  record: InferSchemaTypesWrite<S>,
  transaction?: Transaction | string
): Promise<DBRecordInstance<S>>
```

Creates a new record in the database.

**Parameters**:
- `record`: The record data to create
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the created record

**Throws**: UniquenessError if the record violates uniqueness constraints

### attach()

```typescript
attach(
  {
    source,
    target,
    options
  }: {
    source: DBRecordTarget
    target: RelationTarget
    options?: RelationOptions
  },
  transaction?: Transaction | string
): Promise<any>
```

Attaches a relationship between records.

**Parameters**:
- `params`: Object containing source, target, and relationship options
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the result of the attach operation

## Type Definitions

### InferType

```typescript
export type InferType<M extends Model<any> = Model<any>> = FlattenTypes<InferSchemaTypesRead<M['schema']>>
```

Helper type that infers the type structure from a Model instance.

**Type Parameters**:
- `M`: The Model type to infer from

## TypeScript: extend SDK types for schema-aware suggestions

To get first-class IDE autocomplete and end-to-end type safety that respects your model schemas (including nested related-queries), augment the SDK’s empty `Models` interface with your app’s models.

### 1) Define your models and export a Models map

Create a file like `types.ts` in your app and export a mapping of label → schema, plus any handy derived types:

```ts
// types.ts

import { SearchQuery, Model } from '@rushdb/javascript-sdk';

// Optional utility in your app
import { getCurrentISO } from '@/common/utils/getCurrentISO';

export const USER = 'USER' as const;

export const UserModel = new Model(USER, {
  name: { type: 'string' },
  avatar: { type: 'string' },
  login: { type: 'string', unique: true },
  password: { type: 'string' },
  createdAt: { type: 'datetime', default: getCurrentISO },
  tags: { type: 'string', multiple: true, required: false },
});

export type UserRecord = typeof UserModel.record;
export type UserRecordResult<T extends Record<PropertyKey, any> = never> =
  typeof UserModel.recordInstance & { data: T };
export type UserRecordsArrayResult = typeof UserModel.recordsArrayInstance;
export type UserRecordDraft = typeof UserModel.draft;
export type UserSearchQuery = SearchQuery<typeof UserModel.schema>;

// This map is used to teach the SDK about your labels and their schemas
export type Models = {
  [USER]: typeof UserModel.schema;
};
```

Notes:
- The property for uniqueness is `unique` in the SDK.
- Adjust any import aliases like `@/common/...` to match your project setup.

### 2) Augment the SDK’s Models interface

Create a project-level declaration file (for example `index.d.ts`) that merges your `Models` into the SDK. Make sure this file is included by your tsconfig (see next section):

```ts
// index.d.ts (path can be anything, just ensure tsconfig includes it)
import {} from '@rushdb/javascript-sdk';

// Import the map you exported in step 1 (adjust the path for your project)
import { Models as AppModels } from '@/platform-app/types';

declare module '@rushdb/javascript-sdk' {
  // This merges your labels/schemas so SearchQuery<Related> gets rich intellisense
  export interface Models extends AppModels {}
}
```

After this merge, places that use `Related<Models>` (e.g., `where` clauses with nested related objects) will autocomplete and type-check against your actual fields.

### 3) TS config tips for great DX

Ensure TypeScript can find your declaration file and resolve any path aliases used in your imports.

Minimal tsconfig settings to verify:

```json
{
  "compilerOptions": {
    "strict": true,
    "module": "esnext",
    "target": "es2020",
    "moduleResolution": "bundler", // or "node"/"node16" depending on your toolchain
    "baseUrl": ".",                 // if you use path aliases
    "paths": {
      "@/*": ["src/*"]              // adjust to match your alias usage like '@/...'
    },
    // Keep this true if you want to validate library .d.ts files too
    // "skipLibCheck": false
  },
  "include": [
    "src",
    "index.d.ts"      // include your augmentation (or its actual path)
  ]
}
```

Common gotchas:
- If the module augmentation isn’t picked up, double-check that the `.d.ts` file is inside tsconfig’s `include` (or not excluded).
- Monorepo/workspaces: put the augmentation in the consuming app/package and ensure that package’s tsconfig includes it.
- If you use ESM + TS 5’s “verbatimModuleSyntax”, keep imports as type-only where needed.

### 4) Example of typed related queries

With the augmentation in place, related objects in `where` become schema-aware:

```ts
const posts = await db.records.find({
  labels: ['post'],
  where: {
    author: {
      login: { $contains: 'john' },      // autocomplete from Author schema
    },
  },
});
```

This setup gives you end-to-end safety: drafts, records, queries, and nested relations all derive types from your single source of truth—the model schemas you define.
