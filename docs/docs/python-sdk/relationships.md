---
sidebar_position: 3
---

# Relationships

```python
# Leo acted in Inception
db.records.attach(
    source=leo,
    target=inception,
    options={"type": "ACTED_IN"}
)

# Detach
db.records.detach(
    source=leo,
    target=inception,
    options={"type": "ACTED_IN"}
)
```

## `attach()`

```python
# With direction
db.records.attach(
    source=movie,
    target=actor,
    options={"type": "STARS_IN", "direction": "out"}
)

# One-to-many (target list)
db.records.attach(
    source=movie,
    target=[actor1, actor2, actor3],
    options={"type": "STARS_IN"}
)
```

## `detach()`

```python
db.records.detach(
    source=movie,
    target=actor,
    options={"type": "STARS_IN"}
)
```

## Direction

| Value | Meaning |
|---|---|
| `"out"` | source → target |
| `"in"` | target → source |

## With a transaction

```python
tx = db.transactions.begin()
try:
    db.records.attach(source=movie, target=actor, options={"type": "STARS_IN"}, transaction=tx)
    tx.commit()
except Exception:
    tx.rollback()
    raise
```

For traversal in queries, see [Get Records — Relationship traversal](./records/get-records.md#relationship-traversal).


