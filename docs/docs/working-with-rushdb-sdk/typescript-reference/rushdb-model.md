---
sidebar_position: 2
---
# Model

The `Model` class represents a data model in the RushDB SDK. It provides methods for performing CRUD operations, managing relationships, and validating records according to a defined schema.

### Type Definition
```typescript
class Model<S extends Schema = Schema, R extends Relations = Relations> extends RestApiProxy {
  private readonly label: string;
  public schema: S;
  public relationships: R;
  private validator?: Validator;

  constructor(modelName: string, schema: S, relationships: R = {} as R) {
    super();
    this.label = modelName;
    this.schema = schema;
    this.relationships = relationships;
  }
}
```

### Constructor Parameters

#### modelName

- **Type:** `string`
- **Required:** Yes

A unique string identifier for the model. It's used to reference the model within the SDK and to associate records with their corresponding model type in the database.

#### schema

- **Type:** `Schema`
- **Required:** Yes

The schema definition based on `Schema`, which dictates the structure and rules of the data stored.

#### relationships

- **Type:** `Relations`
- **Optional:** Yes

Defines how this model relates to other models, which is essential for establishing connections between different data types.

### Methods

#### find

Finds multiple records based on specified query parameters.

**Signature:**
```typescript
find(params?: SearchQuery<S> & { labels?: never }, transaction?: Transaction | string): Promise<DBRecordsArrayInstance<S>>;
```

#### findOne

Finds a single record based on specified query parameters.

**Signature:**
```typescript
findOne(params?: SearchQuery<S> & { labels?: never }, transaction?: Transaction | string): Promise<DBRecordInstance<S>>;
```

#### findById

Finds a single record by its ID.

**Signature:**
```typescript
findById(id: string, transaction?: Transaction | string): Promise<DBRecordInstance<S>>;
```

#### create

Creates a single record.

**Signature:**
```typescript
create(record: InferSchemaTypesWrite<S>, transaction?: Transaction | string, options?: { validate: boolean }): Promise<DBRecordInstance<InferSchemaTypesWrite<S>>>;
```

#### attach

Attaches a target record to the source record.

**Signature:**
```typescript
attach(sourceId: string, target: RelationTarget, transaction?: Transaction | string): Promise<ApiResponse<{ message: string }>>;
```

#### detach

Detaches a target record from the source record.

**Signature:**
```typescript
detach(sourceId: string, target: RelationTarget, transaction?: Transaction | string): Promise<ApiResponse<{ message: string }>>;
```

#### updateById

Updates a single record by its ID.

**Signature:**
```typescript
updateById(id: string, record: InferSchemaTypesWrite<S>, transaction?: Transaction | string, options?: { validate: boolean }): Promise<DBRecordInstance<S>>;
```

#### createMany

Creates multiple records in a single operation.

**Signature:**
```typescript
createMany(records: InferSchemaTypesWrite<S>[], transaction?: Transaction | string, options?: { validate: boolean }): Promise<DBRecordsArrayInstance<S>>;
```

#### delete

Deletes multiple records based on specified query parameters.

**Signature:**
```typescript
delete<T extends InferSchemaTypesWrite<S> = InferSchemaTypesWrite<S>>(params?: Omit<SearchQuery<T>, 'labels'>, transaction?: Transaction | string): Promise<ApiResponse<{ message: string }>>;
```

#### validate

Validates a record according to the schema rules.

**Signature:**
```typescript
validate(data: InferSchemaTypesWrite<S>): Promise<unknown>;
```

### Example Usage
```typescript
const Author = new Model('author', {
  name: { type: 'string' },
  email: { type: 'string', uniq: true }
});

// Find multiple records
Author.find({ where: { name: { $contains: 'Doe' } } }).then(records => {
  console.log(records.data);
  // Expected output: Array of authors with name containing 'Doe'
});

// Find one record
Author.findOne({ where: { email: 'john.doe@example.com' } }).then(record => {
  console.log(record.data);
  // Expected output: Single author record with email 'john.doe@example.com'
});

// Find by ID
Author.findById('some-id').then(record => {
  console.log(record.data);
  // Expected output: Author record with the specified ID
});

// Create a record
Author.create({ name: 'John Doe', email: 'john.doe@example.com' }).then(record => {
  console.log(record.data);
  // Expected output: Newly created author record
});

// Attach a related record
const Blog = new Model('blog', {
  title: { type: 'string' },
  description: { type: 'string' }
});

Author.attach('author-id', { model: 'blog', __id: 'blog-id' }).then(response => {
  console.log(response.message);
  // Expected output: "Attached successfully" or similar message
});

// Detach a related record
Author.detach('author-id', { model: 'blog', __id: 'blog-id' }).then(response => {
  console.log(response.message);
  // Expected output: "Detached successfully" or similar message
});

// Update a record by ID
Author.updateById('some-id', { name: 'John Smith' }).then(record => {
  console.log(record.data);
  // Expected output: Updated author record with name 'John Smith'
});

// Create multiple records
Author.createMany([
  { name: 'John Doe', email: 'john.doe@example.com' },
  { name: 'Jane Doe', email: 'jane.doe@example.com' }
]).then(records => {
  console.log(records.data);
  // Expected output: Array of newly created author records
});

// Delete records
Author.delete({ where: { name: { $contains: 'Doe' } } }).then(response => {
  console.log(response.message);
  // Expected output: "Deleted successfully" or similar message
});
```
