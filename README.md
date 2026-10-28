<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### One database for your records, their relationships, and search over them.

Write JSON. RushDB infers the types, links nested objects into a graph, and
lets you query it with its JSON query language, SQL, Cypher, Gremlin or
Mongo-style documents, plus vector, full-text and hybrid search. One Rust
binary, one data directory, Apache-2.0.

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[Website](https://rushdb.com) • [Documentation](https://docs.rushdb.com) • [Manual](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [Deploy](deploy/README.md)

**English** • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## Quick start

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

The script installs the `rushdb` binary into `~/.rushdb/bin` after checking
the release checksum (and its Sigstore signature when `cosign` is installed).
`rushdb up` starts a durable node on `http://127.0.0.1:3000` with its data in
`./.rushdb` and the local token `local-token`.

Write a nested JSON document. Every nested object becomes its own record,
linked to its parent:

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

Then find the engineers at Acme. The filter reaches across relationships by
naming the related labels:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

Continue with the [Quickstart](docs/manual/getting-started/quickstart.md).

## Query it your way

The same records answer every interface.

**SQL** over the PostgreSQL wire protocol, with `psql` or any PostgreSQL
driver. Labels are tables:

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher** over HTTP, or over Bolt with the Neo4j drivers
(`rushdb up --languages sql,cypher --bolt-port 7687`):

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**The SDKs** in direct mode (`npm install @rushdb/javascript-sdk`):

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, Mongo-style documents and ISO GQL use the
same HTTP endpoint. See [Query languages](docs/manual/languages/README.md).

## What's in the box

| Capability | What it means |
|---|---|
| **No schema to design** | Write any JSON shape. Types are inferred per value, nested objects become linked records, and the engine keeps an exact catalog of labels, properties and relationships that you can query. |
| **Properties are first-class** | The Labeled Meta-Property Graph (LMPG) stores each property as a typed definition shared by the records that use it, so schema discovery, value ranges and facets come from the storage layout itself. |
| **Many query languages** | A native JSON query language, SQL, Gremlin, SQL/PGQ, Mongo-style documents, and openCypher and ISO GQL as release candidates. |
| **Search** | Vector indexes (exact and HNSW) over the vectors you supply, BM25 full-text indexes, and hybrid search that fuses both. |
| **Durable and transactional** | A write-ahead log, MVCC snapshots, multi-statement transactions, backups with point-in-time restore, and streaming replication to read-only followers. |
| **Standard protocols** | HTTP, the PostgreSQL wire protocol, gRPC and Bolt from one process. |
| **Multitenancy** | Optional fail-closed tenants, each with its own keyspace. |
| **One binary** | `rushdb serve` runs the server; `rushdb up` runs a local node. No other service to operate. |

## Deploy

| Where | How |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/), with an HTTPS variant behind Caddy |
| Kubernetes | [Helm chart](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| Linux VM | [cloud-init](deploy/templates/cloud-init/) for DigitalOcean, Hetzner, AWS, GCP, Azure; [systemd unit](deploy/templates/systemd/) |
| Platforms | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### Other ways to install

| Method | Command | Notes |
|---|---|---|
| Install script | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) and macOS, x86-64 and arm64. `sh -s -- --version v3.0.0` pins a release. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS and Linux. `brew services start rushdb` runs it in the background. |
| Docker | `rushdb/rushdb` | `linux/amd64` and `linux/arm64`; also `ghcr.io/rush-db/rushdb`. |
| Cargo | `cargo binstall rushdb-server` or `cargo install rushdb-server --locked` | `binstall` fetches the release binary; `install` compiles (needs a C++ compiler). |
| Release archive | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Archives with SHA-256 checksums, an SBOM, and Sigstore signatures. |
| Source | `cargo build --release -p rushdb-server --bin rushdb` | See [Installation](docs/manual/getting-started/installation.md). |

Windows is not supported natively; use WSL 2 or Docker. Read
[Security](docs/manual/operations/security.md) before exposing a node beyond
loopback.

## Status

| Area | Status |
|---|---|
| Native protocol, SQL, Gremlin text traversals, Mongo-style documents | Stable |
| SQL/PGQ `GRAPH_TABLE` | Stable, read-only |
| openCypher, ISO GQL, Bolt | Release candidate, opt-in: the external conformance suites do not pass in full ([details](docs/manual/languages/README.md)) |
| PostgreSQL wire | SQL reads and single-statement writes; checked with `psql`, node-postgres, psycopg and `tokio-postgres`, not every client or ORM |
| Replication | Asynchronous streaming to read-only followers; promotion is manual, with no automatic failover |
| Raft, storage sharding | Preview, not for production: Raft runs in one process with in-memory state; multi-shard commits are not crash-atomic |

The [manual](docs/manual/README.md) documents each surface's exact limits.

## Connect

| Surface | Default | Enable |
|---|---|---|
| HTTP native protocol | `127.0.0.1:3000`, `POST /protocol/v1/execute` | always on |
| PostgreSQL wire | `127.0.0.1:5432` | on with `sql` (default) |
| gRPC (`rushdb.v1.Database/Execute`) | off | `--grpc-port` |
| Bolt 5.0 | off | `--bolt-port` with `cypher` |

The [JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) and
[Python](https://pypi.org/project/rushdb/) SDKs, the
[MCP server](https://www.npmjs.com/package/@rushdb/mcp-server) and the
[agent skills](https://github.com/rush-db/skills) work with a self-hosted
engine in direct mode.

## Self-hosted engine and RushDB Cloud

This engine is the part of RushDB you self-host. It does not generate
embeddings, host a dashboard, or manage users: those come with
[RushDB Cloud](https://app.rushdb.com), the managed service that runs this
engine and adds the dashboard, managed embeddings and the AI features
(natural-language search, relationship-pattern suggestions). The SDKs and the
MCP server work with either: give them a RushDB Cloud API key, or point them
at your engine (`direct: true`, `engineUrl`) for everything except the Cloud
features, which they refuse with `CloudOnlyError`.

## Documentation

- [Manual](docs/manual/README.md): getting started, data model, queries,
  languages, search, operations, and reference tables.
- [docs.rushdb.com](https://docs.rushdb.com): tutorials, the SDKs and the MCP
  server.
- [Upgrading from RushDB 2.x](docs/manual/operations/upgrading-from-2x.md):
  2.x was the Neo4j-backed stack, kept at the `v2` tag of this repository.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The quality gate is:

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

Licensed under [Apache-2.0](LICENSE).
