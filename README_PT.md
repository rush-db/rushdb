<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### A camada de memória para agentes e aplicações de IA.

Envie qualquer JSON. Seu agente ganha um schema vivo e consultável, relacionamentos em grafo e busca semântica — tudo inferido automaticamente.
Sem pipeline. Sem armazenamentos separados. Sem schema para projetar.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 Website](https://rushdb.com) • [📖 Documentation](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 Examples](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • **Português** • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## O problema

Seu agente precisa de memória. A resposta padrão são três bancos de dados: Redis para chave-valor, um vector store para busca semântica, um banco de dados em grafo para relacionamentos — além do código de cola para mantê-los sincronizados.

O RushDB substitui os três. Envie o JSON uma vez. Consulte-o com travessia de grafo, busca semântica, ou ambos — em uma única chamada.

| Sem o RushDB                                   | Com o RushDB                                                    |
| ---------------------------------------------- | --------------------------------------------------------------- |
| Redis + Pinecone + Neo4j + código de cola      | Uma única API                                                   |
| Projetar schema → escrever migrações → repetir | Envie qualquer formato, sem schema necessário                   |
| Pipeline de embeddings separado                | Embeddings gerenciados, no lado do servidor                     |
| Criar arestas de relacionamento à mão          | Detectadas automaticamente a partir da estrutura dos seus dados |

---

## Início rápido

Dois caminhos, dependendo da sua configuração:

- **Cloud** — Gerenciado, plano gratuito, rodando em 30 segundos. [Obtenha a chave de API →](https://app.rushdb.com)
- **Auto-hospedagem** — Docker + sua própria instância do Neo4j. [Ir para Auto-hospedagem →](#self-hosting)

### Caminho Cloud

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### Armazene e recupere a memória do agente

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

## Trabalhando com seus dados

### Importe JSON aninhado

Envie qualquer formato de JSON. Objetos aninhados e arrays de objetos tornam-se registros vinculados — labels, tipos e relacionamentos são inferidos na escrita. Sem schema, sem etapa de migração.

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

Cada chave aninhada (`DEPARTMENT`, `EMPLOYEE`) torna-se um label, cada objeto um registro, e o aninhamento um relacionamento — tudo criado automaticamente.

### Importe CSV

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

`suggestTypes` infere números, booleanos e datas a partir de strings; `skipEmptyValues` trata células em branco como não definidas em vez de armazenar valores vazios (`0` e `false` são mantidos).

### Percorra o grafo

Filtre registros raiz por condições em seus registros _relacionados_ — arbitrariamente profundos — em uma única consulta. Labels relacionados vão **dentro** de `where`, não em `labels`:

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

### Consultas analíticas (agregação e agrupamento)

`select` molda a saída com agregações (`$sum`, `$avg`, `$count`, `$min`, `$max`); `groupBy` controla as dimensões. Não adicione `limit` a uma agregação — isso faria a varredura apenas dos primeiros N registros e distorceria os totais.

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

As agregações se combinam com a travessia — por exemplo, número de funcionários e folha de pagamento por departamento:

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

## Conecte-se ao Claude, Cursor ou qualquer cliente MCP

O RushDB vem com um servidor MCP. Seu agente ganha memória persistente e estruturada — pronta para usar.

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

Coloque isto na configuração MCP do seu Claude Desktop, Cursor ou Windsurf. O agente agora pode criar registros, buscar por significado, percorrer relacionamentos e inspecionar o schema — tudo via linguagem natural.

---

## O que vem na caixa

| Recurso                           | O que significa                                                                                                                                                                             |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Embeddings gerenciados**        | Indexe qualquer propriedade do tipo string uma vez — cada escrita recebe embedding automático no lado do servidor                                                                           |
| **Grafo + vetor em uma consulta** | Similaridade semântica e travessia de relacionamentos se combinam em uma única chamada                                                                                                      |
| **Zero schema**                   | Envie qualquer formato de JSON. O RushDB infere tipos, cria propriedades, vincula registros                                                                                                 |
| **Schema inferido**               | As propriedades tornam-se nós de primeira classe — tipos, labels e relacionamentos são descobertos na escrita, então um schema consultável se constrói sozinho à medida que os dados chegam |
| **Transações ACID**               | Agentes concorrentes não corrompem a memória compartilhada. Neo4j por baixo dos panos                                                                                                       |
| **Autodescritivo**                | Os agentes inspecionam o schema inferido — labels, propriedades, intervalos de valores — para saber o que podem consultar com segurança                                                     |
| **Nativo de MCP**                 | Servidor MCP completo com prompt de consulta orientado à descoberta embutido                                                                                                                |
| **Agent Skills**                  | Pacote `@rushdb/skills` instalável — ensine qualquer agente compatível com skills a consultar, modelar e lembrar com o RushDB em um único comando                                           |
| **API de consulta unificada**     | Um único formato JSON para grafo, vetor, agregação e introspecção                                                                                                                           |
| **Auto-hospedagem ou cloud**      | Docker + seu Neo4j, ou cloud gerenciado. Propriedade total dos dados                                                                                                                        |

---

## Casos de uso

| Caso de uso                      | O que o RushDB substitui        | API principal                                                       |
| -------------------------------- | ------------------------------- | ------------------------------------------------------------------- |
| **Memória de agente**            | Redis + vector store + grafo DB | `db.records.vectorSearch({ query, where: { agent_id } })`           |
| **RAG com contexto**             | Vector store plano              | `db.records.find({ where, labels })` + travessia de relacionamentos |
| **Apps sem schema**              | Postgres + migrações + ETL      | `db.records.importJson(nestedJson)`                                 |
| **Produtos de dados conectados** | Múltiplos serviços com joins    | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })`       |

---

## Auto-hospedagem

> **Caminho de auto-hospedagem** — rode o RushDB na sua própria infraestrutura. Requer Neo4j 2026.01.4+ com o plugin APOC.

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
<summary>Variáveis de ambiente completas</summary>

| Nome                              | Descrição                                                  | Obrigatório | Padrão   |
| --------------------------------- | ---------------------------------------------------------- | ----------- | -------- |
| `NEO4J_URL`                       | URL de conexão do Neo4j                                    | sim         | —        |
| `NEO4J_USERNAME`                  | Usuário do Neo4j                                           | sim         | neo4j    |
| `NEO4J_PASSWORD`                  | Senha do Neo4j                                             | sim         | —        |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | Chave de 32 caracteres para criptografia do token de API   | sim (prod)  | —        |
| `RUSHDB_PORT`                     | Porta HTTP                                                 | não         | 3000     |
| `RUSHDB_LOGIN`                    | Login de administrador                                     | não         | admin    |
| `RUSHDB_PASSWORD`                 | Senha de administrador                                     | não         | password |
| `RUSHDB_BASE_URL`                 | URL pública/base da API para atribuições do synx           | não         | —        |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | Token interno para workers synx gerenciados                | não         | —        |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | Chave de escrita interna para gravações de destino do synx | não         | —        |

Os workers synx gerenciados rodam como daemons. Eles fazem polling em busca de conectores executáveis, renovam os leases dos conectores, liberam os leases em desligamentos graciosos e permitem que o platform/core reivindique leases expirados após falhas.

</details>

<details>
<summary>Desenvolvimento local (Neo4j incluído)</summary>

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
<summary>Arquitetura: como o RushDB estrutura os dados (LMPG)</summary>

O RushDB usa um modelo de **Labeled Meta Property Graph (LMPG)**. As propriedades são elevadas a nós de grafo de primeira classe ("HyperProperties") — não apenas pares chave-valor anexados aos registros.

Isso significa:

- **Schema sem projeto antecipado** — porque as propriedades são nós de grafo, o schema é _inferido a partir dos seus dados, não projetado_: labels, tipos, intervalos de valores e topologia de relacionamentos são descobertos na escrita e imediatamente consultáveis — sem modelagem manual de schema, sem cadeia de ferramentas RDF/OWL
- **Relacionamentos detectados automaticamente** — registros que compartilham propriedades são vinculados sem criar arestas à mão
- **Introspecção de schema** — os agentes podem enumerar labels, tipos de propriedades, intervalos de valores e topologia de relacionamentos em uma única consulta
- **Restrições flexíveis** — pontuação de coesão de tipos, rastreamento de cardinalidade e imposição de dimensão de vetor sem schemas rígidos definidos antecipadamente
- **Superfície de consulta unificada** — a mesma expressão de filtro funciona em registros, labels, propriedades e relacionamentos

Uma única SearchQuery recupera múltiplas perspectivas simultaneamente (registros + estatísticas de propriedades + agregações), evitando o padrão de inspeção N+1 comum em arquiteturas de sistemas separados.

[Leia o artigo completo sobre a arquitetura LMPG →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## Documentação

| Tópico                     | Link                                                           |
| -------------------------- | -------------------------------------------------------------- |
| Tutorial rápido            | https://docs.rushdb.com/get-started/quick-tutorial             |
| Busca vetorial / semântica | https://docs.rushdb.com/concepts/search/where#vector-operators |
| Filtragem e travessia      | https://docs.rushdb.com/concepts/search/where                  |
| Agrupamento e agregações   | https://docs.rushdb.com/concepts/search/group-by               |
| SDK TypeScript             | https://docs.rushdb.com/typescript-sdk/introduction            |
| SDK Python                 | https://docs.rushdb.com/python-sdk/introduction                |
| API REST                   | https://docs.rushdb.com/rest-api/introduction                  |
| Servidor MCP               | packages/mcp-server/README.md                                  |
| Agent Skills               | packages/skills/README.md                                      |

---

## Quando não usar o RushDB

- Você precisa de latência inferior a um milissegundo com throughput de escrita muito alto — o RushDB é construído sobre o Neo4j, que prioriza consistência e expressividade de consultas em vez de velocidade bruta de escrita.
- Você só precisa de armazenamento chave-valor plano, sem relacionamentos ou busca semântica — um armazenamento mais simples será mais leve.
- Você precisa de um schema relacional rígido imposto no nível do banco de dados — o RushDB é deliberadamente sem schema.

---

## Contribuindo

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

Veja [CONTRIBUTING.md](CONTRIBUTING.md) para as diretrizes completas. Issues e PRs são bem-vindos.

---

## Licença

| Caminho                 | Licença             |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

Precisa de algo que ainda não é suportado? Abra uma issue — discussões de design são bem-vindas.
