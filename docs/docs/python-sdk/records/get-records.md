---
sidebar_position: 5
---

# Get Records

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "where": {"rating": {"$gte": 8}},
    "limit": 10,
    "orderBy": {"rating": "desc"}
})

for movie in result:
    print(movie.get("title"), movie.get("rating"))

print(f"{len(result)} shown, {result.total} total")
```

## Find by ID

`db.records.find_by_id()`

```python
# Single record
movie = db.records.find_by_id("movie-123")

# Multiple records
movies = db.records.find_by_id(["movie-123", "movie-456"])
```

## SearchQuery parameters

| Parameter | Type        | Description                                         |
| --------- | ----------- | --------------------------------------------------- |
| `labels`  | `list[str]` | Filter by one or more labels                        |
| `where`   | `dict`      | Field conditions and operators                      |
| `orderBy` | `dict`      | `{"field": "asc" \| "desc"}`                        |
| `limit`   | `int`       | Max records to return. **Omit when using `select`** |
| `skip`    | `int`       | Records to skip (pagination offset)                 |
| `select`  | `dict`      | Output-shaping expressions (preferred)              |
| `groupBy` | `list[str]` | Group results (with select)                         |

## Relationship traversal

```python
# MOVIE that has ACTOR named DiCaprio
result = db.records.find({
    "labels": ["MOVIE"],
    "where": {
        "ACTOR": {
            "name": {"$contains": "DiCaprio"}
        }
    }
})

# With explicit relationship type and direction
result = db.records.find({
    "labels": ["MOVIE"],
    "where": {
        "ACTOR": {
            "$relation": {"type": "STARS_IN", "direction": "in"},
            "country": "USA"
        }
    }
})
```

## Where operators

```python
# Common operators
{"rating":  {"$gt": 7, "$lt": 10}}       # gt, gte, lt, lte
{"genre":   {"$in": ["sci-fi", "drama"]}} # in, nin
{"title":   {"$contains": "Inc"}}         # contains, startsWith, endsWith
{"sequel":  {"$exists": False}}           # exists, not exists
{"id":      {"$id": "movie-123"}}         # match by __id
```

## Select Expressions

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "select": {
        "count":     {"$count": "*"},
        "avgRating": {"$avg": "$record.rating"},
        "titles":    {"$collect": {"from": "$record", "select": {"title": "$record.title"}}}
    }
    # Do NOT add "limit" when using select
})
```

Expressions: `$count` · `$sum` · `$avg` · `$min` · `$max` · `$collect` · `$timeBucket` · `$ref` · `$add` · `$subtract` · `$multiply` · `$divide`

:::danger
**Never set `limit` with `select`** — it restricts the record scan and produces wrong totals.
:::

## GroupBy

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "select": {
        "count":     {"$count": "*"},
        "avgRating": {"$avg": "$record.rating"}
    },
    "groupBy": ["$record.genre"],
    "orderBy": {"count": "desc"}   # late-ordering: ensures correct totals
})
```

## TimeBucket (time-series)

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "select": {
        "month": {"$timeBucket": {"field": "$record.releasedAt", "unit": "month"}},
        "count": {"$count": "*"}
    },
    "groupBy": ["month"],
    "orderBy": {"month": "asc"}
})
```

Unit values: `"day"` · `"week"` · `"month"` · `"quarter"` · `"year"` · `"hours"` · `"minutes"` · `"seconds"` (use plural forms with `"size"` for custom windows).

## Collect related records

Label-based (no alias needed — preferred for nesting):

```python
result = db.records.find({
    "labels": ["COMPANY"],
    "select": {
        "departments": {
            "$collect": {
                "label": "DEPARTMENT",
                "select": {
                    "name": "$self.name",
                    "projects": {
                        "$collect": {
                            "label": "PROJECT",
                            "select": {"name": "$self.name"}
                        }
                    }
                }
            }
        }
    }
})
```

Alias-based (requires `$alias` in `where`):

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "where": {
        "ACTOR": {"$alias": "$actor"}
    },
    "select": {
        "actors": {
            "$collect": {
                "from": "$actor",
                "select": {"name": "$actor.name", "country": "$actor.country"}
            }
        }
    }
})
```

## SearchResult

```python
result = db.records.find({"labels": ["MOVIE"], "limit": 10})

len(result)       # records returned in this page (≤ limit)
result.total      # total records matching in the database
result.has_more   # True if more pages remain
result[0]         # access by index
for r in result:  # iterable
    pass
```

## With a transaction

```python
tx = db.tx.begin()
try:
    result = db.records.find({"labels": ["MOVIE"]}, transaction=tx)
    tx.commit()
except Exception:
    tx.rollback()
    raise
```
