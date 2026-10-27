<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### 一个数据库，存放你的记录、它们之间的关系，并在其上进行搜索。

写入 JSON。RushDB 自动推断类型，把嵌套对象连接成图，并让你用它的 JSON 查询语言、SQL、Cypher、Gremlin 或 Mongo 风格文档进行查询，还支持向量、全文和混合搜索。一个 Rust 二进制文件，一个数据目录，Apache-2.0 许可。

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[官网](https://rushdb.com) • [文档](https://docs.rushdb.com) • [手册](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [部署](deploy/README.md)

[English](README.md) • **中文** • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## 快速开始

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

安装脚本会校验发布包的 SHA-256 校验和（安装了 `cosign` 时还会校验 Sigstore 签名），然后把 `rushdb` 二进制文件安装到 `~/.rushdb/bin`。`rushdb up` 会在 `http://127.0.0.1:3000` 启动一个持久化节点，数据保存在 `./.rushdb`，本地令牌为 `local-token`。

写入一个嵌套的 JSON 文档。每个嵌套对象都会成为独立的记录，并与其父记录相连：

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

然后查找 Acme 的工程师。过滤条件通过写出相关标签跨越关系：

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

接下来请阅读[快速入门](docs/manual/getting-started/quickstart.md)。

## 用你习惯的方式查询

同一批记录可以通过每一种接口访问。

**SQL**：通过 PostgreSQL 线协议，使用 `psql` 或任意 PostgreSQL 驱动。标签就是表：

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher**：通过 HTTP，或通过 Bolt 使用 Neo4j 驱动（`rushdb up --languages sql,cypher --bolt-port 7687`）：

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**SDK** 的直连模式（`npm install @rushdb/javascript-sdk`）：

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB(process.env.RUSHDB_TOKEN, { direct: true, engineUrl: 'http://127.0.0.1:3000' })

const engineers = await db.records.find({
  labels: ['EMPLOYEE'],
  where: { role: { $contains: 'Engineer' }, DEPARTMENT: { COMPANY: { name: 'Acme' } } },
  orderBy: { name: 'asc' }
})
console.log(engineers.data.map((employee) => employee.data.name)) // [ 'Ada', 'Linus' ]
```

Gremlin、SQL/PGQ `GRAPH_TABLE`、Mongo 风格文档和 ISO GQL 使用同一个 HTTP 端点。参见[查询语言](docs/manual/languages/README.md)。

## 功能一览

| 能力 | 含义 |
|---|---|
| **无需设计 schema** | 写入任意形状的 JSON。类型按值推断，嵌套对象成为相互连接的记录，引擎维护一份精确的标签、属性和关系目录，并且可以查询。 |
| **属性是一等公民** | 标签化元属性图（Labeled Meta-Property Graph，LMPG）把每个属性存储为一个带类型的定义，由使用它的记录共享，因此 schema 发现、取值范围和分面统计直接来自存储布局本身。 |
| **多种查询语言** | 原生 JSON 查询语言、SQL、Gremlin、SQL/PGQ、Mongo 风格文档，以及处于候选发布阶段的 openCypher 和 ISO GQL。 |
| **搜索** | 基于你提供的向量的向量索引（精确和 HNSW）、BM25 全文索引，以及融合两者的混合搜索。 |
| **持久且支持事务** | 预写日志、MVCC 快照、多语句事务、支持时间点恢复的备份，以及到只读从节点的流式复制。 |
| **标准协议** | 一个进程同时提供 HTTP、PostgreSQL 线协议、gRPC 和 Bolt。 |
| **多租户** | 可选的故障即拒绝（fail-closed）租户模式，每个租户拥有独立的键空间。 |
| **一个二进制文件** | `rushdb serve` 运行服务器；`rushdb up` 运行本地节点。无需运维其他服务。 |

## 部署

| 环境 | 方式 |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/)，另有通过 Caddy 提供 HTTPS 的版本 |
| Kubernetes | [Helm chart](charts/rushdb/)：`helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| Linux 虚拟机 | 适用于 DigitalOcean、Hetzner、AWS、GCP、Azure 的 [cloud-init](deploy/templates/cloud-init/)；[systemd 单元](deploy/templates/systemd/) |
| 平台 | [Fly.io](deploy/templates/fly/)、[Render](deploy/templates/render/)、[Coolify](deploy/templates/coolify/)、[Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### 其他安装方式

| 方式 | 命令 | 说明 |
|---|---|---|
| 安装脚本 | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux（glibc 2.28+）和 macOS，x86-64 与 arm64。`sh -s -- --version v3.0.0` 可固定版本。 |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS 和 Linux。`brew services start rushdb` 在后台运行。 |
| Docker | `rushdb/rushdb` | `linux/amd64` 和 `linux/arm64`；也可用 `ghcr.io/rush-db/rushdb`。 |
| Cargo | `cargo binstall rushdb-server` 或 `cargo install rushdb-server --locked` | `binstall` 下载发布的二进制；`install` 从源码编译（需要 C++ 编译器）。 |
| 发布包 | [GitHub releases](https://github.com/rush-db/rushdb/releases) | 带 SHA-256 校验和、SBOM 和 Sigstore 签名的压缩包。 |
| 源码 | `cargo build --release -p rushdb-server --bin rushdb` | 参见[安装](docs/manual/getting-started/installation.md)。 |

不原生支持 Windows；请使用 WSL 2 或 Docker。在把节点暴露到本机以外之前，请阅读[安全](docs/manual/operations/security.md)。

## 状态

| 领域 | 状态 |
|---|---|
| 原生协议、SQL、Gremlin 文本遍历、Mongo 风格文档 | 稳定 |
| SQL/PGQ `GRAPH_TABLE` | 稳定，只读 |
| openCypher、ISO GQL、Bolt | 候选发布，需手动启用：外部一致性测试套件尚未全部通过（[详情](docs/manual/languages/README.md)） |
| PostgreSQL 线协议 | SQL 读取和单语句写入；已用 `psql`、node-postgres、psycopg 和 `tokio-postgres` 验证，并非所有客户端或 ORM |
| 复制 | 到只读从节点的异步流式复制；需手动提升，没有自动故障转移 |
| Raft、存储分片 | 预览，不适合生产：Raft 在单进程内运行且状态在内存中；跨分片提交不具备崩溃原子性 |

[手册](docs/manual/README.md)记录了每个接口的确切限制。

## 连接

| 接口 | 默认 | 启用方式 |
|---|---|---|
| HTTP 原生协议 | `127.0.0.1:3000`，`POST /protocol/v1/execute` | 始终开启 |
| PostgreSQL 线协议 | `127.0.0.1:5432` | 启用 `sql` 时开启（默认） |
| gRPC（`rushdb.v1.Database/Execute`） | 关闭 | `--grpc-port` |
| Bolt 5.0 | 关闭 | `--bolt-port` 并启用 `cypher` |

[JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) 和 [Python](https://pypi.org/project/rushdb/) SDK、[MCP 服务器](https://www.npmjs.com/package/@rushdb/mcp-server)以及[智能体技能](https://github.com/rush-db/skills)都能以直连模式使用自托管引擎。

## 自托管引擎与 RushDB Cloud

这个引擎是 RushDB 中可以自托管的部分。它不生成嵌入、不提供控制台，也不管理用户：这些由 [RushDB Cloud](https://app.rushdb.com) 提供，它是运行此引擎的托管服务，并增加了控制台、托管嵌入和 AI 功能（自然语言搜索、关系模式建议）。SDK 和 MCP 服务器两者都能使用：给它们一个 RushDB Cloud API 密钥，或者让它们直连你的引擎（`direct: true`、`engineUrl`），这时除 Cloud 专属功能外都可使用，Cloud 专属功能会以 `CloudOnlyError` 拒绝。

## 文档

- [手册](docs/manual/README.md)：入门、数据模型、查询、语言、搜索、运维和参考表。
- [docs.rushdb.com](https://docs.rushdb.com)：教程、SDK 和 MCP 服务器。
- [从 RushDB 2.x 升级](docs/manual/operations/upgrading-from-2x.md)：2.x 是基于 Neo4j 的版本，保留在本仓库的 `v2` 标签。

## 参与贡献

参见 [CONTRIBUTING.md](CONTRIBUTING.md)。质量门禁：

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [安全策略](SECURITY.md)
- [发布流程](RELEASING.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [变更日志](CHANGELOG.md)

采用 [Apache-2.0](LICENSE) 许可。
