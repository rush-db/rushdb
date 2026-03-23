---
sidebar_position: 8
---

# Raw Queries

:::warning Requires a connected Neo4j instance
This endpoint is only available when your project is connected to your own Neo4j database. Connecting a custom Neo4j instance is available on the free tier — see the RushDB dashboard to set it up.
:::

### REST API

```http
POST /api/v1/query/raw
```

```bash
curl -X POST https://api.rushdb.com/api/v1/query/raw \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "query": "MATCH (n:Person) RETURN n LIMIT $limit",
    "params": { "limit": 10 }
  }'
```

Response: raw Neo4j driver result object.

### Real-world example: employees at a company

```bash
curl -X POST https://api.rushdb.com/api/v1/query/raw \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{
    "query": "MATCH (c:Company { name: $company })<-[:EMPLOYS]-(p:Person) RETURN p { .name, .email, company: c.name } AS employee ORDER BY p.name LIMIT $limit",
    "params": { "company": "Acme Corp", "limit": 50 }
  }'
```
