# Schema catalog

This page describes the schema catalog: the exact, continuously maintained
record of which labels, properties, types and relationship shapes exist in a
tenant, and the `describe` and `propertyValues` operations that read it. It
is for developers building schema-aware tools (form builders, query builders,
AI agents that need to know what data exists) and for anyone exploring an
unfamiliar dataset. The `rdb` helper used in the examples is defined in the
[Quickstart](../getting-started/quickstart.md#1-install-and-start-a-node).

## An emergent schema

RushDB has no `CREATE TABLE` and no schema declaration. Instead, every commit
updates a catalog as a side effect of the write:

| The catalog records | For example |
|---|---|
| Every label and how many records carry it | `Person: 3`, `Company: 1` |
| Every property, per type, with how many records carry it | `born` (number) on 3 records; a property's explicit nulls are its `null` entry |
| Whether a property holds arrays | `languages` is an array property |
| Properties whose name exists with several types | `code` as both string and number |
| Value dictionaries per label and property | distinct values of `status`, with counts |
| Every relationship shape with its count and properties | `Person -INFLUENCED-> Person: 2` |

Because the catalog is maintained on every commit, reading it does not scan
your records. Counts are exact, never estimates.

## List labels

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"describe","describe":{
  "kind":"labels","query":{}}}}'
```

```json
{"protocolVersion":1,"response":{"kind":"labels","labels":["Person","Company","address","department"]}}
```

## Describe the whole schema

`describe.schema` returns labels with record counts, properties and
relationship shapes in one response:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"describe","describe":{
  "kind":"schema","query":{},"detail":"catalog"}}}'
```

```json
{
  "protocolVersion": 1,
  "response": {
    "kind": "schema",
    "schema": {
      "labels": [
        { "name": "Person", "recordsCount": 3 },
        { "name": "Company", "recordsCount": 1 }
      ],
      "properties": [
        { "id": "p…", "name": "born", "type": "number", "recordsCount": 3, "isArray": false }
      ],
      "relationships": [
        { "sourceLabel": "Person", "targetLabel": "Person", "type": "INFLUENCED", "count": 2,
          "properties": [ { "name": "via", "type": "string", "relationshipsCount": 2 } ] }
      ]
    }
  }
}
```

The request takes:

| Field | Meaning |
|---|---|
| `query` | A query document (`labels`, `predicate`) that scopes the description. `{}` describes the whole tenant. |
| `detail` | `"full"` (default) adds sample values and min/max per property; `"catalog"` returns only catalog facts and never reads records. |
| `skip`, `limit` | Paginate the `properties` list. |

## Properties

`describe.properties` returns the property list on its own:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"describe","describe":{
  "kind":"properties","query":{"labels":["Person"]}}}}'
```

Each entry has:

| Field | Meaning |
|---|---|
| `id` | A stable property id (`p` followed by 16 hex digits), usable in place of the name in `propertyValues`. Ids are local to the tenant. |
| `name` | The normalised property name (see [Property names](properties-and-types.md#property-names)). |
| `type` | Display type: `string`, `number`, `boolean` or `datetime`. |
| `recordsCount` | Exact number of records carrying the property with a value of this entry's type (for the `null` entry: holding `null`). The entries of one name add up to the records that carry it. |
| `isArray` | `true` when the property holds arrays. |
| `siblingTypes` | Present when the property also holds other display types on the same label; lists those types by logical name (`int64`, `float64`, `utf8`, `boolean`, `datetime`). Integers and floats are both `number`, so they are never each other's sibling. |
| `label` | The label the entry was observed on, when the response is broken down per label. |
| `sampleValues`, `min`, `max` | With `detail: "full"`: a bounded set of distinct values, and the exact range over every record. Unfiltered requests sample up to 10,000 records for `sampleValues` only; the range comes from the value dictionary and is left out where it does not hold every value (array properties, strings over 96 bytes). |

A property name stored with several types appears once per display type
(per label when broken down by label); integers and floats are one `number`
entry with their combined count. A client
that caches property types should treat a non-empty `siblingTypes` as a sign
that one cached type does not cover every record.

> **Note:** `detail: "catalog"` cannot be combined with a non-empty
> `predicate`; drop the predicate or use the default `full` detail.

## Relationship shapes

The `relationships` part of `describe.schema` aggregates edges by
`(sourceLabel, type, targetLabel)`:

| Field | Meaning |
|---|---|
| `sourceLabel`, `targetLabel` | Labels of the endpoints. |
| `type` | Relationship type. |
| `count` | Number of relationships with this shape. |
| `properties` | Each property carried by relationships of this shape, with its display type and `relationshipsCount` (relationships of this shape that carry it). |

This is the cheapest way to answer "how are my labels connected?". Use the
`relationships` query only when you need the individual edges; see
[Relationships](relationships.md#list-relationships).

## Narrowing with a predicate

`describe.labels`, `describe.properties`, `describe.schema` and
`propertyValues` all accept the same query document as a records query. A
`labels` list or `predicate` narrows every count to the matching records:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"describe","describe":{
  "kind":"schema","query":{"labels":["Person"],"predicate":{"born":{"$gte":1900}}}}}}'
```

Without a predicate the answer comes straight from the catalog, independent
of data size. With a predicate the engine first finds the matching records,
then counts labels, properties and relationship shapes over that set — a cost
that grows with the number of matches and is bounded by the request's memory
budget (a request that would exceed it fails with `resource_exhausted` rather
than returning a partial count). A relationship shape is counted when at
least one endpoint is in the match set. This is how faceted search UIs are
built; see [Facets](../query/facets.md).

## Property values

`propertyValues` reads the value domain of one property:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"query","query":{
  "kind":"propertyValues","property":"languages","mode":"topK","limit":5,
  "query":{"labels":["Person"]}}}}'
```

| `mode` | Returns |
|---|---|
| `values` (default) | Distinct values; `limit` caps how many are shipped, `truncated` says whether it did. |
| `count` | `distinctCount` only. |
| `range` | `min` and `max` of one ordered class: numbers if any, else datetimes by instant, else text lexically. |
| `topK` | The `limit` most frequent values, with record counts in `counts`. |

`distinctCount` is always exact and untruncated. With an empty predicate the
answer comes from the per-label value dictionaries, without reading records:
`values` and `range` included (a property with explicit nulls, arrays or
values too long for the dictionary is read from its records instead). Under
one label and one stored type the cost does not grow with the number of
distinct values either: the count is a counter, a numeric range is the
dictionary's first and last entry, and `topK` or a `values` list with a
`limit` holds only its head, so a column of unique ids fits
`query-memory-max`.

## A compact summary

`describe.schemaSummary` returns every label with its properties in one
read, in the shape a schema-aware agent needs: per property its type, record
count, exact distinct count, the range of numbers and dates, and every value
of a property that holds only a few; per label its relationship shapes. A
property stored in several types has one entry per type (and a `null` entry
for its nulls), each with its own count, distinct count, range and values.
It takes `labels` and `fewValues` only; for a filtered view use
`describe.properties` and `propertyValues` with a predicate.

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"describe","describe":{
  "kind":"schemaSummary","labels":["Examination"],"fewValues":10}}}'
```

```json
{"kind":"schemaSummary","labels":[{"name":"Examination","recordsCount":106,
  "properties":[
    {"name":"Examination Date","type":"datetime","recordsCount":102,"distinctCount":94,
     "min":"1992-07-20","max":"1998-04-02"},
    {"name":"KCT","type":"string","recordsCount":21,"distinctCount":2,"values":["-","+"]}]}]}
```

(Two of the label's properties shown, from the BIRD `thrombosis_prediction` database.
A label with relationships also lists `relationships`, as `{type, label, direction,
count}`.)

| Field | Default | Meaning |
|---|---|---|
| `labels` | all | Labels to summarize. |
| `fewValues` | `10` (at most 100; `0` lists none) | List a property's values when it has at most this many distinct ones, most frequent first. |

`distinctCount` is absent only for a property holding arrays (its values are
whole arrays). Text values longer than the value dictionary holds (96 bytes) are
counted by write-time refcounts of their digests, so the count is exact and
costs no scan. Listing such values (when there are at most `fewValues`) reads
the label's records until each is found, at most 65,536 records; past that the
entry keeps its `distinctCount` and lists no `values`. A type-split entry names the other stored
types in `siblingTypes`, as `describe.properties` does.

Introspection never shows a long text whole: every sample, listed value and
bound in `describe` schema and properties and in `schemaSummary` is cut to
`resources.introspection-value-max-chars` characters (default 64; `0` = no cap)
and ends with `…`, so a column of book chapters cannot flood a schema read.
Two long values that share their first 64 characters then read alike; the
count is still of whole values. Value queries (`propertyValues`,
`searchValues`) and records return values whole. Each property's dictionary is query
working set while it is summarized; past `query-memory-max` (or the request's
`context.queryMemoryMaxBytes`) the read fails with `resourceExhausted`.

## Find a name

`describe.names` answers "no such table or column": the labels and
properties spelled like a name, ignoring case, separators (`_`, `-`, spaces,
camelCase) and plural endings, then ranked by spelling.

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"describe","describe":{
  "kind":"names","text":"patients.birthdate","limit":3}}}'
```

```json
{"kind":"names","names":[{"label":"Patient","property":"Birthday","score":0.78},
  {"label":"Patient","property":"First Date","score":0.67}]}
```

A `Label.property` text scores both parts; a plain text matches labels and
properties alike. Scores run from 0 to 1 (1: equal once case, separators and
plurals are ignored); suggestions below 0.5 are dropped. SQL errors for an
unknown table carry the same suggestions.

## Value overlap

`describe.valueOverlap` compares property pairs over the exact value
dictionaries: for each pair, how many of the source's distinct values the
target also holds. It is the evidence a join suggestion needs (does
`disp.client_id` name `client` records?), counted over every value rather
than a sample. It returns evidence only; deciding which pairs are references
is the caller's (RushDB Cloud's relationship suggestions use it).

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"describe","describe":{
  "kind":"valueOverlap","pairs":[
    {"source":{"label":"disp","property":"client_id"},
     "target":{"label":"client","property":"client_id"}}]}}}'
```

```json
{"kind":"valueOverlap","pairs":[{
  "source":{"label":"disp","property":"client_id"},
  "target":{"label":"client","property":"client_id"},
  "sourceDistinct":5369,"targetDistinct":5369,"matchedDistinct":5369,
  "matchedRecords":5369,"coverage":1.0,"sourceUniqueness":1.0,
  "targetUniqueness":1.0,"numericSpan":1.0}]}
```

(From the BIRD `financial` database.)

| Field | Meaning |
|---|---|
| `matchedDistinct` / `matchedRecords` | Distinct source values the target holds, and the source records holding them. |
| `coverage` | `matchedDistinct / sourceDistinct`. |
| `sourceUniqueness` / `targetUniqueness` | Distinct values per record carrying the property; 1 is a key. |
| `numericSpan` | When (nearly) all values of both are numbers, digits stored as text included: the source's value span as a share of the target's. Small codes inside a large id range score near 0, a coincidence rather than a reference. |
| `notCompared` | Why a pair was not compared: no such property (or one the policy hides), array values, values too long for the dictionary, more than 200,000 distinct values, or overlap withheld under a policy's `minCount`. |

Pairs come back in request order, at most 10,000 per call. Values compare as
text, so a numeric id matches the same id stored as a string. The
dictionaries compared are query working set for the whole call: past
`query-memory-max` (or the request's `context.queryMemoryMaxBytes`) it fails
with `resourceExhausted`, so compare fewer pairs per call.

## Remove a property everywhere

`deleteProperty` removes a property from the whole tenant: every value of
it, in every type, and its catalog entries. Use it to clean up a misspelled or
retired field.

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"deleteProperty","name":"nickname"}}}'
```

`name` is matched by its normalised form. The response reports, as
`affected`, how many owners (records or relationships) carried the property.
Naming a property that does not exist is an error.

A property name used both on records and on relationships is two separate
properties; one call removes one of them. Repeat the call until it reports
that the property does not exist to remove both.

> **Warning:** `deleteProperty` is tenant-wide and cannot be undone. To remove
> a property from a single record, `replace` the record without it.

## See also

- [Properties and types](properties-and-types.md)
- [Facets](../query/facets.md)
- [Aggregations](../query/aggregations.md)
- [Derived state](../architecture/derived-state.md) — how the catalog is
  maintained
