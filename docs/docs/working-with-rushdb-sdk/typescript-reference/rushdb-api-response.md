---
sidebar_position: 10
---

# ApiResponse | CollectSDKResult

## ApiResponse
The `ApiResponse` type defines the structure of the response returned by the RushDB SDK's API methods. It encapsulates the data returned by the API, as well as metadata about the success of the request and any additional information.

### Type Definition
```typescript
export type ApiResponse<T, E = Record<string, any>> = {
  data: T;
  success: boolean;
  total?: number;
} & E;
```

### Properties

#### data

- **Type:** `T`
- **Required:** Yes
- **Description:** The actual data returned by the API.

#### success

- **Type:** `boolean`
- **Required:** Yes
- **Description:** Indicates whether the API request was successful.

#### total

- **Type:** `number`
- **Required:** No
- **Description:** The total number of items, useful for paginated responses.

### Example Usage
```typescript
// Define a response type for a list of users
type UserApiResponse = ApiResponse<User[]>;

// Example API response
const response: UserApiResponse = {
  data: [
    { id: '1', name: 'John Doe', email: 'john.doe@example.com' },
    { id: '2', name: 'Jane Doe', email: 'jane.doe@example.com' }
  ],
  success: true,
  total: 2
};

// Accessing the data
console.log(response.data); // [{ __id: '1', name: 'John Doe', email: 'john.doe@example.com' }, { __id: '2', name: 'Jane Doe', email: 'jane.doe@example.com' }]
console.log(response.success); // true
console.log(response.total); // 2
```

## CollectSDKResult

The `CollectSDKResult` type is a utility type that flattens the structure of the result returned by the SDK methods. It ensures the correct type inference for complex nested structures.

### Type Definition
```typescript
export type CollectSDKResult<T extends (...args: any[]) => Promise<any>> = FlattenTypes<
  Awaited<ReturnType<T>>
>;
```

### Properties

The `CollectSDKResult` type does not have explicit properties since it is used to infer types from the SDK methods.

### Example Usage
```typescript
// Define a model with specific schema
const Author = new Model('author', {
  name: { type: 'string' },
  email: { type: 'string', uniq: true }
});

// Define a function to create an author and return a promise
const createAuthor = async () => {
  return await Author.create({ name: 'John Doe', email: 'john.doe@example.com' });
};

// Use CollectSDKResult to infer the return type of the function
type CreateAuthorResult = CollectSDKResult<typeof createAuthor>;

// Example usage of inferred type
const result: CreateAuthorResult = await createAuthor();

console.log(result); // { __id: '1', name: 'John Doe', email: 'john.doe@example.com' }
```