# Relationships and traversal

RushDB records are connected by typed, directed relationships. This page
shows how to filter records by what they are connected to, control the
direction, type and depth of the traversal, detect cycles, bind related
records to aliases for aggregation, list relationships themselves, and count
property-overlap joins between two record sets.

For how relationships are created, see [Writing data](writing-data.md#relationships).
For the data model, see [Relationships](../data-model/relationships.md).

## Filtering by a related record

Inside `predicate`, a key whose value is an object (that is not an operator
object) is a **related-record block**. The key is the **label** of the
related record, and the object is a predicate evaluated on that related
record. The block matches when **at least one** related record satisfies it.

Companies that have an `Engineering` department with at least 20 people:

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
          "labels": ["COMPANY"],
          "predicate": {
            "DEPARTMENT": { "name": "Engineering", "headcount": { "$gte": 20 } }
          }
        }
      }
    }
  }'
```

Everything from [Filtering](filtering.md) works inside a block: operators,
`$and`/`$or`/`$nor`/`$xor`/`$not`, `$id`, and further nested blocks.

| Block form | Meaning |
|---|---|
| `{ "DEPARTMENT": {} }` | Has at least one related `DEPARTMENT` record, through any relationship type, in either direction. |
| `{ "DEPARTMENT": { "name": "Sales" } }` | Has a related `DEPARTMENT` whose `name` is `Sales`. |
| `{ "DEPARTMENT": { "$or": [{ "name": "Sales" }, { "name": "Support" }] } }` | Logical combination evaluated on the same related record. |
| `{ "DEPARTMENT": { "$id": "0192…" } }` | Is related to that specific record. |
| `{ "$not": { "DEPARTMENT": {} } }` | Has no related `DEPARTMENT` record. |
| `{ "DEPARTMENT": {...}, "OFFICE": {...} }` | Both conditions hold (each on its own related record). |
| `{ "$or": [{ "DEPARTMENT": {...} }, { "OFFICE": {...} }] }` | Either condition holds. |

The label in a block is matched exactly against the related record's label.

> **Note:** If a label key maps to an **array** of block objects, the blocks
> are merged into one block whose conditions must all hold on the same
> related record. To express alternatives, use `$or` inside the block or at
> the parent level.

## Relationship type and direction: `$relation`

By default a block follows relationships of **any type** in **both**
directions. Add `$relation` to restrict them:

```json
{
  "labels": ["COMPANY"],
  "predicate": {
    "DEPARTMENT": {
      "$relation": { "type": "HAS_DEPARTMENT", "direction": "out" },
      "name": "Sales"
    }
  }
}
```

| `$relation` form | Meaning |
|---|---|
| `"HAS_DEPARTMENT"` | Only relationships of this type, either direction. |
| `{ "type": "HAS_DEPARTMENT" }` | Same. |
| `{ "direction": "out" }` | Outgoing: the current record is the relationship's source. `"outgoing"` is accepted too. |
| `{ "direction": "in" }` | Incoming: the current record is the relationship's target. `"incoming"` is accepted too. |
| `{ "direction": "both" }` (or `"any"`), or no direction | Both directions. Any other value is refused with `invalid_request`. |
| `{ "hops": … }` | Variable-length traversal, see below. |

The `type` in `$relation` matches relationship types exactly as they were
written: `works_at` and `WORKS_AT` are different types.

## Variable-length traversal: `hops`

`hops` inside `$relation` follows chains of relationships:

| `hops` value | Depth range |
|---|---|
| `3` | Exactly 3 hops. |
| `{ "min": 1, "max": 4 }` | 1 to 4 hops. |
| `{ "max": 4 }` | `min` defaults to 1. |
| `{ "min": 2 }` | `max` defaults to 10. |

`hops` must be at least 1 and `max` must not be smaller than `min`;
otherwise the request fails with `invalid_request`.

Employees whose management chain (up to four `REPORTS_TO` hops) reaches
someone named like "Alice":

```json
{
  "labels": ["EMP"],
  "predicate": {
    "EMP": {
      "$relation": { "type": "REPORTS_TO", "direction": "out", "hops": { "min": 1, "max": 4 } },
      "name": { "$contains": "Alice" }
    }
  }
}
```

Traversal semantics:

- The block's predicate is tested on records reached at a depth between
  `min` and `max`; the block matches as soon as one such record satisfies it.
- Every record along the path, not only the last one, must carry the block's
  label. A chain that passes through a record with another label is not
  followed.
- A path never visits the same record twice.

## Nested blocks: multi-level paths

A block inside a block is evaluated from the related record, so nesting
describes a path. Planets that hosted a battle involving a starship with
firepower of at least 60:

```json
{
  "labels": ["PLANET"],
  "predicate": {
    "BATTLE": {
      "STARSHIP": { "firepower_score": { "$gte": 60 } }
    }
  }
}
```

Each level can carry its own `$relation`, predicates, and `$alias`.

## Aliases

`$alias` names the related record a block binds, so `select` can aggregate
or collect over it. The alias is written with a leading `$`:

```json
{
  "labels": ["FACTION"],
  "predicate": {
    "BATTLE": {
      "$alias": "$battle",
      "$relation": { "type": "HAS_WINNER_FACTION_ID", "direction": "in" }
    }
  },
  "select": {
    "faction": "$record.name",
    "wins": { "$count": "$battle" }
  },
  "groupBy": ["$record.name"],
  "order": { "wins": "desc" }
}
```

An alias keeps the full path from the root, including the predicates of
every level, so metrics over `$battle` only see battles that satisfy the
block. See [Aggregations](aggregations.md#metrics-over-related-records).

## Cycles: `$cycle`

`$cycle` matches records that lie on a cycle of relationships, that is, a
path that starts and ends at the record itself. `hops` is required and its
minimum must be at least 2:

```json
{
  "labels": ["ACCOUNT"],
  "predicate": {
    "$cycle": { "type": "TRANSFERRED_TO", "direction": "out", "hops": { "min": 2, "max": 6 } }
  }
}
```

`type` and `direction` work as in `$relation`. Each relationship is used at
most once per path, so a self-loop (a 1-hop cycle) does not satisfy
`hops.min: 2`, and with `direction: "both"` a single relationship is not a
2-hop cycle. A path may pass through the record more than once (a figure
eight counts). `$cycle` may also appear inside a related-record block in
place of `$relation`.

Cost. The query builds the graph of the relationship type once, splits it
into the parts cycles can live in, and finds each record's shortest cycle
with a breadth-first search, so long cycles in branching graphs (fraud rings
of 10–20 transfers, say) are answered without enumerating paths. Only when a
record's shortest cycle is shorter than `hops.min` does the engine search
for a longer path, within a fixed amount of work; past it the query fails
with `resource_exhausted` ("`$cycle` search exceeds its work budget") rather
than running on. Narrow `hops` or add a filter.

## Listing relationships

The `relationships` query returns the relationships themselves, with their
properties:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": {
        "kind": "relationships",
        "query": {
          "sourceLabels": ["COMPANY"],
          "targetLabels": ["DEPARTMENT"],
          "relationType": "HAS_DEPARTMENT",
          "predicate": { "since": { "$gte": 2020 } },
          "offset": 0,
          "limit": 100
        }
      }
    }
  }'
```

| Field | Default | Meaning |
|---|---|---|
| `sourceLabels` | `[]` (any) | The source record's label must be one of these. |
| `targetLabels` | `[]` (any) | The target record's label must be one of these. |
| `relationType` | any | Exact relationship type (case-sensitive; no upper-casing here). |
| `predicate` | `{}` | Conditions on relationship properties: plain equality, or `$gt`, `$gte`, `$lt`, `$lte`. Other operators are not supported here. |
| `offset` | `0` | Relationships to skip. |
| `limit` | `10000` | Maximum relationships returned. |

Response:

```json
{
  "kind": "relationships",
  "relationships": [
    {
      "id": "0192f1d3-6a2b-7c11-8a4e-3f2d1c0b9a87",
      "type": "HAS_DEPARTMENT",
      "sourceId": "0192f1d2-6c1e-7a40-9d1b-0d6f2c1e4a77",
      "targetId": "0192f1d2-6c20-7c02-a1b3-77e1f0d2c3b4",
      "sourceLabel": "COMPANY",
      "targetLabel": "DEPARTMENT",
      "properties": { "since": 2021 }
    }
  ],
  "total": 1
}
```

`total` counts every matching relationship before `offset`/`limit`.

> **Note:** The relationship listing scans the tenant's relationships. For
> counts per relationship shape, use `describe.schema` instead, which reads
> the maintained relationship catalog (see [Facets](facets.md#relationship-shapes)).

## Property-overlap joins

`joinPairs` counts the pairs of records from two sets whose properties share
a value. It is useful for finding implicit links (for example, records that
reference each other by an external id) before materializing them as
relationships:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": {
        "kind": "joinPairs",
        "left": { "label": "SOURCE", "filter": {} },
        "right": { "label": "TARGET", "filter": {} },
        "predicate": { "kind": "propertyOverlap", "leftProperty": "targetId", "rightProperty": "id" },
        "excludeSameRecord": false,
        "projection": "count",
        "limit": 100
      }
    }
  }'
```

```json
{ "kind": "count", "value": 2 }
```

| Field | Meaning |
|---|---|
| `left`, `right` | `label` (required, non-empty) and an optional `filter` predicate selecting each record set. |
| `predicate.kind` | `propertyOverlap` is the only join predicate. |
| `leftProperty`, `rightProperty` | Property names, matched exactly as stored. A pair joins when any value of the left property equals any value of the right property (arrays are compared element by element, with type-strict equality). |
| `excludeSameRecord` | Skip pairs whose two sides are the same record. |
| `projection` | `count` is the only projection: the response is the number of distinct pairs. |
| `limit` | 1–10,000. Counting stops once this many pairs are found, so the result is `min(actual pairs, limit)`. |

To turn matching pairs into relationships, use `linkRecords` with
`source.property` and `target.property`
(see [Writing data](writing-data.md#linking-by-matching-properties)).

## Performance notes

Related-record blocks are evaluated per candidate record by walking its
relationships, so their cost grows with the number of candidates times the
neighborhood explored. Restrict the root with `labels` and selective
property conditions, bound `hops.max`, and set `$relation.type` and
`direction` whenever you know them.

## See also

- [Filtering](filtering.md)
- [Aggregations](aggregations.md)
- [Relationships](../data-model/relationships.md)
- [Writing data](writing-data.md)
- [Query execution](../architecture/query-execution.md)
