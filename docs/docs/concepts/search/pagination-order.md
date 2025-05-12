---
sidebar_position: 3
---

# Pagination and Order

SearchQuery provides flexible pagination and ordering capabilities to control the volume of returned data and the sequence in which records are presented.

## Overview

When querying data with the SearchQuery DTO, you can control:

- **Result Limit**: The maximum number of records to return
- **Offset**: The number of records to skip
- **Sorting Order**: How results should be sorted

These settings are defined at the top level in the SearchQuery DTO, alongside filtering conditions and other parameters:

```typescript
// SearchQuery
{
  labels: ['COMPANY'],      // Record labels to search
  where: { /* conditions */ },  // Filtering conditions
  limit: 100,               // Results limit (optional)
  skip: 0,                  // Results offset (optional)
  orderBy: { name: 'asc' }, // Sorting (optional)
  aggregate: { /* aggregations */ }  // Aggregations (optional)
}
```

## Pagination Parameters

### `limit`

- **Type**: `number`
- **Optional**: Yes
- **Default**: `100`
- **Valid Range**: `1` to `1000`
- **Description**: Specifies the maximum number of records to return in the result set.

### `skip`

- **Type**: `number`
- **Optional**: Yes
- **Default**: `0`
- **Description**: Specifies the number of records to skip before starting to return results. Useful for implementing paged access to large result sets.

## Sorting Records with `orderBy`

The `orderBy` parameter controls the order in which records are returned.

### Types of Sorting

You can specify sorting in two ways:

1. **Simple Direction Sorting**: Apply a sort direction to the default ID field
2. **Field-Specific Sorting**: Sort by specific fields with individual directions

### Sort Direction Values

- `asc`: Ascending order (A → Z, 0 → 9)
- `desc`: Descending order (Z → A, 9 → 0)

### Examples

#### Simple Direction Sorting

When using a string value for `orderBy`, the system sorts by the internal ID field:

```typescript
{
  labels: ['EMPLOYEE'],
  orderBy: 'asc',  // Sort by ID in ascending order
  limit: 50
}
```

#### Field-Specific Sorting

For sorting by specific fields, use an object with field names as keys and sort directions as values:

```typescript
{
  labels: ['EMPLOYEE'],
  orderBy: {
    salary: 'desc',  // Sort by salary in descending order
    name: 'asc'      // Then by name in ascending order (if salaries are equal)
  },
  limit: 50
}
```

## Pagination Examples

### Basic Pagination

```typescript
// First page (records 1-100)
{
  labels: ['COMPANY'],
  limit: 100,
  skip: 0
}

// Second page (records 101-200)
{
  labels: ['COMPANY'],
  limit: 100,
  skip: 100
}

// Third page (records 201-300)
{
  labels: ['COMPANY'],
  limit: 100,
  skip: 200
}
```

### Different Page Sizes

```typescript
// Get first 25 records
{
  labels: ['PRODUCT'],
  limit: 25,
  skip: 0
}

// Get 50 records starting from the 26th record
{
  labels: ['PRODUCT'],
  limit: 50,
  skip: 25
}
```

## Combined Pagination and Sorting

You can combine pagination and sorting to implement sophisticated data access patterns:

```typescript
// Get the top 10 highest-paid employees
{
  labels: ['EMPLOYEE'],
  orderBy: { salary: 'desc' },
  limit: 10,
  skip: 0
}

// Get the next 10 highest-paid employees
{
  labels: ['EMPLOYEE'],
  orderBy: { salary: 'desc' },
  limit: 10,
  skip: 10
}
```

## Default Behavior

- If `limit` is not specified, the default value is `100`
- If `limit` is greater than `1000`, it will be capped at `1000`
- If `skip` is not specified, the default value is `0`
- If `orderBy` is not specified, results are sorted by the internal ID field in descending order

## Performance Considerations

- For large datasets, combining high `skip` values with complex filtering conditions may impact performance
- Consider using filtering conditions that leverage indexes for better performance with larger offsets
- When possible, structure your application to use smaller page sizes (lower `limit` values)
- The maximum allowed `limit` value (1000) is designed to prevent excessive resource consumption