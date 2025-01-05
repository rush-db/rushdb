---
sidebar_position: 5
---

# DBRecord | DBRecordProps

The `DBRecord` and `DBRecordProps` types are used to define the structure and properties of records in the RushDB SDK. These types provide a way to manage the data fields and internal properties of records.

### DBRecord

The `DBRecord` type represents a complete record, including both user-defined properties and internal metadata.

### Type Definition
```typescript
type DBRecordInternalProps<T extends FlatObject | Schema = Schema> = {
  __id: string;
  __label: string;
  __proptypes?: Record<keyof T, PropertyType>;
};

type DBRecordProps<T extends FlatObject | Schema = Schema> = {
  [K in keyof T]?: T extends Schema ? InferTypesFromSchema<T>[K] : T[K];
};

type DBRecord<T extends FlatObject | Schema = Schema> =
  DBRecordInternalProps<T> & DBRecordProps<T>;
```

### Properties

#### __id

- **Type:** `string`
- **Required:** Yes

A unique identifier for the record.

#### __label

- **Type:** `string`
- **Optional:** Yes

A label to categorize the record.

#### __proptypes

- **Type:** `Record<string, PropertyType>`
- **Optional:** Yes

Property types of the Record.

### Example Usage

Here is an example of how to define a `DBRecord`:
```typescript
const authorRecord: DBRecord<typeof AuthorSchema> = {
  __id: 'some-id',
  __label: 'author', 
  __proptypes: {
    name: 'string',
    email: 'string'
  },
  name: 'John Doe',
  email: 'john.doe@example.com'
};
```

### DBRecordProps

The `DBRecordProps` type represents the user-defined properties of a record, adhering to the schema defined for the model.

### Properties

The properties of `DBRecordProps` depend on the schema defined for the model. Each field in the schema is represented as a key in the `DBRecordProps` object.

### Example Usage

Here is an example of how to define a `DBRecordProps`:
```typescript
const authorRecordProps: DBRecordProps<typeof AuthorSchema> = {
  name: 'John Doe',
  email: 'john.doe@example.com'
};
```

In this example:
- The `DBRecord` type includes both internal metadata fields (prefixed with `__`) and user-defined fields (such as `name` and `email`).
- The `DBRecordProps` type represents only the user-defined fields, adhering to the schema defined for the model.
