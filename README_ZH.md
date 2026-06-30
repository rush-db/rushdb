<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### 面向 AI 智能体与应用的记忆层。

推送任意 JSON。你的智能体即可获得自动推断的实时可查询模式、图关系以及语义搜索。
无需流水线。无需独立的存储系统。无需设计模式。

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 官网](https://rushdb.com) • [📖 文档](https://docs.rushdb.com) • [☁️ 云服务](https://app.rushdb.com) • [🔍 示例](https://github.com/rush-db/examples)

[English](README.md) • **中文** • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## 痛点所在

你的智能体需要记忆。标准的做法是使用三个数据库：用 Redis 存键值、用向量存储做语义搜索、用图数据库处理关系，再加上让它们保持同步的胶水代码。

RushDB 一次性取代这三者。只需推送一次 JSON，即可通过图遍历、语义搜索或两者结合，在一次调用中完成查询。

| 没有 RushDB                         | 使用 RushDB              |
| ----------------------------------- | ------------------------ |
| Redis + Pinecone + Neo4j + 胶水代码 | 一个 API                 |
| 设计模式 → 编写迁移 → 不断重复      | 推送任意结构，无需模式   |
| 独立的嵌入流水线                    | 服务端托管的嵌入处理     |
| 手工构建关系边                      | 从你的数据结构中自动检测 |

---

## 快速开始

根据你的部署方式，有两条路径：

- **云服务** — 托管式，提供免费额度，30 秒内即可运行。[获取 API 密钥 →](https://app.rushdb.com)
- **自托管** — Docker + 你自己的 Neo4j 实例。[跳转到自托管 →](#self-hosting)

### 云服务路径

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### 存储与召回智能体记忆

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

## 处理你的数据

### 导入嵌套 JSON

推送任意 JSON 结构。嵌套对象以及对象数组会成为相互关联的记录，标签、类型和关系在写入时被自动推断。无需模式，无需迁移步骤。

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

每个嵌套键（`DEPARTMENT`、`EMPLOYEE`）会成为一个标签，每个对象成为一条记录，包含关系则成为一种关系，全部自动创建。

### 导入 CSV

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

`suggestTypes` 会从字符串中推断数字、布尔值和日期；`skipEmptyValues` 会将空白单元格视为未设置，而非存储为空值（`0` 和 `false` 会被保留）。

### 遍历图

在单次查询中，根据*相关*记录上的条件来过滤根记录，且可以任意深度。相关标签应放在 `where` **内部**，而非 `labels` 中：

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

### 分析型查询（聚合与分组）

`select` 通过聚合（`$sum`、`$avg`、`$count`、`$min`、`$max`）来塑造输出；`groupBy` 则控制维度。不要给聚合添加 `limit`，否则它只会扫描前 N 条记录并使总计结果失真。

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

聚合可与遍历组合使用，例如计算每个部门的人数和工资总额：

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

## 连接到 Claude、Cursor 或任意 MCP 客户端

RushDB 自带一个 MCP 服务器。你的智能体开箱即可获得持久化的结构化记忆。

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

将其放入你的 Claude Desktop、Cursor 或 Windsurf 的 MCP 配置中。此后智能体即可创建记录、按含义搜索、遍历关系并自省模式，全部通过自然语言完成。

---

## 内置功能一览

| 能力                           | 含义                                                                                               |
| ------------------------------ | -------------------------------------------------------------------------------------------------- |
| **托管嵌入**                   | 为任意字符串属性建立一次索引，此后每次写入都会在服务端自动嵌入                                     |
| **图与向量于一次查询**         | 语义相似度与关系遍历可在单次调用中组合                                                             |
| **零模式**                     | 推送任意 JSON 结构。RushDB 会推断类型、创建属性、关联记录                                          |
| **推断模式**                   | 属性成为一等节点，类型、标签和关系在写入时被发现，因此可查询的模式随数据到达而自我构建             |
| **ACID 事务**                  | 并发的智能体不会破坏共享记忆。底层基于 Neo4j                                                       |
| **自描述**                     | 智能体可自省推断出的模式——标签、属性、取值范围——以了解自己可以安全查询哪些内容                     |
| **MCP 原生**                   | 完整的 MCP 服务器，内置“发现优先”的查询提示                                                        |
| **智能体技能（Agent Skills）** | 可安装的 `@rushdb/skills` 包，一条命令即可教会任何兼容技能的智能体使用 RushDB 进行查询、建模和记忆 |
| **统一查询 API**               | 用一种 JSON 结构完成图、向量、聚合和自省                                                           |
| **自托管或云服务**             | Docker + 你的 Neo4j，或托管式云服务。完全的数据所有权                                              |

---

## 应用场景

| 应用场景           | RushDB 取代的对象           | 关键 API                                                      |
| ------------------ | --------------------------- | ------------------------------------------------------------- |
| **智能体记忆**     | Redis + 向量存储 + 图数据库 | `db.ai.search({ query, where: { agent_id } })`                |
| **带上下文的 RAG** | 扁平的向量存储              | `db.records.find({ where, labels })` + 关系遍历               |
| **无模式应用**     | Postgres + 迁移 + ETL       | `db.records.importJson(nestedJson)`                           |
| **关联数据产品**   | 多个联接的服务              | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })` |

---

## 自托管

> **自托管路径** — 在你自己的基础设施上运行 RushDB。需要带 APOC 插件的 Neo4j 2026.01.4+。

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
<summary>完整环境变量</summary>

| 名称                              | 描述                              | 是否必需       | 默认值   |
| --------------------------------- | --------------------------------- | -------------- | -------- |
| `NEO4J_URL`                       | Neo4j 连接 URL                    | 是             | —        |
| `NEO4J_USERNAME`                  | Neo4j 用户名                      | 是             | neo4j    |
| `NEO4J_PASSWORD`                  | Neo4j 密码                        | 是             | —        |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | 用于 API 令牌加密的 32 字符密钥   | 是（生产环境） | —        |
| `RUSHDB_PORT`                     | HTTP 端口                         | 否             | 3000     |
| `RUSHDB_LOGIN`                    | 管理员登录名                      | 否             | admin    |
| `RUSHDB_PASSWORD`                 | 管理员密码                        | 否             | password |
| `RUSHDB_BASE_URL`                 | 用于 synx 分配的公开/基础 API URL | 否             | —        |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | 用于托管 synx worker 的内部令牌   | 否             | —        |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | 用于 synx 目标写入的内部写入密钥  | 否             | —        |

托管的 synx worker 以守护进程方式运行。它们轮询可运行的连接器、续期连接器租约、在优雅关闭时释放租约，并让 platform/core 在崩溃后回收过期租约。

</details>

<details>
<summary>本地开发（捆绑 Neo4j）</summary>

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
<summary>架构：RushDB 如何组织数据（LMPG）</summary>

RushDB 采用**带标签的元属性图（Labeled Meta Property Graph，LMPG）**模型。属性被提升为一等图节点（“HyperProperties”），而不仅仅是附加在记录上的键值对。

这意味着：

- **无需前期设计的模式** — 由于属性是图节点，模式是*从你的数据中推断出来的，而非设计出来的*：标签、类型、取值范围以及关系拓扑在写入时被发现并立即可查询，无需手工建模，无需 RDF/OWL 工具链
- **自动检测的关系** — 共享属性的记录会被关联起来，无需手工构建边
- **模式自省** — 智能体可在一次查询中枚举标签、属性类型、取值范围和关系拓扑
- **软约束** — 类型内聚评分、基数跟踪和向量维度强制，无需僵化的前期模式
- **统一的查询界面** — 同一个过滤表达式适用于记录、标签、属性和关系

一次 SearchQuery 可同时检索多个视角（记录 + 属性统计 + 聚合），避免了分离式系统架构中常见的 N+1 查询模式。

[阅读完整的 LMPG 架构文章 →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## 文档

| 主题            | 链接                                                           |
| --------------- | -------------------------------------------------------------- |
| 快速教程        | https://docs.rushdb.com/get-started/quick-tutorial             |
| 向量 / 语义搜索 | https://docs.rushdb.com/concepts/search/where#vector-operators |
| 过滤与遍历      | https://docs.rushdb.com/concepts/search/where                  |
| 分组与聚合      | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK  | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK      | https://docs.rushdb.com/python-sdk/introduction                |
| REST API        | https://docs.rushdb.com/rest-api/introduction                  |
| MCP 服务器      | packages/mcp-server/README.md                                  |
| 智能体技能      | packages/skills/README.md                                      |

---

## 何时不应使用 RushDB

- 你需要在极高写入吞吐量下保持亚毫秒级延迟——RushDB 基于 Neo4j 构建，它优先考虑一致性和查询表达力，而非原始写入速度。
- 你只需要扁平的键值存储，没有关系或语义搜索——更简单的存储会更轻量。
- 你需要在数据库层面强制执行僵化的关系型模式——RushDB 是有意设计为无模式的。

---

## 贡献

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

完整指南请参阅 [CONTRIBUTING.md](CONTRIBUTING.md)。欢迎提交 Issue 和 PR。

---

## 许可证

| 路径                    | 许可证              |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

还需要尚未支持的功能？提一个 Issue 吧——欢迎参与设计讨论。
