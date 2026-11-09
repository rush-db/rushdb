# Vector internals

This page explains how RushDB stores vectors and answers similarity
searches: canonical vector storage inside the LMPG keyspace, the segmented
approximate-nearest-neighbour (ANN) cache built with `usearch`, how
segments are frozen, merged, and persisted, exact reranking, and the
memory budget that governs it all. It is for operators sizing vector
workloads and for contributors working on search.

## Canonical vectors, derived ANN

The rule that shapes everything else: **canonical vectors are the truth,
and the ANN index is a cache.**

- Vectors, their bindings to records, and the vector spaces live in the
  ordinary tenant keyspace and are written through ordinary commits. They
  are durable, replicated, snapshot-isolated, and restored by recovery like
  any other row.
- ANN structures are built from those rows, may be persisted to disk to
  avoid rebuilds, and can be thrown away at any time. A missing, stale, or
  over-budget ANN index never fails a search; the search answers exactly
  instead.

```mermaid
flowchart LR
    subgraph canon["Canonical (tenant keyspace, WAL-logged)"]
        SP["emb-space<br/>dims, similarity"]
        BI["emb-binding<br/>(owner, property, element) → slot,<br/>source hash, status"]
        VE["vec<br/>raw f32 vector per slot"]
        VV["emb-version<br/>(previous commit, last commit)<br/>of the space's bindings"]
    end
    subgraph cache["ANN cache (per tenant, per space)"]
        HOT["hot segment<br/>brute force, absorbs writes"]
        FR["frozen segments<br/>usearch HNSW, immutable"]
    end
    D[("vector-segments/<br/>persisted frozen segments")]
    canon -->|"build / absorb"| cache
    FR <-->|"persist / reopen<br/>when fingerprint matches"| D
    cache -->|"candidates"| RR["exact rerank<br/>from canonical vectors<br/>at the pinned view"]
    VE --> RR
```

### Spaces, bindings, and slots

- An **embedding space** fixes the dimensionality and the similarity
  metric (`cosine`, `dot`, or `euclidean`) and is addressed through a stable
  alias — the vector index name. Spaces are immutable once created; a
  tenant can hold up to 256.
- A **binding** attaches a vector to a source: an owner (record or
  relationship), a property, and an array element. It records which
  **slot** holds the vector and a hash of the source value at bind time.
  If the source property value changes, a binding that tracks it becomes
  **stale** and is excluded from search until a new vector is supplied.
- A **slot** holds the raw `f32` vector bytes (little-endian).

A binding row's key names the space and the source; its value is 17 bytes:
the slot (u64), the source hash (u64) and one byte of flags (status, and
whether the binding tracks its source value). Releases up to 3.0.6 stored
the binding as a JSON document; the first start of 3.0.7 rewrites those
rows (see [Data migrations at start](../operations/storage-and-compaction.md#data-migrations-at-start)).

Re-binding a source writes the new vector to a **fresh slot** rather than
overwriting the old one, so a snapshot pinned before the re-bind can still
read the old vector. The old slot is queued for reclamation and erased by
the maintenance tick once the MVCC floor passes the re-bind
([MVCC and snapshots](mvcc-and-snapshots.md#garbage-collection)).

Each space also has a **version row** holding the commit that last changed
its bindings and the one before. A cached ANN structure is current when its
version equals the space's latest; a new commit can be absorbed
incrementally when the cache sits exactly at the previous version. Writes
to other spaces or tenants never invalidate it.

## Scoring

Every score — exact search, ANN rerank, the hot segment — comes from one
kernel. The query is prepared once per search (its norm computed once), and a
stored vector is scored straight from its bytes where the store holds them
(the pinned page on disk, the live map in memory), without copying it out of
the store or decoding it into a new buffer. The sums accumulate in `f64` over 8 independent lanes (component
*i* into lane *i* mod 8), reduced by one fixed tree, then narrowed to `f32`.
The lanes let the loop run on SIMD units: an AVX2 kernel is chosen at runtime
when the CPU has it, a portable one otherwise. FMA is deliberately not used,
so both kernels perform the same IEEE operations and return the same bits on
every CPU. A stored vector with a non-finite component is reported as
corruption (vectors are validated when bound).

Compared with one strictly sequential sum, only the order of the `f64`
additions differs: a relative difference around `dimensions × 2⁻⁵³`, far
below half an `f32` ulp, so a narrowed score is the same or, when it falls
next to an `f32` rounding boundary, one ulp away.

## Exact search

Exact search is always available and is the reference result. It streams a
space's bindings from the pinned view in batches (1 024 rows or 4 MiB),
skips bindings that are not eligible or fail the request's filter, and
scores the rest in parts of 2 048 vectors, at most 65 536 bindings held at a
time. An owner set (a predicate's matches) is read owner by owner in parts
of 1 024 owners. Parts run on the server's shared thread budget (as many
helper threads as it lends at that moment, none when it is spent), each with
its own top *k*, merged in part order. Hits are ranked by a total order —
score, then source — so the top *k* is one well-defined set: the result is
identical whatever the number of threads, and ties are deterministic. An
unfiltered search's label check runs only for a hit that would enter the
top *k* at that point, not for every vector scored.

Exact search is used for:

- spaces whose similarity is `dot` or `euclidean` (the ANN providers gather
  candidates by cosine, and reranking cannot recover neighbours that a
  cosine gather missed);
- any search where the ANN cannot be admitted to the memory budget;
- filtered searches whose allowed set the ANN gather cannot reach with
  confidence.

## The segmented ANN cache

For cosine spaces, each tenant's vector engine keeps a **segment set** per
space:

- one **hot segment** — a small, flat, brute-force-scanned set that absorbs
  new bindings incrementally, without rebuilding anything;
- any number of **frozen segments** — immutable `usearch` HNSW graphs.

A live map records, for every source, which segment holds its current
entry. That one structure gives three behaviours for free: a re-bind moves
the source to its newest segment (newest wins), a retired or deleted
binding disappears from results, and superseded entries left in older
segments are ignored — without touching any segment's physical index.

### Freezing and merging

```mermaid
flowchart LR
    W["new bindings"] --> H["hot segment"]
    H -->|"≥ 256 entries<br/>(maintenance tick)"| F1["frozen segment<br/>tier 0"]
    F1 -->|"10 segments of one tier"| F2["merged segment<br/>next tier"]
    F2 -->|"10 segments of one tier"| F3["merged segment<br/>next tier"]
```

- The maintenance tick **freezes** a hot segment once it reaches 256
  entries, building a `usearch` graph from it. A build reads and decodes
  the vectors in parts of 1 024 on the shared thread budget, one chunk of
  8 192 ahead, and inserts them into the graph on one thread in key order:
  inserts from several threads link a measurably worse graph (recall@10 at
  `ef` 128 on clustered 384-dimension data 0.996–0.999 against 1.000), and
  one order makes every build of the same vectors the same graph. If
  maintenance falls behind,
  the write path itself freezes the hot segment at 1 024 entries as a
  backstop.
- The tick then runs at most one **size-tiered merge** per space: when ten
  frozen segments fall in the same size band, they merge into one segment
  of the next band, rebuilt from the live entries' canonical vectors. The
  merge reserves its working set from the vector cache budget first; a tier
  too large to merge within the free budget stays as it is. Segments that
  are at least half dead are rewritten first. The segment
  count therefore grows logarithmically with the space size rather than
  with its write history.
- Freezing and merging run on a fork of the segment set, outside the cache
  lock. The result is published only if the cache entry is still the one
  the work started from; otherwise it is discarded and retried on the next
  tick. Searches never wait for maintenance.

### Persistence

With a data directory, frozen segments are written under
`vector-segments/<tenant>/space-<id>/` (`seg-<uuid>`, with a manifest and an
`.entries` sidecar listing the bindings it holds) and served through
`usearch`'s memory-mapped view. When a space's ANN is not cached (after a
restart, or after its tenant was evicted for being idle), the next search
reopens every segment of the space and reconciles them with the current
bindings: an entry stays live only if it is still its source's binding, a
segment left without a live entry is deleted, and only the bindings no
segment covers are decoded again (usually the unfrozen tail). A damaged or
incomplete segment is deleted and its bindings decoded; it is a cache miss,
never data loss. Eviction keeps the files. A pure in-memory node keeps
segments resident only. Purging a tenant removes its segment directory.

## Search path

```mermaid
sequenceDiagram
    autonumber
    participant Q as Search request
    participant E as Vector engine
    participant C as ANN cache
    participant V as Pinned view

    Q->>E: query vector, k, filter
    E->>E: validate dims, finite values
    alt similarity is not cosine
        E->>V: exact search
    else cosine
        E->>C: current segment set for this space
        alt not admitted to the budget
            E->>V: exact search (counted as a fallback)
        else cached / built / absorbed
            E->>C: gather candidates from every segment, skipping filtered-out owners during the walk
            E->>E: keep each source's newest live entry
            E->>V: exact rerank from canonical vectors
            alt too few confident candidates
                E->>V: exact search
            end
        end
    end
    E-->>Q: top k, deterministic ties
```

- **Routing by candidate count.** The server resolves a predicate into a
  sorted owner set before searching. Up to `vector-exact-max-pool` candidates
  are scored exactly, one owner-prefix seek per candidate
  (`emb_owner_bindings_prefix`), so the cost follows the candidate count, not
  the space size; only an unfiltered exact search scans the whole space.
- **Filtering during the walk.** Above that, each frozen segment searches its
  graph with usearch's filtered search: a node is admitted to the results
  only when it is its source's live entry and its owner is a candidate, and
  the walk continues past rejected nodes until it has `max(ef, k)`
  admitted ones. The hot segment checks the same condition before scoring.
  A selective filter therefore does not empty the gather, and no
  selectivity estimate is involved. When the walk still finds fewer than
  `min(ef, k)` candidates, the search falls through to exact. The optional
  turbovec provider has no filtered search; it post-filters a gather widened
  by the exact selectivity (candidates over entries).
- **Per-query breadth.** The walk keeps exactly `max(ef, k)` candidates:
  segments are opened with the smallest search expansion `usearch` allows, and
  `usearch` widens the walk to the number of results requested, so a query's
  own `ef` sets it (no shared setting is mutated, so concurrent queries with
  different `ef` never interfere). Every kept candidate is reranked. The
  default `ef` (`vector-default-ef`, 128) walks as wide as segments always
  did; a smaller `ef` visits fewer nodes and reranks fewer vectors at some
  recall cost, a larger one the reverse.
- **Exact rerank.** Candidates are rescored against their canonical vectors
  read at the request's snapshot, so scores are exact and consistent with
  the rest of the request, even though the ANN structure itself may have
  been built at a slightly different commit.
- **Snapshot consistency.** Binding eligibility, filters, and the vectors
  used for scoring are all read from the same pinned view, so a search
  never returns a source deleted before its snapshot.

## Memory budget

ANN structures are charged to the **vector cache budget**
(`vector-cache-bytes`, default 256 MiB), a pool of its own under
`managed-limit`, so a large space does not compete with the text and planner
caches of `logical-cache-bytes`:

1. Before decoding any vector, a build reserves its working set: about
   `2 × dimensions + 320` bytes per vector (checked against real builds).
   A space is built in chunks sized to the free budget; each chunk is
   persisted as its own segment and released before the next, so a
   file-backed space larger than the budget is still built. A chunk that
   cannot be reserved refuses the build (the search answers exactly); the
   chunks already persisted are reopened by the next search. One build
   per space runs at a time. A search never waits for one: it reopens the
   persisted segments and indexes at most a 10,000-vector tail itself;
   when more is uncovered, or a build is already running, it answers
   exactly (`fallback: "annWarming"`) and the rest is built in the
   background.
2. After the build, the reservation is resized to the structure's real
   measured size (for a file-backed segment, its entries and live map: the
   mapped file is page cache, not charged).
3. Each tenant engine also caps its cached ANN bytes (256 MiB) and the
   number of cached space generations (12), evicting least recently used
   entries.
4. When the budget cannot admit a space's index, the search answers
   exactly and increments `rushdb_ann_exact_fallbacks_total`.

> **Note:** ANN requires a vector cache budget. With
> `vector-cache-bytes: 0` there is nothing to bound ANN memory, so no
> ANN is built and every search answers exactly.

The per-tenant vector engine itself is a resident handle governed by
tenant residency: it is dropped when the tenant is idle and rebuilt on the
next request ([resource governance](resource-governance.md#tenant-residency)).

## See also

- [Vector search](../indexes-and-search/vector-search.md)
- [Hybrid search](../indexes-and-search/hybrid-search.md)
- [Derived state](derived-state.md)
- [Resource governance](resource-governance.md)
- [Metrics](../reference/metrics.md)
