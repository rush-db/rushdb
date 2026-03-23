---
sidebar_position: 6
---

# Transactions

Group multiple operations into an atomic unit — all succeed or all roll back.

## Basic pattern

```typescript
const tx = await db.tx.begin()
// optional: { ttl: 10000 }  — default 5000ms, max 30000ms

try {
  const movie = await db.records.create(
    { label: 'MOVIE', data: { title: 'Dune', rating: 8.0 } }, tx)

  const actor = await db.records.create(
    { label: 'ACTOR', data: { name: 'Timothée Chalamet' } }, tx)

  await db.records.attach(
    { source: movie, target: actor, options: { type: 'STARS' } }, tx)

  await tx.commit()          // or: await db.tx.commit(tx)
} catch (e) {
  await tx.rollback()        // or: await db.tx.rollback(tx)
  throw e
}
```

Pass `tx` as the last argument to any record/relationship method.

## API

| Method | Description |
|---|---|
| `db.tx.begin({ ttl? })` | Start a transaction. Returns a `Transaction` object with `.id` |
| `db.tx.get(tx)` | Check if a transaction still exists |
| `db.tx.commit(tx)` | Commit — makes all changes permanent |
| `db.tx.rollback(tx)` | Rollback — discards all changes |
| `tx.commit()` / `tx.rollback()` | Shorthand on the transaction object itself |

## Timeouts

Uncommitted transactions auto-rollback after the TTL expires.

| Parameter | Value |
|---|---|
| Default TTL | 5 000 ms |
| Maximum TTL | 30 000 ms |
      type: "LIVES_AT",
