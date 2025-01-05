---
sidebar_position: 3
---

# Create & Fetch Records
In this section, we'll learn how to use the RushDB SDK to create and retrieve simple data records. This guide assumes you have already initialized the SDK and obtained an API token as described in the previous sections. Here, we'll focus on utilizing the SDK to interact with your data, demonstrating how to define a data model, create a record, and then fetch it back.

## Objective

The goal of this section is to familiarize you with the basics of creating and retrieving data using the RushDB SDK. While the examples provided are agnostic and can be adapted to your specific library/framework, they serve as a foundation for understanding how to work with RushDB data models and records.

## Prerequisites

Ensure that you have initialized the RushDB SDK in your project as follows:

```typescript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB(
    'API_TOKEN'
);
```

Replace `API_TOKEN` with your actual API token.

## Defining a Data Model

First, let's define a data model for a user. This model will specify the structure of the user records we intend to create and retrieve:
```typescript
import { Model } from '@rushdb/javascript-sdk';

// Helper function to generate random numbers for ID
const asyncRandomNumber = async () => Math.floor(Math.random() * 10000);

const User = new Model('user', {
  name: { type: 'string' },
  email: { type: 'string', uniq: true },
  id: { type: 'number', default: asyncRandomNumber },
  jobTitle: { type: 'string' },
  age: { type: 'number' },
  married: { type: 'boolean' },
  dateOfBirth: { type: 'datetime', default: () => new Date().toISOString() }
});

export const UserRepo = db.registerModel(User);
```

## Creating a User Record
With our `User` model defined, we can now create a user record. Here's an example payload for creating a user:
```typescript
const payload = {
    name: 'Jane Doe',
    email: 'jane.doe@example.com',
    id: await asyncRandomNumber(),
    jobTitle: 'Developer',
    age: 28,
    married: false,
    dateOfBirth: '1992-05-15'
};

await UserRepo.create(payload);
```

## Retrieving User Records
After creating users, you may want to fetch them back. Use the find method to retrieve a list of all users:

```typescript
const users = await UserRepo.find();
console.log(users);
```

This simple flow demonstrates how to create and retrieve records using the RushDB SDK. By defining models and utilizing the SDK's methods, you can easily manage your application's data. Feel free to adapt these examples to fit the specific needs of your project.