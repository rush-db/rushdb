# Aggregations

The `select` and `groupBy` fields of a [search query](search-queries.md)
turn a record listing into computed rows: counts, sums, averages, minimums
and maximums, arithmetic over those results, calendar buckets, and nested
collections of related records. This page documents every `select`
operator, how grouping works, and the limits that keep aggregations bounded.

## A first aggregation

Revenue and order count per status, highest revenue first:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": {
        "kind": "records",
        "query": {
          "labels": ["ORDER"],
          "select": {
            "count": { "$count": "*" },
            "revenue": { "$sum": "$record.total" }
          },
          "groupBy": ["$record.status"],
          "order": { "revenue": "desc" }
        }
      }
    }
  }'
```

```json
{
  "kind": "rows",
  "rows": [
    { "status": "paid", "count": 2, "revenue": 150.0 },
    { "status": "pending", "count": 1, "revenue": 25.0 }
  ],
  "total": 3
}
```

`total` is the number of matched records; `rows` holds one row per group.

## `select` reference

Each key of `select` is an output name; its value defines the output.

| Form | Output |
|---|---|
| `"$record.field"` or `"$self.field"` | The current record's property (projection). |
| `"$alias.field"` | A property of a related record bound with `$alias`. |
| `{ "$count": "*" }` | Number of matched root records in the group. |
| `{ "$count": "$record.field" }` | Number of records in the group that have the property. |
| `{ "$count": "$alias" }` | Number of distinct related records bound to the alias. |
| `{ "$sum": "$record.field" }` | Sum of the numeric values. |
| `{ "$sum": { "$multiply": ["$record.price", "$record.qty"] } }` | Sum of a per-record expression. |
| `{ "$avg": "$record.field" }` | Mean of the numeric values. |
| `{ "$avg": "$record.field", "$precision": 2 }` | Mean rounded to 2 decimal places. `$precision: 0` truncates toward zero. The maximum is 15. |
| `{ "$min": "$record.field" }`, `{ "$max": "$record.field" }` | Smallest / largest value. |
| `{ "$add": [a, b] }`, `$subtract`, `$multiply`, `$divide` | Arithmetic over other outputs, see [Derived arithmetic](#derived-arithmetic). |
| `{ "$ref": "outputName" }` | The value of another output. |
| `{ "$timeBucket": { "field": "$record.createdAt", "unit": "month", "size": 1 } }` | Calendar bucket of a datetime, see [Time buckets](#time-buckets). |
| `{ "$collect": { … } }` | An array of related records, see [Collecting related records](#collecting-related-records). |

Field references accept the forms `$record.x`, `$self.x`, and `$x`; property
names are normalized as everywhere else
([Property names](search-queries.md#property-names)). An unknown `$` operator
in `select` is rejected with `invalid_request`.

Value rules:

- `$sum` and `$avg` consider only numeric values; other values are skipped.
  Sums and averages are returned as floating-point numbers (`150.0`).
- An `$avg` over no numeric values is `0`.
- Integers above 2^53 that cannot be represented exactly are rejected rather
  than rounded.
- `$min` and `$max` compare numbers numerically and other values by the same
  rules as [sorting](sorting-and-pagination.md#comparison-rules).

## How `select` and `groupBy` shape the rows

| Document | Rows |
|---|---|
| `select` with metrics only, no `groupBy` | One global row, for example `{ "n": 42 }`. |
| `select` with projections (field references) and no `groupBy` | One row per matching record, carrying the projections and any metrics computed for that record. |
| `select` + `groupBy` | One row per distinct combination of group keys. |
| `groupBy` naming only `select` output names | One global row (see [Grouping](#grouping)). |

## Grouping

`groupBy` lists the properties to group on, as `"$record.field"` or
`"field"`:

```json
{
  "labels": ["BATTLE"],
  "select": {
    "faction": "$record.winner_faction_id",
    "battles_won": { "$count": "*" }
  },
  "groupBy": ["$record.winner_faction_id"],
  "order": { "battles_won": "desc" }
}
```

```json
[
  { "winner_faction_id": "faction_rebel_alliance", "battles_won": 4 },
  { "winner_faction_id": "faction_galactic_republic", "battles_won": 2 },
  { "winner_faction_id": "faction_sith", "battles_won": 1 }
]
```

Rules:

- Each row carries its group keys under the **property name as `groupBy`
  spells it** (`winner_faction_id` above; `$record.winningDriverRef` gives
  `winningDriverRef`), followed by the metric outputs. Plain
  projections in `select` are not emitted as separate columns in grouped
  output.
- Records that lack a group property fall into a group whose key value is
  the string `"__missing__"`.
- If every `groupBy` entry is the name of a `select` output, the query is a
  single global rollup. This is the idiom for KPI rows that combine several
  metrics:

```json
{
  "labels": ["STARSHIP"],
  "predicate": { "is_military": true },
  "select": {
    "totalCrewCapacity": { "$sum": "$record.crew_capacity" },
    "shipCount": { "$count": "*" },
    "crewPerShip": { "$divide": [{ "$ref": "totalCrewCapacity" }, { "$ref": "shipCount" }] }
  },
  "groupBy": ["totalCrewCapacity", "shipCount", "crewPerShip"]
}
```

### Ordering and paging rows

`order` sorts the rows by output name (metric or group key), and `offset` /
`limit` then slice the sorted rows. Every group is computed before ordering,
so `limit` does not reduce the work or the group limit. A plain projection
(one row per record) is the exception: it is not subject to the group
limit, and without an `order` only the requested page of records is read.

## Derived arithmetic

`$add`, `$subtract`, `$multiply` and `$divide` take exactly two operands and
are evaluated after the metrics of each row are computed. An operand can
be:

| Operand | Meaning |
|---|---|
| A number | A literal. |
| `{ "$ref": "name" }` or `"name"` | Another output of the same row, including `$count` outputs. |
| An inline metric, for example `{ "$sum": "$record.price" }` | Computed as a hidden output and used here. |
| Another arithmetic object | Nested arithmetic. |

```json
{
  "labels": ["ITEM"],
  "select": {
    "revenuePerItem": {
      "$divide": [{ "$sum": "$record.price" }, { "$count": "*" }]
    }
  }
}
```

Division by zero fails the query instead of producing `Infinity`.

Inside `$sum` and `$avg`, arithmetic is evaluated **per record** instead,
and string operands there are field references:
`{ "$sum": { "$multiply": ["$record.price", "$record.qty"] } }`.

## Time buckets

`$timeBucket` groups records by a calendar interval of a datetime property.
Name the bucket in `select` and list that name in `groupBy`:

```json
{
  "labels": ["EVENT"],
  "select": {
    "month": { "$timeBucket": { "field": "$record.created", "unit": "month" } },
    "events": { "$count": "*" }
  },
  "groupBy": ["month"],
  "order": { "month": "asc" }
}
```

| Option | Default | Values |
|---|---|---|
| `field` | — | Datetime property reference. |
| `unit` | `month` | `second`/`seconds`, `minute`/`minutes`, `hour`/`hours`, `day`/`days`, `week`/`weeks`, `month`/`months`, `quarter`, `year`/`years` |
| `size` | `1` | Units per bucket, at least 1. |

- The bucket value is the bucket's start as epoch milliseconds (UTC).
- `month`, `quarter` and `year` buckets align to calendar boundaries and
  always span one unit; `size` applies to the fixed-width units (`second`
  through `week`), which align to multiples of their width since the Unix
  epoch.
- Records without the property land in the `"__missing__"` bucket.
- Only one `$timeBucket` per query is supported.

## Metrics over related records

A metric whose target is a traversal alias aggregates over the related
records bound by that alias, applying every predicate on the alias path:

```json
{
  "labels": ["Company"],
  "predicate": { "Employee": { "$alias": "$employee" } },
  "select": {
    "companyName": "$record.name",
    "headcount": { "$count": "$employee" },
    "totalWage": { "$sum": "$employee.salary" },
    "minSalary": { "$min": "$employee.salary" },
    "maxSalary": { "$max": "$employee.salary" }
  }
}
```

```json
[
  { "companyName": "TechCorp", "headcount": 2, "totalWage": 1150000.0, "minSalary": 550000.0, "maxSalary": 600000.0 },
  { "companyName": "OldCorp", "headcount": 1, "totalWage": 480000.0, "minSalary": 480000.0, "maxSalary": 480000.0 }
]
```

Because `companyName` is a projection, there is one row per company. See
[Aliases](relationships-and-traversal.md#aliases) for how aliases are bound.

## Collecting related records

`$collect` attaches an array of related records to each output row or
record. A `select` made **only** of `$collect` entries keeps the query a
listing: the response is `records`, and each record carries the collected
array under the select key inside `properties`. Mixed with other outputs,
the arrays are attached to each row.

### From an alias

```json
{
  "labels": ["Company"],
  "predicate": { "Employee": { "$alias": "$employee" } },
  "select": {
    "employeeNames": {
      "$collect": {
        "from": "$employee",
        "select": { "name": "$employee.name" },
        "orderBy": { "name": "asc" },
        "limit": 10
      }
    }
  }
}
```

`from` follows the alias's full path (every hop and every level's
predicates), so multi-level aliases collect records several hops away.

### By label

```json
{
  "labels": ["Company"],
  "select": {
    "departments": {
      "$collect": {
        "label": "Department",
        "where": { "name": { "$startsWith": "Eng" } },
        "select": {
          "name": "$self.name",
          "projects": {
            "$collect": {
              "label": "Project",
              "select": { "name": "$self.name" }
            }
          }
        }
      }
    }
  }
}
```

A label-based collect follows the current record's **outgoing**
relationships of any type to records with that label. Nested `$collect`
entries in `select` build hierarchical output, each level relative to the
collected record above it.

### `$collect` options

| Option | Default | Meaning |
|---|---|---|
| `from` | — | Alias to collect (mutually exclusive with `label`). An unknown alias is `invalid_request`. |
| `label` | — | Label of directly related records to collect. |
| `select` | whole record | Map of output key to field reference, or to a nested `{ "$collect": … }`. |
| `where` | none | Predicate on each collected record ([Filtering](filtering.md) syntax). |
| `orderBy` | related-record order | `{ "outputKey": "asc" \| "desc" }`; keys may name output keys or stored properties. |
| `skip` | `0` | Collected items to skip. |
| `limit` | all | Maximum collected items. |
| `unique` | `true` | Drop duplicate records reached through several paths. |

## Limits and budgets

| Limit | Value | Error when exceeded |
|---|---|---|
| Distinct groups (rows before paging) in one grouped or rolled-up aggregation | 1,000 | `resource_exhausted` (not retryable): "aggregate exceeds 1000 distinct groups" |
| Aggregation working memory (match set, retained rows, groups, collected arrays) | the per-query budget, `resources.query-memory-max` (default 256 MiB) | `resource_exhausted`: "aggregate working byte budget exceeded" |
| `$avg` `$precision` | 15 | Clamped to 15 |
| `$timeBucket` `size` | at least 1 | `invalid_request` |

Aggregations never return partial results: when a limit is hit, the whole
query fails. To stay within them, filter with `predicate` and group on
lower-cardinality properties. Per-record projections over large match sets
are bounded by the memory budget; page them with `limit` (see
[Plain projection](search-queries.md#plain-projection)).

> **Note:** A global `{ "$count": "*" }` with no predicate on a single label
> is answered from the label catalog in constant time. Any other count
> evaluates the match set but never reconstructs record contents unless a
> predicate requires it.

## See also

- [Search queries](search-queries.md)
- [Relationships and traversal](relationships-and-traversal.md)
- [Facets](facets.md)
- [Sorting and pagination](sorting-and-pagination.md)
- [Resource governance](../architecture/resource-governance.md)
