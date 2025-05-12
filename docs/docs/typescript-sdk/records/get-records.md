---
sidebar_position: 5
---

# Get  Records

RushDB provides flexible TypeScript SDK methods for retrieving records from your database. The Search API is one of the most powerful features of RushDB, allowing you to find records, navigate relationships, and transform results to exactly match your application's needs.

## Overview

The record retrieval and search methods in the SDK enable you to:
- Get a single record by its ID
- Find a single record that matches specific criteria
- Find records that match complex queries with filtering, sorting, and pagination
- Traverse relationships between records
- Perform vector similarity searches
- Retrieve records with related data
- Transform and aggregate search results

## Get Single Records

RushDB provides several methods for retrieving individual records, whether you know their ID or need to find them using search criteria.

### Get a Record by ID with `findById()`

When you already know the unique identifier of the record you need:

```typescript
// Get a single record by ID
const user = await db.records.findById('user-123');

// Get multiple records by their IDs
const users = await db.records.findById(['user-123', 'user-456', 'user-789']);
```

This method retrieves one or more records identified by their unique IDs.

#### Parameters

| Parameter | Type   | Description |
|-----------|--------|-------------|
| `idOrIds` | `String` or `Array<String>` | The unique identifier(s) of the record(s) to retrieve |
| `transaction` | `Transaction` or `String` | Optional transaction for atomic operations |

#### Examples

```typescript
// Retrieve a single record
try {
  const person = await db.records.findById('018e4c71-5f20-7db2-b0b1-e7e681542af9');
  console.log(`Found ${person.label()} with name: ${person.data.name}`);
} catch (error) {
  console.error('Failed to retrieve record:', error);
}

// Retrieve multiple records by ID
try {
  const employees = await db.records.findById([
    '018e4c71-5f20-7db2-b0b1-e7e681542af9',
    '018e4c71-5f20-7db2-b0b1-e7e681542af8'
  ]);
  console.log(`Found ${employees.data.length} records`);
} catch (error) {
  console.error('Failed to retrieve records:', error);
}

// Using with a transaction
const tx = await db.tx.begin();
try {
  const record = await db.records.findById('018e4c71-5f20-7db2-b0b1-e7e681542af9', tx);
  // Use the record
  await tx.commit();
} catch (error) {
  await tx.rollback();
  console.error('Transaction failed:', error);
}
```

### Find a Single Record with `findOne()`

When you need to find a record that matches specific criteria:

```typescript
const user = await db.records.findOne({
  labels: ["USER"],
  where: {
    email: "jane@example.com"
  }
});
```

This method returns a single record that matches your query parameters, or null if no match is found.

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `searchQuery` | `SearchQuery` | Query object with filters to match records |
| `transaction` | `Transaction` or `String` | Optional transaction for atomic operations |

#### Example

```typescript
try {
  const user = await db.records.findOne({
    labels: ["USER"],
    where: {
      email: "jane@example.com"
    }
  });

  if (user.data) {
    console.log(`Found user: ${user.data.name}`);
  } else {
    console.log("User not found");
  }
} catch (error) {
  console.error('Error searching for user:', error);
}
```

### Find a Unique Record with `findUniq()`

When you expect exactly one matching record and want to ensure uniqueness:

```typescript
try {
  const user = await db.records.findUniq({
    labels: ["USER"],
    where: {
      email: "jane@example.com"  // Assuming email is a unique field
    }
  });
} catch (error) {
  if (error instanceof NonUniqueResultError) {
    console.error(`Expected one result but found multiple matches`);
  } else {
    console.error('Error searching for user:', error);
  }
}
```

This method throws a `NonUniqueResultError` if more than one record matches your criteria. This is useful when querying fields that should be unique.

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `searchQuery` | `SearchQuery` | Query object with filters to match records |
| `transaction` | `Transaction` or `String` | Optional transaction for atomic operations |

#### Example with Error Handling

```typescript
try {
  const user = await db.records.findUniq({
    labels: ["USER"],
    where: {
      email: "jane@example.com"
    }
  });

  if (user.data) {
    console.log(`Found unique user: ${user.data.name}`);
  }
} catch (error) {
  if (error instanceof NonUniqueResultError) {
    console.error(`Expected one result but found ${error.count} matches`);
  } else {
    console.error('Error searching for user:', error);
  }
}
```

## Search for Multiple Records

### Basic Searching with `find()`

The most versatile search method is `find()`, which accepts a SearchQuery object to filter, sort, and paginate results.

```typescript
// Basic search for records with the "USER" label
const result = await db.records.find({
  labels: ["USER"],
  where: {
    isActive: true
  },
  limit: 10,
  orderBy: { createdAt: "desc" }
});

// Access the returned records
const users = result.data;
console.log(`Found ${result.total} total users`);
```

This method searches for records that match the specified criteria, with support for filtering, pagination, and sorting.

### Parameters

| Field     | Type   | Description |
|-----------|--------|-------------|
| `where`   | Object | Filter conditions for records ([learn more](/concepts/search/where)) |
| `orderBy` | String or Object | Sorting criteria ([learn more](/concepts/search/pagination-order)) |
| `skip`    | Number | Number of records to skip for pagination ([learn more](/concepts/search/pagination-order)) |
| `limit`   | Number | Maximum number of records to return (default: 1000) |
| `labels`  | Array  | Optional array of labels to filter records by ([learn more](/concepts/search/labels)) |

### Return Value

The find method returns an object containing:
- `data`: An array of record instances matching the query
- `total`: The total number of records that match the query (before pagination)

### Examples

**Basic Search**

```typescript
// Find all active users sorted by name
const result = await db.records.find({
  where: {
    isActive: true
  },
  labels: ["USER"],
  orderBy: { name: 'asc' },
  limit: 50
});

console.log(`Found ${result.total} active users, showing first ${result.data.length}`);
```

**Advanced Filtering**

```typescript
// Find products with specific criteria
const results = await db.records.find({
  labels: ["PRODUCT"],
  where: {
    $or: [
      { status: 'in_stock', price: { $lt: 100 } },
      { status: 'pre_order', releaseDate: { $lt: '2025-06-01' } }
    ]
  },
  orderBy: [
    { popularity: 'desc' },
    { price: 'asc' }
  ],
  limit: 20
});
```

Search queries support a powerful and flexible syntax for filtering records. For a detailed explanation of all the available operators and capabilities, see the [Where clause documentation](/concepts/search/where).

## Advanced Search Features

### Relationship Traversal

One of RushDB's most powerful features is the ability to search across relationships between records:

```typescript
// Find all blog posts by users who work at tech companies
const techBloggers = await db.records.find({
  labels: ["POST"],
  where: {
    USER: {                            // Traverse to related USER records
      COMPANY: {                       // Traverse to related COMPANY records
        industry: "Technology"
      }
    },
    publishedAt: { $lte: new Date() }  // Only published posts
  },
  orderBy: { publishedAt: "desc" },
  limit: 20
});
```

For more complex relationship queries, you can specify relationship types and directions:

```typescript
// Find users who follow specific topics
const users = await db.records.find({
  labels: ["USER"],
  where: {
    TOPIC: {
      $relation: {
        type: "FOLLOWS",
        direction: "out"       // User -> FOLLOWS -> Topic
      },
      name: { $in: ["TypeScript", "GraphDB", "RushDB"] }
    }
  }
});
```

See the [Where clause documentation](/concepts/search/where#relationship-queries) for more details on relationship queries.

### Vector Search

RushDB supports vector similarity searches for AI and machine learning applications:

```typescript
// Find documents similar to a query embedding
const similarDocuments = await db.records.find({
  labels: ["DOCUMENT"],
  where: {
    embedding: {
      $vector: {
        fn: "gds.similarity.cosine",      // Similarity function
        query: queryEmbedding,            // Your vector embedding
        threshold: { $gte: 0.75 }         // Minimum similarity threshold
      }
    }
  },
  limit: 10
});
```

See the [Vector operators documentation](/concepts/search/where#vector-operators) for more details on vector search capabilities.

### Pagination and Sorting

Control the order and volume of results:

```typescript
// Get the second page of results (20 items per page)
const page2 = await db.records.find({
  labels: ["PRODUCT"],
  where: {
    category: "Electronics"
  },
  skip: 20,      // Skip the first 20 results
  limit: 20,     // Return 20 results
  orderBy: {
    price: "asc" // Sort by price ascending
  }
});

// Get total number of results for pagination UI
const totalProducts = page2.total;
```

For more details on pagination and sorting options, see the [Pagination and ordering documentation](/concepts/search/pagination-order).

### Aggregations

Transform and aggregate your search results:

```typescript
// Calculate comapany statis by employees and salaries
const companySalaryStats = await db.records.find({
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee',  // Define alias for employee records
      salary: {
        $gte: 50000  // Filter employees by salary
      }
    }
  },
  aggregate: {
    // Use field directly from record
    companyName: '$record.name',

    // Count unique employees using the defined alias
    employeesCount: {
      fn: 'count',
      uniq: true,
      alias: '$employee'
    },

    // Calculate total salary using the defined alias
    totalWage: {
      fn: 'sum',
      field: 'salary',
      alias: '$employee'
    },

    // Collect unique employees names
    employeeNames: {
      fn: 'collect',
      field: 'name',
      alias: '$employee'
    },

    // Get average salary with precision
    avgSalary: {
      fn: 'avg',
      field: 'salary',
      alias: '$employee',
      precision: 0
    },

    // Get min and max salary
    minSalary: {
      fn: 'min',
      field: 'salary',
      alias: '$employee'
    },
    maxSalary: {
      fn: 'max',
      field: 'salary',
      alias: '$employee'
    }
  }
});
```

For comprehensive details on available aggregation functions and usage, see the [Aggregations documentation](/concepts/search/aggregations).

## Model-Based Search

If you're using RushDB's Model system (recommended), you get the same powerful search capabilities with additional type safety and convenience.

### Searching with Models

Models provide type-safe search methods that understand your data structure:

```typescript
// Define your model
const UserModel = new Model('USER', {
  email: { type: 'string', unique: true },
  name: { type: 'string' },
  age: { type: 'number' },
  isActive: { type: 'boolean', default: true }
});

// Search using the model
const activeUsers = await UserModel.find({
  where: {
    age: { $gte: 21 },
    isActive: true
  },
  orderBy: { name: "asc" }
});

// TypeScript provides full type safety for your results
const firstUser = activeUsers.data[0];
const userName: string = firstUser.name; // Correctly typed as string
```

### Model Search Methods

Models provide the same search methods as direct record search, but with label pre-filled:

```typescript
// Find all matching records
const users = await UserModel.find({
  where: { isActive: true }
});

// Find a single record
const jane = await UserModel.findOne({
  where: { email: "jane@example.com" }
});

// Find by ID
const user = await UserModel.findById("user-123");

// Find a unique record
const uniqueUser = await UserModel.findUniq({
  where: { email: "unique@example.com" }
});
```

Note that when using model search methods, you don't need to specify the `labels` field in the search query since it's automatically set to the model's label.

For more details on models and type safety, see:
- [Models documentation](/typescript-sdk/records/models)
- [Advanced TypeScript usage](/typescript-sdk/advanced-typescript)

## Search Within Transactions

All search operations can be performed within transactions for consistency:

```typescript
// Begin a transaction
const tx = await db.tx.begin();

try {
  // Perform search within the transaction
  const users = await db.records.find({
    labels: ["USER"],
    where: { isActive: true }
  }, tx);

  // Use the results to make changes
  for (const user of users.data) {
    if (user.data.lastLogin < olderThan3Months) {
      await user.update({ isActive: false }, tx);
    }
  }

  // Commit the transaction when done
  await tx.commit();
} catch (error) {
  // Roll back the transaction on error
  await tx.rollback();
  throw error;
}
```

For more details on transactions, see the [Transactions documentation](/typescript-sdk/transactions).

## Performance Best Practices

When working with the Search API, follow these best practices for optimal performance:

1. **Be Specific with Labels**: Always specify labels to narrow the search scope.
2. **Use Indexed Properties**: Prioritize filtering on properties that have indexes.
3. **Limit Results**: Use pagination to retrieve only the records you need.
4. **Optimize Relationship Traversal**: Avoid deep relationship traversals when possible.
5. **Use Aliases Efficiently**: Define aliases only for records you need to reference in aggregations.
6. **Filter Early**: Apply filters as early as possible in relationship traversals to reduce the amount of data processed.

## Search Related Records

You can efficiently search for records that are related to a specific record using the entry point feature in search queries or direct relationship traversal.

```typescript
// Search for records related to a specific record
const relatedRecords = await db.records.find({
  id: 'source-record-id', // Starting from this record
  where: { /* search conditions */ }
});
```

This method searches for records that are directly related to a specific record, identified by its ID.

### Parameters

| Parameter  | Type   | Description |
|------------|--------|-------------|
| `id`       | String | The unique identifier of the source record |
| `where`    | Object | Filter conditions for records ([learn more](/concepts/search/where)) |
| `orderBy`  | String or Object | Sorting criteria (same as regular search) |
| `skip`     | Number | Number of records to skip for pagination |
| `limit`    | Number | Maximum number of records to return |

### Example

```typescript
// Find all documents associated with a specific person
const personId = '018e4c71-5f20-7db2-b0b1-e7e681542af9';
const result = await db.records.find({
  id: personId,
  labels: ['DOCUMENT'],
  where: {
    status: 'active'
  },
  orderBy: { createdAt: 'desc' }
});

console.log(`Found ${result.total} documents for this person`);
```

## Get Record Properties

```typescript
// Get properties of a specific record
const properties = await db.records.getProperties('record-id-here');
```

This method retrieves all properties of a specific record.

### Example

```typescript
const properties = await db.records.getProperties('018e4c71-5f20-7db2-b0b1-e7e681542af9');
console.log(properties);
// Output:
// [
//   { name: 'firstName', type: 'string', value: 'John' },
//   { name: 'lastName', type: 'string', value: 'Doe' },
//   { name: 'age', type: 'number', value: 30 }
// ]
```

## Get Record Relations

```typescript
// Get relationships of a specific record
const relationships = await db.records.getRelations('record-id-here', {
  skip: 0,
  limit: 20
});
```

This method retrieves the relationships of a specific record.

### Parameters

| Parameter  | Type   | Description |
|------------|--------|-------------|
| `id`       | String | The unique identifier of the record |
| `options`  | Object | Optional pagination parameters |

### Example

```typescript
const { data, total } = await db.records.getRelations('018e4c71-5f20-7db2-b0b1-e7e681542af9');

console.log(`This record has ${total} relationships`);
data.forEach(relation => {
  console.log(`Relation type: ${relation.type}`);
  console.log(`Target: ${relation.target.id} (${relation.target.label})`);
});
```

## Search Relations

```typescript
// Search for relationships
const relationships = await db.records.searchRelations({
  where: { /* search conditions */ }
});
```

This method searches for relationships between records based on specified criteria.

### Example

```typescript
// Find all employment relationships created in the last month
const lastMonth = new Date();
lastMonth.setMonth(lastMonth.getMonth() - 1);

const { data, total } = await db.records.searchRelations({
  where: {
    type: 'WORKS_AT',
    startDate: { $gte: lastMonth.toISOString() }
  }
});

console.log(`Found ${total} new employment relationships`);
```

## TypeScript Type Support

The RushDB SDK provides TypeScript types to enhance developer experience and type safety:

```typescript
import {
  SearchQuery,
  DBRecord,
  DBRecordInstance,
  DBRecordsArrayInstance,
  Schema,
  PropertyDefinition,
  Relation
} from '@rushdb/javascript-sdk';

// Define a schema type for better type checking
type UserSchema = {
  name: { type: 'string' };
  age: { type: 'number' };
  email: { type: 'string', unique: true };
  isActive: { type: 'boolean', default: true };
};

// Strongly-typed search
const query: SearchQuery<UserSchema> = {
  where: {
    age: { $gt: 21 },
    isActive: true
  }
};

// Type-safe result handling
const result = await db.records.find<UserSchema>(query);

// Working with typed data
result.data.forEach((record) => {
  // TypeScript knows that name is a string, age is a number, etc.
  console.log(`${record.data.name} (${record.data.age}): ${record.data.email}`);
});
```

## Performance Considerations

To optimize your record retrieval and search operations:

- **Use Appropriate Methods**: Choose the right method for your needs (`findById` for known IDs, `find` for searches)
- **Specify Labels**: Always include label filters to limit the search scope
- **Use Appropriate Limits**: Set reasonable `limit` values to control response size and query performance
- **Implement Pagination**: Use pagination (`skip` and `limit`) for large result sets
- **Optimize Complex Queries**: Break down complex queries when possible
- **Leverage Indexes**: Prioritize filtering on indexed properties
- **Filter Early in Traversals**: Apply filters as early as possible in relationship traversals
- **Consider Caching**: For frequently accessed records, implement caching strategies
- **Use Transactions**: Wrap related operations in transactions for consistency and improved performance
- **Monitor Query Performance**: Test and optimize slow queries

## Related Documentation

- [Search Introduction](/concepts/search/introduction)
- [Where Clause](/concepts/search/where)
- [Labels](/concepts/search/labels)
- [Pagination and Order](/concepts/search/pagination-order)
- [Record Relationships](/concepts/relationships)
- [Aggregations](/concepts/search/aggregations)
- [Transactions](/concepts/transactions)
- [Models](/typescript-sdk/models)
