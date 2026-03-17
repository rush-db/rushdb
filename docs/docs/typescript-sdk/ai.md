---
sidebar_position: 8
---

# AI & Ontology

The `db.ai` namespace exposes the graph ontology of your RushDB project — all labels, their properties with value ranges, and the full cross-label relationship map — in formats designed for LLM agents and schema-aware tooling.

## Why it's useful

Before an LLM can write queries reliably it must know what labels exist, what fields each label has, and how labels relate to each other. Without this context the model guesses names, invents operators, and produces 404s or empty results.

Call `db.ai.getOntologyMarkdown()` at the start of every AI session and inject the result into the LLM context. The model can then answer questions like _"how many paid orders are there?"_ by referencing the correct label (`Order`), field (`status`), and value (`paid`).

Both methods call the same underlying graph scan and share a **1-hour cache** stored on the ProjectNode. Subsequent calls within the TTL return immediately from the cache.

---

## `db.ai.getOntologyMarkdown()`

Returns the full graph schema as compact Markdown tables. This is the **recommended format for LLM consumption**: token-efficient and ready to paste directly into a system prompt or tool result.

### Signature

```typescript
db.ai.getOntologyMarkdown(
  params?: { labels?: string[] },
  transaction?: Transaction | string
): Promise<ApiResponse<string>>
```

### Parameters

| Parameter          | Type             | Required | Description                                                              |
|--------------------|------------------|----------|--------------------------------------------------------------------------|
| `params.labels`    | `string[]`       | no       | Restrict output to specific labels. Omit for the full schema.            |
| `transaction`      | `Transaction` \| `string` | no | Optional transaction for atomic operations.                    |

### Example — full schema for LLM context

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

// Fetch the full schema as Markdown
const { data: schemaMarkdown } = await db.ai.getOntologyMarkdown()

// Inject directly into the LLM as system context
const messages = [
  { role: 'system', content: `You are a data assistant.\n\n${schemaMarkdown}` },
  { role: 'user',   content: 'How many paid orders are there?' }
]
```

### Example — filtered to a single label

```typescript
const { data: orderSchema } = await db.ai.getOntologyMarkdown({ labels: ['Order'] })
```

The underlying cache always covers the full schema; the `labels` filter is applied in-memory.

### Example output

```markdown
# Graph Ontology

## Labels

| Label     | Count |
|-----------|------:|
| `Order`   |  1840 |
| `User`    |   312 |
| `Product` |    95 |

---

## `Order` (1840 records)

### Properties

| Property    | Type     | Values / Range                              |
|-------------|----------|---------------------------------------------|
| `status`    | string   | `pending`, `paid`, `shipped` (+2 more)      |
| `total`     | number   | `4.99`..`2499.00`                           |
| `createdAt` | datetime | `2024-01-03`..`2026-02-27`                  |

### Relationships

| Type        | Direction | Other Label |
|-------------|-----------|-------------|
| `PLACED_BY` | out       | `User`      |
| `CONTAINS`  | out       | `Product`   |
```

---

## `db.ai.getOntology()`

Returns the same ontology as a structured JSON array. Use this when you need to programmatically inspect the schema — for example, to build a schema explorer UI, populate a dropdown of labels, or look up property IDs for `db.properties.values()`.

### Signature

```typescript
db.ai.getOntology(
  params?: { labels?: string[] },
  transaction?: Transaction | string
): Promise<ApiResponse<OntologyItem[]>>
```

### Parameters

| Parameter       | Type             | Required | Description                                          |
|-----------------|------------------|----------|------------------------------------------------------|
| `params.labels` | `string[]`       | no       | Restrict output to specific labels.                  |
| `transaction`   | `Transaction` \| `string` | no | Optional transaction.                      |

### Example — full schema as JSON

```typescript
const { data: ontology } = await db.ai.getOntology()

for (const item of ontology) {
  console.log(`${item.label}: ${item.count} records, ${item.properties.length} properties`)
}
```

### Example — extract property IDs for value enumeration

```typescript
const { data: ontology } = await db.ai.getOntology({ labels: ['Order'] })

const statusProp = ontology[0].properties.find(p => p.name === 'status')

// Use the property ID to fetch all distinct values
const { data: values } = await db.properties.values({ id: statusProp.id })
console.log(values) // ['pending', 'paid', 'shipped', 'cancelled', 'refunded']
```

### Response shape

Each element in the returned array is an `OntologyItem`:

```typescript
type OntologyItem = {
  label: string                  // Label name (case-sensitive)
  count: number                  // Total records with this label
  properties: OntologyProperty[]
  relationships: OntologyRelationship[]
}

type OntologyProperty = {
  id: string                          // Property node ID — use with db.properties.values()
  name: string                        // Field name as stored on the record
  type: string                        // 'string' | 'number' | 'boolean' | 'datetime' | 'vector'
  values?: Array<string | number>     // Up to 10 sample values (string/boolean only)
  min?: number | string               // Min observed value (number/datetime only)
  max?: number | string               // Max observed value (number/datetime only)
}

type OntologyRelationship = {
  label: string           // The other label in the relationship
  type: string            // Relationship type string (e.g. 'PLACED_BY')
  direction: 'in' | 'out' // 'out' = this label is the source; 'in' = this label is the target
}
```

---

## Caching

Both methods share a cache stored on the ProjectNode with a **1-hour TTL**.

| Scenario                        | What happens                                                                             |
|---------------------------------|------------------------------------------------------------------------------------------|
| Cache missing or older than 1h  | Runs three parallel Cypher queries, builds full ontology, writes cache to ProjectNode    |
| Cache fresh (within 1h)         | Reads directly from the cached field — no graph scan                                     |
| `labels` filter provided        | Cache is always stored as the full schema; filtering is done in-memory on the way out    |

---

## Common patterns

### Agent session initialisation

```typescript
async function startAgentSession(userMessage: string) {
  const { data: schema } = await db.ai.getOntologyMarkdown()

  return callLLM({
    system: `You are a data assistant for RushDB.\n\n${schema}`,
    user: userMessage
  })
}
```

### Building a dynamic schema picker

```typescript
const { data: ontology } = await db.ai.getOntology()

const labelOptions = ontology.map(item => ({
  value: item.label,
  label: `${item.label} (${item.count} records)`
}))

// labelOptions is ready for a <select> or autocomplete component
```

### Validate a user-supplied label before querying

```typescript
const { data: ontology } = await db.ai.getOntology()
const knownLabels = new Set(ontology.map(item => item.label))

function assertLabel(label: string) {
  if (!knownLabels.has(label)) {
    throw new Error(`Unknown label "${label}". Known labels: ${[...knownLabels].join(', ')}`)
  }
}
```
