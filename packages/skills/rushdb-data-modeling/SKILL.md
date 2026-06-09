---
name: rushdb-data-modeling
description: Model data in RushDB's property-centric LMPG graph. Use this skill when designing a schema, choosing label names, planning property structure, defining relationships, importing nested JSON data, or evolving an existing schema. Also use when the user asks how RushDB stores data, how relationships work, or how to structure records for a new use case.
homepage: https://rushdb.com
---

# RushDB Data Modeling

RushDB uses the **LMPG** (Labels, Multi-Properties, Graph) model — a property-centric graph where every node (record) has a label, a set of typed properties, and can be connected to other records via named relationships.

**Core insight**: you don't define a schema upfront. You push JSON, and RushDB infers structure. Labels and properties emerge from your data. Schema discovery happens via `getOntologyMarkdown`.

---

## The LMPG Model

| Concept    | RushDB term      | Description                                                     |
| ---------- | ---------------- | --------------------------------------------------------------- |
| Node       | **Record**       | A single data entity — like a row, but with flexible properties |
| Node type  | **Label**        | The record's category/type (`USER`, `ORDER`, `PROJECT`)         |
| Attributes | **Properties**   | Key-value pairs with typed values attached to a record          |
| Edge       | **Relationship** | A named, directed connection between two records                |

Records are schema-free: two `USER` records can have different property sets. Properties carry types: `string`, `number`, `boolean`, `datetime`, `vector`, `null`.

---

## Labels

### Naming Rules

- **Always UPPER_CASE** — RushDB is case-sensitive. `User` ≠ `USER`. Stick to `USER`.
- Singular noun: `USER` not `USERS`, `ORDER` not `ORDERS`
- No spaces or special characters — use underscores: `BLOG_POST`

### Choosing Labels

Labels should reflect the **entity type**, not the action or state:

| Good        | Avoid                            |
| ----------- | -------------------------------- |
| `USER`      | `REGISTERED_USER`, `ACTIVE_USER` |
| `ORDER`     | `PLACED_ORDER`, `ORDER_ENTITY`   |
| `BLOG_POST` | `PUBLISHED_CONTENT`, `POST_NODE` |

Use a single label per record type. Model state with a `status` property, not separate labels.

### Multi-Label Queries

`findRecords` with `labels: ["ARTICLE", "BLOG_POST"]` returns both types (OR). Useful for shared abstract types (e.g., all content records).

---

## Properties

### Supported Types

| Type       | Example values           | Notes                                                          |
| ---------- | ------------------------ | -------------------------------------------------------------- |
| `string`   | `"hello"`, `"active"`    | Case-sensitive equality; case-insensitive for `$contains` etc. |
| `number`   | `42`, `3.14`, `-100`     | Supports full range operators                                  |
| `boolean`  | `true`, `false`          |                                                                |
| `datetime` | `"2026-04-10T09:00:00Z"` | ISO 8601 UTC format. Use component objects for range queries.  |
| `vector`   | `[0.1, 0.2, ..., 0.9]`   | Float array; used for semantic/similarity search               |
| `null`     | `null`                   | Explicit absence; queryable via `$type: "null"`                |

### Naming Conventions

- **camelCase** for all property names: `createdAt`, `isActive`, `userId`
- Consistent types per property across all records of the same label
- Avoid reusing a property name with different types across records of the same label
- ISO 8601 for all timestamps: `"2026-04-10T09:00:00Z"` (always UTC)
- Booleans as `isX` or `hasX`: `isActive`, `hasAccess`, `isPremium`

### Property Discovery

```
getOntologyMarkdown()         → all labels + all properties + value ranges
findProperties({ labels: ["USER"] })  → detailed property list for a label
getOntology()                 → same as above but structured JSON with propertyId values
propertyValues(propertyId)    → distinct values / min-max for a property
```

---

## Relationships

### How Auto-Linking Works

When you pass nested objects to `bulkCreateRecords`, RushDB creates:

- One record per nested label key
- One relationship per parent→child nesting level
- Relationship type defaults to the uppercase label name

Example — this single import:

```json
{
  "label": "COMPANY",
  "data": {
    "name": "Acme Corp",
    "DEPARTMENT": [
      {
        "name": "Engineering",
        "EMPLOYEE": [
          { "name": "Alice", "role": "Engineer" },
          { "name": "Bob", "role": "Manager" }
        ]
      }
    ]
  }
}
```

Produces:

- 1 `COMPANY` record
- 1 `DEPARTMENT` record → linked to COMPANY
- 2 `EMPLOYEE` records → each linked to DEPARTMENT
- Relationships: `COMPANY -[HAS_DEPARTMENT]→ DEPARTMENT`, `DEPARTMENT -[HAS_EMPLOYEE]→ EMPLOYEE`

No schema declaration. No migration. Just push the JSON.

### Manual Relationship Management

```
attachRelation({ sourceId, targetId, type: "AUTHORED" })
detachRelation({ sourceId, targetId, type: "AUTHORED" })
findRelationships({ source: { where: { $id: "record-id" } } })
findRelationships({ target: { where: { $id: "record-id" } } })
```

### Relationship Naming Conventions

- **UPPER_SNAKE_CASE**: `AUTHORED_BY`, `BELONGS_TO`, `DEPENDS_ON`, `HAS_TASK`
- Directional and readable: `AUTHOR -[WROTE]→ POST`, `SESSION -[HAS_DECISION]→ DECISION`
- Verb-based: describes the action/association from source to target

### Relationship Query Patterns

Relationships are traversed in `where` by using the target label as the key:

```json
{
  "labels": ["COMPANY"],
  "where": {
    "DEPARTMENT": {
      "name": "Engineering",
      "$relation": { "type": "HAS_DEPARTMENT", "direction": "out" }
    }
  }
}
```

Omit `$relation` when direction and type are not important — RushDB traverses any edge.

---

## Data Import Patterns

### Flat Records (simple entities)

Use `createRecord` for individual records with no nesting:

```json
{
  "label": "PRODUCT",
  "data": {
    "name": "Widget Pro",
    "sku": "WGT-PRO-001",
    "price": 49.99,
    "category": "hardware",
    "inStock": true,
    "createdAt": "2026-04-10T00:00:00Z"
  }
}
```

### Hierarchical Data (nested structures)

Use `bulkCreateRecords` with nested label keys for structures like org charts, project trees, or content hierarchies. The nesting depth can be arbitrary.

### CSV / Flat Files

Use `exportRecords` to export existing records to CSV. For import, use `bulkCreateRecords` by transforming flat rows into records with a consistent label.

### Enum Fields

Define a consistent set of values upfront and document them as a `status` or `type` field comment. Enforce consistency via `propertyValues` queries during writes if needed.

---

## Schema Discovery Workflow

When working with an existing database:

1. **`getOntologyMarkdown()`** — see all labels, their property names/types, and relationships
2. **`findProperties({ labels: ["LABEL"] })`** — deep-dive a specific label
3. **`propertyValues(propertyId)`** — enumerate distinct string/boolean values or get number/date ranges
4. **`findRelationships({ source: { where: { $id: "sample-id" } } })`** and **`findRelationships({ target: { where: { $id: "sample-id" } } })`** — explore outgoing and incoming connections from a sample record

Never guess label names or property names — always discover them first.

---

## Schema Evolution Patterns

### Add a property

Just include it in new records. Existing records that lack it are unmatched by filters on that field — they're not broken.

### Change a property name

1. `findRecords` to get all affected records
2. `updateRecord` each with the new property name added and old removed
3. There is no schema migration — just update the data

### Add a label

Start creating records with the new label — it appears in `getOntologyMarkdown` once at least one record exists.

### Retire a label

1. Add `deprecated: true` to all existing records of that label
2. Stop creating new ones
3. Filter with `{ deprecated: { $ne: true } }` in queries until fully cleaned up
4. Delete with `bulkDeleteRecords` (always preview first)

### Split a label into two

1. `findRecords` on old label, classify each record by new label
2. `bulkCreateRecords` into new labels
3. Re-attach relationships with `attachRelation`
4. Mark old records as deprecated

---

## Common Modeling Mistakes

| Mistake                                   | Correct approach                                                         |
| ----------------------------------------- | ------------------------------------------------------------------------ |
| Label as lowercase: `user`, `order`       | Always UPPER_CASE: `USER`, `ORDER`                                       |
| Storing state in the label: `ACTIVE_USER` | Store state as `status: "active"` on `USER`                              |
| One giant label with all fields           | Split into focused labels linked by relationships                        |
| Guessing property names                   | Call `getOntologyMarkdown` or `findProperties` first                     |
| Hardcoding enum values                    | Probe with `propertyValues` before filtering                             |
| Plain date strings in range queries       | Use component objects: `{ $gte: { $year: 2024 } }`                       |
| Inventing `$label`/`$direction` operators | Use the label as the key, `$alias` for naming, `$relation` for edge type |
