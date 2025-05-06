---
sidebar_position: 10
---

# Models
# Defining Models with RushDB SDK
:::note
In this section, we focus on how to define models using the RushDB SDK. Defining models accurately is crucial as it not only aids in validating the fields according to the schema but also enhances the developer experience with features like autocomplete and field name suggestions.
:::

## Table of Contents

- [Understanding Schema](#understanding-schema)
- [Creating a Model](#creating-a-model-with-model)
- [Registering and Managing Models](#registering-and-managing-models)

## Understanding Schema

The `Schema` is at the core of model definitions in RushDB. It specifies the structure and constraints of the data fields within your model. Here's a breakdown of the properties you can define within a `Schema`:

```typescript
type Schema = Record<string, {
    default?: SchemaDefaultValue;
    multiple?: boolean;
    required?: boolean;
    type: PropertyType;
    uniq?: boolean;
}>;
```

**Schema Properties Explained:**

- `default`: This is the initial value of the field if no value is provided during record creation. It can be a static value or a function that returns a value asynchronously, allowing for dynamic default values.
- `multiple`: Indicates whether the field can hold multiple values (array) or just a single value.
- `required`: Specifies whether a field is mandatory. If set to true, you cannot create a record without providing a value for this field.
- `type`: Defines the data type of the field. The type determines the available search operators and how data is validated and stored. Possible types include:
  - `boolean`
  - `datetime` (can be either a detailed object or an ISO string)
  - `null`
  - `number`
  - `string`
- `uniq`: If set to true, the field must have a unique value across all records in the database, useful for fields like email addresses or custom identifiers.

### Creating a Model with Model

With an understanding of `Schema`, you can define a model in the RushDB system. Here’s how to define a simple `Author` model:
```typescript
const Author = new Model('author', {
  name: { type: 'string' },
  email: { type: 'string', uniq: true }
});

```

**Model Constructor Parameters:**

- `modelName`: A unique string identifier for the model. It's used to reference the model within the SDK and to associate records with their corresponding model type in the database.
- `schema`: The schema definition based on `Schema`, which dictates the structure and rules of the data stored.
- `relationships` (optional): Defines how this model relates to other models, which is essential for establishing connections between different data types:
```typescript
type Relations = Record<string, {
    model: string;
}>;
```

### Registering and Managing Models

Once you define a model, you must register it with the RushDB SDK to make it operational:

```typescript
const AuthorRepo = db.registerModel(Author);
```
**Working with Models in RushDB class:**

- `registerModel`: Registers the model with the RushDB SDK, making it ready for data operations:
- `getModel` and `getModels`: Retrieve registered models from the SDK, useful for accessing model details programmatically:
```
  public getModel(label: string): Model
  public getModels(): Map<string, Model>
```

### Conclusion

Defining models with `Model` and `Schema` sets a robust foundation for your application's data architecture. It enables strong type-checking, validation, and inter-model relationships, enhancing the robustness and scalability of your applications. In subsequent sections, we will explore how to interact with these models to create, retrieve, update, and delete records.


--- 

## CRUD Operations without Model Registration
:::note
While registering models with `Model` provides strong TypeScript support and a clear structure, you can also perform CRUD operations directly using the `RestAPI` methods in the `RushDB` class. This approach allows you to interact with your data without predefining models, offering flexibility for quick operations or dynamic use cases.
:::

## Creating Records without Model Registration

The `create` method allows you to create a single record without registering a model.

### Example

Creating an author record directly:
```typescript
const newAuthor = await db.records.create('author', {
  name: 'Alice Smith',
  email: 'alice.smith@example.com',
  jobTitle: 'writer',
  age: 28,
  married: true,
  dateOfBirth: '1993-05-15T00:00:00Z'
});
```

## Reading Records without Model Registration

The `find`, `findOne`, and `findById` methods let you read records from the database without predefining models.

### Example

Finding records with specific criteria:
```typescript
const authors = await db.records.find('author', {
    where: {
        jobTitle: { $contains: 'writer' },
        age: { $gte: 25 }
    }
});
```

### Example

Finding a single record:
```typescript
const author = await db.records.findOne('author', {
    where: {
        email: { $contains: 'alice.smith@example.com' }
    }
});
```

## Updating Records without Model Registration

You can update records using the `update` method, which allows for modifications without registered models.

### Example

Updating a record directly:
```typescript
const author = await db.records.findOne('author', {
    where: {
        email: { $contains: 'alice.smith@example.com' }
    }
});

await db.records.update(author.data.__id, {
  jobTitle: 'senior writer'
});

```

## Deleting Records without Model Registration

The `delete` and `deleteById` methods enable you to remove records from the database without using registered models.

### Example

Deleting records with specific criteria:
```typescript
await db.records.delete('author', {
  where: {
    jobTitle: { $contains: 'writer' }
  }
});
```

### Example

Deleting records by ID:
```typescript
await db.records.deleteById('author', ['author_id_1', 'author_id_2']);
```

## Conclusion

Using the `RestAPI` methods in the `RushDB` class provides a flexible way to perform CRUD operations without registering models. This approach is particularly useful for dynamic or ad-hoc operations, offering a straightforward way to interact with your data.
