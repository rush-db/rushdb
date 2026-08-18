# RushDB Agent Skills

Agent Skills that teach AI agents to use RushDB efficiently — querying, data modeling, and persistent memory.

Compatible with Claude, GitHub Copilot, Cursor, Windsurf, and any [Agent Skills](https://agentskills.io)-compatible client.

---

## Install

```bash
npx skills add rush-db/rushdb --path packages/skills
```

Or install from npm:

```bash
npm install @rushdb/skills
```

---

## Available Skills

| Skill                                               | What it teaches                                                                                                              |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| [`rushdb-query-builder`](#rushdb-query-builder)     | Discovery-first workflow, SearchQuery syntax, aggregation, relationship traversal                                            |
| [`rushdb-agent-memory`](#rushdb-agent-memory)       | Route memory across native providers, MCP, and custom harnesses without duplicate writes                                     |
| [`rushdb-memory-adapter`](#rushdb-memory-adapter)   | Build and validate lifecycle-aware RushDB integrations for agent runtimes                                                    |
| [`rushdb-data-modeling`](#rushdb-data-modeling)     | LMPG model, label/property/relationship design, nested JSON import                                                           |
| [`rushdb-faceted-search`](#rushdb-faceted-search)   | Build faceted filter UIs — discover properties, enumerate values, map to widgets, assemble `where`                           |
| [`rushdb-domain-template`](#rushdb-domain-template) | Design a tailored schema for any domain through guided conversation — interview → labels + relationships + bootstrap payload |

---

### `rushdb-query-builder`

Teaches the mandatory 3-step workflow for querying RushDB: schema discovery → intent classification → query construction. Covers the full SearchQuery shape — `where` filters, `aggregate` functions, `groupBy` modes, relationship traversal, datetime operators, and vector similarity.

Includes a bundled reference file (`references/search-query-spec.md`) with the complete operator reference, syntax rules, and annotated examples loaded on demand.

**Triggers when an agent needs to:**

- List, filter, or search records
- Count, sum, average, or group data
- Traverse relationships between record types
- Run semantic/vector similarity search
- Build any `findRecords` query

---

### `rushdb-agent-memory`

Teaches how to select and operate the correct RushDB memory layer: native OpenClaw/Hermes lifecycle integration, native + MCP, MCP only, or a custom harness. It routes each read and write to one owner, enforces canonical scope boundaries, and prevents duplicate turn persistence.

Includes references for integration-mode selection, AgentMemoryEvent v1, host capabilities, and operational/domain memory patterns.

**Triggers when an agent needs to:**

- Store session data or conversation context
- Recall past decisions or prior context
- Build an entity graph that survives across sessions
- Search memory by meaning (semantic recall)
- Associate memories via relationships

---

### `rushdb-memory-adapter`

Guides contributors through building native RushDB memory support for another agent runtime: inspect exact lifecycle hooks, derive trusted authorization scope, reuse AgentMemoryEvent v1, implement fail-open recall and durable outbox delivery, and validate lifecycle conformance.

**Triggers when an agent needs to:**

- Implement or review an agent memory plugin/provider
- Map prompt, turn, compaction, session, and shutdown hooks
- Enforce participant, profile, sandbox, and subagent isolation
- Add deterministic idempotency, retries, replay, and recent-write fallback
- Validate a runtime adapter against the shared contract

---

### `rushdb-data-modeling`

Teaches RushDB's property-centric LMPG (Labels, Multi-Properties, Graph) model: label naming conventions, property types, relationship design, how nested JSON auto-creates relationships on import, and schema evolution patterns.

**Triggers when an agent needs to:**

- Design a schema or data model
- Choose labels and property names
- Understand how nested JSON is imported
- Plan relationship structure
- Evolve or migrate an existing schema

---

### `rushdb-faceted-search`

Teaches the full workflow for generating faceted filter UIs: call `getSchema` for structured property metadata (including `id` values), call `propertyValues` to enumerate distinct options per property, map each property type to the right UI widget (checkbox list, range slider, date picker, toggle), and assemble a live `where` clause as filters change.

Covers the eager/lazy loading split, context-aware vs. original value enumeration, the two-hook pattern used in real RushDB apps, active filter chips, and full reset.

**Triggers when an agent needs to:**

- Generate a filter sidebar, faceted search UI, or drill-down panel
- Enumerate available values for a property
- Map property types to UI controls
- Build a `where` clause that updates as the user selects filters
- Generate `useProperties` / `usePropertyValues` style hooks

---

### `rushdb-domain-template`

Guides users through designing a tailored RushDB schema via a structured interview (5 questions), then outputs a schema summary, a ready-to-run `bulkCreateRecords` bootstrap payload, and starter queries. Includes compact template cards for 10 common domains (e-commerce, SaaS/DevOps, CRM, healthcare, fintech, content, agent memory, project management, scientific research, and product management) — all adapted to RushDB's record-centric LMPG model.

**Triggers when a user:**

- Is starting a new project and doesn't know what labels to define
- Asks "what records should I create for X?"
- Wants to get up and running quickly with a known domain
- Needs a starting point they can customise rather than designing from scratch

---

## Skill Structure

Each skill follows the [Agent Skills](https://agentskills.io) format:

```
skills/<skill-name>/
├── SKILL.md          # Required: YAML frontmatter + instructions
├── agents/           # Optional product-facing metadata
├── references/       # Optional detailed guidance loaded on demand
└── scripts/          # Optional deterministic helpers
```

---

## License

Apache 2.0
