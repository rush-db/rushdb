---
slug: /reference/python/transaction
sidebar_position: 1
---

# Transaction

A `Transaction` groups multiple database operations so they succeed or fail together. Obtain one via `db.tx.begin()`, then pass it to any write method. Call `commit()` to make changes permanent or `rollback()` to discard them.

## Class Definition

```python
class Transaction:
    def __init__(self, client: "RushDB", transaction_id: str)
```

## Properties

### id

```python
id: str
```

The server-assigned transaction identifier. Passed automatically via `X-Transaction-Id` header when you supply the `Transaction` object to any API method.

## Instance Methods

### `commit()`

```python
def commit() -> None
```

Makes all operations performed within this transaction permanent. Raises `RushDBError` if the transaction has already been committed or rolled back.

### `rollback()`

```python
def rollback() -> None
```

Discards all operations performed within this transaction. Raises `RushDBError` if the transaction has already been committed or rolled back.

## Context Manager

`Transaction` implements `__enter__` / `__exit__`, so you can use it with `with`:

```python
with db.tx.begin() as tx:
    db.records.create("User", {"name": "Alice"}, transaction=tx)
    db.records.create("User", {"name": "Bob"}, transaction=tx)
# auto-commits on success; auto-rolls back if an exception is raised
```

## `db.tx` — TransactionsAPI

Transactions are created and managed through the `db.tx` namespace.

### `begin()`

```python
def begin(ttl: Optional[int] = None) -> Transaction
```

Starts a new transaction. Returns a `Transaction` object.

| Parameter | Type  | Description                                                                                |
| --------- | ----- | ------------------------------------------------------------------------------------------ |
| `ttl`     | `int` | Time-to-live in milliseconds before the transaction auto-expires. Defaults to `5000` (5 s) |

```python
tx = db.tx.begin(ttl=30000)  # 30-second window
```

### `get()`

```python
def get(transaction: Union[str, Transaction]) -> Transaction
```

Retrieves an existing transaction by its ID or `Transaction` object. Useful when the original object is unavailable (e.g. recovered from storage or passed across a process boundary).

### `commit()`

```python
def commit(transaction: Union[str, Transaction]) -> None
```

Commits a transaction identified by a `Transaction` object or a raw ID string. Equivalent to calling `tx.commit()` on the object itself.

### `rollback()`

```python
def rollback(transaction: Union[str, Transaction]) -> None
```

Rolls back a transaction identified by a `Transaction` object or a raw ID string.

## Usage Examples

### Manual commit / rollback

```python
tx = db.tx.begin(ttl=10000)
try:
    db.records.create("User", {"name": "Alice"}, transaction=tx)
    db.records.create("User", {"name": "Bob"}, transaction=tx)
    tx.commit()
except Exception:
    tx.rollback()
    raise
```

### Context manager (recommended)

```python
with db.tx.begin() as tx:
    user = db.records.create("User", {"name": "Alice"}, transaction=tx)
    post = db.records.create("Post", {"title": "Hello"}, transaction=tx)
    db.records.attach(user, post, options={"type": "AUTHORED"}, transaction=tx)
```

### Recover a transaction by ID

```python
tx_id = "some-saved-id"
tx = db.tx.get(tx_id)
tx.rollback()
```
