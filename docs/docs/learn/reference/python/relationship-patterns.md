---
sidebar_position: 2
title: Relationship Patterns
---

# Relationship Patterns

RushDB can analyze the project ontology and suggest relationships that are not materialized yet. Suggestions are reviewable: no relationship is created until you approve a pattern.

Access the API through `db.relationships.patterns`.

## Methods

| Method                                         | Description                                                            |
| ---------------------------------------------- | ---------------------------------------------------------------------- |
| `list()`                                       | List saved patterns, ontology relationships, and analysis status       |
| `analyze()`                                    | Queue ontology analysis to generate suggestions                        |
| `approve(pattern_id)`                          | Approve a suggestion and apply its relationships                       |
| `ignore(pattern_id)`                           | Ignore a suggestion without applying it                                |
| `delete(pattern_id, *, delete_existing=False)` | Delete a saved pattern, optionally removing materialized relationships |

## Review Flow

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

# Queue analysis. Fetch results with list() after the analysis completes.
db.relationships.patterns.analyze()

result = db.relationships.patterns.list()
for pattern in result.data["patterns"]:
    print(pattern["id"], pattern["type"], pattern["confidence"])

db.relationships.patterns.approve("pattern-id")
```

Use `ignore()` when a suggestion does not fit your domain model:

```python
db.relationships.patterns.ignore("pattern-id")
```

Deleting a saved pattern does not delete relationships that it already created unless you opt in:

```python
db.relationships.patterns.delete("pattern-id", delete_existing=True)
```

For lifecycle details, matching modes, REST endpoints, and MCP tools, see [Suggested Relationship Patterns](/learn/relationships/suggested-patterns).
