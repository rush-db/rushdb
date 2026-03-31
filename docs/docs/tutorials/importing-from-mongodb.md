---
title: Importing from MongoDB
description: A step-by-step guide to migrating MongoDB collections to RushDB — nested documents, embedded arrays, upsert, and change streams
sidebar_position: 4
tags: [Data, MongoDB, Getting Started]
---

# Importing from MongoDB

This tutorial walks through moving MongoDB data into RushDB. It covers:

- **One-shot bulk import** — dump a collection and push it in one call
- **Embedded documents and arrays** — how `importJson` handles nesting automatically
- **Incremental sync with upsert** — run the same script on a schedule without creating duplicates
- **Cross-collection references** — link documents by ObjectId using `relationships.createMany`
- **Change streams** — react to live MongoDB writes and mirror them into RushDB

---

## Prerequisites

```bash
npm install @rushdb/javascript-sdk mongodb dotenv
```

```env
RUSHDB_API_KEY=your_rushdb_key
MONGO_URI=mongodb+srv://user:pass@cluster.mongodb.net/acme
```

---

## Why `importJson` instead of `createMany`?

MongoDB documents are rarely flat. A typical `users` document might look like:

```json
{
  "_id": "64f1a...",
  "name": "Ada Lovelace",
  "email": "ada@example.com",
  "address": { "city": "London", "country": "UK" },
  "orders": [
    { "_id": "64f2b...", "total": 149.99, "status": "shipped" },
    { "_id": "64f2c...", "total": 29.99,  "status": "delivered" }
  ]
}
```

`records.createMany` only accepts flat rows. `records.importJson` handles nested objects and arrays by recursively creating child records and linking them in the graph. The key you use for nested arrays becomes the **label** of the child records.

---

## 1. One-shot bulk import

The simplest path: dump a collection, reshape each document, push everything to RushDB.

```ts
import RushDB from '@rushdb/javascript-sdk'
import { MongoClient } from 'mongodb'

const db = new RushDB(process.env.RUSHDB_API_KEY!)

async function bulkImport() {
  const mongo = await MongoClient.connect(process.env.MONGO_URI!)
  const mdb = mongo.db()

  // ── 1. Fetch ────────────────────────────────────────────────
  const users = await mdb.collection('users').find({}).toArray()

  // ── 2. Reshape ──────────────────────────────────────────────
  //   • Convert _id (ObjectId) to a plain string
  //   • Name child-array keys after the label you want in RushDB
  const payload = users.map(u => ({
    mongoId: String(u._id),
    name: u.name,
    email: u.email,
    city: u.address?.city,
    country: u.address?.country,
    // Each element of 'Order' array becomes an Order record linked to this User
    Order: (u.orders ?? []).map((o: any) => ({
      mongoId: String(o._id),
      total: o.total,
      status: o.status
    }))
  }))

  // ── 3. Import ───────────────────────────────────────────────
  await db.records.importJson({
    label: 'User',
    data: payload,
    options: { suggestTypes: true, returnResult: false }
  })

  console.log(`Imported ${users.length} users with their orders`)
  await mongo.close()
}

bulkImport().catch(console.error)
```

After this runs, RushDB contains:
- One `User` record per MongoDB user document
- One `Order` record per embedded order, automatically linked to its parent user

---

## 2. Incremental sync with upsert

Add `mergeBy` and `mergeStrategy` to make subsequent runs idempotent. The script can run on a cron without creating duplicates.

```ts
async function incrementalSync() {
  const mongo = await MongoClient.connect(process.env.MONGO_URI!)
  const mdb = mongo.db()

  // Only fetch documents updated in the last hour
  const since = new Date(Date.now() - 60 * 60 * 1000)
  const users = await mdb.collection('users')
    .find({ updatedAt: { $gte: since } })
    .toArray()

  if (!users.length) {
    console.log('No updates since', since.toISOString())
    await mongo.close()
    return
  }

  const payload = users.map(u => ({
    mongoId: String(u._id),
    name: u.name,
    email: u.email,
    city: u.address?.city,
    country: u.address?.country,
    Order: (u.orders ?? []).map((o: any) => ({
      mongoId: String(o._id),
      total: o.total,
      status: o.status
    }))
  }))

  await db.records.importJson({
    label: 'User',
    data: payload,
    options: {
      suggestTypes: true,
      mergeBy: ['mongoId'],        // match existing records by mongoId
      mergeStrategy: 'append'     // update changed fields, keep everything else
    }
  })

  console.log(`Synced ${users.length} updated users`)
  await mongo.close()
}
```

:::info mergeStrategy options
- **`append`** — adds/updates provided fields, keeps any others already in RushDB. Best for incremental enrichment.
- **`rewrite`** — replaces all own properties with the incoming set. Best when RushDB should be an exact mirror of the source.
:::

---

## 3. Cross-collection references

When orders live in a separate collection and reference users by `userId` (an ObjectId), use the "import then link" pattern.

```ts
async function importWithReferences() {
  const mongo = await MongoClient.connect(process.env.MONGO_URI!)
  const mdb = mongo.db()

  const users  = await mdb.collection('users').find({}).toArray()
  const orders = await mdb.collection('orders').find({}).toArray()

  // 1) Import Users
  await db.records.createMany({
    label: 'User',
    data: users.map(u => ({
      mongoId: String(u._id),
      name: u.name,
      email: u.email
    })),
    options: { suggestTypes: true, mergeBy: ['mongoId'], mergeStrategy: 'append' }
  })

  // 2) Import Orders — preserve the reference as userMongoId for the join
  await db.records.createMany({
    label: 'Order',
    data: orders.map(o => ({
      mongoId: String(o._id),
      userMongoId: String(o.userId),   // foreign-key reference stored as plain string
      total: o.total,
      status: o.status,
      createdAt: o.createdAt?.toISOString()
    })),
    options: { suggestTypes: true, mergeBy: ['mongoId'], mergeStrategy: 'append' }
  })

  // 3) Link: User -[:PLACED]-> Order by joining mongoId = userMongoId
  await db.relationships.createMany({
    source: { label: 'User',  key: 'mongoId' },
    target: { label: 'Order', key: 'userMongoId' },
    type: 'PLACED',
    direction: 'out'
  })

  console.log('Import and link complete')
  await mongo.close()
}
```

:::tip Why store the reference as a string?
RushDB joins on property value equality. MongoDB's `ObjectId` must be converted to `String()` before storing so the join `User.mongoId = Order.userMongoId` works correctly — both sides must be the same type.
:::

---

## 4. Deeply nested collections

If your documents have multi-level nesting (e.g. orders containing line items containing product info), nest the keys accordingly. `importJson` handles arbitrary depth.

```ts
const payload = orders.map(o => ({
  mongoId: String(o._id),
  total: o.total,
  // 'LineItem' becomes the child label; each item gets its own record
  LineItem: (o.items ?? []).map((item: any) => ({
    mongoId: String(item._id),
    quantity: item.qty,
    unitPrice: item.price,
    // 'Product' becomes a grandchild record under LineItem
    Product: item.product ? [{
      mongoId: String(item.product._id),
      name: item.product.name,
      sku: item.product.sku
    }] : []
  }))
}))

await db.records.importJson({
  label: 'Order',
  data: payload,
  options: { suggestTypes: true, mergeBy: ['mongoId'], mergeStrategy: 'append' }
})
```

This produces the graph: `Order → LineItem → Product`.

---

## 5. Change streams (real-time sync)

MongoDB change streams let you mirror writes into RushDB as they happen, without polling.

```ts
import RushDB from '@rushdb/javascript-sdk'
import { MongoClient, ChangeStreamDocument } from 'mongodb'

const db = new RushDB(process.env.RUSHDB_API_KEY!)

async function watchCollection() {
  const mongo = await MongoClient.connect(process.env.MONGO_URI!)
  const collection = mongo.db().collection('users')

  const stream = collection.watch([], { fullDocument: 'updateLookup' })

  stream.on('change', async (event: ChangeStreamDocument) => {
    if (event.operationType === 'insert' || event.operationType === 'replace') {
      const doc = (event as any).fullDocument
      await db.records.upsert({
        label: 'User',
        data: {
          mongoId: String(doc._id),
          name: doc.name,
          email: doc.email,
          city: doc.address?.city
        },
        options: { suggestTypes: true, mergeBy: ['mongoId'], mergeStrategy: 'append' }
      })
    }

    if (event.operationType === 'update') {
      const doc = (event as any).fullDocument
      if (!doc) return  // fullDocument is null when not using updateLookup pipeline
      await db.records.upsert({
        label: 'User',
        data: {
          mongoId: String(doc._id),
          name: doc.name,
          email: doc.email,
          city: doc.address?.city
        },
        options: { suggestTypes: true, mergeBy: ['mongoId'], mergeStrategy: 'append' }
      })
    }

    if (event.operationType === 'delete') {
      // Optional: remove the record from RushDB when deleted from MongoDB
      const mongoId = String((event as any).documentKey._id)
      await db.records.delete({ labels: ['User'], where: { mongoId } })
    }
  })

  stream.on('error', err => console.error('Change stream error:', err))
  console.log('Watching users collection for changes...')
}

watchCollection().catch(console.error)
```

:::note Replica set required
Change streams require MongoDB to be running as a replica set (or MongoDB Atlas). They are not available on standalone `mongod` instances.
:::

---

## 6. Batching large collections

For collections with millions of documents, process in batches to avoid memory pressure and respect API rate limits.

```ts
async function importLargeCollection(batchSize = 500) {
  const mongo = await MongoClient.connect(process.env.MONGO_URI!)
  const collection = mongo.db().collection('products')

  let skip = 0
  let imported = 0

  while (true) {
    const batch = await collection
      .find({})
      .skip(skip)
      .limit(batchSize)
      .toArray()

    if (!batch.length) break

    const payload = batch.map(p => ({
      mongoId: String(p._id),
      sku: p.sku,
      name: p.name,
      price: p.price,
      category: p.category,
      tags: p.tags  // string[] — will be AI-indexed if an embedding index exists
    }))

    await db.records.createMany({
      label: 'Product',
      data: payload,
      options: { suggestTypes: true, mergeBy: ['mongoId'], mergeStrategy: 'append' }
    })

    imported += batch.length
    skip += batchSize
    console.log(`Imported ${imported} products`)
  }

  await mongo.close()
}
```

---

## 7. Python equivalent

```python
from pymongo import MongoClient
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")
mongo = MongoClient("mongodb+srv://...")
mdb = mongo["acme"]

# Fetch and reshape
users = list(mdb["users"].find({}))
payload = [
    {
        "mongoId": str(u["_id"]),
        "name": u.get("name"),
        "email": u.get("email"),
        "Order": [
            {"mongoId": str(o["_id"]), "total": o["total"], "status": o["status"]}
            for o in u.get("orders", [])
        ],
    }
    for u in users
]

# Upsert into RushDB
db.records.import_json(
    label="User",
    data=payload,
    options={"suggestTypes": True, "mergeBy": ["mongoId"], "mergeStrategy": "append"},
)

print(f"Imported {len(users)} users")
mongo.close()
```

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `createMany` throws "not a flat object" | Document has embedded objects or arrays | Use `records.importJson` instead |
| Child records created with wrong label | Nested array key name not matching desired label | Rename the key in the reshape step (e.g. rename `orders` → `Order`) |
| Duplicate records after re-import | `mergeBy` not set | Add `mergeBy: ['mongoId']` to options |
| Join not linking records | ObjectId not converted to string | Ensure both sides use `String(objectId)` |
| Change stream `fullDocument` is null | `updateLookup` not enabled or update is partial | Use `{ fullDocument: 'updateLookup' }` in the watch options |
| Import dies on large collections | Memory exhausted on `.toArray()` | Use cursor pagination with `.skip()` / `.limit()` batching |

## See also

- [Importing data from external sources](./importing-data) — HubSpot, Postgres, Firebase, Airtable, Notion
- TypeScript SDK: [Import Data](../typescript-sdk/records/import-data) · [Relationships](../typescript-sdk/relationships)
- Python SDK: [Import Data](../python-sdk/records/import-data) · [Relationships](../python-sdk/relationships)
