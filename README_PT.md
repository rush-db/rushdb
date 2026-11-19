<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="Logo do RushDB" width="96">

# RushDB

<sub>O banco de dados para IA</sub>

**Grafo, vetores e análise sobre os mesmos registros. Cada escrita mantém um schema vivo de rótulos, campos e valores, para que agentes consultem campos reais em vez de adivinhar.**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[Documentação](https://docs.rushdb.com) • [Início rápido](https://docs.rushdb.com/engine/getting-started/quickstart/) • [Discussões](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • **Português** • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

Traga os dados do jeito que você os tem: JSON aninhado, CSV, `INSERT` de SQL,
escritas em Cypher ou no estilo Mongo, ou change streams dos seus bancos de dados. O
RushDB os guarda como registros tipados, ligados em um grafo, e os mesmos registros
respondem a consultas de relações, busca vetorial e de texto completo e agregações,
então não há um segundo armazenamento para manter sincronizado. Cada escrita também
atualiza um catálogo dos rótulos, campos, tipos e valores existentes, então um agente
consulta campos reais. Conecte-se por HTTP, pelo protocolo de rede do PostgreSQL, Bolt,
gRPC ou MCP, pelos SDKs ou por qualquer cliente PostgreSQL. Um binário Rust, Apache-2.0.

| Grafo | Busca vetorial e de texto completo | Análise |
|---|---|---|
| JSON aninhado vira registros ligados. Filtre através de relações e siga relações tipadas e direcionadas em uma única consulta. | Busca vetorial (exata e HNSW), de texto completo BM25 e híbrida, filtrada pelos mesmos predicados do grafo. | Contagem, soma, média, mínimo e máximo com `groupBy`, ou SQL puro com `GROUP BY` pelo protocolo de rede do PostgreSQL. |

## Início rápido

![Gravação do terminal: instalar o RushDB, iniciá-lo com rushdb up e, a partir do Python, importar JSON aninhado, filtrar funcionários por departamento, contar pessoas e calcular o salário médio por departamento e imprimir o schema vivo](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` atende em `http://127.0.0.1:3000`, com os dados em `./.rushdb` e o
token `local-token`. Em outro terminal, `pip install rushdb`:

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
<summary>O mesmo em TypeScript</summary>

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

Já fala SQL? Rótulos são tabelas:

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Próximos passos: o [início rápido completo](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker e outras formas de instalação](https://docs.rushdb.com/engine/getting-started/installation/)

## O que os agentes veem antes de consultar

A última linha do início rápido imprime o esquema que um agente lê primeiro. Ele vem do catálogo que cada escrita atualiza, não de uma varredura dos registros:

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

Em um banco grande e variado, um agente não carrega tudo isso. Ele afunila da visão geral até os valores exatos, uma leitura pequena por vez (a mesma execução de cima):

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

`get_schema_summary` retorna o mesmo catálogo em JSON, com contagens de valores distintos e os valores dos campos pequenos. O servidor MCP oferece cada leitura como ferramenta: `findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues` e `getSchemaSummary`.

## O que você pode construir

- **Memória de agentes.** Memórias são registros delimitados por usuário, agente ou sessão e ligados ao turno que as produziu, então a recuperação mostra de onde veio cada memória. Provedores prontos para OpenClaw e Hermes.
- **Cérebro da empresa e RAG.** Docs, CRM, suporte e cobrança chegam no próprio formato e são ligados por chaves que já compartilham, como um e-mail ou um ID de conta. A recuperação filtra primeiro pela conta ou versão certa e depois ranqueia por vetor e BM25 juntos, e cada fato cita o registro por trás dele.
- **Busca facetada.** Barras de filtro cujas contagens vêm do catálogo, restritas ao filtro atual e exatas, com busca híbrida ranqueada sob o mesmo filtro. Sem um índice de busca separado para manter em sincronia.
- **Busca com IA e assistentes no seu app.** Índices de texto e vetoriais ficam no banco e se mantêm atualizados conforme seu app escreve; os SDKs, o servidor MCP e clientes SQL leem os mesmos registros.
- **Análise de dados com agentes.** O agente lê quais labels e campos existem e como cada valor está escrito, e então escreve SQL, Cypher ou uma agregação sobre eles. Toda resposta vem com uma consulta que qualquer pessoa pode rodar de novo.
- **Apps cujo esquema evolui no caminho.** Envie qualquer JSON: os tipos são inferidos, objetos aninhados viram registros ligados e campos novos aparecem no catálogo com a escrita que os adiciona. Sem migrações.

## Recursos

- **Schema emergente.** Os tipos são inferidos por valor, objetos aninhados viram registros ligados, e o catálogo mantém contagens exatas e listas de valores. [Catálogo de schema](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Consultas de grafo.** Filtre através de relações por rótulo, restrinja-as por tipo e direção e vincule registros relacionados para projeções e agregações. [Relações e travessia](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Busca integrada.** Índices vetoriais sobre os vetores que você fornece, texto completo BM25 e busca híbrida que combina os dois. [Índices e busca](https://docs.rushdb.com/engine/indexes-and-search/)
- **Agregações.** `select` e `groupBy` com contagens, somas, médias, intervalos de tempo e coleções aninhadas. [Agregações](https://docs.rushdb.com/engine/query/aggregations/)
- **Fala o que você usa.** SQL pelo protocolo de rede do PostgreSQL, Gremlin, SQL/PGQ, documentos no estilo Mongo, e openCypher e ISO GQL como release candidates. [Linguagens de consulta](https://docs.rushdb.com/engine/languages/)
- **Durável.** Write-ahead log, transações, backups com restauração para um ponto no tempo e réplicas de leitura. [Operações](https://docs.rushdb.com/engine/operations/)
- **Um binário.** HTTP, protocolo de rede do PostgreSQL, gRPC e Bolt em um único processo, com tenants isolados opcionais. [Conectividade](https://docs.rushdb.com/engine/connectivity/)

## Ecossistema

| Projeto | Para |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | Apps em TypeScript e Python |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor e outros clientes MCP |
| [Skills para agentes](https://github.com/rush-db/skills) · [plugins para agentes](https://github.com/rush-db/agent-plugins) | Agentes de programação |
| [LangChain e LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | Recuperação, memória e property graph stores |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [contrato de memória](https://github.com/rush-db/agent-memory-contract) | Provedores de memória para agentes |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [plugin para Obsidian](https://github.com/rush-db/obsidian-plugin) | Change data capture, notas |

Todos funcionam com um engine auto-hospedado. Para que seu agente de programação
configure o RushDB, passe a ele [rushdb.com/agent-setup](https://rushdb.com/agent-setup).

## Rode em qualquer lugar

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data \
  -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes (Helm), cloud-init, systemd, Fly.io, Render,
Coolify e Dokploy: veja [Deploy](https://docs.rushdb.com/deploy/). Leia
[Segurança](https://docs.rushdb.com/engine/operations/security/) antes de
expor um nó além do loopback.

## Auto-hospedado e RushDB Cloud

Este repositório é o motor, e você pode hospedá-lo por completo. O [RushDB Cloud](https://app.rushdb.com) roda o mesmo motor para você e acrescenta os recursos marcados abaixo. Os SDKs e o servidor MCP funcionam com os dois; em um motor auto-hospedado, uma chamada exclusiva do Cloud falha com `CloudOnlyError` antes de enviar qualquer coisa.

![Painel do RushDB com dados de exemplo sintéticos: a tabela de registros, depois a visão de grafo das empresas e seus grupos de departamentos, com zoom em um departamento cujos funcionários aparecem no painel do registro](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| Recurso | Motor auto-hospedado | RushDB Cloud |
|---|---|---|
| Motor e API: registros, consultas de grafo, busca, agregações, esquema ao vivo | ✓ | ✓ |
| SDKs e servidor MCP | ✓ modo direto, MCP via stdio | ✓ mais MCP hospedado com OAuth |
| Importação de JSON e CSV | ✓ | ✓ |
| [Painel](https://docs.rushdb.com/rushdb-cloud/) com registros, visão de grafo e construtor de consultas | Use os SDKs ou qualquer cliente PostgreSQL | ✓ |
| [Embeddings gerenciados](https://docs.rushdb.com/rushdb-cloud/capabilities/) | Traga vetores do seu próprio modelo | ✓ cota mensal |
| [Busca em linguagem natural](https://docs.rushdb.com/learn/search-query/smart-search/) | Monte a consulta você mesmo ou com um agente | ✓ cota mensal |
| [Sugestões de relacionamentos](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | Crie os relacionamentos você mesmo | ✓ |
| [Definições](https://docs.rushdb.com/learn/search-query/definitions/) para métricas e filtros | Mantenha as regras nos seus prompts e no seu SQL | ✓ |
| [Conexões](https://docs.rushdb.com/learn/data-sources/connections/) com ferramentas SaaS, e RushDB Sync para bancos on-premises | Seu próprio código de sincronização, ou o sink do Debezium | ✓ |
| Projetos e chaves de API | Tenants e tokens gerenciados por você | ✓ |
| Acesso de equipe | — | Planos pagos |
| Backups | Você cuida deles: backup, restauração pontual, réplicas | Planos pagos |

Um ✓ em RushDB Cloud significa que o plano Free inclui. Planos e cotas em [rushdb.com/pricing](https://rushdb.com/pricing).

## FAQ

**Está pronto para produção?** SQL, a linguagem de consulta JSON nativa, Gremlin e
documentos no estilo Mongo são estáveis. openCypher, ISO GQL e Bolt são release
candidates. A replicação é assíncrona, com promoção manual. A
[página de status](https://docs.rushdb.com/engine/reference/status/) lista cada
interface e seus limites.

**Preciso de um banco de dados vetorial separado?** Não. Vetores, índices de texto e
relações ficam em um único engine, e um registro pode ser escrito junto com
seu vetor em um único commit. O engine busca nos vetores que você fornece; o RushDB
Cloud pode gerá-los para você.

**O que é o schema vivo?** Um catálogo de cada rótulo, propriedade, tipo, valor
e formato de relação, atualizado a cada commit, com contagens exatas. Uma chamada
o retorna, então agentes e ferramentas não adivinham nomes de campos.
[Catálogo de schema](https://docs.rushdb.com/engine/data-model/schema-catalog/)

**Windows?** Use WSL 2 ou Docker.

---

[Contribuir](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Segurança](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

O RushDB 2.x (legado, baseado em Neo4j) fica no
[branch `2.x`](https://github.com/rush-db/rushdb/tree/2.x), com correções de segurança
até 2027-10-28. [Atualizar a partir do 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
