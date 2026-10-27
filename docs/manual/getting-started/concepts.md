# Concepts

This page defines the handful of terms used everywhere else in the manual.
Each definition is short and links to the page that covers it in depth. Read
it once after the [Quickstart](quickstart.md); come back to it as a glossary.

## Record

A **record** is the unit of data: a JSON-like object with a stable id, one
primary label, and a set of properties. Ids are UUIDs; you may supply one when
creating a record or let the engine allocate it. Records are returned in a
canonical shape:

```json
{
  "id": "019a0000-0000-7000-8000-000000000001",
  "label": "Person",
  "properties": { "name": "Ada Lovelace", "born": 1815 },
  "propertyTypes": { "name": "string", "born": "number" }
}
```

Deleting a record also deletes every relationship attached to it.
More: [Records and labels](../data-model/records-and-labels.md).

## Label

A **label** names the kind of a record — `Person`, `Order`, `Invoice`. Every
record has **exactly one** primary label, set when the record is created. You
can change it later with an atomic retype that keeps the record's id,
properties and relationships. Labels need no declaration; the first record
with a new label creates it. In SQL, labels appear as tables.
More: [Records and labels](../data-model/records-and-labels.md).

## Property

A **property** is a named value on a record or relationship. Values are typed
(boolean, number, string or datetime), a property can hold several values at
once (an array), and `null` is stored as an explicit null that is different
from the property being absent. The same property name may carry different
types on different records.
More: [Properties and types](../data-model/properties-and-types.md).

## Relationship

A **relationship** connects a source record to a target record. It has a
type (such as `WORKS_AT`), a direction, and optionally its own properties.
Relationships let filters and statement languages traverse from one record to
its neighbours.
More: [Relationships](../data-model/relationships.md) and
[Relationships and traversal](../query/relationships-and-traversal.md).

## Schema catalog

The **schema catalog** is the engine's running record of every label,
property (with its types and how many records carry it) and relationship
shape in the data. You never write it; you read it with the `describe`
operations.
More: [Schema catalog](../data-model/schema-catalog.md).

## Tenant

A **tenant** is an isolated keyspace inside one node. A node runs in one of
two modes, fixed when its data directory is created: **single-tenant**, where
every request uses the one keyspace, or **multitenant**, where every request
must name a tenant scope (for example with the `X-RushDB-Tenant` header) and
can see only that tenant's data.
More: [Tenants](../data-model/tenants.md) and
[Multitenancy architecture](../architecture/multitenancy.md).

## Transaction

A single-record write is atomic on its own; bulk requests such as nested
ingest or query-based deletes commit in bounded steps (see their pages). A
**transaction** groups several writes from separate requests: you `begin` it, `stage` mutations, then
`commit` or `rollback`. Transactions use snapshot isolation with
first-committer-wins conflict detection; a conflicting commit fails with the
retryable `write_conflict` error. Every transaction has a lifetime (60 seconds
by default) after which it expires.
More: [Transactions](../query/transactions.md).

## Snapshot

A **snapshot** is a consistent, read-only view of the database as of one
commit. Each query runs against a single snapshot, so it never sees half of a
concurrent write. A transaction reads from the snapshot taken when it began
(plus its own staged writes), and a paginating cursor keeps reading the
snapshot it started on, so every page reports the same `total`. Clients cannot
ask for an arbitrary historical snapshot; point-in-time recovery is an
offline operator procedure.
More: [MVCC and snapshots](../architecture/mvcc-and-snapshots.md).

## Index

RushDB has no automatic value indexes. You declare the indexes you need:

- **Ordered indexes** on one label's property answer equality, `$in`, range
  and prefix filters without scanning.
- **Text indexes** provide BM25 full-text search over one property or every
  string property of a label.
- **Vector indexes** store embeddings you supply and answer similarity
  searches, exact or approximate.

Indexes are built online; a new index reports `building` until it is
`ready`.
More: [Indexes](../indexes-and-search/indexes.md),
[Full-text search](../indexes-and-search/full-text-search.md),
[Vector search](../indexes-and-search/vector-search.md).

## Capability profile

A **capability profile** is the machine-readable description a node
publishes of what it supports. `describe.queryLanguages` returns one profile
per enabled statement language — its status (`stable`, `release_candidate` or
`coming_soon`), dialect, read/write access, supported features and explicit
limitations. `describe.capabilities` returns feature flags such as
`ingestDocuments`, `linkRecordsById` or `language:sql`. Clients should
negotiate from these responses instead of inferring support from a version
number; a flag that is absent means the feature is unavailable.
More: [Query languages](../languages/README.md).

## See also

- [Introduction](introduction.md)
- [Data model](../data-model/README.md)
- [Glossary](../reference/glossary.md)
