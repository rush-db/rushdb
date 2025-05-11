---
sidebar_position: 7
---

# Aggregations

SearchQuery provides powerful aggregation capabilities that allow you to perform calculations and collect data from your records and their relationships.

#### Aggregation Placement in SearchQuery DTO

All aggregation clauses are defined in the `aggregate` key of the SearchQuery DTO, which is at the same level as other query parameters such as `where`, `limit`, `skip`, `orderBy`, and `labels`:

```typescript
// SearchQuery
{
  labels: ['COMPANY'],       // Record labels to search
  where: { /* conditions */ },  // Filtering conditions
  limit: 10,                 // Results limit
  skip: 0,                   // Results offset
  orderBy: { name: 'asc' },  // Sorting
  aggregate: {               // Aggregation definitions
    count: {
      fn: 'count',
      alias: '$record'
    },
    // More aggregations...
  }
}
```

> **Note**: Aggregations are applied only when fetching records and have no effect on other endpoints supporting SearchQuery. The main goal of aggregations is to fetch Records in the desired shape, optimizing data retrieval and transformation in a single query.


## Available Aggregation Functions

The following aggregation functions are supported:

- `avg` - Calculate average value of a numeric field
- `count` - Count records (with optional `uniq` parameter)
- `max` - Get maximum value from a field
- `min` - Get minimum value from a field
- `sum` - Calculate sum of a numeric field
- `collect` - Gather field values or entire records into an array
- `gds.similarity.*` - Calculate vector similarity using various algorithms:
  - `cosine` - Cosine similarity [-1,1]
  - `euclidean` - Euclidean distance normalized to (0,1]
  - `euclideanDistance` - Raw euclidean distance [0,∞)
  - `jaccard` - Jaccard similarity [0,1]
  - `overlap` - Overlap coefficient [0,1]
  - `pearson` - Pearson correlation [-1,1]


## Aliases

Every aggregation clause requires an `alias` parameter that specifies which record from graph traversal should be used. To reference fields from related records in aggregations, you need to define aliases in the `where` clause using the `$alias` parameter. By default, the root record has alias `$record`:

```typescript
{
  labels: ['COMPANY'],
  where: {
    DEPARTMENT: {
      $alias: '$department',
      PROJECT: {
        $alias: '$project',
        EMPLOYEE: {
          $alias: '$employee'
        }
      }
    }
  },
  aggregate: {
    // Referencing to root record using '$record' alias
    companyName: '$record.name',
    // Now can use $employee in aggregations
    avgSalary: {
      fn: 'avg',
      field: 'salary',
      alias: '$employee'
    }
  }
}
```

## Basic Aggregations

Example topology:

```mermaid
graph LR
  A[COMPANY] --has--> B[EMPLOYEE]
```

###  avg
**Parameters:**
- `fn`: 'avg' - The aggregation function name
- `field`: string - The field to calculate average for
- `alias`: string - The record alias to use
- `precision?`: number - Optional decimal precision for the result

```typescript
{
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee',
      salary: {
        $gte: 50000  // Filter employees by salary
      }
    }
  },
  aggregate: {
    avgSalary: {
      fn: 'avg',
      field: 'salary',
      alias: '$employee',
      precision: 2  // Optional: Set precision for the result
    }
  }
}
```

###  count
**Parameters:**
- `fn`: 'count' - The aggregation function name
- `alias`: string - The record alias to use
- `field?`: string - Optional field to count
- `uniq?`: boolean - Optional flag to count unique values

```typescript
{
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee'
    }
  },
  aggregate: {
    employeesCount: {
      fn: 'count',
      uniq: true,  // Count unique employees
      alias: '$employee'
    }
  }
}
```

###  max
**Parameters:**
- `fn`: 'max' - The aggregation function name
- `field`: string - The field to find maximum value from
- `alias`: string - The record alias to use

```typescript
{
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee'
    }
  },
  aggregate: {
    maxSalary: {
      fn: 'max',
      field: 'salary',
      alias: '$employee'
    }
  }
}
```

###  min
**Parameters:**
- `fn`: 'min' - The aggregation function name
- `field`: string - The field to find minimum value from
- `alias`: string - The record alias to use

```typescript
{
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee'
    }
  },
  aggregate: {
    minSalary: {
      fn: 'min',
      field: 'salary',
      alias: '$employee'
    }
  }
}
```

###  sum
**Parameters:**
- `fn`: 'sum' - The aggregation function name
- `field`: string - The field to calculate sum for
- `alias`: string - The record alias to use

```typescript
{
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee'
    }
  },
  aggregate: {
    totalWage: {
      fn: 'sum',
      field: 'salary',
      alias: '$employee'
    }
  }
}
```

###  collect
**Parameters:**
- `fn`: 'collect' - The aggregation function name
- `alias`: string - The record alias to use
- `field?`: string - Optional field to collect (if not provided, collects entire records)
- `uniq?`: boolean - Optional flag to collect unique values only. True by default.
- `limit?`: number - Optional maximum number of items to collect
- `skip?`: number - Optional number of items to skip
- `orderBy?`: TSearchSort - Optional sorting configuration

```typescript
{
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee'
    }
  },
  aggregate: {
    employeeNames: {
      fn: 'collect',
      field: 'name',
      alias: '$employee',
      uniq: true  // Optional: true by default
    }
  }
}
```

---

### Complete Example

```typescript
{
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee',  // Define alias for employee records
      salary: {
        $gte: 50000  // Filter employees by salary
      }
    }
  },
  aggregate: {
    // Use field directly from record
    companyName: '$record.name',

    // Count unique employees using the defined alias
    employeesCount: {
      fn: 'count',
      uniq: true,
      alias: '$employee'
    },

    // Calculate total salary using the defined alias
    totalWage: {
      fn: 'sum',
      field: 'salary',
      alias: '$employee'
    },

    // Collect unique employees names
    employeeNames: {
      fn: 'collect',
      field: 'name',
      alias: '$employee'
    },

    // Get average salary with precision
    avgSalary: {
      fn: 'avg',
      field: 'salary',
      alias: '$employee',
      precision: 0
    },

    // Get min and max salary
    minSalary: {
      fn: 'min',
      field: 'salary',
      alias: '$employee'
    },
    maxSalary: {
      fn: 'max',
      field: 'salary',
      alias: '$employee'
    }
  }
}
```

<details>

<summary>Example data and response</summary>

```typescript
// Company record
{
  __id: "018838b8-2e1f-7000-8000-a29392548450",
  __label: "COMPANY",
  name: "TechCorp",
  stage: ["seed"]
}

// Employee records
[
  {
    __id: "018838b8-2e1f-7000-8000-b45fd8932abc",
    __label: "EMPLOYEE",
    name: "John Doe",
    salary: 550000
  },
  {
    __id: "018838b8-2e1f-7000-8000-c67de9043def",
    __label: "EMPLOYEE",
    name: "Jane Smith",
    salary: 600000
  }
]

// Query result:
{
  data: [{
    __id: "018838b8-2e1f-7000-8000-a29392548450",
    __label: "COMPANY",
    companyName: "TechCorp",
    employeesCount: 2,
    totalWage: 1150000,
    avgSalary: 575000,
    minSalary: 550000,
    maxSalary: 600000,
    employeeNames: ["Jane Smith", "John Doe"]
  }],
  total: 1,
  success: true
}
```

</details>



## Nested Aggregations

SearchQuery supports two types of nested aggregations:

### 1. Collecting Nested Records

You can use the `collect` operator to build nested JSON structures containing arrays of related records. Due to Cypher limitations, when using nested collection, only the `collect` operator is supported at nested levels.


Example topology:

```mermaid
graph LR
  A[COMPANY] --has--> B[DPEARTMENT]
  B --has--> C[PROJECT]
  C --has--> D[EMPLOYEE]
```

Example with nested where clauses and corresponding aggregations:

```typescript
{
  labels: ['COMPANY'],
  where: {
    DPEARTMENT: {
      $alias: '$department',  // Level 1 alias
      PROJECT: {
        $alias: '$project',   // Level 2 alias
        EMPLOYEE: {
          $alias: '$employee', // Level 3 alias
          salary: {
            $gte: 100000      // Filter condition
          }
        }
      }
    }
  },
  aggregate: {
    departments: {
      fn: 'collect',
      alias: '$department',   // Use Level 1 alias
      aggregate: {
        projects: {
          fn: 'collect',
          alias: '$project',  // Use Level 2 alias
          orderBy: {
            projectName: 'asc'
          },
          aggregate: {
            employees: {
              fn: 'collect',
              alias: '$employee', // Use Level 3 alias
              orderBy: {
                salary: 'desc'
              },
              limit: 3
            }
          }
        }
      }
    }
  }
}
```

<details>

<summary>Example data and response</summary>

```typescript
// Company record
{
  __id: "018838b8-2e1f-7000-8000-a29392548450",
  __label: "COMPANY",
  name: "TechCorp",
  rating: 4
}

// Department records
[
  {
    __id: "018838b8-2e1f-7000-8000-d89ef1154abc",
    __label: "departments",
    name: "Engineering"
  },
  {
    __id: "018838b8-2e1f-7000-8000-e92fg2265bcd",
    __label: "departments",
    name: "Sales"
  }
]

// Project records
[
  {
    __id: "018838b8-2e1f-7000-8000-f34gh3376cde",
    __label: "projects",
    projectName: "Mobile App"
  },
  {
    __id: "018838b8-2e1f-7000-8000-g56hi4487def",
    __label: "projects",
    projectName: "Web Platform"
  }
]

// Employee records
[
  {
    __id: "018838b8-2e1f-7000-8000-h78ij5598efg",
    __label: "employees",
    name: "John Doe",
    salary: 500000
  },
  {
    __id: "018838b8-2e1f-7000-8000-i90kl6609fgh",
    __label: "employees",
    name: "Jane Smith",
    salary: 550000
  },
  {
    __id: "018838b8-2e1f-7000-8000-j12mn7710ghi",
    __label: "employees",
    name: "Bob Wilson",
    salary: 600000
  }
]

// Query result with nested collection:
{
  data: [{
    __id: "018838b8-2e1f-7000-8000-a29392548450",
    __label: "COMPANY",
    departments: [{
      __id: "018838b8-2e1f-7000-8000-d89ef1154abc",
      name: "Engineering",
      projects: [{
        __id: "018838b8-2e1f-7000-8000-f34gh3376cde",
        projectName: "Mobile App",
        employees: [
          {
            __id: "018838b8-2e1f-7000-8000-j12mn7710ghi",
            name: "Bob Wilson",
            salary: 600000
          },
          {
            __id: "018838b8-2e1f-7000-8000-i90kl6609fgh",
            name: "Jane Smith",
            salary: 550000
          },
          {
            __id: "018838b8-2e1f-7000-8000-h78ij5598efg",
            name: "John Doe",
            salary: 500000
          }
        ]
      }]
    }]
  }],
  total: 1,
  success: true
}
```
</details>


### 2. Aggregating Values from Nested Records

Example topology:

```mermaid
graph LR
  A[COMPANY] --has--> B[PROJECT]
  B --has--> C[EMPLOYEE]
```

Example with deep nested aggregation:

```typescript
{
  labels: ['COMPANY'],
  where: {
    PROJECT: {
      EMPLOYEE: {
        $alias: '$employee',
        salary: {
          $gte: 50000  // Filter condition
        }
      }
    }
  },
  aggregate: {
    avgEmployeeSalary: {
      fn: 'avg',
      field: 'salary',
      alias: '$employee' // Use alias from deepest level
    }
  }
}
```

<details>

<summary>Example data and response</summary>

```typescript
// Company record
{
  __id: "018838b8-2e1f-7000-8000-a29392548450",
  __label: "COMPANY",
  name: "TechCorp",
  rating: 4
}

// Project records
[
  {
    __id: "018838b8-2e1f-7000-8000-f34gh3376cde",
    __label: "PROJECT",
    name: "Mobile App"
  }
]

// Employee records under projects
[
  {
    __id: "018838b8-2e1f-7000-8000-h78ij5598efg",
    __label: "EMPLOYEE",
    name: "John Doe",
    salary: 500000
  },
  {
    __id: "018838b8-2e1f-7000-8000-i90kl6609fgh",
    __label: "EMPLOYEE",
    name: "Jane Smith",
    salary: 550000
  }
]

// Query result with aggregated values:
{
  data: [{
    __id: "018838b8-2e1f-7000-8000-a29392548450",
    __label: "COMPANY",
    avgEmployeeSalary: 525000
  }],
  total: 1,
  success: true
}
```

</details>

## Collect Operator Options

The `collect` operator supports additional options for pagination and sorting:

- `limit` - Maximum number of records to collect
- `skip` - Number of records to skip
- `orderBy` - Sort collected records by specified fields
- `uniq` - Collect only unique values (when collecting field values)
- `field` - Collect specific field values instead of entire records

Example:
```typescript
{
  labels: ['COMPANY'],
  where: {
    DEPARTMENT: {
      $alias: '$department'
    }
  },
  aggregate: {
    // Collect unique tags from departments
    tags: {
      fn: 'collect',
      alias: '$department',
      field: 'tags',    // Collect only tags field
      uniq: true,       // Remove duplicates
      limit: 100,       // Collect up to 100 tags
      orderBy: {        // Sort alphabetically
        name: 'asc'
      }
    }
  }
}
```

<details>

<summary>Example data and response</summary>

```typescript
// Company record
{
  __id: "018838b8-2e1f-7000-8000-a29392548450",
  __label: "COMPANY",
  name: "TechCorp"
}

// Department records
[
  {
    __id: "018838b8-2e1f-7000-8000-d89ef1154abc",
    __label: "DEPARTMENT",
    name: "Engineering",
    tags: ["tech", "development", "agile"]
  },
  {
    __id: "018838b8-2e1f-7000-8000-e92fg2265bcd",
    __label: "DEPARTMENT",
    name: "Sales",
    tags: ["sales", "business", "development"]
  }
]

// Query result with collected tags:
{
  data: [{
    __id: "018838b8-2e1f-7000-8000-a29392548450",
    __label: "COMPANY",
    tags: ["agile", "business", "development", "sales", "tech"]
  }],
  total: 1,
  success: true
}
```

</details>




## Vector Similarity Aggregations

Example topology:

```mermaid
graph LR
  A[DOCUMENT] --has--> B[CHUNK]
```

###  gds.similarity.*
**Parameters:**
- `fn`: 'gds.similarity.[algorithm]' - The similarity algorithm to use
  - `cosine` - Cosine similarity [-1,1]
  - `euclidean` - Euclidean distance normalized to (0,1]
  - `euclideanDistance` - Raw euclidean distance [0,∞)
  - `jaccard` - Jaccard similarity [0,1]
  - `overlap` - Overlap coefficient [0,1]
  - `pearson` - Pearson correlation [-1,1]
- `field`: string - The vector field to compare
- `alias`: string - The record alias to use
- `query`: number[] - The query vector to calculate similarity against


Example showing vector search with where clause and similarity aggregation:

```typescript
{
  labels: ['DOCUMENT'],
  where: {},
  aggregate: {
    // Calculate similarity score using root level alias
    similarity: {
      fn: 'gds.similarity.cosine',
      field: 'embedding',
      query: [1, 2, 3, 4, 5],
      alias: '$record'
    }
  }
}
```

<details>

<summary>Example data and response</summary>

```typescript
// Document record
{
  __id: "018838b8-2e1f-7000-8000-k34op8821hij",
  __label: "DOCUMENT",
  title: "Machine Learning Basics"
}

// Chunk records
[
  {
    __id: "018838b8-2e1f-7000-8000-l56pq9932ijk",
    __label: "CHUNK",
    content: "Introduction to neural networks",
    embedding: [1.2, 0.5, -0.3, 0.8, 0.1]
  },
  {
    __id: "018838b8-2e1f-7000-8000-m78rs0043jkl",
    __label: "CHUNK",
    content: "Deep learning architectures",
    embedding: [0.9, 0.4, -0.2, 0.7, 0.3]
  }
]

// Query result with similarity scores:
{
  data: [{
    __id: "018838b8-2e1f-7000-8000-k34op8821hij",
    __label: "DOCUMENT",
    similarity: 0.82,
    chunks: [
      {
        __id: "018838b8-2e1f-7000-8000-l56pq9932ijk",
        __label: "CHUNK",
        content: "Introduction to neural networks",
        similarity: 0.78
      },
      {
        __id: "018838b8-2e1f-7000-8000-m78rs0043jkl",
        __label: "CHUNK",
        content: "Deep learning architectures",
        similarity: 0.65
      }
    ]
  }],
  total: 1,
  success: true
}
```
</details>


