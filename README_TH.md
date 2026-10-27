<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### ฐานข้อมูลเดียวสำหรับเรคคอร์ด ความสัมพันธ์ของมัน และการค้นหาบนข้อมูลเหล่านั้น

เขียน JSON แล้ว RushDB จะอนุมานชนิดข้อมูล เชื่อมออบเจกต์ที่ซ้อนกันให้เป็นกราฟ และให้คุณคิวรีด้วยภาษาคิวรี JSON ของมันเอง, SQL, Cypher, Gremlin หรือเอกสารแบบ Mongo พร้อมการค้นหาแบบเวกเตอร์ ข้อความเต็ม และแบบไฮบริด ไบนารี Rust ตัวเดียว ไดเรกทอรีข้อมูลเดียว ใช้สัญญาอนุญาต Apache-2.0

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[เว็บไซต์](https://rushdb.com) • [เอกสาร](https://docs.rushdb.com) • [คู่มือ](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [การดีพลอย](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • [Bahasa Indonesia](README_ID.md) • **ไทย**

</div>

---

## เริ่มต้นอย่างรวดเร็ว

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

สคริปต์จะตรวจ checksum ของรีลีส (และลายเซ็น Sigstore หากติดตั้ง `cosign` ไว้) แล้วติดตั้งไบนารี `rushdb` ไว้ที่ `~/.rushdb/bin` คำสั่ง `rushdb up` จะเริ่มโหนดแบบถาวรที่ `http://127.0.0.1:3000` เก็บข้อมูลไว้ที่ `./.rushdb` และใช้โทเค็นภายในเครื่อง `local-token`

เขียนเอกสาร JSON แบบซ้อนกัน ออบเจกต์ที่ซ้อนอยู่แต่ละตัวจะกลายเป็นเรคคอร์ดของตัวเอง และเชื่อมกับเรคคอร์ดแม่:

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

จากนั้นค้นหาวิศวกรของ Acme ตัวกรองจะข้ามความสัมพันธ์ได้ด้วยการระบุชื่อเลเบลที่เกี่ยวข้อง:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

อ่านต่อที่ [Quickstart](docs/manual/getting-started/quickstart.md)

## คิวรีในแบบที่คุณถนัด

เรคคอร์ดชุดเดียวกันตอบได้จากทุกอินเทอร์เฟซ

**SQL** ผ่านโปรโตคอล wire ของ PostgreSQL ด้วย `psql` หรือไดรเวอร์ PostgreSQL ใดก็ได้ เลเบลคือตาราง:

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher** ผ่าน HTTP หรือผ่าน Bolt ด้วยไดรเวอร์ของ Neo4j (`rushdb up --languages sql,cypher --bolt-port 7687`):

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**SDK** ในโหมด direct (`npm install @rushdb/javascript-sdk`):

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, เอกสารแบบ Mongo และ ISO GQL ใช้ HTTP endpoint เดียวกัน ดู [ภาษาคิวรี](docs/manual/languages/README.md)

## มีอะไรให้บ้าง

| ความสามารถ | ความหมาย |
|---|---|
| **ไม่ต้องออกแบบสคีมา** | เขียน JSON รูปแบบใดก็ได้ ชนิดข้อมูลถูกอนุมานตามค่า ออบเจกต์ที่ซ้อนกันกลายเป็นเรคคอร์ดที่เชื่อมกัน และเอนจินเก็บแคตตาล็อกของเลเบล พร็อพเพอร์ตี และความสัมพันธ์ไว้อย่างแม่นยำและคิวรีได้ |
| **พร็อพเพอร์ตีเป็นองค์ประกอบหลัก** | Labeled Meta-Property Graph (LMPG) เก็บพร็อพเพอร์ตีแต่ละตัวเป็นนิยามที่มีชนิดข้อมูล ซึ่งเรคคอร์ดที่ใช้งานร่วมกันใช้ ทำให้การค้นพบสคีมา ช่วงค่า และ facet ได้มาจากโครงสร้างการจัดเก็บโดยตรง |
| **หลายภาษาคิวรี** | ภาษาคิวรี JSON แบบเนทีฟ, SQL, Gremlin, SQL/PGQ, เอกสารแบบ Mongo และ openCypher กับ ISO GQL ในสถานะ release candidate |
| **การค้นหา** | ดัชนีเวกเตอร์ (แบบแม่นยำและ HNSW) บนเวกเตอร์ที่คุณให้มา ดัชนีข้อความเต็ม BM25 และการค้นหาแบบไฮบริดที่รวมทั้งสองแบบ |
| **ทนทานและรองรับทรานแซกชัน** | Write-ahead log, สแนปช็อต MVCC, ทรานแซกชันหลายคำสั่ง, การสำรองข้อมูลพร้อมการกู้คืนไปยังจุดเวลา และการทำสำเนาแบบสตรีมไปยัง follower แบบอ่านอย่างเดียว |
| **โปรโตคอลมาตรฐาน** | HTTP, โปรโตคอล wire ของ PostgreSQL, gRPC และ Bolt จากโปรเซสเดียว |
| **หลายผู้เช่า** | ผู้เช่าแบบ fail-closed ที่เลือกใช้ได้ แต่ละรายมี keyspace ของตัวเอง |
| **ไบนารีตัวเดียว** | `rushdb serve` รันเซิร์ฟเวอร์ `rushdb up` รันโหนดในเครื่อง ไม่มีบริการอื่นให้ดูแล |

## การดีพลอย

| ที่ไหน | อย่างไร |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/) พร้อมรุ่น HTTPS ที่อยู่หลัง Caddy |
| Kubernetes | [Helm chart](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| Linux VM | [cloud-init](deploy/templates/cloud-init/) สำหรับ DigitalOcean, Hetzner, AWS, GCP, Azure; [systemd unit](deploy/templates/systemd/) |
| แพลตฟอร์ม | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### วิธีติดตั้งอื่น ๆ

| วิธี | คำสั่ง | หมายเหตุ |
|---|---|---|
| สคริปต์ติดตั้ง | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) และ macOS, x86-64 และ arm64 ใช้ `sh -s -- --version v3.0.0` เพื่อระบุรีลีส |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS และ Linux `brew services start rushdb` รันเบื้องหลัง |
| Docker | `rushdb/rushdb` | `linux/amd64` และ `linux/arm64` และที่ `ghcr.io/rush-db/rushdb` |
| Cargo | `cargo binstall rushdb-server` หรือ `cargo install rushdb-server --locked` | `binstall` ดาวน์โหลดไบนารีของรีลีส ส่วน `install` คอมไพล์เอง (ต้องมีคอมไพเลอร์ C++) |
| ไฟล์รีลีส | [GitHub releases](https://github.com/rush-db/rushdb/releases) | ไฟล์บีบอัดพร้อม checksum SHA-256, SBOM และลายเซ็น Sigstore |
| ซอร์สโค้ด | `cargo build --release -p rushdb-server --bin rushdb` | ดู [การติดตั้ง](docs/manual/getting-started/installation.md) |

ไม่รองรับ Windows แบบเนทีฟ ให้ใช้ WSL 2 หรือ Docker อ่าน [ความปลอดภัย](docs/manual/operations/security.md) ก่อนเปิดโหนดให้เข้าถึงจากนอก loopback

## สถานะ

| ส่วน | สถานะ |
|---|---|
| โปรโตคอลเนทีฟ, SQL, การท่องกราฟด้วยข้อความ Gremlin, เอกสารแบบ Mongo | เสถียร |
| SQL/PGQ `GRAPH_TABLE` | เสถียร อ่านอย่างเดียว |
| openCypher, ISO GQL, Bolt | Release candidate ต้องเปิดใช้เอง: ชุดทดสอบความสอดคล้องภายนอกยังผ่านไม่ครบ ([รายละเอียด](docs/manual/languages/README.md)) |
| PostgreSQL wire | การอ่านด้วย SQL และการเขียนแบบคำสั่งเดียว ตรวจสอบกับ `psql`, node-postgres, psycopg และ `tokio-postgres` แต่ไม่ใช่ทุกไคลเอนต์หรือ ORM |
| การทำสำเนา | สตรีมแบบอะซิงโครนัสไปยัง follower แบบอ่านอย่างเดียว การเลื่อนสถานะทำด้วยมือ ไม่มี failover อัตโนมัติ |
| Raft, การแบ่ง shard ของที่เก็บข้อมูล | ตัวอย่างทดลอง ไม่ใช่สำหรับโปรดักชัน: Raft ทำงานในโปรเซสเดียวโดยเก็บสถานะในหน่วยความจำ และคอมมิตข้าม shard ไม่เป็นอะตอมมิกเมื่อเกิดการล่ม |

[คู่มือ](docs/manual/README.md) ระบุข้อจำกัดที่แน่นอนของแต่ละอินเทอร์เฟซ

## การเชื่อมต่อ

| อินเทอร์เฟซ | ค่าเริ่มต้น | การเปิดใช้ |
|---|---|---|
| โปรโตคอล HTTP เนทีฟ | `127.0.0.1:3000`, `POST /protocol/v1/execute` | เปิดเสมอ |
| PostgreSQL wire | `127.0.0.1:5432` | เปิดเมื่อใช้ `sql` (ค่าเริ่มต้น) |
| gRPC (`rushdb.v1.Database/Execute`) | ปิด | `--grpc-port` |
| Bolt 5.0 | ปิด | `--bolt-port` พร้อม `cypher` |

SDK สำหรับ [JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) และ [Python](https://pypi.org/project/rushdb/), [เซิร์ฟเวอร์ MCP](https://www.npmjs.com/package/@rushdb/mcp-server) และ [agent skills](https://github.com/rush-db/skills) ทำงานกับเอนจินที่โฮสต์เองได้ในโหมด direct

## เอนจินที่โฮสต์เองและ RushDB Cloud

เอนจินนี้คือส่วนของ RushDB ที่คุณโฮสต์เองได้ มันไม่สร้าง embedding ไม่มีแดชบอร์ด และไม่จัดการผู้ใช้ สิ่งเหล่านี้มาพร้อมกับ [RushDB Cloud](https://app.rushdb.com) บริการแบบจัดการที่รันเอนจินนี้และเพิ่มแดชบอร์ด embedding แบบจัดการ และฟีเจอร์ AI (การค้นหาด้วยภาษาธรรมชาติ, คำแนะนำรูปแบบความสัมพันธ์) SDK และเซิร์ฟเวอร์ MCP ใช้ได้กับทั้งสองแบบ: ให้ API key ของ RushDB Cloud หรือชี้ไปที่เอนจินของคุณโดยตรง (`direct: true`, `engineUrl`) ซึ่งใช้ได้ทุกอย่างยกเว้นฟีเจอร์ของ Cloud ที่จะถูกปฏิเสธด้วย `CloudOnlyError`

## เอกสาร

- [คู่มือ](docs/manual/README.md): การเริ่มต้น, โมเดลข้อมูล, คิวรี, ภาษา, การค้นหา, การดูแลระบบ และตารางอ้างอิง
- [docs.rushdb.com](https://docs.rushdb.com): บทเรียน, SDK และเซิร์ฟเวอร์ MCP
- [การอัปเกรดจาก RushDB 2.x](docs/manual/operations/upgrading-from-2x.md): 2.x คือสแตกที่ใช้ Neo4j ซึ่งเก็บไว้ที่แท็ก `v2` ของรีโพนี้

## การมีส่วนร่วม

ดู [CONTRIBUTING.md](CONTRIBUTING.md) เกณฑ์คุณภาพ:

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [นโยบายความปลอดภัย](SECURITY.md)
- [ขั้นตอนการรีลีส](RELEASING.md)
- [จรรยาบรรณ](CODE_OF_CONDUCT.md)
- [บันทึกการเปลี่ยนแปลง](CHANGELOG.md)

เผยแพร่ภายใต้สัญญาอนุญาต [Apache-2.0](LICENSE)
