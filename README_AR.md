<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### طبقة الذاكرة لوكلاء وتطبيقات الذكاء الاصطناعي.

ادفع أي JSON. يحصل وكيلك على مخطط حيّ قابل للاستعلام، وعلاقات رسم بياني، وبحث دلالي — يُستنتج تلقائيًا.
لا خطوط معالجة. لا مخازن منفصلة. لا مخطط لتصميمه.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI Version](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](packages/javascript-sdk/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/rush-db/rushdb/ci.yml?label=tests)](https://github.com/rush-db/rushdb/actions)

[🌐 Website](https://rushdb.com) • [📖 Documentation](https://docs.rushdb.com) • [☁️ Cloud](https://app.rushdb.com) • [🔍 Examples](https://github.com/rush-db/examples)

[English](README.md) • [中文](README_ZH.md) • [日本語](README_JA.md) • [한국어](README_KO.md) • [Deutsch](README_DE.md) • [Français](README_FR.md) • [Português](README_PT.md) • [Español](README_ES.md) • [हिन्दी](README_HI.md) • **العربية** • [Bahasa Indonesia](README_ID.md) • [ไทย](README_TH.md)

</div>

---

<div dir="rtl">

## المشكلة

يحتاج وكيلك إلى ذاكرة. الإجابة المعتادة هي ثلاث قواعد بيانات: Redis للمفتاح-القيمة، ومخزن متجهات للبحث الدلالي، وقاعدة بيانات رسم بياني للعلاقات — بالإضافة إلى شيفرة ربط للحفاظ على تزامنها.

يستبدل RushDB الثلاثة جميعًا. ادفع JSON مرة واحدة. استعلم عنه بالتنقل في الرسم البياني، أو البحث الدلالي، أو كليهما — في استدعاء واحد.

</div>

| بدون RushDB                              | مع RushDB                        |
| ---------------------------------------- | -------------------------------- |
| Redis + Pinecone + Neo4j + شيفرة ربط     | واجهة برمجة تطبيقات واحدة        |
| تصميم المخطط ← كتابة الترحيلات ← التكرار | ادفع أي شكل، دون الحاجة إلى مخطط |
| خط معالجة تضمين منفصل                    | تضمينات مُدارة، على جانب الخادم  |
| صياغة حواف العلاقات يدويًا               | اكتشاف تلقائي من بنية بياناتك    |

---

<div dir="rtl">

## بداية سريعة

مساران حسب إعدادك:

- **السحابة (Cloud)** — مُدارة، طبقة مجانية، تعمل خلال 30 ثانية. [احصل على مفتاح API ←](https://app.rushdb.com)
- **الاستضافة الذاتية** — Docker + نسخة Neo4j الخاصة بك. [انتقل إلى الاستضافة الذاتية ←](#self-hosting)

### مسار السحابة

</div>

```bash
npm install @rushdb/javascript-sdk
# or
pip install rushdb
```

<div dir="rtl">

### تخزين واسترجاع ذاكرة الوكيل

</div>

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

<div dir="rtl">

## التعامل مع بياناتك

### استيراد JSON المتداخل

ادفع أي شكل من JSON. تتحوّل الكائنات المتداخلة ومصفوفات الكائنات إلى سجلات مرتبطة — تُستنتج التسميات والأنواع والعلاقات عند الكتابة. لا مخطط، لا خطوة ترحيل.

</div>

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

<div dir="rtl">

يصبح كل مفتاح متداخل (`DEPARTMENT`، `EMPLOYEE`) تسمية، وكل كائن سجلًا، ويصبح الاحتواء علاقة — كلها تُنشأ تلقائيًا.

### استيراد CSV

</div>

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

<div dir="rtl">

يستنتج `suggestTypes` الأرقام والقيم المنطقية والتواريخ من السلاسل النصية؛ ويعامل `skipEmptyValues` الخلايا الفارغة على أنها غير محدّدة بدلًا من تخزين قيم فارغة (تُحفظ `0` و`false`).

### التنقل في الرسم البياني

رشّح السجلات الجذرية بشروط على سجلاتها _المرتبطة_ — بأي عمق كان — في استعلام واحد. توضع التسميات المرتبطة **داخل** `where`، وليس في `labels`:

</div>

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

<div dir="rtl">

### الاستعلامات التحليلية (التجميع والتجميع حسب)

يشكّل `select` المخرجات بعمليات التجميع (`$sum`، `$avg`، `$count`، `$min`، `$max`)؛ ويتحكم `groupBy` في الأبعاد. لا تُضِف `limit` إلى عملية تجميع — فهي ستفحص أول N سجلات فقط وتشوّه الإجماليات.

</div>

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

<div dir="rtl">

تتكامل عمليات التجميع مع التنقل — مثلًا عدد الموظفين وكشف الرواتب لكل قسم:

</div>

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

<div dir="rtl">

## الاتصال بـ Claude أو Cursor أو أي عميل MCP

يأتي RushDB مع خادم MCP. يحصل وكيلك على ذاكرة دائمة ومنظّمة — جاهزة فور الاستخدام.

</div>

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

<div dir="rtl">

ضع هذا في إعداد MCP الخاص بـ Claude Desktop أو Cursor أو Windsurf. يصبح بإمكان الوكيل الآن إنشاء السجلات، والبحث بالمعنى، والتنقل عبر العلاقات، واستبطان المخطط — كل ذلك عبر اللغة الطبيعية.

</div>

---

<div dir="rtl">

## ما الذي تحتويه العلبة

</div>

| القدرة                               | ماذا تعني                                                                                                                                   |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| **تضمينات مُدارة**                   | فهرس أي خاصية نصية مرة واحدة — وتُضمَّن كل عملية كتابة تلقائيًا على جانب الخادم                                                             |
| **رسم بياني + متجه في استعلام واحد** | يتكامل التشابه الدلالي والتنقل عبر العلاقات في استدعاء واحد                                                                                 |
| **بلا مخطط**                         | ادفع أي شكل من JSON. يستنتج RushDB الأنواع، وينشئ الخصائص، ويربط السجلات                                                                    |
| **مخطط مُستنتَج**                    | تصبح الخصائص عقدًا من الدرجة الأولى — تُكتشف الأنواع والتسميات والعلاقات عند الكتابة، فيبني مخطط قابل للاستعلام نفسه بنفسه مع وصول البيانات |
| **معاملات ACID**                     | لا يُفسد الوكلاء المتزامنون الذاكرة المشتركة. Neo4j تحت الغطاء                                                                              |
| **ذاتي الوصف**                       | يستبطن الوكلاء المخطط المُستنتَج — التسميات والخصائص ونطاقات القيم — لمعرفة ما يمكنهم الاستعلام عنه بأمان                                   |
| **أصيل في MCP**                      | خادم MCP كامل مع موجّه استعلام مبني على مبدأ الاكتشاف أولًا                                                                                 |
| **مهارات الوكيل (Agent Skills)**     | حزمة `@rushdb/skills` قابلة للتثبيت — علّم أي وكيل متوافق مع المهارات أن يستعلم وينمذج ويتذكر باستخدام RushDB بأمر واحد                     |
| **واجهة استعلام موحّدة**             | شكل JSON واحد للرسم البياني والمتجه والتجميع والاستبطان                                                                                     |
| **استضافة ذاتية أو سحابة**           | Docker + Neo4j الخاص بك، أو سحابة مُدارة. ملكية كاملة للبيانات                                                                              |

---

<div dir="rtl">

## حالات الاستخدام

</div>

| حالة الاستخدام            | ما يستبدله RushDB                            | واجهة API الأساسية                                            |
| ------------------------- | -------------------------------------------- | ------------------------------------------------------------- |
| **ذاكرة الوكيل**          | Redis + مخزن متجهات + قاعدة بيانات رسم بياني | `db.records.vectorSearch({ query, where: { agent_id } })`     |
| **RAG مع السياق**         | مخزن متجهات مسطّح                            | `db.records.find({ where, labels })` + التنقل عبر العلاقات    |
| **تطبيقات بلا مخطط**      | Postgres + ترحيلات + ETL                     | `db.records.importJson(nestedJson)`                           |
| **منتجات بيانات مترابطة** | خدمات متعددة مدمجة                           | `db.records.find({ labels, where: { SOME_LABEL: { ... } } })` |

---

<div dir="rtl">

## الاستضافة الذاتية

> **مسار الاستضافة الذاتية** — شغّل RushDB على بنيتك التحتية الخاصة. يتطلب Neo4j 2026.01.4+ مع إضافة APOC.

</div>

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
<summary>متغيرات البيئة الكاملة</summary>

| الاسم                             | الوصف                                           | مطلوب       | الافتراضي |
| --------------------------------- | ----------------------------------------------- | ----------- | --------- |
| `NEO4J_URL`                       | عنوان اتصال Neo4j                               | نعم         | —         |
| `NEO4J_USERNAME`                  | اسم مستخدم Neo4j                                | نعم         | neo4j     |
| `NEO4J_PASSWORD`                  | كلمة مرور Neo4j                                 | نعم         | —         |
| `RUSHDB_AES_256_ENCRYPTION_KEY`   | مفتاح من 32 حرفًا لتشفير رموز API               | نعم (إنتاج) | —         |
| `RUSHDB_PORT`                     | منفذ HTTP                                       | لا          | 3000      |
| `RUSHDB_LOGIN`                    | تسجيل دخول المسؤول                              | لا          | admin     |
| `RUSHDB_PASSWORD`                 | كلمة مرور المسؤول                               | لا          | password  |
| `RUSHDB_BASE_URL`                 | عنوان API العام/الأساسي لتعيينات synx           | لا          | —         |
| `RUSHDB_SYNX_CONTROL_TOKEN`       | رمز داخلي لعمّال synx المُدارين                 | لا          | —         |
| `RUSHDB_SYNX_DESTINATION_API_KEY` | مفتاح كتابة داخلي لعمليات الكتابة إلى وجهة synx | لا          | —         |

تعمل عمّال synx المُدارة كخدمات خفية (daemons). تستقصي عن الموصّلات القابلة للتشغيل، وتجدّد عقود إيجار الموصّلات، وتحرّر العقود عند الإيقاف السلس، وتتيح لـ platform/core استعادة العقود المنتهية بعد الأعطال.

</details>

<details>
<summary>التطوير المحلي (Neo4j مضمّن)</summary>

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
<summary>البنية المعمارية: كيف يهيكل RushDB البيانات (LMPG)</summary>

<div dir="rtl">

يستخدم RushDB نموذج **الرسم البياني الفائق للخصائص المُسمّى (Labeled Meta Property Graph - LMPG)**. تُرفَّع الخصائص لتصبح عقدًا من الدرجة الأولى في الرسم البياني ("HyperProperties") — وليست مجرد أزواج مفتاح-قيمة مرتبطة بالسجلات.

هذا يعني:

- **مخطط دون تصميم مسبق** — لأن الخصائص عقد في الرسم البياني، فإن المخطط _يُستنتج من بياناتك، ولا يُصمّم_: تُكتشف التسميات والأنواع ونطاقات القيم وطوبولوجيا العلاقات عند الكتابة وتكون قابلة للاستعلام فورًا — دون نمذجة مخطط يدوية، ودون سلسلة أدوات RDF/OWL
- **علاقات مكتشفة تلقائيًا** — تُربط السجلات التي تتشارك خصائص دون صياغة حواف يدويًا
- **استبطان المخطط** — يمكن للوكلاء تعداد التسميات وأنواع الخصائص ونطاقات القيم وطوبولوجيا العلاقات في استعلام واحد
- **قيود مرنة** — تقييم تماسك الأنواع، وتتبّع التعدّدية (cardinality)، وفرض أبعاد المتجهات دون مخططات صارمة مسبقة
- **سطح استعلام موحّد** — يعمل تعبير الترشيح نفسه عبر السجلات والتسميات والخصائص والعلاقات

يسترجع SearchQuery واحد عدة منظورات في آن واحد (السجلات + إحصاءات الخصائص + عمليات التجميع)، متجنّبًا نمط الفحص N+1 الشائع في معماريات الأنظمة المنفصلة.

[اقرأ منشور بنية LMPG الكامل ←](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture)

</div>

</details>

---

<div dir="rtl">

## التوثيق

</div>

| الموضوع                  | الرابط                                                         |
| ------------------------ | -------------------------------------------------------------- |
| البرنامج التعليمي السريع | https://docs.rushdb.com/get-started/quick-tutorial             |
| البحث المتجهي / الدلالي  | https://docs.rushdb.com/concepts/search/where#vector-operators |
| الترشيح والتنقل          | https://docs.rushdb.com/concepts/search/where                  |
| التجميع وعمليات التجميع  | https://docs.rushdb.com/concepts/search/group-by               |
| TypeScript SDK           | https://docs.rushdb.com/typescript-sdk/introduction            |
| Python SDK               | https://docs.rushdb.com/python-sdk/introduction                |
| REST API                 | https://docs.rushdb.com/rest-api/introduction                  |
| خادم MCP                 | packages/mcp-server/README.md                                  |
| مهارات الوكيل            | packages/skills/README.md                                      |

---

<div dir="rtl">

## متى لا تستخدم RushDB

- تحتاج إلى زمن استجابة دون المليّ ثانية عند معدّل كتابة عالٍ جدًا — بُني RushDB على Neo4j الذي يعطي الأولوية للاتساق وقدرة التعبير في الاستعلام على حساب سرعة الكتابة الخام.
- تحتاج فقط إلى تخزين مفتاح-قيمة مسطّح دون علاقات أو بحث دلالي — سيكون مخزن أبسط أخف.
- تحتاج إلى مخطط علائقي صارم مفروض على مستوى قاعدة البيانات — صُمّم RushDB عمدًا ليكون بلا مخطط.

## المساهمة

</div>

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
pnpm install
pnpm test
```

<div dir="rtl">

راجع [CONTRIBUTING.md](CONTRIBUTING.md) للاطلاع على الإرشادات الكاملة. المشكلات وطلبات السحب (PRs) مرحّب بها.

## الترخيص

</div>

| المسار                  | الترخيص             |
| ----------------------- | ------------------- |
| platform/core           | Elastic License 2.0 |
| platform/dashboard      | Elastic License 2.0 |
| docs                    | Apache 2.0          |
| website                 | Apache 2.0          |
| packages/javascript-sdk | Apache 2.0          |
| packages/mcp-server     | Apache 2.0          |

---

<div dir="rtl">

هل تحتاج إلى شيء غير مدعوم بعد؟ افتح مشكلة (issue) — مناقشات التصميم مرحّب بها.

</div>
