---
sidebar_position: 7
title: Relationship Patterns
---

# Relationship Patterns

RushDB can analyze the project schema and suggest relationships that are not materialized yet. Suggestions are reviewable: no relationship is created until you approve a pattern.

Access the API through `db.relationships.patterns`.

## Methods

| Method                | Description                                                            |
| --------------------- | ---------------------------------------------------------------------- |
| `list()`              | List saved patterns, schema relationships, and analysis status         |
| `analyze()`           | Queue schema analysis to generate suggestions                          |
| `approve(id)`         | Approve a suggestion and apply its relationships                       |
| `ignore(id)`          | Ignore a suggestion without applying it                                |
| `delete(id, options)` | Delete a saved pattern, optionally removing materialized relationships |

## Review Flow

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

// Queue analysis. Fetch results with list() after the analysis completes.
await db.relationships.patterns.analyze()

const { data } = await db.relationships.patterns.list()
for (const pattern of data.patterns) {
  console.log(pattern.id, pattern.type, pattern.confidence)
}

await db.relationships.patterns.approve('pattern-id')
```

Use `ignore()` when a suggestion does not fit your domain model:

```typescript
await db.relationships.patterns.ignore('pattern-id')
```

Deleting a saved pattern does not delete relationships that it already created unless you opt in:

```typescript
await db.relationships.patterns.delete('pattern-id', { deleteExisting: true })
```

For lifecycle details, matching modes, REST endpoints, and MCP tools, see [Suggested Relationship Patterns](/learn/relationships/suggested-patterns).
