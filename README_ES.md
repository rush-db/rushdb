<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="Logo de RushDB" width="96">

# RushDB

<sub>La base de datos para IA</sub>

**Grafo, vectores y analítica sobre los mismos registros. Cada escritura mantiene un esquema vivo de labels, campos y valores, para que los agentes consulten campos reales en lugar de adivinar.**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[Documentación](https://docs.rushdb.com) • [Inicio rápido](https://docs.rushdb.com/engine/getting-started/quickstart/) • [Discusiones](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • **Español** • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

Trae los datos tal como los tienes: JSON anidado, CSV, `INSERT` de SQL, escrituras en
Cypher o al estilo Mongo, o flujos de cambios desde tus bases de datos. RushDB los guarda
como registros tipados enlazados en un grafo, y esos mismos registros responden a consultas
de relaciones, búsqueda vectorial y de texto completo, y agregaciones, así que no hay un
segundo almacén que mantener sincronizado. Cada escritura también actualiza un catálogo de
los labels, campos, tipos y valores que existen, así que un agente consulta campos reales.
Conéctate por HTTP, el protocolo de red de PostgreSQL, Bolt, gRPC o MCP, desde los SDK o
desde cualquier cliente de PostgreSQL. Un solo binario de Rust, Apache-2.0.

| Grafo | Búsqueda vectorial y de texto completo | Analítica |
|---|---|---|
| El JSON anidado se convierte en registros enlazados. Filtra a través de relaciones y sigue relaciones tipadas y dirigidas en una sola consulta. | Búsqueda vectorial (exacta y HNSW), de texto completo BM25 e híbrida, filtrada con los mismos predicados de grafo. | Conteo, suma, media, mínimo y máximo con `groupBy`, o SQL normal con `GROUP BY` sobre el protocolo de red de PostgreSQL. |

## Inicio rápido

```bash
curl -fsSL https://rushdb.com/install.sh | sh    # or: brew install rush-db/tap/rushdb
export PATH="$HOME/.rushdb/bin:$PATH"             # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` sirve en `http://127.0.0.1:3000`, con sus datos en `./.rushdb` y el
token `local-token`. En otra terminal, ejecuta `pip install rushdb`:

```python
from rushdb import RushDB

db = RushDB("local-token", direct=True, engine_url="http://127.0.0.1:3000")

# Nested JSON becomes linked records: COMPANY -> DEPARTMENT -> EMPLOYEE
db.records.import_json(label="COMPANY", data={"name": "Acme", "DEPARTMENT": [
    {"name": "Engineering", "EMPLOYEE": [
        {"name": "Ada", "role": "Staff Engineer", "salary": 180000},
        {"name": "Linus", "role": "Engineer", "salary": 140000}]},
    {"name": "Sales", "EMPLOYEE": [{"name": "Grace", "role": "Account Executive", "salary": 120000}]}]})

# Graph: filter across relationships by naming the related labels
engineers = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {"role": {"$contains": "Engineer"}, "DEPARTMENT": {"COMPANY": {"name": "Acme"}}},
    "orderBy": {"name": "asc"},
})
print([employee.data["name"] for employee in engineers])  # ['Ada', 'Linus']

# Analytics: headcount and average salary per department
per_department = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {"DEPARTMENT": {"$alias": "$department"}},
    "select": {"people": {"$count": "*"}, "avgSalary": {"$avg": "$record.salary"}},
    "groupBy": ["$department.name"],
})
print([row.data for row in per_department])
# [{'name': 'Engineering', 'people': 2, 'avgSalary': 160000.0}, {'name': 'Sales', 'people': 1, 'avgSalary': 120000.0}]

# The live schema an agent reads before it writes a query
print(db.ai.get_schema_markdown().data)
```

El esquema sale del catálogo, no de recorrer los registros:

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
```

<details>
<summary>Lo mismo en TypeScript</summary>

`npm install @rushdb/javascript-sdk`:

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('local-token', { direct: true, engineUrl: 'http://127.0.0.1:3000' })

// Nested JSON becomes linked records: COMPANY -> DEPARTMENT -> EMPLOYEE
await db.records.importJson({
  label: 'COMPANY',
  data: { name: 'Acme', DEPARTMENT: [
    { name: 'Engineering', EMPLOYEE: [
      { name: 'Ada', role: 'Staff Engineer', salary: 180000 },
      { name: 'Linus', role: 'Engineer', salary: 140000 } ] },
    { name: 'Sales', EMPLOYEE: [{ name: 'Grace', role: 'Account Executive', salary: 120000 }] } ] }
})

// Graph: filter across relationships by naming the related labels
const engineers = await db.records.find({
  labels: ['EMPLOYEE'],
  where: { role: { $contains: 'Engineer' }, DEPARTMENT: { COMPANY: { name: 'Acme' } } },
  orderBy: { name: 'asc' }
})
console.log(engineers.data.map((employee) => employee.data.name)) // [ 'Ada', 'Linus' ]

// Analytics: headcount and average salary per department
const perDepartment = await db.records.find({
  labels: ['EMPLOYEE'],
  where: { DEPARTMENT: { $alias: '$department' } },
  select: { people: { $count: '*' }, avgSalary: { $avg: '$record.salary' } },
  groupBy: ['$department.name']
})
console.log(perDepartment.data.map((row) => row.data))
// [ { name: 'Engineering', people: 2, avgSalary: 160000 }, { name: 'Sales', people: 1, avgSalary: 120000 } ]

// The live schema an agent reads before it writes a query
console.log((await db.ai.getSchemaMarkdown()).data)
```

</details>

¿Ya hablas SQL? Los labels son tablas:

```bash
PGPASSWORD=local-token psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Siguiente: el [inicio rápido completo](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker y otras instalaciones](https://docs.rushdb.com/engine/getting-started/installation/)

## Qué puedes construir

| | |
|---|---|
| **Memoria para agentes** | Recuerdos como registros con relaciones, filtrados por ámbito y ordenados según los vectores que guardas. Proveedores listos para usar con OpenClaw y Hermes. |
| **Búsqueda sobre los datos de tu aplicación** | Búsqueda de texto completo, vectorial e híbrida sobre los registros que tu aplicación ya escribe, con filtros de grafo en la misma consulta. |
| **Preguntas sobre datos conectados** | Un agente lee el esquema y luego escribe una consulta que puedes revisar: en el lenguaje de consultas JSON, en SQL o en Cypher. |
| **Informes sin exportar** | Agrupa y agrega los registros que escribe tu aplicación, desde los SDK o desde cualquier cliente de PostgreSQL. |

## Características

- **Esquema emergente.** Los tipos se infieren por valor, los objetos anidados se convierten en registros enlazados y el catálogo mantiene conteos exactos y listas de valores. [Catálogo de esquema](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Consultas de grafo.** Filtra a través de relaciones por label, restríngelas por tipo y dirección, y vincula registros relacionados para proyecciones y agregaciones. [Relaciones y recorrido](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Búsqueda integrada.** Índices vectoriales sobre los vectores que aportas, texto completo BM25 y búsqueda híbrida que combina ambos. [Índices y búsqueda](https://docs.rushdb.com/engine/indexes-and-search/)
- **Agregaciones.** `select` y `groupBy` con conteos, sumas, medias, intervalos de tiempo y colecciones anidadas. [Agregaciones](https://docs.rushdb.com/engine/query/aggregations/)
- **Habla lo que usas.** SQL sobre el protocolo de red de PostgreSQL, Gremlin, SQL/PGQ, documentos al estilo Mongo, y openCypher e ISO GQL como release candidates. [Lenguajes de consulta](https://docs.rushdb.com/engine/languages/)
- **Duradero.** Un registro de escritura anticipada (WAL), transacciones, copias de seguridad con restauración a un punto en el tiempo y réplicas de lectura. [Operaciones](https://docs.rushdb.com/engine/operations/)
- **Un solo binario.** HTTP, protocolo de red de PostgreSQL, gRPC y Bolt desde un único proceso, con tenants aislados opcionales. [Conectividad](https://docs.rushdb.com/engine/connectivity/)

## Ecosistema

| Proyecto | Para |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | Aplicaciones TypeScript y Python |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor y otros clientes MCP |
| [Skills para agentes](https://github.com/rush-db/skills) · [plugins para agentes](https://github.com/rush-db/agent-plugins) | Agentes de programación |
| [LangChain y LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | Recuperación, memoria y almacenes de grafos de propiedades |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [contrato de memoria](https://github.com/rush-db/agent-memory-contract) | Proveedores de memoria para agentes |
| [Sink de Debezium](https://github.com/rush-db/debezium-server-rushdb) · [plugin de Obsidian](https://github.com/rush-db/obsidian-plugin) | Captura de datos de cambios (CDC), notas |

Todos funcionan con un motor autoalojado. Para que tu agente de programación configure
RushDB, pásale [rushdb.com/agent-setup](https://rushdb.com/agent-setup).

## Ejecútalo en cualquier parte

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes (Helm), cloud-init, systemd, Fly.io, Render,
Coolify y Dokploy: consulta [Deploy](https://docs.rushdb.com/deploy/). Lee
[Seguridad](https://docs.rushdb.com/engine/operations/security/) antes de
exponer un nodo más allá de loopback.

## Autoalojado y RushDB Cloud

- **Autoalojado:** este repositorio. Todo lo anterior se ejecuta en el motor que instalas.
- **[RushDB Cloud](https://app.rushdb.com):** el mismo motor, gestionado, más un
  panel de control, embeddings gestionados, búsqueda en lenguaje natural y conectores para
  herramientas SaaS. Los SDK y el servidor MCP funcionan con ambos.

## Preguntas frecuentes

**¿Está listo para producción?** SQL, el lenguaje de consultas JSON nativo, Gremlin y
los documentos al estilo Mongo son estables. openCypher, ISO GQL y Bolt son release
candidates. La replicación es asíncrona con promoción manual. La
[página de estado](https://docs.rushdb.com/engine/reference/status/) enumera cada
interfaz y sus límites.

**¿Necesito una base de datos vectorial aparte?** No. Los vectores, los índices de texto y
las relaciones viven en un solo motor, y un registro puede escribirse junto con
su vector en un único commit. El motor busca en los vectores que aportas; RushDB
Cloud puede generarlos por ti.

**¿Qué es el esquema vivo?** Un catálogo de cada label, propiedad, tipo, valor
y forma de relación, actualizado en cada commit, con conteos exactos. Una sola llamada
lo devuelve, así que los agentes y las herramientas no adivinan nombres de campos.
[Catálogo de esquema](https://docs.rushdb.com/engine/data-model/schema-catalog/)

**¿Windows?** Usa WSL 2 o Docker.

---

[Contribuir](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Seguridad](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Registro de cambios](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (legado, basado en Neo4j) vive en la
[rama `2.x`](https://github.com/rush-db/rushdb/tree/2.x) con correcciones de seguridad
hasta el 28/10/2027. [Actualizar desde 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
