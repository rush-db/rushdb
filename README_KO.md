<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### 레코드와 그 관계, 그리고 그 위의 검색까지 하나의 데이터베이스로.

JSON을 쓰기만 하면 됩니다. RushDB가 타입을 추론하고, 중첩된 객체를 그래프로 연결하며, 자체 JSON 쿼리 언어, SQL, Cypher, Gremlin, Mongo 스타일 문서로 조회할 수 있게 해 줍니다. 벡터, 전문(full-text), 하이브리드 검색도 지원합니다. Rust 바이너리 하나, 데이터 디렉터리 하나, Apache-2.0.

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[웹사이트](https://rushdb.com) • [문서](https://docs.rushdb.com) • [매뉴얼](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [배포](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • **한국어** • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## 빠른 시작

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

설치 스크립트는 릴리스 체크섬을 확인하고(`cosign`이 있으면 Sigstore 서명도 확인) `rushdb` 바이너리를 `~/.rushdb/bin`에 설치합니다. `rushdb up`은 `http://127.0.0.1:3000`에서 영속 노드를 시작하며, 데이터는 `./.rushdb`에 저장되고 로컬 토큰은 `local-token`입니다.

중첩된 JSON 문서를 씁니다. 중첩된 객체는 각각 별도의 레코드가 되어 부모와 연결됩니다:

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

이제 Acme의 엔지니어를 찾습니다. 관련 레이블을 적기만 하면 필터가 관계를 따라갑니다:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

이어서 [빠른 시작 가이드](docs/manual/getting-started/quickstart.md)를 보세요.

## 원하는 방식으로 조회

같은 레코드를 모든 인터페이스에서 조회할 수 있습니다.

**SQL**: PostgreSQL 와이어 프로토콜을 통해 `psql`이나 어떤 PostgreSQL 드라이버로든. 레이블이 곧 테이블입니다:

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher**: HTTP로, 또는 Neo4j 드라이버로 Bolt를 통해(`rushdb up --languages sql,cypher --bolt-port 7687`):

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**SDK**의 다이렉트 모드(`npm install @rushdb/javascript-sdk`):

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, Mongo 스타일 문서, ISO GQL도 같은 HTTP 엔드포인트를 사용합니다. [쿼리 언어](docs/manual/languages/README.md)를 참고하세요.

## 주요 기능

| 기능 | 의미 |
|---|---|
| **스키마 설계 불필요** | 어떤 형태의 JSON이든 씁니다. 타입은 값마다 추론되고, 중첩 객체는 연결된 레코드가 되며, 엔진은 레이블·속성·관계의 정확한 카탈로그를 유지하고 이를 조회할 수 있습니다. |
| **속성이 일급 객체** | Labeled Meta-Property Graph(LMPG)는 각 속성을, 그것을 사용하는 레코드들이 공유하는 타입이 있는 정의로 저장합니다. 따라서 스키마 탐색, 값 범위, 패싯이 저장 구조 자체에서 나옵니다. |
| **다양한 쿼리 언어** | 네이티브 JSON 쿼리 언어, SQL, Gremlin, SQL/PGQ, Mongo 스타일 문서, 그리고 릴리스 후보인 openCypher와 ISO GQL. |
| **검색** | 사용자가 제공한 벡터에 대한 벡터 인덱스(정확 검색 및 HNSW), BM25 전문 인덱스, 둘을 결합한 하이브리드 검색. |
| **내구성과 트랜잭션** | 선행 기록 로그(WAL), MVCC 스냅샷, 다중 문장 트랜잭션, 특정 시점 복구가 가능한 백업, 읽기 전용 팔로워로의 스트리밍 복제. |
| **표준 프로토콜** | 하나의 프로세스에서 HTTP, PostgreSQL 와이어 프로토콜, gRPC, Bolt를 제공합니다. |
| **멀티테넌시** | 선택 가능한 fail-closed 테넌트, 테넌트마다 독립된 키 공간. |
| **바이너리 하나** | `rushdb serve`는 서버를, `rushdb up`은 로컬 노드를 실행합니다. 따로 운영할 서비스가 없습니다. |

## 배포

| 환경 | 방법 |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/), Caddy를 앞에 둔 HTTPS 버전 포함 |
| Kubernetes | [Helm 차트](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| Linux VM | DigitalOcean, Hetzner, AWS, GCP, Azure용 [cloud-init](deploy/templates/cloud-init/), [systemd 유닛](deploy/templates/systemd/) |
| 플랫폼 | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### 다른 설치 방법

| 방법 | 명령 | 비고 |
|---|---|---|
| 설치 스크립트 | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux(glibc 2.28 이상)와 macOS, x86-64 및 arm64. `sh -s -- --version v3.0.0`로 버전을 고정합니다. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS와 Linux. `brew services start rushdb`로 백그라운드에서 실행합니다. |
| Docker | `rushdb/rushdb` | `linux/amd64`와 `linux/arm64`; `ghcr.io/rush-db/rushdb`에서도 제공합니다. |
| Cargo | `cargo binstall rushdb-server` 또는 `cargo install rushdb-server --locked` | `binstall`은 릴리스 바이너리를 받고, `install`은 소스에서 컴파일합니다(C++ 컴파일러 필요). |
| 릴리스 아카이브 | [GitHub releases](https://github.com/rush-db/rushdb/releases) | SHA-256 체크섬, SBOM, Sigstore 서명이 포함된 아카이브. |
| 소스 | `cargo build --release -p rushdb-server --bin rushdb` | [설치](docs/manual/getting-started/installation.md)를 참고하세요. |

Windows는 네이티브로 지원하지 않습니다. WSL 2나 Docker를 사용하세요. 노드를 루프백 밖으로 노출하기 전에 [보안](docs/manual/operations/security.md)을 읽어 보세요.

## 상태

| 영역 | 상태 |
|---|---|
| 네이티브 프로토콜, SQL, Gremlin 텍스트 순회, Mongo 스타일 문서 | 안정 |
| SQL/PGQ `GRAPH_TABLE` | 안정, 읽기 전용 |
| openCypher, ISO GQL, Bolt | 릴리스 후보, 명시적으로 활성화: 외부 적합성 테스트 스위트를 모두 통과하지는 못합니다([자세히](docs/manual/languages/README.md)) |
| PostgreSQL 와이어 프로토콜 | SQL 읽기와 단일 문장 쓰기. `psql`, node-postgres, psycopg, `tokio-postgres`로 확인했으며, 모든 클라이언트나 ORM은 아닙니다 |
| 복제 | 읽기 전용 팔로워로의 비동기 스트리밍. 승격은 수동이며 자동 장애 조치는 없습니다 |
| Raft, 스토리지 샤딩 | 미리 보기, 프로덕션 사용 불가: Raft는 단일 프로세스에서 메모리 상태로 동작하며, 여러 샤드에 걸친 커밋은 장애 시 원자적이지 않습니다 |

각 인터페이스의 정확한 제한은 [매뉴얼](docs/manual/README.md)에 정리되어 있습니다.

## 연결

| 인터페이스 | 기본값 | 활성화 |
|---|---|---|
| HTTP 네이티브 프로토콜 | `127.0.0.1:3000`, `POST /protocol/v1/execute` | 항상 켜짐 |
| PostgreSQL 와이어 프로토콜 | `127.0.0.1:5432` | `sql` 사용 시(기본값) |
| gRPC(`rushdb.v1.Database/Execute`) | 꺼짐 | `--grpc-port` |
| Bolt 5.0 | 꺼짐 | `--bolt-port`와 `cypher` |

[JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk)와 [Python](https://pypi.org/project/rushdb/) SDK, [MCP 서버](https://www.npmjs.com/package/@rushdb/mcp-server), [에이전트 스킬](https://github.com/rush-db/skills)은 다이렉트 모드로 셀프 호스팅 엔진과 함께 동작합니다.

## 셀프 호스팅 엔진과 RushDB Cloud

이 엔진은 RushDB에서 셀프 호스팅할 수 있는 부분입니다. 임베딩 생성, 대시보드 제공, 사용자 관리는 하지 않습니다. 이것들은 [RushDB Cloud](https://app.rushdb.com)가 제공합니다. RushDB Cloud는 이 엔진을 운영하는 관리형 서비스로, 대시보드, 관리형 임베딩, AI 기능(자연어 검색, 관계 패턴 제안)을 더합니다. SDK와 MCP 서버는 둘 다와 함께 쓸 수 있습니다. RushDB Cloud API 키를 주거나 엔진을 직접 가리키면(`direct: true`, `engineUrl`) Cloud 전용 기능을 제외한 모든 기능을 쓸 수 있고, Cloud 전용 기능은 `CloudOnlyError`로 거부됩니다.

## 문서

- [매뉴얼](docs/manual/README.md): 시작하기, 데이터 모델, 쿼리, 언어, 검색, 운영, 참조 표.
- [docs.rushdb.com](https://docs.rushdb.com): 튜토리얼, SDK, MCP 서버.
- [RushDB 2.x에서 업그레이드](docs/manual/operations/upgrading-from-2x.md): 2.x는 Neo4j 기반 구성으로, 이 저장소의 `v2` 태그에 남아 있습니다.

## 기여하기

[CONTRIBUTING.md](CONTRIBUTING.md)를 참고하세요. 품질 게이트:

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [보안 정책](SECURITY.md)
- [릴리스 절차](RELEASING.md)
- [행동 강령](CODE_OF_CONDUCT.md)
- [변경 이력](CHANGELOG.md)

[Apache-2.0](LICENSE) 라이선스로 제공됩니다.
