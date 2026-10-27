<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### Una sola base de datos para tus registros, sus relaciones y la búsqueda sobre ellos.

Escribe JSON. RushDB infiere los tipos, enlaza los objetos anidados en un grafo y te permite consultarlo con su lenguaje de consultas JSON, SQL, Cypher, Gremlin o documentos al estilo Mongo, además de búsqueda vectorial, de texto completo e híbrida. Un binario de Rust, un directorio de datos, Apache-2.0.

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[Sitio web](https://rushdb.com) • [Documentación](https://docs.rushdb.com) • [Manual](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [Despliegue](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • **Español** • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## Inicio rápido

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

El script comprueba el checksum de la versión (y su firma Sigstore si `cosign` está instalado) e instala el binario `rushdb` en `~/.rushdb/bin`. `rushdb up` arranca un nodo persistente en `http://127.0.0.1:3000`, con los datos en `./.rushdb` y el token local `local-token`.

Escribe un documento JSON anidado. Cada objeto anidado se convierte en un registro propio, enlazado con su padre:

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

Después busca a los ingenieros de Acme. El filtro recorre las relaciones nombrando las etiquetas relacionadas:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

Continúa con el [inicio rápido del manual](docs/manual/getting-started/quickstart.md).

## Consulta a tu manera

Los mismos registros responden por todas las interfaces.

**SQL** por el protocolo de red de PostgreSQL, con `psql` o cualquier driver de PostgreSQL. Las etiquetas son tablas:

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher** por HTTP, o por Bolt con los drivers de Neo4j (`rushdb up --languages sql,cypher --bolt-port 7687`):

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**Los SDK** en modo directo (`npm install @rushdb/javascript-sdk`):

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, los documentos al estilo Mongo e ISO GQL usan el mismo endpoint HTTP. Consulta [Lenguajes de consulta](docs/manual/languages/README.md).

## Qué incluye

| Capacidad | Qué significa |
|---|---|
| **Sin esquema que diseñar** | Escribe JSON de cualquier forma. Los tipos se infieren por valor, los objetos anidados se convierten en registros enlazados y el motor mantiene un catálogo exacto y consultable de etiquetas, propiedades y relaciones. |
| **Propiedades de primera clase** | El Labeled Meta-Property Graph (LMPG) guarda cada propiedad como una definición tipada compartida por los registros que la usan, así que el descubrimiento del esquema, los rangos de valores y las facetas salen de la propia disposición del almacenamiento. |
| **Muchos lenguajes de consulta** | Un lenguaje de consultas JSON nativo, SQL, Gremlin, SQL/PGQ, documentos al estilo Mongo, y openCypher e ISO GQL como release candidates. |
| **Búsqueda** | Índices vectoriales (exactos y HNSW) sobre los vectores que aportas, índices de texto completo BM25 y búsqueda híbrida que combina ambos. |
| **Duradera y transaccional** | Registro de escritura anticipada, snapshots MVCC, transacciones de varias sentencias, copias de seguridad con restauración a un punto en el tiempo y replicación en streaming a seguidores de solo lectura. |
| **Protocolos estándar** | HTTP, el protocolo de red de PostgreSQL, gRPC y Bolt desde un solo proceso. |
| **Multitenencia** | Inquilinos opcionales que fallan cerrados, cada uno con su propio espacio de claves. |
| **Un solo binario** | `rushdb serve` ejecuta el servidor; `rushdb up`, un nodo local. Ningún otro servicio que operar. |

## Despliegue

| Dónde | Cómo |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/), con una variante HTTPS detrás de Caddy |
| Kubernetes | [Chart de Helm](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| VM Linux | [cloud-init](deploy/templates/cloud-init/) para DigitalOcean, Hetzner, AWS, GCP, Azure; [unidad systemd](deploy/templates/systemd/) |
| Plataformas | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### Otras formas de instalar

| Método | Comando | Notas |
|---|---|---|
| Script de instalación | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) y macOS, x86-64 y arm64. `sh -s -- --version v3.0.0` fija una versión. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS y Linux. `brew services start rushdb` lo ejecuta en segundo plano. |
| Docker | `rushdb/rushdb` | `linux/amd64` y `linux/arm64`; también `ghcr.io/rush-db/rushdb`. |
| Cargo | `cargo binstall rushdb-server` o `cargo install rushdb-server --locked` | `binstall` descarga el binario publicado; `install` compila (necesita un compilador de C++). |
| Archivo de la versión | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Archivos con checksums SHA-256, SBOM y firmas Sigstore. |
| Código fuente | `cargo build --release -p rushdb-server --bin rushdb` | Consulta [Instalación](docs/manual/getting-started/installation.md). |

Windows no está soportado de forma nativa; usa WSL 2 o Docker. Lee [Seguridad](docs/manual/operations/security.md) antes de exponer un nodo más allá de loopback.

## Estado

| Área | Estado |
|---|---|
| Protocolo nativo, SQL, recorridos de texto Gremlin, documentos al estilo Mongo | Estable |
| SQL/PGQ `GRAPH_TABLE` | Estable, solo lectura |
| openCypher, ISO GQL, Bolt | Release candidate, opcional: las suites de conformidad externas no pasan por completo ([detalles](docs/manual/languages/README.md)) |
| Protocolo de PostgreSQL | Lecturas SQL y escrituras de una sola sentencia; verificado con `psql`, node-postgres, psycopg y `tokio-postgres`, no con todos los clientes u ORM |
| Replicación | Streaming asíncrono a seguidores de solo lectura; la promoción es manual, sin conmutación por error automática |
| Raft, particionado del almacenamiento | Vista previa, no para producción: Raft se ejecuta en un proceso con estado en memoria; los commits entre particiones no son atómicos ante fallos |

El [manual](docs/manual/README.md) documenta los límites exactos de cada interfaz.

## Conectar

| Interfaz | Por defecto | Activación |
|---|---|---|
| Protocolo HTTP nativo | `127.0.0.1:3000`, `POST /protocol/v1/execute` | siempre activo |
| Protocolo de PostgreSQL | `127.0.0.1:5432` | activo con `sql` (por defecto) |
| gRPC (`rushdb.v1.Database/Execute`) | desactivado | `--grpc-port` |
| Bolt 5.0 | desactivado | `--bolt-port` con `cypher` |

Los SDK de [JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) y [Python](https://pypi.org/project/rushdb/), el [servidor MCP](https://www.npmjs.com/package/@rushdb/mcp-server) y las [skills para agentes](https://github.com/rush-db/skills) funcionan con un motor autoalojado en modo directo.

## Motor autoalojado y RushDB Cloud

Este motor es la parte de RushDB que tú alojas. No genera embeddings, no aloja un panel y no gestiona usuarios: eso viene con [RushDB Cloud](https://app.rushdb.com), el servicio gestionado que ejecuta este motor y añade el panel, los embeddings gestionados y las funciones de IA (búsqueda en lenguaje natural, sugerencias de patrones de relación). Los SDK y el servidor MCP funcionan con cualquiera de los dos: dales una clave de API de RushDB Cloud o apúntalos a tu motor (`direct: true`, `engineUrl`) para todo salvo las funciones de Cloud, que rechazan con `CloudOnlyError`.

## Documentación

- [Manual](docs/manual/README.md): primeros pasos, modelo de datos, consultas, lenguajes, búsqueda, operación y tablas de referencia.
- [docs.rushdb.com](https://docs.rushdb.com): tutoriales, los SDK y el servidor MCP.
- [Actualizar desde RushDB 2.x](docs/manual/operations/upgrading-from-2x.md): la 2.x era la pila basada en Neo4j, conservada en la etiqueta `v2` de este repositorio.

## Contribuir

Consulta [CONTRIBUTING.md](CONTRIBUTING.md). El control de calidad:

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [Política de seguridad](SECURITY.md)
- [Proceso de release](RELEASING.md)
- [Código de conducta](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

Con licencia [Apache-2.0](LICENSE).
