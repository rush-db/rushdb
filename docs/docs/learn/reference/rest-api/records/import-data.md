---
sidebar_position: 1
---

# Import Data

## Import JSON

`POST /api/v1/records/import/json`

Pass nested JSON — RushDB walks the structure and creates linked records automatically.

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/json \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "MOVIE",
    "data": {
      "title": "Inception",
      "rating": 8.8,
      "ACTOR": [
        {"name": "Leonardo DiCaprio", "country": "USA"},
        {"name": "Ken Watanabe", "country": "Japan"}
      ]
    },
    "options": {"suggestTypes": true}
  }'
```

`label` is required for top-level arrays, primitive arrays, and JSON objects that have primitive top-level properties.
It can be omitted for container objects whose top-level values are objects or arrays of nested records; in that case each top-level key is used as the label for its nested records:

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/json \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "data": {
      "CHARACTER": [{"name": "Leia Organa"}],
      "STARSHIP": [{"name": "Tantive IV"}]
    },
    "options": {"suggestTypes": true}
  }'
```

### Options

| Option                          | Default                         | Description                                                                                                                                                         |
| ------------------------------- | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `suggestTypes`                  | `true`                          | Infer property types automatically                                                                                                                                  |
| `convertNumericValuesToNumbers` | `false`                         | Convert string numbers to number type                                                                                                                               |
| `capitalizeLabels`              | `false`                         | Uppercase all inferred label names                                                                                                                                  |
| `relationshipType`              | `__RUSHDB__RELATION__DEFAULT__` | Relationship type for nested links                                                                                                                                  |
| `returnResult`                  | `false`                         | Return created records in the response. For imports exceeding 1000 records, this option is ignored and a summary object (`{ message, count }`) is returned instead. |
| `mergeBy`                       | —                               | Property names to match on for upsert-by-property imports                                                                                                           |
| `mergeStrategy`                 | `append`                        | `append` or `rewrite`. Providing either option enables upsert semantics                                                                                             |

## Import CSV

`POST /api/v1/records/import/csv`

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/csv \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "ACTOR",
    "data": "name,country\nLeonardo DiCaprio,USA\nKen Watanabe,Japan",
    "options": {"suggestTypes": true},
    "parseConfig": {"header": true, "dynamicTyping": true}
  }'
```

### `parseConfig` options

| Option           | Default | Description                       |
| ---------------- | ------- | --------------------------------- |
| `delimiter`      | `,`     | Column separator                  |
| `header`         | `true`  | First row is header               |
| `skipEmptyLines` | `true`  | Ignore blank rows                 |
| `dynamicTyping`  | `true`  | Auto-convert numbers and booleans |
| `quoteChar`      | `"`     | Quote character                   |
| `escapeChar`     | `"`     | Escape character                  |
| `newline`        | auto    | Explicit newline sequence         |

## Upsert by Property During Import

Both import endpoints support native upsert-by-property through `options.mergeBy`.
When an incoming row or object matches an existing record on the listed properties, RushDB updates that record instead of creating a duplicate.

`mergeStrategy` controls how matched records are updated:

| Strategy           | Behaviour                                                             |
| ------------------ | --------------------------------------------------------------------- |
| `append` (default) | Add or update incoming fields; preserve other existing fields         |
| `rewrite`          | Replace fields with the incoming data; unmentioned fields are removed |

`mergeBy` controls the match key:

| `mergeBy` value | Match behaviour                     |
| --------------- | ----------------------------------- |
| `["field"]`     | Match only on listed fields         |
| `[]` or omitted | Match on all incoming property keys |

### JSON import upsert

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/json \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "ACTOR",
    "data": [
      {"name": "Leonardo DiCaprio", "country": "USA"},
      {"name": "Ken Watanabe", "country": "Japan"}
    ],
    "options": {"mergeBy": ["name"], "mergeStrategy": "append"}
  }'
```

### CSV import upsert

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/csv \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "ACTOR",
    "data": "name,country\nLeonardo DiCaprio,USA\nKen Watanabe,Japan",
    "options": {"mergeBy": ["name"], "mergeStrategy": "rewrite"},
    "parseConfig": {"header": true}
  }'
```

## See also

- [Writing Records with Vectors](/learn/reference/rest-api/ai-and-vectors/write-with-vectors) — attach pre-computed embedding vectors to records at write time
