---
sidebar_position: 6
---

# Update Records

## `PATCH /api/v1/records/:entityId` — partial update

Unspecified fields are preserved.

```bash
curl -X PATCH https://api.rushdb.com/api/v1/records/movie-123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"data": {"rating": 9.0}}'
```

## `PUT /api/v1/records/:entityId` — full replacement

All previous fields are removed, then replaced with the new data.

```bash
curl -X PUT https://api.rushdb.com/api/v1/records/movie-123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"label": "MOVIE", "data": {"title": "Inception", "rating": 9.0, "genre": "sci-fi"}}'
```

## Request body

| Field | Type | Description |
|---|---|---|
| `label` | `string` | (Optional) New label for the record |
| `data` | `object` | Properties to write |

