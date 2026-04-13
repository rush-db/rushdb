# RushDB Memory Patterns

Concrete JSON structures and naming conventions for using RushDB as an agent memory layer.

---

## Table of Contents

1. [Minimal Session Record](#minimal-session-record)
2. [Session with Nested Entities (Auto-Linking)](#session-with-nested-entities-auto-linking)
3. [Decision Record](#decision-record)
4. [Task / Action Item](#task--action-item)
5. [Entity Record](#entity-record)
6. [Observation / Note](#observation--note)
7. [Preference Record](#preference-record)
8. [Artifact Record](#artifact-record)
9. [Relationship Attach Pattern](#relationship-attach-pattern)
10. [Naming Conventions](#naming-conventions)
11. [Schema Evolution](#schema-evolution)

---

## Minimal Session Record

```json
{
  "label": "SESSION",
  "data": {
    "sessionId": "sess_20260410_001",
    "startedAt": "2026-04-10T09:00:00Z",
    "topic": "architecture review",
    "agentId": "copilot-agent-v2",
    "userId": "user_alice"
  }
}
```

---

## Session with Nested Entities (Auto-Linking)

Use `bulkCreateRecords` with nested objects. RushDB creates one record per nested label and links them automatically via relationships.

```json
{
  "label": "SESSION",
  "data": {
    "sessionId": "sess_20260410_001",
    "startedAt": "2026-04-10T09:00:00Z",
    "topic": "authentication design",
    "DECISION": [
      {
        "topic": "auth provider",
        "decision": "Use Clerk for authentication, replacing Auth0",
        "rationale": "Better Next.js App Router integration, simplified ops",
        "decidedAt": "2026-04-10T09:15:00Z",
        "status": "confirmed"
      }
    ],
    "ENTITY": [
      {
        "name": "Clerk",
        "type": "service",
        "role": "auth provider",
        "url": "https://clerk.com"
      },
      {
        "name": "Auth0",
        "type": "service",
        "role": "replaced auth provider",
        "status": "deprecated"
      }
    ],
    "TASK": [
      {
        "title": "Remove Auth0 dependency",
        "status": "pending",
        "assignee": "Alice",
        "dueDate": "2026-04-17T00:00:00Z"
      },
      {
        "title": "Install and configure Clerk",
        "status": "pending",
        "assignee": "Alice",
        "dueDate": "2026-04-15T00:00:00Z"
      }
    ]
  }
}
```

This produces:
- 1 `SESSION` record
- 1 `DECISION` record → linked to the `SESSION`
- 2 `ENTITY` records → each linked to the `SESSION`
- 2 `TASK` records → each linked to the `SESSION`

---

## Decision Record

```json
{
  "label": "DECISION",
  "data": {
    "topic": "database selection",
    "decision": "Use PostgreSQL for primary storage",
    "alternatives": ["MySQL", "MongoDB", "CockroachDB"],
    "rationale": "Strong JSONB support, mature ecosystem, team familiarity",
    "decidedAt": "2026-04-10T10:00:00Z",
    "decidedBy": ["Alice", "Bob"],
    "status": "confirmed",
    "sessionId": "sess_20260410_001",
    "reversible": true,
    "tags": ["infrastructure", "database"]
  }
}
```

**Standard `status` values:** `"confirmed"` | `"proposed"` | `"superseded"` | `"rejected"`

---

## Task / Action Item

```json
{
  "label": "TASK",
  "data": {
    "title": "Set up CI/CD pipeline",
    "description": "Configure GitHub Actions for automated testing and deployment",
    "status": "in-progress",
    "priority": "high",
    "assignee": "Bob",
    "createdAt": "2026-04-10T09:00:00Z",
    "dueDate": "2026-04-20T00:00:00Z",
    "completedAt": null,
    "tags": ["devops", "automation"],
    "sessionId": "sess_20260410_001"
  }
}
```

**Standard `status` values:** `"pending"` | `"in-progress"` | `"done"` | `"blocked"` | `"cancelled"`
**Standard `priority` values:** `"critical"` | `"high"` | `"medium"` | `"low"`

---

## Entity Record

An entity is any named thing the agent should remember: a person, service, file, concept, or tool.

```json
{
  "label": "ENTITY",
  "data": {
    "name": "RushDB",
    "type": "service",
    "category": "database",
    "description": "Property-centric graph database with managed embeddings",
    "url": "https://rushdb.com",
    "role": "memory layer",
    "status": "active",
    "tags": ["graph", "ai", "memory"]
  }
}
```

**Common `type` values:** `"service"`, `"person"`, `"file"`, `"concept"`, `"tool"`, `"repository"`, `"organization"`

---

## Observation / Note

Less structured than a decision — a raw finding, thought, or piece of context.

```json
{
  "label": "OBSERVATION",
  "data": {
    "content": "The current auth flow has 3 redirects before reaching the dashboard. Users report confusion.",
    "source": "user interview",
    "observedAt": "2026-04-10T09:45:00Z",
    "topic": "authentication UX",
    "confidence": "high",
    "sessionId": "sess_20260410_001"
  }
}
```

---

## Preference Record

A persistent user preference or constraint the agent should always respect.

```json
{
  "label": "PREFERENCE",
  "data": {
    "userId": "user_alice",
    "category": "coding style",
    "preference": "Prefer TypeScript strict mode with no implicit any",
    "scope": "all projects",
    "createdAt": "2026-04-01T00:00:00Z",
    "active": true
  }
}
```

---

## Artifact Record

A produced output that the agent should be able to reference later.

```json
{
  "label": "ARTIFACT",
  "data": {
    "name": "auth-flow-diagram.png",
    "type": "image",
    "description": "Diagram of the new Clerk-based authentication flow",
    "path": "/docs/diagrams/auth-flow-diagram.png",
    "url": null,
    "createdAt": "2026-04-10T10:30:00Z",
    "sessionId": "sess_20260410_001",
    "tags": ["authentication", "diagram"]
  }
}
```

---

## Relationship Attach Pattern

After creating records separately, link them with `attachRelation`:

```json
{
  "sourceId": "<SESSION_record_id>",
  "targetId": "<DECISION_record_id>",
  "type": "HAS_DECISION"
}
```

Common relationship types:
| Type | Meaning |
|---|---|
| `HAS_DECISION` | SESSION → DECISION |
| `HAS_TASK` | SESSION → TASK |
| `MENTIONS_ENTITY` | SESSION or DECISION → ENTITY |
| `PRODUCES` | SESSION → ARTIFACT |
| `SUPERSEDES` | DECISION → DECISION (new decision replaces old) |
| `DEPENDS_ON` | TASK → TASK |
| `ASSIGNED_TO` | TASK → ENTITY (person) |

---

## Naming Conventions

### Labels
- **UPPER_CASE** — required by RushDB's LMPG model
- Use nouns, not verbs: `DECISION` not `DECIDED`, `TASK` not `TASK_CREATED`
- Keep them broad enough to reuse across sessions

### Properties
- **camelCase** — e.g. `sessionId`, `decidedAt`, `isActive`
- Use ISO 8601 for all datetimes: `"2026-04-10T09:00:00Z"`
- Use consistent enum values across records (see `rushdb-data-modeling` skill for more)

### Relationship Types
- **UPPER_SNAKE_CASE** — e.g. `HAS_DECISION`, `DEPENDS_ON`
- Directional: read left→right as a sentence (`SESSION HAS_DECISION DECISION`)

### ID References
- Store `sessionId` as a property on child records for easy flat filtering
- Don't rely solely on graph traversal when a scalar filter is faster

---

## Schema Evolution

### Adding a new property
Just include it in `data` on new records. Existing records that lack the property are simply unmatched by filters on that field.

### Renaming a label
Use `findRecords` to retrieve all records of the old label, re-create them with the new label, attach original relationships, then `bulkDeleteRecords` on the old label. Always preview before deleting.

### Deprecating a label
Add a `deprecated: true` property to existing records, stop creating new ones, and filter `{ deprecated: { $ne: true } }` in queries.

### Splitting a label
Create the two new labels, copy records, update relationships, deprecate the old label.
