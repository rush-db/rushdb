# Query language conformance

Status: active implementation contract.

RushDB exposes six statement query frontends (SQL, SQL/PGQ, openCypher, ISO
GQL, Gremlin, and Mongo-style statements) next to the native query document.
How to use each one is in the manual's
[Query languages](manual/languages/README.md) section; this document is the
conformance contract behind their advertised status. A frontend is conformant only
when both its syntax and its execution semantics pass the named conformance
suite. Parsing a statement without being able to execute it does not count as
support.

## Architecture

```text
ISO GQL --------+
openCypher -----+--> Grafeo typed logical plan --> Grafeo executor --> tenant snapshot
Gremlin text ---+                                                    |
SQL/PGQ --------+                         successful graph write --> atomic LMPG batch

SearchQuery --------> RushDB typed query plan ------------------> LMPG executor
Relational SQL -----> Apache DataFusion plan/executor ----------> Arrow snapshot
                                                               successful DML
                                                                    |
                                                        atomic LMPG batch
```

The community language frontends own parsing, name binding, parameter binding,
planning, optimization, execution, and language-specific result shaping.
Graph writes execute against a copy-on-write layer over the tenant's pinned
view (only what the statement creates or changes is held) and are converted
to one version-checked LMPG batch; errors and concurrent snapshot changes
publish nothing. Equality on a property is answered from declared ordered
indexes when every label holding that property has one. DataFusion receives
a bounded Arrow snapshot of the same tenant.
Relational `INSERT`, `UPDATE`, and `DELETE` execute against disposable mutable
DataFusion tables and publish the resulting record delta through that same
optimistic atomic boundary. Stable `__id` values cannot be updated; omitted
insert IDs are generated as UUIDv7 values.

## Profiles and authorities

| Frontend | Syntax authority | Execution authority | Interop surface |
| --- | --- | --- | --- |
| SQL | Apache DataFusion SQL parser/planner, PostgreSQL dialect | RushDB SQL conformance suite plus PostgreSQL-compatible differential fixtures | PostgreSQL wire and typed protocol |
| SQL/PGQ | Grafeo SQL/PGQ parser and shared logical plan | SQL:2023 `GRAPH_TABLE` fixtures | Typed protocol |
| Cypher | Grafeo openCypher parser and shared logical plan | openCypher 9 Technology Compatibility Kit | Typed protocol |
| GQL | Grafeo ISO GQL parser and shared logical plan | Public OpenGQL examples and LDBC workloads | Typed protocol |
| Gremlin | Grafeo's TinkerPop-derived text parser | RushDB traversal-profile fixtures; TinkerPop Process remains the certification target | Typed protocol |
| Mongo-style | RushDB `rushdb-mongodb` translator onto the native query and mutation surface | `rushdb-mongodb` tests and the `query_languages.rs` Mongo suite | Typed protocol |

`GQL` means ISO Graph Query Language, not GraphQL. GraphQL is an application
API concern and remains outside the storage engine.

## Published GQL profile

[`../crates/rushdb-language/conformance/gql-profile-v1.json`](../crates/rushdb-language/conformance/gql-profile-v1.json)
is the machine-readable **RushDB GQL property-graph profile v1 (ISO/IEC 39075
syntax subset)**. It names accepted and excluded feature families, its evidence
suites, and its public corpus: every reference Cypher query of the pinned LDBC
SNB Interactive v1 implementation (29 queries; not an audited benchmark
result). Capability discovery reports the excluded constructs as GQL
limitations.

[`../crates/rushdb-language/conformance/gql-ldbc-snb-v1-mapping.json`](../crates/rushdb-language/conformance/gql-ldbc-snb-v1-mapping.json)
classifies all 29 (checksum-bound to the pinned source):

- **15 accepted and executable** (IS1, IS3–IS7, IC2, IC4, IC8, IC9, IC11,
  IU2, IU3, IU5, IU8): translated to GQL with the translation recorded per
  query (parameter directives removed; the `Message` supertype written as
  `x:Post OR x:Comment` for the one-label model; a chained comparison written
  with `AND`; a `WITH DISTINCT` before a join written as `RETURN DISTINCT`),
  run against the manifest's fixture, and compared with hand-derived rows
  (reads) or checked effects (writes) by `tests/gql_ldbc_profile_mapping.rs`.
- **14 excluded by the profile**, each with its reason: a `MATCH` after a
  `WITH` pipeline (the embedded engine's `NEXT` composition returns wrong
  rows), `ORDER BY` inside `WITH`, `shortestPath` (wrong result), pattern
  predicates, list and pattern comprehensions, creates after `WITH … UNWIND`
  (silently dropped by the engine), `CASE` in a property map, and multi-label
  nodes (`Post:Message`, the `Organisation` supertype).

CI (`gql-ldbc-profile-input`) recomputes the ledger and fails if any pinned
query is unclassified. The excluded constructs are deviations of the embedded
Grafeo engine or the one-label model; each comes back into the profile when
it is fixed.

LMPG records have one primary label. The native `retypeRecord` mutation can
change that label; graph statements do not currently expose label mutation or
multiple-label sets.

## Conformance rules

1. Unknown or unimplemented constructs fail with a typed `unsupported`
   error. They are never ignored or approximated.
2. Every accepted statement is bound to a tenant before planning. No language
   frontend may bypass the server's fail-closed tenant selection.
3. Query cancellation and bounded resource accounting apply after lowering,
   independently of the source language.
4. Missing, explicit null, list values, duplicate rows, path multiplicity,
   ordering, and mutation atomicity retain the source language's semantics.
5. Language and profile versions are explicit protocol data, not inferred
   from user-agent strings.
6. Equivalent read queries have differential tests across SearchQuery, SQL,
   Cypher, GQL, and Gremlin.
7. A profile is documented as complete only when its conformance suite has no
   unexplained skips. Intentional feature exclusions are versioned as a
   smaller profile and reported by capability discovery.

## Delivery gates

- [x] GQL, Cypher, Gremlin text, and SQL/PGQ share one typed logical-plan and
      executor implementation over a tenant-scoped LMPG `GraphStore` adapter.
- [x] GQL, Cypher, and Gremlin writes cross a separate write-authorized
      protocol operation and publish as one version-checked LMPG batch.
- [x] Relational SQL uses Apache DataFusion for parsing, planning,
      optimization, execution, parameters, CTEs, joins, windows, aggregation,
      ordering, and result metadata.
- [x] Relational SQL `INSERT`, `UPDATE`, and `DELETE` target tenant-visible
      record tables through a write-authorized protocol operation and publish
      as one version-checked LMPG batch. Relationship tables remain read-only.
- [x] Pgwire uses the community `pgwire` crate with planner-derived prepared
      metadata and typed positional bind values.
- [x] Capability discovery (`describe.queryLanguages`) reports each
      language's status, access mode, parameter styles, transports, tested
      features, and explicit limitations.
- [ ] Shared typed logical IR is the only frontend-to-executor boundary across
      SearchQuery and relational SQL as well.
- [ ] SearchQuery lowers to the shared IR without response regressions.
- [ ] Cypher passes the openCypher TCK with no unexplained skips. *Every
      scenario runs and every failure is attributed (below): 1,466 test
      features outside the Cypher profile, 1,385 of the other 2,431 pass,
      and 1,046 are known Grafeo defects. Cypher stays release candidate
      while defects remain.*
- [x] GQL's supported ISO profile is advertised explicitly and passes its
      public conformance corpus. *Profile v1 (`gql-profile-v1.json`, its
      exclusions reported by capability discovery) classifies all 29 pinned
      LDBC SNB Interactive queries: 15 run with verified results, 14 use
      excluded constructs, 0 unclassified.*
- [x] Gremlin accepts canonical traversal bytecode; text parsing is a
      convenience adapter validated by the official TinkerPop grammar.
      *GraphSON 3 bytecode is accepted (`query.gremlinBytecode` /
      `mutate.gremlinBytecode`) and translated to canonical text. The
      profile [`gremlin-profile-v1.json`](../crates/rushdb-language/conformance/gremlin-profile-v1.json)
      (44 traversals: reads, writes, bytecode with pinned translations) runs
      on the engine (`tests/gremlin_profile.rs`) and is checked against the
      official grammar (TinkerPop 3.7.3 `Gremlin.g4`, checksum-pinned) by CI
      `gremlin-grammar` (`scripts/validate-gremlin-grammar.sh`): 0 rejected.*
- [x] All text language operations are available through the transport-neutral
      protocol; pgwire is SQL-specific and supports atomic single-statement
      DML, while explicit SQL transactions remain unavailable.
- [x] Cross-language read, tenancy, mutation, Pgwire, and protocol fixtures
      pass locally.
- [ ] Official TCK/corpus suites and cancellation inside every community
      operator pass in CI. *The full openCypher TCK runs in CI against its
      ledger. Graph-language statements are cancelled inside Grafeo's
      operators: a running statement's reads fail once the request is
      cancelled or its deadline passes (`on_language_stack`). The GQL LDBC
      corpus is fully classified in CI.*

### openCypher TCK

The CI `opencypher-tck` job runs **every scenario** of the openCypher TCK
(tag `2024.2`, commit `59edf2e1c17b845bf97c334ed06b2eb780950c13`; 3,897
scenarios with Scenario Outlines expanded) against a Cypher-enabled
Protocol v1 server, with `scripts/run-opencypher-tck.py`. Each scenario runs
on a purged tenant: its graph and setup statements, its query (writes through
`mutate.statement`), then its result table, expected error and side effects
are compared with the upstream expectations. Comparison rules: an expected
error passes when the statement is refused with any error (the TCK's error
type is not compared); integers and floats are distinct; nodes compare by
labels and properties, relationships by type and properties; side effects
are set differences of nodes, relationships, label names and
`(element, key, value)` properties.

Every outcome is recorded in
[`../crates/rushdb-server/conformance/opencypher-tck-v2024.2-ledger.json`](../crates/rushdb-server/conformance/opencypher-tck-v2024.2-ledger.json),
and every failure is attributed to a category of the profile
[`../crates/rushdb-server/conformance/opencypher-tck-v2024.2-profile.json`](../crates/rushdb-server/conformance/opencypher-tck-v2024.2-profile.json).
A category is either an **exclusion**, a feature outside the supported
Cypher profile that capability discovery reports in `limitations`
(`query_languages.rs` checks every one is advertised), or a **defect**, a
wrong result of the embedded Grafeo engine. RushDB does not patch Grafeo:
defects are documented and tracked for upstream reports.

Attribution is checked, not asserted. CI also builds plain Grafeo as a
reference (`crates/rushdb-language/examples/grafeo_reference.rs`: Grafeo's
own in-memory store, no LMPG, no RushDB adapter, the same row conventions)
and reruns every defect failure on it. A scenario that passes there is
RushDB's fault: it becomes `rushdb-adapter`, which no profile rule explains,
and fails the job. The one exception is a rule marked `confirmed` with its
reason: three empty variable-length interval scenarios (`*2..1`, `*..0`)
pass on plain Grafeo only in the TCK's two-`MATCH` form through a different
join plan, while a single `MATCH` returns the same wrong rows there.

The job fails when any scenario's outcome differs from the ledger (a
regression, or an improvement not yet recorded with `--write-ledger`), when a
failure matches no profile category, or when plain Grafeo passes a defect.
Current result (2026-10-12): **1,385 of 3,897 scenarios pass**. 1,466
scenarios test excluded features; of the 2,431 in the profile, 1,385 pass
(57%) and 1,046 fail on known defects:

| Category | Kind | Scenarios | Cause |
| --- | --- | ---: | --- |
| `temporal` | exclusion (`limitedTemporalTypes`) | 991 | The openCypher temporal type system beyond ISO `date`/`time`/`datetime`/`duration` |
| `lmpg-single-label` | exclusion (`singlePrimaryLabelPerRecord`, `statementLabelMutationUnavailable`) | 425 | LMPG records carry exactly one label: 388 create label-less nodes, 22 multi-label nodes, 15 change labels |
| `procedures` | exclusion (`noProcedures`) | 50 | `CALL` procedures |
| `grafeo-error-detection` | defect | 478 | Grafeo accepts statements the TCK requires to fail |
| `grafeo-result` | defect | 118 | Grafeo returns a different result |
| `grafeo-semantic` | defect | 108 | Grafeo's binder refuses a valid statement |
| `grafeo-quantifiers` | defect | 92 | `all`/`any`/`none`/`single` results |
| `grafeo-internal` | defect | 65 | Grafeo internal errors (50 in `WITH ... ORDER BY`) |
| `grafeo-write-column-names` | defect | 65 | Write statements name aggregate columns differently |
| `grafeo-syntax` | defect | 57 | Syntax Grafeo's parser does not accept |
| `grafeo-precedence` | defect | 52 | Operator precedence |
| `grafeo-lazy-writes` | defect | 10 | `LIMIT` after an updating clause limits the updates |
| `grafeo-nontermination` | defect | 1 | A variable-length match around a cycle never ends; the deadline cancels it |

Running the full TCK found and fixed eight adapter defects on the RushDB side:
a long statement (the TCK's 500-clause movie graph) overflowed the worker
stack and aborted the process; a never-ending statement could not be
cancelled; graph-language statement errors were all reported as internal
errors; temporal values were serialized as internal structures instead of
ISO 8601 text; paths returned internal ordinals; writes without `RETURN`
returned internal ids; setting a property to null stored an explicit null
instead of removing it; and nodes inside lists and maps without a declared
list type (`tail(nodes(p))`) returned Grafeo's internal maps instead of
records. The last was found by the plain-Grafeo attribution, which then
reported no further RushDB-caused failure.

The curated in-repo corpus
(`crates/rushdb-server/tests/cypher_conformance.rs`) classifies 22 statements —
core reads/writes plus `WITH`, `UNWIND`, aggregation, `DISTINCT`, `OPTIONAL
MATCH`, `UNION`, `MERGE`, list/map literals, `STARTS WITH`,
`size(collect(...))`, and a standalone-`collect` gap — as supported or
explicitly unsupported with a reason, and fails if any classification diverges.
It is a regression guard, not the TCK, and does not change Cypher's
release-candidate status. Multi-hop patterns and the standard functions
(`size`, `date`, `datetime`, `toString`, `toUpper`, `abs`, `round`, `range`)
were probed and execute; the standalone `collect` gap and the index/constraint
DDL scope remain open (see [openCypher](manual/languages/cypher.md)).

## Non-goals

- A private replacement for PostgreSQL wire, GraphBinary, or official grammar
  tooling.
- Treating SearchQuery's JSON syntax as the universal internal AST.
- Claiming compatibility based on syntax-only smoke tests.
