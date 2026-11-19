<div align="center" dir="rtl">

<img src="https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg" alt="RushDB logo" width="96">

# RushDB

<sub>قاعدة البيانات للذكاء الاصطناعي</sub>

**الرسم البياني والمتجهات والتحليلات على السجلات نفسها. كل عملية كتابة تُبقي مخططًا حيًّا للتسميات والحقول والقيم، فيستعلم الوكلاء عن حقول حقيقية بدلًا من التخمين.**

[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?branch=main&label=CI)](https://github.com/rush-db/rushdb/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rush-db/rushdb?label=release)](https://github.com/rush-db/rushdb/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/rush-db/rushdb/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)

[التوثيق](https://docs.rushdb.com) • [البدء السريع](https://docs.rushdb.com/engine/getting-started/quickstart/) • [النقاشات](https://github.com/orgs/rush-db/discussions) • [X](https://x.com/RushDatabase) • [RushDB Cloud](https://app.rushdb.com)

[English](https://github.com/rush-db/rushdb/blob/main/README.md) • [中文](https://github.com/rush-db/rushdb/blob/main/README_ZH.md) • [日本語](https://github.com/rush-db/rushdb/blob/main/README_JA.md) • [한국어](https://github.com/rush-db/rushdb/blob/main/README_KO.md) • [Deutsch](https://github.com/rush-db/rushdb/blob/main/README_DE.md) • [Français](https://github.com/rush-db/rushdb/blob/main/README_FR.md) • [Português](https://github.com/rush-db/rushdb/blob/main/README_PT.md) • [Español](https://github.com/rush-db/rushdb/blob/main/README_ES.md) • [हिन्दी](https://github.com/rush-db/rushdb/blob/main/README_HI.md) • **العربية** • [Bahasa Indonesia](https://github.com/rush-db/rushdb/blob/main/README_ID.md) • [ไทย](https://github.com/rush-db/rushdb/blob/main/README_TH.md)

</div>

---

<div dir="rtl">

أدخِل بياناتك بالشكل الذي هي عليه: JSON متداخل، أو CSV، أو `INSERT` في SQL، أو عمليات
كتابة بـ Cypher أو بنمط Mongo، أو تدفقات التغييرات من قواعد بياناتك. يخزّنها RushDB سجلاتٍ
ذات أنواع مرتبطةً في رسم بياني، وتجيب السجلات نفسها عن استعلامات العلاقات، والبحث المتجهي
والنصي الكامل، والتجميعات، فلا يوجد مخزن ثانٍ عليك مزامنته. وكل عملية كتابة تحدّث أيضًا
فهرسًا بالتسميات والحقول والأنواع والقيم الموجودة، فيستعلم الوكيل عن حقول حقيقية. اتصل عبر
HTTP أو بروتوكول PostgreSQL أو Bolt أو gRPC أو MCP، من حزم SDK أو من أي عميل PostgreSQL.
ملف تنفيذي واحد مكتوب بلغة Rust، بترخيص Apache-2.0.

| الرسم البياني | البحث المتجهي والنصي الكامل | التحليلات |
|---|---|---|
| يتحوّل JSON المتداخل إلى سجلات مترابطة. رشّح عبر العلاقات، وتتبّع علاقات ذات أنواع واتجاهات في استعلام واحد. | بحث متجهي (دقيق وHNSW)، وبحث نصي كامل بـ BM25، وبحث هجين، مع الترشيح بشروط الرسم البياني نفسها. | العدّ والمجموع والمتوسط والحد الأدنى والأقصى مع `groupBy`، أو SQL عادي مع `GROUP BY` عبر بروتوكول PostgreSQL. |

## البدء السريع

![تسجيل للطرفية: تثبيت RushDB وتشغيله بالأمر rushdb up، ثم من Python استيراد JSON متداخل، وتصفية الموظفين حسب القسم، وحساب عدد الأشخاص ومتوسط الراتب لكل قسم، وطباعة المخطط الحي](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/quickstart.gif)

</div>

```bash
# or: brew install rush-db/tap/rushdb
curl -fsSL https://rushdb.com/install.sh | sh
export PATH="$HOME/.rushdb/bin:$PATH"   # or open a new shell
mkdir my-app && cd my-app && rushdb up
```

<div dir="rtl">

يخدم الأمر `rushdb up` على `http://127.0.0.1:3000`، وتُحفظ بياناته في `./.rushdb`،
والرمز هو `local-token`. في طرفية أخرى، نفّذ `pip install rushdb`:

</div>

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
<summary>المثال نفسه بلغة TypeScript</summary>

<div dir="rtl">

`npm install @rushdb/javascript-sdk`:

</div>

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

<div dir="rtl">

تتقن SQL؟ التسميات هي الجداول:

</div>

```bash
PGPASSWORD=local-token psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

<div dir="rtl">

التالي: [دليل البدء السريع الكامل](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker وطرق التثبيت الأخرى](https://docs.rushdb.com/engine/getting-started/installation/)

## ما يراه الوكلاء قبل الاستعلام

يطبع السطر الأخير من البدء السريع المخطط الذي يقرؤه الوكيل أولًا. يأتي من الفهرس الذي تحدّثه كل عملية كتابة، لا من مسح السجلات:

</div>

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

<div dir="rtl">

على مخزن كبير ومتنوع لا يحمّل الوكيل كل ذلك. بل يضيّق النطاق من النظرة العامة حتى القيم الدقيقة، بقراءة صغيرة في كل مرة (من التشغيل نفسه أعلاه):

</div>

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

<div dir="rtl">

تعيد `get_schema_summary` الفهرس نفسه بصيغة JSON، مع أعداد القيم المميزة وقيم الحقول الصغيرة. ويقدّم خادم MCP كل قراءة كأداة: `findLabels`, `resolveNames`, `getSchemaMarkdown`, `findProperties`, `searchValues`, `propertyValues` و`getSchemaSummary`.

## النشر

| أين | كيف |
|---|---|
| Docker | `docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3` |
| Docker Compose | [`deploy/templates/docker-compose`](https://github.com/rush-db/rushdb/tree/main/deploy/templates/docker-compose)، مع نسخة HTTPS خلف Caddy |
| Kubernetes | [مخطط Helm](https://github.com/rush-db/rushdb/tree/main/charts/rushdb): `helm install rushdb oci://ghcr.io/rush-db/charts/rushdb --set auth.token="$(openssl rand -hex 32)"` |
| آلة Linux افتراضية | [cloud-init](https://github.com/rush-db/rushdb/tree/main/deploy/templates/cloud-init) لـ DigitalOcean وHetzner وAWS وGoogle Cloud وAzure; [وحدة systemd](https://github.com/rush-db/rushdb/tree/main/deploy/templates/systemd) |
| المنصات | [Fly.io](https://github.com/rush-db/rushdb/tree/main/deploy/templates/fly), [Render](https://github.com/rush-db/rushdb/tree/main/deploy/templates/render), [Coolify](https://github.com/rush-db/rushdb/tree/main/deploy/templates/coolify), [Dokploy](https://github.com/rush-db/rushdb/tree/main/deploy/templates/dokploy) |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

المنافذ وفحوص الجاهزية وتقدير الموارد والتشغيل كخدمة: [Deployment](https://docs.rushdb.com/engine/operations/deployment/). كل الخيارات في صفحة واحدة:
[docs.rushdb.com/deploy](https://docs.rushdb.com/deploy/).

### طرق أخرى للتثبيت

| الطريقة | الأمر | ملاحظات |
|---|---|---|
| سكربت التثبيت | `curl -fsSL https://rushdb.com/install.sh \| sh` | Linux (glibc 2.28+) وmacOS 11+، بمعماريتي x86-64 وarm64. الأمر `sh -s -- --version v3.0.0` يثبّت إصدارًا محددًا. |
| Homebrew | `brew install rush-db/tap/rushdb` | macOS وLinux. الأمر `brew services start rushdb` يشغّله في الخلفية. |
| Docker | `rushdb/rushdb:3` | `linux/amd64` و`linux/arm64`، ومتاح أيضًا باسم `ghcr.io/rush-db/rushdb`. الوسوم `3` و`3.0` والإصدارات الدقيقة. |
| Cargo | `cargo binstall rushdb-server` · `cargo install rushdb-server --locked` | `binstall` ينزّل الملف التنفيذي للإصدار؛ و`install` يبنيه من المصدر ويحتاج إلى مترجم C/C++. |
| أرشيف الإصدار | [إصدارات GitHub](https://github.com/rush-db/rushdb/releases) | أرشيفات مع مجاميع SHA-256 وقائمة SBOM بصيغة SPDX وتواقيع Sigstore. |
| المصدر | `cargo build --release -p rushdb-server --bin rushdb` | سلسلة الأدوات مثبّتة في `rust-toolchain.toml`. راجع [التثبيت](https://docs.rushdb.com/engine/getting-started/installation/). |

لا يُدعم Windows دعمًا أصليًا؛ استخدم WSL 2 أو Docker. اقرأ
[الأمان](https://docs.rushdb.com/engine/operations/security/) قبل أن تكشف عقدة
خارج واجهة loopback.

## ما الذي يمكنك بناؤه

- **ذاكرة الوكلاء.** الذكريات سجلاتٌ مقيّدة بالمستخدم أو الوكيل أو الجلسة ومرتبطة بالدور الذي أنتجها، فيُظهر الاسترجاع مصدر كل ذكرى. مزوّدات جاهزة لـ OpenClaw وHermes.
- **دماغ الشركة وRAG.** تصل بيانات المستندات وCRM والدعم والفوترة بأشكالها الخاصة وتُربط بمفاتيح تتشاركها أصلًا، مثل البريد الإلكتروني أو معرّف الحساب. يرشّح الاسترجاع أولًا إلى الحساب أو الإصدار الصحيح، ثم يرتّب بالمتجهات وBM25 معًا، وتستشهد كل معلومة بالسجل الذي وراءها.
- **البحث متعدد الأوجه.** أشرطة ترشيح تأتي أعدادها من الفهرس، محصورة في المرشّح الحالي ودقيقة، مع بحث هجين يُرتَّب تحت المرشّح نفسه. لا فهرس بحث منفصل يحتاج إلى مزامنة.
- **البحث بالذكاء الاصطناعي والمساعدون في تطبيقك.** تعيش فهارس النص والمتجهات داخل قاعدة البيانات وتبقى محدّثة مع كل كتابة من تطبيقك؛ وتقرأ حزم SDK وخادم MCP وعملاء SQL السجلات نفسها.
- **تحليلات البيانات بالوكلاء.** يقرأ الوكيل ما الموجود من وسوم وحقول وكيف تُكتب كل قيمة، ثم يكتب SQL أو Cypher أو تجميعًا عليها. وتأتي كل إجابة مع استعلام يستطيع أي شخص إعادة تشغيله.
- **تطبيقات يتطوّر مخططها أثناء العمل.** أرسل أي JSON: تُستنتج الأنواع، وتصبح الكائنات المتداخلة سجلات مترابطة، وتظهر الحقول الجديدة في الفهرس مع الكتابة التي تضيفها. بلا عمليات ترحيل.

## الميزات

- **مخطط ناشئ.** تُستنتج الأنواع لكل قيمة على حدة، وتصبح الكائنات المتداخلة سجلات مترابطة، ويحتفظ الفهرس بأعداد دقيقة وقوائم بالقيم. [فهرس المخطط](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **استعلامات الرسم البياني.** رشّح عبر العلاقات حسب التسمية، وقيّدها بالنوع والاتجاه، واربط السجلات المرتبطة لاستخدامها في الإسقاطات والتجميعات. [العلاقات والاجتياز](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **بحث مدمج.** فهارس متجهية للمتجهات التي تقدّمها، وبحث نصي كامل بـ BM25، وبحث هجين يدمج الاثنين. [الفهارس والبحث](https://docs.rushdb.com/engine/indexes-and-search/)
- **التجميعات.** `select` و`groupBy` مع العدّ والمجاميع والمتوسطات والفواصل الزمنية والمجموعات المتداخلة. [التجميعات](https://docs.rushdb.com/engine/query/aggregations/)
- **يتحدث ما تستخدمه.** SQL عبر بروتوكول PostgreSQL، وGremlin، وSQL/PGQ، ومستندات بنمط Mongo، إضافةً إلى openCypher وISO GQL كمرشّحَين للإصدار. [لغات الاستعلام](https://docs.rushdb.com/engine/languages/)
- **متين.** سجل كتابة مسبق، ومعاملات، ونسخ احتياطية مع الاستعادة إلى نقطة زمنية، ونسخ متماثلة للقراءة. [التشغيل](https://docs.rushdb.com/engine/operations/)
- **ملف تنفيذي واحد.** HTTP وبروتوكول PostgreSQL وgRPC وBolt من عملية واحدة، مع مستأجرين معزولين اختياريًا. [الاتصال](https://docs.rushdb.com/engine/connectivity/)

## الاتصال

| الواجهة | الافتراضي | التفعيل |
|---|---|---|
| [بروتوكول HTTP الأصلي](https://docs.rushdb.com/engine/connectivity/http-api/) | `127.0.0.1:3000`, `POST /protocol/v1/execute` | يعمل دائمًا؛ `--port` يغيّر المنفذ |
| [بروتوكول PostgreSQL](https://docs.rushdb.com/engine/connectivity/postgresql-wire/) | `127.0.0.1:5432` | يعمل ما دامت `sql` مفعّلة (افتراضيًا)؛ `--pgwire-port 0` يوقفه |
| [gRPC](https://docs.rushdb.com/engine/connectivity/grpc/) (`rushdb.v1.Database/Execute`) | متوقف | `--grpc-port <port>` |
| [Bolt 5.0](https://docs.rushdb.com/engine/connectivity/bolt/) | متوقف | `--bolt-port <port>` مع `cypher` ضمن `--languages` |

تستمع كل الواجهات على عنوان `--listen` (الافتراضي `127.0.0.1`) وتتشارك شهادة TLS واحدة
(`--tls-cert` و`--tls-key`). المزيد في [الاتصال](https://docs.rushdb.com/engine/connectivity/) و[المصادقة](https://docs.rushdb.com/engine/connectivity/authentication/)
و[عملاء SQL وCypher](https://docs.rushdb.com/connect/sql-and-cypher/). تتصل حزم SDK وخادم MCP والتكاملات في الجدول أدناه
بمحرّك مستضاف ذاتيًا في الوضع المباشر: راجع [Connect](https://docs.rushdb.com/connect/).

## المنظومة

| المشروع | لمن |
|---|---|
| [`@rushdb/javascript-sdk`](https://github.com/rush-db/javascript-sdk) · [`rushdb`](https://github.com/rush-db/rushdb-python) (PyPI) | تطبيقات TypeScript وPython |
| [`@rushdb/mcp-server`](https://github.com/rush-db/mcp-server) | Claude وCursor وعملاء MCP الآخرون |
| [مهارات الوكلاء](https://github.com/rush-db/skills) · [إضافات الوكلاء](https://github.com/rush-db/agent-plugins) | وكلاء البرمجة |
| [LangChain وLangGraph](https://github.com/rush-db/langchain-rushdb) · [LlamaIndex](https://github.com/rush-db/llama-index-rushdb) | الاسترجاع والذاكرة ومخازن الرسوم البيانية للخصائص |
| [OpenClaw](https://github.com/rush-db/rushdb-openclaw-memory) · [Hermes](https://github.com/rush-db/rushdb-hermes-memory) · [عقد الذاكرة](https://github.com/rush-db/agent-memory-contract) | مزوّدات ذاكرة الوكلاء |
| [Debezium sink](https://github.com/rush-db/debezium-server-rushdb) · [إضافة Obsidian](https://github.com/rush-db/obsidian-plugin) | التقاط تغييرات البيانات، والملاحظات |

تعمل جميعها مع محرّك مستضاف ذاتيًا. لتدع وكيل البرمجة لديك يُعدّ RushDB، أعطه
[rushdb.com/agent-setup](https://rushdb.com/agent-setup).

## الاستضافة الذاتية وRushDB Cloud

هذا المستودع هو المحرّك، ويمكنك استضافته بالكامل بنفسك. تشغّل [RushDB Cloud](https://app.rushdb.com) المحرّك نفسه نيابةً عنك وتضيف الميزات المعلَّمة أدناه. تعمل حزم SDK وخادم MCP مع كليهما؛ وعلى محرّك مستضاف ذاتيًا، يفشل أي استدعاء خاص بالسحابة بالخطأ `CloudOnlyError` قبل إرسال أي شيء.

![لوحة تحكم RushDB ببيانات نموذجية اصطناعية: جدول السجلات، ثم عرض الرسم البياني للشركات ومجموعات أقسامها، مع تكبير على قسم واحد تظهر قائمة موظفيه في لوحة السجل](https://raw.githubusercontent.com/rush-db/rushdb/main/assets/dashboard.gif)

| الميزة | المحرّك المستضاف ذاتيًا | RushDB Cloud |
|---|---|---|
| المحرّك وواجهة API: السجلات، واستعلامات الرسم البياني، والبحث، والتجميعات، والمخطط الحي | ✓ | ✓ |
| حزم SDK وخادم MCP | ✓ الوضع المباشر، وMCP عبر stdio | ✓ ومعها MCP مستضاف مع OAuth |
| استيراد JSON وCSV | ✓ | ✓ |
| [لوحة تحكم](https://docs.rushdb.com/rushdb-cloud/) فيها السجلات وعرض الرسم البياني ومنشئ الاستعلامات | استخدم حزم SDK أو أي عميل PostgreSQL | ✓ |
| [تضمينات مُدارة](https://docs.rushdb.com/rushdb-cloud/capabilities/) | أحضر المتجهات من نموذجك الخاص | ✓ حصة شهرية |
| [البحث باللغة الطبيعية](https://docs.rushdb.com/learn/search-query/smart-search/) | اكتب الاستعلام بنفسك أو بمساعدة وكيل | ✓ حصة شهرية |
| [اقتراحات العلاقات](https://docs.rushdb.com/learn/relationships/suggested-patterns/) | أنشئ العلاقات بنفسك | ✓ |
| [التعريفات](https://docs.rushdb.com/learn/search-query/definitions/) للمقاييس والمرشّحات | احفظ القواعد في موجّهاتك وSQL الخاص بك | ✓ |
| [الاتصالات](https://docs.rushdb.com/learn/data-sources/connections/) بأدوات SaaS، وRushDB Sync لقواعد البيانات المحلية | شيفرة المزامنة الخاصة بك، أو مصرِف Debezium | ✓ |
| المشاريع ومفاتيح API | المستأجرون والرموز تديرها بنفسك | ✓ |
| وصول الفريق | — | الخطط المدفوعة |
| النسخ الاحتياطية | تديرها بنفسك: النسخ الاحتياطي، والاستعادة إلى نقطة زمنية، والنسخ المتماثلة | الخطط المدفوعة |

تعني علامة ✓ تحت RushDB Cloud أن الخطة المجانية Free تتضمنها. الخطط والحصص في [rushdb.com/pricing](https://rushdb.com/pricing).

## الأسئلة الشائعة

**هل هو جاهز للإنتاج؟** SQL ولغة الاستعلام JSON الأصلية وGremlin والمستندات بنمط
Mongo مستقرة. أما openCypher وISO GQL وBolt فهي مرشّحة للإصدار. النسخ المتماثل
غير متزامن مع ترقية يدوية. تسرد
[صفحة الحالة](https://docs.rushdb.com/engine/reference/status/) كل واجهة
وحدودها.

**هل أحتاج إلى قاعدة بيانات متجهية منفصلة؟** لا. تعيش المتجهات وفهارس النصوص
والعلاقات في محرّك واحد، ويمكن كتابة السجل مع متجهه في عملية إيداع واحدة. يبحث
المحرّك في المتجهات التي تقدّمها؛ ويمكن لـ RushDB Cloud توليدها لك.

**ما المخطط الحي؟** فهرس لكل تسمية وخاصية ونوع وقيمة وشكل علاقة، يُحدَّث مع كل
إيداع، بأعداد دقيقة. يعيده استدعاء واحد، فلا يخمّن الوكلاء والأدوات أسماء الحقول.
[فهرس المخطط](https://docs.rushdb.com/engine/data-model/schema-catalog/)

---

[المساهمة](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[الأمان](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[سجل التغييرات](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

يوجد RushDB 2.x (الإصدار القديم المبني على Neo4j) في
[فرع `2.x`](https://github.com/rush-db/rushdb/tree/2.x) ويتلقى إصلاحات أمنية
حتى 2027-10-28. [الترقية من 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)

</div>
