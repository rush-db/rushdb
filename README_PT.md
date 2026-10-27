<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### Um só banco de dados para seus registros, suas relações e a busca sobre eles.

Escreva JSON. O RushDB infere os tipos, liga objetos aninhados em um grafo e permite consultá-lo com sua linguagem de consulta JSON, SQL, Cypher, Gremlin ou documentos no estilo Mongo, além de busca vetorial, de texto completo e híbrida. Um binário Rust, um diretório de dados, Apache-2.0.

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[Site](https://rushdb.com) • [Documentação](https://docs.rushdb.com) • [Manual](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [Implantação](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • **Português** • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## Início rápido

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

O script confere o checksum da versão (e a assinatura Sigstore quando o `cosign` está instalado) e instala o binário `rushdb` em `~/.rushdb/bin`. `rushdb up` inicia um nó persistente em `http://127.0.0.1:3000`, com os dados em `./.rushdb` e o token local `local-token`.

Escreva um documento JSON aninhado. Cada objeto aninhado vira um registro próprio, ligado ao seu pai:

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

Depois encontre os engenheiros da Acme. O filtro atravessa relações ao nomear os rótulos relacionados:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

Continue com o [início rápido do manual](docs/manual/getting-started/quickstart.md).

## Consulte do seu jeito

Os mesmos registros respondem por todas as interfaces.

**SQL** pelo protocolo de rede do PostgreSQL, com `psql` ou qualquer driver PostgreSQL. Rótulos são tabelas:

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher** por HTTP, ou por Bolt com os drivers do Neo4j (`rushdb up --languages sql,cypher --bolt-port 7687`):

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**Os SDKs** em modo direto (`npm install @rushdb/javascript-sdk`):

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, documentos no estilo Mongo e ISO GQL usam o mesmo endpoint HTTP. Veja [Linguagens de consulta](docs/manual/languages/README.md).

## O que vem incluído

| Recurso | O que significa |
|---|---|
| **Nenhum esquema para projetar** | Escreva JSON de qualquer formato. Os tipos são inferidos por valor, objetos aninhados viram registros ligados, e o motor mantém um catálogo exato e consultável de rótulos, propriedades e relações. |
| **Propriedades de primeira classe** | O Labeled Meta-Property Graph (LMPG) guarda cada propriedade como uma definição tipada compartilhada pelos registros que a usam, então a descoberta do esquema, as faixas de valores e as facetas vêm do próprio layout de armazenamento. |
| **Várias linguagens de consulta** | Uma linguagem de consulta JSON nativa, SQL, Gremlin, SQL/PGQ, documentos no estilo Mongo, e openCypher e ISO GQL como release candidates. |
| **Busca** | Índices vetoriais (exatos e HNSW) sobre os vetores que você fornece, índices de texto completo BM25 e busca híbrida que combina os dois. |
| **Durável e transacional** | Log de escrita antecipada, snapshots MVCC, transações com várias instruções, backups com restauração para um ponto no tempo e replicação em streaming para seguidores somente leitura. |
| **Protocolos padrão** | HTTP, o protocolo de rede do PostgreSQL, gRPC e Bolt a partir de um processo. |
| **Multilocação** | Locatários opcionais que falham fechados, cada um com seu próprio espaço de chaves. |
| **Um binário** | `rushdb serve` executa o servidor; `rushdb up` executa um nó local. Nenhum outro serviço para operar. |

## Implantação

| Onde | Como |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/), com uma variante HTTPS atrás do Caddy |
| Kubernetes | [Chart Helm](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| VM Linux | [cloud-init](deploy/templates/cloud-init/) para DigitalOcean, Hetzner, AWS, GCP, Azure; [unidade systemd](deploy/templates/systemd/) |
| Plataformas | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### Outras formas de instalar

| Método | Comando | Observações |
|---|---|---|
| Script de instalação | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) e macOS, x86-64 e arm64. `sh -s -- --version v3.0.0` fixa uma versão. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS e Linux. `brew services start rushdb` executa em segundo plano. |
| Docker | `rushdb/rushdb` | `linux/amd64` e `linux/arm64`; também `ghcr.io/rush-db/rushdb`. |
| Cargo | `cargo binstall rushdb-server` ou `cargo install rushdb-server --locked` | `binstall` baixa o binário publicado; `install` compila (precisa de um compilador C++). |
| Arquivo da versão | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Arquivos com checksums SHA-256, SBOM e assinaturas Sigstore. |
| Código-fonte | `cargo build --release -p rushdb-server --bin rushdb` | Veja [Instalação](docs/manual/getting-started/installation.md). |

O Windows não é suportado nativamente; use WSL 2 ou Docker. Leia [Segurança](docs/manual/operations/security.md) antes de expor um nó além do loopback.

## Situação

| Área | Situação |
|---|---|
| Protocolo nativo, SQL, travessias de texto Gremlin, documentos no estilo Mongo | Estável |
| SQL/PGQ `GRAPH_TABLE` | Estável, somente leitura |
| openCypher, ISO GQL, Bolt | Release candidate, opcional: as suítes externas de conformidade não passam por completo ([detalhes](docs/manual/languages/README.md)) |
| Protocolo PostgreSQL | Leituras SQL e escritas de instrução única; verificado com `psql`, node-postgres, psycopg e `tokio-postgres`, não com todo cliente ou ORM |
| Replicação | Streaming assíncrono para seguidores somente leitura; a promoção é manual, sem failover automático |
| Raft, sharding de armazenamento | Prévia, não para produção: o Raft roda em um processo com estado em memória; commits entre shards não são atômicos em caso de falha |

O [manual](docs/manual/README.md) documenta os limites exatos de cada interface.

## Conectar

| Interface | Padrão | Ativação |
|---|---|---|
| Protocolo HTTP nativo | `127.0.0.1:3000`, `POST /protocol/v1/execute` | sempre ativo |
| Protocolo PostgreSQL | `127.0.0.1:5432` | ativo com `sql` (padrão) |
| gRPC (`rushdb.v1.Database/Execute`) | desativado | `--grpc-port` |
| Bolt 5.0 | desativado | `--bolt-port` com `cypher` |

Os SDKs de [JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) e [Python](https://pypi.org/project/rushdb/), o [servidor MCP](https://www.npmjs.com/package/@rushdb/mcp-server) e as [skills para agentes](https://github.com/rush-db/skills) funcionam com um motor auto-hospedado em modo direto.

## Motor auto-hospedado e RushDB Cloud

Este motor é a parte do RushDB que você auto-hospeda. Ele não gera embeddings, não hospeda um painel e não gerencia usuários: isso vem com o [RushDB Cloud](https://app.rushdb.com), o serviço gerenciado que executa este motor e adiciona o painel, embeddings gerenciados e os recursos de IA (busca em linguagem natural, sugestões de padrões de relação). Os SDKs e o servidor MCP funcionam com qualquer um: passe uma chave de API do RushDB Cloud ou aponte-os para o seu motor (`direct: true`, `engineUrl`) para tudo exceto os recursos do Cloud, que eles recusam com `CloudOnlyError`.

## Documentação

- [Manual](docs/manual/README.md): primeiros passos, modelo de dados, consultas, linguagens, busca, operação e tabelas de referência.
- [docs.rushdb.com](https://docs.rushdb.com): tutoriais, os SDKs e o servidor MCP.
- [Atualizando do RushDB 2.x](docs/manual/operations/upgrading-from-2x.md): a 2.x era a pilha baseada em Neo4j, mantida na tag `v2` deste repositório.

## Contribuir

Veja [CONTRIBUTING.md](CONTRIBUTING.md). O controle de qualidade:

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [Política de segurança](SECURITY.md)
- [Processo de release](RELEASING.md)
- [Código de conduta](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

Licenciado sob [Apache-2.0](LICENSE).
