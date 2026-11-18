# Changelog

This project follows [Semantic Versioning](https://semver.org/). Release notes
describe observable engine and protocol changes; migration instructions belong
in [`docs/manual/operations/upgrading-from-2x.md`](docs/manual/operations/upgrading-from-2x.md).

## Unreleased

### Changed

- **Breaking: Cypher, GQL, Gremlin and SQL/PGQ run on Grafeo 0.5.44**
  (was 0.5.42). Some statement results change:
  - An unaliased column is named after its expression as written:
    `RETURN count(n)` returns a column `count(n)`, not `countnonnull(...)`.
  - Statements that returned wrong rows now fail with an error: two columns
    with the same name (`RETURN 1 AS a, 2 AS a`; give one another alias),
    an unnamed expression in `WITH`, and `UNION` mixed with `UNION ALL`. A
    parameter that was not supplied fails with `Missing parameter: $name`
    instead of an internal error.
  - `ORDER BY` uses the openCypher order across value types (strings before
    numbers), and GQL `NULLS FIRST`/`NULLS LAST` are no longer reversed
    with `DESC`.
  - Cypher `=~` must match the whole string: `'abc' =~ 'b'` is false.
  - A variable-length relationship variable holds the list of the path's
    relationships (`size(rs)` was null).
  - `MERGE` binds every matching node, one row each, instead of the first.
  - `keys(r)` of a relationship lists its keys (was null).
  - `EXISTS { ... }` over a multi-hop pattern runs (was an internal error).
  - Computed property values in `CREATE` (`CREATE (:Item {n: r[0]})`) now
    work; they used to be refused as `unsupported`.
  - One form that worked is now refused: a `WITH ... WHERE` that reads a
    variable the `WITH` does not project, such as the anti-join
    `OPTIONAL MATCH (a)-[r]->(c) WITH c WHERE r IS NULL`. Write
    `WITH c, r WHERE r IS NULL`.

  The openCypher TCK now passes 1,653 of 3,897 scenarios (was 1,385); of
  the 2,403 in the supported profile, 1,653 pass (was 1,385 of 2,431).
- **Commits are published once their write-ahead log record is durable.**
  Before, a commit became visible only after it was also written into the
  store file, one group at a time, so every tenant's write waited for the
  whole store-file write of any large commit ahead of it, such as a merge
  import (one commit per request since 3.0.11). That write now happens on
  a background thread per store file; reads include the commits it has not
  written yet, with the same answers at every snapshot. The backlog is
  bounded at a quarter of the managed budget, at most 256 MiB; at the bound
  commits wait for it. 50 light tenants writing single records beside a
  200,000-record merge import, same host, before → after: requests of
  5,000 documents: light p99 491–500 → 188–287 ms, import 3,663–3,664 →
  3,226–3,496 records/s (one further run measured 2,527 records/s while
  every commit phase on the host was slow alike); requests of 1,000
  documents: light p99 383–407 → 183–184 ms, import 3,011–3,146 →
  3,402–3,460 records/s. The same import alone: 44.9 → 49.1–49.4 s (about
  9% slower: the import's index lookups also check the not yet written
  commits). A commit of 16,384 keys or more, such as a large import
  request, enters that backlog as one sorted run instead of key by key, so
  bulk loads are not slowed by it; against 3.0.12: a 1,000,000-row CSV
  import 22.9–23.0k → 24.2k rows/s, nested JSON documents in requests of
  5,000 4.5–4.6k → 4.7–5.4k documents/s and of 500 3.6k → 4.8k
  documents/s. The advice to send merge imports of about 1,000
  records on shared hosts is withdrawn. Metrics: the writer-hold label
  `publish` is now `materialize`; new `rushdb_storage_unmaterialized_bytes`
  and commit phase `gap_push`; `backend_apply` now times the background
  writes.
- **Faster unfiltered ANN vector search.** The label-membership check of an
  unfiltered `query.vector` (and the vector leg of `query.hybrid`) now runs
  on the reranked hits in rank order until `limit + offset` pass, instead
  of on every gathered candidate, and a hit's record is read once. Responses
  are byte-identical. 100,000 × 1,536-d cosine vectors, top 10: server time
  1.54 → 1.37 ms at `ef` 128 and 0.93 → 0.86 ms at `ef` 64.
- **Exact vector search twice as fast once a space is resident.** A
  space's resident vectors (`vector-cache-bytes`) now also hold its ready
  bindings and each vector's norm, so an exact search without a predicate
  (or with one matching more records than the space holds vectors) scores
  them in one parallel pass instead of first reading every binding from
  storage on one thread, and cosine scoring computes one sum per vector
  instead of two. Scores and order are bit-identical. 100,000 × 1,536-d
  cosine vectors, exact top 10: server time 24.1 → 11.7 ms. The resident
  form of a space grows by 48 bytes per vector (under 1% at 1,536-d).
- **Filtered vector searches pick exact or ANN by cost.**
  `resources.vector-exact-max-pool` now defaults to `auto`: a filtered
  search is scored exactly while `pool² × dimensions ≤ 26,000 × ef × index
  vectors` (the walk slows down as the filter admits less of the index), an
  unfiltered one while `index vectors × dimensions ≤ 144,000 × ef`, instead
  of a fixed 5,000 at any size. 100,000 × 1,536-d, a filter matching 10%,
  `mode: ann`, `ef` 512: 18.7 → 4.4 ms (now exact, recall 1.0); a filter
  matching 50% stays on the graph. A number still fixes the pool, and the
  new form `fixed:<number>` does too.
  **Behaviour change for existing data directories:** every `config.yml`
  written before 3.0.13 holds `vector-exact-max-pool: 5000`, which now
  reads as `auto`. If you set 5,000 on purpose, write
  `vector-exact-max-pool: fixed:5000` (or `--vector-exact-max-pool
  fixed:5000`, `RUSHDB_VECTOR_EXACT_MAX_POOL=fixed:5000`) to keep it. New
  data directories are written with `auto`.
- **Repeated schema reads are answered from memory.** An unfiltered
  `describe.properties` answer (and the property part of `describe.schema`)
  is kept in a resident layer (`describe`, a share of
  `logical-cache-bytes`) until a write reaches what it reads: the described
  labels' records and property rows, a property they hold, or the property
  definitions. Writes to other labels' own properties and to other tenants
  keep it; a policy still narrows and masks every answer. Answers are
  byte-identical. 10,000 nested documents (4 labels): `describe.properties`
  server time 3.1 → 0.2 ms; one `describe.schema` plus one
  `describe.properties` per label, client side: 11.8 → 1.5–2.0 ms.

### Fixed

- **`rushdb up` no longer keeps its data beside the installed binary.**
  `install.sh` installs into `~/.rushdb/bin`, and `rushdb up` defaults to
  `./.rushdb`, so started from the home directory it put the store in
  `~/.rushdb` itself: deleting that data directory deleted the binary. When
  `./.rushdb` holds the running binary, the default is now `./.rushdb/data`.
  A store already in `~/.rushdb` keeps opening there, with a one-line notice
  on how to move it; nothing is moved. An explicit `--data-dir` or
  `RUSHDB_DATA_DIR` is unchanged.
- **No more half-second write stall after a large load.** Commits are
  applied to the store file without a sync and made durable by periodic
  checkpoints; the first durable commit after a load (a checkpoint, or the
  history-GC batch that completes a sweep) wrote out and synced the whole
  load while it held the file's only writer, so the next commit of every
  tenant waited for it. A store file whose unsynced writes pass 16 MiB is
  now checkpointed right away by the thread that writes commits into it,
  so no durable commit carries more than that. 50,000 nested documents, then 400 single-record
  writes (two runs each, btrfs): longest writer hold above 1 s → under
  0.5 s, slowest write after the load 188–319 → 92–105 ms. New metric
  `rushdb_storage_paced_checkpoints_total`.

## 3.0.12 — 2026-11-17

### Fixed

- **Existing data directories start again.** 3.0.11 refused to start on any
  data directory created by 3.0.0–3.0.10 (`invalid config ... unknown field:
  planner-stats-idle-seconds`): every `config.yml` lists the whole
  `resources` section, including the two keys 3.0.11 removed. Removed keys
  are now ignored with a warning naming the key and the release that removed
  it (``ignoring setting `planner-stats-idle-seconds`: it was removed in
  3.0.11``), in `config.yml` and as the flags `--planner-stats-idle-seconds`,
  `--planner-stats-max-age-commits` and the variables
  `RUSHDB_PLANNER_STATS_IDLE_SECONDS`, `RUSHDB_PLANNER_STATS_MAX_AGE_COMMITS`.
  Any other unknown key is still refused. Data directories written by every
  published 3.x release, in both tenancy modes, are now opened by the test
  suite and by CI with the answers they gave recorded, so an upgrade that
  breaks them fails before release.

## 3.0.11 — 2026-11-17

### Added

- **`query.path`: the shortest path between two records.** `from`, `to`
  (record ids), `type`, `direction` (default `both`), `maxHops` and optional
  `labels` return the records on a shortest path in order as a `records`
  page, or none when no path is within `maxHops`. A breadth-first search from
  both ends over the type's adjacency (resident once traversals at the
  snapshot have read enough of it); past 1,000,000 reached records it fails
  `resource_exhausted` instead of running on. Under an enforced policy it
  never follows a denied type and never passes through a record whose label
  the policy hides.

### Changed

- **Tenants are admitted fairly beside a bulk load.** Before admission a
  commit now takes its own tenant's lock instead of one of 64 locks shared by
  hashing, so a tenant never waits behind another tenant's commits there.
  Admission itself hands turns round robin across tenants (first come first
  served within one) instead of in arrival order, so a tenant writing one
  record waits for at most one turn of each busy tenant. With a timeout
  (`lock-timeout-ms`) the wait goes through the same gate. Each tenant's own
  commits keep their order, and commit indices stay one contiguous sequence.
- **Large vector index builds use several cores.** A build of 50,000 vectors
  or more (a cold build, a large merge) inserts into the graph from as many
  threads as `threads` allows: a cold build of 1,000,000 vectors of 768
  dimensions answers by ANN after 5.6 minutes instead of 24.5 (20
  threads). Smaller builds stay single-threaded and bit-identical; a parallel
  build's graph differs slightly, and every candidate is still reranked
  against its f32 vector, so scores never change.
- **Indexed filters stay cached across unrelated commits.** The owner set an
  indexed filter resolves to was dropped by any commit in the store. It is
  now kept until a commit writes one of the indexes it read, so other
  tenants' writes and writes to other properties no longer force it to be
  read again.
- **Breaking: planner statistics are never stale; two keys are removed.**
  A cached value distribution used to be recomputed after
  `planner-stats-max-age-commits` commits or dropped after
  `planner-stats-idle-seconds` idle. It is now dropped by the first commit
  that writes the property's values. Both keys (flags
  `--planner-stats-idle-seconds`, `--planner-stats-max-age-commits`,
  variables `RUSHDB_PLANNER_STATS_IDLE_SECONDS`,
  `RUSHDB_PLANNER_STATS_MAX_AGE_COMMITS`) are gone, and a configuration
  that still sets one is refused at startup; delete them.
  `planner-stats-bytes` stays. The gauges `rushdb_planner_stats_bytes` and
  `rushdb_planner_stats_entries` are replaced by
  `rushdb_resident_bytes{layer="planner-stats"}` and
  `rushdb_resident_entries{layer="planner-stats"}`.
- **In-memory state follows one version rule.** Owner sets of indexed
  filters, relationship graphs, the label catalog, the property dictionary,
  planner statistics, vector rows and the tenant directory are now layers of
  one framework ([resident state](docs/manual/architecture/resident-state.md)).
  Each entry records the version of the storage areas it read; every commit
  raises the versions of the areas it writes before it becomes visible
  (local commits, commits a follower applies, WAL replay and restore), and
  an entry whose areas changed is not used. Relationship graphs and catalogs
  are no longer dropped by commits to other tenants or areas. Each layer has
  a byte budget and reports `rushdb_resident_*{layer}` metrics; answers are
  the same bytes as without the layers. New keys: `owner-set-cache-bytes`
  (default 64 MiB, the owner-set share of the logical cache),
  `resident-caches` (`on`/`off`; `off` answers every read from storage) and
  `resident-verify-rate` (default `0`: the share of hits rebuilt from storage
  in the background and compared, counted in
  `rushdb_resident_verify_mismatch_total`).
- **Grouped aggregates read resident columns.** A grouped aggregate over
  one label (no traversal) whose properties are stored scalars now reads
  them from in-memory typed columns once those are built: rows are folded
  by their cells' codes, the label's members come from the column, and a
  filter is evaluated once per distinct combination of its properties'
  values instead of per record. Over 1M records, grouped `count`, `sum`,
  `avg`, `min` and `max` by one or two keys, with or without a two-field
  filter, went from 290-440 ms to 15-20 ms. Answers are the same bytes as
  before, and a column never makes an aggregate fail where reading records
  succeeds. Columns are built in the background after the reads they
  replace would pay for one (about one read per record); the first
  aggregates after a start or after a write to the label or property read
  records as before. New key: `column-cache-bytes` (default 256 MiB, about
  17 bytes per record and property; `0` keeps none).
- **A merge import is one commit.** An `ingest` with `mergeBy`,
  `mergeStrategy` or `merge` used to commit its records in chunks of 1,024
  and its relationships in one more commit, so a reader could see part of a
  request and a failure left the committed chunks behind. Now every record
  and relationship of the request is committed at once, all visible or
  none; a cancelled or failed request leaves nothing. Results of a request
  that succeeds are unchanged (same matching, rows sharing a key applied in
  order, existing links not duplicated). Merges running at the same time
  over the same new keys used to each create them, leaving duplicate
  records; now a key is created once: the later merge is re-run on the new
  data, after a short random wait, and updates the record the first one
  created. Merges that only update the same records (other fields, or the
  same key values) do not interfere. Under several writers merging
  overlapping new keys at once, a merge import can now fail with
  `write_conflict` (HTTP 409, `retryable: true`) once its own re-runs are
  used up; nothing of it is written, and clients should retry it with
  backoff (before, such a request succeeded, with duplicates). Because a
  merge request is one commit and commits are published one at a time, a
  large merge request now holds the commit order for its own commit time:
  other tenants' writes beside it wait for up to one such request instead
  of one chunk of 1,024 records. On shared hosts, send merge imports of
  about 1,000 records per request.
  **Breaking for very large merge calls:** a merge request is now bounded
  as a whole by `max-expanded-commit-bytes` (512 MiB by default) and is
  refused with `resource_exhausted` (HTTP 429, `retryable: false`) above
  it, with nothing written; before, each chunk of 1,024
  records was bounded on its own. Typical documents expand 4 to 18 times
  when stored, so a request within the default 16 MiB body limit fits; a
  request made of many tiny nested objects (each one a record and a
  relationship) can expand over 100 times and should be kept to about
  4 MiB. Split larger loads into several requests, and split again when a
  request is refused with `resource_exhausted`.
- **Merge imports can be staged in a transaction.** A transaction `stage`
  of an `ingest` with `mergeBy`/`mergeStrategy` was refused with
  `unsupported`. It is now staged like a standalone merge import, matching
  committed records and the records the transaction staged before it; a
  concurrent write to a matched key aborts the commit (retryable).
- **Filtered describes and label counts read less.** When a query names one
  label, label counts and per-property statistics no longer read each
  match's record header, and per-property statistics over large match sets
  are computed in parallel parts.
- **Value facets on an indexed property read the index.** A filtered
  `propertyValues` (values, count, top-k) over a property with a Ready
  ordered index on every queried label counts its values in one pass over
  the index instead of reading each match's occurrences, when the index holds
  every counted value exactly (one typed, non-datetime definition, no arrays,
  visible under the policy) and the match set is large next to the index.
- **`suggestTypes: false` keeps JSON types.** Turning type inference off now
  only stops strings being read as dates or numbers: JSON numbers, booleans
  and uniform arrays keep their types (`42` stays a number). It used to
  store every value as text.
- **Vector searches score resident vectors.** After a space's first search,
  its vectors are kept as `f32` rows by storage slot (built in the
  background, charged to `vector-cache-bytes`). Exact search and ANN rerank
  score them instead of reading each stored vector; scores are bit for bit
  the same, and a space whose vectors do not fit the budget reads storage as
  before. Measured on 100K × 1536 vectors: exact top-10 −24%, filtered
  top-10 −22% to −49% (0.1% filters unchanged), ANN rerank −62%. A filtered
  search over a few hundred owners now spreads its binding reads over the
  thread budget (parts of 128 owners instead of 1,024).
- **The ANN cache's 256 MiB cap is gone.** Cached generations were limited
  to 256 MiB per tenant engine regardless of `vector-cache-bytes`, and a
  larger generation was silently not cached. The cap is now the vector cache
  budget itself, and a generation that still does not fit is logged
  (`ANN generation not cached`).
- **Multi-hop `$relation` traversals read headers in parallel.** Each
  breadth-first level reads the label of its new records in parallel parts,
  root and target label checks share one read when their labels match, and
  a `select` of scalar properties reads its rows in parallel parts. Result
  sets are unchanged. Measured on a 100K-node, 1.26M-edge graph: 3-hop
  neighbourhood −48%, 2-hop −20%, 2-hop with a filter −10%.
- **Text search makes one pass.** `query.text` no longer counts every match
  before collecting the top k, reads owners from the index's fast field
  instead of its document store, and resolves ties at the k-th score by
  owner however the index is laid out. The background applier now lets the
  index merge the segment each batch writes (5,183 documents left 11
  segments, now 4). Scores and rankings are unchanged; text p50 −35%,
  hybrid −16% to −20%.
- **Less fixed work per request.** An HTTP request's envelope is parsed once
  and its response encoded once (it was parsed twice and encoded four
  times); responses are byte-identical. A text search no longer takes two
  store-wide write locks. 1-hop traversal −18%.

### Fixed

- **A restore or WAL replay no longer leaves cached answers behind.** Cached
  entries computed before a restore into a running store, or before the WAL
  replay that finishes startup, could answer from the rows those paths
  replaced. Both paths now invalidate what they write.
- **Large transactions stay fast as they grow and no longer stall other
  commits.** Each statement staged into a transaction copied everything the
  transaction had staged before it, and a commit compared each key written
  since the transaction began against every read it had recorded, one at a
  time, while other commits waited. Staging now moves the transaction's
  state instead of copying it (a failed statement is undone exactly), and
  the commit check looks reads up in a sorted index. A merge import of
  about 460,000 tiny nested records, which did not finish in 25 minutes,
  now stages in about 10 seconds, and eight concurrent merge imports of 500
  documents each keep the throughput they had before a merge import became
  one commit.
- **A store killed during background maintenance reopens quickly.** A
  process killed (for example by a container stop timeout) after a history
  cleanup batch or a store compaction step, while the store was otherwise
  idle, left a file that the next start checked in full before opening:
  several reads of the whole file, with no log line or progress. Cleanup
  batches are now made durable in groups (one fsync per four batches
  instead of one per batch), each durable one saves the allocator state
  with its commit, and the next
  checkpoint after a compaction step (periodic or at shutdown) is always
  written, so such a kill reopens without the check. When a check is still
  needed (a kill in the middle of a compaction step), the start logs it and
  `/health` reports `repairing-storage` with its `progress`.
- **A follower stops resolving a tenant the leader retired.** A follower kept
  its cached scope → tenant entry after replicating the tenant's retirement
  and kept serving the old tenant until restart.
- **A filter on a reserved name is refused.** `{"__id": ...}` (or any
  `__`-prefixed key) in a `where` matched nothing, silently; it is now
  `invalid_request`, pointing to `$id` for record ids.
- **Authentication refusals on `/protocol/v1/execute` use the protocol error
  envelope.** A missing credential, an invalid one, or a capability without
  the operation's permission answered 401 with `{"error":"unauthorized"}`, a
  bare string clients reading `error.message` could not parse (Python SDK
  direct mode raised `AttributeError`; the JS SDK lost the reason). They now
  answer 401 with `{"protocolVersion":1,"error":{"code":"unauthorized",
  "message":…,"retryable":false}}`; the message says whether the credential
  is missing, invalid, or lacks a permission (naming it), and never echoes
  the credential. The operator endpoints (`/metrics`, `/internal/*`) keep the
  plain form.

## 3.0.10 — 2026-11-13

### Added

- **Self-healing derived state.** Derived rows (counts, presence,
  dictionaries, index entries) are exact by construction; if a bug ever
  breaks that, the engine notices while reading (a label's presence rows
  against its support counter), logs it, counts it in
  `rushdb_derived_inconsistency_total`, answers that label from its base rows
  until repaired, and repairs it within a maintenance tick with a logged
  re-derive every node applies the same way. A suspect label's marker is
  stored, so it survives restarts and replicates until the repair; a repair
  over 200,000 records waits for a quiet store (`compact-quiesce-seconds`).
  A randomized test checks the deriver against a from-scratch derivation
  after every commit.

### Removed

- **`rushdb serve --verify`.** There is no offline integrity check or manual
  repair step to run: derived state is checked as it is read and repaired by
  the engine (see Added).

### Fixed

- **SQL joins through relationship tables start from the selective side and
  fetch the rest by key.** `DataFusion` joined tables in the order a
  statement names them, with no row counts, and built a hash table of the
  first one: `review JOIN rushdb_edges.has_business JOIN business WHERE
  business.is_open = 0` collected every review (millions, with their text)
  before the business filter narrowed anything, so even `LIMIT 100` ran out
  of memory.
  - A chain of inner joins is now ordered from its smallest table outward,
    using the store's exact counts (label and relationship-type counters,
    the planner's count for a pushed filter).
  - A join on a label's `__id` fetches the matching records by id, and a join
    on a relationship table's `__source_id` or `__target_id` fetches each
    record's relationships through its adjacency (`LookupJoinExec` in
    `EXPLAIN`). Such a join reads only the rows that match and holds one
    batch at a time, instead of scanning the table into a hash table.
  - On a store of 10M records, that query's first 100 rows went from refused
    to under 4 s, reading the 135,402 matching reviews by id.
- **SQL top-k queries read wide columns for the rows they return.** `SELECT
  DISTINCT r.__id, r.stars, r.text, b.name ... ORDER BY r.stars LIMIT 100`
  read every match's `text` and grouped all of them before keeping 100 rows.
  - A label column the statement does not filter, join or order by is now
    fetched by `__id` after the cut, for the k rows returned only: a record's
    columns are fixed by its id, so ordering and deduplicating without them
    gives the same rows.
  - `DISTINCT ... ORDER BY ... LIMIT k` keeps the k smallest distinct rows as
    they stream (`DistinctTopKExec`), in memory for k rows instead of every
    distinct row.
  - A count over a `DISTINCT` query (how a client totals a search: `SELECT
    count(*) FROM (<query>)`) deduplicates on the record ids alone.
  - A relationship-table lookup that reads only endpoint ids builds its rows
    from adjacency, without rebuilding each relationship.
  - On a store of 10M records, the 654,598 reviews of open businesses ordered
    by stars: a page of 100 went from 43 s to 12.5 s, its total from 46 s to
    10 s.
- **A SQL statement's working memory is `query-memory-max`, not the response
  size.** Sorts, groups and joins were limited to the response budget plus
  16 MiB (32 MiB with the default 16 MiB body limit) even under a 256 MiB
  `query-memory-max`. The response budget now bounds only the rows returned.
  Each statement charges the server's shared SQL budget as its memory grows
  (concurrent statements stay within it; a statement it cannot hold is
  refused as retryable), and `query-memory-max = 0` means 256 MiB for SQL,
  never unbounded. RushDB's own SQL operators (top-k `DISTINCT`, lookup
  joins) count their rows against it too.
  A result whose rows came from a sort could also be refused for the size of
  the sort's whole buffer rather than its own rows; the rows are now metered
  by their encoded size.
- **SQL lookup joins read their own key.** A join chain's rows repeat
  column names (`__id`, `__target_id`); a lookup join took the first column
  of that name, so a three-hop join could look relationships up by the wrong
  table's ids and return nothing.
- **An outer join under `ORDER BY … LIMIT` keeps its unmatched rows.** A
  wide column of the null-padded side was fetched back by an inner join on
  its `__id`, dropping every row without a match.
- **A relationship lookup reads an endpoint's relationships a batch at a
  time and stops at the deadline**, however many relationships the endpoint
  has; full relationship-table scans stop at the deadline too.
- **MongoDB `$group` then `$sort: {_id: 1}` works again.** The aggregate
  order check refused `_id`; it now sorts by the group key. A `$sort` before
  `$group` (which only orders its input) is dropped, and refused with
  `$first`; a compound `_id` and a second `$group` are refused instead of
  run as something else.
- **Ordered pages hold bounded memory.** A deep page read from the end of a
  large index match walked the owner set backwards by copying it; a
  descending index page over a value held by many records read every one of
  them before its first row. Both now stream. `describe.explain` plans the
  order path from counters only (none is reported for a filtered page), and
  a page whose ordered walk gives up reuses its predicate plan.
- **PostgreSQL clients and BI tools can connect and list the database.**
  Metabase could not connect: the JDBC driver asks `SHOW TRANSACTION
  ISOLATION LEVEL`, which was refused.
  - The session's transaction characteristics are answered and set
    (`transaction_isolation`, default `read committed`;
    `transaction_read_only`, which refuses writes with `25006`).
  - `BEGIN … COMMIT/ROLLBACK` blocks run their reads; a write inside a
    block is refused, as it would commit on its own.
  - `pg_catalog` (`pg_namespace`, `pg_class`, `pg_attribute`, `pg_type`, …)
    and `information_schema` (now with PostgreSQL's columns and type names:
    `data_type` is `bigint`, `text`, …, no longer Arrow's names) list what
    the session can see, with the catalog functions tools call and
    `format()`. Checked end to end with Metabase 0.65.
- **Deleting by filter and batch writes obey an enforced data-access
  policy.** A delete-by-filter could remove records of a denied label, or
  choose them through a denied relationship type; a batch could delete or
  update a denied label's records or delete a denied relationship. Both are
  refused now, the first like a search of the same filter.
- **Describing a statement over the PostgreSQL wire shows only what the
  policy shows.** Parse/Describe planned against the whole catalog, so a
  restricted session could learn hidden tables' and columns' names and
  types.
- **Shutdown closes the store cleanly.** A request still running at
  shutdown (on the PostgreSQL wire, gRPC or Bolt), an idle pooled
  PostgreSQL connection, or a long maintenance step could hold the store
  past the drain, and the next start repaired the whole file. Requests on
  every transport and open reads are cancelled, PostgreSQL sessions closed,
  maintenance stops between steps, and the process exits only after the
  store has closed.
- **A malformed `since` on the replication WAL tail is refused** instead of
  replaying from the first commit; an online base backup writes only inside
  the data directory's `backups` folder; a single-tenant HTTP request with an
  `X-RushDB-Tenant` header is refused like gRPC and Bolt already were; a
  `$relation` hops search fails past one million reached records instead of
  growing without bound.
- **SQL limit errors name the limit that tripped.** Every SQL resource
  refusal said "SQL snapshot exceeds the 100000 row safety limit", followed
  by a byte count when the limit was the response size or the memory pool.
  Now:
  - "SQL result exceeds the 100000 row safety limit (N rows); add a LIMIT or
    narrow the query";
  - "response would exceed the response size limit (N bytes); raise
    --body-limit or add a LIMIT";
  - "query working set exceeded" for `query-memory-max`;
  - a retryable refusal when the SQL memory shared by concurrent statements
    is in use.

  Clients matching the old message must match the new ones.

## 3.0.9 — 2026-11-12

### Changed

- **`linkRecords` and `unlinkRecords` commit in chunks of 10,000 pairs
  (behaviour change).** They used to stage every edge of a join in one
  atomic commit. A join that creates or deletes up to 10,000 edges is still
  one atomic commit. A larger one now commits chunk by chunk, so a stop
  partway leaves the committed chunks in place. Both stay idempotent:
  running the same call again creates (or deletes) only what is left, and
  `skippedExisting` counts the edges already there. Callers that need a
  larger join to be all or nothing must split it themselves (narrower
  `predicate`s, or `linkRecordsPairs` per request).

### Fixed

- **`$sum` over an arithmetic expression is in the rows** (#436).
  `{"$sum": {"$multiply": ["$record.price", "$record.qty"]}}` was computed
  but left out of every row; a global rollup over no matches gives `null`,
  like `$sum` of a field.
- **`"$alias.field"` reads the related record, in `groupBy` and in
  `select`** (#437, #440). It was taken as a root property literally named
  `alias.field`, so every record grouped under `"__missing__"` and the
  projection was left out of every row. A group key is named by its
  property (`"$c.city"` → `city`); a record whose alias binds several
  records with the property gets the array of their values. A reference to
  an alias that no `$alias` in the predicate binds is refused with
  `invalid_request` instead of being read as a root property.
- **An alias binds only the records that match the blocks nested below
  it** (#438). In `{"ORDER": {"$alias": "$o", "PRODUCT": {"category":
  "office"}}}`, alias metrics (`{"$count": "$o"}`) and `$collect` from `$o`
  bound every order of the root record, not just those with an office
  product; the root records were filtered right. The two walkers that built
  alias paths are now one, and a path's level keeps its nested blocks.
- **An aggregation's `order` must name one of its outputs** (#439). A key
  naming no `select` output or group key (a typo) was ignored and, with a
  `limit`, silently returned the wrong rows; it is now refused with
  `invalid_request` listing the outputs. Listings still order by any
  property.

- **`linkRecords`, `unlinkRecords` and `joinPairs` no longer rebuild every
  record of both sides.** Each side was read as full records through a
  page whose 100,000 limit was never applied, so any side was read whole.
  A join of a few million records to a smaller label failed with
  `resource exhausted: query working set exceeded 268435456 bytes` before
  linking anything.
  - Both sides are now resolved to ordinals (8 bytes each).
  - The smaller side's key values are held in a map charged against
    `query-memory-max`.
  - The larger side streams past it, reading only its key property.
  - Memory follows the smaller side and one chunk, not the join. A request
    cancelled or past its deadline stops between records.
- **Ordered and deep pages stop at their rows.** The planner used to plan
  a records page by its filter only and then read every candidate to order
  it, so ordered pages and deep `offset`s cost the whole label (a page of 100
  ordered by record id, or the last page of a listing, took seconds to
  minutes on a store of millions). It now also costs reading a source
  already in the requested order and stopping at the page ("order-first")
  against reading every candidate, from the counters it already uses, and
  takes the cheaper:
  - Record id order (`"order": "asc"` / `"desc"`), from the identity rows,
    either way.
  - Creation order (no `order`) and id order from the far end, for a page
    past the middle of the matches.
  - One property (`{"field": …}`): the records missing it (first ascending,
    last descending) are found without reading the property, and only the
    property's holders are read and sorted. A property few records hold no
    longer costs every record.
  - An ordered index on the sort property, from either end, when the
    property holds one type (integers, datetimes or strings) and every
    label holding it has one.

  A walk that passes more records than reading every candidate would gives
  up and the page is read the plain way, so a wrong estimate costs at most
  twice. Ids are read from the record header instead of rebuilding the
  record. `describe.explain` reports the choice as `orderPath`. Storage
  gained a reverse key seek for the descending walks; it also makes the
  min/max lookups of value dictionaries one seek instead of about nine per
  key byte.

## 3.0.8 — 2026-11-10

### Changed

- **An unfiltered `describe.properties` with full detail reads its samples
  through each property's presence rows.** It used to read every occurrence
  of the first 10,000 records per call, although only the sample values came
  from that read (counts, shape and ranges come from the catalog). Each
  property's samples are now read from the sampled records that hold it, in
  the same order, until they are complete: the cap of 10, or every distinct
  value the label holds when that is fewer. The response is unchanged.
  Measured on 10,000 drifting nested documents (3 labels), the four calls an
  agent makes to describe a schema took 48 ms and now take 11 ms.

## 3.0.7 — 2026-11-09

### Fixed

- **A crash while a write-ahead log segment was being created no longer
  stops the next start.** The new segment could be left shorter than its
  header, and opening the log refused it as corrupt. Such a segment holds
  no record (an append is acknowledged only once the header and the record
  are durable), so the newest segment in that state is removed and the log
  resumes; an older segment that short is still reported as corrupt.
- **Multi-hop `$relation` queries no longer check every record of the
  label.** A `$relation` block with `hops` was evaluated by a separate
  breadth-first search from every candidate record, so a neighbourhood query
  anchored on one record cost the label times its neighbourhood: on a graph
  of 100,000 records and 1.26M relationships, 1 hop took 15 s and 2 hops
  more than 10 minutes. The planner now resolves the block's own labels and
  predicate first (an ordered index on the anchor property makes this a
  lookup) and expands backwards from those records, when they are at most
  half as many as the root records it would otherwise check. Leaves and
  expansions are ordered together by expected set size, so a root condition
  such as `city = "Lisbon"` is checked only on the records the expansion
  reached. Measured in-process on that graph with `node_id` indexed, median
  of 40 anchors: 1 hop 0.5 ms, 2 hops 7.6 ms, 3 hops 147 ms, 2 hops with a
  `city` condition 3.0 ms. `describe.explain` reports the plan as
  `accessPath: "relation_expand"` with `relationExpansion.targetEstimate`.
- Automatic compaction logs "store file is over compact-max-file-bytes"
  once when the file passes the cap, not on every 5-minute tick.
- **A `select` of plain fields is a listing, not an aggregate.** A projection
  such as `"select": { "doc_id": "$record.doc_id" }` with a large `limit` was
  run by the aggregate executor, which read every match before paging and
  refused the rows with `resource_exhausted` on an estimate of ~770 bytes per
  row, about 40 times their JSON: 50,000 integers under 1 MB were refused
  against a 16 MiB response. A `select` made only of field references (and
  `$collect`s), without `groupBy`, now runs as a listing: the same matching,
  paging and early stop as a records query, only the selected properties of
  the page read, and each row charged by its serialized size against the
  response size limit. The response keeps its shape (`rows`, selected keys
  in `select` order, `total` of every match), `order` may name an output
  key, and a projected property hidden by an enforced policy is still
  refused. A MongoDB `find` with an inclusion projection reads only the
  projected fields the same way.
- **Aggregate rows are charged by their real size.** Rendering an aggregate
  charged an estimate of every bucket against the smaller of the response
  size limit and `query-memory-max`; each row is now charged by its
  serialized size, as it is rendered, against the response size limit.
  Accumulating the groups still runs under `query-memory-max`.
- **A budget refusal names the budget that tripped.** Listings,
  projections, aggregates and catalog reads say either
  `query working set would exceed query-memory-max (N bytes); raise
  --query-memory-max or narrow the filter` or `response would exceed the
  response size limit (N bytes); raise --body-limit or request a smaller
  page`. Before, an aggregate said `query-memory-max` even when the response
  size limit tripped, and a listing said only `query working set exceeded N
  bytes`. Clients matching the old messages must match the new ones.
- **A filtered vector search costs what its candidates cost.** A predicate
  is resolved into a sorted owner set, and the search is routed on its size:
  `auto` searches exactly when at most `resources.vector-exact-max-pool`
  records match (before, it looked only at the whole index's size, so a
  narrow filter on a large index walked the ANN graph). Exact search over a
  filter reads only the matching records' vectors, one lookup each, instead
  of scanning every binding of the index, so its cost no longer grows with
  the index. Above the threshold, the ANN graph walk skips non-matching
  records (usearch filtered search) instead of post-filtering a gather sized
  from a 256-entry selectivity sample, which at low selectivity gathered the
  whole graph. `query.hybrid`'s vector leg routes the same way.
- **The write counters report from the first commit.**
  `rushdb_storage_wal_bytes_total`, `rushdb_storage_applied_bytes_total`,
  `rushdb_storage_checkpoints_total`, `rushdb_storage_checkpoint_wait_ms_total`,
  `rushdb_storage_admission_waits_total` and
  `rushdb_storage_admission_wait_ms_total` were read only from the storage
  walk that refreshes every 15 minutes, so they stayed 0 until its first run
  (and lagged by up to 15 minutes after it). A scrape now reads them live.
- **An idle store writes nothing per maintenance tick.** The periodic
  checkpoint (an fsynced redb commit that also records the allocator state)
  ran every 5 s whether or not anything had changed. A shard with no write
  since its last checkpoint, at the same LSN, now skips it, so the store
  file is not written while idle; WAL truncation still runs. `rushdb_storage_checkpoints_total` counts only checkpoints that
  wrote.
- **Nested-document ingest links children in linear time.** The
  relationship commit of a plain-create import found each relationship's
  two endpoints by scanning every imported record, so a document with N
  children cost N² comparisons; it now uses the same map as the merge path
  (one implementation for both). Inline vector binding after an ingest
  looks up its records the same way.

- **Exact vector search is several times faster.** Scores are computed by
  one kernel over 8 independent `f64` lanes (AVX2 when the CPU has it,
  chosen at runtime; a portable kernel otherwise, returning the same bits),
  with the query's norm computed once per search instead of per vector, and
  each stored vector is scored straight from its bytes instead of being
  decoded into a new buffer. An unfiltered search checks a hit's label
  membership only when the hit would enter the top `k`, not for every vector
  it scores. A large exact search is split into fixed-size parts scored on
  the server's shared thread budget (`RUSHDB_THREADS`) and merged in order:
  the result is identical whatever the number of threads. Measured
  in-process on 100,000 random 1536-d cosine vectors in an on-disk store,
  top 10: 595 ms before, 422 ms on one thread, 174 ms with 16; most of what
  remains is reading each 6 KB vector from the store.
- **`ef` is honoured below 128.** ANN segments walked at least 128
  candidates whatever `ef` a query asked for. The walk now keeps exactly
  `max(ef, offset + limit)` candidates, so a smaller `ef` is cheaper (and
  less exact) and a larger one more exact, per query. On the same 100,000
  vectors, p50 at `ef` 32 went from 1.6 ms to 0.5 ms; at 128, 2.0 ms to
  1.9 ms.
- **An unfiltered `query.hybrid` no longer reads the whole label.** Without
  a `predicate`, every member of `labels` was resolved into an owner set
  before either leg ran, so each query cost the label's size. The vector leg
  now takes the unfiltered route of `query.vector` and the text leg searches
  its index whole, both checking label membership on their hits (a text hit
  whose record left the label sends that leg back to the owner set, so the
  result is unchanged); the reads of such a query no longer grow with the
  label. The two legs also run concurrently.
- An ANN build reads and decodes its vectors in parallel parts, one bounded
  chunk ahead of the graph inserts, which stay on one thread in key order:
  every build of the same vectors is the same graph, with the recall of a
  sequential build. (Inserting from several threads built a graph about 4×
  faster but lost 0.1–0.5% recall@10 at the default `ef`, so it is not
  used.)

### Changed

- **Vector search scores stored vectors in place.** Exact search, the ANN
  rerank and segment builds read each candidate's vector through a borrowed
  point read (`ReadView::with_value`) that hands the stored bytes to the
  scoring kernel where the store holds them (the pinned redb page, the
  in-memory map) instead of copying every vector into a new buffer first.
  Visibility, read limits, cancellation and read accounting are those of
  `get`, which is now the same read plus a copy. Measured in-process on a
  redb store with 100,000 vectors of 1,536 dimensions (shared host), exact
  top-10 p50: 262 ms before, 253 ms now on one thread; 81 ms before, 79 ms
  now on eight. The remaining cost is the per-candidate point lookup itself.
- **Grouped aggregates read each record once and run on several threads.**
  An aggregate with metrics (`groupBy` payment: `$count` + `$sum` fare)
  read the record header of every candidate again, although a label's
  members and the index's owners are live records by construction; probed a
  property's newest typed definition first, so a CSV column inferred as both
  Int64 and Float64 cost two point reads for most rows; and built a JSON
  object, a rendered JSON group key and JSON metric maps for every row. It
  now reads no record header per candidate, probes typed definitions in the
  order most records of the labels hold them, groups on typed keys with
  typed counters, sums and extremes (JSON once per group), and splits the
  candidates into parts of about 8,192 records that run on the server's free
  worker threads (`threads`) and merge in part order. A label walk no longer
  collects every member's ordinal first: each part reads its own range of
  ordinals. Results do not depend on the number of threads: part bounds
  depend only on the data, groups and extremes are those of one pass, and a
  floating-point `$sum`/`$avg` is the sum of per-part sums in part order
  (deterministic for a given dataset; it may differ in the last bits from a
  strictly left-to-right sum). The partial groups of every part count
  toward `query-memory-max`; cancellation and the request deadline stop
  every part. Grouping semantics are unchanged (an integer `15` and a float
  `15.0` are separate groups). Measured in-process on 500,000 records in a
  redb store (shared, loaded host), `groupBy payment: count + sum fare`:
  0.92 s before, 0.61 s now on one thread, 0.20 s on eight;
  `groupBy vendor: avg + max`: 1.2 s before, 0.80 s / 0.21 s now.
- **A filter without an index checks its owners on several threads.** An
  unindexed equality, `$in` or range leaf (`payment = "card"`) read each
  owner's value one after another, which dominated a filtered aggregate
  over a large label. The owners to check are now split into parts of
  about 4,096 (the candidates earlier leaves left by rank, the property's
  presence rows by ranges of record ordinals that each part reads itself)
  that run on the server's free worker threads (`threads`) and merge in
  part order: the owner set, whether it passes the owner budget, and the
  result are the same whatever the threads lent, and cancellation is
  checked between rounds of parts. Measured in-process on 300,000 records
  in memory, `where: {payment: "card", passengers: {$gte: 2}}`: grouped
  count 0.46 s on one thread, 0.20 s on eight; the same filter as a
  listing 0.43 s / 0.19 s.
- A batch that writes one property of a record twice (`{"x": 1, "X": 2.5}`,
  or two updates in one transaction) keeps only the last value. It kept the
  first value too when the two had different types (or the first was a
  longer array), so the record held two values of one property.
- **Bulk ingest copies a commit's rows once.** One ingest request is still
  one atomic commit, but its rows no longer pass through a copy at every
  stage. The import plan takes the request's documents (or CSV rows) by
  value and moves each record's fields into the commit; `ingest.csv` streams
  decoded rows into the plan in slices of 1,024 instead of building every
  row as a JSON object first. The commit sequencer keeps one shared,
  key-indexed layer per in-flight commit (the commit's own ops, shared with
  the publisher queue) instead of copying every in-flight key and value into
  a new overlay for each admitted commit. The publisher derives against
  borrowed group state, applies base and derived ops as two slices of one
  write instead of concatenating them, and hands the conflict window the
  write set it already holds. redb takes a replaced row from `insert` and
  `remove` instead of a lookup before each write. Transient heap of a
  plain-create ingest, per row: 3.1 KB, down from 10.3 KB (counting
  allocator, 20,000 taxi-shaped rows, in memory). Release server, 200,000
  taxi-shaped CSV rows on one host under heavy unrelated load (two runs
  each, so treat the ranges as indicative): two 8 MiB requests peaked at
  2.2 GB RSS, down from 3.4 GB; 33 requests of 256 KiB from 8 clients
  loaded 23,700–27,500 rows/s on 2.8 cores, up from 14,400–18,200 rows/s on
  2.0 cores, at 1.4 GB peak RSS instead of 1.8–2.1 GB.
- **A large commit derives on several threads.** Occurrence-derived rows
  (presence, value dictionary, null and relationship-shape counters) of a
  commit touching more than 4,096 property occurrences are derived in parts
  split at record boundaries, on the shared worker-thread budget
  (`RUSHDB_THREADS`), and summed in order; the rows are exactly those of a
  single-threaded pass (checked against the from-scratch derivation).
- Embedding API: `rushdb_storage::StorageStatistics` no longer carries the
  write counters (read `Store::write_counters()`), `Store::checkpoint()`
  returns whether it wrote, `rushdb_lmpg::import::import_bfs_with_cancel`
  takes the plan by value, and `serialize_bfs_owned`, `coerce_value` and
  `BfsPlan::append` are the by-value forms of the import planner.
- **Breaking: `$relation` paths are trails.** A path never uses the same
  relationship twice, as in Cypher variable-length patterns; it may pass
  through a record more than once. Before, a path never revisited a record
  but could leave and come back over one relationship, so with
  `"direction": "both"` and `max` of 2 or more the record a block anchors on
  matched its own neighbourhood query. It now matches only over a closed
  trail (a cycle of at most `max` distinct relationships).
- Traversals keep a relationship type's adjacency in memory (a resident
  graph per snapshot) once the adjacency rows they read at that snapshot add
  up to the cost of building it. New budget `resources.graph-cache-bytes`
  (`--graph-cache-bytes`, `RUSHDB_GRAPH_CACHE_BYTES`, default 128 MiB); `0`
  keeps none. `$cycle` builds its per-query graph from the same resident
  adjacency.
- Graph-language statements read a relationship's type from the adjacency
  row that listed it instead of reading and decoding its header for every
  edge a variable-length pattern walks.
- Read accounting (`rushdb_storage::read_accounting`) counts the in-memory
  backend's reads too; it counted only redb's, so read-count assertions on
  in-memory stores passed without measuring anything.
- **Behaviour change: `vectorSearch.pool` is the pool the search could
  score**: the records matching `predicate` (at most the index's vectors)
  when filtered, the index's vectors otherwise. It used to be the index's
  vectors in every case.
- A forced `mode: "ann"` whose filtered pool is within
  `resources.vector-exact-max-pool` is answered exactly and reports
  `fallback: "smallCandidateSet"` (also a `reason` of
  `rushdb_vector_searches_total`).
- `describe.explainVector` resolves the named `index` (and requires one when
  `resources.require-vector-index-name` is on, like the search), counts
  `candidateCount` from the same owner set the search uses instead of
  reconstructing every matching record, and reports `search`: the
  `vectorSearch` report the search would return, decided by the same
  routing rule. Without a `predicate`, `candidateCount` is the index's
  vectors.

- **Binary vectors on the wire.** Every vector a request carries
  (`upsertVectors` items, inline `vectors`/`$vectors`, `query.vector`, the
  hybrid vector leg) may be sent as `{"f32": "<base64>"}`: the components as
  little-endian 32-bit floats, base64-encoded. It keeps every bit and takes
  about a quarter of the text of a JSON number array; a wrong length, a NaN
  or an infinity is `invalid_request`. Number arrays are still accepted.
- `resources.vector-default-ef` (`RUSHDB_VECTOR_DEFAULT_EF`) defaults to
  128 instead of 64, and `ef` is the ANN walk breadth: the default search
  walks exactly as wide as before and re-scores the 128 candidates it kept
  (it re-scored 64). Set it lower to trade recall for latency.
- **Stored format: a vector binding row is 17 bytes** (slot, source hash,
  flags) instead of a JSON document; its key already names the space and
  source. Exact search no longer parses JSON per vector. Existing stores are
  upgraded automatically: the first start of 3.0.7 rewrites every JSON
  binding row of every tenant in the new form before the node serves
  (migration `0001-binary-vector-bindings`, committed through the write-ahead
  log in batches of 4,096 rows; a start that stopped part way finishes it,
  and starting an up-to-date store writes nothing; logged as
  `data migration applied`, boot phase `migrations`). A row in
  neither form stops the start with its key named. A replica receives the
  leader's rewrites. No re-indexing is needed.

## 3.0.6 — 2026-11-08

### Security

- **`--pgwire-port 0` disables the PostgreSQL wire listener** (#429, #431).
  Port 0 bound pgwire on an ephemeral port, while the non-loopback listen
  guard treated it as disabled, so `--listen 0.0.0.0 --pgwire-port 0` with
  `sql` enabled opened a cleartext pgwire listener the guard never checked,
  also on multitenant servers. Port 0 (or `server.pgwire-port: 0`) now binds
  nothing, as for gRPC and Bolt; the startup log reports `pgwire_port=0` and
  SQL stays available over HTTP. A non-zero cleartext pgwire port on a
  non-loopback multitenant listener is still refused.

### Fixed

- **Bolt selects the tenant the way drivers send it** (#430, #432). Neo4j
  Bolt 5 drivers send the session database in `BEGIN` and auto-commit `RUN`,
  not in `HELLO`, so every driver statement on a multitenant server failed
  with `scope is required in multitenant mode`. `BEGIN` `db` now selects the
  tenant of the transaction and `RUN` `db` that of an auto-commit statement;
  `HELLO.db` is still accepted as the session default. A capability token
  bound to a tenant logs in without `db` and is refused with
  `Neo.ClientError.Security.Forbidden` for any other tenant; the bootstrap
  token may name any tenant per transaction.
- **`neo4j://` routing works.** `ROUTE` used the wrong message tag and table
  shape, so routing drivers never connected; it now answers the routing table
  drivers read (`ROUTE`, `READ` and `WRITE` on this node, `--bolt-advertised-address`).

### Changed

- On a single-tenant server, a Bolt `BEGIN`/`RUN` naming a database other
  than `neo4j` is refused with `scope_mismatch`, as `HELLO.db` already was;
  it used to be ignored and write to the one keyspace.
- tantivy 0.27.0 (from 0.26.1; the on-disk index format is unchanged, so
  existing text indexes open as before) and turbovec 1.1.2 (optional
  `turbovec` vector feature, not in release builds).

## 3.0.5 — 2026-11-08

### Fixed

- **Dropping a vector index frees its vector space** (#423, #424). A drop
  kept the index's space row, and the limit of 256 vector spaces per tenant
  counted those rows, so after 256 create/drop cycles a tenant could no
  longer create vector indexes even with none left. A drop now deletes the
  space row, its slot counter, its ANN manifest and its persisted segments,
  and the limit counts live indexes, so tenants already at the limit on
  3.0.4 recover on upgrade with no migration. Past the limit, `createVector`
  answers `resource_exhausted` (not retryable) with "vector space limit
  reached (256 per tenant)" instead of an opaque `execution_failed`.
- **Concurrent transactions that create records no longer conflict** (#425,
  #424). Each transaction allocated record and relationship ids from the
  counter as of its snapshot, so concurrent creators were handed the same id
  and about 75% of commits aborted with four writers, even on disjoint
  labels. Ids are now reserved when a write is staged, from one allocator per
  store (like a PostgreSQL sequence: an aborted transaction leaves a gap), the
  counter only moves forward, and commit-time expansion reads current state.
  Writes to the same record, records read with `getRecord`, two transactions
  introducing the same new property name, and writes during an index build
  still conflict, as documented.

## 3.0.4 — 2026-11-07

### Added

- **TLS and tenant-bound capabilities on the PostgreSQL listener** (#394).
  With `--tls-cert`/`--tls-key`, the same certificate as HTTP, pgwire speaks
  TLS (`SSLRequest` and PostgreSQL 17 direct TLS) and refuses a plaintext
  session with SQLSTATE `28000` before any password is sent. A capability
  token is accepted as the password, bound to the tenant in `database`; a
  statement the capability's scope does not grant fails with `42501`, as
  do operator operations. The listen guard judges pgwire on its own: on a
  non-loopback multitenant listener it needs TLS and `--capability-secret`;
  on a single-tenant one TLS alone removes the need for
  `--allow-insecure-listen`. gRPC and Bolt are unchanged.
- **TLS and tenant-bound capabilities on the gRPC and Bolt listeners**
  (#395). With `--tls-cert`/`--tls-key` gRPC serves HTTP/2 over TLS and
  Bolt serves direct TLS (`bolt+s://`, `neo4j+s://`); each then speaks only
  TLS. A capability token is accepted as the gRPC bearer token (bound to
  `x-rushdb-tenant`, checked per call) and as the Bolt `HELLO` credentials
  (bound to `db`, checked at login); an operation it does not grant is
  `PERMISSION_DENIED` on gRPC and `Neo.ClientError.Security.Forbidden` on
  Bolt. One rule decides every transport's credential. The listen guard
  treats gRPC and Bolt as it treats pgwire: on a non-loopback multitenant
  listener they need TLS and `--capability-secret`. `--bolt-advertised-address`
  names the `host:port` a Bolt `ROUTE` answer returns.
- `--body-limit` / `RUSHDB_BODY_LIMIT` sets the request body, gRPC message
  and response size limit from the command line (`server.body-limit` in
  `config.yml` keeps overriding it, and disagreeing with an explicit flag is
  refused as for every other setting).
- `query.text` and the hybrid text leg take `syntax`: `"terms"` (the
  default) searches the query as words, `"query"` reads the operator syntax
  (`+`, `-`, quotes, parentheses, `AND`/`OR`/`NOT`).
- `--compact-max-file-bytes` (default 1 GiB): the largest store file
  automatic compaction rewrites. A compaction holds every request while it
  copies the file; a larger file is left to the offline
  `rushdb serve --compact`, and the skip is logged.
- `rushdb_storage_wal_bytes_total` and `rushdb_storage_applied_bytes_total`
  count the two write streams a commit produces, to attribute disk writes.
- **`mutate.updateRelationshipsById` edits relationship properties.** For
  every source and target id it updates the existing relationships of
  `relationType` in `direction`: `set` writes properties (`null` stores an
  explicit null), `unset` removes them, and a name in both is
  `invalid_request`. A pair without the relationship is skipped; an unknown
  id fails the call before anything is written, and all updates are one
  commit. The answer is `{"kind": "mutation", "affected": <relationships
  updated>}`, and `describe.schema` reflects added and removed relationship
  properties. `describe.capabilities` advertises `updateRelationshipsById`.
- **Typed relationship properties.** `linkRecordsById`, `linkRecordsPairs`
  and `updateRelationshipsById` take an optional `types` map from property
  name to `string`, `number`, `boolean` or `datetime`. A `datetime` takes
  an ISO 8601 date or datetime string and stores a datetime, so a
  relationship can now hold one (a date string sent untyped still stays a
  string). Every typed name must be among the written properties, and a
  non-null value must match its type (datetime arrays are rejected, as for
  records); otherwise the call is `invalid_request` and nothing is written.
  `null` stays an explicit null. `describe.capabilities` advertises
  `relationshipPropertyTypes`.

### Changed

- **Text search reads plain words by default.** `query.text` and the hybrid
  text leg handed the string to the query parser, so a colon, an apostrophe
  or the words `AND`/`OR`/`NOT` failed with `execution_failed` and a
  message only the server log held. The query is now tokenized by the
  index's own analysis and every character is text; the operator syntax is
  `syntax: "query"`. SQL's `text_search`, Cypher's and GQL's
  `text_match`/`text_score` and Mongo's `$text` keep the operator syntax
  they document. A text search's failures are typed: an unknown index is
  `not_found`, a name of another index kind or a query that does not parse
  (under `syntax: "query"`) is `invalid_request` with the parser's message,
  an index still building is `unavailable` and retryable. A query can no
  longer address the index's internal `owner` field.
- **A request body over the limit is a protocol error.** The HTTP layer
  answered 413 with the plain text `length limit exceeded` before the
  handler ran, so a JSON client never saw the reason. Every body is now
  read through one bounded reader after authentication, and the answer is
  the envelope with the new code `payload_too_large` and the limit in
  bytes in the message (gRPC `RESOURCE_EXHAUSTED`, SQLSTATE `54000`,
  Bolt `Neo.ClientError.Request.Invalid`). An unauthenticated oversized
  request is 401, as the auth-first rule says.
- **Relationship datetime properties are returned as ISO 8601 strings.**
  `query.relationships` returned a relationship's datetime property as
  epoch milliseconds with nothing marking it a datetime. Each relationship
  now carries a `propertyTypes` map (name -> `string`, `number`, `boolean`
  or `datetime`), as records do, and a datetime property is its ISO 8601
  string. The `describe.schema` fallback for stores without a relationship
  catalog reports such a property as `datetime` instead of `number`.
- A tenant purge erases 4,096 keys per commit instead of 1,024: a quarter
  of the commits, WAL records and fsyncs for the same tenant.
- `/ready` answers 503 (`storage compaction holds the database`) while a
  compaction runs; `/health` stays a liveness probe. The start of a
  compaction is logged with the file size.

### Fixed

- **Every request blocked after a large bulk load while `/health` stayed
  200** (#408). The first idle compaction after a load rewrote the whole
  store file under the database lock, which every read and commit waits on:
  a 1.9 GB file on a 2 GiB container held the node for over 20 minutes.
  Automatic compaction now runs only on files up to `compact-max-file-bytes`
  (1 GiB) and otherwise logs the skip and leaves the rewrite to the offline
  `rushdb serve --compact`; a running compaction is logged with the file
  size and makes `/ready` answer 503.
- **Grouped aggregations over large labels failed with
  `resource_exhausted`.** The aggregate working set was capped at the
  request's 16 MiB response budget: the match ordinals of 1M records
  filled it, an `$avg` tripled the charge, and a filtered query's presence
  scan ran under a 1 MiB materialization limit. The working set is now
  governed by `query-memory-max` alone, like a records search; the response
  budget caps only the returned page. The error names the limit and the
  flag.
- **Grouped aggregations read every record in full.** Each row was rebuilt
  as a whole JSON document with a fresh reconstruction session, and buckets
  were found by comparing every bucket's keys per row. The aggregate now
  reads only the properties it names by point reads (one session for the
  run, hashed buckets), the residual filter does the same, and
  `groupBy` on one property with `$count: "*"` over one label is answered
  from the value dictionary without reading a record (explicit nulls and
  records without the property keep their own rows).
- Other reads that rebuilt whole records to use one or two properties now
  decode only those: a traversal step's and a `$relation` endpoint's
  filter, alias metrics (`$sum: "$alias.prop"`), a `$collect` with a
  `fields` projection, `mergeBy` upsert lookups (including the label scan
  of an unindexed key), the text index applier for a single-property index,
  and the inline-vector listing.
- **CSV ingest was about five times slower than the same rows as
  documents.** The import serializer popped its queue with `Vec::remove(0)`,
  quadratic in the rows of one call, which an 8 MiB CSV chunk (about 240K
  rows) made dominant. The queue is a deque. `dynamicTyping` now defaults
  to `suggestTypes` as the import path reads it (`true` unless set): the
  CSV decoder had its own `false`.
- The HTTP error taxonomy test lists `write_conflict`.
- A write to a relationship type in the policy's `deniedRelTypes`
  (`linkRecordsById`, `linkRecordsPairs`, `updateRelationshipsById`) is
  `unauthorized` with the type named, not `execution_failed`.
- A statement-language `SET` on a relationship property (Cypher
  `SET r.x = ...`) staged inside a transaction failed at staging; it now
  stages and commits with the transaction.

## 3.0.3 — 2026-11-05

No engine, protocol or storage changes: the binary behaves exactly like 3.0.2.

### Changed

- The Bolt and Cypher manual pages enable Cypher with `--languages sql,cypher`.
  `--languages` replaces the default set, so `--languages cypher` alone turned
  SQL off, and RushDB Cloud's Smart Search needs SQL.

## 3.0.2 — 2026-11-02

No engine, protocol or storage changes: the binary behaves exactly like 3.0.1.

### Changed

- Package and image metadata use RushDB's current description, "the database
  for AI": the container image label, the `rushdb-server` crate description and
  keywords, the Homebrew formula (homepage now https://rushdb.com) and the Helm
  chart description.
- The Apple silicon binary is built on macOS 15.

## 3.0.1 — 2026-11-01

### Changed

- Vector indexes have their own memory budget, `resources.vector-cache-bytes`
  (default 256 MiB), instead of sharing `logical-cache-bytes` with text and
  planner caches. A space of 1M 768-dimension vectors now answers from its
  index on default settings (it fell back to exact search before).

### Fixed

- **Data-access policy (security):** under an enforced policy, a record query that filters, sorts, groups or aggregates on a hidden property, or traverses a denied relationship type, is refused with `unauthorized`. Before, the output was masked but the hidden values still decided which records came back and in what order. A traversal must name its `$relation` type when the policy denies any type, and its endpoint label unless every label is allowed.
- **Nested logic:** `$or`/`$and`/`$nor`/`$xor`/`$not` inside a label block that is itself inside a block (`{ DIAGNOSIS: { CONCEPT: { $or: [...] } } }`) silently matched nothing, and with a `$relation` beside it the query failed. The inner block was read as a property; it is now parsed as the root parses it.
- **`retypeRelations`:** merging into a pair that already has the target type keeps every relationship property from both sides (the existing relationship's values win a conflict, and it keeps its id). Before, the retyped relationships' properties were dropped, and a newly created relationship carried none. `affected` now counts relationships retyped, and the response reports `merged` and `propertyConflicts`. A new `mergeStrategy` option picks the winner of a conflict: `keep` (default, as above), `append` (the retyped value wins), `rewrite` (the retyped relationships' properties replace the existing one's) or `fail` (refuse with `conflict`, writing nothing, and list the first 20 conflicts).

- Building an indexed filter's match set no longer re-counts the set for
  every owner it adds: the cost was quadratic in the tenant's size. The set
  is also kept per snapshot and shared by its readers, so repeated pages of
  an indexed filter no longer re-read the index.
- A vector search never waits for an index build: past a small tail of
  unindexed vectors it answers exactly (`fallback: "annWarming"`) while the
  index is built in the background.

- A relationship created in the same statement or batch as its new endpoint
  records is counted in the relationship catalog: `describe.schema` shows
  it and SQL has its `rushdb_edges` table. Relationship headers now apply
  after record headers in every commit.
- Cypher, GQL and Gremlin writes that create relationships stage in
  transactions, and so over Bolt in explicit and driver-managed
  transactions; they failed with "transaction stage does not support
  operation … yet".
- Bolt reports the query type (`w`, `rw`, `r`) and
  `stats: {"contains-updates": true}` in the summary that ends a write's
  result; drivers saw `query_type = 'r'` and no updates.
- A Cypher `CREATE` property computed from an expression (`r[0]`, `CASE`,
  arithmetic, a function call) fails with `unsupported` and the `WITH`
  workaround instead of an internal execution error (a Grafeo limitation).
- SQL/PGQ: two anonymous nodes in one pattern are separate nodes; they were
  joined as one (a Grafeo defect), which also made `a <> b` match nothing.
- Mongo-style `find` applies its inclusion `projection`: documents keep only
  the projected fields and `_id` (`"_id": 0` drops it).
- Describing a label's properties without a filter reads only its sample
  records, and value search skips columns whose type cannot hold the text;
  property ranges come from the value dictionary's first and last entries.
  None of this changes a result.
- Value search over long texts reads only the records that hold the column
  and matches ASCII text in place.
- A name held under several types on one label is searched once: `total` no
  longer counts its matches twice.
- `$type` matches the type each record holds when a name is stored under
  more than one type (`$type: "number"` no longer matches text values).
- Value counts, top values and value lists over arrays count each element
  once per record, as distinct counts already did.
- A `select` of metrics without `groupBy` over no matches returns one row
  (counts 0, other metrics `null`), as SQL does.
- An unfiltered property description samples every label, so a label
  written after 10,000 others' records still shows sample values.
- An in-memory node no longer rejects writes when the host disk is below the
  free-space threshold: the guard protects a data directory.
- The PostgreSQL wire `INSERT` tag is `INSERT 0 n`: psql no longer reports
  "could not interpret result", and psycopg's `rowcount` is the count, not -1.
- The PostgreSQL wire accepts the client parameters drivers and tools set on
  every connection (`application_name`, `extra_float_digits`, `DateStyle`,
  `TimeZone`, `client_encoding`, `search_path`, …) and `SHOW` answers them;
  Metabase sent `SET application_name` and could not connect. `SET` of
  another parameter fails with `0A000` naming the accepted ones.
- `current_database()`, `current_user` and the other session functions work
  beside other expressions over the PostgreSQL wire, not only alone.
- SQL `UNION`/`UNION ALL` works; it panicked inside DataFusion and returned
  `unavailable` (503). A panic inside the SQL engine now fails only its
  statement, with `execution_failed`.
- SQL statement errors carry their class instead of "internal execution
  error": an unknown column or function is `not_found` (`42703`/`42883` over
  pgwire), a mixed-type column compared with a number is `invalid_request`
  (`22P02`) naming `TRY_CAST`, an unimplemented type or feature is
  `unsupported`.
- SQL `SET`, `PREPARE`, `LISTEN`, cursors, `COPY` and table DDL fail with
  `unsupported` (`0A000`) and the reason, not "send writes as
  mutate.statement".
- SQL `CREATE INDEX … ON supplier (region)` indexes the stored label
  (`SUPPLIER`) a `SELECT` on `supplier` reads; it declared an index on a
  label `supplier` that nothing used.

## 3.0.0 — 2026-10-28

First standalone release of the Rust engine.

### Installation

- One-line installer for Linux (x86-64, arm64; glibc 2.28+) and macOS
  (Apple silicon, Intel; macOS 11+). It verifies the release checksum, and
  the Sigstore signature when `cosign` is installed.
- Container image `rushdb/rushdb` (also `ghcr.io/rush-db/rushdb`) for
  `linux/amd64` and `linux/arm64`, signed, with an SBOM and build provenance.
  The repository's `Dockerfile` also builds it from source.
- Homebrew formula in `rush-db/tap`, with a `brew services` definition.
- Helm chart `oci://ghcr.io/rush-db/charts/rushdb`: a single-node
  StatefulSet with a volume claim, a Service and an optional Ingress.
- Deployment templates in `deploy/`: Docker Compose (plain, and behind Caddy
  with automatic TLS), cloud-init for Linux VMs, the systemd unit, Fly.io,
  Render (with a Deploy button), Coolify and Dokploy.
- The README is translated into Chinese, Japanese, Korean, German, French,
  Portuguese, Spanish, Hindi, Arabic, Indonesian and Thai.
- `cargo binstall rushdb-server` installs the release binary;
  `cargo install rushdb-server` compiles it.
- Linux release binaries are built against glibc 2.28, and Linux arm64 is a
  new target.

### CLI

- One binary, `rushdb`: `rushdb serve` runs the server (it was the separate
  `rushdbd` binary), and `rushdb up` runs it in the same process with a
  `./.rushdb` data directory, takes every server option, and prints where the
  node is listening. The container image is `rushdb/rushdb`.
- New `rushdb health` command: exits 0 when the node's `/health` answers.

### Queries

- A filter without a usable index runs as a scan of the records it names,
  as in PostgreSQL and Neo4j, instead of being refused over 1,000,000
  records. `unindexed-walk-max-owners` now defaults to `0` (no limit).
- A request can bound its own queries, never loosen the server's bounds:
  `context.deadlineMs`, `context.queryMemoryMaxBytes` and
  `context.unindexedWalkMaxOwners` in the protocol envelope, and over the
  PostgreSQL wire the session settings `statement_timeout`, `work_mem`,
  `disallow_full_table_scans` and `large_full_scan_rows` (`SET`, `SHOW`,
  `RESET`). `context.deadlineMs` over HTTP shortens the server's timeout;
  it was ignored.
- SQL reads stream label tables and plan their filters on declared indexes,
  so `SELECT` works on labels with millions of records; `count(*)` without a
  filter reads the label's record counter. A SQL scan stops at the request's
  deadline between batches.
- An upsert on a `mergeBy` key without an index counts the key values it
  holds against `query-memory-max`.
- SQL `INSERT`, `UPDATE` and `DELETE` read their rows like `SELECT` and
  commit only the records they touch, so they work on labels of any size;
  the planned writes count against the request budget. `rushdb_edges` tables
  stream too, so the 100,000-relationship limit is gone, and resolve their
  columns and row count from the relationship catalog without reading
  relationships.
- Unknown tables in SQL errors suggest close names (`Did you mean
  'Patient'?`), for reads and for `INSERT`/`UPDATE`/`DELETE` targets.

### Catalog

- `propertyValues` takes a `search` (`contains`, `prefix`, `exact` or
  `similar`), answered from the value dictionaries; a date such as
  `1993/11/12` or `2013-06` also matches that date stored in another layout,
  and `domainOnMiss` answers a search with no match by listing a small
  property's values. `query.searchValues` searches every property at once.
- `describe.names` suggests labels and properties spelled like a name;
  `describe.schemaSummary` returns every label with its properties, distinct
  counts, ranges and few values in one read.
- `describe.valueOverlap` reports, for property pairs, how much of one's
  values the other holds, over the exact value dictionaries: the evidence
  for join suggestions.
- Under an enforced data-access policy the catalog reads and SQL `SELECT`
  see only the allowed labels, their visible properties and the allowed
  relationship types, instead of failing with `unauthorized`. A policy's
  `minCount` withholds catalog groups counted over fewer records.
- A policy's `allowedLabels: ["*"]` allows every label, including labels
  created later, so a policy can hide properties or set `minCount` without
  listing labels. An empty `allowedLabels` allows none (lockdown). Writes
  compare allowed labels case-insensitively, as reads do, and a write to a
  label outside the list fails with `unauthorized` instead of an internal
  error.
- The value dictionaries these reads hold count as query working set
  (`query-memory-max`, or the request's `context.queryMemoryMaxBytes`) and
  stop at cancellation and the request deadline. Past the cap,
  `describe.valueOverlap` and `describe.schemaSummary` fail with
  `resourceExhausted`; `query.searchValues` lists the property in `skipped`.
- `propertyValues` without a predicate answers `values` and non-numeric
  `range` from the value dictionaries; before, both read every record of the
  label (2 s for a 3-value column of a 1M-record label). Under one label and
  one stored type, distinct counts, numeric ranges (`describe` and
  `propertyValues`), `topK` and limited `values` no longer hold every distinct
  value: a unique-id column of more than ~3M records failed with
  `resourceExhausted` at the default `query-memory-max`.
- `propertyValues` `count` and `topK` over a scanned property (long texts,
  arrays, filtered queries) hold a digest per distinct value instead of the
  value, so a column of long texts is counted within the working set.
- `describe.schemaSummary` counts a text property whose values are too long
  for the value dictionary, so every scalar property reports `distinctCount`
  and few `values`; before, such a property had neither while
  `propertyValues` reported them. The count comes from write-time refcounts
  of the long values' digests (storage format 6), so it costs no scan at any
  size; listing them reads at most 65,536 records, past which the values are
  left out and the count stays. A data directory written by an earlier build (format 5) does not open
  and must be reloaded.
  Type-split entries name their `siblingTypes`, as `describe.properties` does.
- Catalog introspection (`describe` schema and properties, `schemaSummary`)
  cuts every text sample, listed value and bound to
  `resources.introspection-value-max-chars` characters (default 64; 0 = no
  cap), ending with `…`, so long texts cannot flood a schema read. Value
  queries and records keep values whole.

### Fixes

- Numbers in request JSON are parsed exactly: some floats were stored one
  unit in the last place off (`18.916268423199654` as
  `18.916268423199657`).
- An unfiltered `describe.properties` reports the exact `min`/`max` over
  every record; it reported the range of the first 10,000 records sampled.
- `siblingTypes` lists the types a property holds on the described label;
  it listed the types the name held anywhere in the tenant.
- A property stored as text on one label no longer hides another label's
  numeric range in `describe.schemaSummary` and `propertyValues` `range`.
- `describe.names` ranks the name spelled exactly first among equal
  scores, then a label before a property.
- A number property whose first value is a float filters and describes its
  later integers correctly: `{"x": {"$gte": 1}}` skipped them and the
  catalog read them as their bit patterns (`5e-324`). An integer after a
  float is stored under its own integer definition, as a float after an
  integer already was; inside an array, integers mixed with floats are
  stored as floats.
- Explicit nulls are counted once, as the property's `null` entry, beside
  one entry per value type that counts only values of that type. Nulls
  written before a property's first value were not counted, and later nulls
  were counted under whichever type existed. `distinctCount` no longer
  counts `null` as a value.
- A property stored as two types on one label: each `describe.schemaSummary`
  entry reports its own distinct count, values and range, and
  `propertyValues` `range` covers one ordered class (numbers, else datetimes
  by instant, else text) instead of mixing text with datetimes.
- Signed ISO years (`-0032-01-01T08:00:00Z`) are datetimes, ordered before
  year `0000` and rendered signed; with a time they were stored as text, so a
  property of BC and AD dates split by sign.
- `describe` rejects unknown fields as `invalid_request` (a `schemaSummary`
  sent a `query` ignored it), and `describe.schema` with `detail: "catalog"`
  and a predicate is `invalid_request` instead of an internal error.
- `rushdb serve` without `--languages` enables the documented default set (`sql`,
  `gremlin`, `sql-pgq`, `mongodb`) and the PostgreSQL wire listener. It
  previously enabled no statement language.
- The `rushdb listening` startup line is logged (it was filtered out) and
  includes the listen address.
- A startup error such as the default-token refusal exits with a message
  instead of a panic.
- SQL results keep the statement's column order over PostgreSQL wire and
  HTTP; columns were sorted by name.
- Bolt encodes floats and booleans with the PackStream markers drivers
  expect. The official Neo4j drivers misread every boolean and float before,
  including `has_more`, so a result failed after its first page.
- Bolt answers messages after a failure with `IGNORED` until `RESET`, so a
  driver reports the failure that happened; `RESET` also ends an open
  transaction instead of leaving later statements staged in it.
- A read inside a Bolt transaction reads committed data instead of returning
  no rows, so the drivers' managed read transactions and `execute_query` work.
- `BEGIN`, `COMMIT`, `ROLLBACK` and the other transaction commands fail over
  the PostgreSQL wire with SQLSTATE `0A000` and a message naming autocommit,
  instead of a parse error.

## 2.x

The final Neo4j-backed application architecture is maintained at the `2.x` branch
of `rush-db/rushdb`. See the 3.0 migration guide for supported boundaries.
