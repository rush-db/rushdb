---
sidebar_position: 8
---

# Export Data

Export records as CSV. Accepts the same `where` / `orderBy` / `skip` / `limit` / `labels` parameters as a search query.

`POST /api/v1/records/export/csv`

### Request Body

| Field     | Type             | Description                        |
| --------- | ---------------- | ---------------------------------- |
| `where`   | object           | Filter conditions                  |
| `orderBy` | string or object | Sort criteria                      |
| `skip`    | number           | Pagination offset                  |
| `limit`   | number           | Max records to return (up to 1000) |
| `labels`  | array of strings | Restrict to specific labels        |

### Example Request

```bash
curl -X POST https://api.rushdb.com/api/v1/records/export/csv \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "where": {"age": {"$gt": 25}},
    "orderBy": {"name": "asc"},
    "limit": 1000
  }'
```

### Response

```json
{
  "success": true,
  "data": {
    "fileContent": "id,label,name,age,email\n018dfc84...,PERSON,John Doe,30,john@example.com",
    "dateTime": "2025-04-23T10:15:32.123Z"
  }
}
```

`fileContent` is a CSV string with headers on the first row. System properties are stripped automatically.
