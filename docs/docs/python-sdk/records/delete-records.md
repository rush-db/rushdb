---
sidebar_position: 7
---

# Delete Records

## `db.records.delete_by_id()`

```python
# Single record
db.records.delete_by_id("movie-123")

# Multiple records
db.records.delete_by_id(["movie-123", "movie-456"])

# From record object
movie.delete()
```

## `db.records.delete()`

Delete all records matching a query.

```python
db.records.delete({
    "labels": ["MOVIE"],
    "where": {"rating": {"$lt": 5}}
})
```

:::warning
Calling `delete()` without a `where` clause deletes **all** records with the given label.
:::

## With a transaction

```python
tx = db.transactions.begin()
try:
    db.records.delete_by_id("movie-123", transaction=tx)
    tx.commit()
except Exception:
    tx.rollback()
    raise
```


