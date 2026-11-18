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
หรือสไตล์ Mongo หรือ change stream จากฐานข้อมูลของคุณ RushDB จะเก็บเป็นเรคคอร์ดที่มีชนิดข้อมูลและเชื่อมโยงกันเป็นกราฟ
เรคคอร์ดชุดเดียวกันนี้ตอบได้ทั้งคิวรีความสัมพันธ์ การค้นหาแบบเวกเตอร์และฟูลเท็กซ์
และการรวมข้อมูล จึงไม่มีที่เก็บข้อมูลชุดที่สองที่ต้องคอยซิงก์ ทุกการเขียนยังอัปเดตแคตตาล็อกของ
label, ฟิลด์, ชนิดข้อมูล และค่าที่มีอยู่จริงด้วย เอเจนต์จึงคิวรีฟิลด์ที่มีอยู่จริง
เชื่อมต่อผ่าน HTTP, โปรโตคอล wire ของ PostgreSQL, Bolt, gRPC หรือ MCP จาก SDK หรือไคลเอนต์ PostgreSQL ใดก็ได้
ไบนารี Rust ตัวเดียว ภายใต้ Apache-2.0

| กราฟ | การค้นหาแบบเวกเตอร์และฟูลเท็กซ์ | การวิเคราะห์ข้อมูล |
|---|---|---|
| JSON ที่ซ้อนกันกลายเป็นเรคคอร์ดที่เชื่อมโยงกัน กรองข้ามความสัมพันธ์และไล่ตามความสัมพันธ์ที่มีชนิดและทิศทางได้ในคิวรีเดียว | การค้นหาแบบเวกเตอร์ (แบบตรงตัวและ HNSW) ฟูลเท็กซ์ BM25 และแบบไฮบริด กรองด้วยเงื่อนไขกราฟชุดเดียวกัน | นับ ผลรวม ค่าเฉลี่ย ค่าต่ำสุดและสูงสุดด้วย `groupBy` หรือใช้ SQL ธรรมดากับ `GROUP BY` ผ่านโปรโตคอล wire ของ PostgreSQL |

## เริ่มต้นใช้งานอย่างรวดเร็ว

```bash
curl -fsSL https://rushdb.com/install.sh | sh    # or: brew install rush-db/tap/rushdb
export PATH="$HOME/.rushdb/bin:$PATH"             # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` ให้บริการที่ `http://127.0.0.1:3000` โดยเก็บข้อมูลไว้ใน `./.rushdb` และใช้
โทเค็น `local-token` จากนั้นในเทอร์มินัลอื่น รัน `pip install rushdb`:

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

สคีมานี้มาจากแคตตาล็อก ไม่ได้มาจากการสแกนเรคคอร์ด:

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
```

<details>
<summary>ตัวอย่างเดียวกันใน TypeScript</summary>

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

ใช้ SQL เป็นอยู่แล้วใช่ไหม? label ก็คือตาราง:

```bash
PGPASSWORD=local-token psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

ขั้นต่อไป: [คู่มือเริ่มต้นใช้งานฉบับเต็ม](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker และวิธีติดตั้งอื่น ๆ](https://docs.rushdb.com/engine/getting-started/installation/)

## สิ่งที่คุณสร้างได้

| | |
|---|---|
| **หน่วยความจำของเอเจนต์** | เก็บความจำเป็นเรคคอร์ดที่มีความสัมพันธ์ กรองตามขอบเขต และจัดอันดับด้วยเวกเตอร์ที่คุณจัดเก็บ มี provider สำเร็จรูปสำหรับ OpenClaw และ Hermes |
| **การค้นหาข้อมูลในแอปของคุณ** | การค้นหาแบบฟูลเท็กซ์ เวกเตอร์ และไฮบริด บนเรคคอร์ดที่แอปของคุณเขียนอยู่แล้ว พร้อมตัวกรองกราฟในคิวรีเดียวกัน |
| **ตอบคำถามจากข้อมูลที่เชื่อมโยงกัน** | เอเจนต์อ่านสคีมา แล้วเขียนคิวรีที่คุณตรวจสอบได้ ทั้งในภาษาคิวรี JSON, SQL หรือ Cypher |
| **รายงานโดยไม่ต้องส่งออกข้อมูล** | จัดกลุ่มและรวมข้อมูลจากเรคคอร์ดที่แอปของคุณเขียน ผ่าน SDK หรือไคลเอนต์ PostgreSQL ใดก็ได้ |

## ฟีเจอร์

- **สคีมาที่เกิดขึ้นเอง** อนุมานชนิดข้อมูลทีละค่า อ็อบเจกต์ที่ซ้อนกันกลายเป็นเรคคอร์ดที่เชื่อมโยงกัน และแคตตาล็อกเก็บจำนวนที่แม่นยำพร้อมรายการค่า [แคตตาล็อกสคีมา](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **คิวรีกราฟ** กรองข้ามความสัมพันธ์ด้วย label จำกัดตามชนิดและทิศทาง และผูกเรคคอร์ดที่เกี่ยวข้องเพื่อใช้ในการ projection และการรวมข้อมูล [ความสัมพันธ์และการท่องกราฟ](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **การค้นหาในตัว** ดัชนีเวกเตอร์บนเวกเตอร์ที่คุณส่งเข้ามา ฟูลเท็กซ์ BM25 และการค้นหาแบบไฮบริดที่รวมทั้งสองแบบเข้าด้วยกัน [ดัชนีและการค้นหา](https://docs.rushdb.com/engine/indexes-and-search/)
- **การรวมข้อมูล** `select` และ `groupBy` พร้อมการนับ ผลรวม ค่าเฉลี่ย การแบ่งช่วงเวลา และคอลเลกชันแบบซ้อน [การรวมข้อมูล](https://docs.rushdb.com/engine/query/aggregations/)
- **รองรับสิ่งที่คุณใช้อยู่** SQL ผ่านโปรโตคอล wire ของ PostgreSQL, Gremlin, SQL/PGQ, เอกสารสไตล์ Mongo รวมถึง openCypher และ ISO GQL ในสถานะ release candidate [ภาษาคิวรี](https://docs.rushdb.com/engine/languages/)
- **ทนทาน** มี write-ahead log, ทรานแซกชัน, การสำรองข้อมูลพร้อมกู้คืนไปยังจุดเวลาที่ต้องการ และ read replica [การดูแลระบบ](https://docs.rushdb.com/engine/operations/)
- **ไบนารีเดียว** HTTP, PostgreSQL wire, gRPC และ Bolt จากโปรเซสเดียว พร้อม tenant แยกกันแบบเลือกใช้ได้ [การเชื่อมต่อ](https://docs.rushdb.com/engine/connectivity/)

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

## รันได้ทุกที่

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

Docker Compose, Kubernetes (Helm), cloud-init, systemd, Fly.io, Render,
Coolify และ Dokploy: ดู [Deploy](https://docs.rushdb.com/deploy/) อ่าน
[ความปลอดภัย](https://docs.rushdb.com/engine/operations/security/) ก่อน
เปิดโหนดให้เข้าถึงได้นอกเหนือจาก loopback

## โฮสต์เองและ RushDB Cloud

- **โฮสต์เอง:** รีโพสิทอรีนี้ ทุกอย่างข้างต้นทำงานในเอนจินที่คุณติดตั้งเอง
- **[RushDB Cloud](https://app.rushdb.com):** เอนจินตัวเดียวกันแบบมีผู้ดูแลให้ พร้อม
  แดชบอร์ด, embedding ที่จัดการให้, การค้นหาด้วยภาษาธรรมชาติ และคอนเนกเตอร์สำหรับ
  เครื่องมือ SaaS ทั้ง SDK และเซิร์ฟเวอร์ MCP ใช้งานได้กับทั้งสองแบบ

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

**Windows?** ใช้ WSL 2 หรือ Docker

---

[การมีส่วนร่วม](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[ความปลอดภัย](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (เวอร์ชันเดิม ที่สร้างบน Neo4j) อยู่ใน
[branch `2.x`](https://github.com/rush-db/rushdb/tree/2.x) และได้รับการแก้ไขด้านความปลอดภัย
จนถึง 2027-10-28 [การอัปเกรดจาก 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
