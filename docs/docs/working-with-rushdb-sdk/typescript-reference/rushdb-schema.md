---
sidebar_position: 1
---

# Schema

The `Schema` type is used to define the schema for a model in the RushDB SDK. It describes the structure and validation rules for the data fields of a model.

### Type Definition
```typescript
type Schema = Record<
  string,
  {
    default?: SchemaDefaultValue;
    multiple?: boolean;
    required?: boolean;
    type: PropertyType;
    uniq?: boolean;
  }
>;
```

### Properties

#### default

- **Type:** `MaybePromise<PropertyValue>`
- **Optional:** Yes

The initial value of the field if no value is provided during record creation. It can be a static value or a function that returns a value asynchronously, allowing for dynamic default values.

#### multiple

- **Type:** `boolean`
- **Optional:** Yes

Indicates whether the field can hold multiple values (array) or just a single value.

#### required

- **Type:** `boolean`
- **Optional:** Yes

Specifies whether a field is mandatory. If set to `true`, you cannot create a record without providing a value for this field.

#### type

- **Type:** `'boolean' | 'datetime' | 'null' | 'number' | 'string'`
- **Optional:** No

Defines the data type of the field. The type determines the available search operators and how data is validated and stored.

#### uniq

- **Type:** `boolean`
- **Optional:** Yes

If set to `true`, the field must have a unique value across all records in the database, useful for fields like email addresses or custom identifiers.

### Example Usage

Here is an example of how to define a schema using `Schema`:
```typescript
const AuthorSchema: Schema = {
  name: { type: 'string', required: true },
  email: { type: 'string', uniq: true },
  age: { type: 'number', default: 30 },
  isActive: { type: 'boolean', default: true }
};
```

In this example:
- `name` is a required string field.
- `email` is a unique string field.
- `age` is a number field with a default value of 30.
- `isActive` is a boolean field with a default value of true.
