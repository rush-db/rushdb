<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB — JavaScript & TypeScript SDK

### The memory layer for AI agents and apps.

Push any JSON. Get graph relationships and vector search — automatically.
No schema. No pipeline. No glue code.

[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![NPM Downloads](https://img.shields.io/npm/dw/%40rushdb%2Fjavascript-sdk)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![Bundle size](https://img.shields.io/bundlephobia/minzip/%40rushdb%2Fjavascript-sdk)](https://bundlephobia.com/package/@rushdb/javascript-sdk)
[![License](https://img.shields.io/npm/l/%40rushdb%2Fjavascript-sdk)](#license)
[![Made with Node](https://img.shields.io/badge/dynamic/json?label=node&query=%24.engines%5B%22node%22%5D&url=https%3A%2F%2Fraw.githubusercontent.com%2Frush-db%2Frushdb%2Fmain%2Fpackage.json)](https://nodejs.org)

[🌐 Website](https://rushdb.com) • [📖 Documentation](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 Examples](https://github.com/rush-db/examples)

</div>

---

## Why RushDB

Agents need memory. Apps need connected data. The standard answer involves multiple databases, schema design, and an embedding pipeline before you write a single useful line of business logic.

RushDB skips all of that:

- **Managed embeddings** — mark a property for indexing once; every write is auto-embedded server-side
- **Graph auto-structured** — nested JSON becomes a traversable graph; no manual edge creation
- **Semantic + graph in one query** — filter by relationships, rank by meaning, compute metrics — one call (use select/groupBy)
- **Zero schema** — push any shape; RushDB infers types and links records
- **6.9KB gzipped** — zero runtime dependencies
- **Isomorphic** — Node.js and browser

---

## Installation

```bash
npm install @rushdb/javascript-sdk
# yarn add @rushdb/javascript-sdk
# pnpm add @rushdb/javascript-sdk
```

---

## Agent memory in 3 lines

Get an API key at [app.rushdb.com](https://app.rushdb.com).

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

// 1. One-time: index a property for semantic search
await db.ai.indexes.create({ label: 'MEMORY', propertyName: 'output' })

// 2. Store — no embedder needed, server handles it
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

// 3. Recall by meaning, scoped by graph
const memories = await db.ai.search({
  labels: ['MEMORY'],
  propertyName: 'output',
  query: 'what did we decide about Q4?',
  where: { agent_id: 'agent-42' },
  limit: 10
})
```

---

## Graph traversal

```typescript
// Push nested JSON — relationships created automatically
await db.records.importJson({
  label: 'COMPANY',
  payload: {
    name: 'Acme Corp',
    DEPARTMENT: [
      {
        name: 'Engineering',
        EMPLOYEE: [
          {
            name: 'Alice',
            role: 'Staff Engineer'
          }
        ]
      }
    ]
  }
})

// Traverse the auto-created graph
const engineers = await db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    role: { $contains: 'Engineer' },
    DEPARTMENT: { COMPANY: { name: 'Acme Corp' } }
  }
})

// Constrain by relationship type and direction
const authoredPosts = await db.records.find({
  labels: ['USER'],
  where: {
    POST: {
      $relation: { type: 'AUTHORED', direction: 'out' },
      title: { $contains: 'graph' }
    }
  },
  limit: 10
})

// Manage relationships explicitly
const company = await db.records.findUniq({
  labels: ['COMPANY'],
  where: { name: 'Acme Corp' }
})
await company.attach(engineers, { type: 'EMPLOYS' })
```

---

## Query API

One JSON structure works for record queries, aggregations, and vector search:

```typescript
const result = await db.records.find({
  labels: ['TRANSACTION'],
  where: {
    status: 'posted',
    amount: { $gte: 100 }
  },
  select: {
    total: { $sum: '$record.amount' }
  },
  groupBy: ['$record.category'],
  // Legacy: The aggregate clause is deprecated and should only be used for vector similarity until select supports it.
  orderBy: { amount: 'desc' },
  limit: 50
})
```

`where` is resource-local:

- `db.records.find().where` filters Records.
- `db.labels.find().where` and `db.properties.find().where` filter Records before returning matching labels/properties.
- `db.relationships.find().where` filters relationship edges. Use `source` and `target` for endpoint Record predicates.

```typescript
const relationships = await db.relationships.find({
  source: { labels: ['USER'], where: { status: 'active' } },
  target: { labels: ['ORDER'] },
  where: { type: 'ORDERED', confidence: { $gte: 0.8 } }
})
```

---

## SDK configuration

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY', {
  // Override for self-hosted or staging
  url: 'http://your-rushdb-server.com/api/v1',
  timeout: 30000
})
```

Key options:

- `url` — full API URL (default: `https://api.rushdb.com/api/v1`)
- `host`, `port`, `protocol` — alternative to `url`
- `timeout` — request timeout in ms (default: 10000)
- `httpClient` — custom HTTP client for advanced use
- `options.allowForceDelete` — allow deleting all records without criteria (default: false)

Full config reference: [docs.rushdb.com/typescript-sdk/introduction#sdk-configuration-options](https://docs.rushdb.com/typescript-sdk/introduction#sdk-configuration-options)

---

## Documentation

[docs.rushdb.com/typescript-sdk](https://docs.rushdb.com/typescript-sdk/introduction) — full API reference, examples, transactions, CSV import, and more.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Issues and PRs welcome.
