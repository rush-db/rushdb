<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### AIエージェントとアプリのためのメモリレイヤー。

任意のJSONをプッシュするだけ。エージェントは、ライブでクエリ可能なスキーマ、グラフのリレーションシップ、セマンティック検索を取得できます — すべて自動的に推論されます。
パイプライン不要。別々のストア不要。設計するスキーマも不要。

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 ウェブサイト](https://rushdb.com) • [📖 ドキュメント](https://docs.rushdb.com) • [☁️ クラウド](https://app.rushdb.com) • [🔍 サンプル](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • **日本語** • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## 課題

エージェントにはメモリが必要です。標準的な解決策は3つのデータベースを使うことです。キーバリュー用のRedis、セマンティック検索用のベクトルストア、リレーションシップ用のグラフDB — さらに、それらを同期し続けるためのグルーコードも必要です。

RushDBはこの3つすべてを置き換えます。JSONを一度プッシュするだけ。グラフトラバーサル、セマンティック検索、またはその両方を1回の呼び出しでクエリできます。

| RushDBなしの場合                               | RushDBありの場合                   |
| ---------------------------------------------- | ---------------------------------- |
| Redis + Pinecone + Neo4j + グルーコード        | 1つのAPI                           |
| スキーマ設計 → マイグレーション作成 → 繰り返し | 任意の形状をプッシュ、スキーマ不要 |
| 別個の埋め込みパイプライン                     | サーバーサイドのマネージド埋め込み |
| リレーションシップのエッジを手作業で作成       | データ構造から自動検出             |

---

## クイックスタート

セットアップに応じて2つの方法があります。

- **クラウド** — マネージド、無料枠あり、30秒で稼働。[APIキーを取得 →](https://app.rushdb.com)
- **セルフホスト** — Docker + 自前のNeo4jインスタンス。[セルフホスティングへ →](#self-hosting)

### クラウドの方法

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### エージェントメモリの保存と呼び出し

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

## データの操作

### ネストされたJSONのインポート

任意のJSON形状をプッシュできます。ネストされたオブジェクトやオブジェクトの配列はリンクされたレコードになります — ラベル、型、リレーションシップは書き込み時に推論されます。スキーマもマイグレーションステップも不要です。

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

ネストされた各キー（`DEPARTMENT`、`EMPLOYEE`）はラベルになり、各オブジェクトはレコードに、包含関係はリレーションシップになります — すべて自動的に作成されます。

### CSVのインポート

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

`suggestTypes`は文字列から数値、ブール値、日付を推論します。`skipEmptyValues`は空のセルを空の値として保存する代わりに未設定として扱います（`0`と`false`は保持されます）。

### グラフのトラバーサル

ルートレコードを、その*関連*レコードに対する条件でフィルタリングできます — 任意の深さまで — 単一のクエリで。関連ラベルは`labels`ではなく`where`の**内側**に置きます。

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

### 分析クエリ（集計とグループ化）

`select`は集計（`$sum`、`$avg`、`$count`、`$min`、`$max`）で出力を整形します。`groupBy`はディメンションを制御します。集計に`limit`を追加しないでください — 最初のN件のレコードのみをスキャンし、合計が歪んでしまいます。

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

集計はトラバーサルと組み合わせられます — 例えば部門ごとの人員数と給与総額：

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

## Claude、Cursor、または任意のMCPクライアントへの接続

RushDBはMCPサーバーを同梱しています。エージェントは永続的で構造化されたメモリを — そのまま利用できます。

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

これをClaude Desktop、Cursor、またはWindsurfのMCP設定に配置してください。これでエージェントは、レコードの作成、意味による検索、リレーションシップのトラバーサル、スキーマのイントロスペクションが可能になります — すべて自然言語で行えます。

---

## 同梱されているもの

| 機能                           | その意味                                                                                                                                                                  |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **マネージド埋め込み**         | 任意の文字列プロパティを一度インデックス化すれば — すべての書き込みがサーバーサイドで自動的に埋め込まれます                                                               |
| **1クエリでグラフ + ベクトル** | セマンティックな類似性とリレーションシップのトラバーサルを1回の呼び出しで組み合わせられます                                                                               |
| **ゼロスキーマ**               | 任意のJSON形状をプッシュ。RushDBが型を推論し、プロパティを作成し、レコードをリンクします                                                                                  |
| **推論されたスキーマ**         | プロパティがファーストクラスのノードになります — 型、ラベル、リレーションシップが書き込み時に発見されるため、データが到着するにつれクエリ可能なスキーマが自ら構築されます |
| **ACIDトランザクション**       | 同時実行のエージェントが共有メモリを破損させません。内部ではNeo4jを使用しています                                                                                         |
| **自己記述的**                 | エージェントは推論されたスキーマ — ラベル、プロパティ、値の範囲 — をイントロスペクトして、安全にクエリできる内容を把握します                                              |
| **MCPネイティブ**              | ディスカバリーファーストのクエリプロンプトを組み込んだ完全なMCPサーバー                                                                                                   |
| **エージェントスキル**         | インストール可能な`@rushdb/skills`パッケージ — スキル対応の任意のエージェントに、RushDBでのクエリ、モデリング、記憶を1コマンドで教えられます                              |
| **統一クエリAPI**              | グラフ、ベクトル、集計、イントロスペクションを1つのJSON形状で                                                                                                             |
| **セルフホストまたはクラウド** | Docker + 自前のNeo4j、またはマネージドクラウド。完全なデータ所有権                                                                                                        |

---

## ユースケース

| ユースケース               | RushDBが置き換えるもの            | 主要API                                                               |
| -------------------------- | --------------------------------- | --------------------------------------------------------------------- |
| **エージェントメモリ**     | Redis + ベクトルストア + グラフDB | `db.records.vectorSearch({ query, where: { agent_id } })`             |
| **コンテキスト付きRAG**    | フラットなベクトルストア          | `db.records.find({ where, labels })` + リレーションシップトラバーサル |
| **スキーマフリーアプリ**   | Postgres + マイグレーション + ETL | `db.records.importJson(nestedJson)`                                   |
| **コネクテッドデータ製品** | 複数の結合されたサービス          | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })`         |

---

## セルフホスティング

> **セルフホストの方法** — 自前のインフラでRushDBを実行します。APOCプラグイン付きのNeo4j 2026.01.4以降が必要です。

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
<summary>環境変数の一覧</summary>

| 名前                              | 説明                                               | 必須       | デフォルト |
| --------------------------------- | -------------------------------------------------- | ---------- | ---------- |
| `NEO4J_URL`                       | Neo4j接続URL                                       | はい       | —          |
| `NEO4J_USERNAME`                  | Neo4jユーザー名                                    | はい       | neo4j      |
| `NEO4J_PASSWORD`                  | Neo4jパスワード                                    | はい       | —          |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | APIトークン暗号化用の32文字キー                    | はい(本番) | —          |
| `RUSHDB_PORT`                     | HTTPポート                                         | いいえ     | 3000       |
| `RUSHDB_LOGIN`                    | 管理者ログイン                                     | いいえ     | admin      |
| `RUSHDB_PASSWORD`                 | 管理者パスワード                                   | いいえ     | password   |
| `RUSHDB_BASE_URL`                 | synx割り当て用のパブリック/ベースAPI URL           | いいえ     | —          |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | マネージドsynxワーカー用の内部トークン             | いいえ     | —          |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | synxデスティネーション書き込み用の内部書き込みキー | いいえ     | —          |

マネージドsynxワーカーはデーモンとして実行されます。実行可能なコネクタをポーリングし、コネクタのリースを更新し、グレースフルシャットダウン時にリースを解放し、クラッシュ後にplatform/coreが期限切れのリースを回収できるようにします。

</details>

<details>
<summary>ローカル開発（バンドルされたNeo4j）</summary>

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
<summary>アーキテクチャ：RushDBはどのようにデータを構造化するか（LMPG）</summary>

RushDBは**Labeled Meta Property Graph (LMPG)** モデルを使用します。プロパティはファーストクラスのグラフノード（「HyperProperties」）に昇格されます — レコードに付随する単なるキーバリューペアではありません。

これが意味すること：

- **事前設計なしのスキーマ** — プロパティがグラフノードであるため、スキーマは*設計されるのではなく、データから推論されます*。ラベル、型、値の範囲、リレーションシップのトポロジーが書き込み時に発見され、即座にクエリ可能になります — 手作業のスキーマモデリングも、RDF/OWLツールチェーンも不要です
- **自動検出されるリレーションシップ** — プロパティを共有するレコードは、エッジを手作りすることなくリンクされます
- **スキーマイントロスペクション** — エージェントは、ラベル、プロパティ型、値の範囲、リレーションシップのトポロジーを1つのクエリで列挙できます
- **ソフト制約** — 厳格な事前スキーマなしでの型凝集度スコアリング、カーディナリティ追跡、ベクトル次元の強制
- **統一クエリサーフェス** — 同じフィルタ式が、レコード、ラベル、プロパティ、リレーションシップにわたって機能します

1つのSearchQueryで複数の視点を同時に取得できます（レコード + プロパティ統計 + 集計）。これにより、別々のシステムアーキテクチャでよく見られるN+1の検査パターンを回避できます。

[LMPGアーキテクチャの記事全文を読む →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## ドキュメント

| トピック                      | リンク                                                         |
| ----------------------------- | -------------------------------------------------------------- |
| クイックチュートリアル        | https://docs.rushdb.com/get-started/quick-tutorial             |
| ベクトル / セマンティック検索 | https://docs.rushdb.com/concepts/search/where#vector-operators |
| フィルタリングとトラバーサル  | https://docs.rushdb.com/concepts/search/where                  |
| グループ化と集計              | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK                | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK                    | https://docs.rushdb.com/python-sdk/introduction                |
| REST API                      | https://docs.rushdb.com/rest-api/introduction                  |
| MCPサーバー                   | packages/mcp-server/README.md                                  |
| エージェントスキル            | packages/skills/README.md                                      |

---

## RushDBを使うべきでない場合

- 非常に高い書き込みスループットでサブミリ秒のレイテンシが必要な場合 — RushDBはNeo4j上に構築されており、生の書き込み速度よりも一貫性とクエリの表現力を優先します。
- リレーションシップやセマンティック検索のないフラットなキーバリューストレージだけが必要な場合 — よりシンプルなストアのほうが軽量です。
- データベースレベルで強制される厳格なリレーショナルスキーマが必要な場合 — RushDBは意図的にスキーマフリーです。

---

## コントリビューション

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

完全なガイドラインについては[CONTRIBUTING.md](CONTRIBUTING.md)を参照してください。IssueやPRを歓迎します。

---

## ライセンス

| パス                    | ライセンス          |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

まだサポートされていない機能が必要ですか？Issueを開いてください — 設計に関する議論を歓迎します。
