---
sidebar_position: 5
---

# Get Records

## Get Record by ID

`GET /api/v1/records/:entityId`

```bash
curl https://api.rushdb.com/api/v1/records/movie-123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## Search Records

`POST /api/v1/records/search`

```bash
curl -X POST https://api.rushdb.com/api/v1/records/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "labels": ["MOVIE"],
    "where": {"rating": {"$gte": 8}},
    "orderBy": {"rating": "desc"},
    "limit": 10
  }'
```

### Request body

| Field     | Type       | Description                               |
| --------- | ---------- | ----------------------------------------- |
| `labels`  | `string[]` | Filter by one or more labels              |
| `where`   | `object`   | Field conditions and operators            |
| `orderBy` | `object`   | `{"field": "asc" \| "desc"}`              |
| `limit`   | `number`   | Max records. **Omit when using `select`** |
| `skip`    | `number`   | Records to skip (pagination)              |
| `select`  | `object`   | Output-shaping expressions (preferred)    |
| `groupBy` | `string[]` | Group aggregated results                  |

### Relationship traversal

```bash
curl -X POST https://api.rushdb.com/api/v1/records/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "labels": ["MOVIE"],
    "where": {
      "ACTOR": {
        "$relation": {"type": "STARS_IN", "direction": "in"},
        "country": "USA"
      }
    }
  }'
```

### Select Expressions

```bash
curl -X POST https://api.rushdb.com/api/v1/records/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "labels": ["MOVIE"],
    "select": {
      "count":     {"$count": "*"},
      "avgRating": {"$avg": "$record.rating"}
    }
  }'
```

:::danger
**Never set `limit` with `select`** — it restricts the record scan and produces wrong totals.
:::

### GroupBy

```bash
curl -X POST https://api.rushdb.com/api/v1/records/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "labels": ["MOVIE"],
    "select": {
      "count":     {"$count": "*"},
      "avgRating": {"$avg": "$record.rating"}
    },
    "groupBy": ["$record.genre"],
    "orderBy": {"count": "desc"}
  }'
```

## Search Record Relationships

`POST /api/v1/records/:entityId/search`

Contextual search within a specific record's relationships:

```bash
curl -X POST https://api.rushdb.com/api/v1/records/movie-123/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"labels": ["ACTOR"], "where": {"country": "USA"}}'
```
