<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="RushDB logo" width="96">

# RushDB

<sub>面向 AI 的数据库</sub>

**图、向量与分析都作用于同一批记录。每次写入都会维护一份由标签、字段和值构成的实时 schema，让智能体查询真实存在的字段，而不是靠猜。**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[文档](https://docs.rushdb.com) • [快速开始](https://docs.rushdb.com/engine/getting-started/quickstart/) • [讨论区](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • **中文** • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

数据是什么样，就按什么样导入：嵌套 JSON、CSV、SQL `INSERT`、Cypher 或 Mongo 风格的写入，
或者来自你的数据库的变更流。RushDB 把它存储为带类型的记录，并连接成一张图；
同一批记录既能回答关系查询，也能用于向量搜索、全文搜索和聚合，因此不需要
再维护一个需要同步的第二存储。每次写入还会更新一份目录，记录当前存在的标签、
字段、类型和值，因此智能体查询的是真实存在的字段。可以通过 HTTP、PostgreSQL 线协议、
Bolt、gRPC 或 MCP 连接，使用 SDK 或任意 PostgreSQL 客户端均可。一个 Rust 二进制文件，Apache-2.0 许可。

| 图 | 向量与全文搜索 | 分析 |
|---|---|---|
| 嵌套 JSON 会变成相互连接的记录。在一个查询里跨关系过滤，并沿带类型、有方向的关系遍历。 | 向量搜索（精确与 HNSW）、BM25 全文搜索和混合搜索，均可用同样的图谓词过滤。 | 用 `groupBy` 计算计数、求和、平均值、最小值和最大值，或通过 PostgreSQL 线协议直接用 SQL 的 `GROUP BY`。 |

## 快速开始

![终端录屏：安装 RushDB，用 rushdb up 启动，然后在 Python 中导入嵌套 JSON，按部门筛选员工，统计每个部门的人数和平均薪资，并打印实时 schema](assets/quickstart.gif)

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` 在 `http://127.0.0.1:3000` 提供服务，数据保存在 `./.rushdb`，
令牌为 `local-token`。在另一个终端中执行 `pip install rushdb`：

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
<summary>TypeScript 版本</summary>

`npm install @rushdb/javascript-sdk`：

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

已经熟悉 SQL？标签就是表：

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

下一步：[完整的快速开始](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker 及其他安装方式](https://docs.rushdb.com/engine/getting-started/installation/)

## 智能体查询前看到的内容

快速入门的最后一行打印出智能体最先读取的 schema。它来自每次写入都会更新的目录，而不是对记录的扫描：

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

在规模大、类型多的存储上，智能体不会一次加载全部内容。它从概览逐步收窄到精确的值，每次只做一次小的读取（与上面是同一次运行）：

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

`get_schema_summary` 以 JSON 返回同一份目录，包含去重计数和小字段的取值。MCP 服务器把每种读取都提供为工具：`findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues` 和 `getSchemaSummary`。

## 你可以构建什么

- **智能体记忆。** 记忆是按用户、智能体或会话划分的记录，并与产生它的那一轮对话相连，因此召回时能看到记忆的来源。提供现成的 OpenClaw 和 Hermes 记忆提供程序。
- **企业大脑与 RAG。** 文档、CRM、客服和账单数据以各自的形态写入，并通过它们本就共有的键相连，例如邮箱或账户 ID。检索先过滤到正确的账户或版本，再用向量和 BM25 一起排序，每条事实都引用其背后的记录。
- **分面搜索。** 筛选栏的计数来自目录，按当前筛选条件计算且精确，混合搜索也在同一筛选条件下排序。无需另外同步一个搜索索引。
- **应用内的 AI 搜索与助手。** 文本索引和向量索引就在数据库里，随应用写入保持最新；SDK、MCP 服务器和 SQL 客户端读取的是同一批记录。
- **智能体驱动的数据分析。** 智能体先读取有哪些标签和字段、每个值是如何拼写的，再据此写 SQL、Cypher 或聚合查询。每个答案都附带任何人都能重跑的查询。
- **schema 随时演进的应用。** 写入任意 JSON：类型自动推断，嵌套对象变成相互连接的记录，新字段随添加它的那次写入出现在目录中。无需迁移。

## 功能

- **涌现式 schema。** 按每个值推断类型，嵌套对象变成相互连接的记录，目录保存精确的计数和值列表。[Schema 目录](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **图查询。** 按标签跨关系过滤，按类型和方向限定关系，并绑定相关记录用于投影和聚合。[关系与遍历](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **内置搜索。** 基于你提供的向量建立向量索引，BM25 全文搜索，以及融合两者的混合搜索。[索引与搜索](https://docs.rushdb.com/engine/indexes-and-search/)
- **聚合。** `select` 和 `groupBy`，支持计数、求和、平均值、时间分桶和嵌套集合。[聚合](https://docs.rushdb.com/engine/query/aggregations/)
- **支持你在用的语言。** 基于 PostgreSQL 线协议的 SQL、Gremlin、SQL/PGQ、Mongo 风格文档，以及处于候选发布阶段的 openCypher 和 ISO GQL。[查询语言](https://docs.rushdb.com/engine/languages/)
- **持久可靠。** 预写日志、事务、支持时间点恢复的备份，以及只读副本。[运维](https://docs.rushdb.com/engine/operations/)
- **一个二进制文件。** 由同一个进程提供 HTTP、PostgreSQL 线协议、gRPC 和 Bolt，可选隔离的租户。[连接方式](https://docs.rushdb.com/engine/connectivity/)

## 生态

| 项目 | 适用于 |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | TypeScript 和 Python 应用 |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude、Cursor 及其他 MCP 客户端 |
| [智能体技能](https://github.com/rush-db/skills) · [智能体插件](https://github.com/rush-db/agent-plugins) | 编程智能体 |
| [LangChain 和 LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | 检索、记忆和属性图存储 |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [记忆契约](https://github.com/rush-db/agent-memory-contract) | 智能体记忆提供程序 |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [Obsidian 插件](https://github.com/rush-db/obsidian-plugin) | 变更数据捕获、笔记 |

以上项目都可以配合自托管引擎使用。想让编程智能体帮你配置
RushDB，把 [rushdb.com/agent-setup](https://rushdb.com/agent-setup) 交给它即可。

## 随处运行

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data \
  -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose、Kubernetes（Helm）、cloud-init、systemd、Fly.io、Render、
Coolify 和 Dokploy：参见 [部署](https://docs.rushdb.com/deploy/)。在把节点暴露到
回环地址之外之前，请先阅读 [安全](https://docs.rushdb.com/engine/operations/security/)。

## 自托管与 RushDB Cloud

本仓库就是引擎本身，你可以完整地自托管。[RushDB Cloud](https://app.rushdb.com) 为你运行同一个引擎，并增加下表中标出的功能。SDK 和 MCP 服务器两者都支持；在自托管引擎上，仅限 Cloud 的调用会在发送任何内容之前以 `CloudOnlyError` 失败。

![使用合成示例数据的 RushDB 控制台：先是记录表格，然后是公司及其部门集群的图视图，再放大到一个部门，记录面板中列出该部门的员工](assets/dashboard.gif)

| 功能 | 自托管引擎 | RushDB Cloud |
|---|---|---|
| 引擎与 API：记录、图查询、搜索、聚合、实时 schema | ✓ | ✓ |
| SDK 与 MCP 服务器 | ✓ 直连模式，MCP 走 stdio | ✓ 另有带 OAuth 的托管 MCP |
| 导入 JSON 和 CSV | ✓ | ✓ |
| [控制台](https://docs.rushdb.com/rushdb-cloud/)：记录、图视图和查询构建器 | 使用 SDK 或任意 PostgreSQL 客户端 | ✓ |
| [托管嵌入](https://docs.rushdb.com/rushdb-cloud/capabilities/) | 使用你自己模型生成的向量 | ✓ 按月额度 |
| [自然语言搜索](https://docs.rushdb.com/learn/search-query/smart-search/) | 自己编写查询，或交给智能体 | ✓ 按月额度 |
| [关系建议](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | 自己创建关系 | ✓ |
| 指标与过滤条件的[定义](https://docs.rushdb.com/learn/search-query/definitions/) | 把规则写在你自己的提示词和 SQL 里 | ✓ |
| 连接 SaaS 工具的[连接](https://docs.rushdb.com/learn/data-sources/connections/)，以及用于本地数据库的 RushDB Sync | 你自己的同步代码，或 Debezium sink | ✓ |
| 项目与 API 密钥 | 租户和令牌由你自己管理 | ✓ |
| 团队访问 | — | 付费套餐 |
| 备份 | 由你负责：备份、时间点恢复、副本 | 付费套餐 |

RushDB Cloud 列中的 ✓ 表示 Free 套餐已包含。套餐与额度见 [rushdb.com/pricing](https://rushdb.com/pricing)。

## 常见问题

**可以用于生产环境吗？** SQL、原生 JSON 查询语言、Gremlin 和
Mongo 风格文档已经稳定。openCypher、ISO GQL 和 Bolt 处于候选发布阶段。
复制是异步的，需要手动提升主节点。
[状态页](https://docs.rushdb.com/engine/reference/status/) 列出了每个
接口及其限制。

**还需要单独的向量数据库吗？** 不需要。向量、文本索引和关系都在同一个引擎里，
记录可以和它的向量在一次提交中一起写入。引擎搜索的是你提供的向量；RushDB
Cloud 可以替你生成向量。

**什么是实时 schema？** 一份包含所有标签、属性、类型、值和关系形态的目录，
每次提交都会更新，并带有精确计数。一次调用即可返回，因此智能体和工具不必猜测字段名。
[Schema 目录](https://docs.rushdb.com/engine/data-model/schema-catalog/)

**Windows？** 请使用 WSL 2 或 Docker。

---

[贡献](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[安全](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[更新日志](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x（旧版，基于 Neo4j）位于
[`2.x` 分支](https://github.com/rush-db/rushdb/tree/2.x)，安全修复将持续
到 2027-10-28。[从 2.x 升级](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
