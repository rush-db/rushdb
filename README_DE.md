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

Das Schema stammt aus dem Katalog, nicht aus einem Scan der Datensätze:

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
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

Du sprichst schon SQL? Labels sind Tabellen:

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Weiter: der [vollständige Schnellstart](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker und andere Installationswege](https://docs.rushdb.com/engine/getting-started/installation/)

## Was du damit bauen kannst

- **Agent-Gedächtnis.** Erinnerungen als Datensätze mit Beziehungen, nach Scope gefiltert und nach den Vektoren gerankt, die du speicherst. Fertige Provider für OpenClaw und Hermes.
- **Suche über die Daten deiner App.** Volltext-, Vektor- und hybride Suche über die Datensätze, die deine App ohnehin schreibt, mit Graph-Filtern in derselben Abfrage.
- **Fragen über verknüpfte Daten.** Ein Agent liest das Schema und schreibt dann eine Abfrage, die du prüfen kannst: in der JSON-Abfragesprache, in SQL oder in Cypher.
- **Berichte ohne Export.** Gruppiere und aggregiere die Datensätze, die deine App schreibt, aus den SDKs oder mit jedem PostgreSQL-Client.

## Funktionen

- **Emergentes Schema.** Typen werden pro Wert abgeleitet, verschachtelte Objekte werden zu verknüpften Datensätzen, und der Katalog führt exakte Zählungen und Wertelisten. [Schema-Katalog](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Graph-Abfragen.** Filtere über Beziehungen hinweg nach Label, schränke sie nach Typ und Richtung ein und binde verknüpfte Datensätze für Projektionen und Aggregationen. [Beziehungen und Traversierung](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Suche eingebaut.** Vektorindizes über die Vektoren, die du lieferst, BM25-Volltextsuche und hybride Suche, die beides kombiniert. [Indizes und Suche](https://docs.rushdb.com/engine/indexes-and-search/)
- **Aggregationen.** `select` und `groupBy` mit Anzahlen, Summen, Durchschnitten, Zeit-Buckets und verschachtelten Sammlungen. [Aggregationen](https://docs.rushdb.com/engine/query/aggregations/)
- **Spricht, was du nutzt.** SQL über das PostgreSQL-Wire-Protokoll, Gremlin, SQL/PGQ, Mongo-artige Dokumente sowie openCypher und ISO GQL als Release Candidates. [Abfragesprachen](https://docs.rushdb.com/engine/languages/)
- **Dauerhaft.** Ein Write-Ahead-Log, Transaktionen, Backups mit Point-in-Time-Wiederherstellung und Read-Replicas. [Betrieb](https://docs.rushdb.com/engine/operations/)
- **Ein Binary.** HTTP, PostgreSQL-Wire, gRPC und Bolt aus einem Prozess, optional mit isolierten Mandanten. [Konnektivität](https://docs.rushdb.com/engine/connectivity/)

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

## Überall lauffähig

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data \
  -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes (Helm), cloud-init, systemd, Fly.io, Render,
Coolify und Dokploy: siehe [Deploy](https://docs.rushdb.com/deploy/). Lies
[Sicherheit](https://docs.rushdb.com/engine/operations/security/), bevor du
einen Knoten über Loopback hinaus erreichbar machst.

## Selbst gehostet und RushDB Cloud

Dieses Repository ist die Engine, und du kannst sie vollständig selbst hosten. [RushDB Cloud](https://app.rushdb.com) betreibt dieselbe Engine für dich und ergänzt die unten markierten Funktionen. Die SDKs und der MCP-Server funktionieren mit beiden; auf einer selbst gehosteten Engine schlägt ein Cloud-only-Aufruf mit `CloudOnlyError` fehl, bevor etwas gesendet wird.

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

**Windows?** Nutze WSL 2 oder Docker.

---

[Mitwirken](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Sicherheit](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (Legacy, auf Neo4j-Basis) lebt im
[`2.x`-Branch](https://github.com/rush-db/rushdb/tree/2.x) und erhält Sicherheitsfixes
bis zum 28.10.2027. [Upgrade von 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
