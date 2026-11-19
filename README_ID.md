<div align="center">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="Logo RushDB" width="96">

# RushDB

<sub>Database untuk AI</sub>

**Graf, vektor, dan analitik pada record yang sama. Setiap penulisan menjaga skema hidup berisi label, field, dan nilai, sehingga agen membuat kueri atas field yang benar-benar ada, bukan menebak.**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[Dokumentasi](https://docs.rushdb.com) • [Mulai cepat](https://docs.rushdb.com/engine/getting-started/quickstart/) • [Diskusi](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • [العربية](https://github.com/rush-db/rushdb/blob/main/README_AR.md) • **Bahasa Indonesia** • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

Masukkan data dalam bentuk yang Anda punya: JSON bersarang, CSV, `INSERT` SQL, penulisan
Cypher atau bergaya Mongo, atau change stream dari database Anda. RushDB menyimpannya
sebagai record bertipe yang saling terhubung dalam graf, dan record yang sama menjawab
kueri relasi, pencarian vektor dan teks lengkap, serta agregasi, sehingga tidak ada
penyimpanan kedua yang harus disinkronkan. Setiap penulisan juga memperbarui katalog
label, field, tipe, dan nilai yang ada, sehingga agen mengkueri field yang benar-benar ada.
Hubungkan lewat HTTP, protokol wire PostgreSQL, Bolt, gRPC, atau MCP, dari SDK atau klien
PostgreSQL apa pun. Satu binary Rust, Apache-2.0.

| Graf | Pencarian vektor dan teks lengkap | Analitik |
|---|---|---|
| JSON bersarang menjadi record yang saling terhubung. Filter lintas relasi dan telusuri relasi bertipe dan berarah dalam satu kueri. | Pencarian vektor (eksak dan HNSW), teks lengkap BM25, dan hibrida, difilter dengan predikat graf yang sama. | Count, sum, rata-rata, min, dan max dengan `groupBy`, atau SQL biasa dengan `GROUP BY` lewat protokol wire PostgreSQL. |

## Mulai cepat

![Rekaman terminal: memasang RushDB, menjalankannya dengan rushdb up, lalu dari Python mengimpor JSON bersarang, memfilter karyawan menurut departemen, menghitung jumlah orang dan rata-rata gaji per departemen, dan mencetak skema hidup](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

`rushdb up` melayani `http://127.0.0.1:3000` dengan data di `./.rushdb` dan
token `local-token`. Di terminal lain, `pip install rushdb`:

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
<summary>Hal yang sama dengan TypeScript</summary>

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

Sudah terbiasa dengan SQL? Label adalah tabel:

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

Selanjutnya: [panduan mulai cepat lengkap](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker dan cara instalasi lain](https://docs.rushdb.com/engine/getting-started/installation/)

## Yang dilihat agen sebelum membuat kueri

Baris terakhir quickstart mencetak skema yang pertama kali dibaca agen. Skema ini berasal dari katalog yang diperbarui setiap penulisan, bukan dari pemindaian record:

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

Pada penyimpanan yang besar dan beragam, agen tidak memuat semuanya. Agen mempersempit dari gambaran umum hingga nilai yang persis, satu pembacaan kecil setiap kali (dari run yang sama di atas):

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

`get_schema_summary` mengembalikan katalog yang sama sebagai JSON, dengan jumlah nilai unik dan nilai dari field kecil. Server MCP menyediakan setiap pembacaan sebagai tool: `findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues`, dan `getSchemaSummary`.

## Deploy

| Di mana | Caranya |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3` |
| Docker Compose | [`deploy/templates/docker-compose`](https://github.com/rush-db/rushdb/tree/main/deploy/templates/docker-compose), dengan varian HTTPS di belakang Caddy |
| Kubernetes | [Helm chart](https://github.com/rush-db/rushdb/tree/main/charts/rushdb): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token="$(openssl rand -hex 32)"` |
| VM Linux | [cloud-init](https://github.com/rush-db/rushdb/tree/main/deploy/templates/cloud-init) untuk DigitalOcean, Hetzner, AWS, Google Cloud, dan Azure; [unit systemd](https://github.com/rush-db/rushdb/tree/main/deploy/templates/systemd) |
| Platform | [Fly.io](https://github.com/rush-db/rushdb/tree/main/deploy/templates/fly), [Render](https://github.com/rush-db/rushdb/tree/main/deploy/templates/render), [Coolify](https://github.com/rush-db/rushdb/tree/main/deploy/templates/coolify), [Dokploy](https://github.com/rush-db/rushdb/tree/main/deploy/templates/dokploy) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

Port, probe, sizing, dan menjalankannya sebagai layanan: [Deployment](https://docs.rushdb.com/engine/operations/deployment/). Semua opsi dalam satu halaman:
[docs.rushdb.com/deploy](https://docs.rushdb.com/deploy/).

### Cara lain untuk memasang

| Cara | Perintah | Catatan |
|---|---|---|
| Skrip instalasi | `curl -fsSL https://rushdb.com/install.sh \| sh` | Linux (glibc 2.28+) dan macOS 11+, x86-64 dan arm64. `sh -s -- --version v3.0.0` mengunci rilis tertentu. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS dan Linux. `brew services start rushdb` menjalankannya di latar belakang. |
| Docker | `rushdb/rushdb:3` | `linux/amd64` dan `linux/arm64`, juga `ghcr.io/rush-db/rushdb`. Tag `3`, `3.0`, dan versi persis. |
| Cargo | `cargo binstall rushdb-server` · `cargo install rushdb-server --locked` | `binstall` mengunduh biner rilis; `install` mengompilasinya dan butuh kompiler C/C++. |
| Arsip rilis | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Arsip dengan checksum SHA-256, SBOM SPDX, dan tanda tangan Sigstore. |
| Kode sumber | `cargo build --release -p rushdb-server --bin rushdb` | Toolchain dikunci di `rust-toolchain.toml`. Lihat [Instalasi](https://docs.rushdb.com/engine/getting-started/installation/). |

Windows tidak didukung secara native; gunakan WSL 2 atau Docker. Baca
[Keamanan](https://docs.rushdb.com/engine/operations/security/) sebelum Anda membuka node ke luar
loopback.

## Apa yang bisa Anda bangun

- **Memori agen.** Memori adalah record yang dibatasi per pengguna, agen, atau sesi dan ditautkan ke giliran yang menghasilkannya, sehingga recall menunjukkan asal sebuah memori. Provider siap pakai untuk OpenClaw dan Hermes.
- **Otak perusahaan dan RAG.** Data dokumen, CRM, dukungan, dan penagihan masuk dalam bentuknya sendiri dan ditautkan lewat kunci yang sudah sama, seperti email atau ID akun. Pengambilan data memfilter ke akun atau versi yang tepat lebih dulu, lalu memberi peringkat dengan vektor dan BM25 sekaligus, dan setiap fakta merujuk record di baliknya.
- **Pencarian berfaset.** Panel filter yang hitungannya berasal dari katalog, terbatas pada filter saat ini dan persis, dengan pencarian hibrida yang diperingkat di bawah filter yang sama. Tanpa indeks pencarian terpisah yang harus disinkronkan.
- **Pencarian AI dan asisten di aplikasi Anda.** Indeks teks dan vektor ada di dalam database dan selalu mutakhir saat aplikasi Anda menulis; SDK, server MCP, dan klien SQL membaca record yang sama.
- **Analitik data berbasis agen.** Agen membaca label dan field apa saja yang ada dan bagaimana setiap nilai dieja, lalu menulis SQL, Cypher, atau agregasi terhadapnya. Setiap jawaban disertai kueri yang bisa dijalankan ulang siapa pun.
- **Aplikasi yang skemanya berubah seiring jalan.** Kirim JSON apa pun: tipe disimpulkan, objek bersarang menjadi record yang tertaut, dan field baru muncul di katalog bersama penulisan yang menambahkannya. Tanpa migrasi.

## Fitur

- **Skema yang terbentuk dengan sendirinya.** Tipe disimpulkan per nilai, objek bersarang menjadi record yang saling terhubung, dan katalog menyimpan jumlah yang tepat serta daftar nilai. [Katalog skema](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **Kueri graf.** Filter lintas relasi berdasarkan label, batasi berdasarkan tipe dan arah, dan ikat record terkait untuk proyeksi dan agregasi. [Relasi dan penelusuran](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **Pencarian bawaan.** Indeks vektor atas vektor yang Anda berikan, teks lengkap BM25, dan pencarian hibrida yang menggabungkan keduanya. [Indeks dan pencarian](https://docs.rushdb.com/engine/indexes-and-search/)
- **Agregasi.** `select` dan `groupBy` dengan count, sum, rata-rata, bucket waktu, dan koleksi bersarang. [Agregasi](https://docs.rushdb.com/engine/query/aggregations/)
- **Mendukung yang Anda pakai.** SQL lewat protokol wire PostgreSQL, Gremlin, SQL/PGQ, dokumen bergaya Mongo, serta openCypher dan ISO GQL sebagai release candidate. [Bahasa kueri](https://docs.rushdb.com/engine/languages/)
- **Tahan lama.** Write-ahead log, transaksi, backup dengan pemulihan ke titik waktu tertentu, dan replika baca. [Operasional](https://docs.rushdb.com/engine/operations/)
- **Satu binary.** HTTP, wire PostgreSQL, gRPC, dan Bolt dari satu proses, dengan tenant terisolasi opsional. [Konektivitas](https://docs.rushdb.com/engine/connectivity/)

## Koneksi

| Antarmuka | Bawaan | Cara mengaktifkan |
|---|---|---|
| [Protokol HTTP native](https://docs.rushdb.com/engine/connectivity/http-api/) | `127.0.0.1:3000`, `POST /protocol/v1/execute` | Selalu aktif; `--port` memindahkannya |
| [Protokol PostgreSQL](https://docs.rushdb.com/engine/connectivity/postgresql-wire/) | `127.0.0.1:5432` | Aktif selama `sql` diaktifkan (bawaan); `--pgwire-port 0` mematikannya |
| [gRPC](https://docs.rushdb.com/engine/connectivity/grpc/) (`rushdb.v1.Database/Execute`) | Mati | `--grpc-port <port>` |
| [Bolt 5.0](https://docs.rushdb.com/engine/connectivity/bolt/) | Mati | `--bolt-port <port>` dengan `cypher` di `--languages` |

Semua listener memakai alamat `--listen` (bawaan `127.0.0.1`) dan berbagi satu sertifikat
TLS (`--tls-cert`, `--tls-key`). Selengkapnya di [Konektivitas](https://docs.rushdb.com/engine/connectivity/),
[Autentikasi](https://docs.rushdb.com/engine/connectivity/authentication/), dan [Klien SQL dan Cypher](https://docs.rushdb.com/connect/sql-and-cypher/). SDK, server MCP, dan
integrasi di bawah terhubung ke engine self-hosted dalam mode langsung: lihat
[Connect](https://docs.rushdb.com/connect/).

## Ekosistem

| Proyek | Untuk |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | Aplikasi TypeScript dan Python |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude, Cursor, dan klien MCP lainnya |
| [Skill agen](https://github.com/rush-db/skills) · [plugin agen](https://github.com/rush-db/agent-plugins) | Agen coding |
| [LangChain dan LangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | Retrieval, memori, dan property graph store |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [kontrak memori](https://github.com/rush-db/agent-memory-contract) | Provider memori agen |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [plugin Obsidian](https://github.com/rush-db/obsidian-plugin) | Change data capture, catatan |

Semuanya bekerja dengan engine self-hosted. Agar agen coding Anda bisa
menyiapkan RushDB, berikan [rushdb.com/agent-setup](https://rushdb.com/agent-setup) kepadanya.

## Self-hosted dan RushDB Cloud

Repositori ini adalah mesinnya, dan semuanya bisa Anda host sendiri. [RushDB Cloud](https://app.rushdb.com) menjalankan mesin yang sama untuk Anda dan menambahkan fitur yang ditandai di bawah. SDK dan server MCP bekerja dengan keduanya; pada mesin yang di-host sendiri, panggilan khusus Cloud gagal dengan `CloudOnlyError` sebelum apa pun dikirim.

![Dasbor RushDB dengan data contoh sintetis: tabel record, lalu tampilan graf perusahaan dan kelompok departemennya, diperbesar ke satu departemen yang karyawannya tercantum di panel record](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| Fitur | Mesin yang di-host sendiri | RushDB Cloud |
|---|---|---|
| Mesin dan API: record, kueri graf, pencarian, agregasi, skema langsung | ✓ | ✓ |
| SDK dan server MCP | ✓ mode langsung, MCP lewat stdio | ✓ plus MCP ter-host dengan OAuth |
| Impor JSON dan CSV | ✓ | ✓ |
| [Dasbor](https://docs.rushdb.com/rushdb-cloud/) dengan record, tampilan graf, dan pembuat kueri | Gunakan SDK atau klien PostgreSQL apa pun | ✓ |
| [Embedding terkelola](https://docs.rushdb.com/rushdb-cloud/capabilities/) | Bawa vektor dari model Anda sendiri | ✓ kuota bulanan |
| [Pencarian bahasa alami](https://docs.rushdb.com/learn/search-query/smart-search/) | Susun kueri sendiri atau dengan agen | ✓ kuota bulanan |
| [Saran relasi](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | Buat relasi sendiri | ✓ |
| [Definisi](https://docs.rushdb.com/learn/search-query/definitions/) untuk metrik dan filter | Simpan aturannya di prompt dan SQL Anda sendiri | ✓ |
| [Koneksi](https://docs.rushdb.com/learn/data-sources/connections/) ke alat SaaS, dan RushDB Sync untuk database on-prem | Kode sinkronisasi Anda sendiri, atau sink Debezium | ✓ |
| Proyek dan kunci API | Tenant dan token yang Anda kelola | ✓ |
| Akses tim | — | Paket berbayar |
| Cadangan | Anda yang menjalankan: cadangan, pemulihan titik waktu, replika | Paket berbayar |

Tanda ✓ di bawah RushDB Cloud berarti paket Free sudah mencakupnya. Paket dan kuota ada di [rushdb.com/pricing](https://rushdb.com/pricing).

## FAQ

**Apakah sudah siap untuk produksi?** SQL, bahasa kueri JSON native, Gremlin, dan
dokumen bergaya Mongo sudah stabil. openCypher, ISO GQL, dan Bolt masih berstatus
release candidate. Replikasi bersifat asinkron dengan promosi manual.
[Halaman status](https://docs.rushdb.com/engine/reference/status/) mencantumkan setiap
antarmuka beserta batasannya.

**Apakah saya butuh database vektor terpisah?** Tidak. Vektor, indeks teks, dan
relasi berada dalam satu engine, dan sebuah record bisa ditulis bersama
vektornya dalam satu commit. Engine mencari atas vektor yang Anda berikan; RushDB
Cloud bisa membuatkannya untuk Anda.

**Apa itu skema hidup?** Katalog berisi setiap label, properti, tipe, nilai,
dan bentuk relasi, diperbarui pada setiap commit, dengan jumlah yang tepat. Satu panggilan
mengembalikannya, sehingga agen dan tool tidak perlu menebak nama field.
[Katalog skema](https://docs.rushdb.com/engine/data-model/schema-catalog/)

---

[Berkontribusi](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[Keamanan](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[Changelog](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

RushDB 2.x (legacy, berbasis Neo4j) ada di
[branch `2.x`](https://github.com/rush-db/rushdb/tree/2.x) dengan perbaikan keamanan
hingga 2027-10-28. [Upgrade dari 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)
