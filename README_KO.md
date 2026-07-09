<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### AI 에이전트와 앱을 위한 메모리 레이어.

JSON을 그대로 보내세요. 에이전트는 자동으로 추론된 살아있는 쿼리 가능한 스키마, 그래프 관계, 시맨틱 검색을 얻습니다.
파이프라인도, 별도의 저장소도, 설계할 스키마도 필요 없습니다.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 웹사이트](https://rushdb.com) • [📖 문서](https://docs.rushdb.com) • [☁️ 클라우드](https://app.rushdb.com) • [🔍 예제](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • **한국어** • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

## 문제

에이전트에는 메모리가 필요합니다. 일반적인 해법은 세 개의 데이터베이스입니다. 키-값을 위한 Redis, 시맨틱 검색을 위한 벡터 저장소, 관계를 위한 그래프 DB, 그리고 이들을 동기화하기 위한 접착 코드까지요.

RushDB는 이 세 가지를 모두 대체합니다. JSON을 한 번 보내세요. 그래프 탐색, 시맨틱 검색, 또는 둘 다를 한 번의 호출로 쿼리할 수 있습니다.

| RushDB 없이                            | RushDB와 함께                            |
| -------------------------------------- | ---------------------------------------- |
| Redis + Pinecone + Neo4j + 접착 코드   | 하나의 API                               |
| 스키마 설계 → 마이그레이션 작성 → 반복 | 어떤 형태든 그대로 보내기, 스키마 불필요 |
| 별도의 임베딩 파이프라인               | 서버 측 관리형 임베딩                    |
| 관계 엣지 수작업 작성                  | 데이터 구조에서 자동 감지                |

---

## 빠른 시작

설정에 따라 두 가지 경로가 있습니다.

- **클라우드** — 관리형, 무료 티어, 30초 만에 실행. [API 키 받기 →](https://app.rushdb.com)
- **셀프 호스팅** — Docker + 자체 Neo4j 인스턴스. [셀프 호스팅으로 이동 →](#self-hosting)

### 클라우드 경로

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### 에이전트 메모리 저장 및 회상

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

## 데이터 다루기

### 중첩된 JSON 가져오기

어떤 JSON 형태든 보내세요. 중첩된 객체와 객체 배열은 연결된 레코드가 됩니다. 레이블, 타입, 관계는 쓰기 시점에 추론됩니다. 스키마도, 마이그레이션 단계도 없습니다.

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

각 중첩 키(`DEPARTMENT`, `EMPLOYEE`)는 레이블이 되고, 각 객체는 레코드가 되며, 포함 관계는 관계가 됩니다. 모두 자동으로 생성됩니다.

### CSV 가져오기

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

`suggestTypes`는 문자열에서 숫자, 불리언, 날짜를 추론합니다. `skipEmptyValues`는 빈 셀을 빈 값으로 저장하는 대신 설정되지 않은 것으로 처리합니다(`0`과 `false`는 유지됩니다).

### 그래프 탐색

루트 레코드를 _관련_ 레코드에 대한 조건으로 필터링합니다. 임의의 깊이까지, 단일 쿼리로 가능합니다. 관련 레이블은 `labels`가 아니라 `where` **안**에 들어갑니다.

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

### 분석 쿼리 (집계 및 그룹화)

`select`는 집계(`$sum`, `$avg`, `$count`, `$min`, `$max`)로 출력 형태를 만들고, `groupBy`는 차원을 제어합니다. 집계에 `limit`을 추가하지 마세요. 처음 N개의 레코드만 스캔하여 합계를 왜곡합니다.

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

집계는 탐색과 조합됩니다. 예를 들어 부서별 인원수와 급여 총액을 구할 수 있습니다.

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

## Claude, Cursor, 또는 모든 MCP 클라이언트에 연결

RushDB는 MCP 서버를 함께 제공합니다. 에이전트는 별도 설정 없이 영속적이고 구조화된 메모리를 얻습니다.

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

이것을 Claude Desktop, Cursor, 또는 Windsurf의 MCP 설정에 넣으세요. 이제 에이전트는 레코드를 생성하고, 의미로 검색하고, 관계를 탐색하고, 스키마를 조사할 수 있습니다. 모두 자연어를 통해서요.

---

## 무엇이 들어 있나

| 기능                          | 의미                                                                                                                                  |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **관리형 임베딩**             | 문자열 속성을 한 번 인덱싱하면, 모든 쓰기가 서버 측에서 자동으로 임베딩됩니다                                                         |
| **그래프 + 벡터 단일 쿼리**   | 시맨틱 유사도와 관계 탐색이 단일 호출로 조합됩니다                                                                                    |
| **제로 스키마**               | 어떤 JSON 형태든 보내세요. RushDB가 타입을 추론하고, 속성을 생성하고, 레코드를 연결합니다                                             |
| **추론된 스키마**             | 속성이 일급 노드가 됩니다. 타입, 레이블, 관계가 쓰기 시점에 발견되므로, 데이터가 도착하는 대로 쿼리 가능한 스키마가 스스로 구축됩니다 |
| **ACID 트랜잭션**             | 동시 실행 에이전트가 공유 메모리를 손상시키지 않습니다. 내부적으로 Neo4j를 사용합니다                                                 |
| **자기 기술적**               | 에이전트는 추론된 스키마(레이블, 속성, 값 범위)를 조사하여 안전하게 쿼리할 수 있는 대상을 파악합니다                                  |
| **MCP 네이티브**              | 발견 우선 쿼리 프롬프트가 내장된 완전한 MCP 서버                                                                                      |
| **에이전트 스킬**             | 설치 가능한 `@rushdb/skills` 패키지. 스킬 호환 에이전트에게 RushDB로 쿼리, 모델링, 기억하는 법을 단 한 번의 명령으로 가르칩니다       |
| **통합 쿼리 API**             | 그래프, 벡터, 집계, 조사를 위한 하나의 JSON 형태                                                                                      |
| **셀프 호스트 또는 클라우드** | Docker + 자체 Neo4j, 또는 관리형 클라우드. 완전한 데이터 소유권                                                                       |

---

## 사용 사례

| 사용 사례              | RushDB가 대체하는 것            | 핵심 API                                                      |
| ---------------------- | ------------------------------- | ------------------------------------------------------------- |
| **에이전트 메모리**    | Redis + 벡터 저장소 + 그래프 DB | `db.records.vectorSearch({ query, where: { agent_id } })`     |
| **컨텍스트 기반 RAG**  | 평면 벡터 저장소                | `db.records.find({ where, labels })` + 관계 탐색              |
| **스키마 없는 앱**     | Postgres + 마이그레이션 + ETL   | `db.records.importJson(nestedJson)`                           |
| **연결된 데이터 제품** | 여러 개의 조인된 서비스         | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })` |

---

## 셀프 호스팅

> **셀프 호스트 경로** — 자체 인프라에서 RushDB를 실행합니다. APOC 플러그인이 포함된 Neo4j 2026.01.4+ 가 필요합니다.

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
<summary>전체 환경 변수</summary>

| 이름                              | 설명                               | 필수          | 기본값   |
| --------------------------------- | ---------------------------------- | ------------- | -------- |
| `NEO4J_URL`                       | Neo4j 연결 URL                     | 예            | —        |
| `NEO4J_USERNAME`                  | Neo4j 사용자 이름                  | 예            | neo4j    |
| `NEO4J_PASSWORD`                  | Neo4j 비밀번호                     | 예            | —        |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | API 토큰 암호화를 위한 32자 키     | 예 (프로덕션) | —        |
| `RUSHDB_PORT`                     | HTTP 포트                          | 아니오        | 3000     |
| `RUSHDB_LOGIN`                    | 관리자 로그인                      | 아니오        | admin    |
| `RUSHDB_PASSWORD`                 | 관리자 비밀번호                    | 아니오        | password |
| `RUSHDB_BASE_URL`                 | synx 할당을 위한 공개/기본 API URL | 아니오        | —        |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | 관리형 synx 워커를 위한 내부 토큰  | 아니오        | —        |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | synx 대상 쓰기를 위한 내부 쓰기 키 | 아니오        | —        |

관리형 synx 워커는 데몬으로 실행됩니다. 실행 가능한 커넥터를 폴링하고, 커넥터 리스를 갱신하고, 정상 종료 시 리스를 해제하며, 크래시 후 platform/core가 만료된 리스를 회수하도록 합니다.

</details>

<details>
<summary>로컬 개발 (번들된 Neo4j)</summary>

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
<summary>아키텍처: RushDB가 데이터를 구조화하는 방식 (LMPG)</summary>

RushDB는 **레이블이 지정된 메타 속성 그래프(Labeled Meta Property Graph, LMPG)** 모델을 사용합니다. 속성은 단순히 레코드에 첨부된 키-값 쌍이 아니라 일급 그래프 노드("HyperProperties")로 승격됩니다.

이것이 의미하는 바는 다음과 같습니다.

- **사전 설계 없는 스키마** — 속성이 그래프 노드이기 때문에, 스키마는 _설계되는 것이 아니라 데이터로부터 추론됩니다_. 레이블, 타입, 값 범위, 관계 토폴로지가 쓰기 시점에 발견되어 즉시 쿼리 가능합니다. 수동 스키마 모델링도, RDF/OWL 툴체인도 필요 없습니다
- **자동 감지된 관계** — 속성을 공유하는 레코드들은 엣지를 수작업으로 만들지 않아도 연결됩니다
- **스키마 조사** — 에이전트는 레이블, 속성 타입, 값 범위, 관계 토폴로지를 단일 쿼리로 열거할 수 있습니다
- **소프트 제약** — 경직된 사전 스키마 없이 타입 응집도 점수화, 카디널리티 추적, 벡터 차원 강제를 수행합니다
- **통합 쿼리 표면** — 동일한 필터 표현식이 레코드, 레이블, 속성, 관계 전반에서 작동합니다

하나의 SearchQuery가 여러 관점(레코드 + 속성 통계 + 집계)을 동시에 검색하여, 별도 시스템 아키텍처에서 흔히 나타나는 N+1 조사 패턴을 피합니다.

[전체 LMPG 아키텍처 글 읽기 →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## 문서

| 주제               | 링크                                                           |
| ------------------ | -------------------------------------------------------------- |
| 빠른 튜토리얼      | https://docs.rushdb.com/get-started/quick-tutorial             |
| 벡터 / 시맨틱 검색 | https://docs.rushdb.com/concepts/search/where#vector-operators |
| 필터링 & 탐색      | https://docs.rushdb.com/concepts/search/where                  |
| 그룹화 & 집계      | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK     | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK         | https://docs.rushdb.com/python-sdk/introduction                |
| REST API           | https://docs.rushdb.com/rest-api/introduction                  |
| MCP 서버           | packages/mcp-server/README.md                                  |
| 에이전트 스킬      | packages/skills/README.md                                      |

---

## RushDB를 사용하지 않아야 할 때

- 매우 높은 쓰기 처리량에서 1밀리초 미만의 지연 시간이 필요한 경우 — RushDB는 Neo4j를 기반으로 하며, 이는 원시 쓰기 속도보다 일관성과 쿼리 표현력을 우선시합니다.
- 관계나 시맨틱 검색 없이 평면 키-값 저장만 필요한 경우 — 더 단순한 저장소가 더 가벼울 것입니다.
- 데이터베이스 수준에서 강제되는 경직된 관계형 스키마가 필요한 경우 — RushDB는 의도적으로 스키마가 없습니다.

---

## 기여하기

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

전체 가이드라인은 [CONTRIBUTING.md](CONTRIBUTING.md)를 참고하세요. 이슈와 PR을 환영합니다.

---

## 라이선스

| 경로                    | 라이선스            |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

아직 지원되지 않는 기능이 필요하신가요? 이슈를 열어주세요. 설계 논의를 환영합니다.
