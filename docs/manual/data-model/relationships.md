# Relationships

This page explains relationships in RushDB — their type, direction and
properties — and every way to create, inspect, rename and remove them through
the native protocol. It is for developers modelling connected data. Querying
*across* relationships is covered in
[Relationships and traversal](../query/relationships-and-traversal.md). The
`rdb` helper used in the examples is defined in the
[Quickstart](../getting-started/quickstart.md#1-install-and-start-a-node).

## What a relationship is

A relationship connects a **source** record to a **target** record. It has:

| Part | Meaning |
|---|---|
| Type | A string naming the connection, such as `WORKS_AT` or `HAS_DEPARTMENT`. |
| Direction | Always stored from source to target. Queries can follow it either way. |
| Properties | Optional values on the relationship itself, such as `since` or `role`. |
| Id | A stable identifier assigned by the engine. |

Relationships are first-class: they appear in the
[schema catalog](schema-catalog.md) as shapes (`sourceLabel`, `type`,
`targetLabel`, count and properties), they can be filtered and listed, and
they can carry vectors of their own (see
[Vector search](../indexes-and-search/vector-search.md)).

Deleting a record deletes every relationship attached to it.

## Naming relationship types

Any non-empty string is a valid type, but use **upper-case snake case**
(`WORKS_AT`, `HAS_DEPARTMENT`). The native query document upper-cases the
type you name in a `$relation` filter before matching, so a relationship
stored as `works_at` cannot be selected by type from a native query.

Relationships created by [nested import](nested-import.md) without an
explicit type use the default type `__RUSHDB__RELATION__DEFAULT__`.

## Relationship properties

Relationships carry properties with the same value types as records
(boolean, number, string, arrays, explicit `null`). Two differences:

- No type inference is applied: a date string on a relationship stays a
  string.
- Properties are set when the relationship is created; a link request that
  finds the relationship already present leaves it untouched.

## Link records by id

`linkRecordsById` creates one relationship from **every** source id to
**every** target id (a cross product):

```bash
rdb '{
  "protocolVersion": 1,
  "operation": {
    "kind": "mutate",
    "mutation": {
      "kind": "linkRecordsById",
      "command": {
        "relationType": "INFLUENCED",
        "sourceIds": ["019a0000-0000-7000-8000-000000000001"],
        "targetIds": ["019a0000-0000-7000-8000-000000000002",
                      "019a0000-0000-7000-8000-000000000003"],
        "properties": { "via": "the Analytical Engine notes" }
      }
    }
  }
}'
```

```json
{"protocolVersion":1,"response":{"kind":"linked","created":2,"skippedExisting":0}}
```

| Field | Required | Meaning |
|---|---|---|
| `relationType` | Yes | The relationship type. |
| `sourceIds`, `targetIds` | Yes | Record ids (dashed UUIDs or 32 hex digits). |
| `direction` | No | `out` (default): source → target. `in`: each pair is stored target → source. |
| `properties` | No | Properties for every relationship created. |

Behaviour:

- Every endpoint is resolved first; an unknown id fails the whole call with
  `not_found` and nothing is written.
- An edge of the same type that already exists between the same two records
  is skipped, so the call is idempotent. `created` counts new edges and
  `skippedExisting` the pairs that were already linked.
- All new edges are written in one atomic commit.

## Link explicit pairs

`linkRecordsPairs` takes an explicit list of pairs — one edge per pair, no
cross product — and lets each pair override the type:

```bash
rdb '{
  "protocolVersion": 1,
  "operation": {
    "kind": "mutate",
    "mutation": {
      "kind": "linkRecordsPairs",
      "command": {
        "relationType": "KNOWS",
        "pairs": [
          { "sourceId": "019a0000-0000-7000-8000-000000000002",
            "targetId": "019a0000-0000-7000-8000-000000000003" },
          { "sourceId": "019a0000-0000-7000-8000-000000000003",
            "targetId": "019a0000-0000-7000-8000-000000000001",
            "relationType": "ADMIRES" }
        ]
      }
    }
  }
}'
```

The same guarantees apply: all endpoints are resolved before anything is
written, existing edges are skipped, and the batch commits atomically. This
is the most efficient way to load many edges whose endpoints you already
know; batches of about 500 pairs per request work well.

## Link records by matching values

`linkRecords` joins two sets of records by label, filter and key, much like a
foreign-key join. This links every `Employee` to the `Company` whose `id`
property equals the employee's `companyId`:

```bash
rdb '{
  "protocolVersion": 1,
  "operation": {
    "kind": "mutate",
    "mutation": {
      "kind": "linkRecords",
      "command": {
        "relationType": "WORKS_AT",
        "source": { "label": "Employee", "property": "companyId" },
        "target": { "label": "Company", "property": "id" }
      }
    }
  }
}'
```

| Field | Meaning |
|---|---|
| `source`, `target` | Each selects records: `label` (required), `property` (the join key) and an optional `predicate` filter. |
| `allowCartesian` | When `true`, `property` may be omitted on both sides and every selected source is linked to every selected target. Both sides must then have a non-empty `predicate`. When source and target have the same label, a record is not linked to itself. |
| `direction` | `out` (default) or `in`, as above. |
| `properties` | Properties for every relationship created. |

Join keys compare values with their types (`17` does not match `"17"`), and
a multi-value key matches when any of its values matches. Existing edges of
the same type are skipped. Like the id-based operations, `linkRecords`
writes every new edge in one atomic commit: all of them, or none.

## Other ways to create relationships

- **Nested import** creates parent → child relationships for every nested
  object; see [Nested import](nested-import.md).
- **Transactions** stage `createRelationship` together with other writes and
  commit them atomically; see [Transactions](../query/transactions.md).
- **Statement languages** create relationships with their own syntax, for
  example `CREATE (a)-[:KNOWS]->(b)` in Cypher; see
  [Query languages](../languages/README.md).

## Remove relationships

`unlinkRecordsById` deletes the edges between every source and every target:

- `relationTypes` lists the types to delete; omitted or empty, it deletes
  edges of every type;
- `direction` is `out` (`source → target`) or `in` (`target → source`);
  omitted, it deletes edges pointing either way.

Deleting an edge that does not exist is not an error, so the call is
idempotent:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"unlinkRecordsById",
  "command":{"relationTypes":["INFLUENCED"],"direction":"out",
    "sourceIds":["019a0000-0000-7000-8000-000000000001"],
    "targetIds":["019a0000-0000-7000-8000-000000000002"]}}}}'
```

`unlinkRecords` takes the same selector command as `linkRecords`. The
response of both is `{"kind": "mutation", "affected": <deleted edges>}`.

## Rename a relationship type

`retypeRelations` rewrites every relationship of one type between two labels
to a new type, keeping endpoints and direction:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"retypeRelations",
  "command":{"sourceLabel":"Company","targetLabel":"department",
    "sourceType":"__RUSHDB__RELATION__DEFAULT__","targetType":"HAS_DEPARTMENT"}}}}'
```

`sourceType` defaults to the nested-import default type, which makes this the
natural follow-up to an import that did not name its relationship types.

## List relationships

The `relationships` query returns relationship instances, optionally filtered
by endpoint labels, endpoint records, type and relationship properties:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"relationships","query":{
  "sourceLabels": ["Person"],
  "relationType": "INFLUENCED",
  "predicate": { "via": "the Analytical Engine notes" },
  "limit": 50
}}}}'
```

```json
{
  "protocolVersion": 1,
  "response": {
    "kind": "relationships",
    "total": 2,
    "relationships": [
      {
        "id": "…",
        "type": "INFLUENCED",
        "sourceId": "019a0000-0000-7000-8000-000000000001",
        "targetId": "019a0000-0000-7000-8000-000000000002",
        "sourceLabel": "Person",
        "targetLabel": "Person",
        "properties": { "via": "the Analytical Engine notes" }
      }
    ]
  }
}
```

Every id in a row (`id`, `sourceId`, `targetId`) is a dashed UUID.

The relationship `predicate` filters the relationship's own properties with
every field operator and logical combinator of the record predicate (see
[Filtering](../query/filtering.md)), for example
`{"since": {"$gte": 2020}, "role": {"$in": ["engineer", "lead"]}}`. It cannot
contain traversal blocks.

`sourcePredicate` and `targetPredicate` filter the records at either end,
with the record predicate syntax (`$id` included). They are answered from
those records' own links, so listing one record's relationships costs as
much as that record's relationship count, not the store's:

```json
{ "sourcePredicate": { "$id": "019a0000-0000-7000-8000-000000000001" }, "limit": 100 }
```

`total` counts every match before `offset`/`limit`. When you only
need counts per shape, use `describe.schema` instead — it is answered from the
catalog without visiting individual relationships; see
[Schema catalog](schema-catalog.md#relationship-shapes).

## See also

- [Relationships and traversal](../query/relationships-and-traversal.md)
- [Records and labels](records-and-labels.md)
- [Nested import](nested-import.md)
- [Transactions](../query/transactions.md)
- [Cypher](../languages/cypher.md)
