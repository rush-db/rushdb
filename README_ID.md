<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### Satu database untuk record Anda, relasinya, dan pencarian di atasnya.

Tulis JSON. RushDB menyimpulkan tipe data, menghubungkan objek bersarang menjadi graf, dan memungkinkan Anda mengkuerinya dengan bahasa kueri JSON miliknya, SQL, Cypher, Gremlin, atau dokumen bergaya Mongo, ditambah pencarian vektor, teks penuh, dan hibrida. Satu biner Rust, satu direktori data, Apache-2.0.

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[Situs web](https://rushdb.com) • [Dokumentasi](https://docs.rushdb.com) • [Manual](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [Deploy](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • [العربية](README_AR.md) • **Bahasa Indonesia** • [ไทย](README_TH.md)

</div>

---

## Mulai cepat

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

Skrip memeriksa checksum rilis (dan tanda tangan Sigstore bila `cosign` terpasang), lalu memasang biner `rushdb` ke `~/.rushdb/bin`. `rushdb up` menjalankan node yang tahan lama di `http://127.0.0.1:3000` dengan data di `./.rushdb` dan token lokal `local-token`.

Tulis dokumen JSON bersarang. Setiap objek bersarang menjadi record tersendiri yang terhubung ke induknya:

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

Lalu cari para engineer di Acme. Filter menelusuri relasi dengan menyebut label yang terkait:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

Lanjutkan dengan [Quickstart](docs/manual/getting-started/quickstart.md).

## Kueri dengan cara Anda

Record yang sama menjawab lewat setiap antarmuka.

**SQL** lewat protokol wire PostgreSQL, dengan `psql` atau driver PostgreSQL apa pun. Label adalah tabel:

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

**Cypher** lewat HTTP, atau lewat Bolt dengan driver Neo4j (`rushdb up --languages sql,cypher --bolt-port 7687`):

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

**SDK** dalam mode langsung (`npm install @rushdb/javascript-sdk`):

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

Gremlin, SQL/PGQ `GRAPH_TABLE`, dokumen bergaya Mongo, dan ISO GQL memakai endpoint HTTP yang sama. Lihat [Bahasa kueri](docs/manual/languages/README.md).

## Apa yang tersedia

| Kemampuan | Artinya |
|---|---|
| **Tanpa skema untuk dirancang** | Tulis JSON dengan bentuk apa pun. Tipe disimpulkan per nilai, objek bersarang menjadi record yang terhubung, dan engine menyimpan katalog label, properti, dan relasi yang tepat dan dapat dikueri. |
| **Properti adalah warga kelas satu** | Labeled Meta-Property Graph (LMPG) menyimpan setiap properti sebagai definisi bertipe yang dibagi oleh record yang memakainya, sehingga penemuan skema, rentang nilai, dan facet berasal dari tata letak penyimpanan itu sendiri. |
| **Banyak bahasa kueri** | Bahasa kueri JSON native, SQL, Gremlin, SQL/PGQ, dokumen bergaya Mongo, serta openCypher dan ISO GQL sebagai release candidate. |
| **Pencarian** | Indeks vektor (eksak dan HNSW) atas vektor yang Anda berikan, indeks teks penuh BM25, dan pencarian hibrida yang menggabungkan keduanya. |
| **Tahan lama dan transaksional** | Write-ahead log, snapshot MVCC, transaksi multi-pernyataan, backup dengan pemulihan ke titik waktu, dan replikasi streaming ke follower baca-saja. |
| **Protokol standar** | HTTP, protokol wire PostgreSQL, gRPC, dan Bolt dari satu proses. |
| **Multitenansi** | Tenant opsional yang gagal-tertutup, masing-masing dengan keyspace sendiri. |
| **Satu biner** | `rushdb serve` menjalankan server; `rushdb up` menjalankan node lokal. Tidak ada layanan lain untuk dioperasikan. |

## Deploy

| Di mana | Caranya |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/), dengan varian HTTPS di belakang Caddy |
| Kubernetes | [Helm chart](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| VM Linux | [cloud-init](deploy/templates/cloud-init/) untuk DigitalOcean, Hetzner, AWS, GCP, Azure; [unit systemd](deploy/templates/systemd/) |
| Platform | [Fly.io](deploy/templates/fly/), [Render](deploy/templates/render/), [Coolify](deploy/templates/coolify/), [Dokploy](deploy/templates/dokploy/) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

### Cara lain untuk memasang

| Metode | Perintah | Catatan |
|---|---|---|
| Skrip instalasi | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) dan macOS, x86-64 dan arm64. `sh -s -- --version v3.0.0` mengunci rilis. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS dan Linux. `brew services start rushdb` menjalankannya di latar belakang. |
| Docker | `rushdb/rushdb` | `linux/amd64` dan `linux/arm64`; juga `ghcr.io/rush-db/rushdb`. |
| Cargo | `cargo binstall rushdb-server` atau `cargo install rushdb-server --locked` | `binstall` mengambil biner rilis; `install` mengompilasi (perlu kompiler C++). |
| Arsip rilis | [GitHub releases](https://github.com/rush-db/rushdb/releases) | Arsip dengan checksum SHA-256, SBOM, dan tanda tangan Sigstore. |
| Sumber | `cargo build --release -p rushdb-server --bin rushdb` | Lihat [Instalasi](docs/manual/getting-started/installation.md). |

Windows tidak didukung secara native; gunakan WSL 2 atau Docker. Baca [Keamanan](docs/manual/operations/security.md) sebelum membuka node di luar loopback.

## Status

| Area | Status |
|---|---|
| Protokol native, SQL, traversal teks Gremlin, dokumen bergaya Mongo | Stabil |
| SQL/PGQ `GRAPH_TABLE` | Stabil, baca-saja |
| openCypher, ISO GQL, Bolt | Release candidate, harus diaktifkan: suite kesesuaian eksternal belum lulus sepenuhnya ([detail](docs/manual/languages/README.md)) |
| Wire PostgreSQL | Pembacaan SQL dan penulisan satu pernyataan; diperiksa dengan `psql`, node-postgres, psycopg, dan `tokio-postgres`, bukan dengan setiap klien atau ORM |
| Replikasi | Streaming asinkron ke follower baca-saja; promosi manual, tanpa failover otomatis |
| Raft, sharding penyimpanan | Pratinjau, bukan untuk produksi: Raft berjalan dalam satu proses dengan state di memori; commit lintas shard tidak atomik saat crash |

[Manual](docs/manual/README.md) mendokumentasikan batas tepat setiap antarmuka.

## Terhubung

| Antarmuka | Bawaan | Aktifkan |
|---|---|---|
| Protokol HTTP native | `127.0.0.1:3000`, `POST /protocol/v1/execute` | selalu aktif |
| Wire PostgreSQL | `127.0.0.1:5432` | aktif dengan `sql` (bawaan) |
| gRPC (`rushdb.v1.Database/Execute`) | mati | `--grpc-port` |
| Bolt 5.0 | mati | `--bolt-port` dengan `cypher` |

SDK [JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) dan [Python](https://pypi.org/project/rushdb/), [server MCP](https://www.npmjs.com/package/@rushdb/mcp-server), dan [skill agen](https://github.com/rush-db/skills) bekerja dengan engine yang di-host sendiri dalam mode langsung.

## Engine yang di-host sendiri dan RushDB Cloud

Engine ini adalah bagian RushDB yang Anda host sendiri. Ia tidak membuat embedding, tidak menyediakan dasbor, dan tidak mengelola pengguna: semua itu disediakan oleh [RushDB Cloud](https://app.rushdb.com), layanan terkelola yang menjalankan engine ini dan menambahkan dasbor, embedding terkelola, dan fitur AI (pencarian bahasa alami, saran pola relasi). SDK dan server MCP bekerja dengan keduanya: berikan kunci API RushDB Cloud, atau arahkan langsung ke engine Anda (`direct: true`, `engineUrl`) untuk semua hal kecuali fitur Cloud, yang ditolak dengan `CloudOnlyError`.

## Dokumentasi

- [Manual](docs/manual/README.md): memulai, model data, kueri, bahasa, pencarian, operasi, dan tabel referensi.
- [docs.rushdb.com](https://docs.rushdb.com): tutorial, SDK, dan server MCP.
- [Upgrade dari RushDB 2.x](docs/manual/operations/upgrading-from-2x.md): 2.x adalah stack berbasis Neo4j, disimpan di tag `v2` repositori ini.

## Berkontribusi

Lihat [CONTRIBUTING.md](CONTRIBUTING.md). Gerbang kualitas:

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

- [Kebijakan keamanan](SECURITY.md)
- [Proses rilis](RELEASING.md)
- [Kode etik](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

Dilisensikan di bawah [Apache-2.0](LICENSE).
