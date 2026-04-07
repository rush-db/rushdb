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

### Options

| Option                          | Default                         | Description                            |
| ------------------------------- | ------------------------------- | -------------------------------------- |
| `suggestTypes`                  | `true`                          | Infer property types automatically     |
| `convertNumericValuesToNumbers` | `false`                         | Convert string numbers to number type  |
| `capitalizeLabels`              | `false`                         | Uppercase all inferred label names     |
| `relationshipType`              | `__RUSHDB__RELATION__DEFAULT__` | Relationship type for nested links     |
| `returnResult`                  | `false`                         | Return created records in the response. For imports exceeding 1000 records, this option is ignored and a summary object (`{ message, count }`) is returned instead. |
| `mergeBy`                       | —                               | Fields to match on for upsert          |
| `mergeStrategy`                 | `append`                        | `append` or `rewrite`                  |

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

## Upsert during import

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

## See also

- [Writing Records with Vectors](../ai/write-with-vectors.md) — attach pre-computed embedding vectors to records at write time
