# SQL/PGQ

SQL/PGQ is the property-graph query extension of SQL:2023. RushDB supports its
`GRAPH_TABLE` operator, which runs a graph pattern match and returns the
matches as a relational table that SQL can filter and sort. This page covers
the read-only profile RushDB implements. Use it when you want graph pattern
matching but prefer SQL syntax around it.

| Property | Value |
|---|---|
| Status | Stable (read-only) |
| Enabled | By default (`languages.sql-pgq: true`); a `--languages` list must name `sql-pgq` to keep it |
| Wire id | `sqlPgq` |
| Profile | SQL:2023 `GRAPH_TABLE` read profile |
| Implementation | Grafeo graph planner and executor |
| Access | **read-only** |
| Parameters | named (`$name`) |
| Transports | Protocol v1 (HTTP, gRPC) |

> **Note:** SQL/PGQ is separate from [relational SQL](sql.md). Relational SQL
> sees labels as tables; SQL/PGQ sees the whole tenant as one property graph.
> SQL/PGQ statements are not available over the PostgreSQL wire protocol.

## Data model mapping

The tenant is the graph. Node patterns match records by label; edge patterns
match relationships by type. Inside `COLUMNS (...)` you project node and edge
properties as output columns.

## Supported features

| Feature | Example |
|---|---|
| `GRAPH_TABLE` with `MATCH` | `GRAPH_TABLE (MATCH (n:FACTION) COLUMNS (n.name AS name))` |
| Edge patterns | `MATCH (p:Person)-[:KNOWS]->(f:Person)` |
| Projection with `COLUMNS` | `COLUMNS (p.name AS source, f.name AS target)` |
| Filtering on the result | `... AS g WHERE age >= 29` |
| Ordering on the result | `... AS g ORDER BY name DESC` |

## Examples

All nodes of a label:

```sql
SELECT * FROM GRAPH_TABLE (
  MATCH (n:FACTION)
  COLUMNS (n.name AS name)
) AS g
ORDER BY name
```

A relationship pattern:

```sql
SELECT * FROM GRAPH_TABLE (
  MATCH (p:Person)-[:KNOWS]->(f:Person)
  COLUMNS (p.name AS source, f.name AS target)
) AS g
```

Filter and sort the projected columns:

```sql
SELECT name FROM GRAPH_TABLE (
  MATCH (p:Person)
  COLUMNS (p.name AS name, p.age AS age)
) AS g
WHERE age >= 29
ORDER BY name DESC
```

Send it as a read:

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "query",
    "query": {
      "kind": "statement",
      "language": "sqlPgq",
      "statement": "SELECT * FROM GRAPH_TABLE (MATCH (n:FACTION) COLUMNS (n.name AS name)) AS g ORDER BY name"
    }
  }
}
```

## Limitations

- **Read-only.** The profile advertises `access: readOnly` and the limitation
  `readOnly`. Sending a SQL/PGQ statement with `mutate.statement` fails with
  `invalid_request` ("SQL/PGQ is a read-only profile"), and SQL/PGQ statements
  cannot be staged in transactions. Write with [Cypher](cypher.md),
  [GQL](gql.md), [Gremlin](gremlin.md), or [SQL DML](sql.md#writing-data).
- Only the features listed above are part of the profile. Joining a
  `GRAPH_TABLE` result with relational SQL tables is not supported; use
  [relational SQL](sql.md) joins over `rushdb_edges` tables for that.
- Results are materialized; there is no server-side cursor.

## See also

- [Query languages overview](README.md)
- [SQL](sql.md)
- [ISO GQL](gql.md)
