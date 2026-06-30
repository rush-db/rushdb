---
name: rushdb-domain-template
description: Design a tailored RushDB schema for any use case through guided conversation. Use this skill when a user is starting a new project, unsure what labels to define, asking "what records should I create", or wants a scaffold for a known domain (e-commerce, healthcare, SaaS, CRM, agent memory, etc.). Interviews the user, maps their domain to a starter schema of RECORD labels + properties + relationships, and outputs a ready-to-run bulkCreateRecords bootstrap payload.
homepage: https://rushdb.com
---

# RushDB Domain Template

RushDB has one primitive: a **RECORD**. Every entity — a user, an order, a sensor reading, a memory, a document — is a record with a label and properties. From this single building block you can model any domain.

This skill guides you through **discovering your schema through conversation** rather than picking a fixed template. The output is always tailored to your use case.

## Prerequisites

- **RushDB MCP server** must be connected — it provides `createRecord`, `bulkCreateRecords`, `findRecords`, and all other tools used. Setup: `npx @rushdb/mcp-server` (requires `RUSHDB_API_KEY` env var). See https://docs.rushdb.com/mcp-server/quickstart
- If MCP tools are not available, tell the user to configure the MCP server first and link to the quickstart above.

---

## Schema Interview — 5 Questions

Run this interview before proposing any schema. Never guess or assume — ask first.

```
Q1  What are you building? (Describe in one sentence.)
Q2  What are the 3–5 main "things" you need to track?
     (e.g. "customers, orders, products" or "patients, treatments, providers")
Q3  How do these things connect to each other?
     (e.g. "customers place orders" or "doctors diagnose patients")
Q4  What questions will you most often ask of this data?
     (e.g. "find all open orders by customer" or "which patients have condition X")
Q5  Is there anything domain-specific that's NOT covered by what you described?
     (unique identifiers, external systems, compliance fields, etc.)
```

Use answers to Q1–Q2 to match the domain pattern below. Use Q3–Q5 to customise the template.

**If no domain pattern fits**, follow the [Custom Domain](#custom-domain-no-template-match) path instead — it derives the schema directly from the interview answers.

---

## Domain Pattern Library

Each pattern provides a starter set of labels, their key properties, and the core relationships. Always adapt to what the user actually described.

---

### E-Commerce / Retail

**Core labels:** `CUSTOMER`, `PRODUCT`, `ORDER`, `ORDER_ITEM`, `CATEGORY`, `REVIEW`

```json
// CUSTOMER
{ "customerId": "cust_001", "email": "alice@example.com", "tier": "vip",
  "createdAt": "2026-01-01T00:00:00Z", "isActive": true }

// PRODUCT
{ "sku": "WGT-PRO-001", "name": "Widget Pro", "price": 49.99,
  "category": "hardware", "inStock": true, "rating": 4.3 }

// ORDER
{ "orderId": "ord_001", "status": "shipped", "total": 149.97,
  "placedAt": "2026-06-01T10:00:00Z", "currency": "USD" }
```

**Relationships:** `CUSTOMER -[PLACED]→ ORDER`, `ORDER -[CONTAINS]→ ORDER_ITEM`, `ORDER_ITEM -[REFERS_TO]→ PRODUCT`, `PRODUCT -[BELONGS_TO]→ CATEGORY`

**Starter questions:** "Top VIP customers by order total", "Which products are low stock with pending orders?", "Orders placed in the last 30 days by status"

---

### Software Engineering / DevOps

**Core labels:** `REPOSITORY`, `ENGINEER`, `ISSUE`, `PULL_REQUEST`, `DEPLOYMENT`, `SERVICE`

```json
// REPOSITORY
{ "name": "api-gateway", "language": "TypeScript", "team": "platform",
  "isArchived": false, "defaultBranch": "main" }

// ISSUE
{ "title": "Fix memory leak in worker", "status": "open", "priority": "high",
  "type": "bug", "createdAt": "2026-06-01T00:00:00Z" }

// PULL_REQUEST
{ "title": "Refactor auth middleware", "state": "open", "reviewsRequired": 2,
  "author": "alice", "createdAt": "2026-06-01T00:00:00Z" }
```

**Relationships:** `REPOSITORY -[HAS_ISSUE]→ ISSUE`, `ENGINEER -[OPENED]→ PULL_REQUEST`, `PULL_REQUEST -[TARGETS]→ REPOSITORY`, `DEPLOYMENT -[DEPLOYS]→ SERVICE`

**Starter questions:** "All open high-priority bugs by repository", "PRs awaiting review for more than 3 days", "Deployments that triggered an incident"

---

### CRM / Sales

**Core labels:** `CONTACT`, `COMPANY`, `DEAL`, `ACTIVITY`, `PIPELINE_STAGE`

```json
// CONTACT
{ "name": "Alice Chen", "email": "alice@acme.com", "role": "VP Engineering",
  "source": "inbound", "createdAt": "2026-01-15T00:00:00Z" }

// DEAL
{ "name": "Acme Corp Enterprise", "value": 48000, "stage": "negotiation",
  "currency": "USD", "expectedCloseAt": "2026-07-31T00:00:00Z", "probability": 0.7 }

// ACTIVITY
{ "type": "call", "summary": "Discussed pricing", "durationMin": 30,
  "outcome": "positive", "occurredAt": "2026-06-01T14:00:00Z" }
```

**Relationships:** `CONTACT -[WORKS_AT]→ COMPANY`, `CONTACT -[INVOLVED_IN]→ DEAL`, `DEAL -[HAS_ACTIVITY]→ ACTIVITY`, `DEAL -[IN_STAGE]→ PIPELINE_STAGE`

**Starter questions:** "Deals in negotiation with >$20k value", "Contacts at companies with no activity in 30 days", "Pipeline value by stage"

---

### Product Management

**Core labels:** `FEATURE`, `EPIC`, `USER_PERSONA`, `SPRINT`, `FEEDBACK`, `METRIC`

```json
// FEATURE
{ "title": "CSV export", "status": "in-progress", "priority": "high",
  "effort": "M", "release": "2026-Q3", "requestCount": 47 }

// USER_PERSONA
{ "name": "Data Analyst", "segment": "enterprise", "painPoint": "manual data export",
  "frequency": "daily" }

// FEEDBACK
{ "source": "support", "summary": "Can't export large datasets", "sentiment": "negative",
  "votes": 23, "submittedAt": "2026-05-01T00:00:00Z" }
```

**Relationships:** `EPIC -[CONTAINS]→ FEATURE`, `FEATURE -[REQUESTED_BY]→ USER_PERSONA`, `FEEDBACK -[INFORMS]→ FEATURE`, `FEATURE -[TRACKED_BY]→ METRIC`

**Starter questions:** "High-priority features with > 20 feedback votes", "Features not in any sprint", "Feedback by persona segment"

---

### Healthcare / Clinical

**Core labels:** `PATIENT`, `PROVIDER`, `ENCOUNTER`, `DIAGNOSIS`, `MEDICATION`, `TREATMENT`

```json
// PATIENT
{ "patientId": "pt_001", "dateOfBirth": "1985-04-12T00:00:00Z", "gender": "F",
  "bloodType": "A+", "isActive": true }

// ENCOUNTER
{ "type": "outpatient", "reason": "annual checkup", "status": "completed",
  "occurredAt": "2026-05-15T09:00:00Z", "facilityId": "fac_001" }

// DIAGNOSIS
{ "code": "E11.9", "description": "Type 2 diabetes mellitus", "status": "active",
  "confirmedAt": "2026-05-15T00:00:00Z", "severity": "moderate" }
```

**Relationships:** `PATIENT -[HAD]→ ENCOUNTER`, `ENCOUNTER -[LED_TO]→ DIAGNOSIS`, `PROVIDER -[CONDUCTED]→ ENCOUNTER`, `DIAGNOSIS -[TREATED_WITH]→ MEDICATION`

**Starter questions:** "Patients with active chronic diagnoses", "Encounters per provider in the last quarter", "Medications prescribed for condition X"

---

### Financial Services / Fintech

**Core labels:** `CLIENT`, `ACCOUNT`, `TRANSACTION`, `PORTFOLIO`, `SECURITY`, `COMPLIANCE_EVENT`

```json
// ACCOUNT
{ "accountId": "acc_001", "type": "investment", "balance": 142500.00,
  "currency": "USD", "status": "active", "openedAt": "2020-03-01T00:00:00Z" }

// TRANSACTION
{ "txId": "tx_001", "type": "buy", "amount": 5000.00, "ticker": "AAPL",
  "quantity": 25, "executedAt": "2026-06-01T14:30:00Z", "status": "settled" }
```

**Relationships:** `CLIENT -[OWNS]→ ACCOUNT`, `ACCOUNT -[HAS_TRANSACTION]→ TRANSACTION`, `ACCOUNT -[HOLDS]→ SECURITY`, `CLIENT -[SUBJECT_TO]→ COMPLIANCE_EVENT`

**Starter questions:** "Accounts with balance over $100k", "Transactions flagged for compliance review", "Portfolio value by client tier"

---

### Content / CMS / Media

**Core labels:** `ARTICLE`, `AUTHOR`, `TAG`, `CATEGORY`, `MEDIA_ASSET`, `SERIES`

```json
// ARTICLE
{ "title": "Getting started with RushDB", "status": "published", "wordCount": 1850,
  "slug": "getting-started-rushdb", "publishedAt": "2026-06-01T09:00:00Z",
  "readTimeMin": 8 }

// AUTHOR
{ "name": "Alice Chen", "bio": "Engineer at RushDB", "handle": "alicechen",
  "isActive": true }
```

**Relationships:** `AUTHOR -[WROTE]→ ARTICLE`, `ARTICLE -[TAGGED_WITH]→ TAG`, `ARTICLE -[PART_OF]→ SERIES`, `ARTICLE -[USES]→ MEDIA_ASSET`

**Starter questions:** "Published articles by author this month", "Most used tags", "Articles in a series, ordered by position"

---

### AI Agent Memory

**Core labels:** `SESSION`, `MEMORY`, `DECISION`, `ENTITY`, `TOOL_CALL`

```json
// SESSION
{ "sessionId": "sess_abc123", "agentId": "agent_001", "topic": "architecture review",
  "startedAt": "2026-06-01T09:00:00Z", "isActive": true }

// MEMORY
{ "content": "User prefers TypeScript over Python for backend services",
  "type": "preference", "confidence": 0.9, "storedAt": "2026-06-01T09:15:00Z" }

// DECISION
{ "topic": "authentication", "decision": "Use Clerk for auth",
  "rationale": "Better Next.js DX", "decidedAt": "2026-06-01T09:30:00Z" }
```

**Relationships:** `SESSION -[PRODUCED]→ MEMORY`, `SESSION -[HAS_DECISION]→ DECISION`, `SESSION -[INVOKED]→ TOOL_CALL`, `MEMORY -[MENTIONS]→ ENTITY`

**Starter questions:** "All preferences remembered for a user", "Decisions made in sessions about topic X", "Tool calls that led to a decision"

> For the full agent memory skill, see `rushdb-agent-memory`.

---

### Project / Task Management

**Core labels:** `PROJECT`, `TASK`, `MILESTONE`, `MEMBER`, `COMMENT`, `LABEL`

```json
// PROJECT
{ "name": "API v2 launch", "status": "active", "startDate": "2026-05-01T00:00:00Z",
  "targetDate": "2026-08-31T00:00:00Z", "team": "platform" }

// TASK
{ "title": "Implement rate limiting", "status": "in-progress", "priority": "high",
  "dueAt": "2026-06-15T00:00:00Z", "storyPoints": 5 }
```

**Relationships:** `PROJECT -[HAS_TASK]→ TASK`, `MEMBER -[ASSIGNED_TO]→ TASK`, `TASK -[BLOCKED_BY]→ TASK`, `TASK -[PART_OF]→ MILESTONE`

**Starter questions:** "Overdue high-priority tasks by assignee", "Blocked tasks in current milestone", "Velocity by sprint (story points completed)"

---

### Scientific Research

**Core labels:** `RESEARCHER`, `PAPER`, `EXPERIMENT`, `DATASET`, `GRANT`, `INSTITUTION`

```json
// PAPER
{ "title": "Graph-based RAG for enterprise search", "status": "published",
  "doi": "10.1234/example", "citations": 42, "publishedAt": "2026-03-15T00:00:00Z" }

// EXPERIMENT
{ "name": "Baseline accuracy test", "status": "completed", "accuracy": 0.923,
  "modelVersion": "v2.1", "runAt": "2026-05-01T00:00:00Z" }
```

**Relationships:** `RESEARCHER -[AUTHORED]→ PAPER`, `PAPER -[USES]→ DATASET`, `RESEARCHER -[CONDUCTED]→ EXPERIMENT`, `GRANT -[FUNDS]→ PROJECT`

**Starter questions:** "Most cited papers in the last 2 years", "Experiments by status and accuracy", "Researchers affiliated with an institution"

---

## Custom Domain (No Template Match)

When the user's use case doesn't fit any pattern above, derive the schema from first principles using the interview answers. Follow these steps in order.

### Step 1 — Extract nouns (→ labels)

Every distinct "thing" the user mentioned in Q2 becomes a label candidate. Apply the rules:

- Is it a thing you store and query independently? → **Label**
- Is it just an attribute of another thing? → **Property**
- Is it a verb / action between two things? → **Relationship**

```
User said: "I track beehives, inspection visits, and the issues found during each visit"

Things        → BEEHIVE, INSPECTION, ISSUE
Attributes    → location, status, severity  (not separate labels)
Relationships → BEEHIVE -[HAD]→ INSPECTION, INSPECTION -[FOUND]→ ISSUE
```

### Step 2 — Assign properties per label

For each label, ask: _"What do I need to know about a single [thing]?"_

Properties to always consider:

- **Identity**: a unique ID (`beehiveId`, `inspectionId`) if coming from an external system
- **Status / lifecycle**: `status`, `isActive`, `phase` — almost every entity has one
- **Timestamps**: `createdAt`, `occurredAt`, `completedAt` — always ISO 8601 UTC
- **Owner / actor**: who created or is responsible for this record
- **Measurements / scores**: numbers specific to the domain (weight, temperature, count, score)
- **Free text**: `description`, `notes`, `summary` — add a vector index if you need semantic search

### Step 3 — Define relationships from Q3

Each verb phrase in the user's Q3 answer becomes a relationship. Name it in `UPPER_SNAKE_CASE` as a verb: `HAD`, `FOUND`, `BELONGS_TO`, `TRIGGERED`.

Directionality rule: the relationship flows from the "owner" to the "owned", the "actor" to the "target:

```
BEEHIVE -[HAD]→ INSPECTION       ✓  (beehive owns the inspection)
INSPECTION -[HAD]→ BEEHIVE       ✗  (wrong direction)
```

### Step 4 — Map Q4 queries to the schema

For each question the user listed in Q4, trace which labels and relationships it touches:

```
Q: "Which beehives had more than 3 issues in the last 90 days?"
→ Needs: BEEHIVE → INSPECTION → ISSUE
→ Filter on INSPECTION.occurredAt (last 90 days), count ISSUE per BEEHIVE
→ Confirms the relationship chain is correct
```

If a query requires data that isn't captured in any label or property yet, add it now.

### Step 5 — Synthesise the custom schema

Produce the schema summary, bootstrap payload, and starter queries using the labels and relationships you just derived, following the same output format as the domain templates.

**Worked example — Beekeeping:**

```
Labels:    BEEKEEPER, APIARY, BEEHIVE, INSPECTION, ISSUE
Relations: BEEKEEPER -[MANAGES]→ APIARY
           APIARY    -[CONTAINS]→ BEEHIVE
           BEEHIVE   -[HAD]→ INSPECTION
           INSPECTION-[FOUND]→ ISSUE
```

```json
// Bootstrap payload
{
  "label": "BEEKEEPER",
  "data": {
    "name": "Maria Torres",
    "email": "maria@bees.example.com",
    "licenseId": "BK-2024-001",
    "isActive": true,
    "APIARY": [
      {
        "name": "Riverside Apiary",
        "location": "Riverside, CA",
        "hiveCount": 12,
        "BEEHIVE": [
          {
            "hiveId": "hive_001",
            "queenYear": 2025,
            "status": "healthy",
            "INSPECTION": [
              {
                "occurredAt": "2026-05-20T08:00:00Z",
                "inspector": "Maria Torres",
                "honeyFrames": 8,
                "broodHealth": "good",
                "ISSUE": [{ "type": "varroa", "severity": "low", "resolved": false }]
              }
            ]
          }
        ]
      }
    ]
  }
}
```

```json
// "Beehives with unresolved issues in the last 90 days"
{
  "labels": ["BEEHIVE"],
  "where": {
    "INSPECTION": {
      "occurredAt": { "$gte": { "$subtract": ["$$NOW", { "$days": 90 }] } },
      "ISSUE": { "resolved": false }
    }
  }
}
```

---

## Output Format

After the interview, always produce three artifacts:

### 1. Schema Summary (Markdown table)

| Label      | Key Properties                           | Relationships Out       |
| ---------- | ---------------------------------------- | ----------------------- |
| `CUSTOMER` | `email`, `tier`, `isActive`, `createdAt` | `PLACED → ORDER`        |
| `ORDER`    | `orderId`, `status`, `total`, `placedAt` | `CONTAINS → ORDER_ITEM` |
| `PRODUCT`  | `sku`, `name`, `price`, `inStock`        | `BELONGS_TO → CATEGORY` |

### 2. Bootstrap Payload

A single `bulkCreateRecords` call with 2–3 sample records per label that seeds the graph and auto-creates relationships via nesting:

```json
{
  "label": "CUSTOMER",
  "data": {
    "customerId": "cust_001",
    "email": "alice@example.com",
    "tier": "vip",
    "isActive": true,
    "createdAt": "2026-06-01T00:00:00Z",
    "ORDER": [
      {
        "orderId": "ord_001",
        "status": "shipped",
        "total": 149.97,
        "placedAt": "2026-06-01T10:00:00Z",
        "ORDER_ITEM": [
          {
            "quantity": 3,
            "unitPrice": 49.99,
            "PRODUCT": [{ "sku": "WGT-PRO-001", "name": "Widget Pro", "price": 49.99 }]
          }
        ]
      }
    ]
  }
}
```

> Nesting creates records AND relationships automatically. No separate `attachRelation` calls needed for the initial bootstrap.

### 3. Starter Queries (3–5 examples)

Concrete `findRecords` calls that answer the user's Q4 questions. Show them what their data looks like in use:

```json
// "Find VIP customers with orders in the last 30 days"
{
  "labels": ["CUSTOMER"],
  "where": {
    "tier": "vip",
    "ORDER": {
      "placedAt": { "$gte": { "$subtract": ["$$NOW", { "$days": 30 }] } }
    }
  }
}
```

---

## Customisation Checklist

After matching a domain pattern, verify with the user:

- [ ] Are there any labels unique to their business that aren't in the template?
- [ ] Do they have external IDs (from a CRM, ERP, or SaaS tool) to store as properties?
- [ ] Are there compliance or audit fields required (e.g. `createdBy`, `updatedAt`, `deletedAt`)?
- [ ] Do they need vector/semantic search on any text fields? (add `description` or `summary` properties, enable vector index)
- [ ] Are there access-control dimensions to model (tenant, workspace, org)?

---

## Naming Rules (Quick Reference)

| Element      | Convention               | Example                   |
| ------------ | ------------------------ | ------------------------- |
| Label        | `UPPER_CASE`, singular   | `ORDER`, `BLOG_POST`      |
| Property     | `camelCase`              | `createdAt`, `isActive`   |
| Relationship | `UPPER_SNAKE_CASE`, verb | `HAS_ORDER`, `BELONGS_TO` |

Never use lowercase labels — `User` ≠ `USER`. RushDB is case-sensitive.

---

## After Bootstrap

Once the seed payload is in, run `getSchemaMarkdown()` to confirm the schema looks right, then share the result so the user can see their actual labels and properties as RushDB inferred them. From there, use `rushdb-query-builder` to build queries and `rushdb-data-modeling` for schema evolution guidance.
