---
sidebar_position: 1
---

# Create Records

## Create a Record

`POST /api/v1/records`

```bash
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "MOVIE",
    "data": {"title": "Inception", "rating": 8.8, "genre": "sci-fi"},
    "options": {"suggestTypes": true}
  }'
```

### Request body

| Field     | Type   | Description              |
| --------- | ------ | ------------------------ |
| `label`   | string | Label for the new record |
| `data`    | object | Property key-value pairs |
| `options` | object | See options table below  |

### Options

| Option                          | Default                         | Description                                  |
| ------------------------------- | ------------------------------- | -------------------------------------------- |
| `suggestTypes`                  | `true`                          | Infer property types automatically           |
| `convertNumericValuesToNumbers` | `false`                         | Convert string numbers to number type        |
| `capitalizeLabels`              | `false`                         | Uppercase all inferred label names           |
| `skipEmptyValues`               | `false`                         | Skip empty strings/arrays (`0`/`false` kept) |
| `relationshipType`              | `__RUSHDB__RELATION__DEFAULT__` | Relationship type for nested links           |
| `returnResult`                  | `false`                         | Return the created record in the response    |
| `mergeBy`                       | —                               | Fields to match on for upsert                |
| `mergeStrategy`                 | `append`                        | `append` or `rewrite`                        |

## Upsert (create or update)

Supply `mergeBy` and/or `mergeStrategy` in `options` to trigger upsert behavior.

```bash
# Match on 'title'; append/update rating if found
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "MOVIE",
    "data": {"title": "Inception", "rating": 9.0},
    "options": {"mergeBy": ["title"], "mergeStrategy": "append"}
  }'
```

| `mergeBy` value | Match behaviour                     |
| --------------- | ----------------------------------- |
| `["field"]`     | Match only on listed fields         |
| `[]` or omitted | Match on ALL incoming property keys |

| Strategy           | Behaviour                                                      |
| ------------------ | -------------------------------------------------------------- |
| `append` (default) | Add/update incoming fields; preserve all other existing fields |
| `rewrite`          | Replace all fields; unmentioned fields are removed             |

## Precise type control (properties array)

```bash
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "MOVIE",
    "properties": [
      {"name": "title",      "type": "string",   "value": "Inception"},
      {"name": "rating",     "type": "number",   "value": 8.8},
      {"name": "genres",     "type": "string",   "value": "sci-fi,thriller", "valueSeparator": ","},
      {"name": "releasedAt", "type": "datetime", "value": "2010-07-16T00:00:00Z"}
    ]
  }'
```

## With a transaction

```bash
# 1. Begin a transaction
TX_ID=$(curl -s -X POST https://api.rushdb.com/api/v1/tx \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"ttl": 10000}' | jq -r '.data.id')

# 2. Create records using the transaction header
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Transaction-Id: $TX_ID" \
  -d '{"label": "MOVIE", "data": {"title": "Inception"}}'

# 3. Commit
curl -X POST https://api.rushdb.com/api/v1/tx/$TX_ID/commit \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## See also

- [Writing Records with Vectors](/learn/reference/rest-api/ai-and-vectors/write-with-vectors) — attach embedding vectors when creating or upserting records
