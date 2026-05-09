---
sidebar_position: 8
---

# Raw Queries

:::warning Requires a connected Neo4j instance
This endpoint is only available when your project is connected to your own Neo4j database. Connecting a custom Neo4j instance is available on the free tier — see the RushDB dashboard to set it up.
:::

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
