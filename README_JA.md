<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### レコードとその関係、そしてその上での検索を、ひとつのデータベースで。

JSON を書き込むだけ。RushDB が型を推論し、ネストしたオブジェクトをグラフとしてつなぎ、独自の JSON クエリ言語、SQL、Cypher、Gremlin、Mongo 形式のドキュメントで問い合わせられるようにします。ベクトル検索、全文検索、ハイブリッド検索にも対応。Rust のバイナリひとつ、データディレクトリひとつ、Apache-2.0。

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[ウェブサイト](https://rushdb.com) • [ドキュメント](https://docs.rushdb.com) • [マニュアル](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [デプロイ](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • **日本語** • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## クイックスタート

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

インストールスクリプトはリリースのチェックサムを確認し（`cosign` があれば Sigstore 署名も確認）、`rushdb` バイナリを `~/.rushdb/bin` に入れます。`rushdb up` は `http://127.0.0.1:3000` で永続ノードを起動し、データを `./.rushdb` に保存します。ローカル用トークンは `local-token` です。

ネストした JSON ドキュメントを書き込みます。ネストしたオブジェクトはそれぞれ独立したレコードになり、親とつながります：

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

次に Acme のエンジニアを探します。関連するラベルを書くだけで、フィルターが関係をたどります：

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

続きは[クイックスタート](docs/manual/getting-started/quickstart.md)へ。

## 好きな方法で問い合わせる

同じレコードに、どのインターフェースからでもアクセスできます。

**SQL**：PostgreSQL ワイヤプロトコル経由で、`psql` や任意の PostgreSQL ドライバーから。ラベルがテーブルになります：

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher**：HTTP 経由、または Neo4j ドライバーで Bolt 経由（`rushdb up --languages sql,cypher --bolt-port 7687`）：

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**SDK** のダイレクトモード（`npm install @rushdb/javascript-sdk`）：

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

Gremlin、SQL/PGQ `GRAPH_TABLE`、Mongo 形式のドキュメント、ISO GQL も同じ HTTP エンドポイントを使います。詳しくは[クエリ言語](docs/manual/languages/README.md)を参照してください。

## できること

| 機能 | 内容 |
|---|---|
| **スキーマ設計が不要** | どんな形の JSON でも書き込めます。型は値ごとに推論され、ネストしたオブジェクトはつながったレコードになり、エンジンはラベル・プロパティ・関係の正確なカタログを保持し、それを問い合わせられます。 |
| **プロパティが第一級** | Labeled Meta-Property Graph（LMPG）は、各プロパティを、それを使うレコード間で共有される型付きの定義として保存します。そのため、スキーマの発見、値の範囲、ファセットはストレージのレイアウトそのものから得られます。 |
| **多様なクエリ言語** | ネイティブの JSON クエリ言語、SQL、Gremlin、SQL/PGQ、Mongo 形式のドキュメント、リリース候補の openCypher と ISO GQL。 |
| **検索** | 指定したベクトルに対するベクトルインデックス（厳密検索と HNSW）、BM25 全文インデックス、両者を融合するハイブリッド検索。 |
| **耐久性とトランザクション** | ログ先行書き込み、MVCC スナップショット、複数ステートメントのトランザクション、ポイントインタイムリストア付きのバックアップ、読み取り専用フォロワーへのストリーミングレプリケーション。 |
| **標準プロトコル** | HTTP、PostgreSQL ワイヤプロトコル、gRPC、Bolt をひとつのプロセスで提供。 |
| **マルチテナンシー** | オプションのフェイルクローズ型テナント。テナントごとに独立したキー空間を持ちます。 |
| **バイナリひとつ** | `rushdb serve` がサーバーを、`rushdb up` がローカルノードを起動します。運用すべき別サービスはありません。 |

## デプロイ

| 環境 | 方法 |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/)。Caddy による HTTPS 版もあります |
| Kubernetes | [Helm チャート](charts/rushdb/)：`helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| Linux VM | DigitalOcean、Hetzner、AWS、GCP、Azure 向けの [cloud-init](deploy/templates/cloud-init/)、[systemd ユニット](deploy/templates/systemd/) |
| プラットフォーム | [Fly.io](deploy/templates/fly/)、[Render](deploy/templates/render/)、[Coolify](deploy/templates/coolify/)、[Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### その他のインストール方法

| 方法 | コマンド | 備考 |
|---|---|---|
| インストールスクリプト | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux（glibc 2.28 以上）と macOS、x86-64 と arm64。`sh -s -- --version v3.0.0` でバージョンを固定できます。 |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS と Linux。`brew services start rushdb` でバックグラウンド実行します。 |
| Docker | `rushdb/rushdb` | `linux/amd64` と `linux/arm64`。`ghcr.io/rush-db/rushdb` でも配布しています。 |
| Cargo | `cargo binstall rushdb-server` または `cargo install rushdb-server --locked` | `binstall` はリリース済みバイナリを取得し、`install` はソースからビルドします（C++ コンパイラが必要）。 |
| リリースアーカイブ | [GitHub releases](https://github.com/rush-db/rushdb/releases) | SHA-256 チェックサム、SBOM、Sigstore 署名付きのアーカイブ。 |
| ソース | `cargo build --release -p rushdb-server --bin rushdb` | [インストール](docs/manual/getting-started/installation.md)を参照。 |

Windows はネイティブでは未対応です。WSL 2 か Docker を使ってください。ノードをループバック外に公開する前に[セキュリティ](docs/manual/operations/security.md)を読んでください。

## ステータス

| 領域 | ステータス |
|---|---|
| ネイティブプロトコル、SQL、Gremlin テキストトラバーサル、Mongo 形式のドキュメント | 安定版 |
| SQL/PGQ `GRAPH_TABLE` | 安定版、読み取り専用 |
| openCypher、ISO GQL、Bolt | リリース候補、明示的に有効化：外部の適合性テストスイートはすべては通過していません（[詳細](docs/manual/languages/README.md)） |
| PostgreSQL ワイヤプロトコル | SQL の読み取りと単一ステートメントの書き込み。`psql`、node-postgres、psycopg、`tokio-postgres` で確認済み。すべてのクライアントや ORM ではありません |
| レプリケーション | 読み取り専用フォロワーへの非同期ストリーミング。昇格は手動で、自動フェイルオーバーはありません |
| Raft、ストレージシャーディング | プレビュー、本番利用不可：Raft は単一プロセス内でメモリ上の状態で動作し、複数シャードにまたがるコミットはクラッシュに対してアトミックではありません |

各インターフェースの正確な制限は[マニュアル](docs/manual/README.md)に記載しています。

## 接続

| インターフェース | デフォルト | 有効化 |
|---|---|---|
| HTTP ネイティブプロトコル | `127.0.0.1:3000`、`POST /protocol/v1/execute` | 常に有効 |
| PostgreSQL ワイヤプロトコル | `127.0.0.1:5432` | `sql` 有効時（デフォルト） |
| gRPC（`rushdb.v1.Database/Execute`） | 無効 | `--grpc-port` |
| Bolt 5.0 | 無効 | `--bolt-port` と `cypher` |

[JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) と [Python](https://pypi.org/project/rushdb/) の SDK、[MCP サーバー](https://www.npmjs.com/package/@rushdb/mcp-server)、[エージェントスキル](https://github.com/rush-db/skills)は、ダイレクトモードでセルフホストのエンジンと連携します。

## セルフホストのエンジンと RushDB Cloud

このエンジンは RushDB のうちセルフホストできる部分です。埋め込みの生成、ダッシュボードの提供、ユーザー管理は行いません。それらは [RushDB Cloud](https://app.rushdb.com) が提供します。RushDB Cloud はこのエンジンを運用するマネージドサービスで、ダッシュボード、マネージド埋め込み、AI 機能（自然言語検索、関係パターンの提案）を加えます。SDK と MCP サーバーはどちらとも使えます。RushDB Cloud の API キーを渡すか、エンジンに直接向けてください（`direct: true`、`engineUrl`）。その場合 Cloud 専用機能以外はすべて使え、Cloud 専用機能は `CloudOnlyError` で拒否されます。

## ドキュメント

- [マニュアル](docs/manual/README.md)：はじめに、データモデル、クエリ、言語、検索、運用、リファレンス。
- [docs.rushdb.com](https://docs.rushdb.com)：チュートリアル、SDK、MCP サーバー。
- [RushDB 2.x からのアップグレード](docs/manual/operations/upgrading-from-2x.md)：2.x は Neo4j ベースの構成で、このリポジトリの `v2` タグに残っています。

## コントリビュート

[CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。品質ゲート：

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [セキュリティポリシー](SECURITY.md)
- [リリース手順](RELEASING.md)
- [行動規範](CODE_OF_CONDUCT.md)
- [変更履歴](CHANGELOG.md)

[Apache-2.0](LICENSE) ライセンスで提供しています。
