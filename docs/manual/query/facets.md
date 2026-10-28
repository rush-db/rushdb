# Facets

Facets answer "what is in this result set?": which labels the matching
records carry, which properties they have and with what types, which values
a property takes and how often, and how the records are connected. In RushDB
every facet takes the **same query document** as a records search, so the
facet counts always describe exactly the records the search would return.
This page shows each facet request and how narrowing works.

## The facet requests

| Request | Returns |
|---|---|
| `describe.labels` | Label names present in the match set. |
| `describe.properties` | Property entries: name, type, number of records carrying it, array shape, samples, min/max. |
| `describe.schema` | Labels with record counts, property entries, and relationship shapes, from one match set. |
| `query.propertyValues` | Distinct values of one property: the list, an exact distinct count, the min/max range, or the most frequent values with counts. |

Each one takes a `query` with `labels` and `predicate`
([Filtering](filtering.md) syntax). `select`, `groupBy`, `order` and
pagination fields in that document do not affect facets.

## Narrowing by the query document

Without a `predicate`, facets are answered from maintained catalogs (label
counters, property support counts, per-label value dictionaries), so their
cost does not depend on the number of records. With a `predicate`, RushDB
resolves the matching records once and counts labels, properties, values
and relationships over that match set, without building record JSON.

A typical faceted-search page sends the same document to a listing and to
the facets:

```bash
QUERY='{ "labels": ["Person"], "predicate": { "city": "Riga" } }'

# The page of results
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H "Content-Type: application/json" \
  -d "{\"protocolVersion\":1,\"operation\":{\"kind\":\"query\",\"query\":{\"kind\":\"records\",\"query\":$QUERY}}}"

# Distinct ages of exactly those records, most frequent first
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H "Content-Type: application/json" \
  -d "{\"protocolVersion\":1,\"operation\":{\"kind\":\"query\",\"query\":{\"kind\":\"propertyValues\",\"property\":\"age\",\"mode\":\"topK\",\"limit\":5,\"query\":$QUERY}}}"
```

Counts cover the whole match set (not a page of it), so their cost is linear
in the number of matching records. The match set is bounded by the per-query
working-set budget: a match set that does not fit fails with
`resource_exhausted`; a facet never returns a partial count. A predicate on a
property with a declared ordered index is resolved from the index (see
[Indexes](../indexes-and-search/indexes.md)).

## Labels

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "describe",
    "describe": { "kind": "labels", "query": { "predicate": { "city": "Riga" } } }
  }
}
```

```json
{ "kind": "labels", "labels": ["Company", "Person"] }
```

| Query | Labels returned |
|---|---|
| No `labels`, no `predicate` | Every label in the tenant. |
| `labels`, no `predicate` | The requested labels that currently have records. |
| With `predicate` | Labels of the matching records. |

For per-label record counts, use `describe.schema`.

## Properties

```json
{
  "kind": "describe",
  "describe": {
    "kind": "properties",
    "query": { "labels": ["Person"] },
    "detail": "catalog",
    "skip": 0,
    "limit": 50
  }
}
```

| Field | Default | Meaning |
|---|---|---|
| `query` | — | The query document. |
| `detail` | `full` | `full` adds `sampleValues` and `min`/`max`. An unfiltered request samples up to 10,000 records for `sampleValues`; its `min`/`max` and counts are exact. `catalog` returns names, types, exact counts, array shape and type-split markers from catalogs only, without reading records, and rejects a non-empty `predicate`. |
| `skip`, `limit` | all | Slice of the returned property entries. |

Each entry:

| Field | Meaning |
|---|---|
| `id` | Stable property id. |
| `name` | Property name. |
| `type` | `string`, `number`, `boolean`, or `datetime`. |
| `recordsCount` | Number of records that carry the property with a value of this entry's type; for the `null` entry, the records holding an explicit `null`. |
| `label` | The label the entry was counted on, when reported per label. |
| `isArray` | Some occurrence is stored as an array. |
| `siblingTypes` | Other stored types the property holds on this label (for example, written both as a string and as a number); empty when there is one type. |
| `sampleValues` | A bounded set of distinct observed values (`full` only). |
| `min`, `max` | Smallest and largest values over every matching record (`full` only). Unfiltered, they come from the value dictionary and are left out where it does not hold every value (array properties, strings over 96 bytes). |

## Schema

`describe.schema` returns all three facets from one match set:

```json
{
  "kind": "describe",
  "describe": { "kind": "schema", "query": { "labels": ["Person"], "predicate": { "age": { "$gte": 30 } } } }
}
```

```json
{
  "kind": "schema",
  "schema": {
    "labels": [{ "name": "Person", "recordsCount": 3 }],
    "properties": [{ "id": "…", "name": "age", "type": "number", "recordsCount": 3, "isArray": false }],
    "relationships": [
      { "sourceLabel": "Person", "targetLabel": "Company", "type": "WORKS_AT", "count": 3,
        "properties": [{ "name": "since", "type": "number", "relationshipsCount": 3 }] }
    ]
  }
}
```

`describe.schema` also accepts `detail`, `skip` and `limit` for its property
entries.

### Relationship shapes

Relationship shapes are grouped by `(sourceLabel, type, targetLabel)` with
an exact `count` and the properties observed on those relationships. Property
counts are per shape too: two shapes of the same type each list only the
properties their own relationships carry, so the unfiltered catalog and a
filtered read agree.

| Query | Shapes returned |
|---|---|
| No `labels`, no `predicate` | Every shape, from the relationship catalog. |
| `labels`, no `predicate` | Catalog shapes whose source or target label is one of the requested labels. |
| With `predicate` | Every relationship with at least one endpoint in the match set, each counted once. |

> **Tip:** When you need only relationship counts, use `describe.schema`
> rather than listing relationships with `query.relationships`: the schema
> facet reads the catalog instead of walking relationships.

## Property values

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": {
        "kind": "propertyValues",
        "property": "city",
        "mode": "topK",
        "limit": 3,
        "query": { "labels": ["Person"] }
      }
    }
  }'
```

```json
{
  "kind": "propertyValues",
  "property": "city",
  "values": ["Riga", "Oslo"],
  "distinctCount": 2,
  "counts": [2, 1]
}
```

| `mode` | Response fields | Notes |
|---|---|---|
| `values` (default) | `values`, `distinctCount`, `truncated` | Distinct values, by descending record count, ties by value (as a search lists them). `limit` keeps the first `limit`; `truncated` is `true` when it did. |
| `count` | `distinctCount` | Exact number of distinct values. |
| `range` | `min`, `max`, `distinctCount` | Exact extremes of one ordered class: the numbers if the property has any, else its datetimes (by instant), else its text (lexically). Classes never mix. Absent when there are no comparable values. |
| `topK` | `values`, `counts`, `distinctCount` | The `limit` most frequent values (default 10, at most 1,000) with the number of records carrying each, most frequent first. Values with equal counts come in no defined order. |

`distinctCount` is always exact and reflects every distinct value, even when
`values` is truncated. An array value counts as one value (the whole array).

### Searching values

Add `search` to keep only the values whose text matches, for example to map a
word in a question to the values actually stored:

```json
{
  "kind": "propertyValues",
  "property": "county",
  "mode": "topK",
  "limit": 20,
  "search": { "text": "orange", "match": "contains" },
  "query": { "labels": ["ZipCode"] }
}
```

| `search` field | Default | Meaning |
|---|---|---|
| `text` | (required) | The text to look for. |
| `match` | `contains` | `contains`, `prefix`, `exact` or `similar` (below). |
| `caseSensitive` | `false` | Compare case-sensitively. |
| `domainOnMiss` | (none) | When nothing matches and the property has at most this many distinct values in scope (capped at 1000), answer with all of them instead. |
| `minSimilarity` | `0.6` | `similar` only: the least similarity, from 0 to 1. |

Strings match as stored; numbers, booleans and datetimes match as the response
renders them (`31`, `true`, `2020-01-02T03:04:05.000Z`); an array matches when any
element does. Every mode answers over the matching values: `distinctCount`
counts matches, and `values` and `topK` list them by descending record count,
ties by value. With no predicate in `query`, the search reads the per-label
value dictionaries, not the records.

**Dates.** A `text` that reads as a year-first date with at least a month
(`1993-11-12`, `1993/11/12`, `2010/2/22`, `2013-06`) also matches values that
hold that date in another layout: a datetime, `1993.11.12`, `1993/11/12 08:30`,
or a compact `19931112` / `201306` (as a string or a number). It matches at the
text's precision, so `2013-06` matches every day in June 2013; with `exact`, the
value must have the same precision. Day-first and month-first layouts
(`12/11/1993`) are ambiguous and are matched as text only.

**Near matches.** `match: "similar"` finds values spelled like the text
rather than containing it. A value scores 1 when some run of its whole words
spells the text once case, punctuation and spacing are ignored ("Spider-Man"
for `spiderman`, "Orange County, Virginia" for `orange county virginia`), and
otherwise the share of the text's character trigrams it holds, so a typo
still scores high and an unrelated word low. Values at or above
`minSimilarity` are kept, ranked by similarity, then by record count. They are
near matches, not exact ones: check before using one as a filter.

**When nothing matches.** A word in a question often names a value the data
stores as a code ("positive" for `+`, "male" for `M`). With `domainOnMiss: N`, a
search with no match on a property with at most `N` distinct values answers
with all of them, by descending record count with `counts`, and sets
`"searchMissed": true`. The values listed are then *not* matches: they show how
the property is actually written. A larger domain, or a search that matches,
answers as without the field.

```json
{
  "kind": "propertyValues",
  "property": "KCT",
  "mode": "topK",
  "search": { "text": "positive", "domainOnMiss": 20 },
  "query": { "labels": ["Examination"] }
}
```

```json
{ "property": "kct", "values": ["-", "+"], "counts": [15, 6],
  "distinctCount": 2, "searchMissed": true }
```

### Searching every property

`query.searchValues` runs the same search over every property at once, for
when the question names a value but not where it is stored:

```json
{
  "kind": "searchValues",
  "search": { "text": "Orange County", "match": "contains" },
  "labels": ["schools", "frpm"],
  "limit": 2
}
```

```json
{ "kind": "valueMatches", "total": 23,
  "matches": [
    { "label": "schools", "property": "District", "value": "Orange County Department of Education", "count": 55 },
    { "label": "frpm", "property": "District Name", "value": "Orange County Department of Education", "count": 5 } ] }
```

(From the BIRD `california_schools` database; `total` counts every match before `limit`.)

| Field | Default | Meaning |
|---|---|---|
| `search` | (required) | As above; `domainOnMiss` is ignored. |
| `labels` | all | Labels to search. |
| `limit` | `50` (at most 1000) | Most matches returned; `total` counts them all. |

Each (label, property) is searched like `propertyValues` with a `search`:
from the dictionaries where they answer, scanning long values otherwise (one
scan of a label's records covers all of its long-valued properties). Matches
rank by record count (by similarity first for `similar`). The values read are
query working set, capped by `query-memory-max` or the request's
`context.queryMemoryMaxBytes`: a property that does not fit is listed in
`skipped` instead of failing the request. Cancellation and the request
deadline still fail it.

## Facets and data-access policies

When a tenant runs with an enforced data-access policy, the catalog reads
answer over what the policy allows: `describe.labels`, `describe.schema`,
`describe.properties`, `describe.schemaSummary`, `describe.names` and
`describe.valueOverlap` see only allowed labels, their visible properties
and relationship shapes of allowed types between allowed labels;
`query.propertyValues` and `query.searchValues` count only records the policy
allows and never search a masked property. With the policy's `minCount`,
groups counted over fewer records are withheld: such values are left out of
every mode (so `count`, `range` and `topK` cover the reported values only).
`describe.explain` is still refused with `unauthorized`. See
[Security](../operations/security.md).

## See also

- [Schema catalog](../data-model/schema-catalog.md)
- [Search queries](search-queries.md)
- [Filtering](filtering.md)
- [Aggregations](aggregations.md)
- [Indexes](../indexes-and-search/indexes.md)
