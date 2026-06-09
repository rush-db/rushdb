---
name: rushdb-agent-memory
description: Use RushDB as a persistent, structured memory layer for AI agents. Use this skill whenever an agent needs to store session data, remember past decisions, recall prior context by meaning, build a knowledge graph that survives across conversations, associate memories via relationships, or replace a separate vector DB / key-value store with a single ACID-safe graph. Also use when the user says "remember this", "store that", or "what did we decide about X".
homepage: https://rushdb.com
---

# RushDB Agent Memory

RushDB replaces three separate memory systems — vector DB, key-value store, and graph — with a unified, ACID-safe, semantically searchable property graph.

## Prerequisites

- **RushDB MCP server** must be connected — it provides `createRecord`, `findRecords`, `bulkCreateRecords`, and all other tools used in this skill. Setup: `npx @rushdb/mcp-server` (requires `RUSHDB_API_KEY` env var). See https://docs.rushdb.com/mcp-server/quickstart
- If the MCP tools are not available in the current session, tell the user the MCP server is not configured and link them to the quickstart above.

- **Records** store structured data (any JSON, any shape)
- **Auto-linking** turns nested JSON into a relationship graph — no manual edge creation
- **Semantic search** retrieves memories by meaning (managed embeddings, no pipeline)
- **Transactions** keep concurrent agents from corrupting shared memory

---

## Core Pattern: Store → Link → Recall

### 1. Store a memory

Call `createRecord` with a label that classifies the memory type and a `data` object:

```json
{
  "label": "DECISION",
  "data": {
    "topic": "authentication",
    "decision": "Use Clerk for auth, replacing Auth0",
    "rationale": "Better Next.js integration, lower ops overhead",
    "decidedAt": "2026-04-10T00:00:00Z",
    "participants": ["Alice", "Bob"],
    "sessionId": "sess_abc123"
  }
}
```

### 2. Store a session with nested entities (auto-linking)

Supply nested objects to `bulkCreateRecords` — RushDB auto-creates relationships:

```json
{
  "label": "SESSION",
  "data": {
    "sessionId": "sess_abc123",
    "startedAt": "2026-04-10T09:00:00Z",
    "topic": "architecture review",
    "DECISION": [
      {
        "topic": "authentication",
        "decision": "Use Clerk",
        "decidedAt": "2026-04-10T09:15:00Z"
      },
      {
        "topic": "database",
        "decision": "Use RushDB for memory layer",
        "decidedAt": "2026-04-10T09:30:00Z"
      }
    ],
    "ENTITY": [
      { "name": "Clerk", "type": "service", "role": "auth provider" },
      { "name": "RushDB", "type": "service", "role": "memory layer" }
    ]
  }
}
```

This creates one `SESSION`, two `DECISION` records, and two `ENTITY` records — all linked by relationships automatically.

### 3. Recall by meaning (semantic search)

Find memories semantically without knowing the exact words:

```json
{
  "labels": ["DECISION"],
  "where": {
    "topic": { "$contains": "auth" }
  }
}
```

For vector/embedding-based recall, use the `aggregate` `vector.similarity.cosine` function (see `rushdb-query-builder` skill, references/search-query-spec.md §1 vector section).

### 4. Traverse related memories

```json
{
  "labels": ["SESSION"],
  "where": {
    "topic": { "$contains": "architecture" },
    "DECISION": { "$alias": "$decision" }
  },
  "aggregate": {
    "sessionTopic": "$record.topic",
    "startedAt": "$record.startedAt",
    "decisions": {
      "fn": "collect",
      "alias": "$decision",
      "aggregate": {}
    }
  }
}
```

---

## Recommended Label Conventions

Use these as starting points — adapt to your agent's domain:

| Label         | Stores                                         |
| ------------- | ---------------------------------------------- |
| `SESSION`     | A conversation or work session                 |
| `DECISION`    | A decision made, with rationale and timestamp  |
| `ENTITY`      | A named thing (person, service, file, concept) |
| `TASK`        | A work item or action, with status             |
| `OBSERVATION` | A raw note or finding (less structured)        |
| `PREFERENCE`  | A user preference or constraint                |
| `PLAN`        | A proposed sequence of steps                   |
| `ARTIFACT`    | A produced output (code file, document, etc.)  |

---

## Memory Operations Reference

### Write memory

| Goal                   | Tool                | Notes                                              |
| ---------------------- | ------------------- | -------------------------------------------------- |
| Store one memory       | `createRecord`      | `{ label, data }`                                  |
| Store many + auto-link | `bulkCreateRecords` | Nested JSON = auto relationships                   |
| Update a memory        | `updateRecord`      | Patch fields; preserves unmentioned fields         |
| Replace a memory       | `setRecord`         | Overwrites all fields                              |
| Delete a memory        | `deleteRecordById`  | Irreversible — confirm first                       |
| Delete many            | `bulkDeleteRecords` | **Destructive** — preview with `findRecords` first |

### Read memory

| Goal                    | Tool                                      | Notes                                        |
| ----------------------- | ----------------------------------------- | -------------------------------------------- |
| Search by topic/content | `findRecords`                             | Use `where` with `$contains` for fuzzy match |
| Get by ID               | `getRecord`                               | When you have the record ID                  |
| Get all of type         | `findRecords` with `labels`               | e.g. all `DECISION` records                  |
| Semantic recall         | `findRecords` with `aggregate.similarity` | Needs embedding index set up                 |
| Recall related memories | `findRecords` with traversal              | Traverse by label in `where`                 |
| List memory types       | `getOntologyMarkdown`                     | Returns all labels + counts                  |

### Link memories

| Goal                   | Tool                | Notes                                                                            |
| ---------------------- | ------------------- | -------------------------------------------------------------------------------- |
| Connect two records    | `attachRelation`    | `{ sourceId, targetId, type }`                                                   |
| Disconnect two records | `detachRelation`    |                                                                                  |
| Explore connections    | `findRelationships` | Use `source.where.$id` and `target.where.$id` to see outgoing and incoming links |

---

## Working with Transactions

For write-heavy operations or concurrent agents, use transactions to keep memory consistent:

1. Start a transaction (available in the RushDB SDK — pass `transactionId` to tool calls)
2. Perform all writes inside the transaction
3. Commit or roll back

The MCP tools accept an optional `transactionId` parameter. Passing the same ID to multiple tool calls groups them into one atomic operation.

---

## Session Memory Pattern

At the start of a session:

1. Call `getOntologyMarkdown` — get existing memory types and counts
2. Call `findRecords` with `labels:["SESSION"]`, `orderBy:{ startedAt:'desc' }`, `limit:1` — recall the most recent session
3. Store a new `SESSION` record for this conversation

At the end of a session (or when directed):

1. Store key `DECISION`, `ENTITY`, and `TASK` records from the conversation
2. Link them to the `SESSION` with `attachRelation` (or via nested JSON on the SESSION write)

---

## Recall Patterns

### "What did we decide about X?"

```json
{
  "labels": ["DECISION"],
  "where": { "topic": { "$contains": "X" } },
  "orderBy": { "decidedAt": "desc" },
  "limit": 10
}
```

### "What sessions have we had?"

```json
{
  "labels": ["SESSION"],
  "orderBy": { "startedAt": "desc" },
  "limit": 20
}
```

### "What do we know about entity Y?"

```json
{
  "labels": ["ENTITY"],
  "where": { "name": { "$contains": "Y" } }
}
```

Then traverse: `findRelationships` with `source.where.$id` and `target.where.$id` for the returned record to see connected decisions, sessions, and tasks.

### "What happened in the last 7 days?"

Compute ISO boundary for 7 days ago:

```json
{
  "labels": ["SESSION", "DECISION", "TASK"],
  "where": { "createdAt": { "$gte": "2026-04-06T00:00:00Z" } },
  "orderBy": { "createdAt": "desc" }
}
```

---

## Reference

For the full SearchQuery syntax (operators, aggregation, traversal), load:
`references/search-query-spec.md` (in the `rushdb-query-builder` skill)

or call the MCP tool `getSearchQuerySpec`.
