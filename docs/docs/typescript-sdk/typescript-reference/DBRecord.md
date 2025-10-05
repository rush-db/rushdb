---
sidebar_position: 2
---

# DBRecord

`DBRecord` is a type representing a database [record](../../concepts/records) in RushDB. It combines internal system properties with schema-defined data [properties](../../concepts/properties).

## Type Definition

```typescript
export type DBRecord<S extends Schema = Schema> = FlattenTypes<
  DBRecordInternalProps<S> & FlattenTypes<RecordProps<S>>
>
```

## Type Parameters

| Parameter | Description |
|-----------|-------------|
| `S extends Schema = Schema` | The schema type that defines the structure of the record |

## Properties

A `DBRecord` consists of both system properties and schema-defined properties:

### System Properties

| Property | Type | Description |
|----------|------|-------------|
| `__id` | `string` | The unique identifier of the record |
| `__label` | `string` | The label/type of the record |
| `__proptypes` | `object` | Property type information for the record |

### Schema Properties

The remaining properties are defined by the schema `S` that was used to create the record. These properties can be:

- Required properties (as defined in the schema with `required: true`)
- Optional properties (as defined in the schema without `required: true` or with `required: false`)

## Related Types

### DBRecordInternalProps

```typescript
type DBRecordInternalProps<S extends Schema = Schema> = {
  readonly __id: string
  readonly __label: string
  readonly __proptypes?: FlattenTypes<
    {
      [Key in RequiredKeysRead<S>]: S[Key]['type']
    } & {
      [Key in OptionalKeysRead<S>]?: S[Key]['type']
    }
  >
}
```

Contains the internal system properties of a database record.

### RecordProps

```typescript
export type RecordProps<S extends Schema = Schema> =
  S extends S ? InferSchemaTypesRead<S>
  : {
      [K in keyof S]?: S[K]
    }
```

Contains the schema-defined properties of a database record.

### DBRecordInferred

```typescript
export type DBRecordInferred<S extends Schema, Q extends SearchQuery<S>> =
  Q extends { aggregate: infer A extends Record<string, any> } ? DBRecord<S> & ExtractAggregateFields<A>
  : DBRecord<S>
```

An extension of `DBRecord` that includes any aggregated fields from a search query.

## Usage Example

```typescript
// Define a schema
const UserSchema = {
  name: { type: 'string', required: true },
  email: { type: 'string', required: true, unique: true },
  age: { type: 'number' }
};

// A record matching this schema would have type:
type UserRecord = DBRecord<typeof UserSchema>;

// Example of what the record would look like:
const user: UserRecord = {
  __id: 'user_123',
  __label: 'User',
  __proptypes: { name: 'string', email: 'string', age: 'number' },
  name: 'John Doe',
  email: 'john@example.com',
  age: 30
};
```
