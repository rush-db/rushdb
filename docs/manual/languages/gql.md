# ISO GQL

RushDB accepts ISO/IEC 39075 Graph Query Language (GQL) statements for reading
and writing the property graph. This page describes the profile RushDB
implements, how records map to GQL nodes, the statements you can use, and the
boundaries of the profile. It is for developers who want a standards-based
graph language rather than a vendor dialect.

| Property | Value |
|---|---|
| Status | **Release candidate** |
| Enable with | `--languages gql` (`languages.gql: true`) |
| Wire id | `gql` |
| Profile | RushDB GQL property-graph profile v1 (ISO/IEC 39075 syntax subset) |
| Implementation | Grafeo graph planner and executor |
| Access | read and write |
| Parameters | named (`$name`) |
| Transports | Protocol v1 (HTTP, gRPC) |

> **Note:** GQL is a release candidate. Reads and writes are implemented and
> tested against RushDB's GQL profile v1, a subset of ISO GQL. The server
> advertises its exclusions as limitations: `noFullIsoGqlConformance`,
> `noMatchAfterWithPipeline`, `noNextComposition`, `noOrderByInWith`,
> `noShortestPath`, `noPatternPredicates`, `noListOrPatternComprehensions`
> and `noLabelAlternation`. Rely on the statement forms on this page, not on
> the whole standard.

## Data model mapping

| GQL concept | RushDB |
|---|---|
| Node | A record |
| Node label | The record's single primary label |
| Node property | A record property |
| Edge | A relationship |
| Edge label | The relationship type |
| Edge property | A relationship property |
| Graph | The request's tenant |

When a statement returns a whole node, the value is the record as a JSON object
with its properties plus `__id` (the record's dashed UUID) and `__label`. A
returned edge carries its properties plus `__id`, `__type`, `__source_id`,
and `__target_id` (dashed UUIDs).

## Profile v1 features

The published profile (`rushdb-gql-property-graph-v1`) accepts these feature
families:

| Feature | Example |
|---|---|
| Pattern matching (`match`) | `MATCH (p:Person)-[:KNOWS]->(f:Person)` |
| Filtering (`filter`) | `WHERE p.age >= $minAge` |
| Projection | `RETURN p.name AS name` |
| Parameters | `$minAge` |
| `OPTIONAL MATCH` | `OPTIONAL MATCH (p)-[:KNOWS]->(f:Person)` |
| Aggregation | `RETURN count(p) AS total` |
| `UNWIND` | `UNWIND [3, 1, 2] AS value` |
| Ordering and pagination | `ORDER BY name SKIP 1 LIMIT 1` |
| Create nodes | `INSERT (:Person {name: 'Eve'})` |
| Update properties | `SET n.age = 36` |
| Delete nodes | `DELETE n` |
| Create and delete edges | `INSERT (a)-[:LIKES]->(b)`, `DELETE r` |
| Atomic statement commit | each write statement is one commit |

Explicitly excluded: multiple labels per record, label mutation in statements,
and full ISO GQL conformance.

## Reading

```gql
MATCH (n:FACTION) RETURN n.name AS name ORDER BY name
```

Traverse relationships:

```gql
MATCH (p:Person)-[:KNOWS]->(f:Person)
RETURN p.name AS source, f.name AS target
```

Optional matches keep rows with no partner, returning `null`:

```gql
MATCH (p:Person)
OPTIONAL MATCH (p)-[:KNOWS]->(f:Person)
RETURN p.name AS source, f.name AS target
ORDER BY source
```

```json
[
  { "source": "Alice", "target": "Bob" },
  { "source": "Bob", "target": null },
  { "source": "Carol", "target": null }
]
```

Aggregation, `UNWIND`, and pagination:

```gql
MATCH (p:Person) RETURN count(p) AS total
```

```gql
UNWIND [3, 1, 2] AS value RETURN value ORDER BY value
```

```gql
MATCH (p:Person) RETURN p.name AS name ORDER BY name SKIP 1 LIMIT 1
```

### Parameters

Pass named parameters as a JSON object. Integers must fit a signed 64-bit
integer; larger unsigned values are rejected rather than rounded.

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "query",
    "query": {
      "kind": "statement",
      "language": "gql",
      "statement": "MATCH (p:Person) WHERE p.age >= $minAge RETURN p.name AS name ORDER BY name",
      "parameters": { "minAge": 30 }
    }
  }
}
```

### Keyword search

`text_match(n.prop, 'query')` is true when a record matches the query in the
text index on that property; `text_score(n.prop, 'query')` is its BM25 score.
``n.`*` `` names the index over every string of the record.

```gql
MATCH (d:Doc) WHERE text_match(d.title, 'quick fox')
RETURN d.title AS title
ORDER BY text_score(d.title, 'quick fox') DESC LIMIT 10
```

See [Full-text search](../indexes-and-search/full-text-search.md#from-statement-languages).

## Writing

Send writes with `mutate.statement`. Each statement runs against a private
copy of the tenant graph, and its complete change set is published as one
commit. If the statement fails, nothing is published.

```gql
INSERT (:FACTION {name: 'Rebel Alliance'})
```

Create two nodes and the edge between them in one statement:

```gql
INSERT (:Person {name: 'Alice'})-[:LIKES {weight: 2}]->(:Person {name: 'Bob'})
```

Update and delete:

```gql
MATCH (n:Person) WHERE n.name = 'Alice' SET n.age = 36
```

```gql
MATCH (n:Person) WHERE n.name = 'Carol' DELETE n
```

```gql
MATCH ()-[r:LIKES]->() DELETE r
```

A write that changes nothing (for example, a `MATCH ... SET` with no matches)
succeeds with `affected: 0` and commits nothing.

### Writes inside a transaction

GQL write statements can be staged in a
[multi-request transaction](../query/transactions.md) with a `statement`
mutation. The statement sees the transaction's earlier staged writes, and its
changes commit or roll back with the transaction:

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "transaction",
    "transaction": {
      "kind": "stage",
      "transactionId": "<id from begin>",
      "mutation": {
        "kind": "statement",
        "language": "gql",
        "statement": "INSERT (:Person {name: 'Eve'})"
      }
    }
  }
}
```

A transaction that stages a graph statement conflicts with any other commit
to the tenant that lands before it commits. Expect `write_conflict` under
concurrent writes and retry.

## Index DDL

GQL accepts the same index statements as Cypher. Send `CREATE` and `DROP` with
`mutate.statement`; `SHOW INDEXES` is a read:

```gql
CREATE INDEX person_name IF NOT EXISTS FOR (n:Person) ON (n.name)
CREATE FULLTEXT INDEX note_body FOR (n:Note) ON EACH [n.body]
  OPTIONS {indexConfig: {consistency: 'immediate'}}
DROP INDEX person_name IF EXISTS
SHOW INDEXES
```

When the name is omitted, the index is named `<Label>_<property>`. See
[Indexes](../indexes-and-search/indexes.md).

## Limitations

| Limitation | Behavior |
|---|---|
| One label per node | Inserting a node with several labels fails; nothing is published. |
| No label mutation | Statements cannot add or remove labels. Use the native `retypeRecord` mutation. |
| Write working set | A write holds only the records and relationships it creates or changes; it is refused when they exceed 100,000 records or 64 MiB, whatever the tenant's size. |
| No server-side cursor | Results are materialized; page with `SKIP`/`LIMIT`. |
| No native transport | GQL is served over Protocol v1 (HTTP, gRPC) only. Bolt carries Cypher. |
| Partial standard | Only the profile above is supported; other ISO GQL constructs fail. |

## Errors

| Situation | Error code |
|---|---|
| GQL not enabled | `language_disabled` |
| Syntax the parser does not accept | `unsupported` |
| Statement error (undefined variable, type error, a write sent through `query.statement`) | `invalid_request`, with the engine's reason |
| More than 8,192 clauses in one statement | `invalid_request` |
| Index DDL sent through `query.statement` | `invalid_request` |
| A write in an unsupported form (several labels, label mutation) | `unsupported` |
| Deadline elapsed or request cancelled (also mid-statement) | `cancelled` |

## See also

- [openCypher](cypher.md), which shares the same planner and data mapping
- [Query languages overview](README.md)
- [Transactions](../query/transactions.md)
- [Relationships](../data-model/relationships.md)
