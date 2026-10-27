<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### आपके records, उनके relationships और उन पर search — सब एक database में।

JSON लिखिए। RushDB types का अनुमान लगाता है, nested objects को एक graph में जोड़ता है, और आपको उसे अपनी JSON query language, SQL, Cypher, Gremlin या Mongo-style documents से query करने देता है — साथ में vector, full-text और hybrid search भी। एक Rust binary, एक data directory, Apache-2.0।

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[वेबसाइट](https://rushdb.com) • [दस्तावेज़](https://docs.rushdb.com) • [मैनुअल](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [Deploy](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • **हिन्दी** • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## Quick start

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

Script release का checksum जाँचता है (और `cosign` installed हो तो उसका Sigstore signature भी), फिर `rushdb` binary को `~/.rushdb/bin` में install करता है। `rushdb up` `http://127.0.0.1:3000` पर एक durable node शुरू करता है; data `./.rushdb` में रहता है और local token `local-token` है।

एक nested JSON document लिखिए। हर nested object अपना अलग record बनता है, जो अपने parent से जुड़ा होता है:

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

फिर Acme के engineers ढूँढिए। Filter related labels का नाम लेकर relationships के पार जाता है:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

आगे [Quickstart](docs/manual/getting-started/quickstart.md) पढ़िए।

## अपने तरीके से query करें

वही records हर interface से जवाब देते हैं।

**SQL**, PostgreSQL wire protocol पर, `psql` या किसी भी PostgreSQL driver से। Labels ही tables हैं:

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher**, HTTP पर, या Neo4j drivers के साथ Bolt पर (`rushdb up --languages sql,cypher --bolt-port 7687`):

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**SDKs** direct mode में (`npm install @rushdb/javascript-sdk`):

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, Mongo-style documents और ISO GQL भी उसी HTTP endpoint का उपयोग करते हैं। [Query languages](docs/manual/languages/README.md) देखें।

## इसमें क्या है

| क्षमता | मतलब |
|---|---|
| **Schema design करने की ज़रूरत नहीं** | किसी भी shape का JSON लिखिए। Types हर value के हिसाब से inferred होते हैं, nested objects जुड़े हुए records बनते हैं, और engine labels, properties और relationships का एक सटीक catalog रखता है जिसे आप query कर सकते हैं। |
| **Properties first-class हैं** | Labeled Meta-Property Graph (LMPG) हर property को एक typed definition के रूप में store करता है, जिसे उसका उपयोग करने वाले records साझा करते हैं — इसलिए schema discovery, value ranges और facets सीधे storage layout से मिलते हैं। |
| **कई query languages** | एक native JSON query language, SQL, Gremlin, SQL/PGQ, Mongo-style documents, और release candidate के रूप में openCypher और ISO GQL। |
| **Search** | आपके दिए vectors पर vector indexes (exact और HNSW), BM25 full-text indexes, और दोनों को मिलाने वाला hybrid search। |
| **Durable और transactional** | Write-ahead log, MVCC snapshots, multi-statement transactions, point-in-time restore वाले backups, और read-only followers तक streaming replication। |
| **Standard protocols** | एक ही process से HTTP, PostgreSQL wire protocol, gRPC और Bolt। |
| **Multitenancy** | वैकल्पिक fail-closed tenants, हर एक का अपना keyspace। |
| **एक binary** | `rushdb serve` server चलाता है; `rushdb up` एक local node। चलाने के लिए कोई दूसरी service नहीं। |

## Deploy

| कहाँ | कैसे |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/), Caddy के पीछे HTTPS variant के साथ |
| Kubernetes | [Helm chart](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| Linux VM | DigitalOcean, Hetzner, AWS, GCP, Azure के लिए [cloud-init](deploy/templates/cloud-init/); [systemd unit](deploy/templates/systemd/) |
| Platforms | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### Install करने के दूसरे तरीके

| तरीका | Command | टिप्पणी |
|---|---|---|
| Install script | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) और macOS, x86-64 और arm64। `sh -s -- --version v3.0.0` से कोई release तय करें। |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS और Linux। `brew services start rushdb` इसे background में चलाता है। |
| Docker | `rushdb/rushdb` | `linux/amd64` और `linux/arm64`; `ghcr.io/rush-db/rushdb` पर भी। |
| Cargo | `cargo binstall rushdb-server` या `cargo install rushdb-server --locked` | `binstall` release binary लाता है; `install` compile करता है (C++ compiler चाहिए)। |
| Release archive | [GitHub releases](https://github.com/rush-db/rushdb/releases) | SHA-256 checksums, SBOM और Sigstore signatures वाले archives। |
| Source | `cargo build --release -p rushdb-server --bin rushdb` | [Installation](docs/manual/getting-started/installation.md) देखें। |

Windows natively supported नहीं है; WSL 2 या Docker इस्तेमाल करें। किसी node को loopback से बाहर खोलने से पहले [Security](docs/manual/operations/security.md) पढ़ें।

## स्थिति

| क्षेत्र | स्थिति |
|---|---|
| Native protocol, SQL, Gremlin text traversals, Mongo-style documents | Stable |
| SQL/PGQ `GRAPH_TABLE` | Stable, read-only |
| openCypher, ISO GQL, Bolt | Release candidate, opt-in: external conformance suites पूरी तरह pass नहीं होते ([विवरण](docs/manual/languages/README.md)) |
| PostgreSQL wire | SQL reads और single-statement writes; `psql`, node-postgres, psycopg और `tokio-postgres` से जाँचा गया, हर client या ORM से नहीं |
| Replication | Read-only followers तक asynchronous streaming; promotion manual है, automatic failover नहीं |
| Raft, storage sharding | Preview, production के लिए नहीं: Raft एक process में in-memory state के साथ चलता है; multi-shard commits crash-atomic नहीं हैं |

[मैनुअल](docs/manual/README.md) हर interface की सटीक सीमाएँ बताता है।

## Connect

| Interface | Default | Enable |
|---|---|---|
| HTTP native protocol | `127.0.0.1:3000`, `POST /protocol/v1/execute` | हमेशा चालू |
| PostgreSQL wire | `127.0.0.1:5432` | `sql` के साथ चालू (default) |
| gRPC (`rushdb.v1.Database/Execute`) | बंद | `--grpc-port` |
| Bolt 5.0 | बंद | `cypher` के साथ `--bolt-port` |

[JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) और [Python](https://pypi.org/project/rushdb/) SDKs, [MCP server](https://www.npmjs.com/package/@rushdb/mcp-server) और [agent skills](https://github.com/rush-db/skills) direct mode में self-hosted engine के साथ काम करते हैं।

## Self-hosted engine और RushDB Cloud

यह engine RushDB का वह हिस्सा है जिसे आप self-host करते हैं। यह embeddings नहीं बनाता, dashboard host नहीं करता और users manage नहीं करता: ये [RushDB Cloud](https://app.rushdb.com) के साथ आते हैं — वह managed service जो इसी engine को चलाती है और dashboard, managed embeddings और AI features (natural-language search, relationship-pattern suggestions) जोड़ती है। SDKs और MCP server दोनों के साथ काम करते हैं: उन्हें RushDB Cloud API key दें, या उन्हें सीधे अपने engine पर point करें (`direct: true`, `engineUrl`) — तब Cloud features को छोड़कर सब कुछ चलता है, और Cloud features `CloudOnlyError` के साथ मना कर दिए जाते हैं।

## Documentation

- [मैनुअल](docs/manual/README.md): शुरुआत, data model, queries, languages, search, operations और reference tables।
- [docs.rushdb.com](https://docs.rushdb.com): tutorials, SDKs और MCP server।
- [RushDB 2.x से upgrade](docs/manual/operations/upgrading-from-2x.md): 2.x Neo4j-आधारित stack था, जो इस repository के `v2` tag पर रखा गया है।

## योगदान

[CONTRIBUTING.md](CONTRIBUTING.md) देखें। Quality gate:

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [Security policy](SECURITY.md)
- [Release process](RELEASING.md)
- [Code of conduct](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

[Apache-2.0](LICENSE) के तहत licensed।
