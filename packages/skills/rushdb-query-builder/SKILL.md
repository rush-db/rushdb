---
name: rushdb-query-builder
description: Build RushDB queries, searches, filters, and aggregations. Use this skill whenever an agent needs to interact with RushDB data — listing records, filtering by properties, counting, summing, averaging, grouping by dimension, traversing relationships between record types, running semantic/vector search, or building any findRecords call. Also use when discovering what labels and properties exist in the database.
homepage: https://rushdb.com
---

# RushDB Query Builder

A discovery-first workflow for safely and correctly querying RushDB.

**Never guess label names, property names, or field values. Always discover them first.**

## Prerequisites

- **RushDB MCP server** must be connected — it provides `getSchemaMarkdown`, `findRecords`, and all other tools used in this skill. Setup: `npx @rushdb/mcp-server` (requires `RUSHDB_API_KEY` env var). See https://docs.rushdb.com/mcp-server/quickstart
- If the MCP tools are not available in the current session, tell the user the MCP server is not configured and link them to the quickstart above.

---

## Mandatory Workflow (always follow this order)

### Step 0 — Schema (every session, first call)

Call `getSchemaMarkdown` before any other tool. It returns:

- All label names (case-sensitive — use them exactly)
- All property names and types per label
- Array fields shown as `type[]`; in structured `getSchema`, `isArray: true` marks primitive-array properties
- Value ranges for numeric/datetime fields
- The full relationship map between labels
- A **Semantic Search** column per property: non-`—` value means the property is indexed and queryable via `aiSemanticSearch`

Do not call `findLabels`, `findProperties`, or `findRecords` before this.

```
getSchemaMarkdown()
```

If the schema looks stale (e.g. new labels or properties were added recently), pass `{ force: true }` to bypass the 1-hour cache and force a fresh recalculation.

### Step 1 — Classify Intent

Before building a query, identify what is being asked:

| Intent                    | Pattern                                                                                  | Tool                                                                                                         |
| ------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| **Aggregation**           | count / total / sum / avg / breakdown / per X / top N by metric / distribution / grouped | `findRecords` with `select` + `groupBy`                                                                      |
| **Related count ranking** | which/what parent has most/more/least/less/fewer/fewest related child records            | `findRecords` with parent `labels`, child traversal in `where`, `select` count alias, `groupBy` parent field |
| **Listing**               | show / list / find / search / get                                                        | `findRecords` with `where` + `limit` + `orderBy`                                                             |
| **Single record**         | get by ID, find one unique                                                               | `getRecord` / `findOneRecord` / `findUniqRecord`                                                             |
| **Relationships**         | connected to / linked / related                                                          | `findRelationships`                                                                                          |
| **Mutation**              | create / update / delete                                                                 | confirm + preview first                                                                                      |

⚠ **Aggregation intent**: NEVER fetch raw records and count them manually. ALWAYS use `select` + `groupBy` on `findRecords`.

### Step 2 — Load Query Spec (for complex queries)

Before calling `findRecords` with any of these, call `getSearchQuerySpec`:

- Date/time filters or date ranges
- `select` + `groupBy` (metrics, aggregations)
- Relationship traversal (`where` keys that are label names)
- Vector/semantic similarity

`getSearchQuerySpec` returns the complete operator reference, select/groupBy syntax, late-ordering rules, and annotated examples. It is the source of truth — do not guess syntax.

### Step 3 — Build and Execute

Use only label and property names from the schema. Labels are **case-sensitive**.

Root-label rule:

- `labels` contains only the root record type(s) you want returned as rows.
- Related record types belong inside `where` traversal blocks, not beside the root in `labels`.
- If a related record is needed in `select` or `groupBy`, declare `$alias` on that related label in `where`.

Top-N rule:

- "Top N records by a scalar field on the same label" is a listing: use `orderBy` + `limit`, no `select`.
- "Which parent has most/more/least/less/fewer/fewest related children" is an aggregation: root the parent label, traverse the child label with `$alias`, count the child alias, group by a parent display field.
- Related-count direction: most/more/highest/largest/greatest => `desc`; least/less/fewer/fewest/lowest/smallest => `asc`.
- Do not let the related/filter label become the root just because it owns the filtered field. If the requested parent-to-related traversal path is absent, do not silently switch roots; state the path is unavailable or use a closest valid fallback with an explicit assumption.

Named-reference rule:

- When filtering a display field such as `name`, `title`, or another schema-backed label/name field with free text the user typed, default to `{ $contains: "<user text>" }`. This applies to filters on any label, including related labels traversed with `$alias`.
- Use exact equality only when the value is an ID, or the user explicitly asks for an exact match (e.g. "named exactly").
- If you need to confirm a canonical value before filtering, use discovery (list/search a few records) rather than guessing an exact string.

Example:

```js
findRecords({
  labels: ['DEPARTMENT'],
  where: { PROJECT: { $alias: '$project' } },
  select: {
    department: '$record.name',
    projects: { $count: '$project' }
  },
  groupBy: ['$record.name'],
  orderBy: { projects: 'desc' }
})
```

---

## Tool Reference

### Discovery

| Tool                | When to use                                                                                                                  |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `getSchemaMarkdown` | Step 0 — always first, once per session                                                                                      |
| `getSchema`         | Same as above but structured JSON; use when you need `propertyId` values or `vectorIndexes` per property                     |
| `findLabels`        | Skip if `getSchemaMarkdown` already ran                                                                                      |
| `findProperties`    | Discover field names + types for a specific label when not in schema                                                         |
| `findRelationships` | Inspect relationships; `where` filters edge type/properties, `source`/`target` filter endpoint records; no aggregate/groupBy |
| `propertyValues`    | List distinct values for a property (needs `propertyId` from `getSchema` or `findProperties`)                                |

> **Semantic search:** Properties listed as `getSchema` results with a non-empty `vectorIndexes` array are eligible for semantic search. Use `aiSemanticSearch` with the matching `propertyName` and `labels`.

### Querying

| Tool              | When to use                                                                                            |
| ----------------- | ------------------------------------------------------------------------------------------------------ |
| `findRecords`     | Primary query/read/list/search tool — listing, filtering, aggregation, groupBy, semantic search        |
| `findOneRecord`   | Return the first matching record                                                                       |
| `findUniqRecord`  | Return exactly one record (errors if multiple match)                                                   |
| `getRecord`       | Fetch a single record by ID                                                                            |
| `getRecordsByIds` | Fetch multiple records by IDs                                                                          |
| `exportRecords`   | Use only when the user explicitly asks for CSV export/download; do not substitute it for `findRecords` |

### Mutations (confirm before use)

| Tool                                | Notes                                                           |
| ----------------------------------- | --------------------------------------------------------------- |
| `createRecord`                      | Store a new record                                              |
| `updateRecord`                      | Patch fields on an existing record                              |
| `setRecord`                         | Replace all fields on an existing record                        |
| `deleteRecord` / `deleteRecordById` | Destructive — preview first                                     |
| `bulkCreateRecords`                 | Batch insert via nested JSON                                    |
| `bulkDeleteRecords`                 | **Destructive batch** — always preview with `findRecords` first |
| `attachRelation`                    | Create a relationship between two records                       |
| `detachRelation`                    | Remove a relationship                                           |

---

## Resource-Local `where` Rule

Do not reuse `where` blindly across resource types:

| Method              | Notes                                                                                                                                            |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `findRecords`       | `where` applies to Records. Full query: `where` + `select` + `groupBy` + pagination                                                              |
| `findRelationships` | `where` applies to relationship edges (`type` + user-defined edge properties). Use `source`/`target` for endpoint Records; no `select`/`groupBy` |
| `findLabels`        | `where` applies to Records before label counts are returned                                                                                      |
| `findProperties`    | `where` applies to Records before property metadata is returned                                                                                  |
| `exportRecords`     | `where` + `labels` apply to Records; streams CSV only when explicitly requested                                                                  |
| `bulkDeleteRecords` | `where` + `labels` apply to Records; **destructive** — always preview first                                                                      |

The same Record filter used in `findRecords` can be passed directly to `bulkDeleteRecords`, `exportRecords`, `findLabels`, or `findProperties`. For relationship lookup, move Record predicates into `source.where` or `target.where`.

---

## Quick Operator Cheat Sheet

```js
// ── Equality ──────────────────────────────────────────────────────────
name: "Alice"
isActive: true
age: 30

// ── String (all comparisons are case-insensitive) ─────────────────────
name: { $contains: "ali" }
name: { $startsWith: "A" }
name: { $endsWith: "e" }
name: { $ne: "deleted" }
status: { $in: ["active", "pending"] }       // matches any value in list
status: { $nin: ["archived", "deleted"] }    // matches none of these values

// ── Number ────────────────────────────────────────────────────────────
score: { $gt: 90 }
score: { $gte: 70, $lte: 100 }              // between (inclusive)
score: { $in: [85, 90, 95] }
score: { $nin: [0, -1] }

// ── Datetime ──────────────────────────────────────────────────────────
// ISO 8601 strings work for equality, $in, and range operators:
created: "2024-01-01T00:00:00Z"
created: { $gte: "2024-04-23T00:00:00Z" }            // "after a timestamp" / relative ranges
created: { $in: ["2024-01-01T00:00:00Z", "2024-06-01T00:00:00Z"] }

// Use component objects for calendar-semantic ranges (year / month / day boundaries):
created: { $gte: { $year: 2024, $month: 1 }, $lt: { $year: 2024, $month: 2 } }  // January 2024
created: { $gte: { $year: 1990 }, $lt: { $year: 2000 } }                         // the 1990s
// Available components: $year $month $day $hour $minute $second $millisecond $microsecond $nanosecond

// ── Existence / type ──────────────────────────────────────────────────
email: { $exists: true }                     // field is present and not null
phone: { $exists: false }                    // field absent
age:   { $type: "number" }                   // "string"|"number"|"boolean"|"datetime"

// ── Logical ───────────────────────────────────────────────────────────
// Implicit AND — multiple keys at the same level:
where: { status: "active", score: { $gte: 70 } }

// Explicit operators:
$and: [ { status: "active" }, { score: { $gte: 70 } } ]
$or:  [ { plan: "pro" }, { plan: "enterprise" } ]
$not: { status: "deleted" }
$nor: [ { status: "deleted" }, { status: "archived" } ]   // none must match
$xor: [ { isPremium: true }, { hasTrial: true } ]         // exactly one must match

// ── Relationship traversal — key IS the label (UPPER_CASE) ────────────
where: {
  DEPARTMENT: {
    name: "Engineering",
    headcount: { $gte: 10 }
  }
}

// Named traversal ($alias for use in select/groupBy):
where: { DEPARTMENT: { $alias: "$dept", budget: { $gte: 50000 } } }

// Constrain relationship type or direction:
where: { POST: { $relation: { type: "AUTHORED", direction: "in" } } }

// Multihop over one pattern (hierarchies, "within N degrees") — hops inside $relation:
where: { EMPLOYEE: { $relation: { type: "REPORTS_TO", direction: "out", hops: { max: 4 } } } }

// Cycle/ring detection (fraud rings, circular ownership) — $cycle block holds only $relation:
where: { RING: { $cycle: true, $relation: { type: "TRANSFERRED_TO", direction: "out", hops: { min: 2, max: 6 } } } }
```

Each Relationships row in the schema is a directed pattern rooted at that label: `(SELF)-[:TYPE]->(OTHER)` is outgoing, `(SELF)<-[:TYPE]-(OTHER)` is incoming. To pin the edge, set `$relation: { type: "TYPE", direction }` — `"out"` for `->`, `"in"` for `<-`. Only patterns shown in the schema are traversable; a scalar `*_id` property is a plain value, not an edge — never nest a label to "join" on it (root on the owning label and `groupBy` it instead).

**Common mistakes to avoid:**

- `$label`, `$direction`, `$as`, `$of`, `$through`, `$hops` — **these do not exist** (multihop depth is `$relation.hops`; `$relation.hops` and `$cycle` ARE valid)
- `{ employee: { $label: "EMPLOYEE" } }` — **WRONG**: key must be the label name
- `{ EMPLOYEE: { $alias: "$emp" } }` — **CORRECT**
- `labels: ["PARENT", "CHILD"]` for a parent-child metric — **WRONG**: keep `labels: ["PARENT"]`, put `CHILD` in `where`
- `labels: ["CHILD"]` for "which parent has most/fewest children" — **WRONG** when a parent-child path exists: keep the requested parent as root and count the child alias
- `groupBy: ["$record"]` or `["$child"]` — **WRONG**: dimensional `groupBy` must include a property, e.g. `"$record.name"`; self-group uses select key names only
- `{ fieldA: "$record.id" }` or `{ fieldA: { $eq: "$alias.id" } }` — **WRONG**: `where` values must be literals. `$record.*`/`$alias.*` references work only in `select`/`groupBy`/`aggregate`; as a `where` value they are matched as a literal string and return nothing (RushDB has no correlated where predicate)
- Simulating a join on a scalar field that is **not** a relationship in the schema — **WRONG**: root on the label that owns the field and `groupBy` it, e.g. `{ labels: ["LABEL"], select: { count: { $count: "*" } }, groupBy: ["$record.someScalarField"] }`
- `name: "partial user text"` for an incomplete named reference — **RISKY**: prefer `name: { $contains: "partial user text" }`
- For calendar-semantic ranges (year / month / day boundaries) use component objects, not ISO strings
- Do NOT include `limit` when using `select` (produces mathematically wrong results)

---

## Key Limits

- `limit` max = 1000
- **Never use `limit` with `select`** — it restricts the scan and gives wrong totals
- For `"how many"` simple counts: read `total` from the `findRecords` response — do NOT use `fn:'count'`
- For groupBy self-group and dimensional groupBy: omit `limit` unless you want "top N"

---

## When to Load the Full Reference

Call `getSearchQuerySpec` (or read `references/search-query-spec.md`) for:

- **Datetime filters** — ISO 8601 strings work with all operators; use component objects for calendar-semantic ranges (year / month / day)
- **Select expressions** — `$count`, `$sum`, `$avg`, `$min`, `$max`, `$collect`, `$timeBucket`, math ops, `$ref`
- **groupBy modes** — dimensional (`$alias.property`) vs self-group (select key names)
- **Late-ordering rule** — critical for correct totals in self-group queries
- **Relationship traversal** — `$alias`, `$relation`, multi-hop BFS algorithm
- **Vector similarity** — use legacy `aggregate` (not `select`) until select supports it
- **Nested collect** — building tree-shaped results with label-based `$collect`
- **Validation checklist** — before submitting any `findRecords` call
