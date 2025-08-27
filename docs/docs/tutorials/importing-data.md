---
title: Importing data from external sources
description: Learn how to import your data to RushDB
sidebar_position: 6
---

# Importing data from external sources

RushDB provides comprehensive toolkit to import data. While most of the data sources operate with flat tabular
data and context awareness emerges at query time, in RushDB instead relationships are static because of graph nature.

This guide will help you to import your data and make it breathe: relationships, types inferring and records itself are
created easily.

What you'll use:
- `records.createMany` (JSON and CSV import)
- `relationships.createMany` (bulk linking by key match)

Tip: You can do the same via REST. See REST docs: Records Import and Relationships API.

## Core pattern: import first, then link by external keys

Most external systems already have stable identifiers (MongoDB's ObjectId, HubSpot record IDs, SQL primary/foreign keys).
When importing to RushDB, store those external IDs on your records (e.g., `mongoId`, `hubspotId`, `pgId`). Then create
relationships by matching those keys using `relationships.createMany`:

1) Import data (keep external IDs as properties).
2) Create relationships by joining `source[key] = target[key]`.

Safeguards and notes
- You control the relationship `type` and `direction` (default direction is `out`).
- For key-based creation, provide both `source.key` and `target.key`.
- Only use `manyToMany` when you explicitly want a cartesian link across filtered sets.

---

## 1) MongoDB → RushDB

Goal: Import MongoDB collections (e.g., `users`, `orders`) and connect them using Mongo's ObjectId values.

Recommended mapping
- Persist the original `_id` as a string field on the RushDB record: `mongoId`.
- For references (e.g., `orders.userId`), persist as `userMongoId` so you can join `users.mongoId = orders.userMongoId`.

### Example: TypeScript (Node.js)

```ts
import RushDB from '@rushdb/javascript-sdk'
import { MongoClient, ObjectId } from 'mongodb'

const db = new RushDB(process.env.RUSHDB_API_KEY!)

async function run() {
	const mongo = await MongoClient.connect(process.env.MONGO_URI!)
	const mdb = mongo.db('acme')

	// 1) Extract from Mongo
	const users = await mdb.collection('users').find({ tenantId: 'ACME' }).toArray()
	const orders = await mdb.collection('orders').find({ tenantId: 'ACME' }).toArray()

	// 2) Normalize docs for RushDB
	const usersPayload = users.map(u => ({
		mongoId: String(u._id), // keep external id as string
		tenantId: u.tenantId,
		name: u.name,
		email: u.email
	}))

	const ordersPayload = orders.map(o => ({
		mongoId: String(o._id),
		tenantId: o.tenantId,
		total: o.total,
		// capture the referenced user id for later linking
		userMongoId: String(o.userId instanceof ObjectId ? o.userId : new ObjectId(o.userId))
	}))

	// 3) Import into RushDB
	await db.records.createMany({ label: 'USER', data: usersPayload })
	await db.records.createMany({ label: 'ORDER', data: ordersPayload })

	// 4) Link: USER -[:ORDERED]-> ORDER using mongo ids
	await db.relationships.createMany({
		source: { label: 'USER', key: 'mongoId', where: { tenantId: 'ACME' } },
		target: { label: 'ORDER', key: 'userMongoId', where: { tenantId: 'ACME' } },
		type: 'ORDERED',
		direction: 'out'
	})

	await mongo.close()
}

run().catch(console.error)
```

### Example: REST (create-many)

```json
POST /api/v1/relationships/create-many
{
	"source": { "label": "USER", "key": "mongoId", "where": { "tenantId": "ACME" } },
	"target": { "label": "ORDER", "key": "userMongoId", "where": { "tenantId": "ACME" } },
	"type": "ORDERED",
	"direction": "out"
}
```

Common pitfalls
- Ensure you convert `ObjectId` to string when storing in RushDB; the join is string equality.
- Keep tenant/workspace scoping in your `where` filters to avoid cross-tenant links.

---

## 2) HubSpot → RushDB

Goal: Import HubSpot objects (Contacts, Companies, Deals) and connect them using HubSpot IDs.

Recommended mapping
- Store the HubSpot object ID on the record (e.g., `hubspotId`).
- For associations, store the associated object’s HubSpot ID on the related record (e.g., a Deal with `companyHubspotId`).

### Example: TypeScript (using official HubSpot client)

```ts
import RushDB from '@rushdb/javascript-sdk'
import Hubspot from '@hubspot/api-client'

const db = new RushDB(process.env.RUSHDB_API_KEY!)
const hubspot = new Hubspot.Client({ accessToken: process.env.HUBSPOT_TOKEN! })

async function importHubspot() {
	// 1) Fetch Contacts and Companies
	const contactsRes = await hubspot.crm.contacts.basicApi.getPage(100, undefined, ['email'])
	const companiesRes = await hubspot.crm.companies.basicApi.getPage(100, undefined, ['name', 'domain'])

	const contacts = contactsRes.results.map(c => ({
		hubspotId: c.id,
		email: c.properties?.email,
		tenantId: 'ACME'
	}))

	const companies = companiesRes.results.map(co => ({
		hubspotId: co.id,
		name: co.properties?.name,
		domain: co.properties?.domain,
		tenantId: 'ACME'
	}))

	// 2) Import
	await db.records.createMany({ label: 'HS_CONTACT', data: contacts })
	await db.records.createMany({ label: 'HS_COMPANY', data: companies })

	// 3) Associate Contacts to Companies by joining HubSpot IDs
	// If you exported associations separately, persist contact.companyHubspotId on contact
	// Example join: HS_CONTACT.companyHubspotId = HS_COMPANY.hubspotId
	await db.relationships.createMany({
		source: { label: 'HS_CONTACT', key: 'companyHubspotId', where: { tenantId: 'ACME' } },
		target: { label: 'HS_COMPANY', key: 'hubspotId', where: { tenantId: 'ACME' } },
		type: 'WORKS_AT',
		direction: 'out'
	})
}

importHubspot().catch(console.error)
```

Alternative: Deals to Companies

```ts
await db.relationships.createMany({
	source: { label: 'HS_DEAL', key: 'companyHubspotId', where: { tenantId: 'ACME' } },
	target: { label: 'HS_COMPANY', key: 'hubspotId', where: { tenantId: 'ACME' } },
	type: 'RELATED_TO',
	direction: 'out'
})
```

Notes
- HubSpot v3 uses string IDs; storing them verbatim is fine for equality joins.
- If you rely on HubSpot association APIs, mirror those association IDs onto one side to enable the key match.

---

## 3) PostgreSQL → RushDB

Goal: Import relational tables (e.g., `users`, `orders`) and connect them using primary/foreign keys.

Recommended mapping
- Store the SQL primary key as `pgId` (for `users`) and the foreign key as `userPgId` (for `orders`).
	Then join `USER.pgId = ORDER.userPgId`.

### Example: TypeScript (Node.js) using `pg`

```ts
import RushDB from '@rushdb/javascript-sdk'
import { Client } from 'pg'

const db = new RushDB(process.env.RUSHDB_API_KEY!)

async function importPg() {
	const client = new Client({ connectionString: process.env.PG_URI })
	await client.connect()

	// 1) Extract
	const usersRes = await client.query('select id, name, email, tenant_id from users where tenant_id = $1', ['ACME'])
	const ordersRes = await client.query('select id, user_id, total, tenant_id from orders where tenant_id = $1', ['ACME'])

	// 2) Normalize
	const users = usersRes.rows.map(r => ({ pgId: String(r.id), name: r.name, email: r.email, tenantId: r.tenant_id }))
	const orders = ordersRes.rows.map(r => ({ pgId: String(r.id), userPgId: String(r.user_id), total: r.total, tenantId: r.tenant_id }))

	// 3) Import
	await db.records.createMany({ label: 'USER', data: users })
	await db.records.createMany({ label: 'ORDER', data: orders })

	// 4) Link: USER -[:ORDERED]-> ORDER by key equality
	await db.relationships.createMany({
		source: { label: 'USER', key: 'pgId', where: { tenantId: 'ACME' } },
		target: { label: 'ORDER', key: 'userPgId', where: { tenantId: 'ACME' } },
		type: 'ORDERED',
		direction: 'out'
	})

	await client.end()
}

importPg().catch(console.error)
```

### CSV path (no code runtime)
If you export tables to CSV, you can import with REST `POST /api/v1/records/import/csv` or SDK `records.createMany`, then run the same `relationships.createMany` call as above by joining the columns you preserved (e.g., `pgId` and `userPgId`).

---

## 4) Supabase → RushDB

Supabase uses PostgreSQL under the hood, so the mapping mirrors the PostgreSQL example. If you prefer the Supabase client:

```ts
import RushDB from '@rushdb/javascript-sdk'
import { createClient } from '@supabase/supabase-js'

const db = new RushDB(process.env.RUSHDB_API_KEY!)
const supabase = createClient(process.env.SUPABASE_URL!, process.env.SUPABASE_SERVICE_ROLE_KEY!)

async function importSupabase() {
	// 1) Extract
	const { data: users, error: uerr } = await supabase
		.from('users')
		.select('id,name,email,tenant_id')
		.eq('tenant_id', 'ACME')
	if (uerr) throw uerr

	const { data: orders, error: oerr } = await supabase
		.from('orders')
		.select('id,user_id,total,tenant_id')
		.eq('tenant_id', 'ACME')
	if (oerr) throw oerr

	// 2) Normalize
	const usersPayload = (users ?? []).map(r => ({ pgId: String(r.id), name: r.name, email: r.email, tenantId: r.tenant_id }))
	const ordersPayload = (orders ?? []).map(r => ({ pgId: String(r.id), userPgId: String(r.user_id), total: r.total, tenantId: r.tenant_id }))

	// 3) Import and link
	await db.records.createMany({ label: 'USER', data: usersPayload })
	await db.records.createMany({ label: 'ORDER', data: ordersPayload })
	await db.relationships.createMany({
		source: { label: 'USER', key: 'pgId', where: { tenantId: 'ACME' } },
		target: { label: 'ORDER', key: 'userPgId', where: { tenantId: 'ACME' } },
		type: 'ORDERED',
		direction: 'out'
	})
}

importSupabase().catch(console.error)
```

---

## 5) Firebase (Firestore) → RushDB

Map Firestore document IDs to a stable key. Example with collections `users` and `orders` (each order has `userId` that equals a user doc id):

```ts
import RushDB from '@rushdb/javascript-sdk'
import admin from 'firebase-admin'

const db = new RushDB(process.env.RUSHDB_API_KEY!)

admin.initializeApp({
	credential: admin.credential.applicationDefault(),
	projectId: process.env.GCLOUD_PROJECT
})

const fs = admin.firestore()

async function importFirestore() {
	// 1) Fetch
	const usersSnap = await fs.collection('users').where('tenantId', '==', 'ACME').get()
	const ordersSnap = await fs.collection('orders').where('tenantId', '==', 'ACME').get()

	// 2) Normalize
	const users = usersSnap.docs.map(d => ({
		firebaseId: d.id,
		tenantId: d.get('tenantId'),
		name: d.get('name'),
		email: d.get('email')
	}))

	const orders = ordersSnap.docs.map(d => ({
		firebaseId: d.id,
		tenantId: d.get('tenantId'),
		total: d.get('total'),
		userFirebaseId: String(d.get('userId')) // reference to users doc id
	}))

	// 3) Import and link
	await db.records.createMany({ label: 'USER', data: users })
	await db.records.createMany({ label: 'ORDER', data: orders })
	await db.relationships.createMany({
		source: { label: 'USER', key: 'firebaseId', where: { tenantId: 'ACME' } },
		target: { label: 'ORDER', key: 'userFirebaseId', where: { tenantId: 'ACME' } },
		type: 'ORDERED',
		direction: 'out'
	})
}

importFirestore().catch(console.error)
```

Notes
- For multi-tenant Firestore, include a `tenantId` field and filter `where` accordingly.
- If orders reference users via DocumentReference objects, resolve to `ref.id` when building the payload.

---

## 6) Airtable → RushDB

Use Airtable record IDs for joins. Example: link Contacts to Companies where a Contact has a single `companyId` field storing the linked record ID.

```ts
import RushDB from '@rushdb/javascript-sdk'
import Airtable from 'airtable'

const db = new RushDB(process.env.RUSHDB_API_KEY!)
const base = new Airtable({ apiKey: process.env.AIRTABLE_TOKEN! }).base(process.env.AIRTABLE_BASE_ID!)

async function importAirtable() {
	// 1) Fetch
	const companiesTable = base('Companies')
	const contactsTable = base('Contacts')

	const companies = await companiesTable.select({ pageSize: 100 }).all()
	const contacts = await contactsTable.select({ pageSize: 100 }).all()

	// 2) Normalize
	const companiesPayload = companies.map(r => ({
		airtableId: r.id,
		tenantId: 'ACME',
		name: r.get('Name') as string,
		domain: (r.get('Domain') as string) || undefined
	}))

	const contactsPayload = contacts.map(r => ({
		airtableId: r.id,
		tenantId: 'ACME',
		name: r.get('Name') as string,
		email: (r.get('Email') as string) || undefined,
		// If you use Airtable "Link to another record", it returns an array of record IDs
		companyAirtableId: Array.isArray(r.get('Company')) && (r.get('Company') as string[])[0]
			? (r.get('Company') as string[])[0]
			: undefined
	}))

	// 3) Import and link
	await db.records.createMany({ label: 'AT_COMPANY', data: companiesPayload })
	await db.records.createMany({ label: 'AT_CONTACT', data: contactsPayload })

	await db.relationships.createMany({
		source: { label: 'AT_CONTACT', key: 'companyAirtableId', where: { tenantId: 'ACME' } },
		target: { label: 'AT_COMPANY', key: 'airtableId', where: { tenantId: 'ACME' } },
		type: 'WORKS_AT',
		direction: 'out'
	})
}

importAirtable().catch(console.error)
```

Notes
- If a contact can link to multiple companies, iterate those IDs and use `records.attach` per contact, or pre-expand into multiple joinable rows.

---

## 7) Notion → RushDB

Use Notion page IDs for joins. Example: People and Tasks databases; each Task has a single-person relation stored in `assignee`.

```ts
import RushDB from '@rushdb/javascript-sdk'
import { Client } from '@notionhq/client'

const db = new RushDB(process.env.RUSHDB_API_KEY!)
const notion = new Client({ auth: process.env.NOTION_TOKEN! })

async function importNotion() {
	const peopleDbId = process.env.NOTION_PEOPLE_DB_ID!
	const tasksDbId = process.env.NOTION_TASKS_DB_ID!

	// 1) Fetch
	const peopleRes = await notion.databases.query({ database_id: peopleDbId })
	const tasksRes = await notion.databases.query({ database_id: tasksDbId })

	// 2) Normalize
	const people = peopleRes.results.map(p => ({
		notionId: p.id,
		tenantId: 'ACME',
		name: (p as any).properties?.Name?.title?.[0]?.plain_text || 'Unknown'
	}))

	const tasks = tasksRes.results.map(t => {
		const props = (t as any).properties
		const assignees = props?.assignee?.relation as Array<{ id: string }> | undefined
		const firstAssigneeId = assignees && assignees.length ? assignees[0].id : undefined
		return {
			notionId: t.id,
			tenantId: 'ACME',
			title: props?.Name?.title?.[0]?.plain_text || 'Untitled',
			assigneeNotionId: firstAssigneeId
		}
	})

	// 3) Import and link (single-assignee example)
	await db.records.createMany({ label: 'NT_PERSON', data: people })
	await db.records.createMany({ label: 'NT_TASK', data: tasks })

	await db.relationships.createMany({
		source: { label: 'NT_TASK', key: 'assigneeNotionId', where: { tenantId: 'ACME' } },
		target: { label: 'NT_PERSON', key: 'notionId', where: { tenantId: 'ACME' } },
		type: 'ASSIGNED_TO',
		direction: 'out'
	})
}

importNotion().catch(console.error)
```

Notes
- If a Task can have multiple assignees, either:
  - iterate assignee IDs and call `records.attach` per Task, or
  - pre-expand into multiple Task rows (each with a single `assigneeNotionId`) before import to keep `createMany`-by-key workflow.

---

## Python equivalents (quick reference)

Below are the equivalent Python SDK calls once you have your lists of dicts ready:

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

# Import
db.records.create_many(label="USER", data=users)
db.records.create_many(label="ORDER", data=orders)

# Link by key equality
db.relationships.create_many(
		source={"label": "USER", "key": "mongoId", "where": {"tenantId": "ACME"}},
		target={"label": "ORDER", "key": "userMongoId", "where": {"tenantId": "ACME"}},
		type="ORDERED",
		direction="out",
)
```

---

## Troubleshooting

- Mismatched types: Ensure the join keys are the same type (strings are safest). Convert DB-specific IDs to strings before import.
- Missing keys: Key-based mode requires both `source.key` and `target.key`. If you truly need cartesian linking, set `manyToMany: true` and provide non-empty `where` on both sides.
- Scope filters: Always restrict with `where` (e.g., `tenantId`) to avoid unintended cross-linking.

## See also

- TypeScript SDK: [Relationships](../typescript-sdk/relationships) · [Import Data](../typescript-sdk/records/import-data)
- Python SDK: [Relationships](../python-sdk/relationships) · [Import Data](../python-sdk/records/import-data)
- REST API: [Relationships API](../rest-api/relationships) · [Records Import](../rest-api/records/import-data)
