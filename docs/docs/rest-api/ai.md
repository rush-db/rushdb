---
sidebar_position: 8
---

# AI & Ontology API

The AI API exposes the **graph ontology** of your RushDB project — all labels, their properties with value ranges, and the full cross-label relationship map — in formats designed for direct LLM and developer consumption.

## Why it exists

Before an LLM can query your data meaningfully it must know what labels exist, what fields each label has, what value ranges those fields span, and how labels relate to each other. Without this context the model guesses names, invents operators, and returns 404s or empty results.

The ontology endpoints solve this in a single call:

- **`/ai/ontology/md`** returns compact Markdown tables — optimal for injecting directly into an LLM context window.
- **`/ai/ontology`** returns the same data as structured JSON — useful for programmatic schema inspection, building UI schema pickers, or powering auto-complete.

Both endpoints query the graph once and cache the result on the ProjectNode for **1 hour**. Subsequent calls within the TTL return instantly from the cache; after expiry the graph is re-scanned automatically.

---

## Get Ontology (Markdown)

```http
POST /api/v1/ai/ontology/md
```

Returns the full graph schema as compact Markdown tables. This is the **recommended format for LLM agents**: token-efficient, human-readable, and ready to paste into a system prompt or tool result.

### Request Body

| Field    | Type             | Required | Description                                                                 |
|----------|------------------|----------|-----------------------------------------------------------------------------|
| `labels` | array of strings | no       | Restrict output to specific labels. Omit (or pass `[]`) for the full schema. |

### Example Request

```http
POST /api/v1/ai/ontology/md
Content-Type: application/json
token: YOUR_API_KEY

{}
```

### Example Response

```markdown
# Graph Ontology

## Labels

| Label   | Count |
|---------|------:|
| `Order` |  1840 |
| `User`  |   312 |
| `Product` | 95  |

---

## `Order` (1840 records)

### Properties

| Property    | Type     | Values / Range           |
|-------------|----------|--------------------------|
| `status`    | string   | `pending`, `paid`, `shipped` (+2 more) |
| `total`     | number   | `4.99`..`2499.00`        |
| `createdAt` | datetime | `2024-01-03`..`2026-02-27` |

### Relationships

| Type       | Direction | Other Label |
|------------|-----------|-------------|
| `PLACED_BY` | out      | `User`      |
| `CONTAINS`  | out      | `Product`   |

---

## `User` (312 records)

### Properties

| Property | Type   | Values / Range              |
|----------|--------|-----------------------------|
| `email`  | string | `alice@…`, `bob@…` (+310 more) |
| `plan`   | string | `free`, `pro`, `enterprise` |

### Relationships

| Type       | Direction | Other Label |
|------------|-----------|-------------|
| `PLACED_BY` | in       | `Order`     |
```

### Filtered request (single label)

```http
POST /api/v1/ai/ontology/md
Content-Type: application/json
token: YOUR_API_KEY

{
  "labels": ["Order"]
}
```

Returns only the `Order` section. The underlying cache still covers the full schema — filtering is applied in-memory.

---

## Get Ontology (JSON)

```http
POST /api/v1/ai/ontology
```

Returns the same ontology as a structured JSON array. Each element describes one label.

### Request Body

| Field    | Type             | Required | Description                                          |
|----------|------------------|----------|------------------------------------------------------|
| `labels` | array of strings | no       | Restrict to specific labels. Omit for the full schema. |

### Response Schema

```json
[
  {
    "label": "Order",
    "count": 1840,
    "properties": [
      {
        "id": "prop_abc123",
        "name": "status",
        "type": "string",
        "values": ["pending", "paid", "shipped", "cancelled", "refunded"]
      },
      {
        "id": "prop_def456",
        "name": "total",
        "type": "number",
        "min": 4.99,
        "max": 2499.00
      },
      {
        "id": "prop_ghi789",
        "name": "createdAt",
        "type": "datetime",
        "min": "2024-01-03T00:00:00.000Z",
        "max": "2026-02-27T18:30:00.000Z"
      }
    ],
    "relationships": [
      { "label": "User",    "type": "PLACED_BY", "direction": "out" },
      { "label": "Product", "type": "CONTAINS",  "direction": "out" }
    ]
  }
]
```

### OntologyItem fields

| Field           | Type            | Description                                                                            |
|-----------------|-----------------|----------------------------------------------------------------------------------------|
| `label`         | string          | Label name (case-sensitive, matches the label stored on records)                       |
| `count`         | number          | Total number of records with this label in the project                                 |
| `properties`    | array           | All non-vector properties attached to records of this label                            |
| `relationships` | array           | Edges connecting this label to other labels; bidirectional entries are generated automatically |

### OntologyProperty fields

| Field    | Type                     | Description                                                                     |
|----------|--------------------------|---------------------------------------------------------------------------------|
| `id`     | string                   | Property node ID — pass this to `GET /api/v1/properties/:id/values` for deeper drill-down |
| `name`   | string                   | Field name as stored on the record                                              |
| `type`   | string                   | One of: `string`, `number`, `boolean`, `datetime`, `vector`                    |
| `values` | array (string/number)    | Up to 10 sample values (string and boolean properties only)                     |
| `min`    | number \| string         | Minimum observed value (number and datetime properties only)                    |
| `max`    | number \| string         | Maximum observed value (number and datetime properties only)                    |

### OntologyRelationship fields

| Field       | Type           | Description                                                        |
|-------------|----------------|--------------------------------------------------------------------|
| `label`     | string         | The other label in the relationship                                |
| `type`      | string         | Relationship type string (e.g. `PLACED_BY`, `CONTAINS`)           |
| `direction` | `in` \| `out`  | `out` = this label is the source; `in` = this label is the target  |

---

## Caching behaviour

The ontology is expensive to compute on large graphs (it scans all records, properties, and relationships). RushDB caches the result on the ProjectNode:

| Scenario                      | Behaviour                                                                                     |
|-------------------------------|-----------------------------------------------------------------------------------------------|
| Cache missing or expired (> 1h) | Runs three parallel Cypher queries, builds the full ontology, writes it to the ProjectNode    |
| Cache fresh (≤ 1h)            | Reads directly from the cached field on the ProjectNode — no graph scan                       |
| `labels` filter provided      | Cache always stores the **full** schema; label filtering is applied in-memory after cache read |

The cache is invalidated automatically on the next request after 1 hour. There is no explicit invalidation endpoint — if you need a fresh view immediately, wait for the TTL to expire or the server restart.

---

## Use cases

### LLM agent session start

Call `/ai/ontology/md` as the very first tool at session start and inject the result as context before building any queries. This prevents hallucinated label names, wrong field names, and invalid operators.

```http
POST /api/v1/ai/ontology/md
Content-Type: application/json
token: YOUR_API_KEY

{}
```

Pass the returned Markdown string directly to your LLM as a system message or tool result. The model can then answer questions like "how many paid orders are there?" by referencing the correct label (`Order`), field (`status`), and value (`paid`).

### Schema inspection in dashboards

Call `/ai/ontology` and render the JSON to build a schema explorer, auto-complete field names in a query builder, or populate a dropdown of available labels and their properties.

### Discovering property IDs for value drill-down

The `id` field in each `OntologyProperty` is the property node ID. Pass it to `GET /api/v1/properties/:id/values` to enumerate all distinct values for that field — useful for building filter UIs or enum pickers.

```http
GET /api/v1/properties/prop_abc123/values
token: YOUR_API_KEY
```
