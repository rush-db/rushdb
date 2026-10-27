# Changelog

This project follows [Semantic Versioning](https://semver.org/). Release notes
describe observable engine and protocol changes; migration instructions belong
in [`docs/manual/operations/upgrading-from-2x.md`](docs/manual/operations/upgrading-from-2x.md).

## 3.0.0 — Unreleased

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
