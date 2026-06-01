---
slug: /rest-api/properties
sidebar_position: 4
---

# Properties

## Search Properties

`POST /api/v1/properties/search`

```bash
curl -X POST https://api.rushdb.com/api/v1/properties/search \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"where": {"type": "string"}}'
```

## Get Property by ID

`GET /api/v1/properties/:propertyId`

```bash
curl https://api.rushdb.com/api/v1/properties/prop-123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## Get Property Values

`POST /api/v1/properties/:propertyId/values`

Returns distinct values for a property — useful for filter UIs.

```bash
curl -X POST https://api.rushdb.com/api/v1/properties/prop-123/values \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"query": "sci", "orderBy": "asc", "limit": 100}'
```

| Field     | Type              | Description                        |
| --------- | ----------------- | ---------------------------------- |
| `query`   | `string`          | Filter values containing this text |
| `orderBy` | `"asc" \| "desc"` | Sort direction                     |
| `skip`    | `number`          | Pagination offset                  |
| `limit`   | `number`          | Max values to return               |

## Delete Property

`DELETE /api/v1/properties/:propertyId`

:::warning
Deletes the property and removes it from **all records** in the database.
:::

```bash
curl -X DELETE https://api.rushdb.com/api/v1/properties/prop-123 \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```
