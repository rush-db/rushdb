---
sidebar_position: 7
---

# Deleting Records
:::note
Deleting records is a fundamental operation to manage the lifecycle of data within your application. The `Model` class provides methods to delete multiple records. We will use the `Author` and `Post` models defined earlier to demonstrate these operations.
:::

## Table of Contents

- [Deleting Records](#delete)
- [Handling Related Records](#handling-related-records)

### `delete`

The `delete` method is used to remove multiple records based on specified criteria.

**Signature:**
```typescript
delete(
  params?: Omit<SearchQuery<S>, 'labels'>,
  transaction?: Transaction | string
): Promise<ApiResponse<{ message: string }>>;
```
**Parameters:**

- `params` (optional): An object specifying query parameters such as filters.
- `transaction` (optional): A transaction object or string to include the operation within a transaction.

**Returns:**

- A promise that resolves to a `ApiResponse` containing the result of the delete operation.

**Examples:**

*Basic Example with Author:*
```typescript
const deleteResponse = await Author.delete({ where: { name: 'John Doe' } });
console.log(deleteResponse);
/*
{
  success: true,
  message: 'Records deleted successfully.'
}
*/
```

*Complex Example with Author:*
```typescript
const transaction = await db.tx.begin();
try {
  const deleteResponse = await Author.delete({ where: { name: { $contains: 'Jane' } } }, transaction);
  await transaction.commit();
  console.log(deleteResponse);
  /*
  {
    success: true,
    message: 'Records deleted successfully.'
  }
  */
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

*Basic Example with Post:*
```typescript
const deleteResponse = await Post.delete({ where: { title: 'Old Blog Post' } });
console.log(deleteResponse);
/*
{
  success: true,
  message: 'Records deleted successfully.'
}
*/
```

*Complex Example with Post:*
```typescript
const transaction = await db.tx.begin();
try {
  const deleteResponse = await Post.delete({ where: { rating: { $lt: 3 } } }, transaction);
  await transaction.commit();
  console.log(deleteResponse);
/*
{
  success: true,
  message: 'Records deleted successfully.'
}
*/
} catch (error) {
  await transaction.rollback();
  throw error;
}
```

*Deleting Posts with `$OR` Operator:*
```typescript
const deleteResponse = await Post.delete({
    where: {
        $OR: [
            { __id: 'post_id_1' },
            { __id: 'post_id_2' }
        ]
    }
});
console.log(deleteResponse);
/*
{
  success: true,
  message: 'Records deleted successfully.'
}
*/
```

*Complex Deletion with Multiple Operators:*
```typescript
const deleteResponse = await Post.delete({
  where: {
    $OR: [
      { title: { $contains: 'Blog' } },
      { rating: { $gte: 4.5 } },
      { created: { $lt: '2023-01-01T00:00:00Z' } }
    ]
  }
});
console.log(deleteResponse);
/*
{
  success: true,
  message: 'Records deleted successfully.'
}
*/
```

### Handling Related Records

When you delete a record that is attached to another record, the relationship is automatically removed. This ensures data integrity and consistency.

**Complex Example with Transactions:**

In this example, we'll delete an `Author` and ensure that any `Post` attached to this `Author` is also handled appropriately.

**Steps:**

1. Begin a transaction.
2. Delete the `Author`.
3. Verify that the related `Post` is updated accordingly.
4. Commit the transaction if the delete operation succeeds.
5. Rollback the transaction if any operation fails.
```typescript
const transaction = await db.tx.begin();
try {
  // Step 1: Delete the author
  const deleteAuthorResponse = await Author.delete({ where: { __id: 'author_id' } }, transaction);
  
  // Step 2: Ensure related Post records are updated
  const relatedPosts = await Post.find({ where: { authorId: 'author_id' } }, transaction);
  for (const post of relatedPosts.data) {
    await Post.delete({ where: { __id: post.__id } }, transaction);
  }

  await transaction.commit();
  console.log(deleteAuthorResponse);
  console.log(relatedPosts);
/*
{
  success: true,
  message: 'Records deleted successfully.'
}
{
  data: [],
  total: 0
}
*/
} catch (error) {
  await transaction.rollback();
  throw error;
}
```
This complex example demonstrates how to manage related records within a transaction during delete operations.

### Conclusion

This section covered how to delete records using the `Model` class. By understanding these methods and their parameters, you can effectively manage your application's data lifecycle with the RushDB SDK. The next sections will delve into other advanced operations and best practices.
