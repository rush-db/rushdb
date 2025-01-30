---
sidebar_position: 3
---

# Create & Fetch Records
In this section, we'll learn how to use the RushDB SDK to create and retrieve simple data records. This guide assumes you have already initialized the SDK and obtained an API token as described in the previous sections. Here, we'll focus on utilizing the SDK to interact with your data, demonstrating how to define a data model, create a record, and then fetch it back.

## Prerequisites

Ensure that you have initialized the RushDB SDK in your project as follows:

```typescript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB('API_TOKEN');
```

Replace `API_TOKEN` with your actual API token.

## Creating Records

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

## Reading Records

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
        email: { $contains: 'alice.smith@' }
    }
});
```

This simple flow demonstrates how to create and retrieve records using the RushDB SDK. By defining models and utilizing the SDK's methods, you can easily manage your application's data. Feel free to adapt these examples to fit the specific needs of your project.
