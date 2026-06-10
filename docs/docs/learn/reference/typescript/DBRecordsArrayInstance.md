---
sidebar_position: 5
---

# DBRecordsArrayInstance

`DBRecordsArrayInstance` is a class that manages an array of `DBRecordInstance` objects. It typically represents the result of a [search query](/learn/search-query) that returns multiple [records](/learn/records-and-queries/store-records).

## Class Definition

```typescript
export class DBRecordsArrayInstance<S extends Schema = Schema, Q extends SearchQuery<S> = SearchQuery<S>> {
  data?: Array<DBRecordInstance<S, Q>>
  total: number | undefined
  searchQuery?: SearchQuery<S>
}
```

## Type Parameters

| Parameter                                   | Description                                               |
| ------------------------------------------- | --------------------------------------------------------- |
| `S extends Schema = Schema`                 | The schema type that defines the structure of the records |
| `Q extends SearchQuery<S> = SearchQuery<S>` | The search query type used to retrieve these records      |

## Constructor

```typescript
constructor(
  data?: Array<DBRecordInstance<S, Q>>,
  total?: number,
  searchQuery?: SearchQuery<S>
)
```

Creates a new `DBRecordsArrayInstance`.

### Parameters

| Parameter     | Type                            | Description                                                                                        |
| ------------- | ------------------------------- | -------------------------------------------------------------------------------------------------- |
| `data`        | `Array<DBRecordInstance<S, Q>>` | Optional array of record instances                                                                 |
| `total`       | `number`                        | Optional total count of records (may be greater than the length of `data` when pagination is used) |
| `searchQuery` | `SearchQuery<S>`                | Optional search query that was used to retrieve these records                                      |

## Properties

### data

```typescript
data?: Array<DBRecordInstance<S, Q>>
```

An array of record instances.

### total

```typescript
total: number | undefined
```

The total number of records that match the search query, which may be greater than the number of records in `data` if pagination is used.

### searchQuery

```typescript
searchQuery?: SearchQuery<S>
```

The search query that was used to retrieve these records.

## Methods

### deleteAll()

```typescript
async deleteAll(transaction?: Transaction | string): Promise<{ success: boolean }>
```

Deletes all records in this result set.

**Parameters**:

- `transaction`: Optional transaction or transaction ID

**Returns**: Promise resolving to `{ success: boolean }`

### next()

```typescript
async next(options?: { preserveData?: boolean }): Promise<DBRecordsArrayInstance<S, Q>>
```

Fetches the next page of results based on the original search query.

**Parameters**:

- `options.preserveData`: If `true`, appends new results to the existing `data` array and returns the same instance. If `false` (default), returns a new `DBRecordsArrayInstance` with the next page.

**Returns**: Promise resolving to a `DBRecordsArrayInstance` with the next page

**Throws**: Error if no `searchQuery` was stored on this instance

### exportCsv()

```typescript
exportCsv(): string
```

Exports the records to a CSV string. Headers are derived from the first record's property keys (system keys like `__id`, `__label`, `__proptypes` are excluded).

**Returns**: CSV string (empty string if no records)

### setProperties()

```typescript
async setProperties(
  patch: Partial<InferSchemaTypesWrite<S>>,
  transaction?: Transaction | string
): Promise<void>
```

Updates properties across all records in this result set. Records are processed in batches of 100.

**Parameters**:

- `patch`: The fields to update and their new values
- `transaction`: Optional transaction or transaction ID

## Usage Example

```typescript
// Get multiple records from a model
const userRecords = await UserModel.find({
  where: {
    age: { $gt: 30 }
  },
  limit: 10
})

// Access the records
console.log(userRecords.total) // Total number of matching records
console.log(userRecords.data?.length) // Number of records in this page (max 10)

// Access individual record instances
userRecords.data?.forEach((user) => {
  console.log(user.id, user.data?.name)
})

// Fetch next page
const nextPage = await userRecords.next()

// Or append next page to existing results
await userRecords.next({ preserveData: true })
console.log(userRecords.data.length) // Now contains up to 20 records

// Export to CSV
const csv = userRecords.exportCsv()

// Bulk update a property
await userRecords.setProperties({ active: false })

// Delete all found records
await userRecords.deleteAll()
```
