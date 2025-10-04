---
sidebar_position: 8
---

# Grouping Search Results (`groupBy`)

The `groupBy` option in a SearchQuery lets you pivot, summarize, and aggregate records instead of returning a raw list. It works together with the `aggregate` clause. If no aggregations are provided, `groupBy` is ignored.

## Core Principles

1. Each entry in `groupBy` must reference an **alias** followed by a property: `"$alias.property"`.
2. The root record always has the implicit alias `$record`.
3. Output rows contain:
   - One key per grouped property (key name = property name, without alias prefix).
   - All aggregation outputs you defined.
4. Grouping changes the shape from a collected list of root records to one row per distinct combination of group keys.
5. Nested hierarchy is not auto-generated; to build nested structures, mix `collect` for arrays with grouping on a parent level.

## Syntax Overview

```typescript
{
  labels: ['ORDER'],
  aggregate: {
    count: { fn: 'count', alias: '$record' },
    avgTotal: { fn: 'avg', field: 'total', alias: '$record' }
  },
  groupBy: ['$record.status'],
  orderBy: { count: 'desc' },
  limit: 1000
}
```

Result (conceptual):
```json
[
  { "status": "pending", "count": 120, "avgTotal": 310.42 },
  { "status": "completed", "count": 85,  "avgTotal": 512.10 }
]
```

## Declaring Aliases for Related Records

To group by related data, first traverse it in `where` and assign an alias via `$alias`:

```typescript
{
  labels: ['DEPARTMENT'],
  where: {
    PROJECT: { $alias: '$project' }
  },
  aggregate: {
    projectCount: { fn: 'count', alias: '$project' },
    projectNames: { fn: 'collect', field: 'name', alias: '$project', unique: true }
  },
  groupBy: ['$record.name'],
  orderBy: { projectCount: 'desc' }
}
```

## Multiple Group Keys (Pivot Style)

```typescript
{
  labels: ['PROJECT'],
  aggregate: { count: { fn: 'count', alias: '$record' } },
  groupBy: ['$record.category', '$record.active'],
  orderBy: { count: 'desc' }
}
```

Produces one row per (category, active) combination.

## Mixing `collect` with Grouping

Use `collect` to retain arrays inside grouped rows:

```typescript
{
  labels: ['DEPARTMENT'],
  where: { EMPLOYEE: { $alias: '$employee' } },
  aggregate: {
    employees: { fn: 'collect', field: 'name', alias: '$employee', unique: true },
    employeeCount: { fn: 'count', alias: '$employee' }
  },
  groupBy: ['$record.name'],
  orderBy: { employeeCount: 'desc' }
}
```

Each row: `{ name: <departmentName>, employees: [...], employeeCount: N }`.

## Grouping by Deeply Nested Aliases

```typescript
{
  labels: ['HS_DEAL'],
  where: {
    HS_APPOINTMENT: { $alias: '$appointment' }
  },
  aggregate: {
    count: { fn: 'count', alias: '$record' },
    avgAmount: { fn: 'avg', field: 'amount', alias: '$record' }
  },
  groupBy: ['$appointment.hs_meeting_location'],
  orderBy: { count: 'desc' }
}
```

## Default Uniqueness & Counting Behavior

- `collect` is **unique by default**; set `unique: false` to keep duplicates.
- `count` without `field` counts distinct records for the specified alias.
- To count field occurrences (with possible duplicates):
  ```typescript
  { fn: 'count', field: 'country', alias: '$employee', unique: false }
  ```

## Ordering Grouped Results

You can sort by aggregated fields or by grouped keys (since they appear in output rows):
```typescript
orderBy: { count: 'desc' }
// or
orderBy: { dealstage: 'asc' }
```

## Common Patterns

| Scenario                                   | Approach |
|-------------------------------------------|----------|
| KPI dashboard by status                   | Group by `$record.status` + count/avg aggregates |
| Department summary with project list      | Group by department name + `collect` project names |
| Sales funnel by stage                     | Group by `$record.dealstage` with count & sum |
| Pivot by category + active flag           | Two keys in `groupBy` |
| Group by related location (appointments)  | Alias nested traversal then group by that alias field |

## Limitations & Tips

- No hierarchical multi-pass grouping; emulate with `collect` arrays.
- At least one aggregation is required; otherwise grouping is ignored.
- Ensure every alias used in `groupBy` is declared in `where` via `$alias`.
- Group keys appear in the result using only the property segment (e.g. `$record.stage` -> `stage`).
- Keep group key count small for performance (high cardinality => more result rows).

## Error Prevention Checklist

- Typo in alias? Ensure `$alias` appears exactly once at the traversal level.
- Group field not appearing? Confirm property exists on that label and is projected (root properties automatically available; related ones via alias reference).
- Unexpected duplicate rows? Add more group keys or remove unnecessary keys.

## See Also
- [Aggregations](./aggregations.md)
- SDK references: TypeScript / Python / REST *get records* pages
