---
sidebar_position: 7
---

# Delete Records

## `DELETE /api/v1/records/:entityId`

```bash
curl -X DELETE https://api.rushdb.com/api/v1/records/movie-123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## `POST /api/v1/records/delete`

Delete all records matching a query.

```bash
curl -X POST https://api.rushdb.com/api/v1/records/delete \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"labels": ["MOVIE"], "where": {"rating": {"$lt": 5}}}'
```

:::warning
Omitting `where` deletes **all** records with the given label.
:::

