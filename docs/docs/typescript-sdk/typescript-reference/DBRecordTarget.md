---
sidebar_position: 4
---

# DBRecordTarget

`DBRecordTarget` is a type that represents a target [record](../../concepts/records) for operations like set, update, attach, detach, and delete. It is commonly used when working with [relationships](../../concepts/relationships) and [property](../../concepts/properties) updates.

## Type Definition

```typescript
export type DBRecordTarget = DBRecord<any> | DBRecordInstance<any> | string
```

This union type allows for multiple ways to reference a record when performing operations on it.

## Type Options

| Type | Description |
|------|-------------|
| `DBRecord<any>` | A record object with all its properties |
| `DBRecordInstance<any>` | A record instance with methods for manipulating the record |
| `string` | A record ID string |

## Usage

This type is used in various operations that need to target a specific record:

### In Model Methods

```typescript
// Using a record ID string
await UserModel.attach({
  source: 'user_123',
  target: 'post_456',
  options: { type: 'AUTHORED' }
});

// Using a record object
const user = {
  __id: 'user_123',
  __label: 'User',
  name: 'John Doe'
};
await UserModel.attach({
  source: user,
  target: 'post_456',
  options: { type: 'AUTHORED' }
});

// Using a record instance
const userInstance = await UserModel.findById('user_123');
await UserModel.attach({
  source: userInstance,
  target: 'post_456',
  options: { type: 'AUTHORED' }
});
```

### In Record Instance Methods

While `DBRecordInstance` methods like `attach` and `detach` use `DBRecordTarget` internally, you don't need to explicitly use this type as the instance's ID is already known:

```typescript
const userInstance = await UserModel.findById('user_123');

// Using a record ID string
await userInstance.attach('post_456', { type: 'AUTHORED' });

// Using a record object
const post = {
  __id: 'post_456',
  __label: 'Post',
  title: 'My Post'
};
await userInstance.attach(post, { type: 'AUTHORED' });

// Using a record instance
const postInstance = await PostModel.findById('post_456');
await userInstance.attach(postInstance, { type: 'AUTHORED' });
```
