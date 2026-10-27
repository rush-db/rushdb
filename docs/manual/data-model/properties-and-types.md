# Properties and types

This page describes the values a property can hold: the supported types, how
JSON input is mapped onto them, how arrays and `null` behave, how property
names are matched, and what happens when the same property has different
types on different records. It is for developers who want predictable
filters, sorting and SQL columns. The `rdb` helper used in the examples is
defined in the [Quickstart](../getting-started/quickstart.md#1-install-and-start-a-node).

## Supported value types

Every stored value has one of five types. JSON input is mapped onto them as
follows.

| JSON input | Stored type | `propertyTypes` name | Notes |
|---|---|---|---|
| `true`, `false` | boolean | `boolean` | |
| Integer, e.g. `42` | 64-bit signed integer | `number` | Integers outside the signed 64-bit range are rejected. |
| Non-integer number, e.g. `4.5` | 64-bit float | `number` | |
| String, e.g. `"Ada"` | UTF-8 string | `string` | |
| ISO-8601 date or datetime string, e.g. `"1815-12-10"` | datetime | `datetime` | Only while type inference is on (the default); see below. |
| `null` | explicit null | — | Present but empty; see [Explicit null](#explicit-null). |
| Array of the above | the element type, several values | element type | See [Multi-value properties](#multi-value-properties). |
| Empty array `[]` | empty array marker | — | Read back as `[]`. |
| Object `{...}` | not a property value | — | Use [nested import](nested-import.md) to turn objects into linked records. |

`propertyTypes` uses display names in which integers and floats are both
`number`. The schema catalog also reports the precise logical types `int64`
and `float64` where the distinction matters; see
[Schema catalog](schema-catalog.md#properties).

## Datetimes

Datetimes are stored as UTC instants with millisecond precision. These string
forms are recognised:

| Input | Interpreted as |
|---|---|
| `2024-05-01` | Midnight UTC on that date |
| `2024-05-01T10:30:00Z`, `2024-05-01T10:30:00.250Z` | That UTC instant (a lowercase `z` is accepted too) |
| `2024-05-01T12:30:00+02:00` | Converted to UTC (`10:30:00Z`); the original offset is not kept |
| `2024-05-01T10:30:00` (no zone) | Treated as UTC |
| `-0032-01-01T08:00:00Z`, `-0600-01-01` | A signed year: ISO 8601 astronomical numbering, so `0000` is 1 BC and `-0032` is 33 BC (proleptic Gregorian), in any of the layouts above |

On output, a datetime that falls exactly on midnight UTC is rendered as a
date (`2024-05-01`); any other instant is rendered as an RFC 3339 timestamp
with milliseconds (`2024-05-01T10:30:00.000Z`). A year before `0000` keeps its
sign (`-0032-01-01T08:00:00.000Z`); RFC 3339 has no signed years, and
JavaScript's `Date` reads them only in the six-digit form (`-000032`). Datetime properties support
range filters and calendar-component filters such as `$year` and `$month`;
see [Filtering](../query/filtering.md).

## Type inference

When records are written with `putRecord` (`create` mode) or through
[nested import](nested-import.md), an `options` object controls how string
values are interpreted:

| Option | Default | Effect |
|---|---|---|
| `suggestTypes` | `true` | Strings that parse as an ISO date or datetime are stored as datetimes. |
| `convertNumericValuesToNumbers` | `false` | With `suggestTypes` on, numeric strings such as `"42"` or `"4.5"` are stored as numbers. |
| `skipEmptyValues` | `false` | Empty strings are not stored, and empty strings are removed from arrays (an array left empty is not stored). |

Keep a date-like string as text by turning inference off:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"putRecord","mode":"create","label":"Code",
  "properties":{"value":"2024-05-01"},
  "options":{"suggestTypes":false}}}}'
```

> **Warning:** With `suggestTypes: false`, values that are not strings are
> also stored as text: `42` becomes `"42"`, `true` becomes `"true"`, and an
> array is converted element by element (`[1, 2]` becomes `["1", "2"]`). Only
> turn inference off for payloads whose values are all meant to be strings.

`putRecord` in `merge` and `replace` mode always uses the defaults. Values on
relationships are never inferred: a date string on a relationship stays a
string.

## Multi-value properties

A property can hold several values. Write an array; each element is stored
as its own typed value, in order:

```json
{ "tags": ["jedi", "pilot"], "scores": [9, 7.5, 10] }
```

Arrays are normalised so that one property has one element type:

| Array | Stored as |
|---|---|
| All strings, all booleans, or all numbers | As given (a mix of integers and floats is stored as floats) |
| All `null` | A list of explicit nulls |
| Mixed types, e.g. `[1, "a", true]` | Every element converted to a string: `["1", "a", "true"]` |
| Values mixed with `null`, e.g. `[1, null]` | Every element converted to a string: `["1", "null"]` |
| Nested arrays | Each inner array converted to a comma-joined string |
| `[]` | Kept as an empty array |

A property value cannot be an object, or an array holding one: `putRecord`
and transaction writes refuse it with `invalid_request`. To store nested
objects, [ingest](nested-import.md) the document; each nested object becomes
a related record.

Filters treat a multi-value property as matching when **any** element
matches. Given the record above:

| Predicate | Matches? |
|---|---|
| `{"tags": "pilot"}` | Yes |
| `{"tags": ["sith", "pilot"]}` | Yes — an array in a predicate means "any of" (`$in`) |
| `{"scores": {"$gte": 10}}` | Yes |
| `{"tags": {"$ne": "jedi"}}` | No — `$ne` requires that no element equals the value |

The schema catalog flags properties that hold arrays (`isArray`).

## Explicit null

`null` is a real, stored value. It is different from the property being
absent:

```bash
rdb '{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{
  "kind":"putRecord","mode":"create","label":"Person",
  "properties":{"name":"Grace Hopper","nickname":null}}}}'
```

| Predicate | Grace (`nickname: null`) | A person without `nickname` |
|---|---|---|
| `{"nickname": null}` | Matches | Does not match |
| `{"nickname": {"$exists": true}}` | Matches | Does not match |
| `{"nickname": {"$exists": false}}` | Does not match | Matches |

To make a property absent again on one record, `replace` the record without
it; a `merge` that sends `null` stores an explicit null. A null has no type:
`describe.properties` and `describe.schema` list a property's nulls as its
own entry of type `null` (the number of records holding `null`), beside an
entry per value type that counts only the records holding a value of that
type. A property that has only ever held `null` has only the `null` entry and
does not appear in `propertyTypes`. Nulls are not values: they are not in
`propertyValues` lists, distinct counts or ranges.

## Property names

Property names are matched by a **normalised** form: ASCII letters are folded
to lower case, other letters and digits are kept, and every other character
becomes `_`.

| You write | Normalised name |
|---|---|
| `firstName`, `FirstName`, `FIRSTNAME` | `firstname` |
| `first-name`, `first name`, `first_name` | `first_name` |

Names with the same normalised form are **one property**. Records show the
spelling used when the property was first written in the tenant, and
predicates, sorting and the schema catalog all use the normalised form, so
`{"FirstName": "Ada"}` and `{"firstname": "Ada"}` are the same filter.

> **Tip:** Pick one naming convention (for example `camelCase` or
> `snake_case`) and use it everywhere. Avoid relying on case to distinguish
> two properties — it cannot.

Names that start with `__` are reserved for the engine (`__id`, `__label`,
`__proptypes`, `__score`, …): a write that uses one is `invalid_request`.
`_id` is an ordinary property. In nested import, `__id` sets the record's id
and the key `$vectors` attaches embeddings.

## One name, several types

There is no schema to violate: the same property may hold a string on one
record and a number on another.

```json
{ "label": "Item", "properties": { "code": "A-17" } }
{ "label": "Item", "properties": { "code": 17 } }
```

The engine keeps a separate typed definition for each type it sees under a
name, and the [schema catalog](schema-catalog.md#properties) lists one entry
per type with a `siblingTypes` marker so clients can tell the property is
split. Integers and floats are one type, `number`: each value is stored as
written (an integer stays an integer, in either order), range filters compare
them as numbers, and a property that holds both is listed once as `number`.
Inside one array a mix of integers and floats is stored as floats.

Mixed types have consequences:

- An equality filter compares values of the matching type, so `{"code": 17}`
  does not match the record whose `code` is the string `"A-17"` or `"17"`.
- Range filters and sorting compare values of the same type; keep a property
  you filter or sort by on one type.

If a property is meant to be numeric, send numbers (or enable
`convertNumericValuesToNumbers` when your source data is text).

## See also

- [Records and labels](records-and-labels.md)
- [Schema catalog](schema-catalog.md)
- [Filtering](../query/filtering.md)
- [Nested import](nested-import.md)
- [LMPG layout](../architecture/lmpg-layout.md)
