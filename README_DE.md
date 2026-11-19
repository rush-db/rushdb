<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="RushDB-Logo" width="96">

# RushDB

<sub>Die Datenbank für KI</sub>

**Graph, Vektoren und Analytics auf denselben Datensätzen. Jeder Schreibvorgang pflegt ein Live-Schema aus Labels, Feldern und Werten, damit Agents echte Felder abfragen, statt zu raten.**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[Dokumentation](https://docs.rushdb.com) • [Schnellstart](https://docs.rushdb.com/engine/getting-started/quickstart/) • [Diskussionen](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • **Deutsch** • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

Bring deine Daten so ein, wie du sie hast: verschachteltes JSON, CSV, SQL-`INSERT`,
Schreibvorgänge in Cypher oder im Mongo-Stil oder Change Streams aus deinen Datenbanken.
RushDB speichert sie als typisierte Datensätze, die zu einem Graphen verknüpft sind, und
dieselben Datensätze beantworten Beziehungsabfragen, Vektor- und Volltextsuche sowie
Aggregationen. Es gibt also keinen zweiten Speicher, den du synchron halten musst. Jeder
Schreibvorgang aktualisiert außerdem einen Katalog der vorhandenen Labels, Felder, Typen
und Werte, sodass ein Agent echte Felder abfragt. Verbinde dich über HTTP, das
PostgreSQL-Wire-Protokoll, Bolt, gRPC oder MCP, aus den SDKs oder mit jedem
PostgreSQL-Client. Ein Rust-Binary, Apache-2.0.

| Graph | Vektor- und Volltextsuche | Analytics |
|---|---|---|
| Verschachteltes JSON wird zu verknüpften Datensätzen. Filtere über Beziehungen hinweg und folge typisierten, gerichteten Beziehungen in einer Abfrage. | Vektorsuche (exakt und HNSW), BM25-Volltextsuche und hybride Suche, gefiltert mit denselben Graph-Prädikaten. | Anzahl, Summe, Durchschnitt, Minimum und Maximum mit `groupBy` oder schlichtes SQL mit `GROUP BY` über das PostgreSQL-Wire-Protokoll. |

## Schnellstart

![Terminalaufnahme: RushDB installieren, mit rushdb up starten, dann aus Python verschachteltes JSON importieren, Mitarbeitende nach Abteilung filtern, Personen und Durchschnittsgehalt je Abteilung zählen und das Live-Schema ausgeben](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

**Lass deinen Coding-Agenten es einrichten.** Füge ihm diesen Prompt ein ([rushdb.com/agent-setup](https://rushdb.com/agent-setup)):

```text
Set up RushDB as this project's database.

1. Install the RushDB MCP server (@rushdb/mcp-server) and the RushDB agent skills (github.com/rush-db/skills).
2. Use RushDB Cloud with an API key from app.rushdb.com, or a local engine:
   curl -fsSL https://rushdb.com/install.sh | sh && rushdb up
   (direct mode: engine URL http://127.0.0.1:3000, token local-token)
3. Before writing any query, read the schema catalog (labels, properties, types, relationship shapes) and use only fields that exist.
4. Docs: https://docs.rushdb.com/
```

Oder richte es von Hand ein:

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` lauscht auf `http://127.0.0.1:3000`, speichert seine Daten in `./.rushdb` und nutzt das
Token `local-token`. Führe in einem anderen Terminal `pip install rushdb` aus:

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
<summary>Dasselbe in TypeScript</summary>

`npm install @rushdb/javascript-sdk`:

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

Du sprichst schon SQL? Labels sind Tabellen:

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Weiter: der [vollständige Schnellstart](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker und andere Installationswege](https://docs.rushdb.com/engine/getting-started/installation/)

## Was Agenten sehen, bevor sie abfragen

Die letzte Zeile des Schnellstarts gibt das Schema aus, das ein Agent zuerst liest. Es stammt aus dem Katalog, den jeder Schreibvorgang aktualisiert, nicht aus einem Scan der Datensätze:

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

Auf einem großen, vielfältigen Store lädt ein Agent nicht alles davon. Er grenzt vom Überblick bis zu exakten Werten ein, mit jeweils einem kleinen Lesezugriff (derselbe Lauf wie oben):

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

`get_schema_summary` liefert denselben Katalog als JSON, mit Distinct-Zählungen und den Werten kleiner Felder. Der MCP-Server bietet jeden dieser Lesezugriffe als Tool an: `findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues` und `getSchemaSummary`.

## Deployment

| Wo | Wie |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3` |
| Docker Compose | [`deploy/templates/docker-compose`](https://github.com/rush-db/rushdb/tree/main/deploy/templates/docker-compose), mit einer HTTPS-Variante hinter Caddy |
| Kubernetes | [Helm-Chart](https://github.com/rush-db/rushdb/tree/main/charts/rushdb): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token="$(openssl rand -hex 32)"` |
| Linux-VM | [cloud-init](https://github.com/rush-db/rushdb/tree/main/deploy/templates/cloud-init) für DigitalOcean, Hetzner, AWS, Google Cloud und Azure; [systemd-Unit](https://github.com/rush-db/rushdb/tree/main/deploy/templates/systemd) |
| Plattformen | [Fly.io](https://github.com/rush-db/rushdb/tree/main/deploy/templates/fly), [Render](https://github.com/rush-db/rushdb/tree/main/deploy/templates/render), [Coolify](https://github.com/rush-db/rushdb/tree/main/deploy/templates/coolify), [Dokploy](https://github.com/rush-db/rushdb/tree/main/deploy/templates/dokploy) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

Ports, Probes, Dimensionierung und Betrieb als Dienst: [Deployment](https://docs.rushdb.com/engine/operations/deployment/). Alle Wege auf einer Seite:
[docs.rushdb.com/deploy](https://docs.rushdb.com/deploy/).

### Weitere Installationswege

| Weg | Befehl | Hinweise |
|---|---|---|
| Installationsskript | `curl -fsSL https://rushdb.com/install.sh \| sh` | Linux (glibc 2.28+) und macOS 11+, x86-64 und arm64. `sh -s -- --version v3.0.0` legt ein Release fest. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS und Linux. `brew services start rushdb` startet es im Hintergrund. |
| Docker | `rushdb/rushdb:3` | `linux/amd64` und `linux/arm64`, auch als `ghcr.io/rush-db/rushdb`. Tags `3`, `3.0` und exakte Versionen. |
| Cargo | `cargo binstall rushdb-server` · `cargo install rushdb-server --locked` | `binstall` lädt das Release-Binary; `install` kompiliert und braucht einen C/C++-Compiler. |
| Release-Archiv | [GitHub Releases](https://github.com/rush-db/rushdb/releases) | Archive mit SHA-256-Prüfsummen, einer SPDX-SBOM und Sigstore-Signaturen. |
| Quellcode | `cargo build --release -p rushdb-server --bin rushdb` | Toolchain festgelegt in `rust-toolchain.toml`. Siehe [Installation](https://docs.rushdb.com/engine/getting-started/installation/). |

Windows wird nicht nativ unterstützt; nutze WSL 2 oder Docker. Lies
[Sicherheit](https://docs.rushdb.com/engine/operations/security/), bevor du einen Knoten über
Loopback hinaus erreichbar machst.

## Was du damit bauen kannst

- **Agent-Gedächtnis.** Erinnerungen sind Datensätze, nach Nutzer, Agent oder Sitzung abgegrenzt und mit dem Gesprächszug verknüpft, aus dem sie stammen, sodass der Abruf zeigt, woher eine Erinnerung kommt. Fertige Provider für OpenClaw und Hermes.
- **Company Brain und RAG.** Dokumente, CRM-, Support- und Abrechnungsdaten kommen in ihrer eigenen Form an und werden über Schlüssel verknüpft, die sie schon teilen, etwa eine E-Mail oder eine Konto-ID. Die Suche filtert zuerst auf das richtige Konto oder die richtige Version und rankt dann nach Vektor und BM25 zusammen; jede Aussage verweist auf den Datensatz dahinter.
- **Facettensuche.** Filterleisten, deren Zählungen aus dem Katalog kommen, auf den aktuellen Filter bezogen und exakt, mit hybrider Suche unter demselben Filter. Kein separater Suchindex, der synchron gehalten werden muss.
- **KI-Suche und Assistenten in deiner App.** Text- und Vektorindizes liegen in der Datenbank und bleiben aktuell, während deine App schreibt; die SDKs, der MCP-Server und SQL-Clients lesen dieselben Datensätze.
- **Agentische Datenanalyse.** Der Agent liest, welche Labels und Felder es gibt und wie jeder Wert geschrieben ist, und schreibt dann SQL, Cypher oder eine Aggregation dagegen. Jede Antwort kommt mit einer Abfrage, die jeder erneut ausführen kann. Mit einem [Read-only-Token](https://docs.rushdb.com/engine/connectivity/authentication/#capability-tokens) lehnt die Engine jeden Schreibversuch des Agenten ab.
- **Apps, deren Schema sich unterwegs ändert.** Schreib beliebiges JSON: Typen werden abgeleitet, verschachtelte Objekte werden zu verknüpften Datensätzen, und neue Felder erscheinen mit dem Schreibvorgang, der sie hinzufügt, im Katalog. Keine Migrationen.

## Funktionen

- **Emergentes Schema.** Typen werden pro Wert abgeleitet, verschachtelte Objekte werden zu verknüpften Datensätzen, und der Katalog führt exakte Zählungen und Wertelisten. [Schema-Katalog](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Graph-Abfragen.** Filtere über Beziehungen hinweg nach Label, schränke sie nach Typ und Richtung ein und binde verknüpfte Datensätze für Projektionen und Aggregationen. [Beziehungen und Traversierung](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Suche eingebaut.** Vektorindizes über die Vektoren, die du lieferst, BM25-Volltextsuche und hybride Suche, die beides kombiniert. [Indizes und Suche](https://docs.rushdb.com/engine/indexes-and-search/)
- **Aggregationen.** `select` und `groupBy` mit Anzahlen, Summen, Durchschnitten, Zeit-Buckets und verschachtelten Sammlungen. [Aggregationen](https://docs.rushdb.com/engine/query/aggregations/)
- **Spricht, was du nutzt.** SQL über das PostgreSQL-Wire-Protokoll, Gremlin, SQL/PGQ, Mongo-artige Dokumente sowie openCypher und ISO GQL als Release Candidates. [Abfragesprachen](https://docs.rushdb.com/engine/languages/)
- **Dauerhaft.** Ein Write-Ahead-Log, Transaktionen, Backups mit Point-in-Time-Wiederherstellung und Read-Replicas. [Betrieb](https://docs.rushdb.com/engine/operations/)
- **Ein Binary.** HTTP, PostgreSQL-Wire, gRPC und Bolt aus einem Prozess, optional mit isolierten Mandanten. [Konnektivität](https://docs.rushdb.com/engine/connectivity/)

## Verbinden

| Schnittstelle | Standard | Aktivieren |
|---|---|---|
| [Natives HTTP-Protokoll](https://docs.rushdb.com/engine/connectivity/http-api/) | `127.0.0.1:3000`, `POST /protocol/v1/execute` | Immer an; `--port` verlegt es |
| [PostgreSQL-Wire-Protokoll](https://docs.rushdb.com/engine/connectivity/postgresql-wire/) | `127.0.0.1:5432` | An, solange `sql` aktiv ist (Standard); `--pgwire-port 0` schaltet es ab |
| [gRPC](https://docs.rushdb.com/engine/connectivity/grpc/) (`rushdb.v1.Database/Execute`) | Aus | `--grpc-port <port>` |
| [Bolt 5.0](https://docs.rushdb.com/engine/connectivity/bolt/) | Aus | `--bolt-port <port>` und `cypher` in `--languages` |
| [Lokaler MCP](https://docs.rushdb.com/connect/mcp/) (`@rushdb/mcp-server`, [tools](https://docs.rushdb.com/learn/reference/mcp/tools/)) | stdio: `npx -y @rushdb/mcp-server` | Direktmodus zu einer selbst gehosteten Engine: `RUSHDB_MODE=direct`, `RUSHDB_ENGINE_URL`, `RUSHDB_ENGINE_TOKEN` |
| [Gehosteter MCP](https://docs.rushdb.com/connect/mcp/) | `https://mcp.rushdb.com/mcp`, OAuth für ChatGPT, Claude.ai und andere Web-Clients | Eine Funktion von RushDB Cloud; nichts zu installieren |

Alle Listener binden an `--listen` (Standard `127.0.0.1`) und teilen sich ein
TLS-Zertifikat (`--tls-cert`, `--tls-key`). Mehr unter [Konnektivität](https://docs.rushdb.com/engine/connectivity/),
[Authentifizierung](https://docs.rushdb.com/engine/connectivity/authentication/) und [SQL- und Cypher-Clients](https://docs.rushdb.com/connect/sql-and-cypher/). Die SDKs,
der MCP-Server und die Integrationen unten erreichen eine selbst gehostete Engine
im Direktmodus: siehe [Connect](https://docs.rushdb.com/connect/).

## Ökosystem

| Projekt | Für |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | TypeScript- und Python-Apps |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor und andere MCP-Clients |
| [Agent-Skills](https://github.com/rush-db/skills) · [Agent-Plugins](https://github.com/rush-db/agent-plugins) | Coding-Agents |
| [LangChain und LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | Retrieval, Gedächtnis und Property-Graph-Stores |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [Memory-Contract](https://github.com/rush-db/agent-memory-contract) | Gedächtnis-Provider für Agents |
| [Debezium-Sink](https://github.com/rush-db/debezium-server-rushdb) · [Obsidian-Plugin](https://github.com/rush-db/obsidian-plugin) | Change Data Capture, Notizen |

Alle funktionieren mit einer selbst gehosteten Engine. Wenn dein Coding-Agent RushDB
einrichten soll, gib ihm [rushdb.com/agent-setup](https://rushdb.com/agent-setup).

## Selbst gehostet und RushDB Cloud

Dieses Repository ist die Engine, und du kannst sie vollständig selbst hosten. [RushDB Cloud](https://app.rushdb.com) betreibt dieselbe Engine für dich und ergänzt die unten markierten Funktionen. Die SDKs und der MCP-Server funktionieren mit beiden; auf einer selbst gehosteten Engine lehnen die SDKs einen Cloud-only-Aufruf mit `CloudOnlyError` ab, bevor etwas gesendet wird, und der MCP-Server bietet diese Tools gar nicht an.

![RushDB-Dashboard mit synthetischen Beispieldaten: die Datensatztabelle, dann die Graph-Ansicht mit Firmen und ihren Abteilungs-Clustern, Zoom auf eine Abteilung, deren Mitarbeitende im Datensatz-Panel aufgelistet sind](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| Funktion | Selbst gehostete Engine | RushDB Cloud |
|---|---|---|
| Engine und API: Datensätze, Graph-Abfragen, Suche, Aggregationen, Live-Schema | ✓ | ✓ |
| SDKs und MCP-Server | ✓ Direct Mode, MCP über stdio | ✓ dazu gehosteter MCP mit OAuth |
| Import von JSON und CSV | ✓ | ✓ |
| [Dashboard](https://docs.rushdb.com/rushdb-cloud/) mit Datensätzen, Graph-Ansicht und Query-Builder | Nutze die SDKs oder einen beliebigen PostgreSQL-Client | ✓ |
| [Verwaltete Embeddings](https://docs.rushdb.com/rushdb-cloud/capabilities/) | Vektoren aus deinem eigenen Modell | ✓ monatliches Kontingent |
| [Suche in natürlicher Sprache](https://docs.rushdb.com/learn/search-query/smart-search/) | Abfrage selbst oder mit einem Agenten bauen | ✓ monatliches Kontingent |
| [Beziehungsvorschläge](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | Beziehungen selbst anlegen | ✓ |
| [Definitionen](https://docs.rushdb.com/learn/search-query/definitions/) für Metriken und Filter | Regeln in eigenen Prompts und SQL pflegen | ✓ |
| [Verbindungen](https://docs.rushdb.com/learn/data-sources/connections/) zu SaaS-Tools und RushDB Sync für On-Prem-Datenbanken | Eigener Sync-Code oder der Debezium-Sink | ✓ |
| Projekte und API-Schlüssel | Tenants und Tokens verwaltest du selbst | ✓ |
| Teamzugriff | — | Kostenpflichtige Pläne |
| Backups | Du betreibst sie: Backup, Point-in-Time-Restore, Replikate | Kostenpflichtige Pläne |

Ein ✓ unter RushDB Cloud bedeutet, dass der Free-Plan es enthält. Pläne und Kontingente stehen auf [rushdb.com/pricing](https://rushdb.com/pricing).

## FAQ

**Ist es produktionsreif?** SQL, die native JSON-Abfragesprache, Gremlin und
Mongo-artige Dokumente sind stabil. openCypher, ISO GQL und Bolt sind Release
Candidates. Die Replikation ist asynchron, mit manueller Promotion. Die
[Statusseite](https://docs.rushdb.com/engine/reference/status/) listet jede
Schnittstelle und ihre Grenzen auf.

**Brauche ich eine separate Vektordatenbank?** Nein. Vektoren, Textindizes und
Beziehungen liegen in einer Engine, und ein Datensatz lässt sich zusammen mit
seinem Vektor in einem Commit schreiben. Die Engine durchsucht die Vektoren, die du lieferst;
RushDB Cloud kann sie für dich erzeugen.

**Was ist das Live-Schema?** Ein Katalog aller Labels, Properties, Typen, Werte
und Beziehungsformen, bei jedem Commit aktualisiert, mit exakten Zählungen. Ein Aufruf
liefert ihn, sodass Agents und Tools keine Feldnamen raten müssen.
[Schema-Katalog](https://docs.rushdb.com/engine/data-model/schema-catalog/)

---

[Mitwirken](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Sicherheit](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (Legacy, auf Neo4j-Basis) lebt im
[`2.x`-Branch](https://github.com/rush-db/rushdb/tree/2.x) und erhält Sicherheitsfixes
bis zum 28.10.2027. [Upgrade von 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
