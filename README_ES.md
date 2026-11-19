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

![Grabación de terminal: instalar RushDB, iniciarlo con rushdb up y, desde Python, importar JSON anidado, filtrar empleados por departamento, contar personas y calcular el salario medio por departamento, e imprimir el esquema vivo](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` sirve en `http://127.0.0.1:3000`, con sus datos en `./.rushdb` y el
token `local-token`. En otra terminal, ejecuta `pip install rushdb`:

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
<summary>Lo mismo en TypeScript</summary>

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

¿Ya hablas SQL? Los labels son tablas:

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Siguiente: el [inicio rápido completo](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker y otras instalaciones](https://docs.rushdb.com/engine/getting-started/installation/)

## Lo que ven los agentes antes de consultar

La última línea del inicio rápido imprime el esquema que un agente lee primero. Sale del catálogo que cada escritura actualiza, no de un recorrido de los registros:

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

En un almacén grande y variado, un agente no carga todo eso. Acota desde la vista general hasta los valores exactos, con una lectura pequeña cada vez (la misma ejecución de arriba):

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

`get_schema_summary` devuelve el mismo catálogo en JSON, con recuentos de valores distintos y los valores de los campos pequeños. El servidor MCP ofrece cada lectura como herramienta: `findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues` y `getSchemaSummary`.

## Qué puedes construir

- **Memoria de agentes.** Los recuerdos son registros acotados por usuario, agente o sesión y enlazados al turno que los produjo, así que la recuperación muestra de dónde viene cada recuerdo. Proveedores listos para OpenClaw y Hermes.
- **Cerebro de empresa y RAG.** Documentos, CRM, soporte y facturación llegan con su propia forma y se enlazan por claves que ya comparten, como un correo o un ID de cuenta. La recuperación filtra primero por la cuenta o versión correcta y luego ordena por vector y BM25 a la vez, y cada dato cita el registro que lo respalda.
- **Búsqueda facetada.** Barras de filtros cuyos recuentos salen del catálogo, acotados al filtro actual y exactos, con búsqueda híbrida ordenada bajo el mismo filtro. Sin un índice de búsqueda aparte que sincronizar.
- **Búsqueda con IA y asistentes en tu app.** Los índices de texto y vectoriales viven en la base de datos y se mantienen al día mientras tu app escribe; los SDK, el servidor MCP y los clientes SQL leen los mismos registros.
- **Análisis de datos con agentes.** El agente lee qué etiquetas y campos existen y cómo se escribe cada valor, y luego escribe SQL, Cypher o una agregación sobre ellos. Cada respuesta incluye una consulta que cualquiera puede volver a ejecutar.
- **Apps cuyo esquema evoluciona sobre la marcha.** Envía cualquier JSON: los tipos se infieren, los objetos anidados se convierten en registros enlazados y los campos nuevos aparecen en el catálogo con la escritura que los añade. Sin migraciones.

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
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data \
  -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes (Helm), cloud-init, systemd, Fly.io, Render,
Coolify y Dokploy: consulta [Deploy](https://docs.rushdb.com/deploy/). Lee
[Seguridad](https://docs.rushdb.com/engine/operations/security/) antes de
exponer un nodo más allá de loopback.

## Autoalojado y RushDB Cloud

Este repositorio es el motor, y puedes alojarlo tú mismo por completo. [RushDB Cloud](https://app.rushdb.com) ejecuta el mismo motor por ti y añade las funciones marcadas abajo. Los SDK y el servidor MCP funcionan con ambos; en un motor autoalojado, una llamada exclusiva de Cloud falla con `CloudOnlyError` antes de enviar nada.

![Panel de RushDB con datos de ejemplo sintéticos: la tabla de registros, luego la vista de grafo de las empresas y sus grupos de departamentos, con zoom en un departamento cuyos empleados aparecen en el panel del registro](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| Función | Motor autoalojado | RushDB Cloud |
|---|---|---|
| Motor y API: registros, consultas de grafo, búsqueda, agregaciones, esquema en vivo | ✓ | ✓ |
| SDK y servidor MCP | ✓ modo directo, MCP por stdio | ✓ más MCP alojado con OAuth |
| Importación de JSON y CSV | ✓ | ✓ |
| [Panel](https://docs.rushdb.com/rushdb-cloud/) con registros, vista de grafo y constructor de consultas | Usa los SDK o cualquier cliente de PostgreSQL | ✓ |
| [Embeddings gestionados](https://docs.rushdb.com/rushdb-cloud/capabilities/) | Trae vectores de tu propio modelo | ✓ cuota mensual |
| [Búsqueda en lenguaje natural](https://docs.rushdb.com/learn/search-query/smart-search/) | Escribe la consulta tú o con un agente | ✓ cuota mensual |
| [Sugerencias de relaciones](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | Crea las relaciones tú mismo | ✓ |
| [Definiciones](https://docs.rushdb.com/learn/search-query/definitions/) para métricas y filtros | Mantén las reglas en tus prompts y tu SQL | ✓ |
| [Conexiones](https://docs.rushdb.com/learn/data-sources/connections/) con herramientas SaaS, y RushDB Sync para bases de datos locales | Tu propio código de sincronización, o el sink de Debezium | ✓ |
| Proyectos y claves de API | Tenants y tokens que gestionas tú | ✓ |
| Acceso de equipo | — | Planes de pago |
| Copias de seguridad | Las gestionas tú: copia, restauración a un punto en el tiempo, réplicas | Planes de pago |

Un ✓ bajo RushDB Cloud significa que el plan Free lo incluye. Planes y cuotas en [rushdb.com/pricing](https://rushdb.com/pricing).

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
