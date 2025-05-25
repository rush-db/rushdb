---
sidebar_position: 3
---

# Properties

[Properties](../concepts/properties.md) are the individual key-value pairs that make up the data within a [record](../concepts/records.md) in RushDB. This guide covers how to work with properties using the TypeScript SDK, including finding, retrieving, and managing property values.

## Overview

The properties API in the SDK enables you to:
- Find properties based on search criteria
- Retrieve specific properties by ID
- Get possible values for a property
- Delete properties from the database

## Finding Properties

### Using RushDB's `find()` Method

To search for properties that match specific criteria, use the `properties.find` method:

```typescript
const properties = await db.properties.find({
  where: {
    name: 'email',
    type: 'string'
  }
});

console.log(properties);
/*
{
  data: [
    {
      id: 'property_id_1',
      name: 'email',
      type: 'string',
      ...
    },
    {
      id: 'property_id_2',
      name: 'email',
      type: 'string',
      ...
    }
  ],
  total: 2
}
*/
```

#### Parameters

- `searchQuery`: A search query object to find matching properties
  - `where`: Conditions to filter properties
  - `sort`: Sort criteria for results
  - `limit`: Maximum number of results to return
  - `skip`: Number of results to skip
- `transaction` (optional): A [transaction](../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to an array of property objects

### Finding Properties in Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const properties = await db.properties.find({
    where: {
      name: { $in: ['email', 'phone'] }
    }
  }, transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(properties);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

## Retrieving a Property by ID

### Using RushDB's `findById()` Method

To retrieve a specific property by its ID, use the `properties.findById` method:

```typescript
const property = await db.properties.findById('property_id_1');

console.log(property);
/*
{
  id: 'property_id_1',
  name: 'email',
  type: 'string',
  ...
}
*/
```

#### Parameters

- `id`: The ID of the property to retrieve
- `transaction` (optional): A [transaction](../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to the property object if found, or null if not found

## Getting Property Values

### Using RushDB's `values()` Method

To retrieve possible values for a specific property, use the `properties.values` method:

```typescript
const values = await db.properties.values('property_id_1', {
  where: {
    status: 'active'
  },
  query: 'john',
  orderBy: 'asc',
  limit: 10
});

console.log(values);
/*
{
  data: ['john@example.com', 'johnny@example.com'],
  total: 2
}
*/
```

#### Parameters

- `id`: The ID of the property to get values for
- `searchQuery` (optional): SearchQuery object with filtering options:
  - `where` (object): Filter criteria for records containing this property
  - `query` (string): Filter values by this text string
  - `orderBy` (string): Sort direction ('asc' or 'desc')
  - `limit` (number): Maximum number of values to return
  - `skip` (number): Number of values to skip for pagination
- `transaction` (optional): A [transaction](../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to an object containing the values and a total count

## Deleting Properties

### Using RushDB's `delete()` Method

To delete a property from the database, use the `properties.delete` method:

```typescript
const result = await db.properties.delete('property_id_1');

console.log(result);
/*
{
  success: true,
  message: "Property deleted successfully"
}
*/
```

#### Parameters

- `id`: The ID of the property to delete
- `transaction` (optional): A [transaction](../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a success object

#### Deleting Properties in Transactions

```typescript
const transaction = await db.tx.begin();
try {
  await db.properties.delete('property_id_1', transaction);

  // Perform other operations...

  await transaction.commit();
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

## Best Practices for Working with Properties

1. **Use Transactions for Related Operations**
   - When performing multiple operations that need to be atomic, use [transactions](../concepts/transactions.mdx)
   - This ensures data consistency and prevents partial changes

2. **Optimize Search Queries**
   - Use specific search criteria to minimize the amount of data returned
   - Filter by name, type, or other attributes to narrow down results

3. **Cache Property IDs When Appropriate**
   - If you frequently access the same properties, cache their IDs
   - This reduces the need for repeated lookups

4. **Consider the Impact of Property Deletion**
   - Deleting a property affects all records that use it
   - Instead of deleting common properties, consider marking them as deprecated

5. **Use Distinct Values for Enumeration**
   - When fetching property values for UI dropdown elements, use the `distinct: true` option
   - This provides a cleaner list of possible values without duplicates

## Conclusion

The Properties API in the RushDB TypeScript SDK provides a comprehensive set of methods for working with properties. By understanding these methods and their parameters, you can effectively manage properties in your application.

For more information on related topics, see:
- [Records](./records/create-records.md) - Work with records that contain properties
- [Relationships](./relationships.md) - Connect records with relationships
- [Models](./models.md) - Define structured schemas for your data
