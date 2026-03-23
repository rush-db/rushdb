---
sidebar_position: 8
---

# Raw Queries

:::warning Requires a connected Neo4j instance
This endpoint is only available when your project is connected to your own Neo4j database. Connecting a custom Neo4j instance is available on the free tier — see the RushDB dashboard to set it up.
:::

### Python SDK example

```py
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

result = db.query.raw({
    "query": "MATCH (n:Person) RETURN n LIMIT $limit",
    "params": {"limit": 10}
})

print(result.get("data"))
```

### Real-world example: employees at a company

```py
company = 'Acme Corp'
result = db.query.raw({
    "query": "MATCH (c:Company { name: $company })<-[:EMPLOYS]-(p:Person) RETURN p { .name, .email, company: c.name } AS employee ORDER BY p.name LIMIT $limit",
    "params": {"company": company, "limit": 50}
})

print(result.get('data'))
```
