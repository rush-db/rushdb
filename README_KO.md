<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="RushDB logo" width="96">

# RushDB

<sub>AI를 위한 데이터베이스</sub>

**그래프, 벡터, 분석을 같은 레코드 위에서. 쓰기마다 레이블, 필드, 값의 라이브 스키마가 갱신되므로, 에이전트는 추측하지 않고 실제 필드로 쿼리합니다.**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[문서](https://docs.rushdb.com) • [빠른 시작](https://docs.rushdb.com/engine/getting-started/quickstart/) • [토론](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • **한국어** • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

데이터를 가진 형태 그대로 가져오세요. 중첩된 JSON, CSV, SQL `INSERT`, Cypher나
Mongo 스타일 쓰기, 또는 사용 중인 데이터베이스의 변경 스트림. RushDB는 이를 타입이
있는 레코드로 저장하고 그래프로 연결합니다. 같은 레코드가 관계 쿼리, 벡터 검색과
전문(full-text) 검색, 집계에 모두 답하므로, 동기화를 유지해야 하는 두 번째 저장소가
필요 없습니다. 쓰기마다 존재하는 레이블, 필드, 타입, 값의 카탈로그도 갱신되므로,
에이전트는 실제 필드를 쿼리합니다. HTTP, PostgreSQL 와이어 프로토콜, Bolt, gRPC, MCP로
연결하고, SDK나 아무 PostgreSQL 클라이언트에서 사용할 수 있습니다. Rust 바이너리 하나, Apache-2.0.

| 그래프 | 벡터 및 전문 검색 | 분석 |
|---|---|---|
| 중첩된 JSON은 서로 연결된 레코드가 됩니다. 하나의 쿼리에서 관계를 가로질러 필터링하고, 타입과 방향이 있는 관계를 따라갑니다. | 벡터 검색(정확 검색 및 HNSW), BM25 전문 검색, 하이브리드 검색. 모두 같은 그래프 조건으로 필터링됩니다. | `groupBy`로 개수, 합계, 평균, 최솟값, 최댓값을 구하거나, PostgreSQL 와이어 프로토콜로 `GROUP BY`를 쓰는 일반 SQL을 사용합니다. |

## 빠른 시작

![터미널 녹화: RushDB를 설치하고 rushdb up으로 시작한 뒤, Python에서 중첩 JSON을 가져오고, 부서로 직원을 필터링하고, 부서별 인원과 평균 급여를 집계하고, 라이브 스키마를 출력하는 과정](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up`은 `http://127.0.0.1:3000`에서 서비스하며, 데이터는 `./.rushdb`에 저장되고
토큰은 `local-token`입니다. 다른 터미널에서 `pip install rushdb`를 실행합니다:

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
<summary>같은 예제를 TypeScript로</summary>

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

SQL이 익숙하신가요? 레이블이 곧 테이블입니다:

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

다음: [전체 빠른 시작](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker 및 기타 설치 방법](https://docs.rushdb.com/engine/getting-started/installation/)

## 에이전트가 쿼리하기 전에 보는 것

빠른 시작의 마지막 줄은 에이전트가 가장 먼저 읽는 스키마를 출력합니다. 이 스키마는 레코드를 스캔한 결과가 아니라, 쓰기마다 갱신되는 카탈로그에서 나옵니다:

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

크고 다양한 저장소에서 에이전트는 이것을 전부 불러오지 않습니다. 개요에서 정확한 값까지, 작은 읽기를 한 번씩 하며 범위를 좁힙니다(위와 같은 실행):

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

`get_schema_summary`는 같은 카탈로그를 JSON으로 반환하며, 고유 값 개수와 값이 적은 필드의 값을 포함합니다. MCP 서버는 각 읽기를 도구로 제공합니다: `findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues`, `getSchemaSummary`.

## 배포

| 환경 | 방법 |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3` |
| Docker Compose | [`deploy/templates/docker-compose`](https://github.com/rush-db/rushdb/tree/main/deploy/templates/docker-compose), Caddy 뒤에서 HTTPS로 동작하는 버전 포함 |
| Kubernetes | [Helm 차트](https://github.com/rush-db/rushdb/tree/main/charts/rushdb): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token="$(openssl rand -hex 32)"` |
| Linux VM | [cloud-init](https://github.com/rush-db/rushdb/tree/main/deploy/templates/cloud-init) (DigitalOcean, Hetzner, AWS, Google Cloud, Azure용); [systemd 유닛](https://github.com/rush-db/rushdb/tree/main/deploy/templates/systemd) |
| 플랫폼 | [Fly.io](https://github.com/rush-db/rushdb/tree/main/deploy/templates/fly), [Render](https://github.com/rush-db/rushdb/tree/main/deploy/templates/render), [Coolify](https://github.com/rush-db/rushdb/tree/main/deploy/templates/coolify), [Dokploy](https://github.com/rush-db/rushdb/tree/main/deploy/templates/dokploy) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

포트, 프로브, 용량 산정, 서비스로 실행하기: [Deployment](https://docs.rushdb.com/engine/operations/deployment/). 모든 방법을 한 페이지에서:
[docs.rushdb.com/deploy](https://docs.rushdb.com/deploy/).

### 다른 설치 방법

| 방법 | 명령 | 참고 |
|---|---|---|
| 설치 스크립트 | `curl -fsSL https://rushdb.com/install.sh \| sh` | Linux(glibc 2.28 이상)와 macOS 11 이상, x86-64와 arm64. `sh -s -- --version v3.0.0`으로 릴리스를 고정합니다. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS와 Linux. `brew services start rushdb`로 백그라운드에서 실행합니다. |
| Docker | `rushdb/rushdb:3` | `linux/amd64`와 `linux/arm64`, `ghcr.io/rush-db/rushdb`로도 배포. 태그는 `3`, `3.0`, 정확한 버전. |
| Cargo | `cargo binstall rushdb-server` · `cargo install rushdb-server --locked` | `binstall`은 릴리스 바이너리를 받고, `install`은 소스에서 컴파일합니다(C/C++ 컴파일러 필요). |
| 릴리스 아카이브 | [GitHub releases](https://github.com/rush-db/rushdb/releases) | SHA-256 체크섬, SPDX SBOM, Sigstore 서명 포함. |
| 소스 | `cargo build --release -p rushdb-server --bin rushdb` | 툴체인은 `rust-toolchain.toml`에 고정. [설치](https://docs.rushdb.com/engine/getting-started/installation/) 참고. |

Windows는 네이티브로 지원하지 않습니다. WSL 2나 Docker를 사용하세요. 노드를
루프백 밖으로 노출하기 전에 [보안](https://docs.rushdb.com/engine/operations/security/) 문서를 읽어 주세요.

## 무엇을 만들 수 있나

- **에이전트 메모리.** 메모리는 사용자, 에이전트, 세션 단위로 구분된 레코드이며 그것을 만든 대화 턴과 연결되어 있어, 불러올 때 메모리의 출처가 함께 나옵니다. OpenClaw와 Hermes용 프로바이더를 바로 쓸 수 있습니다.
- **회사 브레인과 RAG.** 문서, CRM, 지원, 결제 데이터가 각자의 형태로 들어오고, 이메일이나 계정 ID처럼 이미 공유하는 키로 연결됩니다. 검색은 먼저 올바른 계정이나 버전으로 좁힌 뒤 벡터와 BM25를 함께 써서 순위를 매기며, 모든 사실은 그 근거가 되는 레코드를 인용합니다.
- **패싯 검색.** 개수가 카탈로그에서 나오는 필터 사이드바. 개수는 현재 필터 범위에서 정확하며, 하이브리드 검색도 같은 필터 아래에서 순위를 매깁니다. 따로 동기화할 검색 인덱스가 없습니다.
- **앱 안의 AI 검색과 어시스턴트.** 텍스트와 벡터 인덱스가 데이터베이스 안에 있어 앱이 쓸 때마다 최신 상태를 유지하며, SDK, MCP 서버, SQL 클라이언트가 같은 레코드를 읽습니다.
- **에이전트 기반 데이터 분석.** 에이전트가 어떤 라벨과 필드가 있고 각 값이 어떻게 적혀 있는지 읽은 뒤, 그에 맞춰 SQL, Cypher 또는 집계를 작성합니다. 모든 답에는 누구나 다시 실행할 수 있는 쿼리가 함께 나옵니다.
- **스키마가 계속 바뀌는 앱.** 어떤 JSON이든 넣으세요. 타입은 추론되고, 중첩 객체는 연결된 레코드가 되며, 새 필드는 그것을 추가한 쓰기와 함께 카탈로그에 나타납니다. 마이그레이션이 필요 없습니다.

## 기능

- **창발적 스키마.** 타입은 값마다 추론되고, 중첩된 객체는 연결된 레코드가 되며, 카탈로그는 정확한 개수와 값 목록을 유지합니다. [스키마 카탈로그](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **그래프 쿼리.** 레이블로 관계를 가로질러 필터링하고, 타입과 방향으로 관계를 제한하며, 관련 레코드를 바인딩해 프로젝션과 집계에 사용합니다. [관계와 순회](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **내장 검색.** 직접 제공한 벡터에 대한 벡터 인덱스, BM25 전문 검색, 그리고 둘을 결합한 하이브리드 검색. [인덱스와 검색](https://docs.rushdb.com/engine/indexes-and-search/)
- **집계.** `select`와 `groupBy`로 개수, 합계, 평균, 시간 버킷, 중첩 컬렉션을 구합니다. [집계](https://docs.rushdb.com/engine/query/aggregations/)
- **쓰던 언어 그대로.** PostgreSQL 와이어 프로토콜 위의 SQL, Gremlin, SQL/PGQ, Mongo 스타일 문서, 그리고 릴리스 후보 단계의 openCypher와 ISO GQL. [쿼리 언어](https://docs.rushdb.com/engine/languages/)
- **내구성.** 선행 기록 로그(WAL), 트랜잭션, 특정 시점 복구를 지원하는 백업, 읽기 복제본. [운영](https://docs.rushdb.com/engine/operations/)
- **바이너리 하나.** 하나의 프로세스에서 HTTP, PostgreSQL 와이어, gRPC, Bolt를 제공하며, 격리된 테넌트를 선택적으로 쓸 수 있습니다. [연결](https://docs.rushdb.com/engine/connectivity/)

## 연결

| 인터페이스 | 기본값 | 활성화 |
|---|---|---|
| [HTTP 네이티브 프로토콜](https://docs.rushdb.com/engine/connectivity/http-api/) | `127.0.0.1:3000`, `POST /protocol/v1/execute` | 항상 켜짐. `--port`로 변경 |
| [PostgreSQL 와이어 프로토콜](https://docs.rushdb.com/engine/connectivity/postgresql-wire/) | `127.0.0.1:5432` | `sql`이 켜져 있으면 켜짐(기본값). `--pgwire-port 0`으로 끔 |
| [gRPC](https://docs.rushdb.com/engine/connectivity/grpc/) (`rushdb.v1.Database/Execute`) | 꺼짐 | `--grpc-port <port>` |
| [Bolt 5.0](https://docs.rushdb.com/engine/connectivity/bolt/) | 꺼짐 | `--bolt-port <port>`, 그리고 `--languages`에 `cypher` 포함 |

모든 리스너는 `--listen` 주소(기본값 `127.0.0.1`)에 바인딩하고 하나의 TLS 인증서
(`--tls-cert`, `--tls-key`)를 공유합니다. 자세한 내용은 [연결](https://docs.rushdb.com/engine/connectivity/), [인증](https://docs.rushdb.com/engine/connectivity/authentication/),
[SQL과 Cypher 클라이언트](https://docs.rushdb.com/connect/sql-and-cypher/)를 참고하세요. SDK, MCP 서버, 아래 표의 통합은
다이렉트 모드로 셀프 호스팅 엔진에 연결합니다. [Connect](https://docs.rushdb.com/connect/)를 참고하세요.

## 생태계

| 프로젝트 | 용도 |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | TypeScript 및 Python 앱 |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor 및 기타 MCP 클라이언트 |
| [에이전트 스킬](https://github.com/rush-db/skills) · [에이전트 플러그인](https://github.com/rush-db/agent-plugins) | 코딩 에이전트 |
| [LangChain 및 LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | 검색, 메모리, 속성 그래프 저장소 |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [메모리 계약](https://github.com/rush-db/agent-memory-contract) | 에이전트 메모리 프로바이더 |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [Obsidian 플러그인](https://github.com/rush-db/obsidian-plugin) | 변경 데이터 캡처, 노트 |

모두 셀프 호스팅 엔진과 함께 동작합니다. 코딩 에이전트에게 RushDB 설정을
맡기려면 [rushdb.com/agent-setup](https://rushdb.com/agent-setup)을 건네주세요.

## 셀프 호스팅과 RushDB Cloud

이 저장소가 엔진이며, 전부 직접 호스팅할 수 있습니다. [RushDB Cloud](https://app.rushdb.com)는 같은 엔진을 대신 운영하고 아래 표시된 기능을 더합니다. SDK와 MCP 서버는 둘 다에서 동작합니다. 셀프 호스팅 엔진에서는 Cloud 전용 호출이 아무것도 보내기 전에 `CloudOnlyError`로 실패합니다.

![합성 샘플 데이터를 사용한 RushDB 대시보드: 레코드 표, 이어서 회사와 부서 클러스터의 그래프 뷰, 마지막으로 한 부서를 확대하고 레코드 패널에 그 부서의 직원이 나열되는 장면](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| 기능 | 셀프 호스팅 엔진 | RushDB Cloud |
|---|---|---|
| 엔진과 API: 레코드, 그래프 쿼리, 검색, 집계, 라이브 스키마 | ✓ | ✓ |
| SDK와 MCP 서버 | ✓ 다이렉트 모드, stdio를 통한 MCP | ✓ OAuth를 지원하는 호스팅 MCP 포함 |
| JSON 및 CSV 가져오기 | ✓ | ✓ |
| 레코드, 그래프 뷰, 쿼리 빌더가 있는 [대시보드](https://docs.rushdb.com/rushdb-cloud/) | SDK 또는 아무 PostgreSQL 클라이언트 사용 | ✓ |
| [관리형 임베딩](https://docs.rushdb.com/rushdb-cloud/capabilities/) | 직접 쓰는 모델의 벡터를 가져오기 | ✓ 월간 한도 |
| [자연어 검색](https://docs.rushdb.com/learn/search-query/smart-search/) | 쿼리를 직접 또는 에이전트로 작성 | ✓ 월간 한도 |
| [관계 제안](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | 관계를 직접 생성 | ✓ |
| 지표와 필터의 [정의](https://docs.rushdb.com/learn/search-query/definitions/) | 규칙을 직접 만든 프롬프트와 SQL에 보관 | ✓ |
| SaaS 도구 [연결](https://docs.rushdb.com/learn/data-sources/connections/), 온프레미스 데이터베이스용 RushDB Sync | 직접 작성한 동기화 코드 또는 Debezium 싱크 | ✓ |
| 프로젝트와 API 키 | 테넌트와 토큰을 직접 관리 | ✓ |
| 팀 액세스 | — | 유료 플랜 |
| 백업 | 직접 운영: 백업, 특정 시점 복원, 복제본 | 유료 플랜 |

RushDB Cloud 열의 ✓는 Free 플랜에 포함된다는 뜻입니다. 플랜과 한도는 [rushdb.com/pricing](https://rushdb.com/pricing)에서 확인하세요.

## FAQ

**프로덕션에 써도 되나요?** SQL, 네이티브 JSON 쿼리 언어, Gremlin,
Mongo 스타일 문서는 안정 버전입니다. openCypher, ISO GQL, Bolt는 릴리스
후보입니다. 복제는 비동기이며 승격은 수동입니다.
[상태 페이지](https://docs.rushdb.com/engine/reference/status/)에서 모든
인터페이스와 그 제한 사항을 확인할 수 있습니다.

**별도의 벡터 데이터베이스가 필요한가요?** 아니요. 벡터, 텍스트 인덱스,
관계가 하나의 엔진 안에 있으며, 레코드와 그 벡터를 한 번의 커밋으로 함께 쓸 수 있습니다.
엔진은 직접 제공한 벡터를 검색합니다. RushDB
Cloud는 벡터를 대신 생성해 줄 수 있습니다.

**라이브 스키마란 무엇인가요?** 모든 레이블, 속성, 타입, 값, 관계 형태를 담은 카탈로그로,
커밋마다 갱신되고 정확한 개수를 가집니다. 한 번의 호출로 받아 올 수 있어서
에이전트와 도구가 필드 이름을 추측할 필요가 없습니다.
[스키마 카탈로그](https://docs.rushdb.com/engine/data-model/schema-catalog/)

---

[기여하기](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[보안](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[변경 내역](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x(레거시, Neo4j 기반)는
[`2.x` 브랜치](https://github.com/rush-db/rushdb/tree/2.x)에 있으며
2027-10-28까지 보안 수정이 제공됩니다. [2.x에서 업그레이드](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
