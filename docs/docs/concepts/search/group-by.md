---
sidebar_position: 8
---

# Grouping Search Results (`groupBy`)

The `groupBy` option in a SearchQuery lets you pivot, summarize, and compute metrics on records instead of returning a raw list. It works together with the `select` clause. If no select expressions are provided, `groupBy` is ignored. The legacy `aggregate` clause is deprecated and should only be used for vector similarity until select supports it.

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
  select: {
    count:    { $count: '*' },
    avgTotal: { $avg: '$record.total' }
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
  select: {
    projectCount: { $count: '$project' },
    projectNames: {
      $collect: { from: '$project', select: { name: '$project.name' }, unique: true }
    }
  },
  groupBy: ['$record.name'],
  orderBy: { projectCount: 'desc' }
}
```

## Multiple Group Keys (Pivot Style)

```typescript
{
  labels: ['PROJECT'],
  select: { count: { $count: '*' } },
  groupBy: ['$record.category', '$record.active'],
  orderBy: { count: 'desc' }
}
```

Produces one row per (category, active) combination.

## Grouping Only by an Aggregated Value (Self Group)

Sometimes you only want the aggregated metric itself (e.g. total sum or total count) without grouping by any actual property of the records. RushDB supports a lightweight pattern for this: list the aggregation key name itself inside `groupBy`.

Why is this needed? The aggregation engine always emits rows; normally those rows must include at least one group key. If you have only aggregations (e.g. total count) and no natural dimension to group on, you can instruct the planner to treat the aggregation output as the sole grouping key – effectively returning just that metric.

Example – total deal amount across all deals:

```typescript
{
  labels: ['HS_DEAL'],
  select: {
    totalAmount: { $sum: '$record.amount' }
  },
  groupBy: ['totalAmount']
}
```

Result shape (conceptual):
```json
[{ "totalAmount": 123456.78 }]
```

Notes:
- You still get an array (with one row) for consistency.
- If the aggregation key appears in `groupBy`, it is not redundantly projected again as a normal field – it serves as both key and value.
- Add more aggregation keys the same way if you want multiple single-value metrics (each must be repeated in `groupBy`).

Multiple metrics example:

```typescript
{
  labels: ['ORDER'],
  select: {
    totalRevenue: { $sum: '$record.total' },
    orderCount:   { $count: '*' }
  },
  groupBy: ['totalRevenue', 'orderCount']
}
```

Conceptual result:
```json
[{ "totalRevenue": 987654.32, "orderCount": 420 }]
```

When to use:
- KPI endpoints needing a compact payload
- Dashboards showing headline numbers
- Reducing post-processing (no need to pluck values out of nested objects)

Avoid if:
- You actually need per-dimension breakdown (then group by a property instead)
- The metric might be extremely large cardinality (self-group is for single or tiny row counts)

## Mixing `collect` with Grouping

Use `collect` to retain arrays inside grouped rows:

```typescript
{
  labels: ['DEPARTMENT'],
  where: { EMPLOYEE: { $alias: '$employee' } },
  select: {
    employees:     { $collect: { from: '$employee', select: { name: '$employee.name' }, unique: true } },
    employeeCount: { $count: '$employee' }
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
  select: {
    count:     { $count: '*' },
    avgAmount: { $avg: '$record.amount' }
  },
  groupBy: ['$appointment.hs_meeting_location'],
  orderBy: { count: 'desc' }
}
```

## Default Uniqueness & Counting Behavior

- `collect` is **unique by default**; set `unique: false` to keep duplicates.
- `count` without `field` counts distinct records for the specified alias.
- To count field occurrences:
  ```typescript
  { $count: '$employee.country' }
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
| KPI dashboard by status                   | Group by `$record.status` + `select` count/avg expressions |
| Department summary with project list      | Group by department name + `collect` project names |
| Sales funnel by stage                     | Group by `$record.dealstage` with count & sum |
| Pivot by category + active flag           | Two keys in `groupBy` |
| Group by related location (appointments)  | Alias nested traversal then group by that alias field |

## Limitations & Tips

- No hierarchical multi-pass grouping; emulate with `collect` arrays.
- At least one `select` expression is required; otherwise grouping is ignored.
- Ensure every alias used in `groupBy` is declared in `where` via `$alias`.
- Group keys appear in the result using only the property segment (e.g. `$record.stage` -> `stage`).
- Keep group key count small for performance (high cardinality => more result rows).

## Error Prevention Checklist

- Typo in alias? Ensure `$alias` appears exactly once at the traversal level.
- Group field not appearing? Confirm property exists on that label and is projected (root properties automatically available; related ones via alias reference).
- Unexpected duplicate rows? Add more group keys or remove unnecessary keys.

## See Also
- [Select Expressions](./select.md)
- SDK references: TypeScript / Python / REST *get records* pages
