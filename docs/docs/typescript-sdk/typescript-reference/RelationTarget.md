---
sidebar_position: 6
---

# RelationTarget

`RelationTarget` is a type that represents the target(s) for [relationship](../../concepts/relationships) operations like attach and detach.

## Type Definition

```typescript
export type RelationTarget =
  | DBRecordsArrayInstance<any>
  | MaybeArray<DBRecord<any>>
  | MaybeArray<DBRecordInstance<any>>
  | MaybeArray<string>
```

This union type allows for multiple ways to reference one or more target records when creating or removing relationships.

## Type Options

| Type | Description |
|------|-------------|
| `DBRecordsArrayInstance<any>` | An array instance containing multiple record instances |
| `MaybeArray<DBRecord<any>>` | A single record or array of records |
| `MaybeArray<DBRecordInstance<any>>` | A single record instance or array of record instances |
| `MaybeArray<string>` | A single record ID string or array of record ID strings |

Where `MaybeArray<T>` is defined as:

```typescript
type MaybeArray<T> = T | Array<T>
```

## Related Types

### Relation

```typescript
export type Relation = {
  sourceId: string
  sourceLabel: string
  targetId: string
  targetLabel: string
  type: string
}
```

Represents a relationship between two records.

### RelationDirection

```typescript
export type RelationDirection = 'in' | 'out'
```

Specifies the direction of a relationship.

### RelationOptions

```typescript
export type RelationOptions = {
  direction?: RelationDirection;
  type?: string
}
```

Options for creating a relationship.

### RelationDetachOptions

```typescript
export type RelationDetachOptions = {
  direction?: RelationDirection
  typeOrTypes?: MaybeArray<string>
}
```

Options for removing a relationship.

## Usage

### Single Target

```typescript
// Using a record ID string
await userInstance.attach('post_123', { type: 'AUTHORED' });

// Using a record object
const post = {
  __id: 'post_123',
  __label: 'Post',
  title: 'My Post'
};
await userInstance.attach(post, { type: 'AUTHORED' });

// Using a record instance
const postInstance = await PostModel.findById('post_123');
await userInstance.attach(postInstance, { type: 'AUTHORED' });
```

### Multiple Targets

```typescript
// Using an array of record ID strings
await userInstance.attach(['post_123', 'post_456'], { type: 'AUTHORED' });

// Using an array of record objects
const posts = [
  { __id: 'post_123', __label: 'Post', title: 'First Post' },
  { __id: 'post_456', __label: 'Post', title: 'Second Post' }
];
await userInstance.attach(posts, { type: 'AUTHORED' });

// Using an array of record instances
const firstPost = await PostModel.findById('post_123');
const secondPost = await PostModel.findById('post_456');
await userInstance.attach([firstPost, secondPost], { type: 'AUTHORED' });

// Using a DBRecordsArrayInstance from a find operation
const posts = await PostModel.find({
  where: {
    title: { $regex: '^My' }
  }
});
await userInstance.attach(posts, { type: 'AUTHORED' });
```

### Detaching Relationships

```typescript
// Detach all relationships to a specific record
await userInstance.detach('post_123');

// Detach all relationships of a specific type
await userInstance.detach('post_123', { typeOrTypes: 'AUTHORED' });

// Detach multiple relationship types
await userInstance.detach('post_123', { typeOrTypes: ['AUTHORED', 'LIKED'] });

// Detach incoming relationships only
await userInstance.detach('post_123', { direction: 'in' });
```
