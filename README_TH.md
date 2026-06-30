<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### เลเยอร์หน่วยความจำสำหรับ AI agent และแอปพลิเคชัน

ส่ง JSON อะไรก็ได้ Agent ของคุณจะได้ schema ที่ใช้งานได้แบบเรียลไทม์และค้นหาได้ ความสัมพันธ์แบบกราฟ และการค้นหาเชิงความหมาย (semantic search) ทั้งหมดถูกอนุมานโดยอัตโนมัติ
ไม่ต้องมี pipeline ไม่ต้องมีที่จัดเก็บแยกต่างหาก ไม่ต้องออกแบบ schema

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 เว็บไซต์](https://rushdb.com) • [📖 เอกสารประกอบ](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 ตัวอย่าง](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • **ไทย**

</div>

---

## ปัญหา

Agent ของคุณต้องการหน่วยความจำ คำตอบมาตรฐานคือต้องใช้ฐานข้อมูลสามตัว ได้แก่ Redis สำหรับ key-value, vector store สำหรับการค้นหาเชิงความหมาย และฐานข้อมูลกราฟสำหรับความสัมพันธ์ บวกกับโค้ดเชื่อมต่อ (glue code) เพื่อให้ทั้งหมดซิงค์กัน

RushDB แทนที่ทั้งสามตัวนี้ ส่ง JSON เพียงครั้งเดียว แล้วค้นหาด้วยการเดินทางในกราฟ (graph traversal) การค้นหาเชิงความหมาย หรือทั้งสองอย่างในการเรียกครั้งเดียว

| ถ้าไม่มี RushDB                          | ถ้ามี RushDB                                |
| ---------------------------------------- | ------------------------------------------- |
| Redis + Pinecone + Neo4j + โค้ดเชื่อมต่อ | API เดียว                                   |
| ออกแบบ schema → เขียน migration → ทำซ้ำ  | ส่งข้อมูลรูปแบบใดก็ได้ ไม่ต้องมี schema     |
| pipeline สำหรับ embedding แยกต่างหาก     | embedding แบบจัดการให้ ทำงานฝั่งเซิร์ฟเวอร์ |
| สร้าง edge ความสัมพันธ์ด้วยมือ           | ตรวจจับอัตโนมัติจากโครงสร้างข้อมูลของคุณ    |

---

## เริ่มต้นใช้งานอย่างรวดเร็ว

มีสองเส้นทางขึ้นอยู่กับการตั้งค่าของคุณ:

- **Cloud** — แบบจัดการให้ มีแพ็กเกจฟรี เริ่มใช้งานได้ใน 30 วินาที [รับ API key →](https://app.rushdb.com)
- **Self-host** — Docker + อินสแตนซ์ Neo4j ของคุณเอง [ข้ามไปที่ Self-hosting →](#self-hosting)

### เส้นทาง Cloud

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### จัดเก็บและเรียกคืนหน่วยความจำของ agent

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
const memories = await db.ai.search({
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

## การทำงานกับข้อมูลของคุณ

### นำเข้า JSON แบบซ้อน (nested JSON)

ส่ง JSON รูปแบบใดก็ได้ อ็อบเจกต์ที่ซ้อนกันและอาร์เรย์ของอ็อบเจกต์จะกลายเป็น record ที่เชื่อมโยงกัน โดย label, type และความสัมพันธ์จะถูกอนุมานในขณะเขียน ไม่ต้องมี schema ไม่ต้องมีขั้นตอน migration

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

แต่ละ key ที่ซ้อนกัน (`DEPARTMENT`, `EMPLOYEE`) จะกลายเป็น label, แต่ละอ็อบเจกต์กลายเป็น record และการบรรจุอยู่ภายใน (containment) กลายเป็นความสัมพันธ์ ทั้งหมดถูกสร้างขึ้นโดยอัตโนมัติ

### นำเข้า CSV

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

`suggestTypes` อนุมานตัวเลข ค่าบูลีน และวันที่จากสตริง; `skipEmptyValues` ถือว่าเซลล์ว่างเป็นค่าที่ไม่ได้ตั้งค่า แทนที่จะจัดเก็บค่าว่าง (ค่า `0` และ `false` จะถูกเก็บไว้)

### เดินทางในกราฟ (Traverse the graph)

กรอง record ระดับราก (root records) ด้วยเงื่อนไขบน record ที่ _เกี่ยวข้อง_ กับมัน ลึกได้ตามอำเภอใจ ในคิวรีเดียว label ที่เกี่ยวข้องจะอยู่ **ภายใน** `where` ไม่ใช่ใน `labels`:

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

### คิวรีเชิงวิเคราะห์ (aggregate และ group by)

`select` กำหนดรูปร่างผลลัพธ์ด้วยการรวม (`$sum`, `$avg`, `$count`, `$min`, `$max`); `groupBy` ควบคุมมิติ (dimensions) อย่าใส่ `limit` ในการรวม (aggregation) เพราะมันจะสแกนเฉพาะ N record แรกและทำให้ยอดรวมคลาดเคลื่อน

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

การรวมสามารถประกอบเข้ากับการเดินทางในกราฟได้ เช่น จำนวนพนักงานและค่าจ้างต่อแผนก:

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

## เชื่อมต่อกับ Claude, Cursor หรือ MCP client ใดก็ได้

RushDB มาพร้อมกับ MCP server Agent ของคุณจะได้หน่วยความจำที่คงอยู่ถาวรและมีโครงสร้าง พร้อมใช้งานทันที

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

วางสิ่งนี้ในการตั้งค่า MCP ของ Claude Desktop, Cursor หรือ Windsurf จากนั้น agent จะสามารถสร้าง record, ค้นหาตามความหมาย, เดินทางในความสัมพันธ์ และตรวจสอบ schema ได้ ทั้งหมดผ่านภาษาธรรมชาติ

---

## สิ่งที่มีอยู่ในกล่อง

| ความสามารถ                      | ความหมาย                                                                                                                                                           |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Embedding แบบจัดการให้**      | ทำดัชนีคุณสมบัติประเภทสตริงใดก็ได้เพียงครั้งเดียว ทุกการเขียนจะถูก embed โดยอัตโนมัติฝั่งเซิร์ฟเวอร์                                                               |
| **กราฟ + เวกเตอร์ในคิวรีเดียว** | ความคล้ายคลึงเชิงความหมายและการเดินทางในความสัมพันธ์ประกอบกันในการเรียกครั้งเดียว                                                                                  |
| **ไม่ต้องมี schema**            | ส่ง JSON รูปแบบใดก็ได้ RushDB อนุมาน type, สร้างคุณสมบัติ, เชื่อมโยง record                                                                                        |
| **Schema ที่ถูกอนุมาน**         | คุณสมบัติกลายเป็นโหนดชั้นหนึ่ง (first-class nodes) — type, label และความสัมพันธ์ถูกค้นพบในขณะเขียน ดังนั้น schema ที่ค้นหาได้จึงสร้างตัวเองขึ้นมาเมื่อข้อมูลเข้ามา |
| **ACID transactions**           | agent ที่ทำงานพร้อมกันจะไม่ทำให้หน่วยความจำที่ใช้ร่วมกันเสียหาย ใช้ Neo4j อยู่เบื้องหลัง                                                                           |
| **อธิบายตัวเองได้**             | agent ตรวจสอบ schema ที่ถูกอนุมาน — label, คุณสมบัติ, ช่วงค่า — เพื่อให้รู้ว่าสิ่งใดที่ค้นหาได้อย่างปลอดภัย                                                        |
| **MCP-native**                  | MCP server แบบเต็มที่มี query prompt แบบค้นพบก่อน (discovery-first) ในตัว                                                                                          |
| **Agent Skills**                | แพ็กเกจ `@rushdb/skills` ที่ติดตั้งได้ — สอน agent ที่รองรับ skills ใดก็ได้ให้ค้นหา, สร้างโมเดล และจดจำด้วย RushDB ในคำสั่งเดียว                                   |
| **Unified query API**           | รูปแบบ JSON เดียวสำหรับกราฟ, เวกเตอร์, การรวม และการตรวจสอบ                                                                                                        |
| **Self-host หรือ cloud**        | Docker + Neo4j ของคุณ หรือ cloud แบบจัดการให้ เป็นเจ้าของข้อมูลทั้งหมด                                                                                             |

---

## กรณีการใช้งาน

| กรณีการใช้งาน                      | RushDB แทนที่อะไร                    | API หลัก                                                        |
| ---------------------------------- | ------------------------------------ | --------------------------------------------------------------- |
| **หน่วยความจำของ agent**           | Redis + vector store + ฐานข้อมูลกราฟ | `db.ai.search({ query, where: { agent_id } })`                  |
| **RAG พร้อมบริบท**                 | vector store แบบแบน                  | `db.records.find({ where, labels })` + การเดินทางในความสัมพันธ์ |
| **แอปที่ไม่ต้องมี schema**         | Postgres + migration + ETL           | `db.records.importJson(nestedJson)`                             |
| **ผลิตภัณฑ์ข้อมูลที่เชื่อมโยงกัน** | บริการที่ join กันหลายตัว            | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })`   |

---

## Self-hosting

> **เส้นทาง Self-host** — รัน RushDB บนโครงสร้างพื้นฐานของคุณเอง ต้องใช้ Neo4j 2026.01.4+ พร้อมปลั๊กอิน APOC

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
<summary>ตัวแปรสภาพแวดล้อมทั้งหมด</summary>

| ชื่อ                              | คำอธิบาย                                       | จำเป็น     | ค่าเริ่มต้น |
| --------------------------------- | ---------------------------------------------- | ---------- | ----------- |
| `NEO4J_URL`                       | URL การเชื่อมต่อ Neo4j                         | ใช่        | —           |
| `NEO4J_USERNAME`                  | ชื่อผู้ใช้ Neo4j                               | ใช่        | neo4j       |
| `NEO4J_PASSWORD`                  | รหัสผ่าน Neo4j                                 | ใช่        | —           |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | คีย์ 32 อักขระสำหรับเข้ารหัส API token         | ใช่ (prod) | —           |
| `RUSHDB_PORT`                     | พอร์ต HTTP                                     | ไม่        | 3000        |
| `RUSHDB_LOGIN`                    | ล็อกอินของผู้ดูแลระบบ                          | ไม่        | admin       |
| `RUSHDB_PASSWORD`                 | รหัสผ่านของผู้ดูแลระบบ                         | ไม่        | password    |
| `RUSHDB_BASE_URL`                 | URL API สาธารณะ/ฐานสำหรับการมอบหมายงาน synx    | ไม่        | —           |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | token ภายในสำหรับ synx worker แบบจัดการให้     | ไม่        | —           |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | คีย์การเขียนภายในสำหรับการเขียนปลายทางของ synx | ไม่        | —           |

synx worker แบบจัดการให้ทำงานเป็น daemon มันจะคอยตรวจสอบ connector ที่รันได้, ต่ออายุการเช่า connector (connector leases), ปล่อยการเช่าเมื่อปิดระบบอย่างนุ่มนวล และให้ platform/core เรียกคืนการเช่าที่หมดอายุหลังจากเกิด crash

</details>

<details>
<summary>การพัฒนาในเครื่อง (Neo4j แบบมาพร้อมกัน)</summary>

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
<summary>สถาปัตยกรรม: RushDB จัดโครงสร้างข้อมูลอย่างไร (LMPG)</summary>

RushDB ใช้โมเดล **Labeled Meta Property Graph (LMPG)** คุณสมบัติถูกยกระดับให้เป็นโหนดกราฟชั้นหนึ่ง ("HyperProperties") ไม่ใช่แค่คู่ key-value ที่ผูกกับ record

นี่หมายความว่า:

- **Schema โดยไม่ต้องออกแบบล่วงหน้า** — เนื่องจากคุณสมบัติเป็นโหนดกราฟ schema จึง _ถูกอนุมานจากข้อมูลของคุณ ไม่ใช่ถูกออกแบบ_ : label, type, ช่วงค่า และโทโพโลยีของความสัมพันธ์ถูกค้นพบในขณะเขียนและค้นหาได้ทันที ไม่ต้องสร้างโมเดล schema ด้วยมือ ไม่ต้องใช้ toolchain RDF/OWL
- **ความสัมพันธ์ที่ตรวจจับอัตโนมัติ** — record ที่มีคุณสมบัติร่วมกันจะถูกเชื่อมโยงโดยไม่ต้องสร้าง edge ด้วยมือ
- **การตรวจสอบ schema** — agent สามารถระบุรายการ label, type ของคุณสมบัติ, ช่วงค่า และโทโพโลยีของความสัมพันธ์ในคิวรีเดียว
- **ข้อจำกัดแบบนุ่มนวล (soft constraints)** — การให้คะแนนความสอดคล้องของ type, การติดตาม cardinality และการบังคับใช้มิติเวกเตอร์ โดยไม่ต้องมี schema ที่เข้มงวดล่วงหน้า
- **พื้นผิวคิวรีแบบรวม** — นิพจน์กรองเดียวกันทำงานได้ทั้งกับ record, label, คุณสมบัติ และความสัมพันธ์

SearchQuery เดียวดึงมุมมองหลายมุมพร้อมกัน (record + สถิติคุณสมบัติ + การรวม) หลีกเลี่ยงรูปแบบการตรวจสอบแบบ N+1 ที่พบได้ทั่วไปในสถาปัตยกรรมแบบแยกระบบ

[อ่านโพสต์สถาปัตยกรรม LMPG ฉบับเต็ม →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## เอกสารประกอบ

| หัวข้อ                          | ลิงก์                                                          |
| ------------------------------- | -------------------------------------------------------------- |
| บทเรียนแบบรวดเร็ว               | https://docs.rushdb.com/get-started/quick-tutorial             |
| การค้นหาเวกเตอร์ / เชิงความหมาย | https://docs.rushdb.com/concepts/search/where#vector-operators |
| การกรองและการเดินทางในกราฟ      | https://docs.rushdb.com/concepts/search/where                  |
| การจัดกลุ่มและการรวม            | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK                  | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK                      | https://docs.rushdb.com/python-sdk/introduction                |
| REST API                        | https://docs.rushdb.com/rest-api/introduction                  |
| MCP Server                      | packages/mcp-server/README.md                                  |
| Agent Skills                    | packages/skills/README.md                                      |

---

## เมื่อใดที่ไม่ควรใช้ RushDB

- คุณต้องการ latency ระดับต่ำกว่ามิลลิวินาทีที่ write throughput สูงมาก — RushDB สร้างขึ้นบน Neo4j ซึ่งให้ความสำคัญกับความสอดคล้อง (consistency) และความสามารถในการแสดงออกของคิวรี (query expressiveness) มากกว่าความเร็วในการเขียนดิบ
- คุณต้องการแค่ที่จัดเก็บ key-value แบบแบนโดยไม่มีความสัมพันธ์หรือการค้นหาเชิงความหมาย — ที่จัดเก็บที่เรียบง่ายกว่าจะเบากว่า
- คุณต้องการ schema เชิงสัมพันธ์ที่เข้มงวดซึ่งบังคับใช้ในระดับฐานข้อมูล — RushDB ถูกออกแบบให้ไม่มี schema โดยเจตนา

---

## การมีส่วนร่วม (Contributing)

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

ดู [CONTRIBUTING.md](CONTRIBUTING.md) สำหรับแนวทางฉบับเต็ม ยินดีต้อนรับ Issue และ PR

---

## สัญญาอนุญาต (License)

| เส้นทาง                 | สัญญาอนุญาต         |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

ต้องการสิ่งที่ยังไม่รองรับใช่ไหม? เปิด issue ได้เลย — ยินดีต้อนรับการพูดคุยเรื่องการออกแบบ
