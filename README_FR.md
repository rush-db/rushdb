<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="Logo RushDB" width="96">

# RushDB

<sub>La base de données pour l'IA</sub>

**Graphe, vecteurs et analytique sur les mêmes enregistrements. Chaque écriture maintient un schéma vivant des labels, des champs et des valeurs, pour que les agents interrogent de vrais champs au lieu de deviner.**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[Documentation](https://docs.rushdb.com) • [Démarrage rapide](https://docs.rushdb.com/engine/getting-started/quickstart/) • [Discussions](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • **Français** • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

Importez vos données telles que vous les avez : JSON imbriqué, CSV, `INSERT` SQL,
écritures Cypher ou de style Mongo, ou change streams issus de vos bases de données.
RushDB les stocke sous forme d'enregistrements typés reliés en graphe, et ces mêmes
enregistrements répondent aux requêtes sur les relations, à la recherche vectorielle et
plein texte ainsi qu'aux agrégations : pas de second stockage à garder synchronisé. Chaque
écriture met aussi à jour un catalogue des labels, champs, types et valeurs existants, si
bien qu'un agent interroge des champs réels. Connectez-vous via HTTP, le protocole filaire
PostgreSQL, Bolt, gRPC ou MCP, depuis les SDK ou n'importe quel client PostgreSQL. Un seul
binaire Rust, Apache-2.0.

| Graphe | Recherche vectorielle et plein texte | Analytique |
|---|---|---|
| Le JSON imbriqué devient des enregistrements reliés. Filtrez à travers les relations et suivez des relations typées et orientées dans une seule requête. | Recherche vectorielle (exacte et HNSW), plein texte BM25 et hybride, filtrée par les mêmes prédicats de graphe. | Comptage, somme, moyenne, minimum et maximum avec `groupBy`, ou du SQL classique avec `GROUP BY` via le protocole filaire PostgreSQL. |

## Démarrage rapide

![Enregistrement du terminal : installer RushDB, le démarrer avec rushdb up, puis depuis Python importer du JSON imbriqué, filtrer les employés par département, compter les personnes et calculer le salaire moyen par département, et afficher le schéma vivant](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

**Laissez votre agent de code l’installer.** Collez-lui ce prompt ([rushdb.com/agent-setup](https://rushdb.com/agent-setup)) :

```text
Set up RushDB as this project's database.

1. Install the RushDB MCP server (@rushdb/mcp-server) and the RushDB agent skills (github.com/rush-db/skills).
2. Use RushDB Cloud with an API key from app.rushdb.com, or a local engine:
   curl -fsSL https://rushdb.com/install.sh | sh && rushdb up
   (direct mode: engine URL http://127.0.0.1:3000, token local-token)
3. Before writing any query, read the schema catalog (labels, properties, types, relationship shapes) and use only fields that exist.
4. Docs: https://docs.rushdb.com/
```

Ou installez-le à la main :

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # only if rushdb is not found yet
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` écoute sur `http://127.0.0.1:3000`, avec ses données dans `./.rushdb` et le
jeton `local-token`. Dans un autre terminal, lancez `pip install rushdb` :

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
<summary>La même chose en TypeScript</summary>

`npm install @rushdb/javascript-sdk` :

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

Vous parlez déjà SQL ? Les labels sont des tables :

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Ensuite : le [démarrage rapide complet](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker et autres installations](https://docs.rushdb.com/engine/getting-started/installation/)

## Ce que voient les agents avant de requêter

La dernière ligne du démarrage rapide affiche le schéma qu’un agent lit en premier. Il vient du catalogue que chaque écriture met à jour, pas d’un parcours des enregistrements :

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

Sur une base volumineuse et variée, un agent ne charge pas tout cela. Il affine de la vue d’ensemble jusqu’aux valeurs exactes, une petite lecture à la fois (même exécution que ci-dessus) :

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

`get_schema_summary` renvoie le même catalogue en JSON, avec les nombres de valeurs distinctes et les valeurs des petits champs. Le serveur MCP propose chaque lecture comme outil : `findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues` et `getSchemaSummary`.

## Déploiement

| Où | Comment |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3` |
| Docker Compose | [`deploy/templates/docker-compose`](https://github.com/rush-db/rushdb/tree/main/deploy/templates/docker-compose), avec une variante HTTPS derrière Caddy |
| Kubernetes | [Chart Helm](https://github.com/rush-db/rushdb/tree/main/charts/rushdb): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token="$(openssl rand -hex 32)"` |
| VM Linux | [cloud-init](https://github.com/rush-db/rushdb/tree/main/deploy/templates/cloud-init) pour DigitalOcean, Hetzner, AWS, Google Cloud et Azure; [unité systemd](https://github.com/rush-db/rushdb/tree/main/deploy/templates/systemd) |
| Plateformes | [Fly.io](https://github.com/rush-db/rushdb/tree/main/deploy/templates/fly), [Render](https://github.com/rush-db/rushdb/tree/main/deploy/templates/render), [Coolify](https://github.com/rush-db/rushdb/tree/main/deploy/templates/coolify), [Dokploy](https://github.com/rush-db/rushdb/tree/main/deploy/templates/dokploy) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

Ports, sondes, dimensionnement et exécution comme service : [Deployment](https://docs.rushdb.com/engine/operations/deployment/). Toutes les options sur une page :
[docs.rushdb.com/deploy](https://docs.rushdb.com/deploy/).

### Autres façons d’installer

| Méthode | Commande | Notes |
|---|---|---|
| Script d’installation | `curl -fsSL https://rushdb.com/install.sh \| sh` | Linux (glibc 2.28+) et macOS 11+, x86-64 et arm64. `sh -s -- --version v3.0.0` fixe une version. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS et Linux. `brew services start rushdb` le lance en arrière-plan. |
| Docker | `rushdb/rushdb:3` | `linux/amd64` et `linux/arm64`, aussi `ghcr.io/rush-db/rushdb`. Tags `3`, `3.0` et versions exactes. |
| Cargo | `cargo binstall rushdb-server` · `cargo install rushdb-server --locked` | `binstall` récupère le binaire publié ; `install` le compile et demande un compilateur C/C++. |
| Archive de release | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Archives avec sommes SHA-256, un SBOM SPDX et des signatures Sigstore. |
| Sources | `cargo build --release -p rushdb-server --bin rushdb` | Chaîne d’outils fixée dans `rust-toolchain.toml`. Voir [Installation](https://docs.rushdb.com/engine/getting-started/installation/). |

Windows n’est pas pris en charge nativement ; utilisez WSL 2 ou Docker. Lisez
[Sécurité](https://docs.rushdb.com/engine/operations/security/) avant d’exposer un nœud au-delà
de la boucle locale.

## Ce que vous pouvez construire

- **Mémoire d’agent.** Les souvenirs sont des enregistrements délimités par utilisateur, agent ou session et liés au tour qui les a produits, si bien que le rappel indique d’où vient un souvenir. Des fournisseurs prêts à l’emploi pour OpenClaw et Hermes.
- **Cerveau d’entreprise et RAG.** Docs, CRM, support et facturation arrivent sous leur propre forme et sont liés par les clés qu’ils partagent déjà, comme un e-mail ou un identifiant de compte. La recherche filtre d’abord sur le bon compte ou la bonne version, puis classe par vecteur et BM25 ensemble, et chaque fait cite l’enregistrement qui le fonde.
- **Recherche à facettes.** Des barres de filtres dont les comptes viennent du catalogue, limités au filtre courant et exacts, avec une recherche hybride classée sous le même filtre. Aucun index de recherche séparé à synchroniser.
- **Recherche IA et assistants dans votre app.** Les index texte et vectoriels vivent dans la base et restent à jour au fil des écritures de votre app ; les SDK, le serveur MCP et les clients SQL lisent les mêmes enregistrements.
- **Analyse de données agentique.** L’agent lit quels labels et champs existent et comment chaque valeur s’écrit, puis écrit du SQL, du Cypher ou une agrégation sur ces champs. Chaque réponse est livrée avec une requête que chacun peut relancer. Avec un [jeton en lecture seule](https://docs.rushdb.com/engine/connectivity/authentication/#capability-tokens), le moteur refuse toute écriture de l’agent.
- **Apps dont le schéma évolue en route.** Envoyez n’importe quel JSON : les types sont déduits, les objets imbriqués deviennent des enregistrements liés, et les nouveaux champs apparaissent dans le catalogue avec l’écriture qui les ajoute. Aucune migration.

## Fonctionnalités

- **Schéma émergent.** Les types sont déduits valeur par valeur, les objets imbriqués deviennent des enregistrements reliés, et le catalogue tient des comptes exacts et des listes de valeurs. [Catalogue de schéma](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Requêtes de graphe.** Filtrez à travers les relations par label, restreignez-les par type et par direction, et liez les enregistrements associés pour les projections et les agrégations. [Relations et parcours](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Recherche intégrée.** Index vectoriels sur les vecteurs que vous fournissez, plein texte BM25 et recherche hybride qui combine les deux. [Index et recherche](https://docs.rushdb.com/engine/indexes-and-search/)
- **Agrégations.** `select` et `groupBy` avec comptages, sommes, moyennes, tranches temporelles et collections imbriquées. [Agrégations](https://docs.rushdb.com/engine/query/aggregations/)
- **Parle vos langages.** SQL via le protocole filaire PostgreSQL, Gremlin, SQL/PGQ, documents de style Mongo, ainsi qu'openCypher et ISO GQL en version release candidate. [Langages de requête](https://docs.rushdb.com/engine/languages/)
- **Durable.** Un journal d'écriture anticipée (WAL), des transactions, des sauvegardes avec restauration à un instant donné et des réplicas en lecture. [Exploitation](https://docs.rushdb.com/engine/operations/)
- **Un seul binaire.** HTTP, protocole filaire PostgreSQL, gRPC et Bolt depuis un seul processus, avec des tenants isolés en option. [Connectivité](https://docs.rushdb.com/engine/connectivity/)

## Se connecter

| Interface | Par défaut | Activation |
|---|---|---|
| [Protocole HTTP natif](https://docs.rushdb.com/engine/connectivity/http-api/) | `127.0.0.1:3000`, `POST /protocol/v1/execute` | Toujours actif ; `--port` le déplace |
| [Protocole PostgreSQL](https://docs.rushdb.com/engine/connectivity/postgresql-wire/) | `127.0.0.1:5432` | Actif tant que `sql` est activé (par défaut) ; `--pgwire-port 0` le coupe |
| [gRPC](https://docs.rushdb.com/engine/connectivity/grpc/) (`rushdb.v1.Database/Execute`) | Désactivé | `--grpc-port <port>` |
| [Bolt 5.0](https://docs.rushdb.com/engine/connectivity/bolt/) | Désactivé | `--bolt-port <port>` avec `cypher` dans `--languages` |
| [MCP local](https://docs.rushdb.com/connect/mcp/) (`@rushdb/mcp-server`, [tools](https://docs.rushdb.com/learn/reference/mcp/tools/)) | stdio: `npx -y @rushdb/mcp-server` | Mode direct vers un moteur auto-hébergé : `RUSHDB_MODE=direct`, `RUSHDB_ENGINE_URL`, `RUSHDB_ENGINE_TOKEN` |
| [MCP hébergé](https://docs.rushdb.com/connect/mcp/) | `https://mcp.rushdb.com/mcp`, OAuth pour ChatGPT, Claude.ai et d’autres clients web | Une fonctionnalité de RushDB Cloud ; rien à installer |

Tous les listeners écoutent sur `--listen` (par défaut `127.0.0.1`) et partagent un
même certificat TLS (`--tls-cert`, `--tls-key`). Plus de détails dans
[Connectivité](https://docs.rushdb.com/engine/connectivity/), [Authentification](https://docs.rushdb.com/engine/connectivity/authentication/) et
[Clients SQL et Cypher](https://docs.rushdb.com/connect/sql-and-cypher/). Les SDK, le serveur MCP et les intégrations
ci-dessous rejoignent un moteur auto-hébergé en mode direct : voir [Connect](https://docs.rushdb.com/connect/).

## Écosystème

| Projet | Pour |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | Applications TypeScript et Python |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor et autres clients MCP |
| [Skills d'agent](https://github.com/rush-db/skills) · [plugins d'agent](https://github.com/rush-db/agent-plugins) | Agents de code |
| [LangChain et LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | Retrieval, mémoire et stores de graphe de propriétés |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [contrat de mémoire](https://github.com/rush-db/agent-memory-contract) | Fournisseurs de mémoire pour agents |
| [Sink Debezium](https://github.com/rush-db/debezium-server-rushdb) · [plugin Obsidian](https://github.com/rush-db/obsidian-plugin) | Capture de données modifiées (CDC), notes |

Tous fonctionnent avec un moteur auto-hébergé. Pour laisser votre agent de code installer
RushDB, donnez-lui [rushdb.com/agent-setup](https://rushdb.com/agent-setup).

## Auto-hébergé et RushDB Cloud

Ce dépôt est le moteur, et vous pouvez l’auto-héberger en entier. [RushDB Cloud](https://app.rushdb.com) fait tourner le même moteur pour vous et ajoute les fonctionnalités indiquées ci-dessous. Les SDK et le serveur MCP fonctionnent avec les deux ; sur un moteur auto-hébergé, les SDK refusent un appel réservé au Cloud avec `CloudOnlyError` avant tout envoi, et le serveur MCP ne propose pas ces outils.

![Tableau de bord RushDB avec des données d’exemple synthétiques : le tableau des enregistrements, puis la vue graphe des entreprises et de leurs groupes de départements, avec un zoom sur un département dont les employés sont listés dans le panneau d’enregistrement](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| Fonctionnalité | Moteur auto-hébergé | RushDB Cloud |
|---|---|---|
| Moteur et API : enregistrements, requêtes de graphe, recherche, agrégations, schéma en direct | ✓ | ✓ |
| SDK et serveur MCP | ✓ mode direct, MCP via stdio | ✓ plus MCP hébergé avec OAuth |
| Import de JSON et CSV | ✓ | ✓ |
| [Tableau de bord](https://docs.rushdb.com/rushdb-cloud/) avec enregistrements, vue graphe et constructeur de requêtes | Utilisez les SDK ou n’importe quel client PostgreSQL | ✓ |
| [Embeddings gérés](https://docs.rushdb.com/rushdb-cloud/capabilities/) | Apportez les vecteurs de votre propre modèle | ✓ quota mensuel |
| [Recherche en langage naturel](https://docs.rushdb.com/learn/search-query/smart-search/) | Écrivez la requête vous-même ou avec un agent | ✓ quota mensuel |
| [Suggestions de relations](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | Créez les relations vous-même | ✓ |
| [Définitions](https://docs.rushdb.com/learn/search-query/definitions/) pour les métriques et les filtres | Gardez les règles dans vos prompts et votre SQL | ✓ |
| [Connexions](https://docs.rushdb.com/learn/data-sources/connections/) aux outils SaaS, et RushDB Sync pour les bases sur site | Votre propre code de synchronisation, ou le sink Debezium | ✓ |
| Projets et clés d’API | Tenants et jetons gérés par vous | ✓ |
| Accès d’équipe | — | Offres payantes |
| Sauvegardes | À votre charge : sauvegarde, restauration à un instant donné, réplicas | Offres payantes |

Un ✓ sous RushDB Cloud signifie que l’offre Free l’inclut. Offres et quotas sur [rushdb.com/pricing](https://rushdb.com/pricing).

## FAQ

**Est-ce prêt pour la production ?** SQL, le langage de requête JSON natif, Gremlin et
les documents de style Mongo sont stables. openCypher, ISO GQL et Bolt sont des release
candidates. La réplication est asynchrone, avec promotion manuelle. La
[page de statut](https://docs.rushdb.com/engine/reference/status/) liste chaque
interface et ses limites.

**Ai-je besoin d'une base de données vectorielle séparée ?** Non. Vecteurs, index de texte et
relations vivent dans un seul moteur, et un enregistrement peut être écrit avec
son vecteur dans un seul commit. Le moteur recherche dans les vecteurs que vous fournissez ;
RushDB Cloud peut les générer pour vous.

**Qu'est-ce que le schéma vivant ?** Un catalogue de chaque label, propriété, type, valeur
et forme de relation, mis à jour à chaque commit, avec des comptes exacts. Un seul appel
le renvoie, donc les agents et les outils ne devinent pas les noms de champs.
[Catalogue de schéma](https://docs.rushdb.com/engine/data-model/schema-catalog/)

---

[Contribuer](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Sécurité](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Journal des modifications](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (ancienne version, basée sur Neo4j) se trouve sur la
[branche `2.x`](https://github.com/rush-db/rushdb/tree/2.x) et reçoit des correctifs de sécurité
jusqu'au 28/10/2027. [Migrer depuis la 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
