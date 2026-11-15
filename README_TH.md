<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="โลโก้ RushDB" width="96">

# RushDB

<sub>ฐานข้อมูลสำหรับ AI</sub>

**กราฟ เวกเตอร์ และการวิเคราะห์ข้อมูลบนเรคคอร์ดชุดเดียวกัน ทุกการเขียนจะอัปเดตสคีมาที่เป็นปัจจุบันของ label, ฟิลด์ และค่าต่าง ๆ เอเจนต์จึงคิวรีจากฟิลด์ที่มีอยู่จริงแทนการเดา**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[เอกสาร](https://docs.rushdb.com) • [เริ่มต้นใช้งานอย่างรวดเร็ว](https://docs.rushdb.com/engine/getting-started/quickstart/) • [การสนทนา](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • **ไทย**

</div>

---

นำข้อมูลเข้ามาในรูปแบบที่คุณมีอยู่ได้เลย ทั้ง JSON แบบซ้อน, CSV, SQL `INSERT`, การเขียนแบบ Cypher
หรือสไตล์ Mongo หรือ change streams จากฐานข้อมูลของคุณ RushDB จะเก็บเป็นเรคคอร์ดที่มีชนิดข้อมูลและเชื่อมโยงกันเป็นกราฟ
เรคคอร์ดชุดเดียวกันนี้ตอบได้ทั้งคิวรีความสัมพันธ์ การค้นหาแบบเวกเตอร์และฟูลเท็กซ์
และการรวมข้อมูล จึงไม่มีที่เก็บข้อมูลชุดที่สองที่ต้องคอยซิงก์ ทุกการเขียนยังอัปเดตแคตตาล็อกของ
label, ฟิลด์, ชนิดข้อมูล และค่าที่มีอยู่จริงด้วย เอเจนต์จึงคิวรีฟิลด์ที่มีอยู่จริง
เชื่อมต่อผ่าน HTTP, โปรโตคอล wire ของ PostgreSQL, Bolt, gRPC หรือ MCP จาก SDK หรือไคลเอนต์ PostgreSQL ใดก็ได้
ไบนารี Rust ตัวเดียว ภายใต้ Apache-2.0

| กราฟ | การค้นหาแบบเวกเตอร์และฟูลเท็กซ์ | การวิเคราะห์ข้อมูล |
|---|---|---|
| JSON ที่ซ้อนกันกลายเป็นเรคคอร์ดที่เชื่อมโยงกัน กรองข้ามความสัมพันธ์และไล่ตามความสัมพันธ์ที่มีชนิดและทิศทางได้ในคิวรีเดียว | การค้นหาแบบเวกเตอร์ (แบบตรงตัวและ HNSW) ฟูลเท็กซ์ BM25 และแบบไฮบริด กรองด้วยเงื่อนไขกราฟชุดเดียวกัน | นับ ผลรวม ค่าเฉลี่ย ค่าต่ำสุดและสูงสุดด้วย `groupBy` หรือใช้ SQL ธรรมดากับ `GROUP BY` ผ่านโปรโตคอล wire ของ PostgreSQL |

## เริ่มต้นใช้งานอย่างรวดเร็ว

![บันทึกหน้าจอเทอร์มินัล: ติดตั้ง RushDB เริ่มด้วย rushdb up จากนั้นใช้ Python นำเข้า JSON แบบซ้อน กรองพนักงานตามแผนก นับจำนวนคนและเงินเดือนเฉลี่ยของแต่ละแผนก และพิมพ์สคีมาที่เป็นปัจจุบัน](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

**ให้เอเจนต์เขียนโค้ดของคุณตั้งค่าให้** วางพรอมต์นี้ให้เอเจนต์ ([rushdb.com/agent-setup](https://rushdb.com/agent-setup)):

```text
Set up RushDB as this project's database.

1. Install the RushDB MCP server (@rushdb/mcp-server) and the RushDB agent skills (github.com/rush-db/skills).
2. Use RushDB Cloud with an API key from app.rushdb.com, or a local engine:
   curl -fsSL https://rushdb.com/install.sh | sh && rushdb up
   (direct mode: engine URL http://127.0.0.1:3000, token local-token)
3. Before writing any query, read the schema catalog (labels, properties, types, relationship shapes) and use only fields that exist.
4. Docs: https://docs.rushdb.com/
```

หรือตั้งค่าด้วยตัวเอง:

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # only if rushdb is not found yet
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` ให้บริการที่ `http://127.0.0.1:3000` โดยเก็บข้อมูลไว้ใน `./.rushdb` และใช้
โทเค็น `local-token` จากนั้นในเทอร์มินัลอื่น รัน `pip install rushdb`:

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
<summary>ตัวอย่างเดียวกันใน TypeScript</summary>

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

ใช้ SQL เป็นอยู่แล้วใช่ไหม? label ก็คือตาราง:

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

ขั้นต่อไป: [คู่มือเริ่มต้นใช้งานฉบับเต็ม](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker และวิธีติดตั้งอื่น ๆ](https://docs.rushdb.com/engine/getting-started/installation/)

## สิ่งที่เอเจนต์เห็นก่อนคิวรี

บรรทัดสุดท้ายของ quickstart พิมพ์สคีมาที่เอเจนต์อ่านเป็นอย่างแรก สคีมานี้มาจากแคตตาล็อกที่อัปเดตทุกครั้งที่เขียน ไม่ได้มาจากการสแกนเรคคอร์ด:

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

บนสโตร์ที่ใหญ่และหลากหลาย เอเจนต์ไม่โหลดทั้งหมดนี้ แต่จะค่อยๆ แคบลงจากภาพรวมไปจนถึงค่าที่แน่นอน ด้วยการอ่านเล็กๆ ทีละครั้ง (จากการรันเดียวกับด้านบน):

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

`get_schema_summary` คืนแคตตาล็อกเดียวกันในรูป JSON พร้อมจำนวนค่าที่ไม่ซ้ำและค่าของฟิลด์ที่มีค่าน้อย เซิร์ฟเวอร์ MCP มีการอ่านแต่ละแบบเป็นเครื่องมือ: `findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues` และ `getSchemaSummary`

## การ Deploy

| ที่ไหน | อย่างไร |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3` |
| Docker Compose | [`deploy/templates/docker-compose`](https://github.com/rush-db/rushdb/tree/main/deploy/templates/docker-compose) มีแบบ HTTPS ผ่าน Caddy ด้วย |
| Kubernetes | [Helm chart](https://github.com/rush-db/rushdb/tree/main/charts/rushdb): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token="$(openssl rand -hex 32)"` |
| Linux VM | [cloud-init](https://github.com/rush-db/rushdb/tree/main/deploy/templates/cloud-init) สำหรับ DigitalOcean, Hetzner, AWS, Google Cloud และ Azure; [systemd unit](https://github.com/rush-db/rushdb/tree/main/deploy/templates/systemd) |
| แพลตฟอร์ม | [Fly.io](https://github.com/rush-db/rushdb/tree/main/deploy/templates/fly), [Render](https://github.com/rush-db/rushdb/tree/main/deploy/templates/render), [Coolify](https://github.com/rush-db/rushdb/tree/main/deploy/templates/coolify), [Dokploy](https://github.com/rush-db/rushdb/tree/main/deploy/templates/dokploy), [Railway](https://railway.com/deploy/rushdb) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb) [![Deploy on Railway](https://railway.com/button.svg)](https://railway.com/deploy/rushdb)

พอร์ต probe การกำหนดขนาด และการรันเป็นบริการ: ดู [Deployment](https://docs.rushdb.com/engine/operations/deployment/) ทุกวิธีอยู่ในหน้าเดียว:
[docs.rushdb.com/deploy](https://docs.rushdb.com/deploy/)

### วิธีติดตั้งอื่น ๆ

| วิธี | คำสั่ง | หมายเหตุ |
|---|---|---|
| สคริปต์ติดตั้ง | `curl -fsSL https://rushdb.com/install.sh \| sh` | Linux (glibc 2.28+) และ macOS 11+ บน x86-64 และ arm64 ใช้ `sh -s -- --version v3.0.0` เพื่อล็อกรีลีส |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS และ Linux ใช้ `brew services start rushdb` เพื่อรันเบื้องหลัง |
| Docker | `rushdb/rushdb:3` | `linux/amd64` และ `linux/arm64` มีที่ `ghcr.io/rush-db/rushdb` ด้วย แท็ก `3`, `3.0` และเวอร์ชันเต็ม |
| Cargo | `cargo binstall rushdb-server` · `cargo install rushdb-server --locked` | `binstall` ดาวน์โหลดไบนารีของรีลีส ส่วน `install` คอมไพล์เองและต้องมีคอมไพเลอร์ C/C++ |
| ไฟล์รีลีส | [GitHub releases](https://github.com/rush-db/rushdb/releases) | มี checksum SHA-256, SBOM แบบ SPDX และลายเซ็น Sigstore |
| ซอร์สโค้ด | `cargo build --release -p rushdb-server --bin rushdb` | toolchain ล็อกไว้ใน `rust-toolchain.toml` ดู [การติดตั้ง](https://docs.rushdb.com/engine/getting-started/installation/) |

ไม่รองรับ Windows โดยตรง ให้ใช้ WSL 2 หรือ Docker อ่าน
[ความปลอดภัย](https://docs.rushdb.com/engine/operations/security/) ก่อน
เปิดโหนดให้เข้าถึงได้นอกเหนือจาก loopback

## สิ่งที่คุณสร้างได้

- **หน่วยความจำของเอเจนต์**: ความจำคือเรคคอร์ดที่แยกตามผู้ใช้ เอเจนต์ หรือเซสชัน และเชื่อมกับเทิร์นที่สร้างมัน การเรียกคืนจึงบอกได้ว่าความจำนั้นมาจากไหน มีผู้ให้บริการสำเร็จรูปสำหรับ OpenClaw และ Hermes
- **Company brain และ RAG**: ข้อมูลเอกสาร CRM ซัพพอร์ต และบิลลิงเข้ามาในรูปแบบของตัวเอง และเชื่อมกันด้วยคีย์ที่มีร่วมกันอยู่แล้ว เช่น อีเมลหรือ ID บัญชี การค้นคืนจะกรองไปที่บัญชีหรือเวอร์ชันที่ถูกต้องก่อน แล้วจัดอันดับด้วยเวกเตอร์และ BM25 ร่วมกัน และทุกข้อเท็จจริงอ้างอิงเรคคอร์ดที่อยู่เบื้องหลัง
- **การค้นหาแบบ faceted**: แถบตัวกรองที่จำนวนมาจากแคตตาล็อก จำกัดตามตัวกรองปัจจุบันและแม่นยำ พร้อมการค้นหาแบบไฮบริดที่จัดอันดับภายใต้ตัวกรองเดียวกัน ไม่ต้องมีดัชนีค้นหาแยกที่ต้องซิงก์
- **การค้นหาด้วย AI และผู้ช่วยในแอปของคุณ**: ดัชนีข้อความและเวกเตอร์อยู่ในฐานข้อมูลและอัปเดตตามที่แอปเขียน SDK เซิร์ฟเวอร์ MCP และไคลเอนต์ SQL อ่านเรคคอร์ดชุดเดียวกัน
- **การวิเคราะห์ข้อมูลโดยเอเจนต์**: เอเจนต์อ่านว่ามีเลเบลและฟิลด์อะไรบ้าง และแต่ละค่าสะกดอย่างไร แล้วจึงเขียน SQL, Cypher หรือการรวมข้อมูลกับฟิลด์เหล่านั้น ทุกคำตอบมาพร้อมคิวรีที่ใครก็รันซ้ำได้ เมื่อให้[โทเคนแบบอ่านอย่างเดียว](https://docs.rushdb.com/engine/connectivity/authentication/#capability-tokens) เอนจินจะปฏิเสธทุกการเขียนของเอเจนต์
- **แอปที่สคีมาเปลี่ยนไปตามการใช้งาน**: ส่ง JSON อะไรก็ได้ ชนิดข้อมูลจะถูกอนุมาน อ็อบเจกต์ที่ซ้อนกันกลายเป็นเรคคอร์ดที่เชื่อมกัน และฟิลด์ใหม่จะปรากฏในแคตตาล็อกพร้อมกับการเขียนที่เพิ่มมัน ไม่ต้องทำ migration

## ฟีเจอร์

- **สคีมาที่เกิดขึ้นเอง**: อนุมานชนิดข้อมูลทีละค่า อ็อบเจกต์ที่ซ้อนกันกลายเป็นเรคคอร์ดที่เชื่อมโยงกัน และแคตตาล็อกเก็บจำนวนที่แม่นยำพร้อมรายการค่า [แคตตาล็อกสคีมา](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **คิวรีกราฟ**: กรองข้ามความสัมพันธ์ด้วย label จำกัดตามชนิดและทิศทาง และผูกเรคคอร์ดที่เกี่ยวข้องเพื่อใช้ในการ projection และการรวมข้อมูล [ความสัมพันธ์และการท่องกราฟ](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **การค้นหาในตัว**: ดัชนีเวกเตอร์บนเวกเตอร์ที่คุณส่งเข้ามา ฟูลเท็กซ์ BM25 และการค้นหาแบบไฮบริดที่รวมทั้งสองแบบเข้าด้วยกัน [ดัชนีและการค้นหา](https://docs.rushdb.com/engine/indexes-and-search/)
- **การรวมข้อมูล**: `select` และ `groupBy` พร้อมการนับ ผลรวม ค่าเฉลี่ย การแบ่งช่วงเวลา และคอลเลกชันแบบซ้อน [การรวมข้อมูล](https://docs.rushdb.com/engine/query/aggregations/)
- **รองรับสิ่งที่คุณใช้อยู่**: SQL ผ่านโปรโตคอล wire ของ PostgreSQL, Gremlin, SQL/PGQ, เอกสารสไตล์ Mongo รวมถึง openCypher และ ISO GQL ในสถานะ release candidate [ภาษาคิวรี](https://docs.rushdb.com/engine/languages/)
- **ทนทาน**: มี write-ahead log, ทรานแซกชัน, การสำรองข้อมูลพร้อมกู้คืนไปยังจุดเวลาที่ต้องการ และ read replica [การดูแลระบบ](https://docs.rushdb.com/engine/operations/)
- **ไบนารีเดียว**: HTTP, PostgreSQL wire, gRPC และ Bolt จากโปรเซสเดียว พร้อม tenant แยกกันแบบเลือกใช้ได้ [การเชื่อมต่อ](https://docs.rushdb.com/engine/connectivity/)

## การเชื่อมต่อ

| ช่องทาง | ค่าเริ่มต้น | วิธีเปิดใช้ |
|---|---|---|
| [โปรโตคอล HTTP ดั้งเดิม](https://docs.rushdb.com/engine/connectivity/http-api/) | `127.0.0.1:3000`, `POST /protocol/v1/execute` | เปิดเสมอ ใช้ `--port` เพื่อย้ายพอร์ต |
| [PostgreSQL wire](https://docs.rushdb.com/engine/connectivity/postgresql-wire/) | `127.0.0.1:5432` | เปิดเมื่อเปิดใช้ `sql` (ค่าเริ่มต้น) ใช้ `--pgwire-port 0` เพื่อปิด |
| [gRPC](https://docs.rushdb.com/engine/connectivity/grpc/) (`rushdb.v1.Database/Execute`) | ปิด | `--grpc-port <port>` |
| [Bolt 5.0](https://docs.rushdb.com/engine/connectivity/bolt/) | ปิด | `--bolt-port <port>` และมี `cypher` ใน `--languages` |
| [MCP แบบโลคัล](https://docs.rushdb.com/connect/mcp/) (`@rushdb/mcp-server`, [tools](https://docs.rushdb.com/learn/reference/mcp/tools/)) | stdio: `npx -y @rushdb/mcp-server` | โหมดตรงไปยังเอนจินที่โฮสต์เอง: `RUSHDB_MODE=direct`, `RUSHDB_ENGINE_URL`, `RUSHDB_ENGINE_TOKEN` |
| [MCP แบบโฮสต์](https://docs.rushdb.com/connect/mcp/) | `https://mcp.rushdb.com/mcp`, OAuth สำหรับ ChatGPT, Claude.ai และเว็บไคลเอนต์อื่น | ฟีเจอร์ของ RushDB Cloud ไม่ต้องติดตั้งอะไร |

ทุก listener ผูกกับที่อยู่ `--listen` (ค่าเริ่มต้น `127.0.0.1`) และใช้ใบรับรอง TLS ชุดเดียวกัน
(`--tls-cert`, `--tls-key`) ดูเพิ่มเติมที่ [การเชื่อมต่อ](https://docs.rushdb.com/engine/connectivity/), [การยืนยันตัวตน](https://docs.rushdb.com/engine/connectivity/authentication/)
และ [ไคลเอนต์ SQL และ Cypher](https://docs.rushdb.com/connect/sql-and-cypher/) SDK, MCP server และการเชื่อมต่อในตารางด้านล่าง
เชื่อมกับเอนจินที่โฮสต์เองในโหมด direct: ดู [Connect](https://docs.rushdb.com/connect/)

## อีโคซิสเต็ม

| โปรเจกต์ | สำหรับ |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | แอป TypeScript และ Python |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor และไคลเอนต์ MCP อื่น ๆ |
| [Agent skills](https://github.com/rush-db/skills) · [ปลั๊กอินสำหรับเอเจนต์](https://github.com/rush-db/agent-plugins) | เอเจนต์เขียนโค้ด |
| [LangChain และ LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | Retrieval, หน่วยความจำ และ property graph store |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [memory contract](https://github.com/rush-db/agent-memory-contract) | provider หน่วยความจำของเอเจนต์ |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [ปลั๊กอิน Obsidian](https://github.com/rush-db/obsidian-plugin) | Change data capture, โน้ต |

ทั้งหมดใช้งานได้กับเอนจินที่โฮสต์เอง หากต้องการให้เอเจนต์เขียนโค้ดของคุณติดตั้ง
RushDB ให้ ส่ง [rushdb.com/agent-setup](https://rushdb.com/agent-setup) ให้เอเจนต์

## โฮสต์เองและ RushDB Cloud

รีโพนี้คือตัวเอนจิน และคุณโฮสต์เองได้ทั้งหมด [RushDB Cloud](https://app.rushdb.com) รันเอนจินเดียวกันให้คุณและเพิ่มฟีเจอร์ที่ระบุไว้ด้านล่าง SDK และเซิร์ฟเวอร์ MCP ใช้ได้กับทั้งสองแบบ บนเอนจินที่โฮสต์เอง SDK จะปฏิเสธการเรียกที่มีเฉพาะใน Cloud ด้วย `CloudOnlyError` ก่อนส่งข้อมูลใดๆ ออกไป และเซิร์ฟเวอร์ MCP จะไม่มีเครื่องมือเหล่านั้นให้ใช้

![แดชบอร์ด RushDB กับข้อมูลตัวอย่างสังเคราะห์: ตารางเรคคอร์ด ตามด้วยมุมมองกราฟของบริษัทและกลุ่มแผนก แล้วซูมเข้าไปที่แผนกหนึ่งซึ่งแสดงรายชื่อพนักงานในแผงเรคคอร์ด](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| ฟีเจอร์ | เอนจินที่โฮสต์เอง | RushDB Cloud |
|---|---|---|
| เอนจินและ API: เรคคอร์ด คิวรีกราฟ การค้นหา การรวมข้อมูล สคีมาแบบสด | ✓ | ✓ |
| SDK และเซิร์ฟเวอร์ MCP | ✓ โหมด direct, MCP ผ่าน stdio | ✓ พร้อม MCP แบบโฮสต์ที่ใช้ OAuth |
| นำเข้า JSON และ CSV | ✓ | ✓ |
| [แดชบอร์ด](https://docs.rushdb.com/rushdb-cloud/) พร้อมเรคคอร์ด มุมมองกราฟ และตัวสร้างคิวรี | ใช้ SDK หรือไคลเอนต์ PostgreSQL ใดก็ได้ | ✓ |
| [Embedding แบบจัดการให้](https://docs.rushdb.com/rushdb-cloud/capabilities/) | นำเวกเตอร์จากโมเดลของคุณเองมาใช้ | ✓ โควตารายเดือน |
| [การค้นหาด้วยภาษาธรรมชาติ](https://docs.rushdb.com/learn/search-query/smart-search/) | สร้างคิวรีเองหรือให้เอเจนต์สร้าง | ✓ โควตารายเดือน |
| [คำแนะนำความสัมพันธ์](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | สร้างความสัมพันธ์เอง | ✓ |
| [นิยาม](https://docs.rushdb.com/learn/search-query/definitions/) สำหรับเมตริกและตัวกรอง | เก็บกฎไว้ในพรอมต์และ SQL ของคุณเอง | ✓ |
| [การเชื่อมต่อ](https://docs.rushdb.com/learn/data-sources/connections/) กับเครื่องมือ SaaS และ RushDB Sync สำหรับฐานข้อมูลในองค์กร | โค้ดซิงก์ของคุณเอง หรือ Debezium sink | ✓ |
| โปรเจกต์และคีย์ API | tenant และโทเคนที่คุณจัดการเอง | ✓ |
| การเข้าถึงแบบทีม | — | แพ็กเกจแบบชำระเงิน |
| การสำรองข้อมูล | คุณดูแลเอง: สำรองข้อมูล กู้คืนแบบ point-in-time และสำเนา | แพ็กเกจแบบชำระเงิน |

เครื่องหมาย ✓ ใต้ RushDB Cloud หมายถึงแพ็กเกจ Free มีฟีเจอร์นั้น แพ็กเกจและโควตาดูได้ที่ [rushdb.com/pricing](https://rushdb.com/pricing)

## คำถามที่พบบ่อย

**พร้อมใช้งานจริงหรือยัง?** SQL, ภาษาคิวรี JSON ดั้งเดิม, Gremlin และ
เอกสารสไตล์ Mongo มีความเสถียรแล้ว ส่วน openCypher, ISO GQL และ Bolt อยู่ในสถานะ release
candidate การทำ replication เป็นแบบอะซิงโครนัสและต้องเลื่อนสถานะ (promote) ด้วยตนเอง
[หน้าสถานะ](https://docs.rushdb.com/engine/reference/status/) แสดงทุก
อินเทอร์เฟซพร้อมข้อจำกัด

**ต้องมีฐานข้อมูลเวกเตอร์แยกหรือไม่?** ไม่ต้อง เวกเตอร์ ดัชนีข้อความ และ
ความสัมพันธ์อยู่ในเอนจินเดียว และเขียนเรคคอร์ดพร้อม
เวกเตอร์ของมันได้ในคอมมิตเดียว เอนจินค้นหาบนเวกเตอร์ที่คุณส่งเข้ามา ส่วน RushDB
Cloud สร้างเวกเตอร์ให้คุณได้

**สคีมาที่เป็นปัจจุบันคืออะไร?** แคตตาล็อกของทุก label, property, ชนิดข้อมูล, ค่า
และรูปแบบความสัมพันธ์ ที่อัปเดตทุกคอมมิต พร้อมจำนวนที่แม่นยำ เรียกครั้งเดียว
ก็ได้ทั้งหมด เอเจนต์และเครื่องมือต่าง ๆ จึงไม่ต้องเดาชื่อฟิลด์
[แคตตาล็อกสคีมา](https://docs.rushdb.com/engine/data-model/schema-catalog/)

---

[การมีส่วนร่วม](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[ความปลอดภัย](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (เวอร์ชันเดิม ที่สร้างบน Neo4j) อยู่ใน
[branch `2.x`](https://github.com/rush-db/rushdb/tree/2.x) และได้รับการแก้ไขด้านความปลอดภัย
จนถึง 2027-10-28 [การอัปเกรดจาก 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
