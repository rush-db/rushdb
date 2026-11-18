<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="RushDB logo" width="96">

# RushDB

<sub>The database for AI</sub>

**Graph, vector and analytics on the same records. Every write keeps a live schema of labels, fields and values, so agents query real fields instead of guessing.**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[Docs](https://docs.rushdb.com) • [Quickstart](https://docs.rushdb.com/engine/getting-started/quickstart/) • [Discussions](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

**English** • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

Bring data in the way you have it: nested JSON, CSV, SQL `INSERT`, Cypher or
Mongo-style writes, or change streams from your databases. RushDB stores it as
typed records linked into a graph, and the same records answer relationship
queries, vector and full-text search, and aggregations, so there is no second
store to keep in sync. Every write also updates a catalog of the labels,
fields, types and values that exist, so an agent queries real fields. Connect
over HTTP, the PostgreSQL wire protocol, Bolt, gRPC or MCP, from the SDKs or
any PostgreSQL client. One Rust binary, Apache-2.0.

| Graph | Vector and full-text search | Analytics |
|---|---|---|
| Nested JSON becomes linked records. Filter across relationships and follow typed, directed relationships in one query. | Vector (exact and HNSW), BM25 full-text and hybrid search, filtered by the same graph predicates. | Count, sum, average, min and max with `groupBy`, or plain SQL with `GROUP BY` over the PostgreSQL wire protocol. |

## Quickstart

```bash
curl -fsSL https://rushdb.com/install.sh | sh    # or: brew install rush-db/tap/rushdb
export PATH="$HOME/.rushdb/bin:$PATH"             # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` serves `http://127.0.0.1:3000` with its data in `./.rushdb` and the
token `local-token`. In another terminal, `pip install rushdb`:

```python
from rushdb import RushDB

db = RushDB("local-token", direct=True, engine_url="http://127.0.0.1:3000")

# Nested JSON becomes linked records: COMPANY -> DEPARTMENT -> EMPLOYEE
db.records.import_json(label="COMPANY", data={"name": "Acme", "DEPARTMENT": [
    {"name": "Engineering", "EMPLOYEE": [
        {"name": "Ada", "role": "Staff Engineer", "salary": 180000},
        {"name": "Linus", "role": "Engineer", "salary": 140000}]},
    {"name": "Sales", "EMPLOYEE": [{"name": "Grace", "role": "Account Executive", "salary": 120000}]}]})

# Graph: filter across relationships by naming the related labels
engineers = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {"role": {"$contains": "Engineer"}, "DEPARTMENT": {"COMPANY": {"name": "Acme"}}},
    "orderBy": {"name": "asc"},
})
print([employee.data["name"] for employee in engineers])  # ['Ada', 'Linus']

# Analytics: headcount and average salary per department
per_department = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {"DEPARTMENT": {"$alias": "$department"}},
    "select": {"people": {"$count": "*"}, "avgSalary": {"$avg": "$record.salary"}},
    "groupBy": ["$department.name"],
})
print([row.data for row in per_department])
# [{'name': 'Engineering', 'people': 2, 'avgSalary': 160000.0}, {'name': 'Sales', 'people': 1, 'avgSalary': 120000.0}]

# The live schema an agent reads before it writes a query
print(db.ai.get_schema_markdown().data)
```

The schema comes from the catalog, not from a scan of the records:

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
```

<details>
<summary>The same in TypeScript</summary>

`npm install @rushdb/javascript-sdk`:

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('local-token', { direct: true, engineUrl: 'http://127.0.0.1:3000' })

// Nested JSON becomes linked records: COMPANY -> DEPARTMENT -> EMPLOYEE
await db.records.importJson({
  label: 'COMPANY',
  data: { name: 'Acme', DEPARTMENT: [
    { name: 'Engineering', EMPLOYEE: [
      { name: 'Ada', role: 'Staff Engineer', salary: 180000 },
      { name: 'Linus', role: 'Engineer', salary: 140000 } ] },
    { name: 'Sales', EMPLOYEE: [{ name: 'Grace', role: 'Account Executive', salary: 120000 }] } ] }
})

// Graph: filter across relationships by naming the related labels
const engineers = await db.records.find({
  labels: ['EMPLOYEE'],
  where: { role: { $contains: 'Engineer' }, DEPARTMENT: { COMPANY: { name: 'Acme' } } },
  orderBy: { name: 'asc' }
})
console.log(engineers.data.map((employee) => employee.data.name)) // [ 'Ada', 'Linus' ]

// Analytics: headcount and average salary per department
const perDepartment = await db.records.find({
  labels: ['EMPLOYEE'],
  where: { DEPARTMENT: { $alias: '$department' } },
  select: { people: { $count: '*' }, avgSalary: { $avg: '$record.salary' } },
  groupBy: ['$department.name']
})
console.log(perDepartment.data.map((row) => row.data))
// [ { name: 'Engineering', people: 2, avgSalary: 160000 }, { name: 'Sales', people: 1, avgSalary: 120000 } ]

// The live schema an agent reads before it writes a query
console.log((await db.ai.getSchemaMarkdown()).data)
```

</details>

Already speak SQL? Labels are tables:

```bash
PGPASSWORD=local-token psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Next: the [full quickstart](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker and other installs](https://docs.rushdb.com/engine/getting-started/installation/)

## What you can build

| | |
|---|---|
| **Agent memory** | Memories as records with relationships, filtered by scope and ranked by the vectors you store. Ready-made providers for OpenClaw and Hermes. |
| **Search over your app's data** | Full-text, vector and hybrid search over the records your app already writes, with graph filters in the same query. |
| **Questions over connected data** | An agent reads the schema, then writes a query you can inspect: the JSON query language, SQL or Cypher. |
| **Reports without an export** | Group and aggregate the records your app writes, from the SDKs or any PostgreSQL client. |

## Features

- **Emergent schema.** Types are inferred per value, nested objects become linked records, and the catalog keeps exact counts and value lists. [Schema catalog](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Graph queries.** Filter across relationships by label, restrict them by type and direction, and bind related records for projections and aggregations. [Relationships and traversal](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Search built in.** Vector indexes over the vectors you supply, BM25 full-text, and hybrid search that fuses both. [Indexes and search](https://docs.rushdb.com/engine/indexes-and-search/)
- **Aggregations.** `select` and `groupBy` with counts, sums, averages, time buckets and nested collections. [Aggregations](https://docs.rushdb.com/engine/query/aggregations/)
- **Speaks what you use.** SQL over the PostgreSQL wire protocol, Gremlin, SQL/PGQ, Mongo-style documents, and openCypher and ISO GQL as release candidates. [Query languages](https://docs.rushdb.com/engine/languages/)
- **Durable.** A write-ahead log, transactions, backups with point-in-time restore, and read replicas. [Operations](https://docs.rushdb.com/engine/operations/)
- **One binary.** HTTP, PostgreSQL wire, gRPC and Bolt from one process, with optional isolated tenants. [Connectivity](https://docs.rushdb.com/engine/connectivity/)

## Ecosystem

| Project | For |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | TypeScript and Python apps |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor and other MCP clients |
| [Agent skills](https://github.com/rush-db/skills) · [agent plugins](https://github.com/rush-db/agent-plugins) | Coding agents |
| [LangChain and LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | Retrieval, memory and property graph stores |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [memory contract](https://github.com/rush-db/agent-memory-contract) | Agent memory providers |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [Obsidian plugin](https://github.com/rush-db/obsidian-plugin) | Change data capture, notes |

All of them work with a self-hosted engine. To let your coding agent set
RushDB up, hand it [rushdb.com/agent-setup](https://rushdb.com/agent-setup).

## Run it anywhere

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes (Helm), cloud-init, systemd, Fly.io, Render,
Coolify and Dokploy: see [Deploy](https://docs.rushdb.com/deploy/). Read
[Security](https://docs.rushdb.com/engine/operations/security/) before you
expose a node beyond loopback.

## Self-hosted and RushDB Cloud

- **Self-hosted:** this repository. Everything above runs in the engine you install.
- **[RushDB Cloud](https://app.rushdb.com):** the same engine, managed, plus a
  dashboard, managed embeddings, natural-language search and connectors for
  SaaS tools. The SDKs and the MCP server work with both.

## FAQ

**Is it production-ready?** SQL, the native JSON query language, Gremlin and
Mongo-style documents are stable. openCypher, ISO GQL and Bolt are release
candidates. Replication is asynchronous with manual promotion. The
[status page](https://docs.rushdb.com/engine/reference/status/) lists every
surface and its limits.

**Do I need a separate vector database?** No. Vectors, text indexes and
relationships live in one engine, and a record can be written together with
its vector in one commit. The engine searches the vectors you supply; RushDB
Cloud can generate them for you.

**What is the live schema?** A catalog of every label, property, type, value
and relationship shape, updated on every commit, with exact counts. One call
returns it, so agents and tools don't guess field names.
[Schema catalog](https://docs.rushdb.com/engine/data-model/schema-catalog/)

**Windows?** Use WSL 2 or Docker.

---

[Contributing](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Security](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (legacy, Neo4j-based) lives on the
[`2.x` branch](https://github.com/rush-db/rushdb/tree/2.x) with security fixes
until 2027-10-28. [Upgrading from 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
