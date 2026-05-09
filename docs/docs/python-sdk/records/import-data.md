---
sidebar_position: 1
---

# Import Data

Pass nested dicts — RushDB walks the structure and links each level as a related record.

```python
db.records.create_many(
    label="MOVIE",
    data={
        "title": "Inception",
        "rating": 8.8,
        "ACTOR": [
            {"name": "Leonardo DiCaprio", "country": "USA"},
            {"name": "Ken Watanabe",      "country": "Japan"}
        ]
    }
)
# MOVIE → ACTOR × 2: all created and linked automatically
```

## Flat batch (`create_many`)

Use `create_many` with a list for flat rows — no nested objects inside items.

```python
db.records.create_many(
    label="ACTOR",
    data=[
        {"name": "Leonardo DiCaprio", "country": "USA"},
        {"name": "Ken Watanabe",      "country": "Japan"}
    ],
    options={"suggestTypes": True}
)
```

## CSV import (`import_csv`)

```python
with open("actors.csv") as f:
    csv_content = f.read()

db.records.import_csv(
    label="ACTOR",
    data=csv_content,
    options={"returnResult": False},
    parse_config={"header": True, "dynamicTyping": True}
)
```

### `parse_config` options

| Option | Default | Description |
|---|---|---|
| `delimiter` | `,` | Column separator |
| `header` | `True` | First row is header |
| `skipEmptyLines` | `True` | Ignore blank rows |
| `dynamicTyping` | `True` | Auto-convert numbers and booleans |
| `quoteChar` | `"` | Quote character |
| `escapeChar` | `"` | Escape character |
| `newline` | auto | Explicit newline sequence |

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

## Upsert during import

```python
# Append — update matched, preserve other fields
db.records.create_many(
    label="ACTOR",
    data=actors,
    options={"mergeBy": ["name"], "mergeStrategy": "append"}
)

# Rewrite — replace all properties for matched records
db.records.import_csv(
    label="ACTOR",
    data=csv_content,
    options={"mergeBy": ["name"], "mergeStrategy": "rewrite"}
)
```

## Quick rules recap

- `create_many` with a dict or list of dicts: flat or nested JSON
- `import_csv`: CSV string input with `parse_config`; `dynamicTyping` inherits from `options.suggestTypes` when omitted
- Set `returnResult: False` for large imports to improve performance


