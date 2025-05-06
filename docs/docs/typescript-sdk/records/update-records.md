---
sidebar_position: 6
---

# Update Records
:::note
Updating records is a crucial operation for maintaining and modifying data within your application. The `Model` class provides methods to update single or multiple records. We will use the `Author` and `Post` models defined earlier to demonstrate these operations.
:::

## Table of Contents

- [Updating a Single Record](#update)
- [Updating Multiple Records](#updating-multiple-records)
- [Updating Records in a Transaction](#complex-example-with-transactions)

### `update`

The `update` method is used to update a single record by its ID.

**Signature:**
```typescript
update(
  id: string,
  record: InferSchemaTypesWrite<S>,
  transaction?: Transaction | string
): Promise<DBRecordInstance<S>>;
```

**Parameters:**

- `id`: The ID of the record to update.
- `record`: An object containing the updated data that adheres to the schema defined for the model.
- `transaction` (optional): A transaction object or string to include the operation within a transaction.
- `options` (optional): An object to specify additional options, such as whether to validate the record before updating it.

**Returns:**

- A promise that resolves to a `DBRecordInstance` containing the updated record.

**Examples:**

*Basic Example with Author:*
```typescript
const updatedAuthor = await Author.update('author_id', {
  name: 'John Doe Updated'
});
console.log(updatedAuthor);
/*
{
  data: {
    __id: 'author_id',
    __label: 'author',
    name: 'John Doe Updated',
    email: 'john.doe@example.com'
  }
}
*/
```

*Complex Example with Author:*
```typescript
const transaction = await db.tx.begin();
try {
  const updatedAuthor = await Author.update('author_id', {
    name: 'Jane Doe Updated'
  }, transaction);
  await transaction.commit();
  console.log(updatedAuthor);
  /*
  {
    data: {
      __id: 'author_id',
      __label: 'author',
      name: 'Jane Doe Updated',
      email: 'jane.doe@example.com'
    }
  }
  */
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

*Basic Example with Post:*
```typescript
const updatedPost = await PostRepo.update('post_id', {
  title: 'Updated Blog Post Title'
});
console.log(updatedPost);
/*
{
  data: {
    __id: 'post_id',
    __label: 'post',
    created: '2023-01-02T00:00:00Z',
    title: 'Updated Blog Post Title',
    content: 'This is a new blog post content.',
    rating: 4.5
  }
}
*/
```

*Complex Example with Post:*
```typescript
const transaction = await db.tx.begin();
try {
  const updatedPost = await PostRepo.update('post_id', {
    title: 'Updated Title in Transaction',
    rating: 5
  }, transaction);
  await transaction.commit();
  console.log(updatedPost);
  /*
  {
    data: {
      __id: 'post_id',
      __label: 'post',
      created: '2023-01-02T00:00:00Z',
      title: 'Updated Title in Transaction',
      content: 'This is a new blog post content.',
      rating: 5
    }
  }
  */
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

### Updating Multiple Records

To update multiple records, you can use a combination of `find` and `update`. This involves retrieving the records you want to update, modifying them, and then saving the changes.

**Examples:**

*Basic Example with Author:*
```typescript
const authorsToUpdate = await Author.find({ where: { name: 'John Doe' } });
for (const author of authorsToUpdate.data) {
  await Author.update(author.__id, { name: 'John Doe Updated' });
}
console.log(authorsToUpdate);
/*
{
  data: [
    {
      __id: 'author_id_1',
      __label: 'author',
      name: 'John Doe Updated',
      email: 'john.doe@example.com'
    },
    {
      __id: 'author_id_2',
      __label: 'author',
      name: 'John Doe Updated',
      email: 'john.doe@example.com'
    }
  ]
}
*/
```

*Complex Example with Post:*
```typescript
const postsToUpdate = await PostRepo.find({ where: { rating: { $lt: 5 } } });
const transaction = await db.tx.begin();
try {
  for (const post of postsToUpdate.data) {
    await PostRepo.update(post.__id, { rating: 5 }, transaction);
  }
  await transaction.commit();
  console.log(postsToUpdate);
  /*
  {
    data: [
      {
        __id: 'post_id_1',
        __label: 'post',
        created: '2023-01-02T00:00:00Z',
        title: 'Blog Post Title 1',
        content: 'This is a blog post content.',
        rating: 5
      },
      {
        __id: 'post_id_2',
        __label: 'post',
        created: '2023-01-03T00:00:00Z',
        title: 'Blog Post Title 2',
        content: 'This is another blog post content.',
        rating: 5
      }
    ],
    total: 2
  }
  */
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

### Complex Example with Transactions

In this example, we'll update an `Author` and a `Post` within the same transaction. This ensures that either both updates succeed, or both are rolled back in case of an error.

**Steps:**

1. Begin a transaction.
2. Update the `Author`.
3. Update the `Post`.
4. Commit the transaction if both updates succeed.
5. Rollback the transaction if any update fails.

```typescript
const transaction = await db.tx.begin();
try {
  // Update the author
  const updatedAuthor = await Author.update('author_id', {
    name: 'Updated Author Name'
  }, transaction);

  // Update the post
  const updatedPost = await PostRepo.update('post_id', {
    title: 'Updated Post Title',
    content: 'Updated content for the post.',
    rating: 4.8
  }, transaction);

  await transaction.commit();
  console.log(updatedAuthor);
  console.log(updatedPost);
  /*
  {
    data: {
      __id: 'author_id',
      __label: 'author',
      name: 'Updated Author Name',
      email: 'john.doe@example.com'
    }
  }
  {
    data: {
      __id: 'post_id',
      __label: 'post',
      created: '2023-01-02T00:00:00Z',
      title: 'Updated Post Title',
      content: 'Updated content for the post.',
      rating: 4.8
    }
  }
  */
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

This complex example demonstrates how to perform multiple updates atomically within a transaction.

### Conclusion

This section covered how to update records using the `Model` class. By understanding these methods and their parameters, you can effectively modify your application's data with the RushDB SDK. The next sections will explore deleting records and other advanced operations.
