---
sidebar_position: 5
---

# Labels

## `POST /api/v1/labels/search`

Returns all labels and their record counts. Pass a `where` clause to filter by record properties.

```bash
# All labels
curl -X POST https://api.rushdb.com/api/v1/labels/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{}'

# Labels that have records matching a condition
curl -X POST https://api.rushdb.com/api/v1/labels/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"where": {"rating": {"$gte": 8}}}'
```

Response: map of `{ label: count }`.


