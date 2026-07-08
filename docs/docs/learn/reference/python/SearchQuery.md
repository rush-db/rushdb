---
sidebar_position: 4
---

# SearchQuery

`SearchQuery` is a dictionary type that defines the structure for querying [records](/learn/records-and-queries/store-records) in RushDB. It provides a flexible way to filter, sort, paginate, and shape results. For more information on search concepts, see the [search documentation](/learn/search-query).

## Query Shape

```python
search_query = {
    "labels":    [...],   # list[str]  — filter by record type(s); multi-label = OR
    "where":     {...},   # dict       — filter conditions; see Where Clause below
    "select":    {...},   # dict       — output-shaping expressions; see Select below
    "groupBy":   [...],   # list[str]  — shapes select output; see GroupBy below
    "orderBy":   {...},   # str | dict — 'asc'|'desc' or { field: 'asc'|'desc' }
    "limit":     100,     # int        — max root records (default 100, max 1000)
    "skip":      0        # int        — pagination offset
}
```

## Query Components

### Labels

```python
{ "labels": ["USER", "ADMIN"] }
```

Specifies which record types to search. Multiple labels are combined with OR. If omitted, all types are searched.

### Pagination

| Field   | Type  | Description                            |
| ------- | ----- | -------------------------------------- |
| `limit` | `int` | Maximum number of records to return    |
| `skip`  | `int` | Number of records to skip (for paging) |

### Order

```python
{ "orderBy": { "createdAt": "desc" } }  # field → direction
{ "orderBy": "asc" }                     # global direction
```

### Where Clause

The `where` dictionary filters records based on property values and relationships.

#### Number Operators

| Operator  | Meaning               |
| --------- | --------------------- |
| `$gt`     | Greater than          |
| `$gte`    | Greater than or equal |
| `$lt`     | Less than             |
| `$lte`    | Less than or equal    |
| `$ne`     | Not equal             |
| `$in`     | Matches any in list   |
| `$nin`    | Matches none in list  |
| `$exists` | Field exists / absent |

```python
{ "where": { "age": { "$gte": 21, "$lt": 65 } } }
{ "where": { "score": { "$in": [10, 20, 30] } } }
```

#### String Operators

| Operator      | Meaning                            |
| ------------- | ---------------------------------- |
| `$contains`   | Substring match (case-insensitive) |
| `$startsWith` | Prefix match (case-insensitive)    |
| `$endsWith`   | Suffix match (case-insensitive)    |
| `$ne`         | Not equal                          |
| `$in`         | Matches any value in list          |
| `$nin`        | Matches none of the values         |
| `$exists`     | Field exists / absent              |

```python
{ "where": { "name":   { "$contains":   "John"      } } }
{ "where": { "email":  { "$endsWith":   "@gmail.com" } } }
{ "where": { "status": { "$in": ["active", "pending"] } } }
```

#### Boolean Operators

```python
{ "where": { "isActive": True } }                   # direct match
{ "where": { "isActive": { "$ne": False } } }       # not equal
{ "where": { "verified": { "$exists": True } } }    # field must exist
```

#### Datetime Operators

Datetime fields support ISO 8601 exact match **or** component objects for range comparisons.

```python
# Exact ISO match
{ "where": { "createdAt": "2023-01-01T00:00:00Z" } }

# Component object — exact point in time
{ "where": { "createdAt": { "$year": 2023, "$month": 1, "$day": 1 } } }
```

Available components: `$year`, `$month`, `$day`, `$hour`, `$minute`, `$second`, `$millisecond`, `$microsecond`, `$nanosecond`.

:::warning Use component objects for range comparisons
Never use plain ISO strings with `$gt` / `$lt`:

```python
# Records created in 2024
{ "where": { "createdAt": { "$gte": { "$year": 2024 }, "$lt": { "$year": 2025 } } } }

# Records from Q1 2023
{ "where": { "issuedAt": { "$gte": { "$year": 2023, "$month": 1 }, "$lt": { "$year": 2023, "$month": 4 } } } }

# Records from a specific day
{ "where": { "eventDate": { "$gte": { "$year": 2024, "$month": 3, "$day": 15 }, "$lt": { "$year": 2024, "$month": 3, "$day": 16 } } } }

# Records from the 1990s
{ "where": { "publishedAt": { "$gte": { "$year": 1990 }, "$lt": { "$year": 2000 } } } }
```

:::

#### Type Expression

Check whether a field is stored as a specific type:

```python
{ "where": { "age":  { "$type": "number" } } }    # "string"|"number"|"boolean"|"datetime"
{ "where": { "tags": { "$type": "string" } } }
```

#### $id Operator

Filter records by their own ID without a separate lookup:

```python
# Records from a known set of IDs
result = db.records.find({
    "where": { "$id": { "$in": ["id1", "id2", "id3"] } }
})

# Filter a nested node by specific ID
result = db.records.find({
    "labels": ["COMPANY"],
    "where": {
        "EMPLOYEE": { "$id": "specific-employee-id" }
    }
})
```

#### Logical Operators

| Operator | Meaning                       |
| -------- | ----------------------------- |
| `$and`   | All conditions must match     |
| `$or`    | At least one must match       |
| `$not`   | Condition must NOT match      |
| `$nor`   | None of the conditions match  |
| `$xor`   | Exactly one condition matches |

```python
# Implicit AND (multiple keys at same level)
{ "where": { "status": "active", "age": { "$gte": 18 } } }

# Explicit $and
{ "where": { "$and": [{ "status": "active" }, { "age": { "$gte": 18 } }] } }

# $or
{ "where": { "$or": [{ "status": "active" }, { "status": "pending" }] } }

# $not
{ "where": { "$not": { "status": "deleted" } } }

# $nor — none of these statuses
{ "where": { "$nor": [{ "status": "deleted" }, { "status": "archived" }] } }

# $xor — exactly one must be true
{ "where": { "$xor": [{ "isPremium": True }, { "hasFreeTrialAccess": True }] } }
```

#### Relationship Traversal

Any key in a `where` block that is a label name (not an operator) is interpreted as a related-record traversal:

```python
# Filter by related record properties
result = db.records.find({
    "labels": ["COMPANY"],
    "where": {
        "DEPARTMENT": {               # traverse to related DEPARTMENT records
            "name": "Engineering",
            "headcount": { "$gte": 10 }
        }
    }
})

# Multi-level nesting
result = db.records.find({
    "labels": ["COMPANY"],
    "where": {
        "DEPARTMENT": {
            "name": "Engineering",
            "PROJECT": {              # DEPARTMENT → PROJECT
                "status": "active"
            }
        }
    }
})

# $alias — name a traversal for use in select / groupBy
result = db.records.find({
    "labels": ["COMPANY"],
    "where": {
        "EMPLOYEE": { "$alias": "$employee" }
    },
    "select": {
        "headcount": { "$count": "$employee" }
    }
})

# $relation — constrain relationship type and/or direction
result = db.records.find({
    "labels": ["USER"],
    "where": {
        "POST": {
            "$relation": { "type": "AUTHORED", "direction": "in" },
            "title": { "$contains": "Graph" }
        }
    }
})
# Shorthand (type only): "$relation": "AUTHORED"
```

##### Variable-length traversal (`hops`)

Add `hops` to `$relation` to match records up to N hops away. A number means exactly N hops; a `{"min": ..., "max": ...}` dict is a range (`min` defaults to `1`). `type` and `direction` apply to every hop; property criteria and the block's label constrain only the final record — intermediates are anonymous. `max` is capped per deployment (`RUSHDB_MAX_TRAVERSAL_HOPS`, default 10); omitting it requests unbounded traversal, allowed only on self-hosted deployments or projects with a custom Neo4j instance:

```python
# Employees whose management chain (up to 4 hops) contains Alice
result = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {
        "EMPLOYEE": {
            "$relation": {
                "type": "REPORTS_TO",
                "direction": "out",
                "hops": { "min": 1, "max": 4 }
            },
            "name": { "$contains": "Alice" }
        }
    }
})
```

##### Cycle detection (`$cycle`)

`$cycle` is a record-level predicate that matches records on a closed path back to themselves (rings, circular ownership). Its value is the traversal spec itself — `type`, `direction`, `hops` — with `hops` mandatory (`min` ≥ 2, defaults to 2). It accepts nothing else — no `$alias`, no property criteria, no nested labels; intermediate node labels are unconstrained. It composes under `$not`/`$or`/`$and`, and can be placed inside a label block to anchor the cycle at a related record:

```python
# Accounts on a directed transfer ring of 2–6 hops
result = db.records.find({
    "labels": ["ACCOUNT"],
    "where": {
        "$cycle": {
            "type": "TRANSFERRED_TO",
            "direction": "out",
            "hops": { "min": 2, "max": 6 }
        }
    }
})
```

See [Variable-Length Traversal and Cycle Detection](/learn/search-query/where-operators#variable-length-traversal-hops) for full semantics.

## Select Expressions

Each key of the `select` dict maps to either a **field reference** (string) or an **expression** (`$`-prefixed operator object).

### Field References

Copy a field value into the output row:

```python
"select": {
    "companyName":   "$record.name",     # root-label field
    "projectBudget": "$record.budget"    # another root field
}
```

### Expressions

| Expression                          | Description                                           |
| ----------------------------------- | ----------------------------------------------------- |
| `{ "$sum": expr }`                  | Sum of a numeric expression                           |
| `{ "$avg": expr, "$precision": n }` | Average with optional decimal precision               |
| `{ "$count": "*" \| expr }`         | Count: `"*"` = root records; expr = distinct values   |
| `{ "$min": expr }`                  | Minimum value                                         |
| `{ "$max": expr }`                  | Maximum value                                         |
| `{ "$divide": [expr, expr] }`       | Division                                              |
| `{ "$multiply": [expr, expr] }`     | Multiplication                                        |
| `{ "$add": [expr, expr] }`          | Addition                                              |
| `{ "$subtract": [expr, expr] }`     | Subtraction                                           |
| `{ "$ref": "key" }`                 | Reference another output key in the same `select` map |
| `{ "$collect": CollectExpr }`       | Collect related records into an array                 |
| `{ "$timeBucket": TimeBucketExpr }` | Bucket a datetime field into calendar intervals       |

```python
# Per-company employee statistics
result = db.records.find({
    "labels": ["COMPANY"],
    "where": { "EMPLOYEE": { "$alias": "$employee" } },
    "select": {
        "companyName":  "$record.name",
        "headcount":    { "$count": "$employee" },
        "totalWage":    { "$sum": "$employee.salary" },
        "avgSalary":    { "$avg": "$employee.salary", "$precision": 0 },
        "minSalary":    { "$min": "$employee.salary" },
        "maxSalary":    { "$max": "$employee.salary" }
    }
})
```

### $collect Options

Two forms — `from` (alias-based) and `label` (inline traversal, preferred for nesting):

| Option    | Type   | Description                                                             |
| --------- | ------ | ----------------------------------------------------------------------- |
| `from`    | `str`  | `"$alias"` — alias declared in `where` (alias-based form)               |
| `label`   | `str`  | Related record label to traverse to (label-based form; no alias needed) |
| `where`   | `dict` | Flat property filter on this traversal level (label-based only)         |
| `select`  | `dict` | Field projection; nested `$collect` allowed (label-based)               |
| `unique`  | `bool` | Deduplicate (default `True`)                                            |
| `limit`   | `int`  | Max items in the collected list                                         |
| `skip`    | `int`  | Skip N items in the collected list                                      |
| `orderBy` | `dict` | Sort collected items                                                    |

> Use `"$self"` in `select` to reference the current traversal level when using `label`.
> `from` and `label` are mutually exclusive.

```python
# Alias-based (requires $alias in where)
"employeeNames": {
    "$collect": {
        "from": "$employee",
        "select": { "name": "$employee.name" },
        "unique": True,
        "orderBy": { "name": "asc" },
        "limit": 10
    }
}

# Label-based ($self = current level, no $alias needed)
"employees": {
    "$collect": {
        "label": "EMPLOYEE",
        "where": { "salary": { "$gte": 50000 } },
        "select": { "name": "$self.name", "salary": "$self.salary" },
        "orderBy": { "salary": "desc" },
        "limit": 10
    }
}
```

### $collect for Hierarchies

Label-based `$collect` supports unlimited nesting via nested `select`:

```python
org_tree = db.records.find({
    "labels": ["COMPANY"],
    "select": {
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
                                "employees": {
                                    "$collect": {
                                        "label": "EMPLOYEE",
                                        "orderBy": { "salary": "desc" },
                                        "limit": 3
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
})
# Output: [{ "departments": [{ "name": "Eng", "projects": [{ "name": "Platform", "employees": [...] }] }] }]
```

### $timeBucket — Time-Series Aggregation

```python
# Daily order count
daily = db.records.find({
    "labels": ["ORDER"],
    "select": {
        "day":   { "$timeBucket": { "field": "$record.createdAt", "unit": "day" } },
        "count": { "$count": "*" }
    },
    "groupBy": ["day"],
    "orderBy": { "day": "asc" }
})

# Monthly revenue
monthly = db.records.find({
    "labels": ["ORDER"],
    "select": {
        "month":   { "$timeBucket": { "field": "$record.issuedAt", "unit": "month" } },
        "revenue": { "$sum": "$record.amount" }
    },
    "groupBy": ["month"],
    "orderBy": { "month": "asc" }
})

# Bi-monthly buckets
bi_monthly = db.records.find({
    "labels": ["ORDER"],
    "select": {
        "period": { "$timeBucket": { "field": "$record.issuedAt", "unit": "months", "size": 2 } },
        "count":  { "$count": "*" }
    },
    "groupBy": ["period"],
    "orderBy": { "period": "asc" }
})
```

`unit` options: `"day"`, `"week"`, `"month"`, `"quarter"`, `"year"`, `"hour"`, `"minute"`, `"second"`, `"months"`, `"hours"`, `"minutes"`, `"seconds"`, `"years"` (plural forms accept a `"size"` for custom window widths).

## GroupBy

`groupBy` operates in two modes:

### Mode A — Dimensional (one row per distinct value)

Entries are `"$alias.propertyName"` strings. Each distinct value combination becomes its own output row.

```python
# Count and avg per deal stage
by_stage = db.records.find({
    "labels": ["DEAL"],
    "select": {
        "count":  { "$count": "*" },
        "avgAmt": { "$avg": "$record.amount", "$precision": 2 }
    },
    "groupBy": ["$record.stage"],
    "orderBy": { "count": "desc" }
})
# Output: [{ "stage": "won", "count": 42, "avgAmt": 15200.0 }, ...]

# Pivot on two keys
pivot = db.records.find({
    "labels": ["PROJECT"],
    "select": { "count": { "$count": "*" } },
    "groupBy": ["$record.category", "$record.active"],
    "orderBy": { "count": "desc" }
})
```

### Mode B — Self-group (one row with global KPIs)

Put the **select output key names** themselves into `groupBy` (not `$alias.field` paths).

```python
kpis = db.records.find({
    "labels": ["EMPLOYEE"],
    "select": {
        "totalSalary": { "$sum": "$record.salary" },
        "headcount":   { "$count": "*" },
        "avgSalary":   { "$avg": "$record.salary", "$precision": 0 }
    },
    "groupBy": ["totalSalary", "headcount", "avgSalary"],
    "orderBy": { "totalSalary": "asc" }   # ← required for correct full-scan total
})
# Output: [{ "totalSalary": 4875000, "headcount": 95, "avgSalary": 51315 }]
```

## Critical Rules

> **Never set `limit` when `select` is present** (except to cap root records in per-record flat queries). `limit` restricts the record scan, so `$sum`/`$avg`/etc. cover only the first N rows and return wrong results.
>
> ```python
> # ❌ WRONG — limit cuts the scan, totalBudget is only partial
> db.records.find({
>     "labels": ["PROJECT"],
>     "select": { "totalBudget": { "$sum": "$record.budget" } },
>     "groupBy": ["totalBudget"],
>     "limit": 100   # DO NOT add this
> })
>
> # ✅ CORRECT — no limit; orderBy on select key triggers late ordering
> db.records.find({
>     "labels": ["PROJECT"],
>     "select": { "totalBudget": { "$sum": "$record.budget" } },
>     "groupBy": ["totalBudget"],
>     "orderBy": { "totalBudget": "asc" }   # triggers late ordering → correct full-scan total
> })
> ```
>
> For self-group queries, always include `orderBy` on a `select` output key to trigger late ordering (ORDER BY + LIMIT runs after the full aggregation scan).

## Usage Examples

### Basic Filter

```python
result = db.records.find({
    "labels": ["USER"],
    "where": { "age": { "$gte": 30 } }
})
```

### Complex Logical Filter

```python
result = db.records.find({
    "labels": ["USER"],
    "where": {
        "$and": [
            { "active": True },
            {
                "$or": [
                    { "email": { "$endsWith": "@gmail.com" } },
                    { "email": { "$endsWith": "@outlook.com" } }
                ]
            }
        ]
    }
})
```

### Datetime Range

```python
result = db.records.find({
    "labels": ["ORDER"],
    "where": {
        "createdAt": { "$gte": { "$year": 2024 }, "$lt": { "$year": 2025 } }
    }
})
```

### Filter by Record ID

```python
result = db.records.find({
    "where": { "$id": { "$in": ["id1", "id2", "id3"] } }
})
```

### Relationship Traversal with Select Expressions

```python
result = db.records.find({
    "labels": ["COMPANY"],
    "where": { "EMPLOYEE": { "$alias": "$employee", "salary": { "$gte": 50000 } } },
    "select": {
        "companyName":    "$record.name",
        "headcount":     { "$count": "$employee" },
        "totalWage":     { "$sum": "$employee.salary" },
        "employeeNames": {
            "$collect": {
                "from": "$employee",
                "select": { "name": "$employee.name" },
                "unique": True, "orderBy": { "name": "asc" }, "limit": 10
            }
        }
    }
})
```

### Time-Series ($timeBucket)

```python
result = db.records.find({
    "labels": ["ORDER"],
    "select": {
        "month":   { "$timeBucket": { "field": "$record.issuedAt", "unit": "month" } },
        "revenue": { "$sum": "$record.amount" }
    },
    "groupBy": ["month"],
    "orderBy": { "month": "asc" }
})
```

### Pagination and Sorting

```python
page2 = db.records.find({
    "labels": ["PRODUCT"],
    "where": { "category": "Electronics" },
    "skip": 20,
    "limit": 20,
    "orderBy": { "price": "asc" }
})
```
