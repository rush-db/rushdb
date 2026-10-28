<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### Une seule base de données pour vos enregistrements, leurs relations et la recherche.

Écrivez du JSON. RushDB déduit les types, relie les objets imbriqués en un graphe et vous permet de l'interroger avec son langage de requête JSON, SQL, Cypher, Gremlin ou des documents de style Mongo, ainsi qu'avec la recherche vectorielle, plein texte et hybride. Un binaire Rust, un répertoire de données, Apache-2.0.

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[Site web](https://rushdb.com) • [Documentation](https://docs.rushdb.com) • [Manuel](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [Déploiement](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • **Français** • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## Démarrage rapide

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

Le script vérifie la somme de contrôle de la version (et sa signature Sigstore si `cosign` est installé), puis installe le binaire `rushdb` dans `~/.rushdb/bin`. `rushdb up` démarre un nœud persistant sur `http://127.0.0.1:3000`, avec ses données dans `./.rushdb` et le jeton local `local-token`.

Écrivez un document JSON imbriqué. Chaque objet imbriqué devient un enregistrement à part entière, relié à son parent :

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

Cherchez ensuite les ingénieurs d'Acme. Le filtre traverse les relations en nommant les étiquettes liées :

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

Poursuivez avec le [guide de démarrage](docs/manual/getting-started/quickstart.md).

## Interrogez-la à votre façon

Les mêmes enregistrements répondent à toutes les interfaces.

**SQL** via le protocole PostgreSQL, avec `psql` ou n'importe quel pilote PostgreSQL. Les étiquettes sont des tables :

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher** via HTTP, ou via Bolt avec les pilotes Neo4j (`rushdb up --languages sql,cypher --bolt-port 7687`) :

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**Les SDK** en mode direct (`npm install @rushdb/javascript-sdk`) :

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, les documents de style Mongo et ISO GQL passent par le même point d'accès HTTP. Voir [Langages de requête](docs/manual/languages/README.md).

## Ce qu'elle offre

| Fonctionnalité | Ce que cela signifie |
|---|---|
| **Aucun schéma à concevoir** | Écrivez du JSON de n'importe quelle forme. Les types sont déduits valeur par valeur, les objets imbriqués deviennent des enregistrements liés, et le moteur tient un catalogue exact, interrogeable, des étiquettes, propriétés et relations. |
| **Les propriétés sont de premier ordre** | Le Labeled Meta-Property Graph (LMPG) stocke chaque propriété comme une définition typée partagée par les enregistrements qui l'utilisent : la découverte du schéma, les plages de valeurs et les facettes viennent de la disposition du stockage elle-même. |
| **De nombreux langages de requête** | Un langage de requête JSON natif, SQL, Gremlin, SQL/PGQ, des documents de style Mongo, ainsi qu'openCypher et ISO GQL en version candidate. |
| **Recherche** | Index vectoriels (exacts et HNSW) sur les vecteurs que vous fournissez, index plein texte BM25 et recherche hybride qui combine les deux. |
| **Durable et transactionnelle** | Journal d'écriture anticipée, instantanés MVCC, transactions multi-instructions, sauvegardes avec restauration à un instant donné et réplication en continu vers des suiveurs en lecture seule. |
| **Protocoles standard** | HTTP, le protocole PostgreSQL, gRPC et Bolt depuis un seul processus. |
| **Multilocataire** | Locataires optionnels à refus par défaut, chacun avec son propre espace de clés. |
| **Un seul binaire** | `rushdb serve` lance le serveur, `rushdb up` un nœud local. Aucun autre service à exploiter. |

## Déploiement

| Où | Comment |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/), avec une variante HTTPS derrière Caddy |
| Kubernetes | [Chart Helm](charts/rushdb/) : `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| VM Linux | [cloud-init](deploy/templates/cloud-init/) pour DigitalOcean, Hetzner, AWS, GCP, Azure ; [unité systemd](deploy/templates/systemd/) |
| Plateformes | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### Autres moyens d'installation

| Méthode | Commande | Remarques |
|---|---|---|
| Script d'installation | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) et macOS, x86-64 et arm64. `sh -s -- --version v3.0.0` fixe une version. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS et Linux. `brew services start rushdb` le lance en arrière-plan. |
| Docker | `rushdb/rushdb` | `linux/amd64` et `linux/arm64` ; aussi `ghcr.io/rush-db/rushdb`. |
| Cargo | `cargo binstall rushdb-server` ou `cargo install rushdb-server --locked` | `binstall` récupère le binaire publié ; `install` compile (nécessite un compilateur C++). |
| Archive de version | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Archives avec sommes de contrôle SHA-256, SBOM et signatures Sigstore. |
| Sources | `cargo build --release -p rushdb-server --bin rushdb` | Voir [Installation](docs/manual/getting-started/installation.md). |

Windows n'est pas pris en charge nativement ; utilisez WSL 2 ou Docker. Lisez [Sécurité](docs/manual/operations/security.md) avant d'exposer un nœud au-delà de la boucle locale.

## État

| Domaine | État |
|---|---|
| Protocole natif, SQL, parcours Gremlin textuels, documents de style Mongo | Stable |
| SQL/PGQ `GRAPH_TABLE` | Stable, lecture seule |
| openCypher, ISO GQL, Bolt | Version candidate, à activer : les suites de conformité externes ne passent pas entièrement ([détails](docs/manual/languages/README.md)) |
| Protocole PostgreSQL | Lectures SQL et écritures en une instruction ; vérifié avec `psql`, node-postgres, psycopg et `tokio-postgres`, pas avec tous les clients ou ORM |
| Réplication | Flux asynchrone vers des suiveurs en lecture seule ; la promotion est manuelle, sans bascule automatique |
| Raft, partitionnement du stockage | Aperçu, pas pour la production : Raft tourne dans un processus avec un état en mémoire ; les commits multi-partitions ne sont pas atomiques en cas de panne |

Le [manuel](docs/manual/README.md) documente les limites exactes de chaque interface.

## Se connecter

| Interface | Par défaut | Activation |
|---|---|---|
| Protocole HTTP natif | `127.0.0.1:3000`, `POST /protocol/v1/execute` | toujours actif |
| Protocole PostgreSQL | `127.0.0.1:5432` | actif avec `sql` (par défaut) |
| gRPC (`rushdb.v1.Database/Execute`) | désactivé | `--grpc-port` |
| Bolt 5.0 | désactivé | `--bolt-port` avec `cypher` |

Les SDK [JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) et [Python](https://pypi.org/project/rushdb/), le [serveur MCP](https://www.npmjs.com/package/@rushdb/mcp-server) et les [compétences pour agents](https://github.com/rush-db/skills) fonctionnent avec un moteur auto-hébergé en mode direct.

## Moteur auto-hébergé et RushDB Cloud

Ce moteur est la partie de RushDB que vous auto-hébergez. Il ne génère pas d'embeddings, n'héberge pas de tableau de bord et ne gère pas d'utilisateurs : cela vient avec [RushDB Cloud](https://app.rushdb.com), le service géré qui exploite ce moteur et ajoute le tableau de bord, les embeddings gérés et les fonctions d'IA (recherche en langage naturel, suggestions de motifs de relations). Les SDK et le serveur MCP fonctionnent avec l'un ou l'autre : donnez-leur une clé d'API RushDB Cloud, ou pointez-les vers votre moteur (`direct: true`, `engineUrl`) pour tout sauf les fonctions Cloud, qu'ils refusent avec `CloudOnlyError`.

## Documentation

- [Manuel](docs/manual/README.md) : prise en main, modèle de données, requêtes, langages, recherche, exploitation et tables de référence.
- [docs.rushdb.com](https://docs.rushdb.com) : tutoriels, SDK et serveur MCP.
- [Mise à niveau depuis RushDB 2.x](docs/manual/operations/upgrading-from-2x.md) : la 2.x était la pile basée sur Neo4j, conservée au tag `v2` de ce dépôt.

## Contribuer

Voir [CONTRIBUTING.md](CONTRIBUTING.md). Le contrôle qualité :

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [Politique de sécurité](SECURITY.md)
- [Processus de publication](RELEASING.md)
- [Code de conduite](CODE_OF_CONDUCT.md)
- [Journal des modifications](CHANGELOG.md)

Sous licence [Apache-2.0](LICENSE).
