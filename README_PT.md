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

```bash
curl -fsSL https://rushdb.com/install.sh | sh    # or: brew install rush-db/tap/rushdb
export PATH="$HOME/.rushdb/bin:$PATH"             # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` atende em `http://127.0.0.1:3000`, com os dados em `./.rushdb` e o
token `local-token`. Em outro terminal, `pip install rushdb`:

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

O schema vem do catálogo, não de uma varredura dos registros:

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
```

<details>
<summary>O mesmo em TypeScript</summary>

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

Já fala SQL? Rótulos são tabelas:

```bash
PGPASSWORD=local-token psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Próximos passos: o [início rápido completo](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker e outras formas de instalação](https://docs.rushdb.com/engine/getting-started/installation/)

## O que você pode construir

| | |
|---|---|
| **Memória para agentes** | Memórias como registros com relações, filtradas por escopo e ordenadas pelos vetores que você armazena. Provedores prontos para OpenClaw e Hermes. |
| **Busca nos dados do seu app** | Busca de texto completo, vetorial e híbrida sobre os registros que seu app já escreve, com filtros de grafo na mesma consulta. |
| **Perguntas sobre dados conectados** | Um agente lê o schema e então escreve uma consulta que você pode inspecionar: na linguagem de consulta JSON, em SQL ou em Cypher. |
| **Relatórios sem exportação** | Agrupe e agregue os registros que seu app escreve, pelos SDKs ou por qualquer cliente PostgreSQL. |

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
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes (Helm), cloud-init, systemd, Fly.io, Render,
Coolify e Dokploy: veja [Deploy](https://docs.rushdb.com/deploy/). Leia
[Segurança](https://docs.rushdb.com/engine/operations/security/) antes de
expor um nó além do loopback.

## Auto-hospedado e RushDB Cloud

- **Auto-hospedado:** este repositório. Tudo o que está acima roda no engine que você instala.
- **[RushDB Cloud](https://app.rushdb.com):** o mesmo engine, gerenciado, com um
  dashboard, embeddings gerenciados, busca em linguagem natural e conectores para
  ferramentas SaaS. Os SDKs e o servidor MCP funcionam com os dois.

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
