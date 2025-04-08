---
sidebar_position: 2
---
# Model

The `Model` class in the RushDB SDK represents a data model and provides methods for performing CRUD operations, managing relationships, and validating records based on a defined schema. It is a central component for interacting with records in RushDB.

## Type Definition
```typescript
class Model<S extends Schema = any> extends RestApiProxy {
  public readonly label: string;
  public readonly schema: S;

  /** @description
   * Type helper for a draft version of the schema.
   */
  readonly draft!: InferType<Model<S>>;

  /** @description
   * Type helper for a fully-defined record with database representation.
   */
  readonly record!: DBRecord<S>;

  /** @description
   * Type helper for a single record instance.
   */
  readonly recordInstance!: DBRecordInstance<S>;

  /** @description
   * Type helper for an array of record instances.
   */
  readonly recordsArrayInstance!: DBRecordsArrayInstance<S>;

  constructor(modelName: string, schema: S, RushDBInstance?: RushDBInstance) {
    super();
    this.label = modelName;
    this.schema = schema;

    RushDBInstance?.registerModel(this);
  }
}
```

## Constructor Parameters

### `modelName`
- **Type:** `string`
- **Required:** Yes

A unique string identifier for the model. It is used to reference the model within the SDK and associate records with their corresponding model type in the database.

### `schema`
- **Type:** `Schema`
- **Required:** Yes

Defines the structure and rules of the data stored in the model. The schema ensures that records adhere to predefined constraints.

### `RushDBInstance`
- **Type:** `RushDBInstance`
- **Optional:** Yes

An optional instance of `RushDBInstance`. If provided, the model will be automatically registered within the RushDB instance, enabling centralized model management.

## Methods

### `find`
Finds multiple records based on specified query parameters.
```typescript
find(params?: SearchQuery<S> & { labels?: never }, transaction?: Transaction | string): Promise<DBRecordsArrayInstance<S>>;
```

### `findOne`
Finds a single record based on specified query parameters.
```typescript
findOne(params?: SearchQuery<S> & { labels?: never }, transaction?: Transaction | string): Promise<DBRecordInstance<S>>;
```

### `findById`
Finds a single record by its ID.
```typescript
findById(id: string, transaction?: Transaction | string): Promise<DBRecordInstance<S>>;
```

### `findUniq`
Finds a unique record based on specified query parameters.
```typescript
findUniq(params?: SearchQuery<S> & { labels?: never }, transaction?: Transaction | string): Promise<DBRecordInstance<S>>;
```

### `create`
Creates a single record.
```typescript
create(record: InferSchemaTypesWrite<S>, transaction?: Transaction | string): Promise<DBRecordInstance<InferSchemaTypesWrite<S>>>;
```

### `set`
Sets the fields of a record by its ID.
```typescript
set(id: string, record: InferSchemaTypesWrite<S>, transaction?: Transaction | string): Promise<DBRecordInstance<S>>;
```

### `update`
Updates specific fields of a record by its ID.
```typescript
update(id: string, record: Partial<InferSchemaTypesWrite<S>>, transaction?: Transaction | string): Promise<DBRecordInstance<S>>;
```

### `attach`
Attaches a target record to the source record.
```typescript
attach(sourceId: string, target: RelationTarget, transaction?: Transaction | string): Promise<ApiResponse<{ message: string }>>;
```

### `detach`
Detaches a target record from the source record.
```typescript
detach(sourceId: string, target: RelationTarget, transaction?: Transaction | string): Promise<ApiResponse<{ message: string }>>;
```

### `createMany`
Creates multiple records in a single operation.
```typescript
createMany(records: InferSchemaTypesWrite<S>[], transaction?: Transaction | string): Promise<DBRecordsArrayInstance<S>>;
```

### `delete`
Deletes multiple records based on specified query parameters.
```typescript
delete<T extends InferSchemaTypesWrite<S> = InferSchemaTypesWrite<S>>(params?: Omit<SearchQuery<T>, 'labels'>, transaction?: Transaction | string): Promise<ApiResponse<{ message: string }>>;
```

### `deleteById`
Deletes a single record or multiple records by their ID(s).
```typescript
deleteById(idOrIds: MaybeArray<string>, transaction?: Transaction | string): Promise<ApiResponse<{ message: string }>>;
```

## Example Usage

Creating and Updating Records
```typescript
import RushDB, { Model } from '@rushdb/javascript-sdk'

const db = new RushDB('API_TOKEN')

const Author = new Model(
  'AUTHOR', 
  {
    name: { type: 'string' },
    email: { type: 'string', uniq: true }
  },
  db
);

// Create a new author
const newAuthor = await Author.create({ name: 'John Doe', email: 'john.doe@example.com' });
console.log(newAuthor.data);

// Update the author's email
await Author.update(newAuthor.id, { email: 'john.doe@newmail.com' })
```

Attaching and Detaching Related Records
```typescript
const Blog = new Model(
  'blog', 
  {
    title: { type: 'string' },
    description: { type: 'string' }
  },
  db
);

// Create new Blog
const newBlog = await Blog.create({ title: 'DS/ML Times', description: '...' });

// Attach a blog to an author
await Author.attach(newAuthor.id, newBlog);

// Detach the blog from the author
await Author.detach(newAuthor.id, newBlog);
```

Deleting Records
```typescript
// Delete an author by ID
await Author.deleteById(newAuthor.id);
```

## Additional Notes

- **Transaction Handling:** Many methods (e.g., `create`, `set`, `update`, `delete`) support transactions, allowing you to commit or roll back operations as needed.
- **Schema Validation:** The schema is automatically validated when creating or updating records to ensure compliance with predefined rules.
- **Error Handling:** Errors like `UniquenessError` are thrown when attempting to create or update records with duplicate values for unique fields.

This documentation is generated based on the actual implementation in `model.ts`, ensuring accuracy and completeness.
