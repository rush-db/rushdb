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

| Skill | What it teaches |
|---|---|
| [`rushdb-query-builder`](#rushdb-query-builder) | Discovery-first workflow, SearchQuery syntax, aggregation, relationship traversal |
| [`rushdb-agent-memory`](#rushdb-agent-memory) | Using RushDB as persistent structured memory for AI agents |
| [`rushdb-data-modeling`](#rushdb-data-modeling) | LMPG model, label/property/relationship design, nested JSON import |
| [`rushdb-faceted-search`](#rushdb-faceted-search) | Build faceted filter UIs — discover properties, enumerate values, map to widgets, assemble `where` |

---

### `rushdb-query-builder`

Teaches the mandatory 3-step workflow for querying RushDB: ontology discovery → intent classification → query construction. Covers the full SearchQuery shape — `where` filters, `aggregate` functions, `groupBy` modes, relationship traversal, datetime operators, and vector similarity.

Includes a bundled reference file (`references/search-query-spec.md`) with the complete operator reference, syntax rules, and annotated examples loaded on demand.

**Triggers when an agent needs to:**
- List, filter, or search records
- Count, sum, average, or group data
- Traverse relationships between record types
- Run semantic/vector similarity search
- Build any `findRecords` query

---

### `rushdb-agent-memory`

Teaches how to use RushDB as a drop-in persistent memory layer for AI agents — replacing separate vector DB, key-value store, and graph systems with a single ACID-safe, semantically searchable graph.

Includes a reference file (`references/memory-patterns.md`) with example JSON structures for sessions, decisions, and entities.

**Triggers when an agent needs to:**
- Store session data or conversation context
- Recall past decisions or prior context
- Build an entity graph that survives across sessions
- Search memory by meaning (semantic recall)
- Associate memories via relationships

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

Teaches the full workflow for generating faceted filter UIs: call `getOntology` for structured property metadata (including `id` values), call `propertyValues` to enumerate distinct options per property, map each property type to the right UI widget (checkbox list, range slider, date picker, toggle), and assemble a live `where` clause as filters change.

Covers the eager/lazy loading split, context-aware vs. original value enumeration, the two-hook pattern used in real RushDB apps, active filter chips, and full reset.

**Triggers when an agent needs to:**
- Generate a filter sidebar, faceted search UI, or drill-down panel
- Enumerate available values for a property
- Map property types to UI controls
- Build a `where` clause that updates as the user selects filters
- Generate `useProperties` / `usePropertyValues` style hooks

---

## Skill Structure

Each skill follows the [Agent Skills](https://agentskills.io) format:

```
skills/<skill-name>/
├── SKILL.md          # Required: YAML frontmatter + instructions
├── references/       # Optional: large reference docs loaded on demand
└── scripts/          # Optional: executable helpers (not used here)
```

---

## License

Apache 2.0
