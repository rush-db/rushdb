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

## `db.records.find_by_id()`

```python
# Single record
movie = db.records.find_by_id("movie-123")

# Multiple records
movies = db.records.find_by_id(["movie-123", "movie-456"])
```

## SearchQuery parameters

| Parameter | Type | Description |
|---|---|---|
| `labels` | `list[str]` | Filter by one or more labels |
| `where` | `dict` | Field conditions and operators |
| `orderBy` | `dict` | `{"field": "asc" \| "desc"}` |
| `limit` | `int` | Max records to return. **Omit when using `aggregate`** |
| `skip` | `int` | Records to skip (pagination offset) |
| `aggregate` | `dict` | Aggregation functions |
| `groupBy` | `list[str]` | Group aggregated results |

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

## Aggregations

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "aggregate": {
        "count":     {"fn": "count",  "alias": "$record"},
        "avgRating": {"fn": "avg",    "alias": "$record", "field": "rating"},
        "titles":    {"fn": "collect","alias": "$record", "field": "title"}
    }
    # Do NOT add "limit" when using aggregate
})
```

Aggregation functions: `count` · `sum` · `avg` · `min` · `max` · `collect`

:::danger
**Never set `limit` with `aggregate`** — it restricts the record scan and produces wrong sums/averages.
:::

## GroupBy

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "aggregate": {
        "count":     {"fn": "count", "alias": "$record"},
        "avgRating": {"fn": "avg",   "alias": "$record", "field": "rating"}
    },
    "groupBy": ["$record.genre"],
    "orderBy": {"count": "desc"}   # late-ordering: ensures correct totals
})
```

## TimeBucket (time-series)

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "aggregate": {
        "month": {"fn": "timeBucket", "field": "releasedAt", "granularity": "month", "alias": "$record"},
        "count": {"fn": "count",                                                       "alias": "$record"}
    },
    "groupBy": ["month"],
    "orderBy": {"month": "asc"}
})
```

Granularity values: `"day"` · `"week"` · `"month"` · `"quarter"` · `"year"` · `"hours"` · `"minutes"` · `"seconds"` (use plural forms with `"size"` for custom windows).

## Nested collect

```python
result = db.records.find({
    "labels": ["MOVIE"],
    "where": {
        "ACTOR": {"$alias": "$actor"}
    },
    "aggregate": {
        "actors": {
            "fn": "collect",
            "alias": "$actor",
            "aggregate": {
                "roles": {"fn": "collect", "alias": "$actor", "field": "role"}
            }
        }
    }
})
```

Only `fn: "collect"` is valid inside nested `aggregate` blocks.

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
tx = db.transactions.begin()
try:
    result = db.records.find({"labels": ["MOVIE"]}, transaction=tx)
    tx.commit()
except Exception:
    tx.rollback()
    raise
```


