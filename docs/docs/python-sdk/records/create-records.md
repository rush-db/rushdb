---
sidebar_position: 1
---

# Create Records

Three methods for writing records. For nested/graph data see [Import Data](./import-data.md).

## `db.records.create()`

```python
movie = db.records.create(
    label="MOVIE",
    data={"title": "Inception", "rating": 8.8, "genre": "sci-fi"}
)
# → Record  { __id, __label, title, rating, genre }
```

## `db.records.create_many()`

Flat rows only — no nested objects. For nested data use [`import_json`](./import-data.md).

```python
result = db.records.create_many(
    label="ACTOR",
    data=[
        {"name": "Leonardo DiCaprio", "country": "USA"},
        {"name": "Ken Watanabe",      "country": "Japan"}
    ]
)
# → SearchResult  { data: [...], total: 2 }
```

## `db.records.upsert()`

Create-or-update based on matching criteria.

```python
# Match on 'title'; update rating if found, create if not
movie = db.records.upsert(
    label="MOVIE",
    data={"title": "Inception", "rating": 9.0, "genre": "sci-fi"},
    options={"mergeBy": ["title"], "mergeStrategy": "append"}
)
```

### Merge strategies

| Strategy | Behaviour |
|---|---|
| `append` (default) | Add / update incoming fields; preserve all other existing fields |
| `rewrite` | Replace all fields with incoming data; unmentioned fields are removed |

### `mergeBy` behaviour

| `mergeBy` value | Match behaviour |
|---|---|
| `['field']` | Match only on listed fields |
| `[]` or omitted | Match on ALL incoming property keys |

## Options

| Option | Default | Description |
|---|---|---|
| `suggestTypes` | `True` | Infer property types automatically |
| `convertNumericValuesToNumbers` | `False` | Convert string numbers to number type |
| `capitalizeLabels` | `False` | Uppercase all inferred label names |
| `relationshipType` | `__RUSHDB__RELATION__DEFAULT__` | Relationship type for nested links |
| `returnResult` | `True` | Return created records in response |
| `mergeBy` | — | Fields to match on for upsert |
| `mergeStrategy` | `append` | `append` or `rewrite` |

## With a transaction

```python
tx = db.transactions.begin()
try:
    movie = db.records.create(
        label="MOVIE",
        data={"title": "Inception"},
        transaction=tx
    )
    actor = db.records.create(
        label="ACTOR",
        data={"name": "Leonardo DiCaprio"},
        transaction=tx
    )
    db.relationships.attach(
        source=movie,
        target=actor,
        options={"type": "STARS"},
        transaction=tx
    )
    tx.commit()
except Exception:
    tx.rollback()
    raise
```


