# RushDB SearchQuery Reference

This reference defines the JSON object that must be returned in `searchQuery`.

## SearchQuery Shape

```ts
type SearchQuery = {
  labels?: string[]
  where?: object
  select?: object
  aggregate?: object
  groupBy?: string[]
  orderBy?: 'asc' | 'desc' | Record<string, 'asc' | 'desc'>
  skip?: number
  limit?: number
}
```

Allowed top-level keys only:

- `labels`
- `where`
- `select`
- `aggregate`
- `groupBy`
- `orderBy`
- `skip`
- `limit`

## Labels

`labels` filters root records by record type.

```json
{
  "labels": ["PROJECT"]
}
```

For user-facing rows, choose the label whose records should appear as rows. Related labels belong in `where` traversal or `$collect`.

For "which/what <parent> has most/more/least/less/fewer/fewest <related records>" questions, choose the parent as the root label. Do not switch the root to the related/filter label just because that label owns the filtered property.

## Listing And Top-N Rows

For simple entity listings, including top-N rows ordered by a scalar property on the same label, do not use `select`.

```json
{
  "labels": ["EMPLOYEE"],
  "orderBy": {
    "salary": "desc"
  },
  "limit": 3
}
```

Use this shape for requests like:

- "Give me 3 top paid workers"
- "Top 10 projects by budget"
- "Show newest 5 orders"

This returns full records and lets the dashboard table render the normal record view.

## WHERE

`where` filters root records and traverses related records.

### Primitive Matching

```json
{
  "name": "Alice",
  "active": true,
  "age": 30
}
```

### String Operators

```json
{
  "name": { "$contains": "ali" },
  "email": { "$startsWith": "a" },
  "domain": { "$endsWith": ".com" },
  "status": { "$ne": "deleted" },
  "plan": { "$in": ["pro", "enterprise"] },
  "state": { "$nin": ["archived", "deleted"] }
}
```

String comparison operators are case-insensitive except exact equality.

For any user-typed named reference, check the property's sample values in the ontology (listed per property, often truncated with `(+N more)`). If the user's term maps to a listed value, filter by that full canonical value; otherwise use `$contains` on the likely display field (`name`, `title`, or an ontology-backed equivalent), on both root and related labels. Never exact-match raw user text against a value you have not seen in the sample list — that returns zero rows when the stored value is longer. Use exact equality only for IDs, a confirmed canonical value, or an explicit exact-match request.

### Number Operators

```json
{
  "score": { "$gt": 90 },
  "age": { "$gte": 18, "$lte": 65 },
  "qty": { "$in": [1, 2, 3] }
}
```

### Datetime Operators

ISO 8601 strings work for equality, `$in`, and range operators:

```json
{
  "createdAt": { "$gte": "2026-01-01T00:00:00.000Z" }
}
```

Use component objects for calendar-semantic ranges:

```json
{
  "createdAt": {
    "$gte": { "$year": 2026, "$month": 1 },
    "$lt": { "$year": 2026, "$month": 2 }
  }
}
```

Supported datetime components:

- `$year`
- `$month`
- `$day`
- `$hour`
- `$minute`
- `$second`
- `$millisecond`
- `$microsecond`
- `$nanosecond`

If the user gives a relative range such as "last 7 days", compute the UTC boundary from the current date provided by the runtime context if available. If no current date is present, use a warning and avoid inventing a date.

### Existence And Type

```json
{
  "email": { "$exists": true },
  "deletedAt": { "$exists": false },
  "budget": { "$type": "number" }
}
```

Supported `$type` values:

- `string`
- `number`
- `boolean`
- `datetime`
- `null`
- `vector`

### Logical Operators

```json
{
  "$and": [{ "active": true }, { "score": { "$gte": 70 } }],
  "$or": [{ "plan": "pro" }, { "plan": "enterprise" }],
  "$not": { "status": "deleted" },
  "$nor": [{ "status": "deleted" }, { "status": "archived" }],
  "$xor": [{ "trial": true }, { "paid": true }]
}
```

Logical operators can appear at any nesting level.

### Relationship Traversal

To traverse to related records, use the related label name as a key in `where`.

```json
{
  "labels": ["DEPARTMENT"],
  "where": {
    "PROJECT": {
      "status": "active"
    }
  }
}
```

Use `$alias` when a related record must be referenced in `select` or `groupBy`.

```json
{
  "labels": ["DEPARTMENT"],
  "where": {
    "PROJECT": {
      "$alias": "$project"
    }
  },
  "select": {
    "department": "$record.name",
    "budget": { "$sum": "$project.budget" }
  },
  "groupBy": ["$record.name"],
  "orderBy": { "budget": "desc" }
}
```

Use `$relation` only to constrain relationship type or direction when the user explicitly asks for it and the ontology supports the path.

```json
{
  "where": {
    "POST": {
      "$relation": { "type": "AUTHORED", "direction": "in" }
    }
  }
}
```

`$relation` can also be a relationship type string.

Never use `$label`, `$direction`, `$as`, `$of`, or `$through`.

### Related-Count Ranking

Use this shape when the user asks which root entity has the most, more, least, less, fewer, or fewest related records, optionally with filters on the related records.

```json
{
  "labels": ["<PARENT_LABEL>"],
  "where": {
    "<RELATED_LABEL>": {
      "$alias": "$related",
      "<relatedFilterField>": true
    }
  },
  "select": {
    "parent": "$record.<displayField>",
    "related_count": {
      "$count": "$related"
    }
  },
  "groupBy": ["$record.<displayField>"],
  "orderBy": {
    "related_count": "asc"
  }
}
```

Direction mapping: most/more/highest/largest/greatest => `"desc"`; least/less/fewer/fewest/lowest/smallest => `"asc"`.

Only use this shape when the ontology shows the parent label, related label, display field, and traversal path. If the traversal path is absent, do not silently root on the related label; return the closest valid query with a warning.

### ID Filter

```json
{
  "$id": { "$in": ["id1", "id2"] }
}
```

```json
{
  "EMPLOYEE": {
    "$id": "specific-id"
  }
}
```

## SELECT

`select` shapes output. Each key maps to an expression.

Do not use `select` merely to choose visible columns for a simple listing. Use `select` when the requested result requires computed fields, aggregate metrics, grouped rows, nested `$collect` output, or another non-record response shape.

Expression forms:

```ts
"$record.field"
"$alias.field"
"$self.field"
42
true
{ "$ref": "otherSelectKey" }
{ "$sum": expr }
{ "$avg": expr, "$precision": 2 }
{ "$count": "*" }
{ "$count": expr }
{ "$min": expr }
{ "$max": expr }
{ "$divide": [expr, expr] }
{ "$multiply": [expr, expr] }
{ "$add": [expr, expr] }
{ "$subtract": [expr, expr] }
{ "$collect": collectExpr }
{ "$timeBucket": timeBucketExpr }
```

Examples:

```json
{
  "select": {
    "name": "$record.name",
    "total": { "$sum": "$record.amount" },
    "count": { "$count": "*" },
    "avg": { "$avg": "$record.value", "$precision": 2 }
  }
}
```

Derived metrics:

```json
{
  "select": {
    "revenue": { "$sum": "$record.amount" },
    "cost": { "$sum": "$record.cost" },
    "profit": { "$subtract": [{ "$ref": "revenue" }, { "$ref": "cost" }] }
  },
  "groupBy": ["profit"],
  "orderBy": { "profit": "desc" }
}
```

Math inside aggregation:

```json
{
  "select": {
    "total": {
      "$sum": {
        "$multiply": ["$record.price", "$record.quantity"]
      }
    }
  },
  "groupBy": ["total"],
  "orderBy": { "total": "desc" }
}
```

## GROUPBY

`groupBy` is valid only with `select`.

Do not use alias-only values in `groupBy`. `"$record"` and `"$employee"` are invalid because grouped rows need a concrete dimension field. Use a property reference such as `"$record.name"` or `"$employee.status"`, or use a select output key such as `"totalBudget"`.

### Dimensional Grouping

Use field references to produce one row per distinct value.

```json
{
  "labels": ["PROJECT"],
  "select": {
    "count": { "$count": "*" },
    "avgBudget": { "$avg": "$record.budget", "$precision": 2 }
  },
  "groupBy": ["$record.status"],
  "orderBy": { "count": "desc" }
}
```

The returned group key appears without the alias prefix.

### Self Grouping

Use select key names to collapse to one metric row.

```json
{
  "labels": ["PROJECT"],
  "select": {
    "totalBudget": { "$sum": "$record.budget" }
  },
  "groupBy": ["totalBudget"],
  "orderBy": { "totalBudget": "desc" }
}
```

For self-group metric queries, include `orderBy` on a select key.

### Ranking Related Metrics By Root Entity

For "top departments by project budget", root is `DEPARTMENT`, project budget is related:

```json
{
  "labels": ["DEPARTMENT"],
  "where": {
    "PROJECT": {
      "$alias": "$project"
    }
  },
  "select": {
    "department": "$record.name",
    "budget": { "$sum": "$project.budget" }
  },
  "groupBy": ["$record.name"],
  "orderBy": { "budget": "desc" }
}
```

Only use aliases and fields that exist in the ontology.

## ORDERBY

Valid forms:

```json
"asc"
```

```json
"desc"
```

```json
{
  "budget": "desc"
}
```

For aggregate/select output, order by a select key.

Invalid form:

```json
{
  "property": "budget",
  "direction": "desc"
}
```

## LIMIT And SKIP

- `limit` is valid for listing queries.
- `limit` maximum is 1000.
- Do not include `limit` or `skip` for scan-level aggregate `select` queries, unless making a per-root-record shaped listing where limiting root records is explicitly intended.
- For top-N aggregation, prefer `orderBy` on a select key. Include `limit` only when the engine is expected to return the top N groups after aggregation.

## COLLECT

Use `$collect` to build nested arrays from related records.

For top-N related records per root record, put `orderBy` and `limit` inside `$collect`. Do not add root `groupBy`, and do not order the root query by the collected array field.

`$collect.label` follows direct relationships from the current root/current `$self`. For indirect paths, put the traversal in `where`, alias the target label, and collect with `$collect.from`.

```json
{
  "labels": ["PROJECT"],
  "select": {
    "project_name": "$record.name",
    "top_paid_employees": {
      "$collect": {
        "label": "EMPLOYEE",
        "select": {
          "employee_name": "$self.name",
          "salary": "$self.salary"
        },
        "orderBy": {
          "salary": "desc"
        },
        "limit": 3
      }
    }
  }
}
```

Indirect alias-based form:

```json
{
  "labels": ["DEPARTMENT"],
  "where": {
    "PROJECT": {
      "EMPLOYEE": {
        "$alias": "$employee"
      }
    }
  },
  "select": {
    "department_name": "$record.name",
    "top_paid_employees": {
      "$collect": {
        "from": "$employee",
        "select": {
          "employee_name": "$employee.name",
          "salary": "$employee.salary"
        },
        "orderBy": { "salary": "desc" },
        "limit": 3
      }
    }
  }
}
```

Label-based form:

```json
{
  "labels": ["COMPANY"],
  "select": {
    "company": "$record.name",
    "departments": {
      "$collect": {
        "label": "DEPARTMENT",
        "select": {
          "name": "$self.name",
          "projects": {
            "$collect": {
              "label": "PROJECT",
              "select": {
                "name": "$self.name",
                "budget": "$self.budget"
              }
            }
          }
        }
      }
    }
  }
}
```

Alias-based form:

```json
{
  "where": {
    "USER": {
      "$alias": "$user"
    }
  },
  "select": {
    "users": {
      "$collect": {
        "from": "$user",
        "select": {
          "id": "$user.id",
          "name": "$user.name"
        },
        "orderBy": { "name": "asc" },
        "limit": 10
      }
    }
  }
}
```

Supported `$collect` options:

- `label`
- `from`
- `where`
- `select`
- `orderBy`
- `limit`
- `skip`

## TIMEBUCKET

```json
{
  "select": {
    "month": {
      "$timeBucket": {
        "field": "$record.createdAt",
        "unit": "month"
      }
    },
    "count": { "$count": "*" }
  },
  "groupBy": ["month"],
  "orderBy": { "month": "asc" }
}
```

Supported units:

- `second`
- `minute`
- `hour`
- `day`
- `week`
- `month`
- `quarter`
- `year`
- plural forms with `size`, such as `months` with `size: 2`

## AGGREGATE

Use `aggregate` only for vector similarity when supported by the ontology and request.

```json
{
  "aggregate": {
    "similarity": {
      "fn": "vector.similarity.cosine",
      "field": "embedding",
      "query": [1, 2, 3],
      "alias": "$record"
    }
  }
}
```

All ordinary metrics must use `select`, not `aggregate`.

## Validation Checklist

Before returning JSON:

- Output has exactly `searchQuery` and `warnings`.
- `searchQuery` uses only allowed top-level keys.
- Every label exists in ontology.
- Every property exists in ontology.
- Every related traversal uses a label key from ontology.
- Every alias referenced in `select` or `groupBy` is declared with `$alias` in `where`.
- `$record.field` references a root-label field.
- `$alias.field` references a field on the aliased related label.
- `select` expressions use valid operators.
- `groupBy` is absent unless `select` exists.
- `orderBy` uses `"asc"`, `"desc"`, or `{ "field": "asc" | "desc" }`.
- `limit` is absent for pure aggregation unless top-N grouped output is explicitly requested.
- No unsupported traversal operators are present.
