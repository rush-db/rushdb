<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### La couche mémoire pour les agents et les applications IA.

Envoyez n'importe quel JSON. Votre agent obtient un schéma vivant et interrogeable, des relations en graphe et une recherche sémantique — déduits automatiquement.
Pas de pipeline. Pas de stockages séparés. Aucun schéma à concevoir.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 Site web](https://rushdb.com) • [📖 Documentation](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 Exemples](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • **Français** • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## Le problème

Votre agent a besoin de mémoire. La réponse classique, ce sont trois bases de données : Redis pour le clé-valeur, un stockage vectoriel pour la recherche sémantique, une base de données en graphe pour les relations — plus du code de liaison pour les garder synchronisées.

RushDB remplace les trois. Envoyez le JSON une seule fois. Interrogez-le par parcours de graphe, recherche sémantique, ou les deux — en un seul appel.

| Sans RushDB                                               | Avec RushDB                                                       |
| --------------------------------------------------------- | ----------------------------------------------------------------- |
| Redis + Pinecone + Neo4j + code de liaison                | Une seule API                                                     |
| Concevoir le schéma → écrire les migrations → recommencer | Envoyez n'importe quelle forme, aucun schéma requis               |
| Pipeline d'embeddings séparé                              | Embeddings gérés, côté serveur                                    |
| Créer à la main les arêtes de relation                    | Détectées automatiquement à partir de la structure de vos données |

---

## Démarrage rapide

Deux voies selon votre configuration :

- **Cloud** — Géré, offre gratuite, opérationnel en 30 secondes. [Obtenir une clé API →](https://app.rushdb.com)
- **Auto-hébergement** — Docker + votre propre instance Neo4j. [Aller à l'auto-hébergement →](#self-hosting)

### Voie Cloud

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### Stocker et rappeler la mémoire de l'agent

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

## Travailler avec vos données

### Importer du JSON imbriqué

Envoyez n'importe quelle forme de JSON. Les objets imbriqués et les tableaux d'objets deviennent des enregistrements liés — les labels, les types et les relations sont déduits à l'écriture. Pas de schéma, pas d'étape de migration.

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

Chaque clé imbriquée (`DEPARTMENT`, `EMPLOYEE`) devient un label, chaque objet un enregistrement, et l'imbrication une relation — le tout créé automatiquement.

### Importer du CSV

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

`suggestTypes` déduit les nombres, les booléens et les dates à partir des chaînes ; `skipEmptyValues` traite les cellules vides comme non définies au lieu de stocker des valeurs vides (`0` et `false` sont conservés).

### Parcourir le graphe

Filtrez les enregistrements racines selon des conditions portant sur leurs enregistrements _liés_ — à une profondeur arbitraire — en une seule requête. Les labels liés vont **à l'intérieur** de `where`, pas dans `labels` :

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

### Requêtes analytiques (agrégation et regroupement)

`select` met en forme la sortie avec des agrégations (`$sum`, `$avg`, `$count`, `$min`, `$max`) ; `groupBy` contrôle les dimensions. N'ajoutez pas de `limit` à une agrégation — cela n'analyserait que les N premiers enregistrements et fausserait les totaux.

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

Les agrégations se combinent avec le parcours — par exemple, l'effectif et la masse salariale par département :

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

## Se connecter à Claude, Cursor ou tout client MCP

RushDB est livré avec un serveur MCP. Votre agent obtient une mémoire structurée et persistante — prête à l'emploi.

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

Placez ceci dans la configuration MCP de votre Claude Desktop, Cursor ou Windsurf. L'agent peut désormais créer des enregistrements, effectuer des recherches par sens, parcourir les relations et inspecter le schéma — le tout en langage naturel.

---

## Ce qu'il y a dans la boîte

| Capacité                            | Ce que cela signifie                                                                                                                                                                                                 |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Embeddings gérés**                | Indexez n'importe quelle propriété de type chaîne une seule fois — chaque écriture est auto-embeddée côté serveur                                                                                                    |
| **Graphe + vecteur en une requête** | La similarité sémantique et le parcours de relations se combinent en un seul appel                                                                                                                                   |
| **Zéro schéma**                     | Envoyez n'importe quelle forme de JSON. RushDB déduit les types, crée les propriétés, lie les enregistrements                                                                                                        |
| **Schéma déduit**                   | Les propriétés deviennent des nœuds de première classe — les types, labels et relations sont découverts à l'écriture, de sorte qu'un schéma interrogeable se construit de lui-même à mesure que les données arrivent |
| **Transactions ACID**               | Les agents concurrents ne corrompent pas la mémoire partagée. Neo4j sous le capot                                                                                                                                    |
| **Auto-descriptif**                 | Les agents inspectent le schéma déduit — labels, propriétés, plages de valeurs — pour savoir ce qu'ils peuvent interroger en toute sécurité                                                                          |
| **Natif MCP**                       | Serveur MCP complet avec une invite de requête « découverte d'abord » intégrée                                                                                                                                       |
| **Agent Skills**                    | Package `@rushdb/skills` installable — apprenez à n'importe quel agent compatible avec les skills à interroger, modéliser et mémoriser avec RushDB en une seule commande                                             |
| **API de requête unifiée**          | Une seule forme JSON pour le graphe, le vecteur, l'agrégation et l'introspection                                                                                                                                     |
| **Auto-hébergement ou cloud**       | Docker + votre Neo4j, ou cloud géré. Propriété totale des données                                                                                                                                                    |

---

## Cas d'usage

| Cas d'usage                        | Ce que RushDB remplace                      | API clé                                                       |
| ---------------------------------- | ------------------------------------------- | ------------------------------------------------------------- |
| **Mémoire d'agent**                | Redis + stockage vectoriel + base en graphe | `db.records.vectorSearch({ query, where: { agent_id } })`     |
| **RAG avec contexte**              | Stockage vectoriel plat                     | `db.records.find({ where, labels })` + parcours de relations  |
| **Applications sans schéma**       | Postgres + migrations + ETL                 | `db.records.importJson(nestedJson)`                           |
| **Produits de données connectées** | Plusieurs services joints                   | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })` |

---

## Auto-hébergement

> **Voie d'auto-hébergement** — exécutez RushDB sur votre propre infrastructure. Nécessite Neo4j 2026.01.4+ avec le plugin APOC.

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
<summary>Variables d'environnement complètes</summary>

| Nom                               | Description                                                   | Requis     | Défaut   |
| --------------------------------- | ------------------------------------------------------------- | ---------- | -------- |
| `NEO4J_URL`                       | URL de connexion Neo4j                                        | oui        | —        |
| `NEO4J_USERNAME`                  | Nom d'utilisateur Neo4j                                       | oui        | neo4j    |
| `NEO4J_PASSWORD`                  | Mot de passe Neo4j                                            | oui        | —        |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | Clé de 32 caractères pour le chiffrement des jetons API       | oui (prod) | —        |
| `RUSHDB_PORT`                     | Port HTTP                                                     | non        | 3000     |
| `RUSHDB_LOGIN`                    | Identifiant administrateur                                    | non        | admin    |
| `RUSHDB_PASSWORD`                 | Mot de passe administrateur                                   | non        | password |
| `RUSHDB_BASE_URL`                 | URL API publique/de base pour les affectations synx           | non        | —        |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | Jeton interne pour les workers synx gérés                     | non        | —        |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | Clé d'écriture interne pour les écritures de destination synx | non        | —        |

Les workers synx gérés s'exécutent en tant que daemons. Ils interrogent les connecteurs exécutables, renouvellent les baux de connecteur, libèrent les baux lors d'un arrêt gracieux, et permettent à platform/core de récupérer les baux expirés après un crash.

</details>

<details>
<summary>Développement local (Neo4j intégré)</summary>

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
<summary>Architecture : comment RushDB structure les données (LMPG)</summary>

RushDB utilise un modèle **Labeled Meta Property Graph (LMPG)**. Les propriétés sont élevées au rang de nœuds de graphe de première classe (« HyperProperties ») — pas seulement des paires clé-valeur attachées aux enregistrements.

Cela signifie :

- **Schéma sans conception préalable** — parce que les propriétés sont des nœuds de graphe, le schéma est _déduit de vos données, pas conçu_ : les labels, les types, les plages de valeurs et la topologie des relations sont découverts à l'écriture et immédiatement interrogeables — pas de modélisation manuelle de schéma, pas de chaîne d'outils RDF/OWL
- **Relations détectées automatiquement** — les enregistrements partageant des propriétés sont liés sans créer les arêtes à la main
- **Introspection du schéma** — les agents peuvent énumérer les labels, les types de propriétés, les plages de valeurs et la topologie des relations en une seule requête
- **Contraintes souples** — scoring de cohésion des types, suivi de cardinalité et application des dimensions vectorielles sans schémas rigides préalables
- **Surface de requête unifiée** — la même expression de filtre fonctionne sur les enregistrements, les labels, les propriétés et les relations

Une seule SearchQuery récupère plusieurs perspectives simultanément (enregistrements + statistiques de propriétés + agrégations), évitant le motif d'inspection N+1 courant dans les architectures à systèmes séparés.

[Lire l'article complet sur l'architecture LMPG →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## Documentation

| Sujet                              | Lien                                                           |
| ---------------------------------- | -------------------------------------------------------------- |
| Tutoriel rapide                    | https://docs.rushdb.com/get-started/quick-tutorial             |
| Recherche vectorielle / sémantique | https://docs.rushdb.com/concepts/search/where#vector-operators |
| Filtrage et parcours               | https://docs.rushdb.com/concepts/search/where                  |
| Regroupement et agrégations        | https://docs.rushdb.com/concepts/search/group-by               |
| SDK TypeScript                     | https://docs.rushdb.com/typescript-sdk/introduction            |
| SDK Python                         | https://docs.rushdb.com/python-sdk/introduction                |
| API REST                           | https://docs.rushdb.com/rest-api/introduction                  |
| Serveur MCP                        | packages/mcp-server/README.md                                  |
| Agent Skills                       | packages/skills/README.md                                      |

---

## Quand ne pas utiliser RushDB

- Vous avez besoin d'une latence sous la milliseconde à un très haut débit d'écriture — RushDB est construit sur Neo4j, qui privilégie la cohérence et l'expressivité des requêtes plutôt que la vitesse d'écriture brute.
- Vous n'avez besoin que d'un stockage clé-valeur plat sans relations ni recherche sémantique — un stockage plus simple sera plus léger.
- Vous avez besoin d'un schéma relationnel rigide appliqué au niveau de la base de données — RushDB est délibérément sans schéma.

---

## Contribuer

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

Consultez [CONTRIBUTING.md](CONTRIBUTING.md) pour les directives complètes. Les issues et les PR sont les bienvenues.

---

## Licence

| Chemin                  | Licence             |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

Besoin de quelque chose qui n'est pas encore pris en charge ? Ouvrez une issue — les discussions de conception sont les bienvenues.
