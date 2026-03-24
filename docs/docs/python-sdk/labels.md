---
sidebar_position: 5
---

# Labels

```python
# All labels and their record counts
result = db.labels.find({})
# → [LabelResult(name='MOVIE', count=3), LabelResult(name='ACTOR', count=3), ...]

# Labels that have records matching a condition
result = db.labels.find({"where": {"rating": {"$gte": 8}}})
# → [LabelResult(name='MOVIE', count=1)]
```

`db.labels.find()` accepts a standard [SearchQuery](../concepts/search/where) `where` clause to filter labels by the fields of their records. It returns all labels that match regardless of how many records satisfy the condition.



