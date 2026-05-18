---
sidebar_position: 6
---

# Transactions

Group writes atomically — all succeed or all roll back.

## Context manager (idiomatic)

```python
# Auto-commit on success, auto-rollback on exception
with db.tx.begin() as tx:
    leo = db.records.create(label="ACTOR", data={"name": "Leonardo DiCaprio"}, transaction=tx)
    inception = db.records.create(label="MOVIE", data={"title": "Inception"}, transaction=tx)
    db.records.attach(source=leo, target=inception, options={"type": "ACTED_IN"}, transaction=tx)
# committed automatically — no explicit commit() call needed
```

## Manual commit / rollback

```python
tx = db.tx.begin()
try:
    movie = db.records.create(label="MOVIE", data={"title": "Inception"}, transaction=tx)
    actor = db.records.create(label="ACTOR", data={"name": "Leonardo DiCaprio"}, transaction=tx)
    db.records.attach(source=movie, target=actor, options={"type": "STARS_IN"}, transaction=tx)
    tx.commit()
except Exception:
    tx.rollback()
    raise
```

## API

| Method              | Description             |
| ------------------- | ----------------------- |
| `db.tx.begin(ttl?)` | Start a new transaction |
| `tx.commit()`       | Persist all operations  |
| `tx.rollback()`     | Discard all operations  |

## Timeouts

| Setting     | Value    |
| ----------- | -------- |
| Default TTL | 5000 ms  |
| Maximum TTL | 30000 ms |

```python
tx = db.tx.begin(ttl=15000)   # 15 s timeout
```

## Supported operations

`create` · `create_many` · `update` · `set` · `delete` · `delete_by_id` · `attach` · `detach` · `find`
