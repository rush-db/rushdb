<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### The memory layer for AI agents and apps.

Push any JSON. Your agent gets a live, queryable schema, graph relationships, and semantic search — inferred automatically.
No pipeline. No separate stores. No schema to design.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 Website](https://rushdb.com) • [📖 Documentation](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 Examples](https://github.com/rush-db/examples)

**English** • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## The problem

Your agent needs memory. The standard answer is three databases: Redis for key-value, a vector store for semantic search, a graph DB for relationships — plus glue code to keep them in sync.

RushDB replaces all three. Push JSON once. Query it with graph traversal, semantic search, or both — in one call.

| Without RushDB                            | With RushDB                            |
| ----------------------------------------- | -------------------------------------- |
| Redis + Pinecone + Neo4j + glue code      | One API                                |
| Design schema → write migrations → repeat | Push any shape, no schema required     |
| Separate embedding pipeline               | Managed embeddings, server-side        |
| Hand-craft relationship edges             | Auto-detected from your data structure |

---

## Quick start

Two paths depending on your setup:

- **Cloud** — Managed, free tier, running in 30 seconds. [Get API key →](https://app.rushdb.com)
- **Self-host** — Docker + your own Neo4j instance. [Jump to Self-hosting →](#self-hosting)

### Cloud path

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### Store and recall agent memory

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

// One-time: tell RushDB to auto-embed 'output' on every write
await db.ai.indexes.create({ label: 'MEMORY', propertyName: 'output' })

// Store an agent action — no embedder, no vectors array
await db.records.create({
  label: 'MEMORY',
  data: {
    agent_id: 'agent-42',
    session_id: 'sess-001',
    action: 'summarized',
    topic: 'Q4 results',
    output: summaryText
  }
})

// Recall by meaning — graph filter + semantic search in one call
const memories = await db.records.vectorSearch({
  labels: ['MEMORY'],
  propertyName: 'output',
  query: 'what did we decide about Q4?',
  where: { agent_id: 'agent-42' },
  limit: 10
})
```

```python
from rushdb import RushDB

db = RushDB('RUSHDB_API_KEY')

# Store — graph links sessions, actions, and context automatically
db.records.create(
    label='MEMORY',
    data={
        'agent_id': 'agent-42',
        'action': 'summarized',
        'topic': 'Q4 results',
        'output': summary_text,
    },
)

# Recall — traverse relationships and filter by meaning
results = db.records.find({
    'labels': ['MEMORY'],
    'where': {
        'agent_id': 'agent-42',
        'topic': {'$contains': 'Q4'},
    },
    'limit': 10,
})
```

---

## Working with your data

### Import nested JSON

Push any JSON shape. Nested objects and arrays of objects become linked records — labels, types, and relationships are inferred on write. No schema, no migration step.

```typescript
await db.records.importJson({
  label: 'COMPANY',
  data: {
    name: 'Acme Corp',
    DEPARTMENT: [
      {
        name: 'Engineering',
        budget: 2_000_000,
        EMPLOYEE: [
          { name: 'Alice', role: 'Staff Engineer', salary: 210_000 },
          { name: 'Bob', role: 'Engineer', salary: 160_000 }
        ]
      }
    ]
  }
})
```

Each nested key (`DEPARTMENT`, `EMPLOYEE`) becomes a label, each object a record, and containment a relationship — all created automatically.

### Import CSV

```typescript
const csv = `name,email,department
Alice,alice@acme.co,Engineering
Bob,bob@acme.co,Sales`

await db.records.importCsv({
  label: 'EMPLOYEE',
  data: csv,
  options: { suggestTypes: true, skipEmptyValues: true },
  parseConfig: { header: true }
})
```

`suggestTypes` infers numbers, booleans, and dates from strings; `skipEmptyValues` treats blank cells as unset instead of storing empty values (`0` and `false` are kept).

### Traverse the graph

Filter root records by conditions on their _related_ records — arbitrarily deep — in a single query. Related labels go **inside** `where`, not in `labels`:

```typescript
// Engineers in Acme's Engineering department
const engineers = await db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    role: { $contains: 'Engineer' },
    DEPARTMENT: {
      name: 'Engineering',
      COMPANY: { name: 'Acme Corp' }
    }
  }
})
```

### Analytical queries (aggregate & group by)

`select` shapes the output with aggregations (`$sum`, `$avg`, `$count`, `$min`, `$max`); `groupBy` controls the dimensions. Don't add `limit` to an aggregation — it would scan only the first N records and skew the totals.

```typescript
// Portfolio KPIs across ALL projects → one row
const kpis = await db.records.find({
  labels: ['PROJECT'],
  select: {
    totalBudget: { $sum: '$record.budget' },
    avgBudget: { $avg: '$record.budget', $precision: 2 },
    projectCount: { $count: '*' }
  },
  groupBy: ['totalBudget', 'avgBudget', 'projectCount'],
  orderBy: { totalBudget: 'asc' } // late-ordering → aggregates the full dataset
})
// → [{ totalBudget: 18_750_000, avgBudget: 568181.82, projectCount: 33 }]

// Breakdown by dimension → one row per status
const byStatus = await db.records.find({
  labels: ['PROJECT'],
  select: { count: { $count: '*' }, avgBudget: { $avg: '$record.budget', $precision: 2 } },
  groupBy: ['$record.status'],
  orderBy: { count: 'desc' }
})
// → [{ status: 'active', count: 18, avgBudget: 612000 }, { status: 'paused', count: 9, ... }]
```

Aggregations compose with traversal — e.g. headcount and payroll per department:

```typescript
const payroll = await db.records.find({
  labels: ['DEPARTMENT'],
  where: { EMPLOYEE: { $alias: '$emp' } }, // alias the related node for use in select
  select: {
    headcount: { $count: '*' },
    payroll: { $sum: '$emp.salary' }
  },
  groupBy: ['$record.name'],
  orderBy: { payroll: 'desc' }
})
// → [{ name: 'Engineering', headcount: 2, payroll: 370000 }, ...]
```

---

## Connect to Claude, Cursor, or any MCP client

RushDB ships an MCP server. Your agent gets persistent, structured memory — out of the box.

```json
{
  "mcpServers": {
    "rushdb": {
      "command": "npx",
      "args": ["@rushdb/mcp-server"],
      "env": {
        "RUSHDB_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

Place this in your Claude Desktop, Cursor, or Windsurf MCP config. The agent can now create records, search by meaning, traverse relationships, and introspect the schema — all via natural language.

---

## What's in the box

| Capability                      | What it means                                                                                                                                       |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Managed embeddings**          | Index any string property once — every write is auto-embedded server-side                                                                           |
| **Graph + vector in one query** | Semantic similarity and relationship traversal compose in a single call                                                                             |
| **Zero schema**                 | Push any JSON shape. RushDB infers types, creates properties, links records                                                                         |
| **Inferred schema**             | Properties become first-class nodes — types, labels, and relationships are discovered on write, so a queryable schema builds itself as data arrives |
| **ACID transactions**           | Concurrent agents don't corrupt shared memory. Neo4j under the hood                                                                                 |
| **Self-describing**             | Agents introspect the inferred schema — labels, properties, value ranges — to know what they can safely query                                       |
| **MCP-native**                  | Full MCP server with discovery-first query prompt built in                                                                                          |
| **Agent Skills**                | Installable `@rushdb/skills` package — teach any skills-compatible agent to query, model, and remember with RushDB in one command                   |
| **Unified query API**           | One JSON shape for graph, vector, aggregation, and introspection                                                                                    |
| **Self-host or cloud**          | Docker + your Neo4j, or managed cloud. Full data ownership                                                                                          |

---

## Use cases

| Use case                    | What RushDB replaces            | Key API                                                       |
| --------------------------- | ------------------------------- | ------------------------------------------------------------- |
| **Agent memory**            | Redis + vector store + graph DB | `db.records.vectorSearch({ query, where: { agent_id } })`     |
| **RAG with context**        | Flat vector store               | `db.records.find({ where, labels })` + relationship traversal |
| **Schema-free apps**        | Postgres + migrations + ETL     | `db.records.importJson(nestedJson)`                           |
| **Connected data products** | Multiple joined services        | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })` |

---

## Self-hosting

> **Self-host path** — run RushDB on your own infrastructure. Requires Neo4j 2026.01.4+ with APOC plugin.

```yaml
# docker-compose.yml
version: '3.8'
services:
  rushdb:
    image: rushdb/platform
    ports:
      - '3000:3000'
    environment:
      - NEO4J_URL=neo4j+s://your-instance.neo4j.io
      - NEO4J_USERNAME=neo4j
      - NEO4J_PASSWORD=password
      - RUSHDB_AES_256_ENCRYPTION_KEY=32-char-key-here
      - RUSHDB_LOGIN=admin
      - RUSHDB_PASSWORD=secure-password
      # Optional: managed continuous-sync connectors via synx
      - RUSHDB_BASE_URL=https://rushdb.example.com
      - RUSHDB_SYNX_CONTROL_TOKEN=long-random-shared-token
      - RUSHDB_SYNX_DESTINATION_API_KEY=internal-write-api-key
```

<details>
<summary>Full environment variables</summary>

| Name                              | Description                                    | Required   | Default  |
| --------------------------------- | ---------------------------------------------- | ---------- | -------- |
| `NEO4J_URL`                       | Neo4j connection URL                           | yes        | —        |
| `NEO4J_USERNAME`                  | Neo4j username                                 | yes        | neo4j    |
| `NEO4J_PASSWORD`                  | Neo4j password                                 | yes        | —        |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | 32-char key for API token encryption           | yes (prod) | —        |
| `RUSHDB_PORT`                     | HTTP port                                      | no         | 3000     |
| `RUSHDB_LOGIN`                    | Admin login                                    | no         | admin    |
| `RUSHDB_PASSWORD`                 | Admin password                                 | no         | password |
| `RUSHDB_BASE_URL`                 | Public/base API URL for synx assignments       | no         | —        |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | Internal token for managed synx workers        | no         | —        |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | Internal write key for synx destination writes | no         | —        |

Managed synx workers run as daemons. They poll for runnable connectors, renew connector leases, release leases on graceful shutdown, and let platform/core reclaim expired leases after crashes.

</details>

<details>
<summary>Local development (bundled Neo4j)</summary>

```yaml
version: '3.8'
services:
  rushdb:
    image: rushdb/platform
    depends_on:
      neo4j:
        condition: service_healthy
    ports:
      - '3000:3000'
    environment:
      - NEO4J_URL=bolt://neo4j
      - NEO4J_USERNAME=neo4j
      - NEO4J_PASSWORD=password
  neo4j:
    image: neo4j:2026.01.4
    healthcheck:
      test: ['CMD-SHELL', 'wget --no-verbose --tries=1 --spider localhost:7474 || exit 1']
      interval: 5s
      retries: 30
      start_period: 10s
    ports:
      - '7474:7474'
      - '7687:7687'
    environment:
      - NEO4J_ACCEPT_LICENSE_AGREEMENT=yes
      - NEO4J_AUTH=neo4j/password
    volumes:
      - ./neo4j-plugins:/var/lib/neo4j/plugins
```

</details>

```bash
# Manage users via CLI
rushdb create-user admin@example.com securepassword123
rushdb update-password admin@example.com newsecurepassword456
```

<details>
<summary>Architecture: how RushDB structures data (LMPG)</summary>

RushDB uses a **Labeled Meta Property Graph (LMPG)** model. Properties are elevated to first-class graph nodes ("HyperProperties") — not just key-value pairs attached to records.

This means:

- **Schema without upfront design** — because properties are graph nodes, the schema is _inferred from your data, not designed_: labels, types, value ranges, and relationship topology are discovered on write and queryable immediately — no manual schema modeling, no RDF/OWL toolchain
- **Auto-detected relationships** — records sharing properties get linked without hand-crafting edges
- **Schema introspection** — agents can enumerate labels, property types, value ranges, and relationship topology in one query
- **Soft constraints** — type cohesion scoring, cardinality tracking, and vector dimension enforcement without rigid upfront schemas
- **Unified query surface** — the same filter expression works across records, labels, properties, and relationships

One SearchQuery retrieves multiple perspectives simultaneously (records + property stats + aggregations), avoiding the N+1 inspection pattern common in separate-system architectures.

[Read the full LMPG architecture post →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## Documentation

| Topic                    | Link                                                           |
| ------------------------ | -------------------------------------------------------------- |
| Quick Tutorial           | https://docs.rushdb.com/get-started/quick-tutorial             |
| Vector / Semantic Search | https://docs.rushdb.com/concepts/search/where#vector-operators |
| Filtering & Traversal    | https://docs.rushdb.com/concepts/search/where                  |
| Grouping & Aggregations  | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK           | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK               | https://docs.rushdb.com/python-sdk/introduction                |
| REST API                 | https://docs.rushdb.com/rest-api/introduction                  |
| MCP Server               | packages/mcp-server/README.md                                  |
| Agent Skills             | packages/skills/README.md                                      |

---

## When not to use RushDB

- You need sub-millisecond latency at very high write throughput — RushDB is built on Neo4j, which prioritises consistency and query expressiveness over raw write speed.
- You only need flat key-value storage with no relationships or semantic search — a simpler store will be lighter.
- You need a rigid relational schema enforced at the database level — RushDB is deliberately schema-free.

---

## Contributing

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for full guidelines. Issues and PRs welcome.

---

## License

| Path                    | License             |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

Need something not supported yet? Open an issue — design discussions are welcome.
