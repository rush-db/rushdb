---
sidebar_position: 3
---

# DBRecordInstance

`DBRecordInstance` is a class that wraps a DBRecord and provides methods for manipulating it. This class serves as an interface for working with individual [records](../../concepts/records) in the database. It allows for updating [properties](../../concepts/properties) and managing [relationships](../../concepts/relationships).

## Class Definition

```typescript
export class DBRecordInstance<
  S extends Schema = Schema,
  Q extends SearchQuery<S> = SearchQuery<S>
> extends RestApiProxy {
  data?: DBRecordInferred<S, Q>
}
```

## Type Parameters

| Parameter | Description |
|-----------|-------------|
| `S extends Schema = Schema` | The schema type that defines the structure of the record |
| `Q extends SearchQuery<S> = SearchQuery<S>` | The search query type used to retrieve this record |

## Constructor

```typescript
constructor(data?: DBRecordInferred<S, Q>)
```

Creates a new `DBRecordInstance`.

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `data` | `DBRecordInferred<S, Q>` | Optional data for the record |

## Properties

### data

```typescript
data?: DBRecordInferred<S, Q>
```

The actual record data, which may include aggregated fields if the record was retrieved via a query with aggregation.

## Methods

### id()

```typescript
id(): string
```

Gets the unique identifier of the record.

**Returns**: The ID of the record

**Throws**: Error if the record's ID is missing or incorrect

### label()

```typescript
label(): string
```

Gets the label/type of the record.

**Returns**: The label of the record

**Throws**: Error if the record's label is missing or incorrect

### proptypes()

```typescript
proptypes(): object | undefined
```

Gets the property types of the record.

**Returns**: The property types of the record or undefined if not available

**Throws**: Error if the record's proptypes are missing or incorrect

### date()

```typescript
date(): Date
```

Gets the date derived from the record's ID.

**Returns**: The date from the record's ID

**Throws**: Error if the record's ID is missing or incorrect

### timestamp()

```typescript
timestamp(): number
```

Gets the timestamp derived from the record's ID.

**Returns**: The timestamp from the record's ID

**Throws**: Error if the record's ID is missing or incorrect

### delete()

```typescript
async delete(transaction?: Transaction | string): Promise<any>
```

Deletes the record from the database.

**Parameters**:
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the result of the delete operation

**Throws**: Error if the record data is undefined

### update()

```typescript
async update<S extends Schema = Schema>(
  data: Partial<InferSchemaTypesWrite<S>> | Array<PropertyDraft>,
  transaction?: Transaction | string
): Promise<any>
```

Updates the record with the given data.

**Parameters**:
- `data`: The data to update the record with
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the result of the update operation

**Throws**: Error if the record data is undefined

### set()

```typescript
async set<S extends Schema = Schema>(
  data: InferSchemaTypesWrite<S> | Array<PropertyDraft>,
  transaction?: Transaction | string
): Promise<any>
```

Replaces the record's data with the given data.

**Parameters**:
- `data`: The data to set on the record
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the result of the set operation

**Throws**: Error if the record data is undefined

### attach()

```typescript
async attach(
  target: RelationTarget,
  options?: RelationOptions,
  transaction?: Transaction | string
): Promise<any>
```

Creates a relationship from this record to the target record(s).

**Parameters**:
- `target`: The target record(s) to create a relationship to
- `options`: Optional relationship options
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the result of the attach operation

**Throws**: Error if the record data is undefined

### detach()

```typescript
async detach(
  target: RelationTarget,
  options?: RelationDetachOptions,
  transaction?: Transaction | string
): Promise<any>
```

Removes a relationship from this record to the target record(s).

**Parameters**:
- `target`: The target record(s) to remove a relationship from
- `options`: Optional relationship detach options
- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to the result of the detach operation

**Throws**: Error if the record data is undefined

## Usage Example

```typescript
// Get a record instance from a model
const userRecord = await UserModel.findById('user_123');

// Access record data
console.log(userRecord.id()); // 'user_123'
console.log(userRecord.label()); // 'User'
console.log(userRecord.data?.name); // 'John Doe'

// Update the record
await userRecord.update({
  name: 'Jane Doe',
  age: 31
});

// Create a relationship to another record
const postRecord = await PostModel.findById('post_456');
await userRecord.attach(postRecord, { type: 'AUTHORED' });

// Delete the record
await userRecord.delete();
```
