---
sidebar_position: 5
---

# Get Records

## `GET /api/v1/records/:entityId`

```bash
curl https://api.rushdb.com/api/v1/records/movie-123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## `POST /api/v1/records/search`

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

| Field | Type | Description |
|---|---|---|
| `labels` | `string[]` | Filter by one or more labels |
| `where` | `object` | Field conditions and operators |
| `orderBy` | `object` | `{"field": "asc" \| "desc"}` |
| `limit` | `number` | Max records. **Omit when using `aggregate`** |
| `skip` | `number` | Records to skip (pagination) |
| `aggregate` | `object` | Aggregation functions |
| `groupBy` | `string[]` | Group aggregated results |

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

### Aggregations

```bash
curl -X POST https://api.rushdb.com/api/v1/records/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "labels": ["MOVIE"],
    "aggregate": {
      "count":     {"fn": "count",  "alias": "$record"},
      "avgRating": {"fn": "avg",    "alias": "$record", "field": "rating"}
    }
  }'
```

:::danger
**Never set `limit` with `aggregate`** — it restricts the record scan and produces wrong totals.
:::

### GroupBy

```bash
curl -X POST https://api.rushdb.com/api/v1/records/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "labels": ["MOVIE"],
    "aggregate": {
      "count":     {"fn": "count", "alias": "$record"},
      "avgRating": {"fn": "avg",   "alias": "$record", "field": "rating"}
    },
    "groupBy": ["$record.genre"],
    "orderBy": {"count": "desc"}
  }'
```

## `POST /api/v1/records/:entityId/search`

Contextual search within a specific record's relationships:

```bash
curl -X POST https://api.rushdb.com/api/v1/records/movie-123/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"labels": ["ACTOR"], "where": {"country": "USA"}}'
```


