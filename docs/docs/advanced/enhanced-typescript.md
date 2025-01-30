---
sidebar_position: 11
---

# Enhanced TypeScript
:::note
When working with RushDB SDK, achieving perfect TypeScript contracts ensures a seamless development experience. TypeScript's strong typing system allows for precise autocomplete suggestions and error checking, particularly when dealing with complex queries and nested models. This section will guide you on how to enhance TypeScript support by defining comprehensive type definitions for your models.
:::

## Defining Comprehensive TypeScript Types

To fully leverage TypeScript's capabilities, you can define types that include all schemas you've registered with `Model`. This will allow you to perform complex queries with nested model fields, ensuring type safety and better autocompletion.

### Step 1: Create Models with Model

First, define your models using `Model`:
```typescript
import { Model } from '@rushdb/javascript-sdk'

const Author = new Model('author', {
  name: { type: 'string' },
  email: { type: 'string', uniq: true }
});

const Post = new Model('post', {
  created: { type: 'datetime', default: () => new Date().toISOString() },
  title: { type: 'string' },
  content: { type: 'string' },
  rating: { type: 'number' }
});

const Blog = new Model('blog', {
  title: { type: 'string' },
  description: { type: 'string' }
});
```

### Step 2: Create an Exportable Type for All Schemas

Next, create an exportable type that includes all the schemas defined in your application:
```typescript
export type MyModels = {
  author: typeof Author.schema
  post: typeof Post.schema
  blog: typeof Blog.schema
}
```

### Step 3: Extend the Models Interface

Add this type declaration to your project. This ensures that RushDB SDK is aware of your models:
```typescript
// index.d.ts or other d.ts file added to include in tsconfig.json

import { MyModels } from './types';

declare module '@rushdb/javascript-sdk' {
  export interface Models extends MyModels {}
}
```

### Example Usage

By following these steps, you can now write complex queries with confidence, knowing that TypeScript will help you avoid errors and provide accurate autocomplete suggestions. Here's an example demonstrating how you can leverage this setup:

#### Finding Posts Rated by a Specific Author with a Rating Above 5
```typescript
const query = await db.records.find('post', {
  where: {
    author: {
      name: { $contains: 'John' }, // Checking if the author's name contains 'John'
      post: {
        rating: { $gt: 5 } // Posts with rating greater than 5
      }
    }
  }
});
```
In this example, the `db.records.find` method allows you to use nested fields in the `where` condition, thanks to the enhanced TypeScript definitions. This ensures that you can easily and accurately query your data, leveraging the full power of TypeScript.

### Conclusion

By defining comprehensive type definitions for your models and extending the `Models` interface, you can significantly enhance your TypeScript support when working with RushDB SDK. This approach ensures type safety, better autocompletion, and a more efficient development experience.
