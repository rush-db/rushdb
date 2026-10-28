<div align="center" dir="rtl">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### قاعدة بيانات واحدة لسجلاتك وعلاقاتها والبحث فيها.

اكتب JSON. يستنتج RushDB الأنواع، ويربط الكائنات المتداخلة في رسم بياني، ويتيح لك الاستعلام عنه بلغة الاستعلام JSON الخاصة به أو SQL أو Cypher أو Gremlin أو مستندات بنمط Mongo، إلى جانب البحث المتجهي والنصي الكامل والهجين. ملف تنفيذي واحد مكتوب بلغة Rust، ومجلد بيانات واحد، وترخيص Apache-2.0.

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/RushDatabase?style=social)](https://x.com/RushDatabase)

[الموقع](https://rushdb.com) • [التوثيق](https://docs.rushdb.com) • [الدليل](docs/manual/README.md) • [RushDB Cloud](https://app.rushdb.com) • [النشر](deploy/README.md)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • **العربية** • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

<div dir="rtl">

## البدء السريع

</div>

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
rushdb up
```

<div dir="rtl">

يتحقق السكربت من المجموع الاختباري للإصدار (ومن توقيع Sigstore إذا كان `cosign` مثبتًا)، ثم يثبّت الملف التنفيذي `rushdb` في `~/.rushdb/bin`. يشغّل الأمر `rushdb up` عقدة دائمة على `http://127.0.0.1:3000`، وتُحفظ بياناتها في `./.rushdb`، والرمز المحلي هو `local-token`.

اكتب مستند JSON متداخلًا. يصبح كل كائن متداخل سجلًا مستقلًا مرتبطًا بأصله:

</div>

```bash
export RUSHDB_TOKEN=local-token
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"ingest","ingest":{"kind":"documents","label":"COMPANY",
       "documents":{"name":"Acme","DEPARTMENT":[
         {"name":"Engineering","headcount":12,"EMPLOYEE":[{"name":"Ada","role":"Staff Engineer"},{"name":"Linus","role":"Engineer"}]},
         {"name":"Sales","headcount":4,"EMPLOYEE":[{"name":"Grace","role":"Account Executive"}]}]}}}}'
```

<div dir="rtl">

ثم ابحث عن مهندسي Acme. يعبر المرشّح العلاقات بذكر أسماء التسميات المرتبطة:

</div>

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"query","query":{"kind":"records","query":{
       "labels":["EMPLOYEE"],
       "predicate":{"role":{"$contains":"Engineer"},"DEPARTMENT":{"COMPANY":{"name":"Acme"}}}}}}}'
```

<div dir="rtl">

تابع مع [دليل البدء السريع](docs/manual/getting-started/quickstart.md).

## استعلم بالطريقة التي تناسبك

السجلات نفسها متاحة عبر كل واجهة.

**SQL** عبر بروتوكول PostgreSQL، باستخدام `psql` أو أي برنامج تشغيل لـ PostgreSQL. التسميات هي الجداول:

</div>

```bash
PGPASSWORD=$RUSHDB_TOKEN psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, headcount FROM department ORDER BY headcount DESC"
```

<div dir="rtl">

**Cypher** عبر HTTP، أو عبر Bolt باستخدام برامج تشغيل Neo4j (`rushdb up --languages sql,cypher --bolt-port 7687`):

</div>

```cypher
MATCH (d:DEPARTMENT)-->(e:EMPLOYEE)
RETURN d.name AS department, count(e) AS people ORDER BY people DESC
```

<div dir="rtl">

**حزم SDK** في الوضع المباشر (`npm install @rushdb/javascript-sdk`):

</div>

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

<div dir="rtl">

تستخدم Gremlin و SQL/PGQ `GRAPH_TABLE` والمستندات بنمط Mongo و ISO GQL نقطة HTTP نفسها. راجع [لغات الاستعلام](docs/manual/languages/README.md).

## ماذا يقدّم

| الإمكانية | المعنى |
|---|---|
| **لا مخطط لتصميمه** | اكتب JSON بأي شكل. تُستنتج الأنواع لكل قيمة، وتصبح الكائنات المتداخلة سجلات مترابطة، ويحتفظ المحرك بفهرس دقيق قابل للاستعلام للتسميات والخصائص والعلاقات. |
| **الخصائص عناصر من الدرجة الأولى** | يخزّن Labeled Meta-Property Graph (LMPG) كل خاصية كتعريف ذي نوع تتشاركه السجلات التي تستخدمها، لذا يأتي اكتشاف المخطط ونطاقات القيم والتصنيفات من بنية التخزين نفسها. |
| **لغات استعلام متعددة** | لغة استعلام JSON أصلية، و SQL، و Gremlin، و SQL/PGQ، ومستندات بنمط Mongo، و openCypher و ISO GQL كإصدارات مرشحة. |
| **البحث** | فهارس متجهية (دقيقة و HNSW) على المتجهات التي تقدّمها، وفهارس نصية كاملة BM25، وبحث هجين يجمع بينهما. |
| **دائم ويدعم المعاملات** | سجل كتابة مسبقة، ولقطات MVCC، ومعاملات متعددة الجمل، ونسخ احتياطية مع الاستعادة إلى نقطة زمنية، ونسخ متدفق إلى تابعين للقراءة فقط. |
| **بروتوكولات قياسية** | HTTP وبروتوكول PostgreSQL و gRPC و Bolt من عملية واحدة. |
| **تعدد المستأجرين** | مستأجرون اختياريون بنمط الإغلاق عند الفشل، لكل منهم مساحة مفاتيح خاصة. |
| **ملف تنفيذي واحد** | يشغّل `rushdb serve` الخادم، ويشغّل `rushdb up` عقدة محلية. لا خدمة أخرى لتشغيلها. |

## النشر

| أين | كيف |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb` |
| Docker Compose | [`deploy/templates/docker-compose`](deploy/templates/docker-compose/)، مع نسخة HTTPS خلف Caddy |
| Kubernetes | [Helm chart](charts/rushdb/): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token=...` |
| آلة Linux افتراضية | [cloud-init](deploy/templates/cloud-init/) لـ DigitalOcean و Hetzner و AWS و GCP و Azure؛ [وحدة systemd](deploy/templates/systemd/) |
| المنصات | [Fly.io](deploy/templates/fly/)، [Render](deploy/templates/render/)، [Coolify](deploy/templates/coolify/)، [Dokploy](deploy/templates/dokploy/) |

</div>

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

<div dir="rtl">

### طرق أخرى للتثبيت

| الطريقة | الأمر | ملاحظات |
|---|---|---|
| سكربت التثبيت | `curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh \| sh` | Linux (glibc 2.28+) و macOS، x86-64 و arm64. يثبّت `sh -s -- --version v3.0.0` إصدارًا محددًا. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS و Linux. يشغّله `brew services start rushdb` في الخلفية. |
| Docker | `rushdb/rushdb` | `linux/amd64` و `linux/arm64`؛ وأيضًا `ghcr.io/rush-db/rushdb`. |
| Cargo | `cargo binstall rushdb-server` أو `cargo install rushdb-server --locked` | يجلب `binstall` الملف التنفيذي المنشور؛ ويبني `install` من المصدر (يحتاج مترجم C++). |
| أرشيف الإصدار | [GitHub releases](https://github.com/rush-db/rushdb/releases) | أرشيفات مع مجاميع SHA-256 الاختبارية و SBOM وتواقيع Sigstore. |
| المصدر | `cargo build --release -p rushdb-server --bin rushdb` | راجع [التثبيت](docs/manual/getting-started/installation.md). |

لا يُدعم Windows بشكل أصلي؛ استخدم WSL 2 أو Docker. اقرأ [الأمان](docs/manual/operations/security.md) قبل كشف عقدة خارج الواجهة المحلية.

## الحالة

| المجال | الحالة |
|---|---|
| البروتوكول الأصلي، SQL، اجتيازات Gremlin النصية، المستندات بنمط Mongo | مستقر |
| SQL/PGQ `GRAPH_TABLE` | مستقر، للقراءة فقط |
| openCypher و ISO GQL و Bolt | إصدار مرشح، يُفعَّل يدويًا: لا تنجح مجموعات اختبار المطابقة الخارجية بالكامل ([التفاصيل](docs/manual/languages/README.md)) |
| بروتوكول PostgreSQL | قراءات SQL وكتابات بجملة واحدة؛ جرى التحقق باستخدام `psql` و node-postgres و psycopg و `tokio-postgres`، وليس مع كل عميل أو ORM |
| النسخ المتماثل | تدفق غير متزامن إلى تابعين للقراءة فقط؛ الترقية يدوية بلا تجاوز فشل تلقائي |
| Raft وتجزئة التخزين | معاينة، ليست للإنتاج: يعمل Raft في عملية واحدة بحالة في الذاكرة؛ والإيداعات متعددة الأجزاء ليست ذرية عند الانهيار |

يوثّق [الدليل](docs/manual/README.md) الحدود الدقيقة لكل واجهة.

## الاتصال

| الواجهة | الافتراضي | التفعيل |
|---|---|---|
| بروتوكول HTTP الأصلي | `127.0.0.1:3000`، `POST /protocol/v1/execute` | مفعّل دائمًا |
| بروتوكول PostgreSQL | `127.0.0.1:5432` | مفعّل مع `sql` (افتراضيًا) |
| gRPC (`rushdb.v1.Database/Execute`) | معطّل | `--grpc-port` |
| Bolt 5.0 | معطّل | `--bolt-port` مع `cypher` |

تعمل حزم SDK لـ [JavaScript](https://www.npmjs.com/package/@rushdb/javascript-sdk) و [Python](https://pypi.org/project/rushdb/) و[خادم MCP](https://www.npmjs.com/package/@rushdb/mcp-server) و[مهارات الوكلاء](https://github.com/rush-db/skills) مع محرك مستضاف ذاتيًا في الوضع المباشر.

## المحرك المستضاف ذاتيًا و RushDB Cloud

هذا المحرك هو الجزء من RushDB الذي تستضيفه بنفسك. لا يولّد التضمينات، ولا يستضيف لوحة تحكم، ولا يدير المستخدمين: تأتي هذه مع [RushDB Cloud](https://app.rushdb.com)، الخدمة المُدارة التي تشغّل هذا المحرك وتضيف لوحة التحكم والتضمينات المُدارة وميزات الذكاء الاصطناعي (البحث باللغة الطبيعية، واقتراحات أنماط العلاقات). تعمل حزم SDK وخادم MCP مع أي منهما: أعطها مفتاح API من RushDB Cloud، أو وجّهها مباشرة إلى محركك (`direct: true`، `engineUrl`) لكل شيء ما عدا ميزات Cloud، التي ترفضها بالخطأ `CloudOnlyError`.

## التوثيق

- [الدليل](docs/manual/README.md): البدء، ونموذج البيانات، والاستعلامات، واللغات، والبحث، والتشغيل، والجداول المرجعية.
- [docs.rushdb.com](https://docs.rushdb.com): دروس، وحزم SDK، وخادم MCP.
- [الترقية من RushDB 2.x](docs/manual/operations/upgrading-from-2x.md): كان الإصدار 2.x هو الحزمة المبنية على Neo4j، وهو محفوظ عند الوسم `v2` في هذا المستودع.

## المساهمة

راجع [CONTRIBUTING.md](CONTRIBUTING.md). بوابة الجودة:

</div>

```bash
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked
cargo fmt --all -- --check
scripts/check-quality-bar.sh
```

<div dir="rtl">

- [سياسة الأمان](SECURITY.md)
- [عملية الإصدار](RELEASING.md)
- [مدونة السلوك](CODE_OF_CONDUCT.md)
- [سجل التغييرات](CHANGELOG.md)

مرخّص بموجب [Apache-2.0](LICENSE).

</div>
