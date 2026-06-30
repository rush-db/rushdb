---
sidebar_position: 0
title: Overview
---

# AI & Semantic Search

RushDB is a **self-aware memory layer for agents, humans, and apps**. It continuously understands its own structure — labels, fields, value distributions, relationships — and exposes that knowledge so agents can reason over real data without hallucinating schema details, and apps can retrieve semantically relevant context on demand.

The AI API covers three capabilities:

| Capability            | Description                                                                                                            |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Graph Schema**      | Self-describing schema discovery: label names, field types, value ranges, and the relationship map — always up to date |
| **Embedding Indexes** | Per-label vector policies that turn string properties into long-term semantic memory                                   |
| **Semantic Search**   | Cosine/euclidean similarity retrieval over indexed properties, for agents and apps alike                               |

---

## How it fits together

```
┌─────────────────────────────────────────────────────┐
│  Your data (records + relationships)                │
│                                                     │
│  BOOK { title: "...", description: "..." }          │
└────────────────────┬────────────────────────────────┘
                     │
         POST /api/v1/ai/indexes
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│  Embedding index policy                             │
│  label: BOOK  property: description  dims: 1536    │
│  sourceType: managed | external                     │
└────────────────────┬────────────────────────────────┘
                     │
      Backfill (managed) / inline vectors (external)
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│  Vector stored on VALUE relationship                │
│  rel._emb_managed_cosine_1536 = [0.1, 0.2, ...]    │
└────────────────────┬────────────────────────────────┘
                     │
          POST /api/v1/ai/search
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│  Records ranked by similarity score                 │
│  result.__score = 0.94  (cosine similarity)        │
└─────────────────────────────────────────────────────┘
```

---

## Quick links

| Topic                                                                                  | Description                                                                    |
| -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| [Schema](#graph-schema)                                                                | Schema discovery with `POST /api/v1/ai/schema/md` and `POST /api/v1/ai/schema` |
| [Indexing](/learn/reference/rest-api/ai-and-vectors/indexing)                          | Create and manage managed embedding indexes                                    |
| [Advanced Indexing — BYOV](/learn/reference/rest-api/ai-and-vectors/advanced-indexing) | Bring Your Own Vectors: external indexes, inline writes                        |
| [Semantic Search](/learn/reference/rest-api/ai-and-vectors/search)                     | Query by meaning with `POST /api/v1/ai/search`                                 |
| [Writing with Vectors](/learn/reference/rest-api/ai-and-vectors/write-with-vectors)    | Attach vectors at create / upsert / importJson time                            |
| [Agent Skills](#agent-skills)                                                          | Installable skills that teach any compatible agent to use RushDB               |

---

## Graph Schema

The schema endpoints expose a live snapshot of your database structure — without any manual schema definitions.

### Get Schema (Markdown)

`POST /api/v1/ai/schema/md`

Returns the full schema as compact Markdown — the **recommended format for LLM context injection**: token-efficient, human-readable, and ready to paste into a system prompt or tool result.

#### Request Body

| Field    | Type             | Required | Description                                                                  |
| -------- | ---------------- | -------- | ---------------------------------------------------------------------------- |
| `labels` | array of strings | no       | Restrict output to specific labels. Omit (or pass `[]`) for the full schema. |
| `force`  | boolean          | no       | Bypass the 1-hour cache and force a full recalculation.                      |

#### Example Request

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/schema/md \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{}'
```

#### Example Response

```text
# Graph Schema

## Labels

| Label     | Count |
|-----------|------:|
| `Order`   |  1840 |
| `User`    |   312 |
| `Product` |    95 |

---

## `Order` (1840 records)

### Properties

| Property    | Type     | Values / Range                         | Semantic Search                  |
|-------------|----------|----------------------------------------|----------------------------------|
| `status`    | string   | `pending`, `paid`, `shipped` (+2 more) | —                                |
| `total`     | number   | `4.99`..`2499.00`                      | —                                |
| `name`      | string   | `Widget A`, `Widget B` (+8 more)       | `managed` cosine 1536d [ready]   |
| `createdAt` | datetime | `2024-01-03`..`2026-02-27`             | —                                |

### Relationships

| Type        | Direction | Other Label |
|-------------|-----------|-------------|
| `PLACED_BY` | out       | `User`      |
| `CONTAINS`  | out       | `Product`   |
```

#### Filtered request (single label)

```bash
curl -X POST https://api.rushdb.com/api/v1/ai/schema/md \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -d '{"labels": ["Order"]}'
```

Returns only the `Order` section. The underlying cache still covers the full schema — filtering is applied in-memory.

---

### Get Schema (JSON)

`POST /api/v1/ai/schema`

Returns the same schema as a structured JSON array. Each element describes one label.

#### Request Body

| Field    | Type             | Required | Description                                             |
| -------- | ---------------- | -------- | ------------------------------------------------------- |
| `labels` | array of strings | no       | Restrict to specific labels. Omit for the full schema.  |
| `force`  | boolean          | no       | Bypass the 1-hour cache and force a full recalculation. |

#### Response Schema

```json
[
  {
    "label": "Order",
    "count": 1840,
    "properties": [
      { "id": "prop_abc123", "name": "status", "type": "string", "values": ["pending", "paid", "shipped"] },
      { "id": "prop_def456", "name": "total", "type": "number", "min": 4.99, "max": 2499.0 },
      {
        "id": "prop_xyz789",
        "name": "name",
        "type": "string",
        "values": ["Widget A", "Widget B"],
        "vectorIndexes": [
          {
            "id": "idx_001",
            "sourceType": "managed",
            "similarityFunction": "cosine",
            "dimensions": 1536,
            "status": "ready",
            "modelKey": "text-embedding-3-small"
          }
        ]
      }
    ],
    "relationships": [
      { "label": "User", "type": "PLACED_BY", "direction": "out" },
      { "label": "Product", "type": "CONTAINS", "direction": "out" }
    ]
  }
]
```

- `properties[].id` — pass to `GET /api/v1/properties/:id/values` to enumerate all distinct values
- `properties[].values` — up to 10 samples (string/boolean only)
- `properties[].min` / `.max` — range info (number/datetime only)
- `properties[].vectorIndexes` — non-empty when one or more embedding indexes exist for this property; each entry has `id`, `sourceType`, `similarityFunction`, `dimensions`, `status`, and `modelKey`. Use `POST /api/v1/ai/search` to query semantically.
- `relationships[].direction` — `out` = this label is source; `in` = this label is target
- `relationships[].count` — number of observed edges for that relationship pattern
- `relationships[].properties` — optional summaries of user-defined edge properties discovered on that pattern

---

:::note Caching
Both endpoints share a **1-hour cache** on the ProjectNode. First call after TTL expiry triggers a full graph scan; all subsequent calls within the hour are instant. Pass `"force": true` in the request body to bypass the cache and trigger an immediate recalculation.
:::

:::tip Agent quickstart
Call `POST /api/v1/ai/schema/md` first in every AI session. Without it, models will hallucinate label and field names.
:::

---

## Agent Skills

`@rushdb/skills` is a collection of [Agent Skills](https://agentskills.io) — installable instructions that teach any skills-compatible AI agent (Claude, GitHub Copilot, Cursor, Windsurf, and others) to use RushDB efficiently, without manual system prompt engineering.

```bash
npx skills add rush-db/rushdb --path packages/skills
```

| Skill                    | What it teaches                                                                                                                                                   |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rushdb-query-builder`   | Discovery-first workflow, SearchQuery syntax, aggregation, relationship traversal, and semantic search                                                            |
| `rushdb-agent-memory`    | Using RushDB as persistent structured memory — store, link, and semantically recall sessions, decisions, and entities                                             |
| `rushdb-data-modeling`   | LMPG model design, label/property naming conventions, nested JSON import, and schema evolution                                                                    |
| `rushdb-faceted-search`  | Build faceted filter UIs — discover properties and types, enumerate distinct values, map to widgets, assemble a live `where` clause                               |
| `rushdb-domain-template` | Design a tailored schema for any domain through guided conversation — interview, entity/relationship mapping, bootstrap payload, and 10 built-in domain templates |

Each skill bundles a `SKILL.md` with concise instructions and optional reference files (like the full SearchQuery spec) that the agent loads on demand.

:::note MCP server vs. Agent Skills
The [MCP server](/connect/mcp) gives agents direct tool access to RushDB at runtime. Agent Skills teach agents _how_ to use those tools correctly — they complement each other.
:::
