---
sidebar_position: 6
---

# Raw Queries

> Available only on managed service or with custom database connected to rushdb.

> **Important (cloud-only):** This endpoint is available only on the RushDB managed cloud service or when your project is connected to a custom database through RushDB Cloud. It is not available for self-hosted or local-only deployments — attempting to use it against a non-cloud instance will fail.

### Python SDK example

```py
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

result = db.query.raw({
	"query": "MATCH (n:Person) RETURN n LIMIT $limit",
	"params": {"limit": 10}
})

print(result)
```

### Real-world example: employees at a company

```py
company = 'Acme Corp'
result = db.query.raw({
	"query": "MATCH (c:Company { name: $company })<-[:EMPLOYS]-(p:Person) RETURN p { .name, .email, company: c.name } AS employee ORDER BY p.name LIMIT $limit",
	"params": {"company": company, "limit": 50}
})

print(result['data'])
```