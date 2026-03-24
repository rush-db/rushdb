---
sidebar_position: 4
---

# SearchQuery

`SearchQuery` is a dictionary type that defines the structure for querying [records](../../concepts/records) in RushDB. It provides a flexible way to filter, sort, paginate, and aggregate data. For more information on search concepts, see the [search documentation](../../concepts/search/introduction.md).

## Query Shape

```python
search_query = {
    "labels":    [...],   # list[str]  — filter by record type(s); multi-label = OR
    "where":     {...},   # dict       — filter conditions; see Where Clause below
    "aggregate": {...},   # dict       — aggregation map; see Aggregation below
    "groupBy":   [...],   # list[str]  — shapes aggregate output; see GroupBy below
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

| Field   | Type  | Description                              |
|---------|-------|------------------------------------------|
| `limit` | `int` | Maximum number of records to return      |
| `skip`  | `int` | Number of records to skip (for paging)   |

### Order

```python
{ "orderBy": { "createdAt": "desc" } }  # field → direction
{ "orderBy": "asc" }                     # global direction
```

### Where Clause

The `where` dictionary filters records based on property values and relationships.

#### Number Operators

| Operator | Meaning               |
|----------|-----------------------|
| `$gt`    | Greater than          |
| `$gte`   | Greater than or equal |
| `$lt`    | Less than             |
| `$lte`   | Less than or equal    |
| `$ne`    | Not equal             |
| `$in`    | Matches any in list   |
| `$nin`   | Matches none in list  |
| `$exists`| Field exists / absent |

```python
{ "where": { "age": { "$gte": 21, "$lt": 65 } } }
{ "where": { "score": { "$in": [10, 20, 30] } } }
```

#### String Operators

| Operator      | Meaning                              |
|---------------|--------------------------------------|
| `$contains`   | Substring match (case-insensitive)   |
| `$startsWith` | Prefix match (case-insensitive)      |
| `$endsWith`   | Suffix match (case-insensitive)      |
| `$ne`         | Not equal                            |
| `$in`         | Matches any value in list            |
| `$nin`        | Matches none of the values           |
| `$exists`     | Field exists / absent                |

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
{ "where": { "age":  { "$type": "number" } } }    # "string"|"number"|"boolean"|"datetime"|"null"|"vector"
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

| Operator | Meaning                        |
|----------|--------------------------------|
| `$and`   | All conditions must match      |
| `$or`    | At least one must match        |
| `$not`   | Condition must NOT match       |
| `$nor`   | None of the conditions match   |
| `$xor`   | Exactly one condition matches  |

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

# $alias — name a traversal for use in aggregate / groupBy
result = db.records.find({
    "labels": ["COMPANY"],
    "where": {
        "EMPLOYEE": { "$alias": "$employee" }
    },
    "aggregate": {
        "headcount": { "fn": "count", "unique": True, "alias": "$employee" }
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

## Aggregation

Each key of the `aggregate` dict maps to either an **inline ref** or an **aggregation function**.

### Inline Refs

Copy a field value into the output row without applying a function:

```python
"aggregate": {
    "companyName":   "$record.name",     # root-label field
    "projectBudget": "$record.budget"    # another root field
}
```

### Aggregation Functions

| Function      | Description                                         |
|---------------|-----------------------------------------------------|
| `count`       | Count matching records                              |
| `sum`         | Sum a numeric field                                 |
| `avg`         | Average a numeric field                             |
| `min`         | Minimum value of a field                            |
| `max`         | Maximum value of a field                            |
| `collect`     | Gather field values into a list                     |
| `timeBucket`  | Group records into time buckets (time-series)       |

`alias` defaults to `"$record"` for root-label fields; set it to the `$alias` declared in `where` for related nodes.

```python
# Per-company employee statistics
result = db.records.find({
    "labels": ["COMPANY"],
    "where": { "EMPLOYEE": { "$alias": "$employee" } },
    "aggregate": {
        "companyName":   "$record.name",
        "headcount":    { "fn": "count", "unique": True,             "alias": "$employee" },
        "totalWage":    { "fn": "sum",   "field": "salary",          "alias": "$employee" },
        "avgSalary":    { "fn": "avg",   "field": "salary",          "alias": "$employee", "precision": 0 },
        "minSalary":    { "fn": "min",   "field": "salary",          "alias": "$employee" },
        "maxSalary":    { "fn": "max",   "field": "salary",          "alias": "$employee" }
    }
})
```

### Collect Options

| Option    | Type      | Description                              |
|-----------|-----------|------------------------------------------|
| `field`   | `str`     | Field to gather; omit to collect records |
| `unique`  | `bool`    | Deduplicate (default `True`)             |
| `limit`   | `int`     | Max items in the collected list          |
| `skip`    | `int`     | Skip N items in the collected list       |
| `orderBy` | `dict`    | Sort collected items                     |

```python
"employeeNames": {
    "fn": "collect",
    "field": "name",
    "alias": "$employee",
    "unique": True,
    "orderBy": { "name": "asc" },
    "limit": 10
}
```

### Nested Collect

Nest `collect` inside another `collect` for hierarchical output. Only `fn: "collect"` is valid inside a nested `aggregate` block.

```python
org_tree = db.records.find({
    "labels": ["COMPANY"],
    "where": {
        "DEPARTMENT": {
            "$alias": "$dept",
            "PROJECT": { "$alias": "$proj" }
        }
    },
    "aggregate": {
        "company": "$record.name",
        "departments": {
            "fn": "collect",
            "alias": "$dept",
            "aggregate": {
                "projects": {
                    "fn": "collect",
                    "alias": "$proj",
                    "orderBy": { "name": "asc" },
                    "limit": 20
                }
            }
        }
    }
})
# Output: [{ "company": "Acme", "departments": [{ "name": "Eng", "projects": [...] }, ...] }]
```

### TimeBucket — Time-Series Aggregation

```python
# Daily order count
daily = db.records.find({
    "labels": ["ORDER"],
    "aggregate": {
        "day":   { "fn": "timeBucket", "field": "createdAt", "granularity": "day",   "alias": "$record" },
        "count": { "fn": "count",                                                      "alias": "$record" }
    },
    "groupBy": ["day"],
    "orderBy": { "day": "asc" }
})

# Monthly revenue
monthly = db.records.find({
    "labels": ["ORDER"],
    "aggregate": {
        "month":   { "fn": "timeBucket", "field": "issuedAt", "granularity": "month", "alias": "$record" },
        "revenue": { "fn": "sum",         "field": "amount",                           "alias": "$record" }
    },
    "groupBy": ["month"],
    "orderBy": { "month": "asc" }
})

# Bi-monthly buckets
bi_monthly = db.records.find({
    "labels": ["ORDER"],
    "aggregate": {
        "period": { "fn": "timeBucket", "field": "issuedAt", "granularity": "months", "size": 2, "alias": "$record" },
        "count":  { "fn": "count",                                                                 "alias": "$record" }
    },
    "groupBy": ["period"],
    "orderBy": { "period": "asc" }
})
```

`granularity` options: `"day"`, `"week"`, `"month"`, `"quarter"`, `"year"`, `"months"`, `"hours"`, `"minutes"`, `"seconds"` (plural forms accept a `"size"` for custom window widths).

## GroupBy

`groupBy` operates in two modes:

### Mode A — Dimensional (one row per distinct value)

Entries are `"$alias.propertyName"` strings. Each distinct value combination becomes its own output row.

```python
# Count and avg per deal stage
by_stage = db.records.find({
    "labels": ["DEAL"],
    "aggregate": {
        "count":  { "fn": "count", "alias": "$record" },
        "avgAmt": { "fn": "avg",   "field": "amount", "alias": "$record", "precision": 2 }
    },
    "groupBy": ["$record.stage"],
    "orderBy": { "count": "desc" }
})
# Output: [{ "stage": "won", "count": 42, "avgAmt": 15200.0 }, ...]

# Pivot on two keys
pivot = db.records.find({
    "labels": ["PROJECT"],
    "aggregate": { "count": { "fn": "count", "alias": "$record" } },
    "groupBy": ["$record.category", "$record.active"],
    "orderBy": { "count": "desc" }
})
```

### Mode B — Self-group (one row with global KPIs)

Put the **aggregation key names** themselves into `groupBy` (not `$alias.field` paths).

```python
kpis = db.records.find({
    "labels": ["EMPLOYEE"],
    "aggregate": {
        "totalSalary": { "fn": "sum",   "field": "salary", "alias": "$record" },
        "headcount":   { "fn": "count",                    "alias": "$record" },
        "avgSalary":   { "fn": "avg",   "field": "salary", "alias": "$record", "precision": 0 }
    },
    "groupBy": ["totalSalary", "headcount", "avgSalary"],
    "orderBy": { "totalSalary": "asc" }   # ← required for correct full-scan total
})
# Output: [{ "totalSalary": 4875000, "headcount": 95, "avgSalary": 51315 }]
```

## Critical Rules

> **Never set `limit` when `aggregate` is present** (except to cap root records in per-record flat aggregation). `limit` restricts the record scan, so `sum`/`avg`/etc. cover only the first N rows and return wrong results.
>
> ```python
> # ❌ WRONG — limit cuts the scan, totalBudget is only partial
> db.records.find({
>     "labels": ["PROJECT"],
>     "aggregate": { "totalBudget": { "fn": "sum", "field": "budget", "alias": "$record" } },
>     "groupBy": ["totalBudget"],
>     "limit": 100   # DO NOT add this
> })
>
> # ✅ CORRECT — no limit; orderBy on aggregation key triggers late ordering
> db.records.find({
>     "labels": ["PROJECT"],
>     "aggregate": { "totalBudget": { "fn": "sum", "field": "budget", "alias": "$record" } },
>     "groupBy": ["totalBudget"],
>     "orderBy": { "totalBudget": "asc" }   # triggers late ordering → correct full-scan total
> })
> ```
>
> For self-group queries, always include `orderBy` on an aggregation key to trigger late ordering (ORDER BY + LIMIT runs after the full aggregation scan).

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

### Relationship Traversal with Aggregation

```python
result = db.records.find({
    "labels": ["COMPANY"],
    "where": { "EMPLOYEE": { "$alias": "$employee", "salary": { "$gte": 50000 } } },
    "aggregate": {
        "companyName":    "$record.name",
        "headcount":     { "fn": "count", "unique": True,   "alias": "$employee" },
        "totalWage":     { "fn": "sum",   "field": "salary", "alias": "$employee" },
        "employeeNames": {
            "fn": "collect", "field": "name", "alias": "$employee",
            "unique": True, "orderBy": { "name": "asc" }, "limit": 10
        }
    }
})
```

### Time-Series (TimeBucket)

```python
result = db.records.find({
    "labels": ["ORDER"],
    "aggregate": {
        "month":   { "fn": "timeBucket", "field": "issuedAt", "granularity": "month", "alias": "$record" },
        "revenue": { "fn": "sum",         "field": "amount",                           "alias": "$record" }
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
