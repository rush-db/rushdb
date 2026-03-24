---
sidebar_position: 0
title: Introduction
---

# TypeScript / JavaScript SDK

Push JSON, query by value or meaning, traverse graphs — from Node.js or the browser.

## Install

```bash
npm install @rushdb/javascript-sdk
# or: yarn add @rushdb/javascript-sdk  |  pnpm add @rushdb/javascript-sdk
```

## Connect

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')
```

Get your API token from the [RushDB Dashboard](https://app.rushdb.com/).

## First write

```typescript
// Nested objects become linked records automatically
await db.records.importJson({
  label: 'MOVIE',
  data: {
    title: 'Inception',
    rating: 8.8,
    genre: 'sci-fi',
    ACTOR: [
      { name: 'Leonardo DiCaprio', country: 'USA' },
      { name: 'Ken Watanabe',      country: 'Japan' }
    ]
  }
})
// Created: MOVIE → ACTOR × 2 (relationships wired automatically)
```

## First read

```typescript
const { data: movies, total } = await db.records.find({
  labels: ['MOVIE'],
  where: { rating: { $gte: 8 } },
  orderBy: { rating: 'desc' }
})
```

## Configuration

```typescript
const db = new RushDB('RUSHDB_API_KEY', {
  url: 'http://localhost:3000/api/v1', // or use host/port/protocol
  timeout: 5000                        // default: 30000ms
})
```

| Option | Default | Description |
|---|---|---|
| `url` | — | Full API URL (alternative to host/port/protocol) |
| `host` | — | Domain or IP |
| `port` | 80 / 443 | Port number |
| `protocol` | `https` | `http` or `https` |
| `timeout` | `30000` | Request timeout in ms |
| `httpClient` | — | Custom HTTP client — required for Edge / Cloudflare Workers |
| `logger` | — | Custom logging function |
| `options.allowForceDelete` | `false` | Must be `true` to delete all records without criteria (safety gate) |

## Namespaces

| Namespace | Use |
|---|---|
| `db.records` | Create, find, update, delete records |
| `db.relationships` | Attach and detach edges |
| `db.tx` | Transactions |
| `db.labels` | List labels and counts |
| `db.properties` | Inspect field names, types, value ranges |
| `db.ai` | Schema export + semantic search |
