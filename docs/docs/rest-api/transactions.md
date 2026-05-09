---
sidebar_position: 6
---

# Transactions

## Begin Transaction

`POST /api/v1/tx`

```bash
TX_ID=$(curl -s -X POST https://api.rushdb.com/api/v1/tx \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"ttl": 10000}' | jq -r '.data.id')
```

| Field | Type     | Description                          |
| ----- | -------- | ------------------------------------ |
| `ttl` | `number` | TTL in ms. Default: 5000. Max: 30000 |

## Check Transaction Existence

`GET /api/v1/tx/:txId`

```bash
curl https://api.rushdb.com/api/v1/tx/$TX_ID \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## Commit Transaction

`POST /api/v1/tx/:txId/commit`

```bash
curl -X POST https://api.rushdb.com/api/v1/tx/$TX_ID/commit \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## Roll Back Transaction

`POST /api/v1/tx/:txId/rollback`

```bash
curl -X POST https://api.rushdb.com/api/v1/tx/$TX_ID/rollback \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```

## Use with Requests

Pass `X-Transaction-Id` with the transaction ID on any create, update, delete, or relationship endpoint:

```bash
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Transaction-Id: $TX_ID" \
  -d '{"label": "MOVIE", "data": {"title": "Inception"}}'
```

## Full example

```bash
# 1. Begin
TX_ID=$(curl -s -X POST https://api.rushdb.com/api/v1/tx \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"ttl": 10000}' | jq -r '.data.id')

# 2. Create records
MOVIE_ID=$(curl -s -X POST https://api.rushdb.com/api/v1/records \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Transaction-Id: $TX_ID" \
  -d '{"label": "MOVIE", "data": {"title": "Inception"}, "options": {"returnResult": true}}' | jq -r '.data.__id')

ACTOR_ID=$(curl -s -X POST https://api.rushdb.com/api/v1/records \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Transaction-Id: $TX_ID" \
  -d '{"label": "ACTOR", "data": {"name": "Leonardo DiCaprio"}, "options": {"returnResult": true}}' | jq -r '.data.__id')

# 3. Link
curl -X POST https://api.rushdb.com/api/v1/records/$MOVIE_ID/relationships \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Transaction-Id: $TX_ID" \
  -d '{"targetIds": "'$ACTOR_ID'", "type": "STARS_IN"}'

# 4. Commit
curl -X POST https://api.rushdb.com/api/v1/tx/$TX_ID/commit \
  -H "Authorization: Bearer $RUSHDB_API_KEY"
```
