---
sidebar_position: 4
---

# Relationships

Relationships in RushDB allow you to create links between [records](../concepts/records), simplifying the modeling of complex data interactions and enhancing relationships between different Records in your application.

## Overview

Relationships in RushDB are a powerful feature that allows you to:

- Connect related records with typed relationships (e.g., "BELONGS_TO", "HAS_EMPLOYEE")
- Specify directionality of relationships (incoming or outgoing (default))
- Query related records efficiently
- Build complex graph structures that represent your domain model
- Manage relationships independently from record creation

## Creating Relationships

There are two main ways to create relationships between records:

### 1. Using the `attach` Method on a Model

```typescript
await Model.attach({
  source: sourceRecord,
  target: targetRecord,
  options: {
    type: "RELATIONSHIP_TYPE",
    direction: "out"
  }
}, transaction?);
```

### 2. Using the `attach` Method on a Record Instance

```typescript
await recordInstance.attach(
  targetRecord,
  { type: "RELATIONSHIP_TYPE", direction: "out" },
  transaction?
);
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `source` | DBRecordTarget | The source record (required for Model.attach) |
| `target` | RelationTarget | The target record(s) - can be a single record or an array |
| `options` | RelationOptions | Optional configuration for the relationship |
| `options.type` | string | Type of relationship (defaults to `__RUSHDB__RELATION__DEFAULT__`) |
| `options.direction` | 'in' \| 'out' | Direction of the relationship (defaults to `out`) |
| `transaction` | Transaction \| string | Optional transaction for atomic operations |

#### Examples

**Creating a simple relationship:**

```typescript
import { db } from './db';

// First, let's create some records
const company = await db.records.create({
  label: "Company",
  data: { name: "Acme Corp", founded: 1942 }
});

const employee = await db.records.create({
  label: "Person",
  data: { name: "John Doe", age: 35 }
});

// Now let's create a relationship between them
await db.records.attach({
  source: company,
  target: employee,
  options: {
    type: "EMPLOYS",
    direction: "out"
  }
});

// The relationship is now established: (Company)-[EMPLOYS]->(Person)
```

**Creating multiple relationships at once:**

```typescript
// Create multiple employees
const employees = await db.records.createMany({
  label: "Person",
  payload: [
    { name: "Jane Smith", age: 28 },
    { name: "Bob Johnson", age: 42 }
  ]
});

// Connect all employees to the company in a single operation
await db.records.attach({
  source: company,
  target: employees,
  options: { type: "EMPLOYS" }
});
```

**With a transaction:**

```typescript
const transaction = await db.tx.begin();

try {
  await db.records.attach({
    source: company.id(),
    target: [employee1.id(), employee2.id()],
    options: { type: "EMPLOYS" }
  }, transaction);

  // Perform other operations...

  await transaction.commit();
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

**Using a model:**

```typescript
// Define a Company model
const CompanyModel = db.defineModel("Company", {
  name: { type: "string", required: true },
  founded: { type: "number" }
});

// Create a relationship using the model
await CompanyModel.attach({
  source: companyId,
  target: employeeIds,
  options: { type: "EMPLOYS" }
});
```

**Using a record instance:**

```typescript
// From a record instance
const companyRecord = await CompanyModel.findById("company-123");
await companyRecord.attach(
  employeeRecord,
  { type: "EMPLOYS" }
);
```

## Retrieving Relationships

You can retrieve relationships for a specific record using the `relationships` method:

```typescript
const relationsResponse = await db.records.relationships(recordId);

// Response contains an array of relationships
console.log(relationsResponse.data);
```

### Example Response

```typescript
{
  success: true,
  data: [
    {
      sourceId: "company-123",
      sourceLabel: "Company",
      targetId: "person-456",
      targetLabel: "Person",
      type: "EMPLOYS"
    },
    {
      sourceId: "department-789",
      sourceLabel: "Department",
      targetId: "company-123",
      targetLabel: "Company",
      type: "BELONGS_TO"
    }
  ],
  total: 2
}
```

## Searching for Relationships

You can search for relationships across your database using the `relationships.find` method:

```typescript
const relationSearchResponse = await db.relationships.find({
  where: {
    // Filter conditions
  }
});
```

### Example: Finding All Relationships of a Specific Type

```typescript
const employmentRelations = await db.relationships.find({
  where: {
    type: "EMPLOYS"
  },
  limit: 10
});

console.log(employmentRelations.data);
```

### Example: Finding Relationships with Complex Conditions

```typescript
// Find relationships where a Company employs a Person with specific attributes
const relationships = await db.relationships.find({
  where: {
    sourceRecord: {
      __label: "Company",
      name: "Acme Corp"
    },
    targetRecord: {
      __label: "Person",
      age: { $gt: 30 }
    },
    type: "EMPLOYS"
  }
});
```

## Removing Relationships

You can remove relationships between records using the `detach` method:

### 1. Using the `detach` Method on a Model

```typescript
await Model.detach({
  source: sourceRecord,
  target: targetRecord,
  options: {
    typeOrTypes: "RELATIONSHIP_TYPE",
    direction: "out"
  }
}, transaction?);
```

### 2. Using the `detach` Method on a Record Instance

```typescript
await recordInstance.detach(
  targetRecord,
  { typeOrTypes: "RELATIONSHIP_TYPE", direction: "out" },
  transaction?
);
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `source` | DBRecordTarget | The source record (required for Model.detach) |
| `target` | RelationTarget | The target record(s) - can be a single record or an array |
| `options` | RelationDetachOptions | Optional configuration for the detachment |
| `options.typeOrTypes` | string \| string[] | Type(s) of relationship to remove |
| `options.direction` | 'in' \| 'out' | Direction of the relationship |
| `transaction` | Transaction \| string | Optional transaction for atomic operations |

#### Examples

**Removing a specific relationship:**

```typescript
await db.records.detach({
  source: company,
  target: employee,
  options: {
    typeOrTypes: "EMPLOYS",
    direction: "out"
  }
});
```

**Removing multiple relationship types:**

```typescript
await db.records.detach({
  source: company,
  target: employee,
  options: {
    typeOrTypes: ["EMPLOYS", "MANAGES"],
    direction: "out"
  }
});
```

**Removing all relationships of any type:**

```typescript
// Remove all relationships between the company and employee
await db.records.detach({
  source: company,
  target: employee
  // No typeOrTypes specified means all relationships
});
```

**Using a record instance:**

```typescript
const companyRecord = await CompanyModel.findById("company-123");
await companyRecord.detach(employeeRecord);
```

## Relationship Directionality

RushDB supports three types of relationship directionality:

1. **Outgoing relationships (`direction: "out"`)**:
   The source record points to the target record: `(source)-[relationship]->(target)`

2. **Incoming relationships (`direction: "in"`)**:
   The target record points to the source record: `(source)<-[relationship]-(target)`

3. **Undirected relationships (no direction specified)**:
   The relationship has no specific direction: `(source)-[relationship]-(target)`

When creating a relationship, you can specify the direction using the `direction` property in the options object. When not specified, the default direction is `"out"`.

## Working with Relationships in Transactions

For data consistency, you can manage relationships within transactions:

```typescript
const transaction = await db.tx.begin();

try {
  // Create records
  const department = await db.records.create({
    label: "Department",
    data: { name: "Engineering" }
  }, transaction);

  const employee = await db.records.create({
    label: "Person",
    data: { name: "Alice Brown", title: "Engineer" }
  }, transaction);

  // Create relationships
  await db.records.attach({
    source: department,
    target: employee,
    options: { type: "HAS_MEMBER" }
  }, transaction);

  // Commit all changes atomically
  await transaction.commit();
  console.log("Records and relationships created successfully");
} catch (error) {
  // Rollback on any error
  await transaction.rollback();
  console.error("Transaction failed:", error);
}
```

## Examples of Common Relationship Patterns

### One-to-Many Relationship

```typescript
// A company has many employees
const company = await db.records.create({
  label: "Company",
  data: { name: "Innovatech" }
});

const employees = await db.records.createMany({
  label: "Person",
  payload: [
    { name: "Employee 1", role: "Developer" },
    { name: "Employee 2", role: "Designer" },
    { name: "Employee 3", role: "Manager" }
  ]
});

// Create one-to-many relationship
await db.records.attach({
  source: company,
  target: employees,
  options: { type: "HAS_EMPLOYEE" }
});
```

### Many-to-Many Relationship

```typescript
// Students can take many courses, and courses can have many students
const students = await db.records.createMany({
  label: "Student",
  payload: [
    { name: "Student A", grade: "A" },
    { name: "Student B", grade: "B" }
  ]
});

const courses = await db.records.createMany({
  label: "Course",
  payload: [
    { name: "Math 101", credits: 3 },
    { name: "History 202", credits: 4 }
  ]
});

// Create many-to-many relationships
for (const student of students.data) {
  await db.records.attach({
    source: student,
    target: courses.data,
    options: { type: "ENROLLED_IN" }
  });
}
```

### Self-Referential Relationship

```typescript
// Employees can manage other employees
const manager = await db.records.create({
  label: "Employee",
  data: { name: "Senior Manager", level: 5 }
});

const subordinates = await db.records.createMany({
  label: "Employee",
  payload: [
    { name: "Team Lead 1", level: 3 },
    { name: "Team Lead 2", level: 3 }
  ]
});

// Create self-referential relationships
await db.records.attach({
  source: manager,
  target: subordinates.data,
  options: { type: "MANAGES" }
});
```

## Best Practices

1. **Use meaningful relationship types**: Choose relationship types that clearly describe the connection between records. For example, `"AUTHORED"` is more descriptive than a generic relationship.

2. **Consider directionality**: Choose the right direction for your relationships based on your domain model. For example, `(Author)-[WROTE]->(Book)` makes more sense than the reverse.

3. **Manage relationships in transactions**: When creating or modifying multiple records and their relationships, use transactions to ensure data consistency.

4. **Use consistent naming conventions**: Establish naming conventions for relationship types, such as uppercase with underscores (e.g., `"HAS_MEMBER"`, `"BELONGS_TO"`).

5. **Retrieve relationships efficiently**: When working with highly connected records, use pagination (with `skip` and `limit` parameters) to manage performance.

6. **Delete relationships before records**: When deleting records that have relationships, consider removing the relationships first to maintain referential integrity.

7. **Document relationship types**: Keep a clear documentation of all relationship types used in your application to maintain consistency.

## Type Definitions

For TypeScript users, RushDB SDK provides the following types for working with relationships:

```typescript
// Relation type definition
type Relation = {
  sourceId: string;
  sourceLabel: string;
  targetId: string;
  targetLabel: string;
  type: string;
};

// Direction options
type RelationDirection = 'in' | 'out';

// Options when creating a relationship
type RelationOptions = {
  direction?: RelationDirection;
  type?: string;
};

// Options when removing a relationship
type RelationDetachOptions = {
  direction?: RelationDirection;
  typeOrTypes?: string | string[];
};
```

These types help ensure type safety when working with relationships in your TypeScript code.
