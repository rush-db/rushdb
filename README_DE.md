<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### Eine Datenbank für deine Datensätze, ihre Beziehungen und die Suche darüber.

Schreib JSON. RushDB leitet die Typen ab, verknüpft verschachtelte Objekte zu einem Graphen und lässt dich ihn mit seiner JSON-Abfragesprache, SQL, Cypher, Gremlin oder Mongo-artigen Dokumenten abfragen, dazu Vektor-, Volltext- und hybride Suche. Ein Rust-Binary, ein Datenverzeichnis, Apache-2.0.

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[Website](https://rushdb.com) • [Dokumentation](https://docs.rushdb.com) • [Handbuch](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [Deployment](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • **Deutsch** • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## Schnellstart

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

Das Skript prüft die Prüfsumme des Releases (und seine Sigstore-Signatur, wenn `cosign` installiert ist) und installiert das `rushdb`-Binary nach `~/.rushdb/bin`. `rushdb up` startet einen dauerhaften Knoten auf `http://127.0.0.1:3000` mit den Daten in `./.rushdb` und dem lokalen Token `local-token`.

Schreib ein verschachteltes JSON-Dokument. Jedes verschachtelte Objekt wird zu einem eigenen Datensatz, der mit seinem Elternteil verknüpft ist:

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

Dann finde die Engineers bei Acme. Der Filter folgt Beziehungen, indem er die verknüpften Labels nennt:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

Weiter geht es mit dem [Quickstart](docs/manual/getting-started/quickstart.md).

## Frag ab, wie du willst

Dieselben Datensätze antworten über jede Schnittstelle.

**SQL** über das PostgreSQL-Wire-Protokoll, mit `psql` oder jedem PostgreSQL-Treiber. Labels sind Tabellen:

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher** über HTTP oder über Bolt mit den Neo4j-Treibern (`rushdb up --languages sql,cypher --bolt-port 7687`):

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**Die SDKs** im Direct Mode (`npm install @rushdb/javascript-sdk`):

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, Mongo-artige Dokumente und ISO GQL nutzen denselben HTTP-Endpunkt. Siehe [Abfragesprachen](docs/manual/languages/README.md).

## Was drin ist

| Fähigkeit | Bedeutung |
|---|---|
| **Kein Schema zu entwerfen** | Schreib JSON in beliebiger Form. Typen werden pro Wert abgeleitet, verschachtelte Objekte werden zu verknüpften Datensätzen, und die Engine führt einen exakten, abfragbaren Katalog der Labels, Eigenschaften und Beziehungen. |
| **Eigenschaften sind erstklassig** | Der Labeled Meta-Property Graph (LMPG) speichert jede Eigenschaft als typisierte Definition, die sich die nutzenden Datensätze teilen. Schema-Erkennung, Wertebereiche und Facetten ergeben sich so aus dem Speicherlayout selbst. |
| **Viele Abfragesprachen** | Eine native JSON-Abfragesprache, SQL, Gremlin, SQL/PGQ, Mongo-artige Dokumente sowie openCypher und ISO GQL als Release Candidates. |
| **Suche** | Vektorindizes (exakt und HNSW) über die Vektoren, die du lieferst, BM25-Volltextindizes und hybride Suche, die beides kombiniert. |
| **Dauerhaft und transaktional** | Write-Ahead-Log, MVCC-Snapshots, Transaktionen über mehrere Anweisungen, Backups mit Point-in-Time-Restore und Streaming-Replikation auf schreibgeschützte Follower. |
| **Standardprotokolle** | HTTP, das PostgreSQL-Wire-Protokoll, gRPC und Bolt aus einem Prozess. |
| **Mandantenfähigkeit** | Optionale Fail-closed-Mandanten mit je eigenem Keyspace. |
| **Ein Binary** | `rushdb serve` startet den Server, `rushdb up` einen lokalen Knoten. Kein weiterer Dienst zu betreiben. |

## Deployment

| Wo | Wie |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/), mit einer HTTPS-Variante hinter Caddy |
| Kubernetes | [Helm-Chart](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| Linux-VM | [cloud-init](deploy/templates/cloud-init/) für DigitalOcean, Hetzner, AWS, GCP, Azure; [systemd-Unit](deploy/templates/systemd/) |
| Plattformen | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### Weitere Installationswege

| Methode | Befehl | Hinweise |
|---|---|---|
| Installationsskript | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) und macOS, x86-64 und arm64. `sh -s -- --version v3.0.0` legt ein Release fest. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS und Linux. `brew services start rushdb` startet es im Hintergrund. |
| Docker | `rushdb/rushdb` | `linux/amd64` und `linux/arm64`; auch `ghcr.io/rush-db/rushdb`. |
| Cargo | `cargo binstall rushdb-server` oder `cargo install rushdb-server --locked` | `binstall` lädt das Release-Binary, `install` kompiliert (braucht einen C++-Compiler). |
| Release-Archiv | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Archive mit SHA-256-Prüfsummen, SBOM und Sigstore-Signaturen. |
| Quellcode | `cargo build --release -p rushdb-server --bin rushdb` | Siehe [Installation](docs/manual/getting-started/installation.md). |

Windows wird nicht nativ unterstützt; nutze WSL 2 oder Docker. Lies [Sicherheit](docs/manual/operations/security.md), bevor du einen Knoten über Loopback hinaus erreichbar machst.

## Status

| Bereich | Status |
|---|---|
| Natives Protokoll, SQL, Gremlin-Text-Traversals, Mongo-artige Dokumente | Stabil |
| SQL/PGQ `GRAPH_TABLE` | Stabil, nur lesend |
| openCypher, ISO GQL, Bolt | Release Candidate, explizit zu aktivieren: Die externen Konformitätssuiten laufen nicht vollständig durch ([Details](docs/manual/languages/README.md)) |
| PostgreSQL-Wire | SQL-Lesezugriffe und Schreibvorgänge mit einer Anweisung; geprüft mit `psql`, node-postgres, psycopg und `tokio-postgres`, nicht mit jedem Client oder ORM |
| Replikation | Asynchrones Streaming auf schreibgeschützte Follower; Promotion ist manuell, ohne automatisches Failover |
| Raft, Storage-Sharding | Vorschau, nicht für Produktion: Raft läuft in einem Prozess mit Zustand im Speicher; Commits über mehrere Shards sind nicht absturzatomar |

Das [Handbuch](docs/manual/README.md) dokumentiert die genauen Grenzen jeder Schnittstelle.

## Verbinden

| Schnittstelle | Standard | Aktivieren |
|---|---|---|
| Natives HTTP-Protokoll | `127.0.0.1:3000`, `POST /protocol/v1/execute` | immer an |
| PostgreSQL-Wire | `127.0.0.1:5432` | an mit `sql` (Standard) |
| gRPC (`rushdb.v1.Database/Execute`) | aus | `--grpc-port` |
| Bolt 5.0 | aus | `--bolt-port` mit `cypher` |

Die SDKs für [JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) und [Python](https://pypi.org/project/rushdb/), der [MCP-Server](https://www.npmjs.com/package/@rushdb/mcp-server) und die [Agent Skills](https://github.com/rush-db/skills) arbeiten im Direct Mode mit einer selbst gehosteten Engine.

## Selbst gehostete Engine und RushDB Cloud

Diese Engine ist der Teil von RushDB, den du selbst hostest. Sie erzeugt keine Embeddings, hostet kein Dashboard und verwaltet keine Benutzer: Das kommt mit [RushDB Cloud](https://app.rushdb.com), dem verwalteten Dienst, der diese Engine betreibt und das Dashboard, verwaltete Embeddings und die KI-Funktionen (Suche in natürlicher Sprache, Vorschläge für Beziehungsmuster) ergänzt. Die SDKs und der MCP-Server funktionieren mit beidem: Gib ihnen einen RushDB-Cloud-API-Key oder richte sie direkt auf deine Engine (`direct: true`, `engineUrl`). Dann funktioniert alles außer den Cloud-Funktionen, die sie mit `CloudOnlyError` ablehnen.

## Dokumentation

- [Handbuch](docs/manual/README.md): Einstieg, Datenmodell, Abfragen, Sprachen, Suche, Betrieb und Referenztabellen.
- [docs.rushdb.com](https://docs.rushdb.com): Tutorials, die SDKs und der MCP-Server.
- [Upgrade von RushDB 2.x](docs/manual/operations/upgrading-from-2x.md): 2.x war der Neo4j-basierte Stack und bleibt am `v2`-Tag dieses Repositorys erhalten.

## Mitwirken

Siehe [CONTRIBUTING.md](CONTRIBUTING.md). Das Quality Gate:

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [Sicherheitsrichtlinie](SECURITY.md)
- [Release-Prozess](RELEASING.md)
- [Verhaltenskodex](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

Lizenziert unter [Apache-2.0](LICENSE).
