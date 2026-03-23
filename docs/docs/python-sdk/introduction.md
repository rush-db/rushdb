---
title: Introduction
sidebar_position: 0
---

# Python SDK

Push JSON, query by value or meaning, traverse graphs — from Python.

## Install

```bash
pip install rushdb
```

## Connect

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")
```

Get your API token from the [RushDB Dashboard](https://app.rushdb.com/).

## First write

```python
# Nested objects become linked records automatically
db.records.create_many(
    label="MOVIE",
    data={
        "title": "Inception",
        "rating": 8.8,
        "genre": "sci-fi",
        "ACTOR": [
            {"name": "Leonardo DiCaprio", "country": "USA"},
            {"name": "Ken Watanabe",      "country": "Japan"}
        ]
    }
)
# Created: MOVIE → ACTOR × 2 (relationships wired automatically)
```

## First read

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "where": {"rating": {"$gte": 8}},
    "limit": 10
})

for movie in result:
    print(movie["title"])

print(f"{result.total} total")
```

## Configuration

| Parameter | Default | Description |
|---|---|---|
| `api_key` | — | Your RushDB API token (required) |
| `url` | `https://app.rushdb.com` | RushDB instance URL |

```python
# Self-hosted instance
db = RushDB("RUSHDB_API_KEY", url="https://your-rushdb-instance.com")
```

## Namespaces

| Namespace | Purpose |
|---|---|
| `db.records` | Create, read, update, delete records |
| `db.relationships` | Attach / detach record links |
| `db.labels` | List labels in the database |
| `db.properties` | Inspect property metadata |
| `db.transactions` | Begin / commit / rollback |
| `db.ai` | Ontology, embedding indexes, semantic search |


