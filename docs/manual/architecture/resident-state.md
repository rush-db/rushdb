# Resident state

RushDB keeps a few structures in memory across requests because rebuilding
them on every request is too slow: owner sets of indexed filters,
relationship graphs, the label catalog, the property dictionary, planner
statistics, the rows of vector spaces and the tenant directory. Each one is a
**layer** of one framework (`rushdb-storage/src/resident/`), and storage is
always the reference: a layer can make an answer faster, never different.

## The rule

Every layer follows one version rule.

1. Each layer entry reflects a set of storage areas, its **deps**: one
   declared index, one property, one label, the tenant's label catalog, its
   property definitions, its adjacency, one embedding space, its security
   rows, or the tenant directory.
2. Every commit raises the version of each area it writes to its commit
   index **before** the commit becomes visible. This happens in the one
   place every write passes through: local commit groups, commits a follower
   applies from the leader, WAL replay at startup and a restore into a
   running store. The mapping from a written key to the areas it changes is
   one exhaustive match over the key families, so a new key family cannot be
   added without deciding what it changes.
3. An entry records the version `V` of its deps when it was built (or
   computed by a reader and kept), at a snapshot that includes `V`.
4. A reader at snapshot `r` uses the entry only if the deps' version is still
   `V` and `r ≥ V`. Anything else is a miss, and the reader reads storage.

So an entry survives commits that do not touch it (other tenants, other
properties, other relationship types' tenants), and any write to one of its
deps makes it a miss at once, also for readers pinned to older snapshots.

Versions live in a fixed table of 2^20 slots (8 MiB), indexed by a hash of
the area. Two areas that share a slot can only make an entry look newer than
it is, which costs a rebuild, never a wrong answer. The table does not grow
with the number of tenants or properties.

## Entries are immutable

An entry is made from one pinned snapshot and shared read-only. Nothing
patches it in place: no deltas, no partial refresh. When its deps change it
is dropped (on the next lookup, or by the layer's LRU) and a new one is built
from a later snapshot.

## Building

A layer is filled in one of two ways:

- **Offered:** a request that read storage anyway hands its result to the
  layer (owner sets, catalogs, planner statistics, the tenant directory).
- **Rent-or-buy:** a request that read storage reports how many rows it read
  toward a build. Once the rows read since the deps last changed reach the
  cost of one build, the entry is built in the background at a fresh view,
  on a helper thread of the shared query thread budget; the paying request
  keeps its storage answer (relationship graphs, vector rows). A build that
  would not fit its budget stops early and is not retried until a dep
  changes.

A build that raced a write to one of its deps is never kept.

## Budgets

Each layer has its own byte budget and LRU. A query is never refused because
of a layer: an entry that does not fit is simply not kept.

| Layer (`layer` label) | Holds | Deps | Budget | Default |
|---|---|---|---|---|
| `owner-sets` | Owners of an indexed filter leaf | the leaf's indexes | `owner-set-cache-bytes` (child of the logical cache) | 64 MiB |
| `graph` | Both directions of one relationship type | the tenant's adjacency | `graph-cache-bytes` | 128 MiB |
| `label-catalog` | Labels and per-label counts | label catalog, security rows | share of `logical-cache-bytes` | 128 MiB |
| `property-dictionary` | Property name → type | property definitions, security rows | share of `logical-cache-bytes` | 128 MiB |
| `describe` | One unfiltered `describe.properties` answer (also the property part of `describe.schema`), before policy masking | property definitions; per described label its members, its label-scoped property rows and the properties it holds; the label catalog when no label is named | share of `logical-cache-bytes` | 128 MiB |
| `planner-stats` | One value distribution | the property's values | `planner-stats-bytes` (child of the logical cache) | 32 MiB |
| `vector-arena` | One space's ready bindings and their vectors as `f32`, with each vector's norm | the space | share of `vector-cache-bytes` | 256 MiB |
| `tenant-directory` | Scope → active tenant ordinal | the tenant directory | share of `logical-cache-bytes` | 128 MiB |
| `column` | One stored property of one label: each member's value | the label's members, the property's values, the tenant's definitions | `column-cache-bytes` | 256 MiB |

## Resident typed columns

A column holds, for every member of one label at one snapshot, the value
of one stored scalar property as a typed cell (absent, null, boolean,
integer or datetime milliseconds, float, or string by dictionary code):
about 17 bytes per member plus the distinct strings. Values are copies of
the stored values, decoded the way point reads decode them, so an
aggregate folds the same values in the same order either way. There is no
column for a property stored as an array anywhere, for system fields, or
for a label marked suspect by self-heal.

Grouped aggregates over one label, with no traversal, use columns when
every property they read has one:

- **Rows.** Each member's cells come from the columns instead of point
  reads; groups are found by their key cells' codes, and a walk takes its
  candidates from the column's members instead of the membership rows.
- **The filter.** A filter on the label's stored properties is evaluated
  once per distinct combination of its properties' cells, by the same
  evaluator every read uses, and the matching members become the
  candidates. Above 4,096 distinct combinations (a filter over many
  distinct values) the planner answers it as before.

Columns are built in the background once the reads they would replace pay
for one build (about one read per member): point reads of the rows, and
the planner's candidates for a filter. Until then, and whenever a column is
not resident, the aggregate reads storage. A pass that read columns and
then exceeds `query-memory-max` is run again the way storage runs it, so
columns never refuse an aggregate that storage answers.

## Off is the reference

`resident-caches: off` (or `RUSHDB_RESIDENT_CACHES=off`) keeps no entry in
any layer: storage answers every request. A budget of `0` does the same for
one layer. Results are the same bytes either way; only latency changes. The
test suite runs twice, with layers on and off, and differential tests
compare the two at the same snapshot.

`resident-verify-rate` (default `0`) rebuilds a sampled share of hits from
storage in the background and compares them. A difference increments
`rushdb_resident_verify_mismatch_total{layer}`, drops the entry and logs the
layer at `warn`; the answer already returned is not changed. Only one
verification runs at a time.

## Metrics

Every layer reports the same counters, labelled `layer`:
`rushdb_resident_bytes`, `rushdb_resident_entries`,
`rushdb_resident_hits_total`, `rushdb_resident_misses_total`,
`rushdb_resident_builds_total`, `rushdb_resident_build_refused_total`,
`rushdb_resident_build_seconds`, `rushdb_resident_invalidations_total`,
`rushdb_resident_evictions_total`, `rushdb_resident_verify_total` and
`rushdb_resident_verify_mismatch_total`. See the
[metrics reference](../reference/metrics.md).

## What is not resident state

These keep memory too, but they are not a cache of a storage answer, so they
are outside the framework, each with its own contract:

- **ANN generations** ([vector internals](vector-internals.md)): an
  approximate index by contract, patched in place as bindings arrive. With
  `vector-cache-bytes: 0` searches answer exactly instead, which is a
  different (exact) answer, so "off = same bytes" cannot hold for them.
- **Text indexes** ([text internals](text-internals.md)): persisted derived
  indexes with their own watermark and `eventual` / `immediate` consistency.
  Their files are memory-mapped (page cache), and the writer heap is not
  charged to a RushDB budget.
- **Derived rows in storage** (value dictionary, counters, presence, index
  entries): they are storage, kept exact in every commit
  ([derived state](derived-state.md)).
- **The redb page cache** (`storage-cache`): below the reference.
- **Request-scoped state** (reconstruction caches, per-request family and
  label tables, cycle graphs): lives inside one pinned view and ends with the
  request.
- **Observability snapshots** (per-tenant record counts, storage
  statistics): refreshed on a timer and read only by `/metrics`.

## Related

- [Derived state](derived-state.md)
- [Resource governance](resource-governance.md)
- [Configuration reference](../reference/configuration-reference.md)
