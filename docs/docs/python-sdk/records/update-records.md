---
sidebar_position: 6
---

# Update Records

## Partial Update

`db.records.update()`

Unspecified fields are preserved.

```python
# Update via record object
movie.update({"rating": 9.0})

# Update by ID
db.records.update(
    record_id=movie.__id,
    data={"rating": 9.0}
)
```

## Full Replacement

`db.records.set()`

All previous fields are removed, then replaced with the new data.

```python
# Set via record object
movie.set({"title": "Inception", "rating": 9.0, "genre": "sci-fi"})

# Set by ID
db.records.set(
    record_id=movie.__id,
    data={"title": "Inception", "rating": 9.0, "genre": "sci-fi"}
)
```

## Parameters

| Parameter | Type | Description |
|---|---|---|
| `record_id` | `str` | ID of the record to update |
| `data` | `dict` | Properties to write |
| `transaction` | `Transaction` | Optional transaction |

## With a transaction

```python
tx = db.transactions.begin()
try:
    db.records.update(record_id=movie.__id, data={"rating": 9.0}, transaction=tx)
    tx.commit()
except Exception:
    tx.rollback()
    raise
```


