# Gremlin

RushDB accepts Apache TinkerPop Gremlin traversals written as text strings.
Traversals read and write the same graph that SQL, Cypher, and the native API
use. This page covers the supported steps, how records map to vertices, and
the parts of TinkerPop that are not exposed. It is for developers with
existing Gremlin traversals or TinkerPop experience.

| Property | Value |
|---|---|
| Status | Stable (text traversal and GraphSON 3 bytecode) |
| Enabled | By default (`languages.gremlin: true`); a `--languages` list must name `gremlin` to keep it |
| Wire id | `gremlin` |
| Profile | Apache TinkerPop Gremlin traversal profile (text and GraphSON 3 bytecode) |
| Implementation | Grafeo graph planner with a gremlin-rs grammar |
| Access | read and write |
| Parameters | named (`$name`) |
| Transports | Protocol v1 (HTTP, gRPC) |

## Data model mapping

| Gremlin concept | RushDB |
|---|---|
| Vertex | A record |
| Vertex label | The record's single primary label |
| Edge | A relationship |
| Edge label | The relationship type |
| Property | A record or relationship property |
| Graph traversal source `g` | The request's tenant |

## Reading

Traversals are sent as strings with `query.statement`:

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "query",
    "query": {
      "kind": "statement",
      "language": "gremlin",
      "statement": "g.V().hasLabel('FACTION').values('name')"
    }
  }
}
```

Tested read steps:

```groovy
g.V().hasLabel('Person').values('name')
g.V().hasLabel('Person').has('age', gt(30)).values('name')
g.V().hasLabel('Person').has('name', 'Alice').out('KNOWS').values('name')
g.V().hasLabel('Person').values('name').order().range(1, 2)
g.V().hasLabel('Person').count()
```

Named parameters are bound by the engine:

```json
{
  "kind": "statement",
  "language": "gremlin",
  "statement": "g.V().hasLabel('Person').has('name', $name).values('age')",
  "parameters": { "name": "Alice" }
}
```

## Writing

Send writes with `mutate.statement`. Each traversal is published as one atomic
commit.

Add a vertex:

```groovy
g.addV('FACTION').property('name', 'Rebel Alliance')
```

Add an edge between existing vertices:

```groovy
g.V().has('name', 'Alice').as('a').V().has('name', 'Bob').addE('LIKES').from('a').property('weight', 3)
```

Update a property and drop elements:

```groovy
g.V().hasLabel('Person').has('name', 'Alice').property('age', 36)
g.V().hasLabel('Person').has('name', 'Carol').drop()
g.E().hasLabel('LIKES').drop()
```

Gremlin writes can also be staged in a
[multi-request transaction](../query/transactions.md) with a `statement`
mutation (`"language": "gremlin"`).

## Bytecode

TinkerPop language variants (gremlin-python, gremlin-javascript, Gremlin.Net,
the Java driver) build a traversal as bytecode. Serialize it as GraphSON 3
and send it with `query.gremlinBytecode` (reads) or `mutate.gremlinBytecode`
(writes); RushDB translates it into the equivalent text traversal and runs
it exactly as one:

```json
{
  "kind": "query",
  "query": {
    "kind": "gremlinBytecode",
    "bytecode": {"@type": "g:Bytecode", "@value": {"step": [
      ["V"], ["hasLabel", "person"],
      ["has", "age", {"@type": "g:P", "@value": {"predicate": "gt", "value": 28}}],
      ["values", "name"]
    ]}}
  }
}
```

The traversals RushDB's profile covers are listed in
`crates/rushdb-language/conformance/gremlin-profile-v1.json`; each runs on the
engine in CI and is checked against the official TinkerPop grammar.
Anonymous traversals are written as bare chained steps
(`where(out('created'))`).

Accepted argument types: JSON scalars, `g:Int32`, `g:Int64`, `g:Float`,
`g:Double`, `g:List`, `g:Set`, `g:Map`, `g:P` and `g:TextP` (including
`and`, `or`, `not`), the `g:T`, `g:Order`, `g:Scope`, `g:Column`,
`g:Direction`, `g:Pop` and `g:Cardinality` enums, and nested `g:Bytecode`
(anonymous traversals). Lambdas, traversal strategies and other source
instructions, and any other type are refused with `unsupported`.

## Limitations

The capability profile advertises these limitations:

| Limitation | Meaning |
|---|---|
| `graphBinaryNotExposed` | GraphBinary serialization and a Gremlin Server endpoint are not provided, so TinkerPop drivers cannot connect directly. Send their traversal bytecode as GraphSON 3 through Protocol v1 (see [Bytecode](#bytecode)), or traversal strings. |
| `singlePrimaryLabelPerRecord` | A vertex has exactly one label. |
| `statementLabelMutationUnavailable` | Traversals cannot change a vertex's label. Use the native `retypeRecord` mutation. |

Also:

- A write holds only the vertices and edges it creates or changes, refused
  above 100,000 records or 64 MiB of them (not the tenant's size).
- Results are materialized; use `range()` to page.
- A write that changes nothing succeeds with `affected: 0` and commits
  nothing.
- Steps other than those shown above may parse but are not covered by the
  test profile. Unsupported syntax fails with `unsupported`; it is never
  silently ignored.

## See also

- [Query languages overview](README.md)
- [openCypher](cypher.md)
- [Relationships and traversal](../query/relationships-and-traversal.md)
- [HTTP API](../connectivity/http-api.md)
