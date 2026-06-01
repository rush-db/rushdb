---
slug: /reference/python/
sidebar_position: 0
title: Python SDK
---

# Python SDK

The official Python SDK for RushDB. Compatible with Python 3.8+.

## Installation

```bash
pip install rushdb
```

## Quick Start

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

# Store a record
db.records.create(
    label="User",
    data={"name": "Alice", "role": "engineer"},
)

# Query records
result = db.records.find({
    "labels": ["User"],
    "where": {"role": "engineer"},
    "limit": 10,
})
```

To connect to a self-hosted instance pass the `url` parameter:

```python
db = RushDB(
    api_key="RUSHDB_API_KEY",
    url="https://your-rushdb-instance.com",
)
```

## API Reference

| Class                                                            | Description                                                                       |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| [RushDB](/reference/python/RushDB)                               | Main client — entry point for all operations                                      |
| [Record](/reference/python/record)                               | Typed record class with instance methods (`attach`, `detach`, `update`, `delete`) |
| [Relationship Patterns](/reference/python/relationship-patterns) | Review and apply AI-suggested relationships                                       |
| [SearchQuery](/reference/python/SearchQuery)                     | Query builder types and interfaces                                                |
| [SearchResult](/reference/python/search-result)                  | Paginated result wrapper returned by `find` and `ai.search`                       |
| [Transaction](/reference/python/transaction)                     | ACID transaction handle                                                           |
