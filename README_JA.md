<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="RushDB logo" width="96">

# RushDB

<sub>AI のためのデータベース</sub>

**グラフ、ベクトル、分析を同じレコードの上で。書き込みのたびにラベル、フィールド、値のライブスキーマが更新されるので、エージェントは推測ではなく実在するフィールドに対してクエリを書けます。**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[ドキュメント](https://docs.rushdb.com) • [クイックスタート](https://docs.rushdb.com/engine/getting-started/quickstart/) • [ディスカッション](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • **日本語** • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

データは手元にある形のまま取り込めます。ネストした JSON、CSV、SQL の `INSERT`、
Cypher や Mongo 形式の書き込み、あるいはお使いのデータベースからの変更ストリーム。
RushDB はそれを型付きのレコードとして保存し、グラフとしてつなぎます。同じレコードが
リレーションのクエリ、ベクトル検索と全文検索、集計に答えるので、同期を保たなければ
ならない 2 つ目のストアは必要ありません。書き込みのたびに、存在するラベル、フィールド、
型、値のカタログも更新されるので、エージェントは実在するフィールドをクエリします。
HTTP、PostgreSQL ワイヤプロトコル、Bolt、gRPC、MCP で接続でき、SDK からでも任意の
PostgreSQL クライアントからでも使えます。Rust のバイナリひとつ、Apache-2.0。

| グラフ | ベクトル検索と全文検索 | 分析 |
|---|---|---|
| ネストした JSON はつながったレコードになります。1 つのクエリでリレーションをまたいで絞り込み、型と向きを持つリレーションをたどれます。 | ベクトル検索（厳密検索と HNSW）、BM25 全文検索、ハイブリッド検索。どれも同じグラフ述語で絞り込めます。 | `groupBy` による件数、合計、平均、最小、最大。または PostgreSQL ワイヤプロトコル経由で `GROUP BY` を使った素の SQL。 |

## クイックスタート

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` は `http://127.0.0.1:3000` で待ち受け、データを `./.rushdb` に保存し、
トークンは `local-token` です。別のターミナルで `pip install rushdb` を実行します：

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
})

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

スキーマはレコードを走査して作るのではなく、カタログから得られます：

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
```

<details>
<summary>同じ内容を TypeScript で</summary>

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
  }
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

SQL に慣れていますか？ ラベルがそのままテーブルです：

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

次へ：[完全版のクイックスタート](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker とその他のインストール方法](https://docs.rushdb.com/engine/getting-started/installation/)

## 作れるもの

- **エージェントのメモリ。** メモリをリレーション付きのレコードとして保存し、スコープで絞り込み、保存したベクトルで順位付けします。OpenClaw と Hermes 向けのプロバイダーをすぐに使えます。
- **アプリのデータを検索。** アプリがすでに書き込んでいるレコードに対する全文検索、ベクトル検索、ハイブリッド検索。同じクエリの中でグラフによる絞り込みもできます。
- **つながったデータへの問い合わせ。** エージェントがスキーマを読み、あなたが確認できるクエリを書きます。JSON クエリ言語、SQL、Cypher のいずれかです。
- **エクスポート不要のレポート。** アプリが書き込んだレコードを、SDK や任意の PostgreSQL クライアントからグループ化・集計できます。

## 機能

- **創発的なスキーマ。** 型は値ごとに推論され、ネストしたオブジェクトはつながったレコードになり、カタログは正確な件数と値の一覧を保持します。[スキーマカタログ](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **グラフクエリ。** ラベルを指定してリレーションをまたいで絞り込み、型と向きでリレーションを限定し、関連レコードを射影や集計に使えるよう束縛します。[リレーションとトラバーサル](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **検索を内蔵。** あなたが渡すベクトルに対するベクトルインデックス、BM25 全文検索、そしてその両方を融合するハイブリッド検索。[インデックスと検索](https://docs.rushdb.com/engine/indexes-and-search/)
- **集計。** `select` と `groupBy` による件数、合計、平均、時間バケット、ネストしたコレクション。[集計](https://docs.rushdb.com/engine/query/aggregations/)
- **使い慣れた言語で。** PostgreSQL ワイヤプロトコル上の SQL、Gremlin、SQL/PGQ、Mongo 形式のドキュメント、そしてリリース候補として openCypher と ISO GQL。[クエリ言語](https://docs.rushdb.com/engine/languages/)
- **耐久性。** ログ先行書き込み、トランザクション、ポイントインタイムリストア対応のバックアップ、リードレプリカ。[運用](https://docs.rushdb.com/engine/operations/)
- **バイナリひとつ。** HTTP、PostgreSQL ワイヤ、gRPC、Bolt をひとつのプロセスから提供し、分離されたテナントもオプションで使えます。[接続](https://docs.rushdb.com/engine/connectivity/)

## エコシステム

| プロジェクト | 用途 |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | TypeScript と Python のアプリ |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude、Cursor などの MCP クライアント |
| [エージェントスキル](https://github.com/rush-db/skills) · [エージェントプラグイン](https://github.com/rush-db/agent-plugins) | コーディングエージェント |
| [LangChain と LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | 検索、メモリ、プロパティグラフストア |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [メモリコントラクト](https://github.com/rush-db/agent-memory-contract) | エージェントメモリのプロバイダー |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [Obsidian プラグイン](https://github.com/rush-db/obsidian-plugin) | 変更データキャプチャ、ノート |

どれもセルフホストのエンジンで動きます。コーディングエージェントに
RushDB をセットアップさせるなら、[rushdb.com/agent-setup](https://rushdb.com/agent-setup) を渡してください。

## どこでも動かせる

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data \
  -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose、Kubernetes（Helm）、cloud-init、systemd、Fly.io、Render、
Coolify、Dokploy については [デプロイ](https://docs.rushdb.com/deploy/) を参照してください。
ノードをループバックの外に公開する前に、[セキュリティ](https://docs.rushdb.com/engine/operations/security/) を読んでください。

## セルフホストと RushDB Cloud

このリポジトリはエンジンそのもので、すべてをセルフホストできます。[RushDB Cloud](https://app.rushdb.com) は同じエンジンを代わりに運用し、下の表で示す機能を追加します。SDK と MCP サーバーはどちらでも使えます。セルフホストのエンジンでは、Cloud 専用の呼び出しは何も送信する前に `CloudOnlyError` で失敗します。

| 機能 | セルフホストのエンジン | RushDB Cloud |
|---|---|---|
| エンジンと API：レコード、グラフクエリ、検索、集計、ライブスキーマ | ✓ | ✓ |
| SDK と MCP サーバー | ✓ ダイレクトモード、MCP は stdio 経由 | ✓ OAuth 対応のホスト型 MCP も |
| JSON と CSV のインポート | ✓ | ✓ |
| レコード、グラフビュー、クエリビルダーを備えた[ダッシュボード](https://docs.rushdb.com/rushdb-cloud/) | SDK または任意の PostgreSQL クライアントを使用 | ✓ |
| [マネージドな埋め込み生成](https://docs.rushdb.com/rushdb-cloud/capabilities/) | 自分のモデルのベクトルを持ち込む | ✓ 月間枠 |
| [自然言語検索](https://docs.rushdb.com/learn/search-query/smart-search/) | クエリを自分で、またはエージェントで組み立てる | ✓ 月間枠 |
| [リレーションの提案](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | リレーションを自分で作成 | ✓ |
| 指標とフィルターの[定義](https://docs.rushdb.com/learn/search-query/definitions/) | ルールを自分のプロンプトと SQL で管理 | ✓ |
| SaaS ツールへの[接続](https://docs.rushdb.com/learn/data-sources/connections/)と、オンプレミスのデータベース向けの RushDB Sync | 自前の同期コード、または Debezium シンク | ✓ |
| プロジェクトと API キー | テナントとトークンは自分で管理 | ✓ |
| チームでのアクセス | — | 有料プラン |
| バックアップ | 自分で運用：バックアップ、ポイントインタイムリストア、レプリカ | 有料プラン |

RushDB Cloud 列の ✓ は Free プランに含まれることを示します。プランと利用枠は [rushdb.com/pricing](https://rushdb.com/pricing) を参照してください。

## FAQ

**本番環境で使えますか？** SQL、ネイティブの JSON クエリ言語、Gremlin、
Mongo 形式のドキュメントは安定版です。openCypher、ISO GQL、Bolt はリリース候補です。
レプリケーションは非同期で、昇格は手動です。
[ステータスページ](https://docs.rushdb.com/engine/reference/status/) に、
すべてのインターフェースとその制限が載っています。

**別途ベクトルデータベースは必要ですか？** いいえ。ベクトル、テキストインデックス、
リレーションはひとつのエンジンにあり、レコードとそのベクトルを 1 回のコミットで一緒に書き込めます。
エンジンはあなたが渡したベクトルを検索します。RushDB
Cloud ではベクトルを生成させることもできます。

**ライブスキーマとは？** すべてのラベル、プロパティ、型、値、リレーションの形を網羅したカタログで、
コミットのたびに更新され、正確な件数を持ちます。1 回の呼び出しで取得できるので、
エージェントやツールがフィールド名を推測する必要はありません。
[スキーマカタログ](https://docs.rushdb.com/engine/data-model/schema-catalog/)

**Windows は？** WSL 2 か Docker を使ってください。

---

[コントリビュート](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[セキュリティ](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[変更履歴](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x（レガシー、Neo4j ベース）は
[`2.x` ブランチ](https://github.com/rush-db/rushdb/tree/2.x) にあり、
2027-10-28 までセキュリティ修正が提供されます。[2.x からのアップグレード](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
