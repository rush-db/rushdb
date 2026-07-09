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

All label, field, and relationship-type names in this document are placeholders — real names come exclusively from the schema, never from these examples. Labels are case-sensitive: copy them exactly as spelled in the schema and never change their case.

Predicate values must be **literals** (strings, numbers, booleans, dates) or sample values from the schema. A `where` value can never be a field or alias reference: `{ "field": "$record.id" }`, `{ "field": { "$eq": "$alias.id" } }`, and `{ "field": { "$ref": "$record.id" } }` are all invalid — `$ref` exists only in `select` expressions, and a plain reference is matched as a literal string and returns nothing. Correlated comparisons between two records (a "join on" a field) are not supported in `where` in any syntax; `$record.*`, `$alias.*`, and `$relation` references belong in `select`, `groupBy`, and `aggregate` only. To rank or group by a scalar field that is not a relationship in the schema, root on the label that owns the field and `groupBy` that field instead of building a traversal.

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

For any user-typed named reference, check the property's sample values in the schema (listed per property, often truncated with `(+N more)`). If the user's term maps to a listed value, filter by that full canonical value; otherwise use `$contains` on the label's display property as shown in the schema — often `name` or `title`, but never assume such a field exists; if the schema shows none, pick from its string properties. Applies to both root and related labels. Never exact-match raw user text against a value you have not seen in the sample list — that returns zero rows when the stored value is longer. Use exact equality only for IDs, a confirmed canonical value, or an explicit exact-match request.

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

Use `$relation` only to constrain relationship type or direction when the user explicitly asks for it and the schema supports the path.

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

`type` must be copied **verbatim** from a schema Relationships row — never invent a semantic type like the examples here, and never write `__RUSHDB__RELATION__DEFAULT__` unless a schema row shows that exact type. When it does — data imported as nested JSON often has only such edges — either use that exact string or omit `type` entirely (untyped traversal is valid, including with `hops`).

Each Relationships row in `schemaMarkdown` is a directed pattern rooted at that label: `(SELF)-[:TYPE]->(OTHER)` is outgoing and `(SELF)<-[:TYPE]-(OTHER)` is incoming. To traverse, nest `OTHER` as a label key under `where` (add `$alias` if it is referenced in `select`/`groupBy`); to pin the edge set `$relation: { "type": "TYPE", "direction": ... }` with `"out"` for `->` and `"in"` for `<-`. Only patterns shown in the schema are traversable — a scalar `*_id` property is a plain value, not an edge, so never nest a label to "join" on it.

### Variable-Length Traversal (`hops`)

Add `hops` to `$relation` when the user asks about reachability across more than one hop of the **same pattern** — hierarchies ("any ancestor/descendant", "the whole reporting chain"), "within N degrees/steps", or transitive closure over one relationship type. `hops` is a number (exactly that many hops) or `{ "min": …, "max": … }` (range; `min` defaults to 1). The endpoint label constrains only the **final** record; intermediate records are unconstrained.

```json
{
  "labels": ["EMPLOYEE"],
  "where": {
    "EMPLOYEE": {
      "$alias": "$manager",
      "$relation": { "type": "REPORTS_TO", "direction": "out", "hops": { "min": 1, "max": 4 } },
      "name": { "$contains": "Alice" }
    }
  }
}
```

Rules:

- Keep `max` as small as the request allows; deep undirected traversal is expensive. Omit `type` only when the user genuinely means "any relationship" — and keep `direction` if the schema gives one, since it prunes the search.
- Always set `hops.max` (an integer as small as the request allows). Omitting `max` means unbounded traversal, which is rejected on shared cloud connections and only accepted on self-hosted/dedicated database setups; `hops.max` is capped per deployment (default 10).
- For prompts like "find/list <records> N hops away from <named origin>", root on the records being returned, not on the named origin. Put the named origin filter inside the same-label traversal block and reverse the direction relative to travel from the origin: if the user means records reachable by following outgoing edges from the origin, the candidate root must have an incoming multihop path from that origin (`direction: "in"`). Example: "depots 1-2 hops away from Depot North" where `Depot North -> South -> East` returns South/East with root `DEPOT` and nested `DEPOT` filtered to Depot North using `direction: "in"`. Do not root on Depot North, because then `total` counts only the origin record.
- Filters and aggregations on the endpoint (`$alias` + `select`) apply per **path**: `$count`/`$collect` deduplicate, but `$sum`/`$avg` over a multihop alias count one row per path — prefer counting to summing across multihop endpoints.
- One hop is still the default: never add `hops` for a plain related-record condition.

### Cycle Detection (`$cycle`)

Use the `$cycle` operator when the user asks for rings, loops, circular flows, or records that "come back to themselves" (fraud rings, circular ownership, dependency cycles). `$cycle` is a record-level predicate — its value is the traversal spec itself (`type`, `direction`, `hops`; `hops` is mandatory, `min` ≥ 2, defaulting to 2). A cycle has no separate endpoint: no `$alias`, no property criteria, no nested labels.

```json
{
  "labels": ["ACCOUNT"],
  "where": {
    "$cycle": { "type": "TRANSFERRED_TO", "direction": "out", "hops": { "min": 2, "max": 6 } }
  }
}
```

This returns every ACCOUNT sitting on a directed transfer ring of 2–6 hops. Set `direction` for flow-like semantics (money, ownership); an undirected cycle also matches innocent back-and-forth pairs. Wrap in `$not` to express "records NOT on a cycle"; place inside a label block (`"ACCOUNT": { "country": "US", "$cycle": {...} }`) to anchor the cycle at the related record. Intermediate node labels are unconstrained. Paths may revisit a record through different relationships (only relationships are unique per path).

Never wrap `$cycle` in a named block and never nest `$relation` inside it — the operator's value IS the relation spec, and it sits directly at a `where` level like any other operator.

Never use `$label`, `$direction`, `$as`, `$of`, `$through`, or `$hops` (hops lives inside `$relation`).

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

Only use this shape when the schema shows the parent label, related label, display field, and traversal path. If the traversal path is absent, do not silently root on the related label; return the closest valid query with a warning.

Exception: this shape requires the traversal itself to express the requested condition. If the condition ("won", "authored", "assigned", …) is recorded only as a scalar property on the related label — typically a reference-style `*_id` name such as `author_id`, but any property counts, whatever its naming (`authorRef`, `writtenBy`, …); the test is that no schema Relationships row expresses the condition — do not use this shape. A traversal cannot state it and `where` cannot correlate the property with the parent (no `$record.*`/`$alias.*` values, no `$ref`). Root on the label that owns the property, `groupBy` that property, count records, and add a warning that rows are keyed by the raw property value:

```json
{
  "labels": ["<RELATED_LABEL>"],
  "select": {
    "parent": "$record.<parentIdField>",
    "count": { "$count": "*" }
  },
  "groupBy": ["$record.<parentIdField>"],
  "orderBy": { "count": "desc" }
}
```

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

The group key is any property the schema lists — a display property works the same way whatever it is called (`$record.title` on a label whose display property is `title`, `$record.name` when it is `name`).

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

Only use aliases and fields that exist in the schema.

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

Use `aggregate` only for vector similarity when supported by the schema and request.

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
- Every label exists in schema.
- Every property exists in schema.
- Every related traversal uses a label key from schema.
- Every alias referenced in `select` or `groupBy` is declared with `$alias` in `where`.
- `$record.field` references a root-label field.
- `$alias.field` references a field on the aliased related label.
- `select` expressions use valid operators.
- `groupBy` is absent unless `select` exists.
- `orderBy` uses `"asc"`, `"desc"`, or `{ "field": "asc" | "desc" }`.
- `limit` is absent for pure aggregation unless top-N grouped output is explicitly requested.
- No unsupported traversal operators are present.
