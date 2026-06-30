---
name: rushdb-faceted-search
description: Build faceted filter UIs on top of RushDB. Use this skill when generating code for filter sidebars, multi-select facets, range sliders, or any UI that lets users narrow a record set by property values. Covers the full flow: discover properties + types, enumerate distinct values, map each type to the right widget, and assemble the live where clause as filters change.
homepage: https://rushdb.com
---

# RushDB Faceted Search

Build filter UIs that stay in sync with your data schema. No hardcoded field names, no hardcoded option lists — everything is derived from the live schema.

## Prerequisites

- **RushDB MCP server** must be connected — it provides `getSchema`, `propertyValues`, `findRecords`, and all other tools used in this skill. Setup: `npx @rushdb/mcp-server` (requires `RUSHDB_API_KEY` env var). See https://docs.rushdb.com/mcp-server/quickstart
- If the MCP tools are not available in the current session, tell the user the MCP server is not configured.

---

## The 4-Step Faceted Search Workflow

```
Step 1  getSchema(labels?)           → property names, types, ids, value ranges
Step 2  propertyValues(propertyId)     → distinct values or {min, max} per property
Step 3  Map type → UI widget
Step 4  findRecords(labels, where)     → filtered results as user interacts
```

---

## Step 1 — Discover Properties with `getSchema`

`getSchema` (not `getSchemaMarkdown`) returns **structured JSON** with `id` per property. The `id` is required for `propertyValues`.

```json
// Tool call
{ "tool": "getSchema", "input": { "labels": ["PRODUCT"] } }

// Response shape (one label entry)
{
  "label": "PRODUCT",
  "count": 4821,
  "properties": [
    { "id": "prop_abc", "name": "category",   "type": "string" },
    { "id": "prop_def", "name": "price",      "type": "number",   "min": 4.99, "max": 1299.99 },
    { "id": "prop_ghi", "name": "inStock",    "type": "boolean" },
    { "id": "prop_jkl", "name": "listedAt",   "type": "datetime", "min": "2023-01-01T00:00:00Z", "max": "2026-04-13T00:00:00Z" },
    {
      "id": "prop_mno", "name": "brand", "type": "string",
      "vectorIndexes": [
        { "id": "idx_001", "sourceType": "managed", "similarityFunction": "cosine",
          "dimensions": 1536, "status": "ready", "modelKey": "text-embedding-3-small" }
      ]
    }
  ]
}
```

**What to capture per property:** `id`, `name`, `type`, and `min`/`max` when present. If `vectorIndexes` is present and non-empty on a string property, it supports semantic search in addition to exact/prefix matching — use `aiSemanticSearch` with that `propertyName`.

---

## Step 2 — Enumerate Values with `propertyValues`

Call `propertyValues` only for **string** and **boolean** properties. Number and datetime properties already expose `min`/`max` from `getSchema` — use those directly for range inputs.

```json
// Tool call — string property
{ "tool": "propertyValues", "input": { "propertyId": "prop_abc" } }

// Response — array of distinct values with counts
[
  { "value": "Electronics", "count": 1240 },
  { "value": "Clothing",    "count": 980  },
  { "value": "Home",        "count": 760  }
]

// Tool call — boolean property
{ "tool": "propertyValues", "input": { "propertyId": "prop_ghi" } }

// Response
[
  { "value": true,  "count": 3200 },
  { "value": false, "count": 1621 }
]
```

**Context-aware values:** Pass `where` + `labels` to `propertyValues` to restrict enumerated options to records matching current active filters. This keeps other facets in sync as the user narrows the set.

```json
{
  "tool": "propertyValues",
  "input": {
    "propertyId": "prop_abc",
    "where": { "inStock": true },
    "labels": ["PRODUCT"]
  }
}
```

---

## Step 3 — Map Property Type to UI Widget

| Property type | `min`/`max` present | Recommended widget       | Notes                                                    |
| ------------- | ------------------- | ------------------------ | -------------------------------------------------------- |
| `string`      | —                   | Multi-select checkboxes  | Load options lazily (on open) for large value sets       |
| `string`      | —                   | Search-as-you-type input | Pass `query` param to `propertyValues` to filter options |
| `boolean`     | —                   | Toggle / checkbox pair   | Always eager-load; only 2 values                         |
| `number`      | yes                 | Range slider             | Use `min`/`max` from schema as bounds                    |
| `number`      | no                  | Number input             | Fallback when no range data                              |
| `datetime`    | yes                 | Date range picker        | Use `min`/`max` as calendar bounds                       |
| `datetime`    | no                  | Date input               | Fallback                                                 |

**Loading strategy:**

- **Eager** (load immediately): `boolean`, `number`, `datetime` — small/bounded data
- **Lazy** (load on popover/dropdown open): `string` with many values — avoids N simultaneous requests on mount

---

## Step 4 — Build the `where` Clause

As the user selects values, assemble a `where` object and pass it to `findRecords`.

### Multi-select (string)

```json
{ "category": { "$in": ["Electronics", "Clothing"] } }
```

### Boolean toggle

```json
{ "inStock": true }
```

### Number range

```json
{ "price": { "$gte": 10, "$lte": 500 } }
```

### Datetime range

```json
{
  "listedAt": {
    "$gte": { "$year": 2025, "$month": 1, "$day": 1 },
    "$lte": { "$year": 2026, "$month": 4, "$day": 13 }
  }
}
```

**Datetime rules:**

- ISO 8601 strings are valid for **equality and `$in`**: `"listedAt": "2025-01-01T00:00:00Z"`
- For **calendar-based range comparisons** (`$gte`/`$lte`/`$gt`/`$lt`), use component objects (`{ $year, $month, $day, ... }`) — plain ISO strings do not work with range operators
- For **relative ranges** ("last 7 days"), compute the ISO UTC boundary on the client and use it as a plain string with `$gte`: `"listedAt": { "$gte": "2026-04-06T00:00:00Z" }`
- See `rushdb-query-builder` / `references/search-query-spec.md` for the full datetime operator reference

### Combining filters (AND by default)

```json
{
  "category": { "$in": ["Electronics"] },
  "price": { "$gte": 50, "$lte": 800 },
  "inStock": true
}
```

All top-level keys in `where` are AND-ed automatically. For OR across properties, wrap in `$or`.

---

## Generated Code Pattern

When generating a faceted filter component, use the **RushDB SDK** — not raw `fetch()` or REST calls directly. The SDK methods map 1:1 to the discovery workflow above and handle auth, base URL, and response unwrapping.

### SDK setup

**TypeScript / JavaScript**

```typescript
import RushDB from '@rushdb/javascript-sdk'
// npm install @rushdb/javascript-sdk

const db = new RushDB(process.env.RUSHDB_API_KEY!)
// Self-hosted: new RushDB(key, { url: 'https://your-host/api/v1' })
```

**Python**

```python
from rushdb import RushDB
# pip install rushdb

db = RushDB(api_key=os.environ["RUSHDB_API_KEY"])
# Self-hosted: RushDB(api_key=key, base_url="https://your-host/api/v1")
```

### 1. Load schema on mount

**TypeScript**

```typescript
const schema = await db.properties.find({ labels: ['PRODUCT'] })
// schema.data: Array<{ id, name, type, min?, max? }>
```

**Python**

```python
schema = db.properties.find(labels=["PRODUCT"])
# schema["data"]: list of { id, name, type, min?, max? }
```

### 2. Load values per property

**TypeScript**

```typescript
// Eager: boolean, number (use min/max directly), datetime (use min/max directly)
// Lazy: string properties — call only when filter panel opens

// String: enumerate options (context-aware)
const values = await db.properties.values(propertyId, { where, labels })
// values.data: Array<{ value, count }>
```

**Python**

```python
values = db.properties.values(property_id, where=where, labels=labels)
# values["data"]: list of { value, count }
```

### 3. Track active filters in state

**TypeScript (React)**

```typescript
type ActiveFilters = Record<string, unknown> // mirrors the where clause
const [filters, setFilters] = useState<ActiveFilters>({})

function setFilter(name: string, value: unknown) {
  setFilters((prev) =>
    value == null ?
      Object.fromEntries(Object.entries(prev).filter(([k]) => k !== name))
    : { ...prev, [name]: value }
  )
}
```

**Python (plain dict — no framework dependency)**

```python
filters: dict = {}

def set_filter(filters: dict, name: str, value) -> dict:
    if value is None:
        return {k: v for k, v in filters.items() if k != name}
    return {**filters, name: value}
```

### 4. Fetch results reactively

**TypeScript**

```typescript
// Re-run on every filter change
const results = await db.records.find({ labels: ['PRODUCT'], where: filters })
```

**Python**

```python
results = db.records.find({'labels': ['PRODUCT'], 'where': filters})
```

### 5. Reset filters

**TypeScript**

```typescript
// Full reset
setFilters({})

// Single facet reset
setFilter('category', null)
```

**Python**

```python
# Full reset
filters = {}

# Single facet reset
filters = set_filter(filters, "category", None)
```

---

## Two-Hook Pattern (reference implementations)

The RushDB example apps use a two-hook pattern worth replicating:

**`useProperties`** — loads properties for the active label scope (schema discovery):

_TypeScript_

```typescript
db.properties.find({ where, labels, skip, limit })
```

_Python_

```python
db.properties.find(labels=labels, where=where, skip=skip, limit=limit)
```

**`usePropertyValues(propertyId)`** — loads distinct values for one property (context-aware):

_TypeScript_

```typescript
db.properties.values(propertyId, { where, labels, query })
```

_Python_

```python
db.properties.values(property_id, where=where, labels=labels, query=query)
```

Generate both hooks/functions and compose them in the filter sidebar. Each facet component receives a `propertyId` and calls `usePropertyValues` / `get_property_values` independently, so panels load in parallel and lazily.

---

## "Original" vs "Context-Aware" Values

Some UIs show two states per facet:

- **Context-aware options** (filtered by current `where`): what's available given active filters
- **Original options** (no `where`): the full universe, useful for showing greyed-out unavailable options

To get original values, call `propertyValues` without `where`:

```json
{ "tool": "propertyValues", "input": { "propertyId": "prop_abc" } }
```

To get context-aware values, include the current `where`:

```json
{ "tool": "propertyValues", "input": { "propertyId": "prop_abc", "where": { "inStock": true } } }
```

---

## Active Filter Display

Always render an "active filters" summary bar so users can see and remove individual filters:

```typescript
// Derive chips from filters state
const chips = Object.entries(filters).map(([field, value]) => ({
  label: `${field}: ${formatValue(value)}`,
  onRemove: () => setFilter(field, null)
}))
```

---

## Reference

For the full datetime operator syntax and `$in` / `$gte` / `$lte` reference:
Load `../rushdb-query-builder/references/search-query-spec.md` or call the MCP tool `getSearchQuerySpec`.
