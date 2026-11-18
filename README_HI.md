<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="RushDB logo" width="96">

# RushDB

<sub>AI के लिए database</sub>

**Graph, vector और analytics, एक ही records पर। हर write labels, fields और values का एक live schema बनाए रखता है, इसलिए agents अनुमान लगाने के बजाय असली fields पर query करते हैं।**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[दस्तावेज़](https://docs.rushdb.com) • [Quickstart](https://docs.rushdb.com/engine/getting-started/quickstart/) • [चर्चाएँ](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • **हिन्दी** • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

Data जिस रूप में आपके पास है, उसी रूप में लाइए: nested JSON, CSV, SQL `INSERT`, Cypher
या Mongo-style writes, या आपके databases से change streams। RushDB उसे typed records
के रूप में store करता है जो एक graph में जुड़े होते हैं, और यही records relationship
queries, vector और full-text search, और aggregations का जवाब देते हैं, इसलिए sync में
रखने के लिए कोई दूसरा store नहीं होता। हर write मौजूद labels, fields, types और values
का catalog भी update करता है, इसलिए agent असली fields पर query करता है। HTTP, PostgreSQL
wire protocol, Bolt, gRPC या MCP से connect कीजिए, SDKs से या किसी भी PostgreSQL client
से। एक Rust binary, Apache-2.0।

| Graph | Vector और full-text search | Analytics |
|---|---|---|
| Nested JSON जुड़े हुए records बन जाता है। Relationships के पार filter कीजिए और typed, directed relationships को एक ही query में follow कीजिए। | Vector (exact और HNSW), BM25 full-text और hybrid search, उन्हीं graph predicates से filter किए हुए। | `groupBy` के साथ count, sum, average, min और max, या PostgreSQL wire protocol पर `GROUP BY` वाला सीधा SQL। |

## Quickstart

```bash
curl -fsSL https://rushdb.com/install.sh | sh    # or: brew install rush-db/tap/rushdb
export PATH="$HOME/.rushdb/bin:$PATH"             # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` `http://127.0.0.1:3000` पर serve करता है; data `./.rushdb` में रहता है और
token `local-token` है। दूसरे terminal में `pip install rushdb` चलाइए:

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

Schema catalog से आता है, records को scan करके नहीं:

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
```

<details>
<summary>यही TypeScript में</summary>

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

SQL पहले से आती है? Labels ही tables हैं:

```bash
PGPASSWORD=local-token psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

आगे: [पूरा quickstart](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker और install के दूसरे तरीके](https://docs.rushdb.com/engine/getting-started/installation/)

## आप क्या बना सकते हैं

| | |
|---|---|
| **Agent memory** | Memories relationships वाले records के रूप में, scope से filter की हुई और आपके store किए vectors से rank की हुई। OpenClaw और Hermes के लिए तैयार providers। |
| **आपके app के data पर search** | उन records पर full-text, vector और hybrid search जिन्हें आपका app पहले से लिखता है, और उसी query में graph filters। |
| **जुड़े हुए data पर सवाल** | Agent schema पढ़ता है, फिर ऐसी query लिखता है जिसे आप जाँच सकते हैं: JSON query language, SQL या Cypher। |
| **Export के बिना reports** | आपके app के लिखे records को group और aggregate कीजिए, SDKs से या किसी भी PostgreSQL client से। |

## Features

- **Emergent schema.** Types हर value के हिसाब से infer होते हैं, nested objects जुड़े हुए records बन जाते हैं, और catalog सटीक counts और value lists रखता है। [Schema catalog](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Graph queries.** Label के आधार पर relationships के पार filter कीजिए, उन्हें type और direction से सीमित कीजिए, और projections व aggregations के लिए जुड़े records को bind कीजिए। [Relationships और traversal](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Search साथ में।** आपके दिए vectors पर vector indexes, BM25 full-text, और दोनों को मिलाने वाला hybrid search। [Indexes और search](https://docs.rushdb.com/engine/indexes-and-search/)
- **Aggregations.** Counts, sums, averages, time buckets और nested collections के साथ `select` और `groupBy`। [Aggregations](https://docs.rushdb.com/engine/query/aggregations/)
- **वही भाषा जो आप इस्तेमाल करते हैं।** PostgreSQL wire protocol पर SQL, Gremlin, SQL/PGQ, Mongo-style documents, और release candidates के रूप में openCypher और ISO GQL। [Query languages](https://docs.rushdb.com/engine/languages/)
- **Durable.** Write-ahead log, transactions, point-in-time restore वाले backups, और read replicas। [Operations](https://docs.rushdb.com/engine/operations/)
- **एक binary.** एक ही process से HTTP, PostgreSQL wire, gRPC और Bolt, साथ में optional isolated tenants। [Connectivity](https://docs.rushdb.com/engine/connectivity/)

## Ecosystem

| Project | किसके लिए |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | TypeScript और Python apps |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor और दूसरे MCP clients |
| [Agent skills](https://github.com/rush-db/skills) · [agent plugins](https://github.com/rush-db/agent-plugins) | Coding agents |
| [LangChain और LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | Retrieval, memory और property graph stores |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [memory contract](https://github.com/rush-db/agent-memory-contract) | Agent memory providers |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [Obsidian plugin](https://github.com/rush-db/obsidian-plugin) | Change data capture, notes |

ये सभी self-hosted engine के साथ काम करते हैं। RushDB का setup अपने coding agent से
करवाने के लिए उसे [rushdb.com/agent-setup](https://rushdb.com/agent-setup) दीजिए।

## कहीं भी चलाइए

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes (Helm), cloud-init, systemd, Fly.io, Render,
Coolify और Dokploy: [Deploy](https://docs.rushdb.com/deploy/) देखिए। किसी node को
loopback से बाहर expose करने से पहले [Security](https://docs.rushdb.com/engine/operations/security/)
पढ़िए।

## Self-hosted और RushDB Cloud

- **Self-hosted:** यही repository। ऊपर बताई हर चीज़ उसी engine में चलती है जिसे आप install करते हैं।
- **[RushDB Cloud](https://app.rushdb.com):** वही engine, managed रूप में, साथ में
  dashboard, managed embeddings, natural-language search और SaaS tools के लिए
  connectors। SDKs और MCP server दोनों के साथ काम करते हैं।

## FAQ

**क्या यह production के लिए तैयार है?** SQL, native JSON query language, Gremlin और
Mongo-style documents stable हैं। openCypher, ISO GQL और Bolt release candidates
हैं। Replication asynchronous है और promotion manual होता है।
[Status page](https://docs.rushdb.com/engine/reference/status/) हर surface और उसकी
सीमाएँ बताता है।

**क्या मुझे अलग vector database चाहिए?** नहीं। Vectors, text indexes और
relationships एक ही engine में रहते हैं, और किसी record को उसके vector के साथ एक ही
commit में लिखा जा सकता है। Engine आपके दिए vectors पर search करता है; RushDB
Cloud उन्हें आपके लिए generate कर सकता है।

**Live schema क्या है?** हर label, property, type, value और relationship shape का
catalog, जो हर commit पर सटीक counts के साथ update होता है। एक call उसे लौटा देती
है, इसलिए agents और tools को field names का अनुमान नहीं लगाना पड़ता।
[Schema catalog](https://docs.rushdb.com/engine/data-model/schema-catalog/)

**Windows?** WSL 2 या Docker इस्तेमाल कीजिए।

---

[योगदान](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Security](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (legacy, Neo4j-आधारित)
[`2.x` branch](https://github.com/rush-db/rushdb/tree/2.x) पर है और उसे
2027-10-28 तक security fixes मिलेंगे। [2.x से upgrade करना](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
