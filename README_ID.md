<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### Lapisan memori untuk agen dan aplikasi AI.

Push JSON apa pun. Agen Anda mendapatkan skema langsung yang dapat di-query, relasi graph, dan pencarian semantik — disimpulkan secara otomatis.
Tanpa pipeline. Tanpa penyimpanan terpisah. Tanpa skema yang perlu dirancang.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 Situs Web](https://rushdb.com) • [📖 Dokumentasi](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 Contoh](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • **Bahasa Indonesia** • [ไทย](README_TH.md)

</div>

---

## Masalahnya

Agen Anda membutuhkan memori. Jawaban standarnya adalah tiga basis data: Redis untuk key-value, vector store untuk pencarian semantik, dan graph DB untuk relasi — ditambah kode perekat untuk menjaga semuanya tetap sinkron.

RushDB menggantikan ketiganya. Push JSON sekali. Query dengan penelusuran graph, pencarian semantik, atau keduanya — dalam satu panggilan.

| Tanpa RushDB                            | Dengan RushDB                               |
| --------------------------------------- | ------------------------------------------- |
| Redis + Pinecone + Neo4j + kode perekat | Satu API                                    |
| Rancang skema → tulis migrasi → ulangi  | Push bentuk apa pun, tanpa skema            |
| Pipeline embedding terpisah             | Embedding terkelola, di sisi server         |
| Membuat edge relasi secara manual       | Terdeteksi otomatis dari struktur data Anda |

---

## Mulai cepat

Dua jalur tergantung pengaturan Anda:

- **Cloud** — Terkelola, tier gratis, berjalan dalam 30 detik. [Dapatkan API key →](https://app.rushdb.com)
- **Self-host** — Docker + instance Neo4j Anda sendiri. [Lompat ke Self-hosting →](#self-hosting)

### Jalur Cloud

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

### Simpan dan ingat kembali memori agen

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

## Bekerja dengan data Anda

### Impor JSON bersarang

Push bentuk JSON apa pun. Objek bersarang dan array objek menjadi record yang saling terhubung — label, tipe, dan relasi disimpulkan saat penulisan. Tanpa skema, tanpa langkah migrasi.

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

Setiap key bersarang (`DEPARTMENT`, `EMPLOYEE`) menjadi sebuah label, setiap objek menjadi sebuah record, dan penyimpanan bersarang menjadi sebuah relasi — semuanya dibuat secara otomatis.

### Impor CSV

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

`suggestTypes` menyimpulkan angka, boolean, dan tanggal dari string; `skipEmptyValues` memperlakukan sel kosong sebagai tidak diatur alih-alih menyimpan nilai kosong (`0` dan `false` tetap dipertahankan).

### Telusuri graph

Filter record akar berdasarkan kondisi pada record _terkait_ mereka — sedalam apa pun — dalam satu query. Label terkait diletakkan **di dalam** `where`, bukan di `labels`:

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

### Query analitis (agregasi & group by)

`select` membentuk output dengan agregasi (`$sum`, `$avg`, `$count`, `$min`, `$max`); `groupBy` mengontrol dimensinya. Jangan tambahkan `limit` ke sebuah agregasi — ini hanya akan memindai N record pertama dan membuat total menjadi keliru.

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

Agregasi dapat dikombinasikan dengan penelusuran — misalnya jumlah karyawan dan penggajian per departemen:

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

## Hubungkan ke Claude, Cursor, atau klien MCP apa pun

RushDB menyertakan server MCP. Agen Anda mendapatkan memori yang persisten dan terstruktur — langsung dari kotaknya.

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

Tempatkan ini di konfigurasi MCP Claude Desktop, Cursor, atau Windsurf Anda. Agen kini dapat membuat record, mencari berdasarkan makna, menelusuri relasi, dan melakukan introspeksi skema — semuanya melalui bahasa alami.

---

## Apa yang ada di dalam kotak

| Kemampuan                           | Apa artinya                                                                                                                                                           |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Embedding terkelola**             | Indeks properti string apa pun sekali — setiap penulisan otomatis di-embed di sisi server                                                                             |
| **Graph + vektor dalam satu query** | Kemiripan semantik dan penelusuran relasi digabungkan dalam satu panggilan                                                                                            |
| **Tanpa skema**                     | Push bentuk JSON apa pun. RushDB menyimpulkan tipe, membuat properti, menghubungkan record                                                                            |
| **Skema yang disimpulkan**          | Properti menjadi node kelas satu — tipe, label, dan relasi ditemukan saat penulisan, sehingga skema yang dapat di-query terbangun sendiri saat data tiba              |
| **Transaksi ACID**                  | Agen yang berjalan bersamaan tidak merusak memori bersama. Neo4j di balik layar                                                                                       |
| **Mendeskripsikan diri**            | Agen melakukan introspeksi skema yang disimpulkan — label, properti, rentang nilai — untuk mengetahui apa yang dapat mereka query dengan aman                         |
| **MCP-native**                      | Server MCP lengkap dengan prompt query discovery-first yang sudah terpasang                                                                                           |
| **Agent Skills**                    | Paket `@rushdb/skills` yang dapat diinstal — ajarkan agen yang kompatibel dengan skills untuk meng-query, memodelkan, dan mengingat dengan RushDB dalam satu perintah |
| **API query terpadu**               | Satu bentuk JSON untuk graph, vektor, agregasi, dan introspeksi                                                                                                       |
| **Self-host atau cloud**            | Docker + Neo4j Anda, atau cloud terkelola. Kepemilikan data penuh                                                                                                     |

---

## Kasus penggunaan

| Kasus penggunaan          | Apa yang digantikan RushDB      | API utama                                                     |
| ------------------------- | ------------------------------- | ------------------------------------------------------------- |
| **Memori agen**           | Redis + vector store + graph DB | `db.records.vectorSearch({ query, where: { agent_id } })`     |
| **RAG dengan konteks**    | Vector store datar              | `db.records.find({ where, labels })` + penelusuran relasi     |
| **Aplikasi tanpa skema**  | Postgres + migrasi + ETL        | `db.records.importJson(nestedJson)`                           |
| **Produk data terhubung** | Banyak layanan yang di-join     | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })` |

---

## Self-hosting

> **Jalur self-host** — jalankan RushDB pada infrastruktur Anda sendiri. Membutuhkan Neo4j 2026.01.4+ dengan plugin APOC.

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
<summary>Variabel lingkungan lengkap</summary>

| Nama                              | Deskripsi                                        | Wajib     | Default  |
| --------------------------------- | ------------------------------------------------ | --------- | -------- |
| `NEO4J_URL`                       | URL koneksi Neo4j                                | ya        | —        |
| `NEO4J_USERNAME`                  | Nama pengguna Neo4j                              | ya        | neo4j    |
| `NEO4J_PASSWORD`                  | Kata sandi Neo4j                                 | ya        | —        |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | Kunci 32 karakter untuk enkripsi token API       | ya (prod) | —        |
| `RUSHDB_PORT`                     | Port HTTP                                        | tidak     | 3000     |
| `RUSHDB_LOGIN`                    | Login admin                                      | tidak     | admin    |
| `RUSHDB_PASSWORD`                 | Kata sandi admin                                 | tidak     | password |
| `RUSHDB_BASE_URL`                 | URL API publik/dasar untuk penugasan synx        | tidak     | —        |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | Token internal untuk worker synx terkelola       | tidak     | —        |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | Kunci tulis internal untuk penulisan tujuan synx | tidak     | —        |

Worker synx terkelola berjalan sebagai daemon. Mereka melakukan polling untuk konektor yang dapat dijalankan, memperbarui lease konektor, melepaskan lease saat shutdown yang anggun, dan membiarkan platform/core merebut kembali lease yang kedaluwarsa setelah crash.

</details>

<details>
<summary>Pengembangan lokal (Neo4j terpaket)</summary>

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
<summary>Arsitektur: bagaimana RushDB menstrukturkan data (LMPG)</summary>

RushDB menggunakan model **Labeled Meta Property Graph (LMPG)**. Properti diangkat menjadi node graph kelas satu ("HyperProperties") — bukan sekadar pasangan key-value yang dilampirkan pada record.

Ini berarti:

- **Skema tanpa perancangan di muka** — karena properti adalah node graph, skema _disimpulkan dari data Anda, bukan dirancang_: label, tipe, rentang nilai, dan topologi relasi ditemukan saat penulisan dan dapat di-query langsung — tanpa pemodelan skema manual, tanpa toolchain RDF/OWL
- **Relasi terdeteksi otomatis** — record yang berbagi properti akan terhubung tanpa membuat edge secara manual
- **Introspeksi skema** — agen dapat mengenumerasi label, tipe properti, rentang nilai, dan topologi relasi dalam satu query
- **Batasan lunak** — penilaian kohesi tipe, pelacakan kardinalitas, dan penegakan dimensi vektor tanpa skema kaku di muka
- **Permukaan query terpadu** — ekspresi filter yang sama bekerja pada record, label, properti, dan relasi

Satu SearchQuery mengambil beberapa perspektif secara bersamaan (record + statistik properti + agregasi), menghindari pola inspeksi N+1 yang umum pada arsitektur sistem terpisah.

[Baca postingan arsitektur LMPG selengkapnya →](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</details>

---

## Dokumentasi

| Topik                       | Tautan                                                         |
| --------------------------- | -------------------------------------------------------------- |
| Tutorial Cepat              | https://docs.rushdb.com/get-started/quick-tutorial             |
| Pencarian Vektor / Semantik | https://docs.rushdb.com/concepts/search/where#vector-operators |
| Pemfilteran & Penelusuran   | https://docs.rushdb.com/concepts/search/where                  |
| Pengelompokan & Agregasi    | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK              | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK                  | https://docs.rushdb.com/python-sdk/introduction                |
| REST API                    | https://docs.rushdb.com/rest-api/introduction                  |
| MCP Server                  | packages/mcp-server/README.md                                  |
| Agent Skills                | packages/skills/README.md                                      |

---

## Kapan tidak menggunakan RushDB

- Anda membutuhkan latensi sub-milidetik pada throughput penulisan yang sangat tinggi — RushDB dibangun di atas Neo4j, yang memprioritaskan konsistensi dan ekspresivitas query daripada kecepatan penulisan mentah.
- Anda hanya membutuhkan penyimpanan key-value datar tanpa relasi atau pencarian semantik — penyimpanan yang lebih sederhana akan lebih ringan.
- Anda membutuhkan skema relasional yang kaku dan ditegakkan di tingkat basis data — RushDB sengaja dibuat tanpa skema.

---

## Berkontribusi

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

Lihat [CONTRIBUTING.md](CONTRIBUTING.md) untuk panduan lengkap. Issue dan PR dipersilakan.

---

## Lisensi

| Path                    | Lisensi             |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

Membutuhkan sesuatu yang belum didukung? Buka sebuah issue — diskusi desain dipersilakan.
