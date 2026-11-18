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

</div>

```bash
curl -fsSL https://rushdb.com/install.sh | sh    # or: brew install rush-db/tap/rushdb
export PATH="$HOME/.rushdb/bin:$PATH"             # or open a new shell
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

<div dir="rtl">

يأتي المخطط من الفهرس، لا من مسح السجلات:

</div>

```text
## `EMPLOYEE` (3 records)

| Property | Type | Values / Range | Semantic Search |
|----------|------|----------------|-----------------|
| `name` | string | `Ada`, `Linus`, `Grace` | — |
| `role` | string | `Staff Engineer`, `Engineer`, `Account Executive` | — |
| `salary` | number | `120000`..`180000` | — |
```

<details>
<summary>المثال نفسه بلغة TypeScript</summary>

<div dir="rtl">

`npm install @rushdb/javascript-sdk`:

</div>

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

<div dir="rtl">

تتقن SQL؟ التسميات هي الجداول:

</div>

```bash
PGPASSWORD=local-token psql "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable" \
  -c "SELECT name, role, salary FROM employee ORDER BY salary DESC"
```

<div dir="rtl">

التالي: [دليل البدء السريع الكامل](https://docs.rushdb.com/engine/getting-started/quickstart/) ·
[Docker وطرق التثبيت الأخرى](https://docs.rushdb.com/engine/getting-started/installation/)

## ما الذي يمكنك بناؤه

| | |
|---|---|
| **ذاكرة الوكلاء** | الذكريات سجلاتٌ ذات علاقات، تُرشَّح حسب النطاق وتُرتَّب بالمتجهات التي تخزّنها. مزوّدات جاهزة لـ OpenClaw وHermes. |
| **البحث في بيانات تطبيقك** | بحث نصي كامل ومتجهي وهجين في السجلات التي يكتبها تطبيقك أصلًا، مع مرشّحات الرسم البياني في الاستعلام نفسه. |
| **أسئلة على بيانات مترابطة** | يقرأ الوكيل المخطط، ثم يكتب استعلامًا يمكنك فحصه: بلغة الاستعلام JSON أو SQL أو Cypher. |
| **تقارير بلا تصدير** | جمّع السجلات التي يكتبها تطبيقك واحسب تجميعاتها، من حزم SDK أو من أي عميل PostgreSQL. |

## الميزات

- **مخطط ناشئ.** تُستنتج الأنواع لكل قيمة على حدة، وتصبح الكائنات المتداخلة سجلات مترابطة، ويحتفظ الفهرس بأعداد دقيقة وقوائم بالقيم. [فهرس المخطط](https://docs.rushdb.com/engine/data-model/schema-catalog/)
- **استعلامات الرسم البياني.** رشّح عبر العلاقات حسب التسمية، وقيّدها بالنوع والاتجاه، واربط السجلات المرتبطة لاستخدامها في الإسقاطات والتجميعات. [العلاقات والاجتياز](https://docs.rushdb.com/engine/query/relationships-and-traversal/)
- **بحث مدمج.** فهارس متجهية للمتجهات التي تقدّمها، وبحث نصي كامل بـ BM25، وبحث هجين يدمج الاثنين. [الفهارس والبحث](https://docs.rushdb.com/engine/indexes-and-search/)
- **التجميعات.** `select` و`groupBy` مع العدّ والمجاميع والمتوسطات والفواصل الزمنية والمجموعات المتداخلة. [التجميعات](https://docs.rushdb.com/engine/query/aggregations/)
- **يتحدث ما تستخدمه.** SQL عبر بروتوكول PostgreSQL، وGremlin، وSQL/PGQ، ومستندات بنمط Mongo، إضافةً إلى openCypher وISO GQL كمرشّحَين للإصدار. [لغات الاستعلام](https://docs.rushdb.com/engine/languages/)
- **متين.** سجل كتابة مسبق، ومعاملات، ونسخ احتياطية مع الاستعادة إلى نقطة زمنية، ونسخ متماثلة للقراءة. [التشغيل](https://docs.rushdb.com/engine/operations/)
- **ملف تنفيذي واحد.** HTTP وبروتوكول PostgreSQL وgRPC وBolt من عملية واحدة، مع مستأجرين معزولين اختياريًا. [الاتصال](https://docs.rushdb.com/engine/connectivity/)

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

## شغّله في أي مكان

</div>

```bash
docker run -p 127.0.0.1:3000:3000 -v rushdb:/data -e RUSHDB_TOKEN=change-me rushdb/rushdb:3
```

<div dir="rtl">

Docker Compose وKubernetes (Helm) وcloud-init وsystemd وFly.io وRender
وCoolify وDokploy: راجع [النشر](https://docs.rushdb.com/deploy/). اقرأ
[الأمان](https://docs.rushdb.com/engine/operations/security/) قبل أن تكشف عقدة
خارج واجهة loopback.

## الاستضافة الذاتية وRushDB Cloud

- **الاستضافة الذاتية:** هذا المستودع. كل ما سبق يعمل في المحرّك الذي تثبّته.
- **[RushDB Cloud](https://app.rushdb.com):** المحرّك نفسه، مُدارًا، ومعه
  لوحة تحكم، وتضمينات مُدارة، وبحث باللغة الطبيعية، وموصلات لأدوات SaaS.
  تعمل حزم SDK وخادم MCP مع كليهما.

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

**Windows؟** استخدم WSL 2 أو Docker.

---

[المساهمة](https://github.com/rush-db/rushdb/blob/main/CONTRIBUTING.md) •
[الأمان](https://github.com/rush-db/rushdb/blob/main/SECURITY.md) •
[سجل التغييرات](https://github.com/rush-db/rushdb/blob/main/CHANGELOG.md) •
[Apache-2.0](https://github.com/rush-db/rushdb/blob/main/LICENSE)

يوجد RushDB 2.x (الإصدار القديم المبني على Neo4j) في
[فرع `2.x`](https://github.com/rush-db/rushdb/tree/2.x) ويتلقى إصلاحات أمنية
حتى 2027-10-28. [الترقية من 2.x](https://docs.rushdb.com/engine/operations/upgrading-from-2x/)

</div>
