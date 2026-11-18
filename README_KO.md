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

</div>

---

앱에서 JSON을 쓰세요. RushDB는 이를 타입이 있는 레코드로 저장하고 그래프로
연결합니다. 같은 레코드가 관계 쿼리, 벡터 검색과 전문(full-text) 검색, 집계에
모두 답하므로, 동기화를 유지해야 하는 두 번째 저장소가 필요 없습니다. 쓰기마다
존재하는 레이블, 필드, 타입, 값의 카탈로그도 갱신됩니다. 에이전트는 먼저 무엇이
있는지 확인한 다음, 거기에 맞는 쿼리를 작성합니다.
SDK, SQL, Cypher를 사용할 수 있습니다. Rust 바이너리 하나, Apache-2.0.

| 그래프 | 벡터 및 전문 검색 | 분석 |
|---|---|---|
| 중첩된 JSON은 서로 연결된 레코드가 됩니다. 하나의 쿼리에서 관계를 가로질러 필터링하고, 타입과 방향이 있는 관계를 따라갑니다. | 벡터 검색(정확 검색 및 HNSW), BM25 전문 검색, 하이브리드 검색. 모두 같은 그래프 조건으로 필터링됩니다. | `groupBy`로 개수, 합계, 평균, 최솟값, 최댓값을 구하거나, PostgreSQL 와이어 프로토콜로 `GROUP BY`를 쓰는 일반 SQL을 사용합니다. |

## 빠른 시작

```bash
curl -fsSL https://rushdb.com/install.sh | sh    # or: brew install rush-db/tap/rushdb
export PATH="$HOME/.rushdb/bin:$PATH"             # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up`은 `http://127.0.0.1:3000`에서 서비스하며, 데이터는 `./.rushdb`에 저장되고
토큰은 `local-token`입니다. 다른 터미널에서 `npm install @rushdb/javascript-sdk`를 실행합니다:

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

스키마는 레코드를 스캔해서 얻는 것이 아니라 카탈로그에서 나옵니다:

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
```

<details>
<summary>같은 예제를 Python으로</summary>

`pip install rushdb`:

```python
from rushdb import RushDB

db = RushDB("local-token", direct=True, engine_url="http://127.0.0.1:3000")

db.records.import_json(label="COMPANY", data={"name": "Acme", "DEPARTMENT": [
    {"name": "Engineering", "EMPLOYEE": [
        {"name": "Ada", "role": "Staff Engineer", "salary": 180000},
        {"name": "Linus", "role": "Engineer", "salary": 140000}]},
    {"name": "Sales", "EMPLOYEE": [{"name": "Grace", "role": "Account Executive", "salary": 120000}]}]})

engineers = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {"role": {"$contains": "Engineer"}, "DEPARTMENT": {"COMPANY": {"name": "Acme"}}},
    "orderBy": {"name": "asc"},
})
print([employee.data["name"] for employee in engineers])  # ['Ada', 'Linus']

per_department = db.records.find({
    "labels": ["EMPLOYEE"],
    "where": {"DEPARTMENT": {"$alias": "$department"}},
    "select": {"people": {"$count": "*"}, "avgSalary": {"$avg": "$record.salary"}},
    "groupBy": ["$department.name"],
})
print([row.data for row in per_department])

print(db.ai.get_schema_markdown().data)
```

</details>

SQL이 익숙하신가요? 레이블이 곧 테이블입니다:

```bash
PGPASSWORD=local-token psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

다음: [전체 빠른 시작](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker 및 기타 설치 방법](https://docs.rushdb.com/engine/getting-started/installation/)

## 무엇을 만들 수 있나

| | |
|---|---|
| **에이전트 메모리** | 메모리를 관계가 있는 레코드로 저장하고, 범위로 필터링하며, 저장한 벡터로 순위를 매깁니다. OpenClaw와 Hermes용 프로바이더를 바로 쓸 수 있습니다. |
| **앱 데이터 검색** | 앱이 이미 쓰고 있는 레코드에 대한 전문, 벡터, 하이브리드 검색. 같은 쿼리 안에서 그래프 필터도 함께 씁니다. |
| **연결된 데이터에 대한 질문** | 에이전트가 스키마를 읽은 뒤, 직접 검토할 수 있는 쿼리를 작성합니다. JSON 쿼리 언어, SQL 또는 Cypher로요. |
| **내보내기 없는 리포트** | 앱이 쓴 레코드를 SDK나 아무 PostgreSQL 클라이언트에서 그룹화하고 집계합니다. |

## 기능

- **창발적 스키마.** 타입은 값마다 추론되고, 중첩된 객체는 연결된 레코드가 되며, 카탈로그는 정확한 개수와 값 목록을 유지합니다. [스키마 카탈로그](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **그래프 쿼리.** 레이블로 관계를 가로질러 필터링하고, 타입과 방향으로 관계를 제한하며, 관련 레코드를 바인딩해 프로젝션과 집계에 사용합니다. [관계와 순회](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **내장 검색.** 직접 제공한 벡터에 대한 벡터 인덱스, BM25 전문 검색, 그리고 둘을 결합한 하이브리드 검색. [인덱스와 검색](https://docs.rushdb.com/engine/indexes-and-search/)
- **집계.** `select`와 `groupBy`로 개수, 합계, 평균, 시간 버킷, 중첩 컬렉션을 구합니다. [집계](https://docs.rushdb.com/engine/query/aggregations/)
- **쓰던 언어 그대로.** PostgreSQL 와이어 프로토콜 위의 SQL, Gremlin, SQL/PGQ, Mongo 스타일 문서, 그리고 릴리스 후보 단계의 openCypher와 ISO GQL. [쿼리 언어](https://docs.rushdb.com/engine/languages/)
- **내구성.** 선행 기록 로그(WAL), 트랜잭션, 특정 시점 복구를 지원하는 백업, 읽기 복제본. [운영](https://docs.rushdb.com/engine/operations/)
- **바이너리 하나.** 하나의 프로세스에서 HTTP, PostgreSQL 와이어, gRPC, Bolt를 제공하며, 격리된 테넌트를 선택적으로 쓸 수 있습니다. [연결](https://docs.rushdb.com/engine/connectivity/)

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

## 어디서나 실행

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes(Helm), cloud-init, systemd, Fly.io, Render,
Coolify, Dokploy: [배포](https://docs.rushdb.com/deploy/)를 참고하세요. 노드를
루프백 밖으로 노출하기 전에 [보안](https://docs.rushdb.com/engine/operations/security/) 문서를
읽어 주세요.

## 셀프 호스팅과 RushDB Cloud

- **셀프 호스팅:** 바로 이 저장소입니다. 위의 모든 기능은 직접 설치한 엔진에서 동작합니다.
- **[RushDB Cloud](https://app.rushdb.com):** 같은 엔진의 관리형 버전으로, 여기에
  대시보드, 관리형 임베딩, 자연어 검색, SaaS 도구용 커넥터가 더해집니다. SDK와 MCP 서버는 둘 다에서 동작합니다.

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

**Windows는요?** WSL 2나 Docker를 사용하세요.

---

[기여하기](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[보안](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[변경 내역](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x(레거시, Neo4j 기반)는
[`2.x` 브랜치](https://github.com/rush-db/rushdb/tree/2.x)에 있으며
2027-10-28까지 보안 수정이 제공됩니다. [2.x에서 업그레이드](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • **한국어** • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)
