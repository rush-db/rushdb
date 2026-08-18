# RushDB Memory Patterns

Use these patterns after selecting an integration mode in `SKILL.md`.

## Ownership Matrix

| Data                             | Native/custom adapter     | MCP/application graph                      |
| -------------------------------- | ------------------------- | ------------------------------------------ |
| Completed turn                   | `EPISODE`                 | Do not duplicate                           |
| Curated runtime fact             | `MEMORY_FACT`             | Write only through an authorized fact path |
| Work boundary                    | Optional runtime metadata | `SESSION` when explicitly useful           |
| Decision, task, entity, artifact | No automatic projection   | Explicit structured record                 |

## MCP-Only Session

```json
{
  "label": "SESSION",
  "data": {
    "sessionId": "sess_20260818_001",
    "startedAt": "2026-08-18T09:00:00Z",
    "topic": "architecture review",
    "agentId": "coding-agent"
  }
}
```

Create this only in MCP-only mode or when the application explicitly wants a graph-visible session record.

## Session with Explicit Knowledge

Use `bulkCreateRecords` with nested objects to create and link durable knowledge:

```json
{
  "label": "SESSION",
  "data": {
    "sessionId": "sess_20260818_001",
    "startedAt": "2026-08-18T09:00:00Z",
    "topic": "authentication design",
    "DECISION": [
      {
        "topic": "auth provider",
        "decision": "Use Clerk",
        "rationale": "Better framework integration",
        "decidedAt": "2026-08-18T09:15:00Z",
        "status": "confirmed"
      }
    ],
    "TASK": [
      {
        "title": "Remove the old auth dependency",
        "status": "pending",
        "createdAt": "2026-08-18T09:20:00Z"
      }
    ],
    "ENTITY": [{ "name": "Clerk", "type": "service" }]
  }
}
```

In mixed native + MCP mode, omit the outer `SESSION` when it would merely duplicate the runtime conversation. Create the domain records directly and link them to existing domain entities instead.

## Explicit Preference

```json
{
  "label": "PREFERENCE",
  "data": {
    "subjectKey": "user:local",
    "category": "coding-style",
    "preference": "Prefer TypeScript strict mode",
    "active": true,
    "observedAt": "2026-08-18T09:30:00Z",
    "provenance": "explicit-user-request"
  },
  "options": {
    "mergeBy": ["subjectKey", "category"],
    "mergeStrategy": "append"
  }
}
```

This is a domain record, not a canonical `MEMORY_FACT`. Use the canonical fact contract only when trusted runtime scope fields are available.

## Recall Explicit Decisions

```json
{
  "labels": ["DECISION"],
  "where": { "topic": { "$contains": "auth" } },
  "orderBy": { "decidedAt": "desc" },
  "limit": 10
}
```

## Recall Canonical Memory

Use `vectorSearch` separately for each semantic property. Insert the complete trusted scope in `where` before executing:

```json
{
  "labels": ["EPISODE"],
  "propertyName": "summary",
  "query": "authentication decision",
  "where": {
    "agentId": "<trusted-agent-id>",
    "profileId": "<trusted-profile-id>",
    "privacyScope": "private",
    "participantScopeHash": "<trusted-hash>",
    "sandboxEligible": false
  },
  "limit": 8
}
```

For facts, use `labels: ["MEMORY_FACT"]`, `propertyName: "text"`, and add `active: true` to the same scope filter. Merge and rank the two result sets in the adapter; do not remove authorization predicates to increase recall.

## Relationships

Use explicit relationship types for domain knowledge:

| Type              | Direction                       |
| ----------------- | ------------------------------- |
| `HAS_DECISION`    | `SESSION` → `DECISION`          |
| `HAS_TASK`        | `SESSION` → `TASK`              |
| `MENTIONS_ENTITY` | memory/domain record → `ENTITY` |
| `PRODUCES`        | `SESSION` → `ARTIFACT`          |
| `DEPENDS_ON`      | `TASK` → `TASK`                 |

Keep scalar identity properties such as `sessionId` when they enable direct filtering; do not rely exclusively on traversal.

## Naming and Evolution

- Treat labels as case-sensitive. Use `UPPER_CASE` as the RushDB memory convention, not as a database requirement.
- Use `camelCase` properties and ISO 8601 UTC timestamps.
- Add new properties without migration; records lacking them remain valid.
- Deprecate before deleting, preview destructive targets, and preserve supersession history.
