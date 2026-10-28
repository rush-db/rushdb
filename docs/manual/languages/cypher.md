# openCypher

RushDB accepts openCypher statements for reading and writing the property
graph, over Protocol v1 and over the [Bolt protocol](../connectivity/bolt.md).
This page lists what is supported and tested, how records map to Cypher nodes,
and what is out of scope. It is for developers coming from Neo4j or another
Cypher database.

| Property | Value |
|---|---|
| Status | **Release candidate** |
| Enable with | `--languages cypher` (`languages.cypher: true`) |
| Wire id | `cypher` |
| Profile | openCypher property-graph profile |
| Implementation | Grafeo graph planner and executor |
| Access | read and write |
| Parameters | named (`$name`) |
| Transports | Protocol v1 (HTTP, gRPC) and [Bolt](../connectivity/bolt.md) |

> **Note:** Cypher is a release candidate, not full openCypher. The engine
> reports `noFullOpenCypherTckConformance`. The full openCypher TCK runs in CI:
> of 3,897 scenarios, 1,466 test features outside the supported profile (see
> [Conformance](#conformance)), and 1,385 of the remaining 2,431 pass. The
> other 1,046 hit known defects of the embedded Grafeo engine, listed below.
> Do not rely on behavior this page does not list as supported.

## Data model mapping

| Cypher concept | RushDB |
|---|---|
| Node | A record |
| Node label | The record's single primary label |
| Relationship | A relationship |
| Relationship type | The relationship type |
| Properties | Record or relationship properties |
| Database | The request's tenant |

A returned node is the record as a JSON object with its properties plus `__id`
(its dashed UUID) and `__label`. A returned relationship carries its
properties plus `__id`, `__type`, `__source_id`, and `__target_id` (dashed
UUIDs), the same system fields SQL uses. Names starting with `__` are the
engine's, so they never collide with a property; `_id` is an ordinary
property. A
returned path is `{"nodes": [...], "edges": [...]}` with those objects.
Temporal values are ISO 8601 strings (`1984-10-11`, `12:31:14.645Z`,
`1984-10-11T12:31:14Z`, `P1Y2M`). A statement without `RETURN` (an updating
query) returns no rows.

## Supported statements

These cases are part of the tested conformance corpus:

| Feature | Example |
|---|---|
| Expressions | `RETURN 1 + 1 AS n` |
| Match and return | `MATCH (n:FACTION) RETURN n` |
| Inline property filter | `MATCH (n:FACTION {name: 'Rebel'}) RETURN n.name` |
| Relationship patterns | `MATCH (a:FACTION)-[:OWNS]->(b:PLANET) RETURN b` |
| `WHERE` with `STARTS WITH` | `MATCH (n:FACTION) WHERE n.name STARTS WITH 'R' RETURN n` |
| `ORDER BY`, `SKIP`, `LIMIT` | `MATCH (n:FACTION) RETURN n.name AS name ORDER BY name DESC SKIP 1 LIMIT 2` |
| `WITH` | `MATCH (n:FACTION) WITH n.name AS name RETURN name` |
| `UNWIND` | `UNWIND [1,2,3] AS x RETURN x` |
| Aggregation | `MATCH (n:FACTION) RETURN count(n) AS c` |
| `DISTINCT` | `MATCH (n:FACTION) RETURN DISTINCT n.name` |
| `OPTIONAL MATCH` | `MATCH (n:FACTION) OPTIONAL MATCH (n)-[:OWNS]->(p) RETURN n` |
| `UNION` | `MATCH (n:FACTION) RETURN n.name UNION MATCH (p:PLANET) RETURN p.name` |
| List and map literals | `RETURN [1,2,3] AS xs`, `RETURN {a: 1, b: 'x'} AS m` |
| Nested aggregation | `MATCH (n:FACTION) RETURN n.name AS name, size(collect(n)) AS c` |
| `CREATE` | `CREATE (n:FACTION {name: 'Rebel'})` |
| `MERGE` | `MERGE (n:FACTION {name: 'Hutt'})` |
| `SET` | `MATCH (n:FACTION {name: 'Rebel Alliance'}) SET n.name = 'Alliance'` |
| `DELETE`, `DETACH DELETE` | `MATCH (n:Person {name: 'Bob'}) DETACH DELETE n` |
| Index DDL | `CREATE INDEX FOR (n:FACTION) ON (n.name)`, `DROP INDEX FACTION_name` |

Multi-hop patterns and the standard functions `size`, `date`, `datetime`,
`toString`, `toUpper`, `abs`, `round`, and `range` also execute.

## Reading

```cypher
MATCH (n:FACTION) RETURN n.name AS name ORDER BY name
```

```cypher
MATCH (p:Person) WHERE p.age >= $minAge RETURN p.name AS name ORDER BY name
```

Send parameters as a JSON object:

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "query",
    "query": {
      "kind": "statement",
      "language": "cypher",
      "statement": "MATCH (p:Person) WHERE p.age >= $minAge RETURN p.name AS name ORDER BY name",
      "parameters": { "minAge": 30 }
    }
  }
}
```

Integer parameters must fit a signed 64-bit integer.

### Keyword search

`text_match(n.prop, 'query')` is true when a record matches the query in the
text index on that property; `text_score(n.prop, 'query')` is its BM25 score.
``n.`*` `` names the index over every string of the record.

```cypher
MATCH (d:Doc) WHERE text_match(d.title, 'quick fox')
RETURN d.title AS title
ORDER BY text_score(d.title, 'quick fox') DESC LIMIT 10
```

See [Full-text search](../indexes-and-search/full-text-search.md#from-statement-languages).

## Writing

Send writes with `mutate.statement`. The whole statement, however many nodes
and relationships it touches, is published as one commit:

```cypher
CREATE (a:Person {name: 'Alice'})-[r:KNOWS {since: 2025}]->(b:Person {name: 'Bob'})
RETURN a.name AS source, b.name AS target
```

This statement reports `affected: 3` (two records and one relationship) and
returns its `RETURN` rows in the `statementMutation` response.

Connect existing nodes:

```cypher
MATCH (a:Person {name: 'Alice'}), (c:Person {name: 'Carol'})
CREATE (a)-[r:LIKES {weight: 2}]->(c)
RETURN r
```

Update properties and delete:

```cypher
MATCH (n:Person {name: 'Alice'}) SET n.age = 35, n.city = 'London' RETURN n.age AS age
```

```cypher
MATCH (n:Person {name: 'Bob'}) DETACH DELETE n
```

A write that changes nothing (a `MERGE` that matched, a `SET` over no rows)
succeeds with `affected: 0` and commits nothing. Over [Bolt](../connectivity/bolt.md), RushDB decides
whether a statement is a read or a write from its parsed plan, so you can send
both with `RUN`.

### Writes inside a transaction

Cypher writes can be staged in a
[multi-request transaction](../query/transactions.md) with a `statement`
mutation, exactly as shown for [GQL](gql.md#writes-inside-a-transaction). Over
Bolt, `BEGIN`/`COMMIT`/`ROLLBACK` use the same mechanism.

## Index DDL

Neo4j-style index statements are recognized before the query is planned.
`CREATE` and `DROP` must be sent with `mutate.statement`; `SHOW INDEXES` is a
read.

```cypher
CREATE INDEX [name] [IF NOT EXISTS] FOR (n:Label) ON (n.prop)
CREATE FULLTEXT INDEX [name] [IF NOT EXISTS] FOR (n:Label) ON EACH [n.prop]
  [OPTIONS {indexConfig: {consistency: 'immediate'}}]
DROP INDEX name [IF EXISTS]
SHOW INDEXES
```

An unnamed index is called `<Label>_<property>`, so
`CREATE INDEX FOR (n:FACTION) ON (n.name)` creates `FACTION_name`. Each index
covers one label and one property. See
[Indexes](../indexes-and-search/indexes.md) and
[Full-text search](../indexes-and-search/full-text-search.md).

> **Note:** Composite indexes and constraint DDL are not supported.

## Not supported

These are outside the supported profile. Capability discovery reports each
one in `limitations`.

| Feature | Behavior | Limitation |
|---|---|---|
| Nodes without a label or with several (`CREATE ()`, `CREATE (:Person:Employee)`) | Fails with "exactly one label"; nothing is published. | `singlePrimaryLabelPerRecord` |
| Changing labels (`SET n:Label`, `REMOVE n:Label`) | Fails; nothing is published. Use the native `retypeRecord` mutation. Setting the label a node already has, or removing one it lacks, succeeds with `affected: 0`. | `statementLabelMutationUnavailable` |
| `CALL` procedures, including APOC | Fails with a typed error. | `noProcedures` |
| The openCypher temporal type system: `localdatetime`/`localtime`, named time zones, nanosecond precision, week/ordinal/quarter constructors, `truncate`/`between`/`inSeconds` | Fails or returns a different value. `date()`, `time()`, `datetime()` and `duration()` from ISO 8601 text work and return ISO 8601 text. | `limitedTemporalTypes` |
| Constraints (`CREATE CONSTRAINT ...`) | Fails with a typed error. | |
| Standalone `collect` over a literal (`RETURN collect([1,2])`) | Fails; `collect` aggregates matched rows. | |
| Write working set | A write holds only what it creates or changes, refused above 100,000 records or 64 MiB of them (not the tenant's size). | |
| Server-side cursors | Results are materialized; page with `SKIP`/`LIMIT`. | |

## Known defects

RushDB runs the openCypher implementation of the embedded Grafeo engine
unchanged. Grafeo deviates from openCypher in the ways below. Each class is
measured by the TCK on every CI run. Every failure is also rerun on plain
Grafeo, without RushDB's storage or adapter, and fails there too. They are to
be reported upstream; RushDB does not patch Grafeo.

| Defect | TCK scenarios | What you see |
|---|---:|---|
| Invalid statements accepted | 478 | A statement openCypher requires to fail (type errors such as `NOT 'a'`, undefined variables in some positions, invalid argument types) runs and returns a result. |
| Different result | 118 | Wrong rows, order or null handling in some queries, for example an empty variable-length interval (`*2..1`) still expands. |
| Valid statements refused by the binder | 108 | Some variable scopes, `OPTIONAL MATCH` without input rows, `DELETE` of an expression. |
| Quantifiers | 92 | `all`/`any`/`none`/`single` give different results when nested, over lists containing nulls, and over some list literals. |
| Internal errors | 65 | Some statements fail with an internal error, mostly `WITH ... ORDER BY` (50 of 65). |
| Write column names | 65 | An updating statement names an aggregate column differently from its expression text (`count(...)` for `count(*)`). |
| Syntax | 57 | openCypher syntax the parser does not accept. |
| Operator precedence | 52 | Some operator combinations evaluate in a different order. |
| Lazy writes | 10 | `SKIP`/`LIMIT` after an updating clause also limits the updates. |
| Non-termination | 1 | A variable-length match around a cycle does not end; the request deadline cancels it. |

## Conformance

The CI `opencypher-tck` job runs the whole openCypher TCK (2024.2) against a
Cypher-enabled server. Every scenario's outcome is recorded in a ledger, and
each failure is attributed to an exclusion or a defect in the profile
[`opencypher-tck-v2024.2-profile.json`](../../../crates/rushdb-server/conformance/opencypher-tck-v2024.2-profile.json).
The job fails on any change against the ledger, and on a defect that passes
on plain Grafeo (then RushDB caused it). Share of in-profile scenarios that
pass, by area: triadic selection 100%, `CASE` and null handling 92%,
strings 88%, literals 86%, `WITH ... WHERE` 83%, `SKIP`/`LIMIT` 80%,
`MATCH ... WHERE` 77%, `SET` 76%, quantifiers 72%; weakest are `MATCH` 19%
and boolean expressions 17% (both mostly invalid statements accepted),
aggregation 17% and graph functions 6%. Details and per-category counts are in
[Query language conformance](../../QUERY_LANGUAGE_CONFORMANCE.md#opencypher-tck).

## Errors

| Situation | Error code | Bolt code |
|---|---|---|
| Cypher not enabled | `language_disabled` | `Neo.ClientError.Statement.SyntaxError` |
| Syntax not accepted by the parser | `unsupported` | `Neo.ClientError.Statement.SyntaxError` |
| Statement error (undefined variable, type error, a write sent with `query.statement`) | `invalid_request`, with the engine's reason | `Neo.ClientError.Statement.SyntaxError` |
| More than 8,192 clauses in one statement | `invalid_request` | `Neo.ClientError.Statement.SyntaxError` |
| Index DDL sent with `query.statement` | `invalid_request` | `Neo.ClientError.Statement.SyntaxError` |
| Transaction write conflict | `write_conflict` | `Neo.TransientError.Transaction.DeadlockDetected` |
| Cancelled or deadline elapsed (also mid-statement: a running statement's reads fail) | `cancelled` | `Neo.ClientError.Transaction.TransactionTerminated` |

See [Error codes](../reference/error-codes.md) for the full mapping.

## See also

- [Bolt protocol](../connectivity/bolt.md)
- [ISO GQL](gql.md)
- [Query languages overview](README.md)
- [Indexes](../indexes-and-search/indexes.md)
