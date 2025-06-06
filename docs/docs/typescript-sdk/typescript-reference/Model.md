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
  email: { type: 'string', required: true, uniq: true },
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
