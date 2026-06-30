<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### AI एजेंट्स और ऐप्स के लिए memory layer।

कोई भी JSON पुश करें। आपके एजेंट को एक live, queryable schema, graph relationships, और semantic search मिलता है — सब अपने-आप inferred।
कोई pipeline नहीं। कोई अलग store नहीं। design करने के लिए कोई schema नहीं।

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 वेबसाइट](https://rushdb.com) • [📖 दस्तावेज़ीकरण](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 उदाहरण](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • **हिन्दी** • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## समस्या

आपके एजेंट को memory चाहिए। मानक जवाब है तीन डेटाबेस: key-value के लिए Redis, semantic search के लिए एक vector store, relationships के लिए एक graph DB — और इन्हें sync में रखने के लिए glue code।

RushDB इन तीनों को बदल देता है। JSON एक बार पुश करें। इसे graph traversal, semantic search, या दोनों से query करें — एक ही call में।

| RushDB के बिना                                   | RushDB के साथ                                |
| ------------------------------------------------ | -------------------------------------------- |
| Redis + Pinecone + Neo4j + glue code             | एक API                                       |
| Schema डिज़ाइन करें → migrations लिखें → दोहराएँ | कोई भी shape पुश करें, schema की ज़रूरत नहीं |
| अलग embedding pipeline                           | Managed embeddings, server-side              |
| Relationship edges हाथ से बनाएँ                  | आपकी data structure से अपने-आप detect        |

---

## Quick start

आपके setup के अनुसार दो रास्ते:

- **Cloud** — Managed, free tier, 30 सेकंड में चालू। [API key पाएँ →](https://app.rushdb.com)
- **Self-host** — Docker + आपका अपना Neo4j instance। [Self-hosting पर जाएँ →](#self-hosting)

### Cloud रास्ता

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### एजेंट memory store और recall करें

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
const memories = await db.ai.search({
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

## अपने data के साथ काम करना

### Nested JSON import करें

कोई भी JSON shape पुश करें। Nested objects और objects के arrays linked records बन जाते हैं — labels, types, और relationships write पर inferred होते हैं। कोई schema नहीं, कोई migration step नहीं।

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

प्रत्येक nested key (`DEPARTMENT`, `EMPLOYEE`) एक label बनती है, प्रत्येक object एक record, और containment एक relationship — सब अपने-आप बनते हैं।

### CSV import करें

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

`suggestTypes` strings से numbers, booleans, और dates infer करता है; `skipEmptyValues` खाली cells को empty values store करने के बजाय unset मानता है (`0` और `false` रखे जाते हैं)।

### Graph को traverse करें

Root records को उनके _related_ records पर conditions के आधार पर filter करें — मनमाने रूप से गहराई तक — एक ही query में। Related labels `labels` में नहीं, `where` के **अंदर** जाते हैं:

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

### Analytical queries (aggregate और group by)

`select` output को aggregations (`$sum`, `$avg`, `$count`, `$min`, `$max`) के साथ आकार देता है; `groupBy` dimensions को नियंत्रित करता है। किसी aggregation में `limit` न जोड़ें — यह केवल पहले N records को scan करेगा और totals को विकृत कर देगा।

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

Aggregations traversal के साथ compose होते हैं — जैसे प्रति department headcount और payroll:

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

## Claude, Cursor, या किसी भी MCP client से जुड़ें

RushDB एक MCP server के साथ आता है। आपके एजेंट को persistent, structured memory मिलती है — out of the box।

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

इसे अपने Claude Desktop, Cursor, या Windsurf MCP config में रखें। अब एजेंट records बना सकता है, अर्थ के आधार पर search कर सकता है, relationships traverse कर सकता है, और schema को introspect कर सकता है — सब natural language के माध्यम से।

---

## बॉक्स में क्या है

| क्षमता                          | इसका क्या मतलब है                                                                                                                                           |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Managed embeddings**          | किसी भी string property को एक बार index करें — हर write server-side पर auto-embedded होता है                                                                |
| **एक query में Graph + vector** | Semantic similarity और relationship traversal एक ही call में compose होते हैं                                                                               |
| **Zero schema**                 | कोई भी JSON shape पुश करें। RushDB types infer करता है, properties बनाता है, records को link करता है                                                        |
| **Inferred schema**             | Properties first-class nodes बन जाती हैं — types, labels, और relationships write पर discover होते हैं, इसलिए data आते ही एक queryable schema खुद बन जाता है |
| **ACID transactions**           | Concurrent एजेंट shared memory को corrupt नहीं करते। नीचे Neo4j है                                                                                          |
| **Self-describing**             | एजेंट inferred schema को introspect करते हैं — labels, properties, value ranges — ताकि उन्हें पता हो कि वे सुरक्षित रूप से क्या query कर सकते हैं           |
| **MCP-native**                  | discovery-first query prompt के साथ पूर्ण MCP server built in                                                                                               |
| **Agent Skills**                | Installable `@rushdb/skills` package — किसी भी skills-compatible एजेंट को एक command में RushDB के साथ query, model, और remember करना सिखाएँ                |
| **Unified query API**           | graph, vector, aggregation, और introspection के लिए एक JSON shape                                                                                           |
| **Self-host या cloud**          | Docker + आपका Neo4j, या managed cloud। पूर्ण data ownership                                                                                                 |

---

## उपयोग के मामले

| उपयोग का मामला              | RushDB किसे बदलता है            | प्रमुख API                                                    |
| --------------------------- | ------------------------------- | ------------------------------------------------------------- |
| **Agent memory**            | Redis + vector store + graph DB | `db.ai.search({ query, where: { agent_id } })`                |
| **Context के साथ RAG**      | Flat vector store               | `db.records.find({ where, labels })` + relationship traversal |
| **Schema-free ऐप्स**        | Postgres + migrations + ETL     | `db.records.importJson(nestedJson)`                           |
| **Connected data products** | कई joined services              | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })` |

---

## Self-hosting

> **Self-host रास्ता** — RushDB को अपने स्वयं के infrastructure पर चलाएँ। APOC plugin के साथ Neo4j 2026.01.4+ की आवश्यकता है।

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
<summary>पूर्ण environment variables</summary>

| नाम                               | विवरण                                             | आवश्यक     | Default  |
| --------------------------------- | ------------------------------------------------- | ---------- | -------- |
| `NEO4J_URL`                       | Neo4j connection URL                              | हाँ        | —        |
| `NEO4J_USERNAME`                  | Neo4j username                                    | हाँ        | neo4j    |
| `NEO4J_PASSWORD`                  | Neo4j password                                    | हाँ        | —        |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | API token encryption के लिए 32-char key           | हाँ (prod) | —        |
| `RUSHDB_PORT`                     | HTTP port                                         | नहीं       | 3000     |
| `RUSHDB_LOGIN`                    | Admin login                                       | नहीं       | admin    |
| `RUSHDB_PASSWORD`                 | Admin password                                    | नहीं       | password |
| `RUSHDB_BASE_URL`                 | synx assignments के लिए Public/base API URL       | नहीं       | —        |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | Managed synx workers के लिए internal token        | नहीं       | —        |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | synx destination writes के लिए internal write key | नहीं       | —        |

Managed synx workers daemons के रूप में चलते हैं। वे runnable connectors के लिए poll करते हैं, connector leases को renew करते हैं, graceful shutdown पर leases release करते हैं, और platform/core को crashes के बाद expired leases reclaim करने देते हैं।

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
<summary>Architecture: RushDB data को कैसे structure करता है (LMPG)</summary>

RushDB एक **Labeled Meta Property Graph (LMPG)** model का उपयोग करता है। Properties को first-class graph nodes ("HyperProperties") तक उठाया जाता है — न कि केवल records से जुड़े key-value pairs।

इसका मतलब है:

- **बिना upfront design के Schema** — क्योंकि properties graph nodes हैं, schema _आपके data से inferred होता है, designed नहीं_: labels, types, value ranges, और relationship topology write पर discover होते हैं और तुरंत queryable होते हैं — कोई manual schema modeling नहीं, कोई RDF/OWL toolchain नहीं
- **Auto-detected relationships** — properties share करने वाले records edges हाथ से बनाए बिना link हो जाते हैं
- **Schema introspection** — एजेंट एक query में labels, property types, value ranges, और relationship topology को enumerate कर सकते हैं
- **Soft constraints** — type cohesion scoring, cardinality tracking, और vector dimension enforcement बिना rigid upfront schemas के
- **Unified query surface** — वही filter expression records, labels, properties, और relationships के पार काम करता है

एक SearchQuery एक साथ कई perspectives retrieve करता है (records + property stats + aggregations), जिससे separate-system architectures में आम N+1 inspection pattern से बचा जाता है।

[पूरी LMPG architecture post पढ़ें →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## दस्तावेज़ीकरण

| विषय                     | लिंक                                                           |
| ------------------------ | -------------------------------------------------------------- |
| Quick Tutorial           | https://docs.rushdb.com/get-started/quick-tutorial             |
| Vector / Semantic Search | https://docs.rushdb.com/concepts/search/where#vector-operators |
| Filtering और Traversal   | https://docs.rushdb.com/concepts/search/where                  |
| Grouping और Aggregations | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK           | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK               | https://docs.rushdb.com/python-sdk/introduction                |
| REST API                 | https://docs.rushdb.com/rest-api/introduction                  |
| MCP Server               | packages/mcp-server/README.md                                  |
| Agent Skills             | packages/skills/README.md                                      |

---

## RushDB का उपयोग कब न करें

- आपको बहुत उच्च write throughput पर sub-millisecond latency चाहिए — RushDB Neo4j पर बना है, जो raw write speed के बजाय consistency और query expressiveness को प्राथमिकता देता है।
- आपको केवल flat key-value storage चाहिए, बिना relationships या semantic search के — एक सरल store हल्का होगा।
- आपको database level पर लागू एक rigid relational schema चाहिए — RushDB जानबूझकर schema-free है।

---

## योगदान

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

पूरे दिशानिर्देशों के लिए [CONTRIBUTING.md](CONTRIBUTING.md) देखें। Issues और PRs का स्वागत है।

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

अभी तक support न की गई कोई चीज़ चाहिए? एक issue खोलें — design discussions का स्वागत है।
