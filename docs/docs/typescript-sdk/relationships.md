---
sidebar_position: 4
---

# Relationships

[Relationships](../concepts/relationships.md) in RushDB connect records to form a rich, interconnected network of data. The TypeScript SDK provides powerful methods for creating, managing, and traversing relationships between records.

## Overview

The relationships API in the SDK enables you to:
- Create connections between records
- Remove relationships between records
- Search for relationships based on specific criteria
- Build complex graph-like data structures
- Navigate between connected entities

## Creating Relationships

### Using RushDB's `attach()` Method

To create a relationship between records, use the `records.attach` method:

```typescript
// Attaching one record to another
const result = await db.records.attach({
  source: 'user_123',
  target: 'company_456',
  options: {
    type: 'WORKS_AT',
    direction: 'out' // User -> WORKS_AT -> Company
  }
});

console.log(result);
/*
{
  success: true,
  message: "Relationship created successfully"
}
*/
```

#### Parameters

- `params`: An object containing:
  - `source`: The source record (ID, record object, or record instance)
  - `target`: The target record(s) (ID, array of IDs, record object, record instance, or array of record instances)
  - `options` (optional): Configuration for the relationship:
    - `type`: The type/name of the relationship
    - `direction`: Direction of the relationship ('in' or 'out')
- `transaction` (optional): A [transaction](../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a success object

### Using Model's `attach()` Method

If you're using models, you can use the model's `attach` method:

```typescript
// Define models
const UserModel = new Model('USER', {
  name: { type: 'string' },
  email: { type: 'string', unique: true }
});

const CompanyModel = new Model('COMPANY', {
  name: { type: 'string' },
  industry: { type: 'string' }
});

// Create records
const user = await UserModel.create({
  name: 'John Doe',
  email: 'john@example.com'
});

const company = await CompanyModel.create({
  name: 'Acme Inc.',
  industry: 'Technology'
});

// Create relationship between user and company
await UserModel.attach({
  source: user,
  target: company,
  options: {
    type: 'WORKS_AT',
    direction: 'out'
  }
});
```

### Creating Relationships in Transactions

```typescript
const transaction = await db.tx.begin();
try {
  const user = await db.records.create({
    label: 'USER',
    data: {
      name: 'Jane Smith',
      email: 'jane@example.com'
    }
  }, transaction);

  const company = await db.records.create({
    label: 'COMPANY',
    data: {
      name: 'Tech Corp',
      industry: 'Software'
    }
  }, transaction);

  await db.records.attach({
    source: user.data.__id,
    target: company.data.__id,
    options: {
      type: 'WORKS_AT',
      direction: 'out'
    }
  }, transaction);

  await transaction.commit();
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

### Bulk Relationship Creation by Key Match

Use `relationships.createMany` to create relationships in bulk by matching a key from a source label to a key from a target label. This is useful when you ingest data in batches (e.g., from CSV/JSON) and want to connect records created at different times.

```ts
// Create USER -[:ORDERED]-> ORDER for all pairs where
// USER.id = ORDER.userId and both match the given tenant
await db.relationships.createMany({
  source: { label: 'USER', key: 'id', where: { tenantId } },
  target: { label: 'ORDER', key: 'userId', where: { tenantId } },
  type: 'ORDERED',
  direction: 'out' // (source) -[:ORDERED]-> (target)
})
```

Parameters
- `source`: Object describing the source side
  - `label`: Source record label (string)
  - `key`: Property on the source used for equality match (string)
  - `where` (optional): Additional filters for source records; same shape as SearchQuery `where`
- `target`: Object describing the target side
  - `label`: Target record label (string)
  - `key`: Property on the target used for equality match (string)
  - `where` (optional): Additional filters for target records; same shape as SearchQuery `where`
- `type` (optional): Relationship type. Defaults to the RushDB default type when omitted
- `direction` (optional): 'in' or 'out'. Defaults to 'out'.

Notes
- Matching condition is always `source[key] = target[key]` plus any additional `where` constraints.
- `where` uses the same operators as record search (e.g., plain equality `{ tenantId: 'ACME' }`, or explicit `{ tenantId: 'ACME' }`).
- Operation can run within a transaction if provided.

## Removing Relationships

### Using RushDB's `detach()` Method

To remove a relationship between records, use the `records.detach` method:

```typescript
const result = await db.records.detach({
  source: 'user_123',
  target: 'company_456',
  options: {
    type: 'WORKS_AT'  // Optional: Only detach relationships of this type
  }
});

console.log(result);
/*
{
  success: true,
  message: "Relationship removed successfully"
}
*/
```

#### Parameters

- `params`: An object containing:
  - `source`: The source record (ID, record object, or record instance)
  - `target`: The target record(s) (ID, array of IDs, record object, record instance, or array of record instances)
  - `options` (optional): Configuration for the detach operation:
    - `typeOrTypes`: The type(s) of relationships to remove (string or array of strings)
    - `direction`: Direction of the relationship to remove ('in' or 'out')
- `transaction` (optional): A [transaction](../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to a success object

### Using Model's `detach()` Method

If you're using models, you can use the model's `detach` method:

```typescript
// Detach a relationship between user and company
await UserModel.detach({
  source: user.data.__id,
  target: company.data.__id,
  options: {
    typeOrTypes: 'WORKS_AT'
  }
});
```

### Bulk Relationship Deletion by Key Match

You can remove relationships in bulk with the SDK using `relationships.deleteMany`. It accepts the same shape as `createMany` and supports two modes:

- key-match mode: match source and target records by equality of a pair of properties (e.g. `USER.id = ORDER.userId`) and delete the relationship between matched pairs.
- many-to-many (cartesian) mode: opt-in operation that deletes relationships between every matching source and target pair that satisfy provided filters — use with extreme caution.

TypeScript example — key match deletion:

```ts
await db.relationships.deleteMany({
  source: { label: 'USER', key: 'id', where: { tenantId } },
  target: { label: 'ORDER', key: 'userId', where: { tenantId } },
  type: 'ORDERED',
  direction: 'out'
})
```

TypeScript example — many-to-many deletion (explicit opt-in):

```ts
// WARNING: manyToMany will perform a cartesian-style deletion across the
// filtered sets. Only use with explicit filters on both sides.
await db.relationships.deleteMany({
  source: { label: 'USER', where: { tenantId } },
  target: { label: 'TAG', where: { tenantId } },
  type: 'HAS_TAG',
  direction: 'out',
  manyToMany: true
})
```

Parameters
- `source`: Object describing the source side
  - `label`: Source record label (string)
  - `key` (optional): Property on the source used for equality match (string)
  - `where` (optional): Additional filters for source records; same shape as SearchQuery `where`
- `target`: Object describing the target side
  - `label`: Target record label (string)
  - `key` (optional): Property on the target used for equality match (string)
  - `where` (optional): Additional filters for target records; same shape as SearchQuery `where`
- `type` (optional): Relationship type to restrict deletions
- `direction` (optional): 'in' or 'out'. Defaults to 'out'.
- `manyToMany` (optional): boolean. When `true` the operation will perform deletions across all source/target pairs matching provided filters (cartesian). This must be explicitly set.

Important notes and safeguards
- If `manyToMany` is not provided or is `false`, both `source.key` and `target.key` must be supplied — deletion matches records where `source[key] = target[key]`.
- If `manyToMany` is `true`, the server requires non-empty `where` filters for both `source` and `target` to avoid accidental full-cartesian deletions.
- Use `manyToMany` only when you intentionally want to delete relationships across filtered sets. Consider testing on a staging dataset first.


## Finding Relationships

### Using RushDB's `relationships.find()` Method

To search for relationships based on specific criteria, use the `relationships.find` method:

```typescript
const relationships = await db.relationships.find({
  labels: ['USER'],
  where: {
    name: { $contains: 'John' },
    COMPANY: {
      industry: 'Technology',
      $relation: 'WORKS_AT'
    }
  },
  limit: 10
});

console.log(relationships);
/*
{
  data: [
    {
      sourceId: 'user_123',
      sourceLabel: 'USER',
      targetId: 'company_456',
      targetLabel: 'COMPANY',
      type: 'WORKS_AT'
    },
    // More relationships...
  ],
  total: 5
}
*/
```

#### Parameters

- `searchQuery`: A search query object to find matching relationships
  - `where`: Conditions to filter relationships
  - `limit`: Maximum number of results to return
  - `skip`: Number of results to skip
- `transaction` (optional): A [transaction](../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to an API response object: `{ success: boolean; data: Array<Relation>; total?: number }`

### Finding Relationships in Transactions

```typescript
const transaction = await db.tx.begin();
try {
 const relationships = await db.relationships.find({
    labels: ['USER'],
    where: {
      name: { $contains: 'John' },
      COMPANY: {
        industry: 'Technology',
        $relation: 'WORKS_AT'
      }
    },
    limit: 10
  }, transaction);

  // Perform other operations...

  await transaction.commit();
  console.log(relationships);
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

## Retrieving Relationships for a Record

### Using RushDB's `records.relations()` Method

To get all relationships for a specific record, use the `records.relations` method:

```typescript
const relations = await db.records.relations('user_123');

console.log(relations);
/*
{
  data: [
    {
      id: 'relation_id_1',
      type: 'WORKS_AT',
      source: 'user_123',
      target: 'company_456',
      direction: 'out'
    },
    {
      id: 'relation_id_2',
      type: 'FOLLOWS',
      source: 'user_789',
      target: 'user_123',
      direction: 'in'
    },
    // More relationships...
  ],
  total: 5
}
*/
```

#### Parameters

- `target`: The record to get relationships for (ID, record object, or record instance)
- `transaction` (optional): A [transaction](../concepts/transactions.mdx) object or string to include the operation within a transaction

#### Returns

- A promise that resolves to an array of relationship objects

## Relationship Direction

Relationships in RushDB have a direction, which defines how records are connected. When creating or querying relationships, you can specify the direction:

- `out`: The relationship goes from source to target: `(source) -[RELATIONSHIP]-> (target)`
- `in`: The relationship goes from target to source: `(source) <-[RELATIONSHIP]- (target)`

For example:
```typescript
// Outgoing relationship (User -[WORKS_AT]-> Company)
await db.records.attach({
  source: userId,
  target: companyId,
  options: {
    type: 'WORKS_AT',
    direction: 'out'
  }
});

// Incoming relationship (Department <-[BELONGS_TO]- Employee)
await db.records.attach({
  source: departmentId,
  target: employeeId,
  options: {
    type: 'BELONGS_TO',
    direction: 'in'
  }
});
```

## Custom Relationship Types

By default, RushDB uses a standard relationship type, but you can specify custom types to model your domain more accurately:

```typescript
// Creating a relationship with a custom type
await db.records.attach({
  source: mentorId,
  target: menteeId,
  options: {
    type: 'MENTORS',
    direction: 'out'
  }
});

// Creating a relationship when importing nested data
const company = await db.records.create({
  label: 'COMPANY',
  data: {
    name: 'Tech Corp',
    employees: [
      { name: 'Jane Smith', position: 'CTO' },
      { name: 'John Doe', position: 'Developer' }
    ]
  },
  options: {
    relationshipType: 'EMPLOYS'  // Custom relationship type
  }
});
```

## Best Practices for Working with Relationships

1. **Use Meaningful Relationship Types**
   - Choose descriptive names for relationship types that clearly convey their meaning
   - Establish a consistent naming convention for relationships (e.g., using verbs like 'FOLLOWS', 'WORKS_AT')

2. **Consider Relationship Direction**
   - Use the direction parameter to model the natural flow of relationships
   - For bidirectional relationships, create two relationships with opposite directions

3. **Use Transactions for Multiple Operations**
   - When creating or updating multiple records and their relationships, use transactions
   - This ensures all operations succeed or fail together, maintaining data consistency

4. **Optimize Relationship Queries**
   - Specify relationship types when searching to improve performance
   - Use direction filters to narrow down search results

5. **Model Domain Relationships Carefully**
   - Use relationship types that map to real-world concepts in your domain
   - Consider the cardinality of relationships (one-to-one, one-to-many, many-to-many)

## Conclusion

The Relationships API in the RushDB TypeScript SDK provides a comprehensive set of methods for creating, managing, and querying relationships between records. By understanding these methods and their parameters, you can effectively build interconnected data structures in your application.

For more information on related topics, see:
- [Records](./records/create-records.md) - Work with records that participate in relationships
- [Search](./records/get-records.md) - Advanced querying across relationships
- [Models](./models.md) - Define structured schemas for your data
