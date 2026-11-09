# Changelog

This project follows [Semantic Versioning](https://semver.org/). Release notes
describe observable engine and protocol changes; migration instructions belong
in [`docs/manual/operations/upgrading-from-2x.md`](docs/manual/operations/upgrading-from-2x.md).

## Unreleased

### Fixed

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
