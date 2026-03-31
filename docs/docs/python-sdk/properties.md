---
sidebar_position: 4
---

# Properties

## Find Properties

`db.properties.find()`

```python
# All properties
props = db.properties.find()

# Filtered
props = db.properties.find({
    "where": {"type": "string"},
    "limit": 20
})
```

## Find by ID

`db.properties.find_by_id()`

```python
prop = db.properties.find_by_id("prop-123")
```

## Get Property Values

`db.properties.values()`

Returns distinct values for a property — useful for building filter UIs or feeding into `db.ai.get_ontology()`.

```python
values_data = db.properties.values(
    property_id="prop-123",
    search_query={
        "query": "sci",    # filter values containing this text
        "orderBy": "asc",
        "limit": 100
    }
)

print(values_data.get("values"))  # list of values
print(values_data.get("min"))     # numeric min
print(values_data.get("max"))     # numeric max
```

## Delete Property

`db.properties.delete()`

Deletes a property and removes it from **all records** that have it.

```python
db.properties.delete("prop-123")
```

:::note
Deleting a property removes its data from every record in the database, not just one record.
:::
