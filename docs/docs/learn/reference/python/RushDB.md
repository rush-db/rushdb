---
slug: /reference/python/RushDB
sidebar_position: 0
---

# RushDB

The `RushDB` class is the main entry point for interacting with RushDB from Python. It manages authentication, HTTP communication, and exposes all API namespaces.

## Initialization

```python
from rushdb import RushDB

# Connect to RushDB Cloud
db = RushDB("RUSHDB_API_KEY")

# Connect to a self-hosted instance
db = RushDB(
    api_key="RUSHDB_API_KEY",
    url="https://your-rushdb-instance.com"
)
```

## Constructor

```python
RushDB(api_key: str, url: Optional[str] = None, base_url: Optional[str] = None)
```

| Parameter  | Type  | Description                                                                                                                                                         |
| ---------- | ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `api_key`  | `str` | API key for authentication                                                                                                                                          |
| `url`      | `str` | Base URL of a self-hosted RushDB instance. When provided without `/api/`, the path `/api/v1` is appended automatically. Defaults to `https://api.rushdb.com/api/v1` |
| `base_url` | `str` | Deprecated alias for `url`. Kept for backwards compatibility                                                                                                        |

## Namespaces

All database operations are accessed through sub-namespaces on the client instance.

### `db.records`

CRUD and bulk operations on records.

| Method                                                                    | Description                                                |
| ------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `create(label, data, *, options, vectors, transaction)`                   | Create a single record                                     |
| `create_many(label, data, *, options, vectors, transaction)`              | Create multiple flat records                               |
| `import_json(data, label, *, options, transaction)`                       | Import nested/complex JSON payloads                        |
| `import_csv(label, data, *, options, parse_config, vectors, transaction)` | Import records from CSV text                               |
| `upsert(data, label, *, options, vectors, transaction)`                   | Create or update a record                                  |
| `set(target, data, *, label, vectors, transaction)`                       | Replace all fields of a record                             |
| `update(target, data, *, transaction)`                                    | Partially update a record                                  |
| `find(search_query, *, record_id, transaction)`                           | Search records; returns `SearchResult`                     |
| `find_one(search_query, *, transaction)`                                  | Return the first match or `None`                           |
| `find_uniq(search_query, *, transaction)`                                 | Return the single match or `None`; raises if more than one |
| `find_by_id(target, *, transaction)`                                      | Fetch record(s) by ID                                      |
| `delete(search_query, *, transaction)`                                    | Delete all records matching a query                        |
| `delete_by_id(target, *, transaction)`                                    | Delete record(s) by ID                                     |
| `attach(source, target, *, options, transaction)`                         | Create relationships between records                       |
| `detach(source, target, *, options, transaction)`                         | Remove relationships between records                       |
| `export(search_query, *, transaction)`                                    | Export matching records as CSV text                        |

### `db.tx`

Transaction lifecycle management.

| Method                  | Description                                    |
| ----------------------- | ---------------------------------------------- |
| `begin(ttl)`            | Start a new transaction; returns `Transaction` |
| `get(transaction)`      | Retrieve an existing transaction by ID         |
| `commit(transaction)`   | Commit a transaction by object or ID           |
| `rollback(transaction)` | Roll back a transaction by object or ID        |

See also: [Transaction](/reference/python/transaction) for the `commit()` / `rollback()` instance methods and context manager usage.

### `db.labels`

Discover record labels in the database.

| Method                               | Description                                          |
| ------------------------------------ | ---------------------------------------------------- |
| `find(search_query, *, transaction)` | Return a dict of `{label: count}` matching the query |

### `db.properties`

Inspect property metadata.

| Method                                                   | Description                          |
| -------------------------------------------------------- | ------------------------------------ |
| `find(search_query, *, transaction)`                     | List all properties matching a query |
| `find_by_id(property_id, *, transaction)`                | Retrieve a property by ID            |
| `delete(property_id, *, transaction)`                    | Delete a property by ID              |
| `find_values(property_id, *, search_query, transaction)` | List distinct values for a property  |

### `db.relationships`

Query and bulk-create relationships.

| Method                                                                       | Description                                                 |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `find(search_query, *, pagination, transaction)`                             | Search relationships; returns `List[Relationship]`          |
| `create_many(*, source, target, type, direction, many_to_many, transaction)` | Bulk-create relationships by key-match or cartesian product |

### `db.ai`

Semantic search and ontology exploration. Embedding index management is available under `db.ai.indexes`.

| Method                                          | Description                                                             |
| ----------------------------------------------- | ----------------------------------------------------------------------- |
| `search(params)`                                | Semantic (vector) search over indexed properties; returns `ApiResponse` |
| `get_ontology(params, *, transaction)`          | Return the full graph ontology as structured JSON                       |
| `get_ontology_markdown(params, *, transaction)` | Return the ontology as compact Markdown (token-efficient, for LLMs)     |

#### `db.ai.indexes`

| Method                             | Description                                           |
| ---------------------------------- | ----------------------------------------------------- |
| `find()`                           | List all embedding index policies                     |
| `create(params)`                   | Create a new embedding index for a property           |
| `delete(index_id)`                 | Delete an embedding index by ID                       |
| `stats(index_id)`                  | Get Neo4j-level statistics for an index               |
| `upsert_vectors(index_id, params)` | Bulk-seed an external index with pre-computed vectors |

### `db.query`

Raw Cypher query execution. **Cloud-only** — not available on self-hosted instances without a managed database.

| Method                      | Description                                                |
| --------------------------- | ---------------------------------------------------------- |
| `raw(body, *, transaction)` | Execute a raw Cypher query string with optional parameters |

### `db.settings`

Project configuration.

| Method  | Description                           |
| ------- | ------------------------------------- |
| `get()` | Retrieve the current project settings |

## Instance Methods

### `ping()`

```python
def ping() -> bool
```

Tests connectivity to the RushDB server. Returns `True` if the server is reachable, `False` otherwise. Safe to use in conditional checks — never raises.

```python
if db.ping():
    print("Connected")
else:
    raise RuntimeError("Cannot reach RushDB server")
```

## Usage Example

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

# Create records
user = db.records.create("User", {"name": "Alice", "email": "alice@example.com"})
post = db.records.create("Post", {"title": "Hello World", "published": True})

# Connect them
db.records.attach(user, post, options={"type": "AUTHORED"})

# Query
results = db.records.find({
    "labels": ["User"],
    "where": {"email": {"$contains": "@example.com"}},
    "limit": 10
})

for record in results:
    print(record["name"])

# Transaction
tx = db.tx.begin(ttl=10000)
try:
    db.records.create("Log", {"event": "login"}, transaction=tx)
    tx.commit()
except Exception:
    tx.rollback()

# Or use the context manager — auto-commits on success, auto-rolls back on error
with db.tx.begin() as tx:
    db.records.create("Log", {"event": "login"}, transaction=tx)
```
