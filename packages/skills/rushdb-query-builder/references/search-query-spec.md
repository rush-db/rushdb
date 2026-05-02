# RushDB SearchQuery — Complete Reference

> Ported from `packages/mcp-server/searchQuerySpec.ts`. Load this file before calling `findRecords` with dates, aggregation, groupBy, relationship traversal, or vector search.

---

## Table of Contents

1. [SearchQuery Shape](#searchquery-shape)
2. [WHERE — Filters & Traversal](#1-where--filters--traversal)
3. [AGGREGATE — Functions & Inline Refs](#2-aggregate--functions--inline-refs)
4. [GROUPBY — Two Modes](#3-groupby--two-modes)
5. [COLLECT — Array Gathering](#4-collect--array-gathering)
6. [TIMEBUCKET — Time-Series](#5-timebucket--time-series)
7. [LIMIT Rules by Query Mode](#6-limit-rules-by-query-mode)
8. [Metric Field Discovery Across Labels](#7-metric-field-discovery-across-related-labels)
9. [Range / Distribution Queries](#8-range--distribution-queries)
10. [Multi-Label Filter Distribution](#9-multi-label-filter-distribution)
11. [Enum / Value Normalization](#10-enum--value-normalization)
12. [Relationship & Path Queries](#11-relationship--path-queries)
13. [NL → WHERE Quick Reference](#12-nl--where-translation-quick-reference)
14. [Validation Checklist](#13-validation-checklist)

---

## SearchQuery Shape

```
labels?    string[]      — filter by record type(s); multi-label = OR
where?     object        — filter conditions; see §1
aggregate? object        — aggregation map; see §2
groupBy?   string[]      — shapes aggregate output; see §3
orderBy?   string|object — 'asc'|'desc' or { field:'asc'|'desc' }
limit?     number        — max root records (default 100; max 1000)
skip?      number        — pagination offset
```

**Critical limits:**
- NEVER include `limit` when `aggregate` is present (`sum`/`avg`/`min`/`max`/`count`/`collect`/`timeBucket`). `limit` restricts the record scan → results are mathematically wrong. e.g. "total budget of all 33 projects" with `limit:10` returns only the sum of the first 10.
- Self-group and dimensional groupBy queries: omit `limit` entirely (or scope to root records only).
- `limit` is valid only for listing/browsing queries or per-record flat aggregation (one row per root record).

---

## §1) WHERE — Filters & Traversal

The where clause mechanism: when a nested object key is NOT a criteria operator (like `$gt`, `$contains`, etc.) and NOT a flat value, RushDB interprets that key as the **LABEL** of a related record to traverse.

### Primitive Value Matching

```js
name: "John Doe"                       // exact string (case-sensitive equality)
isActive: true                         // boolean
age: 30                                // number
created: "2023-01-01T00:00:00Z"        // ISO 8601 datetime
```

### String Operators

```js
name: { $contains: "John" }            // substring match (case-insensitive)
name: { $startsWith: "J" }             // prefix match (case-insensitive)
name: { $endsWith: "son" }             // suffix match (case-insensitive)
name: { $ne: "deleted" }               // not equal
status: { $in: ["active","pending"] }  // matches any value in array
status: { $nin: ["deleted","archived"] } // matches none of these values
```

### Number Operators

```js
age: { $gt: 18 }        // greater than
age: { $gte: 21 }       // greater than or equal
age: { $lt: 65 }        // less than
age: { $lte: 64 }       // less than or equal
age: { $ne: 18 }        // not equal
age: { $in: [20,30,40] }   // matches any of these numbers
age: { $nin: [20,30,40] }  // matches none of these numbers
```

### Boolean Operators

```js
isActive: true                  // direct match
isActive: { $ne: false }        // not equal (matches true or unset)
```

### Datetime Operators

⚠ **NEVER use plain date strings with comparison operators like `$gt`/`$gte`/`$lt`/`$lte`.** Use component objects instead.

```js
// ISO 8601 exact match or equality only:
created: "2023-01-01T00:00:00Z"
created: { $in: ["2023-01-01T00:00:00Z", "2023-02-01T00:00:00Z"] }

// Component matching (exact point in time):
created: { $year: 2023, $month: 1, $day: 1 }
// Available: $year $month $day $hour $minute $second $millisecond $microsecond $nanosecond

// Range comparisons — ALWAYS use component objects:
// Year "in 1994":     { field: { $gte: { $year:1994 }, $lt: { $year:1995 } } }
// Month "Jan 1994":   { field: { $gte: { $year:1994,$month:1 }, $lt: { $year:1994,$month:2 } } }
// Day "1994-03-15":   { field: { $gte: { $year:1994,$month:3,$day:15 }, $lt: { $year:1994,$month:3,$day:16 } } }
// Decade "1990s":     { field: { $gte: { $year:1990 }, $lt: { $year:2000 } } }

// Relative ("last 7 days", "this month"): compute ISO UTC boundary → use ISO string with $gte
```

**Month+day WITHOUT year**: unsupported — ask the user for a year.

### Vector Similarity

`$vector` is NOT a `where` operator. Use it only inside `aggregate`:

```js
aggregate: {
  similarity: {
    fn: "vector.similarity.cosine",   // cosine | euclidean
    field: "embedding",
    query: [1, 2, 3, 4, 5],
    alias: "$record"
  }
}
```

### Field Existence & Type

```js
phoneNumber: { $exists: true }    // only records that have this field
phoneNumber: { $exists: false }   // only records that do NOT have this field
age: { $type: "number" }          // "string"|"number"|"boolean"|"datetime"|"null"|"vector"
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
// The operators $label / $direction / $as / $of / $through DO NOT EXIST.

// CORRECT: key IS the label; alias via $alias only
where: { EMPLOYEE: { $alias: "$emp" } }
```

**`$alias`** — name a traversed node for use in `aggregate`/`groupBy`:

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

**`$id`** — filter by record ID:

```js
where: { $id: { $in: ["id1","id2"] } }
where: { EMPLOYEE: { $id: "specific-id" } }
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

---

## §2) AGGREGATE — Functions & Inline Refs

Every aggregate key maps to either an **INLINE REF** or an **AGGREGATION FUNCTION**.

### Inline Ref (copy a field value into the output row — no `fn`)

```js
"outputKey": "$alias.fieldName"
"companyName":   "$record.name"
"projectBudget": "$record.budget"
```

### Aggregation Functions (`alias` defaults to `'$record'` if omitted)

```js
fn:'count'      — count matching records; optional field + unique:bool
fn:'sum'        — { fn:'sum',  field:'salary',  alias:'$employee' }
fn:'avg'        — { fn:'avg',  field:'salary',  alias:'$employee', precision:2 }
fn:'min'        — { fn:'min',  field:'salary',  alias:'$employee' }
fn:'max'        — { fn:'max',  field:'salary',  alias:'$employee' }
fn:'collect'    — gather into array; see §4
fn:'timeBucket' — temporal bucketing; see §5
```

`alias: '$record'` for root-label fields; the `$alias` string declared in `where` for related nodes.
**EVERY fn-based aggregate entry MUST include `alias`.**

### Flat Cross-Label Example (PROJECT root + EMPLOYEE metrics)

```js
labels: ['PROJECT'],
where: { budget:{ $lte:10000000 }, EMPLOYEE:{ $alias:'$employee' } },
aggregate: {
  projectName:    '$record.name',
  projectBudget:  '$record.budget',
  headcount:      { fn:'count',  unique:true,               alias:'$employee' },
  totalWage:      { fn:'sum',    field:'salary',            alias:'$employee' },
  avgSalary:      { fn:'avg',    field:'salary', precision:0, alias:'$employee' },
  minSalary:      { fn:'min',    field:'salary',            alias:'$employee' },
  maxSalary:      { fn:'max',    field:'salary',            alias:'$employee' }
}
// → one row per PROJECT record, each with employee stats embedded
```

---

## §3) GROUPBY — Two Modes

### Mode A — Dimensional (one row per distinct value)

```js
// groupBy entries use "$alias.propertyName" format
aggregate: { count:{ fn:'count', alias:'$record' }, avg:{ fn:'avg', field:'total', alias:'$record' } },
groupBy: ['$record.status'],
orderBy: { count:'desc' }
// Output rows: [{ "status":"pending","count":120,"avg":310.42 }, ...]
// Note: group key appears WITHOUT alias prefix in output ($record.status → "status")
// Multiple keys = pivot: groupBy: ['$record.category','$record.active']
```

### Mode B — Self-Group (collapse to ONE row with global metric)

```js
// groupBy entries are aggregation KEY NAMES (no dot, no alias prefix)
aggregate: { totalBudget:{ fn:'sum', field:'budget', alias:'$record' } },
groupBy:   ['totalBudget']
// Output: [{ "totalBudget": 1875251446 }]
// Multiple KPIs: groupBy: ['totalRevenue','orderCount']
```

### ⚠ Late-Ordering Rule — Critical for Correct Totals

When `orderBy` references an aggregated key, the engine applies ORDER BY + LIMIT AFTER the aggregation (full-scan first, then paginate).

When `orderBy` is absent or references a raw field, LIMIT is applied BEFORE aggregation → only the first N raw records are aggregated → **WRONG totals**.

**Fix**: for self-group and any pure metric query, always add `orderBy` on the aggregation key:

```js
aggregate:{ total:{ fn:'sum', field:'amount', alias:'$record' } },
groupBy:['total'],
orderBy:{ total:'asc' }   // ← triggers late ordering; ensures full dataset is summed
```

---

## §4) COLLECT — Array Gathering & Nested Structures

### select $collect — Label-Based (PREFERRED for nesting)

Use `label` instead of `from` for inline traversal with no `$alias` required. `$self` = current level.

```js
select: {
  departments: {
    $collect: {
      label: 'DEPARTMENT',
      where: { budget: { $gte: 10000 } },  // optional flat filter on this level
      select: {
        name: '$self.name',
        projects: {
          $collect: {
            label: 'PROJECT',
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

### select $collect — Alias-Based (requires `$alias` in `where`)

```js
where: { EMPLOYEE: { $alias: '$emp' } },
select: {
  employees: {
    $collect: {
      from: '$emp',
      select: { name: '$emp.name' },
      orderBy: { name: 'asc' },
      limit: 10
    }
  }
}
```

### aggregate `fn:'collect'` — Legacy (still works; prefer select for new queries)

```js
{ fn:'collect', field:'name', alias:'$employee', unique:true }
```

Options:
- `field?` — specific field; omit to collect entire records
- `unique?` — deduplicate (default `true`)
- `limit?` — max items in collected array
- `skip?` — skip N items in collected array
- `orderBy?` — sort collected items: `{ salary:'desc' }`

### Nested Collect (legacy aggregate form — requires $alias)

```js
labels: ['COMPANY'],
where: {
  DEPARTMENT: { $alias:'$dept',
    PROJECT:  { $alias:'$proj',
      EMPLOYEE: { $alias:'$emp', dob:{ $lte:{ $year:1994 } } }
    }
  }
},
aggregate: {
  company: '$record.name',
  departments: {
    fn:'collect', alias:'$dept',
    aggregate: {
      projects: {
        fn:'collect', alias:'$proj', orderBy:{ projectName:'asc' },
        aggregate: {
          employees: { fn:'collect', alias:'$emp', orderBy:{ salary:'desc' }, limit:3 }
        }
      }
    }
  }
}
```

---

## §5) TIMEBUCKET — Time-Series Bucketing

```js
fn:'timeBucket', field:'createdAt', granularity:'day'|'week'|'month'|'quarter'|'year'
// Custom N-sized windows:
granularity:'months'|'hours'|'minutes'|'seconds'|'years', size:N
```

Examples:

```js
// Daily counts:
aggregate:{
  day:{ fn:'timeBucket', field:'createdAt', granularity:'day', alias:'$record' },
  count:{ fn:'count', alias:'$record' }
},
groupBy:['day'], orderBy:{ day:'asc' }

// Monthly revenue:
aggregate:{
  month:{ fn:'timeBucket', field:'issuedAt', granularity:'month', alias:'$record' },
  revenue:{ fn:'sum', field:'amount', alias:'$record' }
},
groupBy:['month'], orderBy:{ month:'asc' }

// Bi-monthly (every 2 months):
aggregate:{
  period:{ fn:'timeBucket', field:'startedAt', granularity:'months', size:2, alias:'$record' },
  n:{ fn:'count', alias:'$record' }
},
groupBy:['period'], orderBy:{ period:'asc' }
```

---

## §6) LIMIT Rules by Query Mode

| Query mode | Limit |
|---|---|
| Self-group (single KPI row) | NO `limit` — but MUST add `orderBy` on aggregation key for late ordering |
| Dimensional groupBy | NO `limit` to get all groups; add `limit` + `orderBy` on aggregation key for "top N" |
| Per-record flat aggregation (one row per root record) | `limit` IS valid (caps root records) |
| Pure listing (no aggregate) | `limit` always valid |
| "how many" simple count | Read `total` from the `findRecords` response — do NOT use `fn:'count'` |

---

## §7) Metric Field Discovery Across Related Labels

If the metric field is NOT on the target label, search related labels before giving up:

1. Confirm target label. `findProperties(labels:[<target>])` — look for the metric field.
2. If absent, walk adjacent labels via `getOntologyMarkdown` or `findRelationships` probe.
3. For each candidate related label R: `findProperties(labels:[R])` and attempt the same match.
4. When found on CHILD: `where:{ CHILD:{ ...filters..., $alias:'$child' } }`, aggregate `alias:'$child'`. Root-level filters stay at the top-level `where`.
5. Never abandon after one miss — always attempt at least one related-label discovery pass.

---

## §8) Range / Distribution Queries

- **type = number or datetime** → `findRecords` aggregate with `fn:'min'` + `fn:'max'`. Add groupBy key names for self-group mode. Or: `getOntology` (JSON) → `propertyValues(propertyId)` → returns `{ min, max }` directly.
- **type = string or boolean** → `propertyValues(propertyId)` to list all distinct values.
- NEVER call `findRecords` with a `where` filter to "search for" values of a field — that returns records, not ranges.

---

## §9) Multi-Label Filter Distribution

Place each filter with the label that actually holds the field. On zero results, silently retry by moving the filter to the related child block before asking.

---

## §10) Enum / Value Normalization

Never hardcode guessed values for enumerated fields:

1. `findProperties` to locate the property and get its `id`.
2. `propertyValues(propertyId, { query:<user value> })` to probe existing values.
3. If no match: try case variants, abbreviations, partial prefixes.
4. Use ONLY canonical values returned by `propertyValues`.
5. Re-run `propertyValues` with empty query to list top candidates; retry once silently. Ask only if two+ equally plausible values remain.
6. Always mention assumption briefly if mapping is non-obvious, then proceed.

---

## §11) Relationship & Path Queries

### Entity Resolution by Name

Probe with `findRecords(limit:1, where:{ <nameField>:{ $contains:'...' } })`.

### Multi-Hop Relationship Discovery

Pre-check before multi-hop:
1. `findProperties(labels:[parent])` for direct scalar fields.
2. Fetch 1 sample parent record. `findRelationships` filtered by its `id` → discover adjacent labels.
3. If direct path to child exists: `where:{ CHILD:{ $alias:'$child' } }` — STOP. No intermediate wrappers.
4. Only if no direct path: BFS (depth ≤ 4).

**BFS algorithm:**
1. Resolve parent + child labels via `findLabels`.
2. Fetch 1 sample record of current hop; `findRelationships` filtered by its `id` → adjacent labels.
3. On finding path `PARENT→A→B→CHILD`:
   ```js
   where:{ A:{ B:{ CHILD:{ $alias:'$child' } } } }
   aggregate:{ metric:{ fn:'count'|'avg'|..., alias:'$child' } }
   ```
4. No top-level `limit` for pure grouped aggregations.
5. Only the root parent label appears in `labels:[]`. Intermediates appear only inside `where`.
6. NEVER reuse `'$record'` alias for related-node aggregation.
7. After path found, collapse redundant intermediate layers.
8. If BFS exhausts without match: synonym remap using `findProperties` output; if still unavailable, ask.

**Avoid over-nesting:**
```js
// WRONG:
where:{ A:{ B:{ $alias:'$b' } } }   // when B is directly linked to root
// CORRECT:
where:{ B:{ $alias:'$b' } }
```

---

## §12) NL → WHERE Translation Quick Reference

| Natural language | Where syntax |
|---|---|
| equals | `field: value` |
| not equals | `field: { $ne: value }` |
| in set | `field: { $in: [v1,v2] }` |
| not in set | `field: { $nin: [v1,v2] }` |
| greater than | `field: { $gt: N }` |
| between X and Y | `field: { $gte: X, $lte: Y }` |
| contains "text" | `field: { $contains: "text" }` |
| starts with | `field: { $startsWith: "..." }` |
| ends with | `field: { $endsWith: "..." }` |
| has field | `field: { $exists: true }` |
| AND conditions | implicit (multiple keys) or `$and:[...]` |
| OR conditions | `$or: [...]` |
| NOT condition | `$not: { ... }` |
| related to X | `LABEL_NAME: { ...filters... }` |
| last 7 days | compute ISO boundary → `createdAt: { $gte: "ISO-UTC" }` |
| in year 1994 | `date: { $gte: { $year:1994 }, $lt: { $year:1995 } }` |

**Numerics:** 1k=1000, 1m=1000000, 1b=1000000000. Strip currency symbols ($100k→100000).

---

## §13) Validation Checklist

Before submitting any `findRecords` call, verify:

- [ ] No `groupBy` without `aggregate`
- [ ] `alias` present on every fn-based aggregate entry (`'$record'` for root; declared `$alias` for related)
- [ ] Inline refs (`"$alias.field"` string values) do NOT need `fn` or `alias` key
- [ ] `limit` absent for self-group and dimensional groupBy (unless scoping root records in flat aggregation)
- [ ] `orderBy` on aggregation key present for self-group queries (triggers late ordering → correct totals)
- [ ] groupBy mode correct:
  - Dimensional: entries are `"$alias.propertyName"` strings
  - Self-group: entries are aggregation key names (no dot, no alias prefix)
- [ ] Nested collect: prefer `select $collect` with `label` for new queries (unlimited nesting, no `$alias` needed); legacy `aggregate fn:'collect'` still works for existing queries
- [ ] Traversal: key = label name (ALL_CAPS). NEVER `$label`/`$direction`/`$as`/`$of`/`$through`
- [ ] No `'$record'` alias reused for related-node aggregation
- [ ] Vector threshold semantics: euclidean → `$lte`; others → `$gte`
- [ ] Month+day without year → ask for year
- [ ] Aggregation intent? → query MUST include `aggregate` + `groupBy`. Raw records ≠ aggregation
