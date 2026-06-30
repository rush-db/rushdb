<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### Die Speicherschicht für KI-Agenten und Anwendungen.

Schiebe beliebiges JSON hinein. Dein Agent erhält ein lebendiges, abfragbares Schema, Graph-Beziehungen und semantische Suche — automatisch abgeleitet.
Keine Pipeline. Keine separaten Speicher. Kein Schema, das entworfen werden muss.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 Website](https://rushdb.com) • [📖 Dokumentation](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 Beispiele](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • **Deutsch** • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## Das Problem

Dein Agent braucht Speicher. Die übliche Antwort sind drei Datenbanken: Redis für Key-Value, ein Vektorspeicher für die semantische Suche, eine Graph-DB für Beziehungen — plus Klebe-Code, um sie synchron zu halten.

RushDB ersetzt alle drei. Schiebe JSON einmal hinein. Frage es per Graph-Traversierung, semantischer Suche oder beidem ab — in einem einzigen Aufruf.

| Ohne RushDB                                            | Mit RushDB                                       |
| ------------------------------------------------------ | ------------------------------------------------ |
| Redis + Pinecone + Neo4j + Klebe-Code                  | Eine API                                         |
| Schema entwerfen → Migrationen schreiben → wiederholen | Beliebige Form hineinschieben, kein Schema nötig |
| Separate Embedding-Pipeline                            | Verwaltete Embeddings, serverseitig              |
| Beziehungskanten von Hand erstellen                    | Automatisch aus deiner Datenstruktur erkannt     |

---

## Schnellstart

Zwei Wege, je nach deinem Setup:

- **Cloud** — Verwaltet, kostenloses Kontingent, in 30 Sekunden einsatzbereit. [API-Schlüssel holen →](https://app.rushdb.com)
- **Self-Hosting** — Docker + deine eigene Neo4j-Instanz. [Zum Self-Hosting springen →](#self-hosting)

### Cloud-Weg

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### Agentenspeicher speichern und abrufen

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

## Arbeiten mit deinen Daten

### Verschachteltes JSON importieren

Schiebe eine beliebige JSON-Form hinein. Verschachtelte Objekte und Arrays von Objekten werden zu verknüpften Records — Labels, Typen und Beziehungen werden beim Schreiben abgeleitet. Kein Schema, kein Migrationsschritt.

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

Jeder verschachtelte Schlüssel (`DEPARTMENT`, `EMPLOYEE`) wird zu einem Label, jedes Objekt zu einem Record und die Enthaltensein-Beziehung zu einer Relationship — alles automatisch erstellt.

### CSV importieren

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

`suggestTypes` leitet Zahlen, Booleans und Datumsangaben aus Zeichenketten ab; `skipEmptyValues` behandelt leere Zellen als nicht gesetzt, anstatt leere Werte zu speichern (`0` und `false` werden beibehalten).

### Den Graphen traversieren

Filtere Wurzel-Records anhand von Bedingungen auf ihren _verwandten_ Records — beliebig tief — in einer einzigen Abfrage. Verwandte Labels gehören **innerhalb** von `where`, nicht in `labels`:

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

### Analytische Abfragen (Aggregation & Gruppierung)

`select` formt die Ausgabe mit Aggregationen (`$sum`, `$avg`, `$count`, `$min`, `$max`); `groupBy` steuert die Dimensionen. Füge einer Aggregation kein `limit` hinzu — es würde nur die ersten N Records scannen und die Summen verfälschen.

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

Aggregationen lassen sich mit Traversierung kombinieren — z. B. Mitarbeiterzahl und Gehaltssumme pro Abteilung:

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

## Mit Claude, Cursor oder einem beliebigen MCP-Client verbinden

RushDB liefert einen MCP-Server. Dein Agent erhält persistenten, strukturierten Speicher — direkt einsatzbereit.

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

Lege dies in deine MCP-Konfiguration für Claude Desktop, Cursor oder Windsurf. Der Agent kann nun Records erstellen, nach Bedeutung suchen, Beziehungen traversieren und das Schema introspizieren — alles über natürliche Sprache.

---

## Was enthalten ist

| Funktion                            | Was es bedeutet                                                                                                                                                                          |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Verwaltete Embeddings**           | Indexiere eine beliebige Zeichenketten-Eigenschaft einmal — jeder Schreibvorgang wird serverseitig automatisch eingebettet                                                               |
| **Graph + Vektor in einer Abfrage** | Semantische Ähnlichkeit und Beziehungs-Traversierung lassen sich in einem einzigen Aufruf kombinieren                                                                                    |
| **Kein Schema**                     | Schiebe eine beliebige JSON-Form hinein. RushDB leitet Typen ab, erstellt Eigenschaften und verknüpft Records                                                                            |
| **Abgeleitetes Schema**             | Eigenschaften werden zu erstklassigen Knoten — Typen, Labels und Beziehungen werden beim Schreiben entdeckt, sodass sich ein abfragbares Schema selbst aufbaut, während Daten eintreffen |
| **ACID-Transaktionen**              | Nebenläufige Agenten beschädigen den gemeinsamen Speicher nicht. Neo4j unter der Haube                                                                                                   |
| **Selbstbeschreibend**              | Agenten introspizieren das abgeleitete Schema — Labels, Eigenschaften, Wertebereiche — um zu wissen, was sie sicher abfragen können                                                      |
| **MCP-nativ**                       | Vollständiger MCP-Server mit eingebautem Discovery-First-Abfrage-Prompt                                                                                                                  |
| **Agent Skills**                    | Installierbares `@rushdb/skills`-Paket — bringe jedem skills-kompatiblen Agenten mit einem Befehl bei, mit RushDB abzufragen, zu modellieren und sich zu erinnern                        |
| **Einheitliche Abfrage-API**        | Eine JSON-Form für Graph, Vektor, Aggregation und Introspektion                                                                                                                          |
| **Self-Hosting oder Cloud**         | Docker + dein Neo4j, oder verwaltete Cloud. Vollständige Datenhoheit                                                                                                                     |

---

## Anwendungsfälle

| Anwendungsfall              | Was RushDB ersetzt                | Wichtige API                                                    |
| --------------------------- | --------------------------------- | --------------------------------------------------------------- |
| **Agentenspeicher**         | Redis + Vektorspeicher + Graph-DB | `db.ai.search({ query, where: { agent_id } })`                  |
| **RAG mit Kontext**         | Flacher Vektorspeicher            | `db.records.find({ where, labels })` + Beziehungs-Traversierung |
| **Schemafreie Apps**        | Postgres + Migrationen + ETL      | `db.records.importJson(nestedJson)`                             |
| **Vernetzte Datenprodukte** | Mehrere verknüpfte Dienste        | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })`   |

---

## Self-Hosting

> **Self-Hosting-Weg** — betreibe RushDB auf deiner eigenen Infrastruktur. Erfordert Neo4j 2026.01.4+ mit APOC-Plugin.

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
<summary>Vollständige Umgebungsvariablen</summary>

| Name                              | Beschreibung                                           | Erforderlich | Standard |
| --------------------------------- | ------------------------------------------------------ | ------------ | -------- |
| `NEO4J_URL`                       | Neo4j-Verbindungs-URL                                  | ja           | —        |
| `NEO4J_USERNAME`                  | Neo4j-Benutzername                                     | ja           | neo4j    |
| `NEO4J_PASSWORD`                  | Neo4j-Passwort                                         | ja           | —        |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | 32-Zeichen-Schlüssel für API-Token-Verschlüsselung     | ja (prod)    | —        |
| `RUSHDB_PORT`                     | HTTP-Port                                              | nein         | 3000     |
| `RUSHDB_LOGIN`                    | Admin-Login                                            | nein         | admin    |
| `RUSHDB_PASSWORD`                 | Admin-Passwort                                         | nein         | password |
| `RUSHDB_BASE_URL`                 | Öffentliche/Basis-API-URL für synx-Zuweisungen         | nein         | —        |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | Internes Token für verwaltete synx-Worker              | nein         | —        |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | Interner Schreibschlüssel für synx-Zielschreibvorgänge | nein         | —        |

Verwaltete synx-Worker laufen als Daemons. Sie pollen nach ausführbaren Connectors, erneuern Connector-Leases, geben Leases bei einem ordnungsgemäßen Herunterfahren frei und ermöglichen platform/core, abgelaufene Leases nach Abstürzen zurückzufordern.

</details>

<details>
<summary>Lokale Entwicklung (gebündeltes Neo4j)</summary>

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
<summary>Architektur: Wie RushDB Daten strukturiert (LMPG)</summary>

RushDB verwendet ein **Labeled Meta Property Graph (LMPG)**-Modell. Eigenschaften werden zu erstklassigen Graph-Knoten erhoben ("HyperProperties") — nicht nur Key-Value-Paare, die an Records angehängt sind.

Das bedeutet:

- **Schema ohne vorherigen Entwurf** — da Eigenschaften Graph-Knoten sind, wird das Schema _aus deinen Daten abgeleitet, nicht entworfen_: Labels, Typen, Wertebereiche und Beziehungstopologie werden beim Schreiben entdeckt und sind sofort abfragbar — keine manuelle Schemamodellierung, keine RDF/OWL-Toolchain
- **Automatisch erkannte Beziehungen** — Records, die Eigenschaften teilen, werden verknüpft, ohne dass Kanten von Hand erstellt werden müssen
- **Schema-Introspektion** — Agenten können Labels, Eigenschaftstypen, Wertebereiche und Beziehungstopologie in einer einzigen Abfrage aufzählen
- **Weiche Einschränkungen** — Typenkohäsions-Bewertung, Kardinalitäts-Tracking und Vektordimensions-Durchsetzung ohne starre vorab definierte Schemata
- **Einheitliche Abfrageoberfläche** — derselbe Filterausdruck funktioniert über Records, Labels, Eigenschaften und Beziehungen hinweg

Eine SearchQuery ruft mehrere Perspektiven gleichzeitig ab (Records + Eigenschaftsstatistiken + Aggregationen) und vermeidet so das N+1-Inspektionsmuster, das in Architekturen mit getrennten Systemen üblich ist.

[Lies den vollständigen Beitrag zur LMPG-Architektur →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## Dokumentation

| Thema                       | Link                                                           |
| --------------------------- | -------------------------------------------------------------- |
| Schnell-Tutorial            | https://docs.rushdb.com/get-started/quick-tutorial             |
| Vektor- / Semantische Suche | https://docs.rushdb.com/concepts/search/where#vector-operators |
| Filtern & Traversieren      | https://docs.rushdb.com/concepts/search/where                  |
| Gruppieren & Aggregationen  | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK              | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK                  | https://docs.rushdb.com/python-sdk/introduction                |
| REST API                    | https://docs.rushdb.com/rest-api/introduction                  |
| MCP-Server                  | packages/mcp-server/README.md                                  |
| Agent Skills                | packages/skills/README.md                                      |

---

## Wann du RushDB nicht verwenden solltest

- Du benötigst Latenzen im Submillisekundenbereich bei sehr hohem Schreibdurchsatz — RushDB basiert auf Neo4j, das Konsistenz und Abfrage-Ausdrucksstärke über reine Schreibgeschwindigkeit stellt.
- Du benötigst nur flache Key-Value-Speicherung ohne Beziehungen oder semantische Suche — ein einfacherer Speicher wird leichtgewichtiger sein.
- Du benötigst ein starres relationales Schema, das auf Datenbankebene durchgesetzt wird — RushDB ist bewusst schemafrei.

---

## Mitwirken

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

Siehe [CONTRIBUTING.md](CONTRIBUTING.md) für die vollständigen Richtlinien. Issues und PRs sind willkommen.

---

## Lizenz

| Pfad                    | Lizenz              |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

Brauchst du etwas, das noch nicht unterstützt wird? Eröffne ein Issue — Design-Diskussionen sind willkommen.
