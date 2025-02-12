---
sidebar_position: 3
---

# Create & Fetch Records
In this section, we'll learn how to use the RushDB SDK to create and retrieve simple data records. This guide assumes you have already initialized the SDK and obtained an API token as described in the previous sections. Here, we'll focus on utilizing the SDK to interact with your data, demonstrating how to define a data model, create a record, and then fetch it back.

## Prerequisites

Ensure that you have initialized the RushDB SDK in your project as follows:

### TypeScript / Javascript
```typescript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB('API_TOKEN');
```

### Python

```bash
from rushdb import RushDB

db = RushDB("API_TOKEN")
```

Replace `API_TOKEN` with your actual API token.

## Creating Records

The `create` method allows you to create a single record without registering a model.

### TypeScript / Javascript
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

### Python

```python
newAuthor = db.records.create(
    "author", 
    {
        "name": "Alice Smith",
        "email": "alice.smith@example.com",
        "jobTitle": "writer",
        "age": 28,
        "married": True,
        "dateOfBirth": "1993-05-15T00:00:00Z"
    }
)
```


## Reading Records

The `find`  method let you read records from the database without predefining models.

### TypeScript / Javascript

```typescript
const authors = await db.records.find('author', {
  where: {
    jobTitle: { $contains: 'writer' },
    age: { $gte: 25 }
  }
});
```

### Python

```python
authors = db.records.find({
    "labels": ["author"]
    "where": {
        "jobTitle": { "$contains": "writer" },
        "age": { "$gte": 25 }
    }
})
```


This simple flow demonstrates how to create and retrieve records using the RushDB SDK. By defining models and utilizing the SDK's methods, you can easily manage your application's data. Feel free to adapt these examples to fit the specific needs of your project.
