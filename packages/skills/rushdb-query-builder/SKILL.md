---
name: rushdb-query-builder
description: Build RushDB queries, searches, filters, and aggregations. Use this skill whenever an agent needs to interact with RushDB data — listing records, filtering by properties, counting, summing, averaging, grouping by dimension, traversing relationships between record types, running semantic/vector search, or building any findRecords call. Also use when discovering what labels and properties exist in the database.
homepage: https://rushdb.com
---

# RushDB Query Builder

A discovery-first workflow for safely and correctly querying RushDB.

**Never guess label names, property names, or field values. Always discover them first.**

## Prerequisites

- **RushDB MCP server** must be connected — it provides `getOntologyMarkdown`, `findRecords`, and all other tools used in this skill. Setup: `npx @rushdb/mcp-server` (requires `RUSHDB_API_KEY` env var). See https://docs.rushdb.com/mcp-server/quickstart
- If the MCP tools are not available in the current session, tell the user the MCP server is not configured and link them to the quickstart above.

---

## Mandatory Workflow (always follow this order)

### Step 0 — Ontology (every session, first call)

Call `getOntologyMarkdown` before any other tool. It returns:
- All label names (case-sensitive — use them exactly)
- All property names and types per label
- Value ranges for numeric/datetime fields
- The full relationship map between labels
- A **Semantic Search** column per property: non-`—` value means the property is indexed and queryable via `aiSemanticSearch`

Do not call `findLabels`, `findProperties`, or `findRecords` before this.

```
getOntologyMarkdown()
```

If the schema looks stale (e.g. new labels or properties were added recently), pass `{ force: true }` to bypass the 1-hour cache and force a fresh recalculation.

### Step 1 — Classify Intent

Before building a query, identify what is being asked:

| Intent | Pattern | Tool |
|---|---|---|
| **Aggregation** | count / total / sum / avg / breakdown / per X / top N by metric / distribution / grouped | `findRecords` with `aggregate` + `groupBy` |
| **Listing** | show / list / find / search / get | `findRecords` with `where` + `limit` + `orderBy` |
| **Single record** | get by ID, find one unique | `getRecord` / `findOneRecord` / `findUniqRecord` |
| **Relationships** | connected to / linked / related | `findRelationships` |
| **Mutation** | create / update / delete | confirm + preview first |

⚠ **Aggregation intent**: NEVER fetch raw records and count them manually. ALWAYS use `aggregate` + `groupBy` on `findRecords`.

### Step 2 — Load Query Spec (for complex queries)

Before calling `findRecords` with any of these, call `getSearchQuerySpec`:
- Date/time filters or date ranges
- `aggregate` + `groupBy`
- Relationship traversal (`where` keys that are label names)
- Vector/semantic similarity

`getSearchQuerySpec` returns the complete operator reference, aggregation syntax, groupBy modes, and annotated examples. It is the source of truth — do not guess syntax.

### Step 3 — Build and Execute

Use only label and property names from the ontology. Labels are **case-sensitive**.

---

## Tool Reference

### Discovery
| Tool | When to use |
|---|---|
| `getOntologyMarkdown` | Step 0 — always first, once per session |
| `getOntology` | Same as above but structured JSON; use when you need `propertyId` values or `vectorIndexes` per property |
| `findLabels` | Skip if `getOntologyMarkdown` already ran |
| `findProperties` | Discover field names + types for a specific label when not in ontology |
| `findRelationships` | Inspect relationships; supports `where` + `limit` + `orderBy`; no aggregate/groupBy |
| `propertyValues` | List distinct values for a property (needs `propertyId` from `getOntology`) |

> **Semantic search:** Properties listed as `getOntology` results with a non-empty `vectorIndexes` array are eligible for semantic search. Use `aiSemanticSearch` with the matching `propertyName` and `labels`.

### Querying
| Tool | When to use |
|---|---|
| `findRecords` | Primary query tool — listing, filtering, aggregation, groupBy, semantic search |
| `findOneRecord` | Return the first matching record |
| `findUniqRecord` | Return exactly one record (errors if multiple match) |
| `getRecord` | Fetch a single record by ID |
| `getRecordsByIds` | Fetch multiple records by IDs |

### Mutations (confirm before use)
| Tool | Notes |
|---|---|
| `createRecord` | Store a new record |
| `updateRecord` | Patch fields on an existing record |
| `setRecord` | Replace all fields on an existing record |
| `deleteRecord` / `deleteRecordById` | Destructive — preview first |
| `bulkCreateRecords` | Batch insert via nested JSON |
| `bulkDeleteRecords` | **Destructive batch** — always preview with `findRecords` first |
| `attachRelation` | Create a relationship between two records |
| `detachRelation` | Remove a relationship |
| `exportRecords` | Export matching records to CSV |

---

## Quick Operator Cheat Sheet

Use these in the `where` clause of `findRecords`:

```
// Equality
name: "Alice"
isActive: true
age: 30

// String (all case-insensitive)
name: { $contains: "ali" }
name: { $startsWith: "A" }
name: { $endsWith: "e" }
name: { $ne: "deleted" }

// Number
score: { $gt: 90 }
score: { $gte: 70, $lte: 100 }
score: { $in: [85, 90, 95] }

// Existence / type
email: { $exists: true }
age:   { $type: "number" }    // "string"|"number"|"boolean"|"datetime"|"null"|"vector"

// Logical
$and: [ { status: "active" }, { score: { $gte: 70 } } ]
$or:  [ { plan: "pro" }, { plan: "enterprise" } ]
$not: { status: "deleted" }

// Relationship traversal — key IS the label name (UPPER_CASE)
where: {
  DEPARTMENT: {
    name: "Engineering",
    headcount: { $gte: 10 }
  }
}
```

**Common mistakes to avoid:**
- `$label`, `$direction`, `$as`, `$of`, `$through` — **these do not exist**
- `{ employee: { $label: "EMPLOYEE" } }` — **WRONG**: key must be the label name
- `{ EMPLOYEE: { $alias: "$emp" } }` — **CORRECT**
- Do NOT include `limit` when using `aggregate` (produces wrong results)

---

## Key Limits

- `limit` max = 1000
- **Never use `limit` with `aggregate`** — it restricts the scan and gives wrong totals
- For `"how many"` simple counts: read `total` from the `findRecords` response — do NOT use `fn:'count'`
- For groupBy self-group and dimensional groupBy: omit `limit` unless you want "top N"

---

## When to Load the Full Reference

Call `getSearchQuerySpec` (or read `references/search-query-spec.md`) for:

- **Datetime filters** — never use plain strings with `$gt`/`$lt`; needs component objects
- **Aggregation functions** — `count`, `sum`, `avg`, `min`, `max`, `collect`, `timeBucket`
- **groupBy modes** — dimensional (`$alias.property`) vs self-group (aggregation key names)
- **Late-ordering rule** — critical for correct totals in self-group queries
- **Relationship traversal** — `$alias`, `$relation`, multi-hop BFS algorithm
- **Vector similarity** — `fn:'vector.similarity.cosine'` inside `aggregate`, not `where`
- **Nested collect** — building tree-shaped results
- **Validation checklist** — 13-point checklist before submitting any `findRecords` call
