---
sidebar_position: 2
---

# CRUD Operations without Model Registration
:::note
While registering models with `Model` provides strong TypeScript support and a clear structure, you can also perform CRUD operations directly using the `RestAPI` methods in the `RushDB` class. This approach allows you to interact with your data without predefining models, offering flexibility for quick operations or dynamic use cases.
:::

## Creating Records without Model Registration

The `create` method allows you to create a single record without registering a model.

### Example

Creating an author record directly:
```typescript
const newAuthor = await db.records.create('author', {
  name: 'Alice Smith',
  email: 'alice.smith@example.com',
  jobTitle: 'writer',
  age: 28,
  married: true,
  dateOfBirth: '1993-05-15T00:00:00Z'
});
```

## Reading Records without Model Registration

The `find`, `findOne`, and `findById` methods let you read records from the database without predefining models.

### Example

Finding records with specific criteria:
```typescript
const authors = await db.records.find('author', {
    where: {
        jobTitle: { $contains: 'writer' },
        age: { $gte: 25 }
    }
});
```

### Example

Finding a single record:
```typescript
const author = await db.records.findOne('author', {
    where: {
        email: { $contains: 'alice.smith@example.com' }
    }
});
```

## Updating Records without Model Registration

You can update records using the `update` method, which allows for modifications without registered models.

### Example

Updating a record directly:
```typescript
const author = await db.records.findOne('author', {
    where: {
        email: { $contains: 'alice.smith@example.com' }
    }
});

await db.records.update(author.data.__id, {
  jobTitle: 'senior writer'
});

```

## Deleting Records without Model Registration

The `deleteById` and `deleteById` methods enable you to remove records from the database without using registered models.

### Example

Deleting records with specific criteria:
```typescript
await db.records.deleteById('author', {
  where: {
    jobTitle: { $contains: 'writer' }
  }
});
```

### Example

Deleting records by ID:
```typescript
await db.records.deleteById('author', ['author_id_1', 'author_id_2']);
```

## Conclusion

Using the `RestAPI` methods in the `RushDB` class provides a flexible way to perform CRUD operations without registering models. This approach is particularly useful for dynamic or ad-hoc operations, offering a straightforward way to interact with your data.
