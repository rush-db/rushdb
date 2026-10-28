# Records and labels

This page explains what a record is, how its id and its single primary label
work, and how to create, read, update, retype and delete records through the
native protocol. It is for application developers designing their data; the
value rules for properties are on [Properties and types](properties-and-types.md).

All examples post an envelope to `/protocol/v1/execute`, as set up in the
[Quickstart](../getting-started/quickstart.md):

```bash
rdb() {
  curl -s http://127.0.0.1:3000/protocol/v1/execute \
    -H "Authorization: Bearer $RUSHDB_TOKEN" \
    -H 'Content-Type: application/json' \
    --data "$1"
}
```

## Anatomy of a record

A record has three parts: an **id**, one **label**, and **properties**. The
protocol always returns records in the same canonical shape:

```json
{
  "id": "019a0000-0000-7000-8000-000000000001",
  "label": "Person",
  "properties": {
    "name": "Ada Lovelace",
    "born": 1815,
    "birthdate": "1815-12-10",
    "languages": ["en", "fr"],
    "nickname": null
  },
  "propertyTypes": {
    "name": "string",
    "born": "number",
    "birthdate": "datetime",
    "languages": "string"
  }
}
```

| Field | Meaning |
|---|---|
| `id` | The record's stable identity, a UUID in dashed form. It never changes, including when the record is retyped. |
| `label` | The record's single primary label. |
| `properties` | The record's values. Datetimes are rendered as ISO-8601 strings. |
| `propertyTypes` | The display type of each typed property: `string`, `number`, `boolean` or `datetime`. A property holding only `null` has no entry. |

## Record ids

When you create a record without an `id`, the engine allocates a
time-ordered UUID (version 7). You may supply your own instead:

- Accepted forms are a dashed UUID (`019a0000-0000-7000-8000-000000000001`) or
  the same 32 hexadecimal digits without dashes.
- A malformed id is rejected with `invalid_request` (HTTP 400).
- Creating a record whose id already exists fails with `conflict` (HTTP 409);
  `create` never overwrites.

Supplying ids is useful when another system already owns the identity, or
when you want to link records in the same script without reading ids back.

## Labels

A label classifies a record: `Person`, `Order`, `Invoice`. The rules are
simple:

- **Exactly one per record.** A record has one primary label, given when it
  is created. There are no secondary labels.
- **No declaration.** The first record written with a label creates it; the
  [schema catalog](schema-catalog.md) then reports it with its record count.
- **Exact names.** Labels are matched as written in the native protocol, so
  `Person` and `PERSON` are different labels. Pick one convention and keep it.
  (The SQL frontend maps table names onto labels; see [SQL](../languages/sql.md).)
- **Changeable.** A record can be moved to another label with an atomic
  retype that keeps its id, properties, vectors and relationships.

Labels are what queries, indexes and SQL tables are organised around: a query
names the labels it searches, an index covers one label's property, and every
label appears as a table in SQL. Choose labels that describe *kinds of
things*, and model varying attributes as properties rather than as extra
labels.

## Create a record

`putRecord` with `mode: "create"` requires a `label` and a `properties`
object:

```bash
rdb '{
  "protocolVersion": 1,
  "operation": {
    "kind": "mutate",
    "mutation": {
      "kind": "putRecord",
      "mode": "create",
      "label": "Book",
      "properties": { "title": "Dune", "pages": 412, "published": "1965-08-01" }
    }
  }
}'
```

The response has `kind: "record"` and the stored record. `create` also accepts
an `options` object with the type-inference switches described in
[Properties and types](properties-and-types.md#type-inference), and a
`vectors` list for attaching embeddings (see
[Vector search](../indexes-and-search/vector-search.md)).

`putRecord` writes one **flat** record: property values must be scalars or
arrays of scalars. To store a nested JSON document as several linked records,
use [nested import](nested-import.md).

## Read a record

Fetch one record by id with the `record` query:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"query","query":{
  "kind":"record","id":"019a0000-0000-7000-8000-000000000001"}}}'
```

An id that does not exist returns `not_found` (HTTP 404). To find records by
their values, use the `records` query described in
[Search queries](../query/search-queries.md).

## Update a record

`putRecord` has two update modes. Both require `id`.

| Mode | Effect on properties you send | Effect on properties you omit | Label |
|---|---|---|---|
| `merge` | Set to the new value | Kept unchanged | Unchanged. If the id does not exist and you pass `label`, the record is created with that id; without `label` the request fails. |
| `replace` | Set to the new value | Removed | Unchanged (the `label` field is ignored) |

Merge one property into an existing record:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"putRecord","mode":"merge","id":"019a0000-0000-7000-8000-000000000001",
  "properties":{"nickname":"The Enchantress of Numbers"}}}}'
```

Replace the whole property set:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"putRecord","mode":"replace","id":"019a0000-0000-7000-8000-000000000001",
  "properties":{"name":"Augusta Ada King","born":1815}}}}'
```

Sending `"nickname": null` in a merge stores an explicit null; it does not
remove the property. To remove a property from one record, use `replace`
without it. To remove a property from **every** record in the tenant, use
`deleteProperty` (see [Schema catalog](schema-catalog.md#remove-a-property-everywhere)).

## Change a record's label

`retypeRecord` moves a record from one label to another. `fromLabel` is an
optimistic check: if the record's current label is not `fromLabel` (for
example because another client retyped it first), the request fails with
`conflict` and nothing changes.

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"retypeRecord","id":"019a0000-0000-7000-8000-000000000001",
  "fromLabel":"Person","toLabel":"Mathematician"}}}'
```

The response is the record under its new label. Its id, properties, vectors
and relationships are unchanged. Both labels must be non-empty and different.

## Delete records

Delete one record by id:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"deleteRecord","id":"019a0000-0000-7000-8000-000000000001"}}}'
```

Deleting a record also deletes **every relationship attached to it**, in the
same commit. The response is `{"kind": "mutation", "affected": 1}`.

Delete every record that matches a query with `deleteRecords`:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"deleteRecords",
  "query":{"labels":["Book"],"predicate":{"pages":{"$lt":100}}}}}}'
```

`deleteRecords` honours the query's `offset` and `limit`: `offset` matches
are left in place and `limit` caps how many records are deleted (without a
`limit`, every match is deleted). The match set is captured once, then
deleted in pages of 500 records, each page its own atomic commit. Records
inserted after the match set was captured are not deleted, and a concurrent
reader can observe a partially completed bulk delete.

## Count records per label

The cheapest way to count one label is a `$count` projection with no
predicate, which is answered from maintained counters:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
  "labels":["Person"],"select":{"n":{"$count":"*"}}}}}}'
```

The response kind is `rows` (`[{"n": 3}]`). `describe.schema` lists every
label with its record count; see [Schema catalog](schema-catalog.md).

## See also

- [Properties and types](properties-and-types.md)
- [Relationships](relationships.md)
- [Writing data](../query/writing-data.md)
- [Transactions](../query/transactions.md)
- [Error codes](../reference/error-codes.md)
