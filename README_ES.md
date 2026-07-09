<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### La capa de memoria para agentes y aplicaciones de IA.

Envía cualquier JSON. Tu agente obtiene un esquema vivo y consultable, relaciones de grafo y búsqueda semántica — inferidos automáticamente.
Sin pipeline. Sin almacenes separados. Sin esquema que diseñar.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 Sitio web](https://rushdb.com) • [📖 Documentación](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 Ejemplos](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • **Español** • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## El problema

Tu agente necesita memoria. La respuesta estándar son tres bases de datos: Redis para clave-valor, un almacén vectorial para búsqueda semántica, una base de datos de grafos para relaciones — más código de pegamento para mantenerlas sincronizadas.

RushDB reemplaza las tres. Envía JSON una vez. Consúltalo con recorrido de grafo, búsqueda semántica, o ambos — en una sola llamada.

| Sin RushDB                                       | Con RushDB                                              |
| ------------------------------------------------ | ------------------------------------------------------- |
| Redis + Pinecone + Neo4j + código de pegamento   | Una sola API                                            |
| Diseñar esquema → escribir migraciones → repetir | Envía cualquier forma, sin esquema requerido            |
| Pipeline de embeddings separado                  | Embeddings gestionados, del lado del servidor           |
| Crear manualmente las aristas de relación        | Detectadas automáticamente desde tu estructura de datos |

---

## Inicio rápido

Dos caminos según tu configuración:

- **Cloud** — Gestionado, capa gratuita, funcionando en 30 segundos. [Obtén una clave de API →](https://app.rushdb.com)
- **Autoalojado** — Docker + tu propia instancia de Neo4j. [Ir a Autoalojamiento →](#self-hosting)

### Camino Cloud

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### Almacenar y recuperar la memoria del agente

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

## Trabajar con tus datos

### Importar JSON anidado

Envía cualquier forma de JSON. Los objetos anidados y los arrays de objetos se convierten en registros enlazados — las etiquetas, los tipos y las relaciones se infieren en el momento de la escritura. Sin esquema, sin paso de migración.

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

Cada clave anidada (`DEPARTMENT`, `EMPLOYEE`) se convierte en una etiqueta, cada objeto en un registro, y la contención en una relación — todo creado automáticamente.

### Importar CSV

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

`suggestTypes` infiere números, booleanos y fechas a partir de cadenas; `skipEmptyValues` trata las celdas en blanco como no establecidas en lugar de almacenar valores vacíos (`0` y `false` se conservan).

### Recorrer el grafo

Filtra registros raíz por condiciones sobre sus registros _relacionados_ — a cualquier profundidad — en una sola consulta. Las etiquetas relacionadas van **dentro** de `where`, no en `labels`:

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

### Consultas analíticas (agregar y agrupar por)

`select` da forma a la salida con agregaciones (`$sum`, `$avg`, `$count`, `$min`, `$max`); `groupBy` controla las dimensiones. No añadas `limit` a una agregación — escanearía solo los primeros N registros y distorsionaría los totales.

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

Las agregaciones se combinan con el recorrido — p. ej., plantilla y nómina por departamento:

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

## Conectar con Claude, Cursor o cualquier cliente MCP

RushDB incluye un servidor MCP. Tu agente obtiene memoria persistente y estructurada — lista para usar.

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

Coloca esto en tu configuración MCP de Claude Desktop, Cursor o Windsurf. El agente ahora puede crear registros, buscar por significado, recorrer relaciones e inspeccionar el esquema — todo mediante lenguaje natural.

---

## Qué incluye

| Capacidad                          | Qué significa                                                                                                                                                                                                              |
| ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Embeddings gestionados**         | Indexa cualquier propiedad de cadena una vez — cada escritura se auto-embebe del lado del servidor                                                                                                                         |
| **Grafo + vector en una consulta** | La similitud semántica y el recorrido de relaciones se combinan en una sola llamada                                                                                                                                        |
| **Cero esquema**                   | Envía cualquier forma de JSON. RushDB infiere tipos, crea propiedades y enlaza registros                                                                                                                                   |
| **Esquema inferido**               | Las propiedades se convierten en nodos de primera clase — los tipos, las etiquetas y las relaciones se descubren en la escritura, de modo que un esquema consultable se construye a sí mismo a medida que llegan los datos |
| **Transacciones ACID**             | Los agentes concurrentes no corrompen la memoria compartida. Neo4j por debajo                                                                                                                                              |
| **Autodescriptivo**                | Los agentes inspeccionan el esquema inferido — etiquetas, propiedades, rangos de valores — para saber qué pueden consultar de forma segura                                                                                 |
| **Nativo de MCP**                  | Servidor MCP completo con un prompt de consulta orientado al descubrimiento integrado                                                                                                                                      |
| **Agent Skills**                   | Paquete instalable `@rushdb/skills` — enseña a cualquier agente compatible con skills a consultar, modelar y recordar con RushDB en un solo comando                                                                        |
| **API de consulta unificada**      | Una sola forma de JSON para grafo, vector, agregación e introspección                                                                                                                                                      |
| **Autoalojamiento o cloud**        | Docker + tu Neo4j, o cloud gestionado. Propiedad total de los datos                                                                                                                                                        |

---

## Casos de uso

| Caso de uso                       | Qué reemplaza RushDB                                | API clave                                                      |
| --------------------------------- | --------------------------------------------------- | -------------------------------------------------------------- |
| **Memoria de agente**             | Redis + almacén vectorial + base de datos de grafos | `db.records.vectorSearch({ query, where: { agent_id } })`      |
| **RAG con contexto**              | Almacén vectorial plano                             | `db.records.find({ where, labels })` + recorrido de relaciones |
| **Apps sin esquema**              | Postgres + migraciones + ETL                        | `db.records.importJson(nestedJson)`                            |
| **Productos de datos conectados** | Múltiples servicios unidos                          | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })`  |

---

## Autoalojamiento

> **Camino de autoalojamiento** — ejecuta RushDB en tu propia infraestructura. Requiere Neo4j 2026.01.4+ con el plugin APOC.

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
<summary>Variables de entorno completas</summary>

| Nombre                            | Descripción                                                   | Requerido | Predeterminado |
| --------------------------------- | ------------------------------------------------------------- | --------- | -------------- |
| `NEO4J_URL`                       | URL de conexión a Neo4j                                       | sí        | —              |
| `NEO4J_USERNAME`                  | Usuario de Neo4j                                              | sí        | neo4j          |
| `NEO4J_PASSWORD`                  | Contraseña de Neo4j                                           | sí        | —              |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | Clave de 32 caracteres para cifrar tokens de API              | sí (prod) | —              |
| `RUSHDB_PORT`                     | Puerto HTTP                                                   | no        | 3000           |
| `RUSHDB_LOGIN`                    | Login de administrador                                        | no        | admin          |
| `RUSHDB_PASSWORD`                 | Contraseña de administrador                                   | no        | password       |
| `RUSHDB_BASE_URL`                 | URL de API pública/base para asignaciones de synx             | no        | —              |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | Token interno para workers de synx gestionados                | no        | —              |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | Clave de escritura interna para escrituras de destino de synx | no        | —              |

Los workers de synx gestionados se ejecutan como demonios. Sondean en busca de conectores ejecutables, renuevan los arrendamientos (leases) de los conectores, liberan los arrendamientos al apagarse de forma controlada y permiten que platform/core recupere los arrendamientos expirados tras una caída.

</details>

<details>
<summary>Desarrollo local (Neo4j incluido)</summary>

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
<summary>Arquitectura: cómo estructura los datos RushDB (LMPG)</summary>

RushDB utiliza un modelo de **Grafo de Propiedades Meta Etiquetado (LMPG, Labeled Meta Property Graph)**. Las propiedades se elevan a nodos de grafo de primera clase ("HyperProperties") — no son solo pares clave-valor adjuntos a los registros.

Esto significa:

- **Esquema sin diseño previo** — debido a que las propiedades son nodos del grafo, el esquema se _infiere de tus datos, no se diseña_: las etiquetas, los tipos, los rangos de valores y la topología de relaciones se descubren en la escritura y son consultables de inmediato — sin modelado manual de esquema, sin cadena de herramientas RDF/OWL
- **Relaciones detectadas automáticamente** — los registros que comparten propiedades se enlazan sin crear aristas a mano
- **Introspección de esquema** — los agentes pueden enumerar etiquetas, tipos de propiedades, rangos de valores y topología de relaciones en una sola consulta
- **Restricciones flexibles** — puntuación de cohesión de tipos, seguimiento de cardinalidad y aplicación de dimensiones vectoriales sin esquemas rígidos previos
- **Superficie de consulta unificada** — la misma expresión de filtro funciona en registros, etiquetas, propiedades y relaciones

Una sola SearchQuery recupera múltiples perspectivas simultáneamente (registros + estadísticas de propiedades + agregaciones), evitando el patrón de inspección N+1 común en arquitecturas de sistemas separados.

[Lee el artículo completo sobre la arquitectura LMPG →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## Documentación

| Tema                           | Enlace                                                         |
| ------------------------------ | -------------------------------------------------------------- |
| Tutorial rápido                | https://docs.rushdb.com/get-started/quick-tutorial             |
| Búsqueda vectorial / semántica | https://docs.rushdb.com/concepts/search/where#vector-operators |
| Filtrado y recorrido           | https://docs.rushdb.com/concepts/search/where                  |
| Agrupación y agregaciones      | https://docs.rushdb.com/concepts/search/group-by               |
| SDK de TypeScript              | https://docs.rushdb.com/typescript-sdk/introduction            |
| SDK de Python                  | https://docs.rushdb.com/python-sdk/introduction                |
| API REST                       | https://docs.rushdb.com/rest-api/introduction                  |
| Servidor MCP                   | packages/mcp-server/README.md                                  |
| Agent Skills                   | packages/skills/README.md                                      |

---

## Cuándo no usar RushDB

- Necesitas latencia de submilisegundos con un rendimiento de escritura muy alto — RushDB está construido sobre Neo4j, que prioriza la consistencia y la expresividad de las consultas sobre la velocidad bruta de escritura.
- Solo necesitas almacenamiento plano clave-valor sin relaciones ni búsqueda semántica — un almacén más simple será más ligero.
- Necesitas un esquema relacional rígido aplicado a nivel de base de datos — RushDB es deliberadamente sin esquema.

---

## Contribuir

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

Consulta [CONTRIBUTING.md](CONTRIBUTING.md) para ver las directrices completas. Issues y PRs son bienvenidos.

---

## Licencia

| Ruta                    | Licencia            |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

¿Necesitas algo que aún no está soportado? Abre un issue — las discusiones de diseño son bienvenidas.
