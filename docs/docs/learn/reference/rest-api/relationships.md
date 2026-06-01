---
slug: /rest-api/relationships
sidebar_position: 3
---

# Relationships

## Attach

`POST /api/v1/records/:entityId/relationships`

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

| Field       | Type                 | Description                                       |
| ----------- | -------------------- | ------------------------------------------------- |
| `targetIds` | `string \| string[]` | Target record ID(s)                               |
| `type`      | `string`             | Relationship type                                 |
| `direction` | `"in" \| "out"`      | Direction from source to target (`out` = default) |

## Detach

`PUT /api/v1/records/:entityId/relationships`

```bash
curl -X PUT https://api.rushdb.com/api/v1/records/$MOVIE_ID/relationships \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"targetIds": "$ACTOR_ID", "typeOrTypes": ["STARS_IN"]}'
```

## List Relationships

`GET /api/v1/records/:entityId/relationships`

```bash
curl "https://api.rushdb.com/api/v1/records/$MOVIE_ID/relationships?limit=50" \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## Bulk Create by Key Match

`POST /api/v1/relationships/create-many`

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

| Field        | Type                    | Description                                                       |
| ------------ | ----------------------- | ----------------------------------------------------------------- |
| `source`     | `{label, key?, where?}` | Source selector                                                   |
| `target`     | `{label, key?, where?}` | Target selector                                                   |
| `type`       | `string`                | Relationship type                                                 |
| `direction`  | `string`                | `in` or `out`                                                     |
| `manyToMany` | `boolean`               | Cartesian product mode — requires non-empty `where` on both sides |

## Bulk Delete

`POST /api/v1/relationships/delete-many`

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

## Search Relationships

`POST /api/v1/relationships/search`

```bash
curl -X POST https://api.rushdb.com/api/v1/relationships/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"where": {"sourceRecord": {"title": "Inception"}}}'
```

## Suggested Patterns

RushDB can analyze a project's ontology and propose relationship patterns for review before applying them. See [Suggested Relationship Patterns](/build/graph/suggested-patterns) for the lifecycle, SDK examples, and all five REST endpoints.

## Direction

| Value             | Meaning         |
| ----------------- | --------------- |
| `"out"` (default) | source → target |
| `"in"`            | target → source |
