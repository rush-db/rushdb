---
sidebar_position: 5
---

# DBRecordsArrayInstance

`DBRecordsArrayInstance` is a class that manages an array of `DBRecordInstance` objects. It typically represents the result of a [search query](/concepts/search) that returns multiple [records](/concepts/records).

## Class Definition

```typescript
export class DBRecordsArrayInstance<
  S extends Schema = Schema,
  Q extends SearchQuery<S> = SearchQuery<S>
> extends RestApiProxy {
  data?: Array<DBRecordInstance<S, Q>>
  total: number | undefined
  searchQuery?: SearchQuery<S>
}
```

## Type Parameters

| Parameter | Description |
|-----------|-------------|
| `S extends Schema = Schema` | The schema type that defines the structure of the records |
| `Q extends SearchQuery<S> = SearchQuery<S>` | The search query type used to retrieve these records |

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

| Parameter | Type | Description |
|-----------|------|-------------|
| `data` | `Array<DBRecordInstance<S, Q>>` | Optional array of record instances |
| `total` | `number` | Optional total count of records (may be greater than the length of `data` when pagination is used) |
| `searchQuery` | `SearchQuery<S>` | Optional search query that was used to retrieve these records |

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

Currently, the `DBRecordsArrayInstance` class has no dedicated methods implemented. According to code comments, future plans may include:

```typescript
// @TODO: Bulk actions: Delete (by ids or searchQuery?); Export to csv; Props update for found Records; Attach/Detach
// @TODO: Create next({preserveData?: boolean}) method (or smth similar) to fetch next portion of data based on this.searchQuery
```

## Usage Example

```typescript
// Get multiple records from a model
const userRecords = await UserModel.find({
  where: {
    age: { $gt: 30 }
  },
  limit: 10
});

// Access the records
console.log(userRecords.total); // Total number of matching records
console.log(userRecords.data?.length); // Number of records in this page (max 10)

// Access individual record instances
userRecords.data?.forEach(user => {
  console.log(user.id(), user.data?.name);
});

// Access the original search query
console.log(userRecords.searchQuery);
```

## Future Enhancements

According to the code comments, future enhancements may include:

1. Bulk actions for operations like delete, export to CSV, property updates, and relationship management (attach/detach)
2. Pagination methods to fetch the next set of records based on the original search query
