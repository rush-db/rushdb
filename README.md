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

![Terminal recording: install RushDB, start it with rushdb up, then from Python import nested JSON, filter employees by department, count people and average salary per department, and print the live schema](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

**Let your coding agent set it up.** Paste this prompt into it ([rushdb.com/agent-setup](https://rushdb.com/agent-setup)):

```text
Set up RushDB as this project's database.

1. Install the RushDB MCP server (@rushdb/mcp-server) and the RushDB agent skills (github.com/rush-db/skills).
2. Use RushDB Cloud with an API key from app.rushdb.com, or a local engine:
   curl -fsSL https://rushdb.com/install.sh | sh && rushdb up
   (direct mode: engine URL http://127.0.0.1:3000, token local-token)
3. Before writing any query, read the schema catalog (labels, properties, types, relationship shapes) and use only fields that exist.
4. Docs: https://docs.rushdb.com/
```

Or set it up by hand:

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` serves `http://127.0.0.1:3000` with its data in `./.rushdb` and the
token `local-token`. In another terminal, `pip install rushdb`:

```python
from rushdb import RushDB

db = RushDB("local-token", direct=True, engine_url="http://127.0.0.1:3000")

# Nested JSON becomes linked records: COMPANY -> DEPARTMENT -> EMPLOYEE
db.records.import_json(label="COMPANY", data={
    "name": "Acme",
    "DEPARTMENT": [
        {
            "name": "Engineering",
            "EMPLOYEE": [
                {"name": "Ada", "role": "Staff Engineer", "salary": 180000},
                {"name": "Linus", "role": "Engineer", "salary": 140000},
            ],
        },
        {
            "name": "Sales",
            "EMPLOYEE": [
                {"name": "Grace", "role": "Account Executive",
                 "salary": 120000},
            ],
        },
    ],
}, options={"relationshipType": "HAS"})

# Graph: filter across relationships by naming the related labels
engineers = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {
        "role": {"$contains": "Engineer"},
        "DEPARTMENT": {"COMPANY": {"name": "Acme"}},
    },
    "orderBy": {"name": "asc"},
})
print([employee.data["name"] for employee in engineers])  # ['Ada', 'Linus']

# Analytics: headcount and average salary per department
per_department = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {"DEPARTMENT": {"$alias": "$department"}},
    "select": {
        "people": {"$count": "*"},
        "avgSalary": {"$avg": "$record.salary"},
    },
    "groupBy": ["$department.name"],
})
print([row.data for row in per_department])
# [{'name': 'Engineering', 'people': 2, 'avgSalary': 160000.0},
#  {'name': 'Sales', 'people': 1, 'avgSalary': 120000.0}]

# The live schema an agent reads before it writes a query
print(db.ai.get_schema_markdown().data)
```

<details>
<summary>The same in TypeScript</summary>

`npm install @rushdb/javascript-sdk`:

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('local-token', {
  direct: true,
  engineUrl: 'http://127.0.0.1:3000'
})

// Nested JSON becomes linked records: COMPANY -> DEPARTMENT -> EMPLOYEE
await db.records.importJson({
  label: 'COMPANY',
  data: {
    name: 'Acme',
    DEPARTMENT: [
      {
        name: 'Engineering',
        EMPLOYEE: [
          { name: 'Ada', role: 'Staff Engineer', salary: 180000 },
          { name: 'Linus', role: 'Engineer', salary: 140000 }
        ]
      },
      {
        name: 'Sales',
        EMPLOYEE: [
          { name: 'Grace', role: 'Account Executive', salary: 120000 }
        ]
      }
    ]
  },
  options: { relationshipType: 'HAS' }
})

// Graph: filter across relationships by naming the related labels
const engineers = await db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    role: { $contains: 'Engineer' },
    DEPARTMENT: { COMPANY: { name: 'Acme' } }
  },
  orderBy: { name: 'asc' }
})
console.log(engineers.data.map((employee) => employee.data.name))
// [ 'Ada', 'Linus' ]

// Analytics: headcount and average salary per department
const perDepartment = await db.records.find({
  labels: ['EMPLOYEE'],
  where: { DEPARTMENT: { $alias: '$department' } },
  select: {
    people: { $count: '*' },
    avgSalary: { $avg: '$record.salary' }
  },
  groupBy: ['$department.name']
})
console.log(perDepartment.data.map((row) => row.data))
// [ { name: 'Engineering', people: 2, avgSalary: 160000 },
//   { name: 'Sales', people: 1, avgSalary: 120000 } ]

// The live schema an agent reads before it writes a query
console.log((await db.ai.getSchemaMarkdown()).data)
```

</details>

Already speak SQL? Labels are tables:

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Next: the [full quickstart](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker and other installs](https://docs.rushdb.com/engine/getting-started/installation/)

## What agents see before they query

The last line of the quickstart prints the schema an agent reads first. It
comes from the catalog every write updates, not from a scan of the records:

```text
# Graph Schema

## Labels

| Label | Count |
|-------|------:|
| `EMPLOYEE` | 3 |
| `DEPARTMENT` | 2 |
| `COMPANY` | 1 |

---

## `EMPLOYEE` (3 records)

### Properties

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |

### Relationships

| Traversal | Count | Edge Properties |
|-----------|------:|-----------------|
| `(EMPLOYEE)<-[:HAS]-(DEPARTMENT)` | 3 | — |

---

## `DEPARTMENT` (2 records)

### Properties

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Engineering`, `Sales` | — |

### Relationships

| Traversal | Count | Edge Properties |
|-----------|------:|-----------------|
| `(DEPARTMENT)<-[:HAS]-(COMPANY)` | 2 | — |
| `(DEPARTMENT)-[:HAS]->(EMPLOYEE)` | 3 | — |

---

## `COMPANY` (1 record)

### Properties

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Acme` | — |

### Relationships

| Traversal | Count | Edge Properties |
|-----------|------:|-----------------|
| `(COMPANY)-[:HAS]->(DEPARTMENT)` | 2 | — |
```

On a large, diverse store an agent doesn't load all of that. It narrows from
the overview to exact values, one small read at a time (same run as above):

```python
# 1. Labels and their record counts: a cheap first read on any store
print(db.labels.find({})["data"])
# {'COMPANY': 1, 'DEPARTMENT': 2, 'EMPLOYEE': 3}

# 2. The real names behind the words in a question
print(db.ai.resolve_names("employees.salaries").data)
# [{'label': 'EMPLOYEE', 'property': 'salary', 'score': 1.0}]

# 3. The schema of only the labels the question needs
print(db.ai.get_schema_markdown({"labels": ["EMPLOYEE"]}).data)
# the EMPLOYEE part of the schema above; the other labels are left out

# 4. The fields of the records a filter keeps, with counts
props = db.properties.find({
    "labels": ["EMPLOYEE"],
    "where": {"DEPARTMENT": {"name": "Sales"}},
})["data"]
print([(p["name"], p["recordsCount"]) for p in props])
# [('name', 1), ('role', 1), ('salary', 1)]

# 5. Where a value from the question is stored, even misspelled
matches = db.ai.search_values("enginer", match="similar").data["matches"]
print([(m["label"], m["property"], m["value"]) for m in matches])
# [('EMPLOYEE', 'role', 'Engineer'), ('EMPLOYEE', 'role', 'Staff Engineer'),
#  ('DEPARTMENT', 'name', 'Engineering')]

# 6. The exact stored spellings of one field, with counts
role = next(p["id"] for p in props if p["name"] == "role")
print(db.properties.values(role, query="eng")["data"])
# {'values': ['Engineer', 'Staff Engineer'], 'counts': [1, 1],
#  'distinctCount': 2}
```

`get_schema_summary` returns the same catalog as JSON, with distinct counts
and the values of small fields. The MCP server offers each read as a tool:
`findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`,
`searchValues`, `propertyValues` and `getSchemaSummary`.

## Deploy

| Where | How |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3` |
| Docker Compose | [`deploy/templates/docker-compose`](https://github.com/rush-db/rushdb/tree/main/deploy/templates/docker-compose), with an HTTPS variant behind Caddy |
| Kubernetes | [Helm chart](https://github.com/rush-db/rushdb/tree/main/charts/rushdb): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token="$(openssl rand -hex 32)"` |
| Linux VM | [cloud-init](https://github.com/rush-db/rushdb/tree/main/deploy/templates/cloud-init) for DigitalOcean, Hetzner, AWS, Google Cloud and Azure; [systemd unit](https://github.com/rush-db/rushdb/tree/main/deploy/templates/systemd) |
| Platforms | [Fly.io](https://github.com/rush-db/rushdb/tree/main/deploy/templates/fly), [Render](https://github.com/rush-db/rushdb/tree/main/deploy/templates/render), [Coolify](https://github.com/rush-db/rushdb/tree/main/deploy/templates/coolify), [Dokploy](https://github.com/rush-db/rushdb/tree/main/deploy/templates/dokploy) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

Ports, probes, sizing and running as a service: [Deployment](https://docs.rushdb.com/engine/operations/deployment/). Every option on one page:
[docs.rushdb.com/deploy](https://docs.rushdb.com/deploy/).

### Other ways to install

| Method | Command | Notes |
|---|---|---|
| Install script | `curl -fsSL https://rushdb.com/install.sh \| sh` | Linux (glibc 2.28+) and macOS 11+, x86-64 and arm64. `sh -s -- --version v3.0.0` pins a release. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS and Linux. `brew services start rushdb` runs it in the background. |
| Docker | `rushdb/rushdb:3` | `linux/amd64` and `linux/arm64`, also `ghcr.io/rush-db/rushdb`. Tags `3`, `3.0` and exact versions. |
| Cargo | `cargo binstall rushdb-server` · `cargo install rushdb-server --locked` | `binstall` fetches the release binary; `install` compiles it and needs a C/C++ compiler. |
| Release archive | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Archives with SHA-256 checksums, an SPDX SBOM and Sigstore signatures. |
| Source | `cargo build --release -p rushdb-server --bin rushdb` | Toolchain pinned in `rust-toolchain.toml`. See [Installation](https://docs.rushdb.com/engine/getting-started/installation/). |

Windows is not supported natively; use WSL 2 or Docker. Read
[Security](https://docs.rushdb.com/engine/operations/security/) before you expose a node beyond
loopback.

## What you can build

- **Agent memory.** Memories are records scoped by user, agent or session and
  linked to the turn that produced them, so recall returns where a memory came
  from. Ready-made providers for OpenClaw and Hermes.
- **Company brain and RAG.** Docs, CRM, support and billing data land in their
  own shapes and link on keys they already share, like an email or an account
  ID. Retrieval filters to the right account or version first, then ranks by
  vector and BM25 together, and every fact cites the record behind it.
- **Faceted search.** Filter sidebars whose counts come from the catalog,
  scoped to the current filter and exact, with hybrid search ranked under the
  same filter. No separate search index to keep in sync.
- **AI search and assistants in your app.** Text and vector indexes live in the
  database and stay current as your app writes; the SDKs, the MCP server and
  SQL clients read the same records.
- **Agentic data analytics.** The agent reads which labels and fields exist and
  how each value is spelled, then writes SQL, Cypher or an aggregation against
  them. Every answer ships with a query anyone can rerun.
- **Apps whose schema evolves on the go.** Push any JSON: types are inferred,
  nested objects become linked records, and new fields appear in the catalog
  with the write that adds them. No migrations.

## Features

- **Emergent schema.** Types are inferred per value, nested objects become linked records, and the catalog keeps exact counts and value lists. [Schema catalog](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Graph queries.** Filter across relationships by label, restrict them by type and direction, and bind related records for projections and aggregations. [Relationships and traversal](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Search built in.** Vector indexes over the vectors you supply, BM25 full-text, and hybrid search that fuses both. [Indexes and search](https://docs.rushdb.com/engine/indexes-and-search/)
- **Aggregations.** `select` and `groupBy` with counts, sums, averages, time buckets and nested collections. [Aggregations](https://docs.rushdb.com/engine/query/aggregations/)
- **Speaks what you use.** SQL over the PostgreSQL wire protocol, Gremlin, SQL/PGQ, Mongo-style documents, and openCypher and ISO GQL as release candidates. [Query languages](https://docs.rushdb.com/engine/languages/)
- **Durable.** A write-ahead log, transactions, backups with point-in-time restore, and read replicas. [Operations](https://docs.rushdb.com/engine/operations/)
- **One binary.** HTTP, PostgreSQL wire, gRPC and Bolt from one process, with optional isolated tenants. [Connectivity](https://docs.rushdb.com/engine/connectivity/)

## Connect

| Surface | Default | Enable |
|---|---|---|
| [HTTP native protocol](https://docs.rushdb.com/engine/connectivity/http-api/) | `127.0.0.1:3000`, `POST /protocol/v1/execute` | Always on; `--port` moves it |
| [PostgreSQL wire](https://docs.rushdb.com/engine/connectivity/postgresql-wire/) | `127.0.0.1:5432` | On while `sql` is enabled (the default); `--pgwire-port 0` turns it off |
| [gRPC](https://docs.rushdb.com/engine/connectivity/grpc/) (`rushdb.v1.Database/Execute`) | Off | `--grpc-port <port>` |
| [Bolt 5.0](https://docs.rushdb.com/engine/connectivity/bolt/) | Off | `--bolt-port <port>` with `cypher` in `--languages` |
| [Local MCP](https://docs.rushdb.com/connect/mcp/) (`@rushdb/mcp-server`, [tools](https://docs.rushdb.com/learn/reference/mcp/tools/)) | stdio: `npx -y @rushdb/mcp-server` | Direct mode to a self-hosted engine: `RUSHDB_MODE=direct`, `RUSHDB_ENGINE_URL`, `RUSHDB_ENGINE_TOKEN` |
| [Hosted MCP](https://docs.rushdb.com/connect/mcp/) | `https://mcp.rushdb.com/mcp`, OAuth for ChatGPT, Claude.ai and other web clients | A RushDB Cloud feature; nothing to install |

All listeners bind `--listen` (default `127.0.0.1`) and share one TLS
certificate (`--tls-cert`, `--tls-key`). More in [Connectivity](https://docs.rushdb.com/engine/connectivity/),
[Authentication](https://docs.rushdb.com/engine/connectivity/authentication/) and [SQL and Cypher clients](https://docs.rushdb.com/connect/sql-and-cypher/). The SDKs,
the MCP server and the integrations below reach a self-hosted engine in direct
mode: see [Connect](https://docs.rushdb.com/connect/).

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

## Self-hosted and RushDB Cloud

This repository is the engine, and all of it is yours to self-host.
[RushDB Cloud](https://app.rushdb.com) runs the same engine for you and adds
the features marked below. The SDKs and the MCP server work with both; on a
self-hosted engine, a Cloud-only call fails with `CloudOnlyError` before
anything is sent.

![RushDB dashboard with synthetic sample data: the records table, then the graph view of companies and their department clusters, zooming into one department with its employees listed in the record panel](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| Feature | Self-hosted engine | RushDB Cloud |
|---|---|---|
| Engine and API: records, graph queries, search, aggregations, live schema | ✓ | ✓ |
| SDKs and MCP server | ✓ direct mode, MCP over stdio | ✓ plus hosted MCP with OAuth |
| Imports of JSON and CSV | ✓ | ✓ |
| [Dashboard](https://docs.rushdb.com/rushdb-cloud/) with records, graph view and query builder | Use the SDKs or any PostgreSQL client | ✓ |
| [Managed embeddings](https://docs.rushdb.com/rushdb-cloud/capabilities/) | Bring vectors from your own model | ✓ monthly allowance |
| [Natural-language search](https://docs.rushdb.com/learn/search-query/smart-search/) | Build the query yourself or with an agent | ✓ monthly allowance |
| [Relationship suggestions](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | Create relationships yourself | ✓ |
| [Definitions](https://docs.rushdb.com/learn/search-query/definitions/) for metrics and filters | Keep the rules in your own prompts and SQL | ✓ |
| [Connections](https://docs.rushdb.com/learn/data-sources/connections/) to SaaS tools, and RushDB Sync for on-prem databases | Your own sync code, or the Debezium sink | ✓ |
| Projects and API keys | Tenants and tokens you manage | ✓ |
| Team access | — | Paid plans |
| Backups | You run them: backup, point-in-time restore, replicas | Paid plans |

A ✓ under RushDB Cloud means the Free plan includes it. Plans and allowances
are on [rushdb.com/pricing](https://rushdb.com/pricing).

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

---

[Contributing](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Security](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (legacy, Neo4j-based) lives on the
[`2.x` branch](https://github.com/rush-db/rushdb/tree/2.x) with security fixes
until 2027-10-28. [Upgrading from 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
