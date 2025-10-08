---
sidebar_position: 7
---

# Raw Queries

> **Important (cloud-only):** This endpoint is available only on the RushDB managed cloud service or when your project is connected to a custom database through RushDB Cloud. It is not available for self-hosted or local-only deployments — attempting to use it against a non-cloud instance will fail.

Use this endpoint to run arbitrary Cypher queries against your connected Neo4j database. This is intended for advanced use-cases and requires the managed service or a custom DB connection.

### TypeScript SDK example

```ts
const result = await db.query.raw({
	query: 'MATCH (n:Person) RETURN n LIMIT $limit',
	params: { limit: 10 }
})

// `result` contains the server response with query records as returned by Neo4j driver
console.log(result)
```

### Real-world example: employees at a company

This example shows a parameterized query that finds people employed by a company and returns selected fields.

```ts
const company = 'Acme Corp'
const result = await db.query.raw({
	query: `
		MATCH (c:Company { name: $company })<-[:EMPLOYS]-(p:Person)
		RETURN p { .name, .email, company: c.name } AS employee
		ORDER BY p.name
		LIMIT $limit
	`,
	params: { company, limit: 50 }
})

console.log(result.data)
```
