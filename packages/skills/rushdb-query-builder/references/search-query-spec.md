# RushDB SearchQuery — Complete Reference

> Ported from `packages/mcp-server/searchQuerySpec.ts`. Load this file before calling `findRecords` with dates, aggregation, groupBy, relationship traversal, or vector search.

---

## Table of Contents

1. [SearchQuery Shape](#searchquery-shape)
2. [WHERE — Filters & Traversal](#1-where--filters--traversal)
3. [SELECT — Expression-Based Output Shaping](#2-select--expression-based-output-shaping)
4. [GROUPBY — Two Modes](#3-groupby--two-modes)
5. [COLLECT — Array Gathering](#4-collect--array-gathering)
6. [LIMIT Rules by Query Mode](#5-limit-rules-by-query-mode)
7. [Metric Field Discovery Across Labels](#6-metric-field-discovery-across-related-labels)
8. [Range / Distribution Queries](#7-range--distribution-queries)
9. [Multi-Label Filter Distribution](#8-multi-label-filter-distribution)
10. [Enum / Value Normalization](#9-enum--value-normalization)
11. [Relationship & Path Queries](#10-relationship--path-queries)
12. [NL → WHERE Quick Reference](#11-nl--where-translation-quick-reference)
13. [Validation Checklist](#12-validation-checklist)
14. [Example Patterns](#13-example-patterns)

---

## SearchQuery Shape

```
labels?    string[]      — filter by record type(s); multi-label = OR
where?     object        — filter conditions; see §1
select?    object        — expression-based output shaping; see §2
groupBy?   string[]      — shapes select output; see §3
orderBy?   string|object — 'asc'|'desc' or { field:'asc'|'desc' }
limit?     number        — max root records (default 100; max 1000)
skip?      number        — pagination offset
```

`labels` defines root records only. For relationship-aware analytics, do not put related labels in `labels`; put related labels inside `where` traversal blocks and add `$alias` when they are referenced by `select` or `groupBy`.

For "which/what <parent> has most/more/least/less/fewer/fewest <related records>" questions, choose the parent as the root label. Do not switch the root to the related/filter label just because that label owns the filtered property.

**Critical limits:**

- NEVER include `limit` when `select` is present (`$sum`/`$avg`/`$min`/`$max`/`$count`/`$collect`/`$timeBucket`). `limit` restricts the record scan → results are mathematically wrong. e.g. "total budget of all 33 projects" with `limit:10` returns only the sum of the first 10.
- Self-group and dimensional groupBy queries: omit `limit` entirely (or scope to root records only).
- `limit` is valid only for listing/browsing queries or per-record flat aggregation (one row per root record).

---

## §1) WHERE — Filters & Traversal

The where clause mechanism: when a nested object key is NOT a criteria operator (like `$gt`, `$contains`, etc.) and NOT a flat value, RushDB interprets that key as the **LABEL** of a related record to traverse.

### Primitive Value Matching

```js
name: 'John Doe' // exact string (case-sensitive equality)
name: {
  $eq: 'John Doe'
} // exact string alias, useful for MongoDB-style queries
isActive: true // boolean
isActive: {
  $eq: true
} // exact boolean alias
age: 30 // number
age: {
  $eq: 30
} // exact number alias
created: '2023-01-01T00:00:00Z' // ISO 8601 datetime
```

### String Operators

```js
name: {
  $contains: 'John'
} // substring match (case-insensitive)
name: {
  $startsWith: 'J'
} // prefix match (case-insensitive)
name: {
  $endsWith: 'son'
} // suffix match (case-insensitive)
name: {
  $eq: 'John'
} // exact equality alias (case-sensitive)
name: {
  $ne: 'deleted'
} // not equal
status: {
  $in: ['active', 'pending']
} // matches any value in array
status: {
  $nin: ['deleted', 'archived']
} // matches none of these values
```

For any user-typed named reference, default to `$contains` on a likely display field (`name`, `title`, or schema-backed equivalent), on both root and related labels. Use exact equality only when the value is an ID or the user explicitly requests an exact match; confirm canonical values with discovery rather than guessing an exact string.

### Number Operators

```js
age: {
  $gt: 18
} // greater than
age: {
  $gte: 21
} // greater than or equal
age: {
  $lt: 65
} // less than
age: {
  $lte: 64
} // less than or equal
age: {
  $eq: 18
} // exact equality alias
age: {
  $ne: 18
} // not equal
age: {
  $in: [20, 30, 40]
} // matches any of these numbers
age: {
  $nin: [20, 30, 40]
} // matches none of these numbers
```

### Boolean Operators

```js
isActive: true // direct match
isActive: {
  $eq: true
} // exact equality alias
isActive: {
  $ne: false
} // not equal (matches true or unset)
```

### Datetime Operators

ISO 8601 strings work with all operators — equality, `$in`, and range comparisons (`$gt`/`$gte`/`$lt`/`$lte`). Use component objects when you need calendar-semantic boundaries (year / month / day) that are easier to express as calendar units than as a computed UTC timestamp.

```js
// ISO 8601 strings — valid for equality, $in, and range operators:
created: "2023-01-01T00:00:00Z"
created: { $in: ["2023-01-01T00:00:00Z", "2023-02-01T00:00:00Z"] }
created: { $gte: "2023-01-01T00:00:00Z" }              // "after a timestamp"

// Relative ranges ("last 7 days"): compute the ISO UTC boundary client-side → use with $gte/$lte:
created: { $gte: "2026-04-23T00:00:00Z" }

// Component objects — use for calendar-semantic boundaries:
created: { $year: 2023, $month: 1, $day: 1 }           // exact point in time
// Available: $year $month $day $hour $minute $second $millisecond $microsecond $nanosecond

// Year "in 1994":     { field: { $gte: { $year:1994 }, $lt: { $year:1995 } } }
// Month "Jan 1994":   { field: { $gte: { $year:1994,$month:1 }, $lt: { $year:1994,$month:2 } } }
// Day "1994-03-15":   { field: { $gte: { $year:1994,$month:3,$day:15 }, $lt: { $year:1994,$month:3,$day:16 } } }
// Decade "1990s":     { field: { $gte: { $year:1990 }, $lt: { $year:2000 } } }
```

**Month+day WITHOUT year**: unsupported — ask the user for a year. Do not mention internal reasons.

### Vector Similarity

Vector similarity is not yet available in `select`. Use the legacy `aggregate` clause ONLY for this case:

```js
aggregate: {
  similarity: {
    fn: "vector.similarity.cosine",   // cosine | euclidean
    field: "embedding",
    query: [1, 2, 3, 4, 5],
    alias: "$record"
  }
}
// All other metrics/analytics must use select.
```

### Field Existence & Type

```js
phoneNumber: {
  $exists: true
} // only records that have this field
phoneNumber: {
  $exists: false
} // only records that do NOT have this field
age: {
  $type: 'number'
} // "string"|"number"|"boolean"|"datetime"
```

### Logical Grouping

```js
// Implicit $and (multiple keys at same level = AND):
where: { name: { $startsWith: "J" }, age: { $gte: 21 } }

// Explicit versions:
$and: [ { name: { $startsWith: "J" } }, { age: { $gte: 21 } } ]
$or:  [ { name: { $startsWith: "J" } }, { age: { $gte: 21 } } ]
$not: { status: "deleted" }
$nor: [ { status: "deleted" }, { status: "archived" } ]
$xor: [ { isPremium: true }, { hasFreeTrialAccess: true } ]   // exactly one must match

// Nested logical grouping:
$or: [
  { status: "active" },
  { $and: [ { status: "pending" }, { createdAt: { $gte: "2023-01-01T00:00:00Z" } } ] }
]
```

### Relationship Traversal

Traversal rule: ANY top-level key that reads as a label name (ALL_CAPS style) is interpreted as a related-record traversal. Uses OPTIONAL MATCH in Cypher — records are included even if the related record doesn't exist UNLESS you explicitly filter for it.

```js
// Basic (filter by related record properties):
where: {
  name: "Tech Corp",
  DEPARTMENT: {                        // traverse to related DEPARTMENT records
    name: "Engineering",
    headcount: { $gte: 10 }
  }
}

// Multi-level nesting (path):
where: {
  DEPARTMENT: {
    name: "Engineering",
    PROJECT: {                         // DEPARTMENT → PROJECT
      name: "Database",
      EMPLOYEE: { role: "Developer" }  // PROJECT → EMPLOYEE
    }
  }
}
```

⚠ **TRAVERSAL SYNTAX — COMMON HALLUCINATION (ALWAYS WRONG):**

```js
// WRONG:
where: { employee: { $label: "EMPLOYEE", $direction: "out", $as: "$emp" } }
// The operators $label / $direction / $as / $of / $through / $hops DO NOT EXIST.
// Multihop depth lives INSIDE $relation as hops, never as a standalone $hops operator.

// CORRECT: key IS the label; alias via $alias only
where: { EMPLOYEE: { $alias: "$emp" } }
// ($relation.hops and $cycle ARE valid — see below.)
```

**`$alias`** — name a traversed node for use in `select`/`groupBy`:

```js
where: {
  DEPARTMENT: {
    $alias: "$department",
    PROJECT: {
      $alias: "$project",
      EMPLOYEE: { $alias: "$employee" }
    }
  }
}
```

**`$relation`** — constrain relationship type and/or direction:

```js
where: {
  POST: {
    $relation: { type: "AUTHORED", direction: "in" },  // full form
    title: { $contains: "Graph" }
  }
}
// Shorthand (type only): $relation: "AUTHORED"
// direction options: 'in' | 'out'  (omit = any direction)
```

Each Relationships row in the schema is a directed pattern rooted at that label: `(SELF)-[:TYPE]->(OTHER)` is outgoing, `(SELF)<-[:TYPE]-(OTHER)` is incoming. Map straight to `$relation: { type: "TYPE", direction }` — `"out"` for `->`, `"in"` for `<-`. Only patterns shown in the schema are traversable; a scalar `*_id` property is a plain value, not an edge.

**`$relation.hops`** — variable-length traversal (multihop over ONE pattern):

```js
where: {
  EMPLOYEE: {
    $alias: "$manager",
    $relation: { type: "REPORTS_TO", direction: "out", hops: { min: 1, max: 4 } },
    name: { $contains: "Alice" }
  }
}
// hops: 3 = exactly 3 hops; hops: { min?, max? } = range (min defaults to 1)
```

Use for hierarchies ("any ancestor/descendant", "whole reporting chain"), "within N degrees", or transitive closure over one relationship type. The endpoint label constrains only the **final** record; intermediates are unconstrained.

- Keep `max` as small as the request allows; deep undirected traversal is expensive. Omit `type` only when the user genuinely means "any relationship"; keep `direction` when the schema gives one.
- `hops.max` is capped per deployment (default 25); omitting `max` (unbounded) is only accepted on self-hosted / dedicated-database setups.
- Endpoint aggregations apply per **path**: `$count`/`$collect` deduplicate, but `$sum`/`$avg` over a multihop alias count one row per path — prefer counting to summing across multihop endpoints.
- One hop stays the default: never add `hops` for a plain related-record condition.

**`$cycle`** — cycle/ring detection (endpoint binds back to the parent record):

```js
where: {
  RING: {
    $cycle: true,
    $relation: { type: "TRANSFERRED_TO", direction: "out", hops: { min: 2, max: 6 } }
  }
}
```

Matches roots sitting on a closed path (fraud rings, circular ownership, dependency cycles). A `$cycle` block accepts **only** `$relation` (with `hops`, `min` ≥ 2 — defaults to 2): no `$alias`, no property criteria, no nested labels — a cycle has no separate endpoint. The block's key is a display name, not matched as a label. Set `direction` for flow-like semantics (money, ownership); undirected cycles also match innocent back-and-forth pairs. Wrap in `$not` for "NOT on a cycle". Paths may revisit a record via different relationships (only relationships are unique per path).

**`$id`** — filter by record ID:

```js
where: {
  $id: {
    $in: ['id1', 'id2']
  }
}
where: {
  EMPLOYEE: {
    $id: 'specific-id'
  }
}
```

### Logical Grouping With Relationships

```js
where: {
  $or: [
    { DEPARTMENT: { name: "Engineering" } },
    { DEPARTMENT: { name: "Product" } }
  ]
}

where: {
  name: "Tech Corp",
  $or: [
    { DEPARTMENT: { name: "Engineering" } },
    { DEPARTMENT: { name: "Product", $not: { PROJECT: { status: "Canceled" } } } }
  ]
}

// Logical operators INSIDE relationship blocks:
where: {
  DEPARTMENT: {
    $or: [ { name: "Engineering" }, { name: "Product" } ],
    PROJECT: {
      $and: [ { budget: { $gte: 10000 } }, { status: { $ne: "Canceled" } } ]
    }
  }
}
```

### Key Behavioral Notes

- Field names are **case-sensitive**
- Missing fields are NOT matched — `{ active: true }` skips records without an `active` field
- String operators (`$contains`, `$startsWith`, `$endsWith`) are case-insensitive
- Array fields: condition satisfied if ANY element matches (`tags:"typescript"` matches `["js","typescript"]`)
- Relationship traversal uses OPTIONAL MATCH — records are returned even if no related record exists, unless you add a property filter on that related block
- Logical operators work at ANY nesting level, including inside relationship blocks

---

## §2) SELECT — Expression-Based Output Shaping

`select` is the canonical output-shaping clause. Every key maps to an `Expr`:

### Expression Types

```
"$record.field"           — field reference (project a field value)
42 / true                 — literal number or boolean
{ $ref: "otherKey" }      — reference another select output key (for derived metrics)
{ $sum: expr }            — sum of an expression
{ $avg: expr }            — average; add $precision:N to round
{ $count: "*" | expr }    — count DISTINCT records or field values
{ $min: expr }            — minimum value
{ $max: expr }            — maximum value
{ $divide: [A, B] }       — A / B
{ $multiply: [A, B] }     — A * B
{ $add: [A, B] }          — A + B
{ $subtract: [A, B] }     — A - B
{ $collect: CollectExpr } — gather related records into an array
{ $timeBucket: TBExpr }   — time-series bucketing
```

### Basic Examples

```js
select: {
  name:     "$record.name",
  total:    { $sum: "$record.amount" },
  count:    { $count: "*" },
  avg:      { $avg: "$record.value" },
  rounded:  { $avg: "$record.value", $precision: 2 }
}
```

### Derived Metrics via `$ref` (evaluated AFTER all non-`$ref` expressions)

```js
select: {
  revenue: { $sum: "$record.amount" },
  cost:    { $sum: "$record.cost" },
  profit:  { $subtract: [{ $ref: "revenue" }, { $ref: "cost" }] },
  margin:  { $divide:   [{ $ref: "profit" },  { $ref: "revenue" }] }
}
```

### Math inside Aggregation

```js
select: {
  total: {
    $sum: {
      $multiply: ['$record.price', '$record.quantity']
    }
  }
}
```

### COLLECT from Related Node — Two Forms

**FORM A (alias-based — requires `$alias` in where):**

```js
where:  { USER: { $alias: "$user" } },
select: {
  users: {
    $collect: {
      from:    "$user",
      select:  { id: "$user.id", name: "$user.name" },
      orderBy: { name: "asc" },
      limit:   10
    }
  }
}
```

**FORM B (label-based — no `$alias` needed; `$self` = current level; supports unlimited nesting):**

```js
select: {
  departments: {
    $collect: {
      label: "DEPARTMENT",
      where: { budget: { $gte: 10000 } },  // optional flat filter on this level
      select: {
        name: "$self.name",
        projects: {
          $collect: {
            label: "PROJECT",
            select: {
              name: "$self.name",
              employees: {
                $collect: { label: "EMPLOYEE", orderBy: { salary: "desc" }, limit: 3 }
              }
            }
          }
        }
      }
    }
  }
}
```

### TIMEBUCKET

```js
select: {
  bucket: { $timeBucket: { field: "$record.createdAt", unit: "day" } },
  count:  { $count: "*" }
},
groupBy: ["bucket"]
```

`unit` options: `"day"` | `"week"` | `"month"` | `"quarter"` | `"year"` | `"months"` | `"hour"` | `"minute"` | `"second"` | `"hours"` | `"minutes"` | `"seconds"` | `"years"`. Add `size:N` when unit is a plural form (e.g. `unit:"months", size:2` = every 2 months).

---

## §3) GROUPBY — Two Modes

`groupBy` is valid only with `select`.

Do not use alias-only values in `groupBy`. `"$record"` and `"$related"` are invalid because grouped rows need a concrete dimension field. Use a property reference such as `"$record.name"` or `"$related.status"`, or use a select output key such as `"totalBudget"`.

### Mode A — Dimensional (one row per distinct value)

```js
// groupBy entries use "$alias.propertyName" format
select: { count: { $count: "*" }, avg: { $avg: "$record.total", $precision: 2 } },
groupBy: ["$record.status"],
orderBy: { count: "desc" }
// Output rows: [{ "status":"pending","count":120,"avg":310.42 }, ...]
// Note: group key appears WITHOUT alias prefix in output ($record.status → "status")
// Multiple keys = pivot: groupBy: ['$record.category','$record.active']
```

### Mode B — Self-Group (collapse to ONE row with global metric)

```js
// groupBy entries are SELECT KEY NAMES (no dot, no alias prefix)
select:  { totalBudget: { $sum: "$record.budget" } },
groupBy: ["totalBudget"]
// Output: [{ "totalBudget": 1875251446 }]
// Multiple KPIs: groupBy: ['totalRevenue','orderCount']
```

### ⚠ Late-Ordering Rule — Critical for Correct Totals

When `orderBy` references a select key, the engine applies ORDER BY + LIMIT AFTER the aggregation (full-scan first, then paginate).

When `orderBy` is absent or references a raw field, LIMIT is applied BEFORE aggregation → only the first N raw records are aggregated → **WRONG totals**.

**Fix**: for self-group and any pure metric query, always add `orderBy` on the select key:

```js
select:  { total: { $sum: "$record.amount" } },
groupBy: ["total"],
orderBy: { total: "asc" }   // ← triggers late ordering; ensures full dataset is summed
```

---

## §4) COLLECT — Array Gathering & Nested Structures

Use `select $collect` (label-based) for all nested queries:

```js
labels: ["COMPANY"],
select: {
  departments: {
    $collect: {
      label: "DEPARTMENT",
      select: {
        name: "$self.name",         // $self = current traversal level
        projects: {
          $collect: {
            label: "PROJECT",
            select: {
              name: "$self.name",
              employees: { $collect: { label: "EMPLOYEE", orderBy: { salary: "desc" }, limit: 3 } }
            }
          }
        }
      }
    }
  }
}
// → returns COMPANY → [DEPARTMENT → [PROJECT → [top-3 EMPLOYEE]]] tree; no $alias boilerplate
// → use where: { field: condition } inside any $collect level to filter that traversal level
```

**Collect options (inside `$collect`):**

| Option     | Notes                                                          |
| ---------- | -------------------------------------------------------------- |
| `label`    | related label to collect from (label-based form)               |
| `from`     | `$alias` string (alias-based form; requires `$alias` in where) |
| `where?`   | flat filter on this traversal level                            |
| `select?`  | shape the collected records (omit to collect entire records)   |
| `orderBy?` | sort collected items: `{ salary: "desc" }`                     |
| `limit?`   | max items in the array                                         |
| `skip?`    | skip N items in the collected array                            |

---

## §5) LIMIT Rules by Query Mode

| Query mode                                            | Limit                                                                           |
| ----------------------------------------------------- | ------------------------------------------------------------------------------- |
| Self-group (single KPI row)                           | NO `limit` — but MUST add `orderBy` on select key for late ordering             |
| Dimensional groupBy                                   | NO `limit` to get all groups; add `limit` + `orderBy` on select key for "top N" |
| Per-record flat aggregation (one row per root record) | `limit` IS valid (caps root records)                                            |
| Pure listing (no select)                              | `limit` always valid                                                            |
| "how many" simple count                               | Read `total` from the `findRecords` response — do NOT use `$count` for this     |

---

## §6) Metric Field Discovery Across Related Labels

If the metric field is NOT on the target label, search related labels before giving up:

1. Confirm target label. `findProperties(labels:[<target>])` — look for the metric field.
2. If absent, walk adjacent labels via `getSchemaMarkdown` or `findRelationships` with `source`/`target` endpoint probes.
3. For each candidate related label R: `findProperties(labels:[R])` and attempt the same match.
4. When found on CHILD: `where:{ CHILD:{ ...filters..., $alias:'$child' } }`, select referencing `'$child.*'`. Root-level filters stay at the top-level `where`.
5. Never abandon after one miss — always attempt at least one related-label discovery pass.

**Related-count ranking:**

For "which/what `<parent>` has most/more/least/less/fewer/fewest `<related records>`", root the parent label, traverse the related label in `where` with `$alias`, put related filters inside that related-label block, count the related alias, group by a parent display field, and order the count.

Direction: most/more/highest/largest/greatest → `desc`; least/less/fewer/fewest/lowest/smallest → `asc`.

If the parent→related traversal path is absent, do not silently root on the related label; ask or return the closest valid query with an explicit assumption.

---

## §7) Range / Distribution Queries

- **type = number or datetime** → `findRecords` with `select:{ min:{ $min:'$record.<field>' }, max:{ $max:'$record.<field>' } }` plus `groupBy:['min','max']`. Or: `getSchema` (JSON) → `propertyValues(propertyId)` → returns `{ min, max }` directly.
- **type = string or boolean** → `propertyValues(propertyId)` to list all distinct values.
- NEVER call `findRecords` with a `where` filter to "search for" values of a field — that returns records, not ranges.

---

## §8) Multi-Label Filter Distribution

Place each filter with the label that actually holds the field. On zero results, silently retry by moving the filter to the related child block before asking.

---

## §9) Enum / Value Normalization

Never hardcode guessed values for enumerated fields:

1. `findProperties` to locate the property and get its `id`.
2. `propertyValues(propertyId, { query:<user value> })` to probe existing values.
3. If no match: try case variants, abbreviations, partial prefixes.
4. Use ONLY canonical values returned by `propertyValues`.
5. Re-run `propertyValues` with empty query to list top candidates; retry once silently. Ask only if two+ equally plausible values remain.
6. Always mention assumption briefly if mapping is non-obvious, then proceed.

---

## §10) Relationship & Path Queries

### Entity Resolution by Name

Probe with `findRecords(limit:1, where:{ <nameField>:{ $contains:'...' } })`.

### Multi-Hop Relationship Discovery

Two native tools, pick by shape:

- **Same pattern repeated N times** (self-referencing hierarchy, "within N degrees", transitive closure over one relationship type) → `$relation.hops` on a single block — no intermediate wrappers, no BFS:
  ```js
  where: { EMPLOYEE: { $alias: '$report', $relation: { type: 'REPORTS_TO', direction: 'in', hops: { max: 5 } } } }
  ```
- **Different labels chained** (`PARENT→A→B→CHILD`) → nested label blocks, one per hop. Discover the chain first (steps below); `hops` does not help here because each hop is a different pattern.

Pre-check before multi-hop:

1. `findProperties(labels:[parent])` for direct scalar fields.
2. Fetch 1 sample parent record. `findRelationships` with `source.where.$id` or `target.where.$id` → discover adjacent labels.
3. If direct path to child exists: `where:{ CHILD:{ $alias:'$child' } }` — STOP. No intermediate wrappers.
4. If the schema shows a self-referencing pattern on one label: use `$relation.hops`.
5. Only if neither: BFS over distinct labels (depth ≤ 4).

**BFS algorithm:**

1. Resolve parent + child labels via `findLabels`.
2. Fetch 1 sample record of current hop; `findRelationships` with `source.where.$id` or `target.where.$id` → adjacent labels.
3. On finding path `PARENT→A→B→CHILD`:
   ```js
   where: {
     A: {
       B: {
         CHILD: {
           $alias: '$child'
         }
       }
     }
   }
   select: {
     metric: {
       $count: '*'
     }
   }
   ```
4. No top-level `limit` for pure grouped aggregations.
5. Only the root parent label appears in `labels:[]`. Intermediates appear only inside `where`.
6. NEVER reuse `'$record'` alias for related-node references.
7. After path found, collapse redundant intermediate layers.
8. If BFS exhausts without match: synonym remap using `findProperties` output; if still unavailable, ask.

**Avoid over-nesting:**

```js
// WRONG:
where: {
  A: {
    B: {
      $alias: '$b'
    }
  }
} // when B is directly linked to root
// CORRECT:
where: {
  B: {
    $alias: '$b'
  }
}

// WRONG (same label chained manually):
where: { EMPLOYEE: { EMPLOYEE: { EMPLOYEE: {} } } }
// CORRECT:
where: { EMPLOYEE: { $relation: { type: 'REPORTS_TO', direction: 'out', hops: { max: 3 } } } }
```

### Cycle / Ring Detection

"Records on a loop back to themselves" (fraud rings, circular ownership, dependency cycles) → `$cycle` block (see §1). Root label = the entity type; the `$cycle` block holds only `$relation`:

```js
findRecords({
  labels: ['ACCOUNT'],
  where: {
    RING: { $cycle: true, $relation: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } } }
  }
})
```

Returns ring **participants**. To reconstruct a specific ring's composition, follow up with bounded one-hop queries from each flagged record.

---

## §11) NL → WHERE Translation Quick Reference

| Natural language             | Where syntax                                                                 |
| ---------------------------- | ---------------------------------------------------------------------------- |
| equals                       | `field: value` or `field: { $eq: value }`                                    |
| not equals                   | `field: { $ne: value }`                                                      |
| in set                       | `field: { $in: [v1,v2] }`                                                    |
| not in set                   | `field: { $nin: [v1,v2] }`                                                   |
| greater than                 | `field: { $gt: N }`                                                          |
| between X and Y              | `field: { $gte: X, $lte: Y }`                                                |
| contains "text"              | `field: { $contains: "text" }`                                               |
| starts with                  | `field: { $startsWith: "..." }`                                              |
| ends with                    | `field: { $endsWith: "..." }`                                                |
| has field                    | `field: { $exists: true }`                                                   |
| AND conditions               | implicit (multiple keys) or `$and:[...]`                                     |
| OR conditions                | `$or: [...]`                                                                 |
| NOT condition                | `$not: { ... }`                                                              |
| related to X                 | `LABEL_NAME: { ...filters... }`                                              |
| within N hops / any ancestor | `LABEL: { $relation: { type, direction, hops: { max: N } } }`                |
| ring / loop / circular flow  | `{ $cycle: true, $relation: { type, direction, hops: { min: 2, max: N } } }` |
| last 7 days                  | compute ISO boundary → `createdAt: { $gte: "ISO-UTC" }`                      |
| in year 1994                 | `date: { $gte: { $year:1994 }, $lt: { $year:1995 } }`                        |

**Numerics:** 1k=1000, 1m=1000000, 1b=1000000000. Strip currency symbols ($100k→100000).

---

## §12) Validation Checklist

Before submitting any `findRecords` call, verify:

- [ ] No `groupBy` without `select`
- [ ] `limit` absent for self-group and dimensional groupBy (unless scoping root records in flat select)
- [ ] `orderBy` on select key present for self-group queries (triggers late ordering → correct totals)
- [ ] groupBy mode correct:
  - Dimensional: entries are `"$alias.propertyName"` strings
  - Self-group: entries are select key names (no dot, no alias prefix)
- [ ] No alias-only `groupBy` values such as `"$record"` or `"$related"`
- [ ] Root labels only in `labels`; related labels go in `where` traversal with `$alias` if referenced
- [ ] Related-count ranking keeps the requested parent/entity as root; the related/filter label does not steal the root
- [ ] Traversal: key = label name (ALL_CAPS). NEVER `$label`/`$direction`/`$as`/`$of`/`$through`/`$hops`
  - WRONG: `{ employee: { $label:'EMPLOYEE' } }` CORRECT: `{ EMPLOYEE: { $alias:'$emp' } }`
  - `$relation.hops` and `$cycle` are VALID operators — do not "correct" them away
- [ ] `hops` only for repeated-pattern traversal (hierarchy/degrees); bounded `max`, as small as possible
- [ ] `$cycle` block contains ONLY `$relation` (`hops` min ≥ 2); no `$alias`/criteria/nested labels inside
- [ ] Vector similarity: still uses legacy `aggregate` (not `select`)
- [ ] Vector threshold semantics: euclidean → `$lte`; others → `$gte`
- [ ] Month+day without year → ask for year
- [ ] Aggregation intent? → query MUST include `select` + `groupBy`. Raw records ≠ aggregation

---

## §13) Example Patterns

_(Actual label/field names always come from `getSchemaMarkdown` — never from these examples.)_

**List with numeric filter:**

```js
findRecords({ labels:['<LABEL>'], where:{ <field>:{ $gt:100000 } }, limit:10 })
```

**Date range:**

```js
findRecords({ labels:['<LABEL>'], where:{ <dateField>:{ $gte:{ $year:1994 }, $lt:{ $year:1995 } } }, limit:10 })
```

**Dimensional groupBy (count + avg per category):**

```js
findRecords({
  labels: ['<LABEL>'],
  select: { count: { $count: '*' }, avg: { $avg: '$record.<field>', $precision: 2 } },
  groupBy: ['$record.<categoryField>'],
  orderBy: { count: 'desc' }
})
```

**Which parent has the most/fewest related children:**

```js
findRecords({
  labels: ['<PARENT_LABEL>'],
  where: { <CHILD_LABEL>: { $alias: '$child' } },
  select: {
    parent: '$record.<nameField>',
    children: { $count: '$child' }
  },
  groupBy: ['$record.<nameField>'],
  orderBy: { children: 'desc' }
})
```

Use `desc` for most/more/highest/largest/greatest, and `asc` for least/less/fewer/fewest/lowest/smallest.

**Self-group single KPI:**

```js
findRecords({
  labels: ['<LABEL>'],
  select: { total: { $sum: '$record.<field>' } },
  groupBy: ['total'],
  orderBy: { total: 'asc' } // required for correct full-scan total
})
```

**Self-group multiple KPIs:**

```js
findRecords({
  labels: ['<LABEL>'],
  select: {
    totalRevenue: { $sum: '$record.<revenueField>' },
    orderCount: { $count: '*' },
    avgOrder: { $avg: '$record.<revenueField>', $precision: 2 }
  },
  groupBy: ['totalRevenue', 'orderCount', 'avgOrder'],
  orderBy: { totalRevenue: 'asc' }
})
```

**Per-record from related label (one row per root record):**

```js
labels: ['PROJECT'],
where:  { budget: { $lte: 10000000 }, EMPLOYEE: { $alias: '$employee' } },
select: {
  projectName: '$record.name',
  headcount:   { $count: '$employee.id' },
  totalWage:   { $sum: '$employee.salary' },
  avgSalary:   { $avg: '$employee.salary', $precision: 0 }
},
limit: 100   // valid: caps root PROJECT records
```

**Nested hierarchy (COMPANY → DEPT → PROJECT → EMPLOYEE):**

```js
labels: ['COMPANY'],
where:  { foundedAt: { $lte: { $year: 1980 } } },
select: {
  company: '$record.name',
  departments: {
    $collect: {
      label: 'DEPARTMENT',
      select: {
        name: '$self.name',
        projects: {
          $collect: {
            label: 'PROJECT',
            orderBy: { name: 'asc' },
            select: {
              name: '$self.name',
              employees: { $collect: { label: 'EMPLOYEE', orderBy: { salary: 'desc' }, limit: 3 } }
            }
          }
        }
      }
    }
  }
}
```

**Time series monthly count:**

```js
findRecords({
  labels: ['<LABEL>'],
  select: { month: { $timeBucket: { field: '$record.<dateField>', unit: 'month' } }, count: { $count: '*' } },
  groupBy: ['month'],
  orderBy: { month: 'asc' }
})
```

**Relationship with specific type/direction:**

```js
where: { POST: { $relation: { type: 'AUTHORED', direction: 'in' }, title: { $contains: 'Graph' } } }
```

**Multihop hierarchy ("everyone in Alice's reporting chain, up to 4 levels"):**

```js
where: { EMPLOYEE: { $relation: { type: 'REPORTS_TO', direction: 'out', hops: { max: 4 } }, name: { $contains: 'Alice' } } }
```

**Cycle detection ("accounts on a circular transfer ring"):**

```js
findRecords({
  labels: ['ACCOUNT'],
  where: {
    RING: { $cycle: true, $relation: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } } }
  }
})
```

**Filter by ID:**

```js
where: {
  $id: {
    $in: ['id1', 'id2']
  }
}
where: {
  EMPLOYEE: {
    $id: 'specific-id'
  }
}
```

**XOR / exclusive range:**

```js
where: { budget: { $xor: { $lte: 10000000, $gte: 15000000 } } }
```

---

Resource-local `where` rule:

- `findRecords.where`, `exportRecords.where`, and `bulkDeleteRecords.where` apply to Records.
- `findRelationships.where` applies to relationship edges: `type` maps to the relationship type, and all other fields map to user-defined edge properties. Use `source` and `target` for endpoint Record predicates.
- `findLabels.where` and `findProperties.where` apply to Records before returning label/property metadata.

`findRelationships` does not support `select` or `groupBy`.
