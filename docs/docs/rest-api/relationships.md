---
sidebar_position: 3
---

# Relationships

## `POST /api/v1/records/:entityId/relationships` — attach

```bash
# Single target
curl -X POST https://api.rushdb.com/api/v1/records/$MOVIE_ID/relationships \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"targetIds": "$ACTOR_ID", "type": "STARS_IN", "direction": "out"}'

# Multiple targets
curl -X POST https://api.rushdb.com/api/v1/records/$MOVIE_ID/relationships \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"targetIds": ["$ACTOR1_ID", "$ACTOR2_ID"], "type": "STARS_IN"}'
```

| Field | Type | Description |
|---|---|---|
| `targetIds` | `string \| string[]` | Target record ID(s) |
| `type` | `string` | Relationship type |
| `direction` | `"in" \| "out"` | Direction from source to target (`out` = default) |

## `PUT /api/v1/records/:entityId/relationships` — detach

```bash
curl -X PUT https://api.rushdb.com/api/v1/records/$MOVIE_ID/relationships \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"targetIds": "$ACTOR_ID", "typeOrTypes": ["STARS_IN"]}'
```

## `GET /api/v1/records/:entityId/relationships` — list

```bash
curl "https://api.rushdb.com/api/v1/records/$MOVIE_ID/relationships?limit=50" \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## `POST /api/v1/relationships/create-many` — bulk create by key match

```bash
curl -X POST https://api.rushdb.com/api/v1/relationships/create-many \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "source": {"label": "MOVIE",  "key": "id"},
    "target": {"label": "ACTOR",  "key": "movieId"},
    "type": "STARS_IN",
    "direction": "out"
  }'
```

| Field | Type | Description |
|---|---|---|
| `source` | `{label, key?, where?}` | Source selector |
| `target` | `{label, key?, where?}` | Target selector |
| `type` | `string` | Relationship type |
| `direction` | `string` | `in` or `out` |
| `manyToMany` | `boolean` | Cartesian product mode — requires non-empty `where` on both sides |

## `POST /api/v1/relationships/delete-many` — bulk delete

Same shape as `create-many`. Omit `type` to delete any type.

```bash
curl -X POST https://api.rushdb.com/api/v1/relationships/delete-many \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "source": {"label": "MOVIE", "key": "id"},
    "target": {"label": "ACTOR", "key": "movieId"},
    "type": "STARS_IN"
  }'
```

:::warning
Setting `manyToMany: true` without `where` filters on both sides creates an unbounded cartesian product.
:::

## `POST /api/v1/relationships/search`

```bash
curl -X POST https://api.rushdb.com/api/v1/relationships/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"where": {"sourceRecord": {"title": "Inception"}}}'
```

## Direction

| Value | Meaning |
|---|---|
| `"out"` (default) | source → target |
| `"in"` | target → source |


