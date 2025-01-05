---
sidebar_position: 5
---

# Reading Records
:::note
Reading records involves fetching data from the database. The `Model` class provides several methods to retrieve records based on different criteria..
:::

## Table of Contents

- [Basic Querying](#find)
- [Complex Query Executions](#complex-example-with-related-models)


### `find`

The `find` method is used to retrieve multiple records based on specified criteria.

**Signature:**

```typescript
find(
  params?: SearchQuery<S> & { labels?: never },
  transaction?: Transaction | string
): Promise<DBRecordsArrayInstance<S>>;

```

**Parameters:**

- `params` (optional): An object specifying query parameters such as filters, sorting, and pagination.
- `transaction` (optional): A transaction object or string to include the operation within a transaction.

**Returns:**

- A promise that resolves to a `DBRecordsArrayInstance` containing the retrieved records.

**Examples:**

*Basic Example:*
```typescript
const authors = await Author.find();
console.log(authors);
/*
{
  data: [
    {
      __id: 'author_id_1',
      __label: 'author',
      name: 'John Doe',
      email: 'john.doe@example.com'
    },
    {
      __id: 'author_id_2',
      __label: 'author',
      name: 'Jane Smith',
      email: 'jane.smith@example.com'
    }
  ],
  total: 2
}
*/

```

*Complex Example:*
```typescript
const authors = await Author.find({
  where: { name: { $contains: 'John' } },
  orderBy: { createdAt: 'desc' },
  limit: 10,
  skip: 5
});
console.log(authors);
/*
{
  data: [
    {
      __id: 'author_id_3',
      __label: 'author',
      name: 'John Brown',
      email: 'john.brown@example.com'
    },
    {
      __id: 'author_id_4',
      __label: 'author',
      name: 'John Green',
      email: 'john.green@example.com'
    }
  ],
  total: 2
}
*/

```

### `findOne`

The `findOne` method is used to retrieve a single record based on specified criteria.

**Signature:**
```typescript
findOne(
  params?: SearchQuery<S> & { labels?: never },
  transaction?: Transaction | string
): Promise<DBRecordInstance<S>>;

```

**Parameters:**

- `params` (optional): An object specifying query parameters.
- `transaction` (optional): A transaction object or string to include the operation within a transaction.

**Returns:**

- A promise that resolves to a `DBRecordInstance` containing the retrieved record.

**Examples:**

*Basic Example:*
```typescript
const author = await Author.findOne({ where: { email: 'john.doe@example.com' } });
console.log(author);
/*
{
  data: {
    __id: 'author_id',
    __label: 'author',
    name: 'John Doe',
    email: 'john.doe@example.com'
  }
}
*/

```

*Complex Example:*
```typescript
const author = await Author.findOne({
  where: {
    $AND: [{ name: { $startsWith: 'Jane' } }, { email: { $contains: '@example.com' } }]
  },
  transaction
});
console.log(author);
/*
{
  data: {
    __id: 'author_id',
    __label: 'author',
    name: 'Jane Doe',
    email: 'jane.doe@example.com'
  }
}
*/

```

### `findById`

The `findById` method is used to retrieve a single record by its ID.

**Signature:**
```typescript
findById(
  id: string,
  transaction?: Transaction | string
): Promise<DBRecordInstance<S>>;

```

**Parameters:**

- `id`: The ID of the record to retrieve.
- `transaction` (optional): A transaction object or string to include the operation within a transaction.

**Returns:**

- A promise that resolves to a `DBRecordInstance` containing the retrieved record.

**Examples:**

*Basic Example:*
```typescript
const author = await Author.findById('author_id');
console.log(author);
/*
{
  data: {
    __id: 'author_id',
    __label: 'author',
    name: 'John Doe',
    email: 'john.doe@example.com'
  }
}
*/

```

*Complex Example:*
```typescript
const transaction = await db.tx.begin();
const author = await Author.findById('author_id', transaction);
await transaction.commit();
console.log(author);
/*
{
  data: {
    __id: 'author_id',
    __label: 'author',
    name: 'John Doe',
    email: 'john.doe@example.com'
  }
}
*/

```

### Complex Example with Related Models

Let's add a `Post` model and perform a series of operations involving both `Author` and `Post`.
```typescript
const Post = new Model('post', {
  created: { type: 'datetime', default: () => new Date().toISOString() },
  title: { type: 'string' },
  content: { type: 'string' },
  rating: { type: 'number' }
});

// Register the Post model
export const PostRepo = db.registerModel(Post);

```

**Steps:**

1. Find an author by ID.
2. Create a new `Post` and attach it to the `Author`.
3. Find the created `Post` using a complex `where` condition.

```typescript
const transaction = await db.tx.begin();
try {
  // Step 1: Find an author by ID
  const author = await Author.findById('author_id', transaction);

  // Step 2: Create a new Post and attach it to the Author
  const newPost = {
    title: 'New Blog Post',
    content: 'This is a new blog post content.',
    rating: 4.5
  };

  const createdPost = await PostRepo.create(newPost, transaction);

  await Author.attach(author.data.__id, {
    model: 'post',
    recordId: createdPost.data.__id
  }, transaction);

  // Step 3: Find the created Post using a complex where condition
  const posts = await PostRepo.find({
    where: {
      $AND: [
        { created: { $gte: '2023-01-01T00:00:00Z' } },
        { rating: { $gte: 4 } }
      ]
    },
    orderBy: { created: 'desc' }
  }, transaction);

  await transaction.commit();
  console.log(posts);
  /*
  {
    data: [
      {
        __id: 'post_id',
        __label: 'post',
        created: '2023-01-02T00:00:00Z',
        title: 'New Blog Post',
        content: 'This is a new blog post content.',
        rating: 4.5
      }
    ],
    total: 1
  }
  */
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

This complex example demonstrates how to work with related models, perform transactions, and use advanced querying features.

## Conclusion

This section provided an in-depth look at the reading operations available through the `Model` class. By understanding these methods and their parameters, you can effectively retrieve your application's data with the RushDB SDK. Subsequent sections will delve into more advanced topics such as relationships between models and custom validations.
