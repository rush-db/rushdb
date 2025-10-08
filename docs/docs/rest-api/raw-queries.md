---
sidebar_position: 7
---

# Raw Queries

> **Important (cloud-only):** This endpoint is available only on the RushDB managed cloud service or when your project is connected to a custom database through RushDB Cloud. It is not available for self-hosted or local-only deployments — attempting to use it against a non-cloud instance will fail.

### REST API

Endpoint: POST /query/raw

Body:

```json
{
	"query": "MATCH (n:Person) RETURN n LIMIT $limit",
	"params": { "limit": 10 }
}
```

Response: raw Neo4j driver result object.

### Real-world example: employees at a company

Request body:

```json
{
	"query": "MATCH (c:Company { name: $company })<-[:EMPLOYS]-(p:Person) RETURN p { .name, .email, company: c.name } AS employee ORDER BY p.name LIMIT $limit",
	"params": { "company": "Acme Corp", "limit": 50 }
}
```
