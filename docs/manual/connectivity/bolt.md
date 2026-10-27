# Bolt

RushDB implements the server side of Bolt, the Neo4j binary protocol, so that
Bolt clients can run [Cypher](../languages/cypher.md) against RushDB. This page
covers enabling the listener, the handshake and messages RushDB supports,
transactions, how values are encoded, and the limits of the implementation. It
is for developers bringing Neo4j-style applications or tools to RushDB.

| Property | Value |
|---|---|
| Status | Stable (Cypher over Bolt) |
| Protocol version | Bolt 5.0 |
| Language | openCypher only |
| Authentication | `basic` scheme; the credentials are the bootstrap token |
| Encryption | none (plaintext) |

> **Note:** No maintained Rust Neo4j driver negotiates Bolt 5, so the release
> suite verifies the wire protocol with a protocol-conformant Bolt 5 client
> (handshake, `HELLO` authentication, `RUN`/`PULL` reads and writes,
> `BEGIN`/`COMMIT`/`ROLLBACK`, and PackStream byte values). The official
> Python (`neo4j` 6.4) and JavaScript (`neo4j-driver` 6.2) drivers were checked
> by hand against the examples on this page; they are not part of the
> automated suite.

## Enabling the listener

The Bolt listener needs a port and a graph language:

```bash
rushdb serve --data-dir ./data --languages cypher --bolt-port 7687
```

It binds only when `--bolt-port` is non-zero **and** `cypher` is enabled:
statements over Bolt are always Cypher, so enabling only `gql` does not start
the listener. The listener binds the `--listen` address and follows
the [listener security](README.md#listener-security) rules.

On a multitenant server, the `db` field of `HELLO` selects the tenant for the
whole connection. On a single-tenant server, leave `db` unset (or `neo4j`):
any other value makes every statement fail with a scope error instead of
opening a second keyspace.

## Connection flow

1. **Handshake.** The client sends the Bolt magic preamble and up to four
   version proposals. RushDB accepts the first proposal whose major version
   is 5 and answers with Bolt 5.0. If no proposal is version 5, the
   connection is closed.
2. **`HELLO`.** Authenticate with `scheme: "basic"` and `credentials` set to
   the bootstrap token (`principal` is ignored). `scheme: "none"` is accepted
   only when the server token is empty. A wrong token returns
   `Neo.ClientError.Security.Unauthorized`. Any message other than `HELLO` or
   `GOODBYE` before successful authentication is refused. The server
   identifies itself as `RushDB/1.0`.
3. **`RUN` and `PULL`.** `RUN` takes the Cypher text and an optional
   parameter map. RushDB decides from the statement's parsed plan whether it
   reads or writes, and routes it to the read or the write operation; you do
   not need separate read and write sessions. `RUN` executes the whole
   statement and buffers its rows; `PULL {n}` streams them in windows (`n: -1`
   means all). `has_more` tells you whether rows remain.
4. **`GOODBYE`** closes the connection.

Supported messages:

| Message | Behavior |
|---|---|
| `HELLO` | Authentication. |
| `RUN` | Execute a Cypher statement (auto-commit, or staged inside `BEGIN`). |
| `PULL` | Stream buffered rows; honors `n`. |
| `DISCARD` | Drop buffered rows; honors `n`. |
| `BEGIN`, `COMMIT`, `ROLLBACK` | Explicit transactions. |
| `RESET` | Clear the pending result and recover from a failure. |
| `ROUTE` | Returns a single-server routing table pointing at this listener. |
| `GOODBYE` | Close the connection. |

Bolt 5.1+ messages (`LOGON`, `LOGOFF`, `TELEMETRY`) are not implemented;
clients must authenticate with `HELLO` as in Bolt 5.0.

## Transactions

`BEGIN` opens a RushDB multi-request transaction, the same mechanism as the
protocol's `transaction` operations. Each write `RUN` inside it stages the
statement's changes; `COMMIT` publishes them atomically and `ROLLBACK`
discards them.

```text
C: BEGIN {}
S: SUCCESS {}
C: RUN "CREATE (p:Person {name: 'commit'})" {} {}
S: SUCCESS {"fields": [], "type": "w"}
C: COMMIT
S: SUCCESS {}
```

Rules inside an explicit transaction:

- A read statement (for example a plain `MATCH ... RETURN`) runs as an
  ordinary query against committed data, as queries do alongside a
  transaction on every transport (only
  [`getRecord`](../query/transactions.md#3-read-your-own-writes) reads through
  one). It does not see the transaction's own staged writes.
- A write statement inside a transaction returns no rows.
- `RESET` ends the open transaction and discards its staged changes.
- The transaction is subject to the server's transaction limits
  (`--tx-default-ttl-seconds`, `--tx-max-operations`, `--tx-max-staged-bytes`)
  and may abort with `Neo.TransientError.Transaction.DeadlockDetected` when a
  concurrent commit conflicts. Retry the whole transaction.

See [Transactions](../query/transactions.md).

## Values

Parameters and results use the Bolt PackStream encoding, limited to JSON-like
values:

| Bolt type | Support |
|---|---|
| Null, Boolean, Integer (64-bit), Float, String | yes |
| List, Map | yes (up to 1,000,000 items per container, nesting depth 64) |
| Bytes | no: a message containing bytes is rejected |
| Node, Relationship, Path structures | not sent: nodes and relationships are returned as **maps** |
| Temporal and spatial structures | no |

A returned node is a map of its properties plus `__id` (its dashed UUID) and
`__label`; a relationship is a map of its properties plus `__id`, `__type`,
`__source_id`, and `__target_id` (dashed UUIDs). Drivers that expect `Node` and `Relationship` objects see plain maps
instead. Return properties (`RETURN n.name AS name`) when you need typed
columns.

## Using a Neo4j driver

Connect with `bolt://` (not `neo4j://`: there is no cluster routing), and pass
the server token as the password; the user name is ignored.

Python (`pip install neo4j`):

```python
from neo4j import GraphDatabase

driver = GraphDatabase.driver("bolt://127.0.0.1:7687", auth=("rushdb", "local-token"))
with driver.session() as session:
    # Auto-commit queries: reads and writes both work.
    session.run("CREATE (a:Artist {name: $name})", name="ada").consume()
    for record in session.run("MATCH (a:Artist) RETURN a.name AS name"):
        print(record["name"])

    # A managed write transaction commits its writes atomically.
    session.execute_write(lambda tx: tx.run("CREATE (:Artist {name: 'grace'})").consume())

    # Managed read transactions and execute_query read committed data.
    count = session.execute_read(lambda tx: tx.run("MATCH (a:Artist) RETURN count(a) AS n").single()["n"])
driver.close()
```

JavaScript (`npm install neo4j-driver`):

```javascript
import neo4j from 'neo4j-driver'

const driver = neo4j.driver('bolt://127.0.0.1:7687', neo4j.auth.basic('rushdb', 'local-token'))
const session = driver.session()
try {
  await session.run('CREATE (a:Artist {name: $name})', { name: 'ada' })
  const result = await session.run('MATCH (a:Artist) RETURN a.name AS name')
  console.log(result.records.map((record) => record.get('name')))
} finally {
  await session.close()
  await driver.close()
}
```

What to expect from a driver:

- **Reads see committed data.** Auto-commit `session.run`, the managed read
  transactions (`execute_read`, `executeRead`) and `execute_query` all work.
  A read inside `execute_write`/`executeWrite` does not see that
  transaction's own writes until it commits.
- **Nodes and relationships are maps.** `RETURN n` gives a map with `__id`,
  `__label` and the properties, not a driver `Node`.
- **Integers are 64-bit.** The JavaScript driver returns them as its
  `Integer` type; call `.toNumber()` or create the driver with
  `{ disableLosslessIntegers: true }`.

## Error codes

| Bolt failure code | Engine error codes |
|---|---|
| `Neo.ClientError.Security.Unauthorized` | `unauthorized`, `missing_scope`, `scope_mismatch`, failed `HELLO` |
| `Neo.ClientError.Statement.SyntaxError` | `invalid_request`, `protocol_violation`, `language_disabled`, `unsupported` |
| `Neo.ClientError.Statement.EntityNotFound` | `not_found` |
| `Neo.ClientError.Constraint.UniqueConstraintViolation` | `conflict` |
| `Neo.TransientError.Transaction.DeadlockDetected` | `write_conflict` |
| `Neo.ClientError.Transaction.TransactionTerminated` | `cancelled` |
| `Neo.TransientError.General.DatabaseUnavailable` | `resource_exhausted`, `unavailable`, or a result over 100,000 rows |
| `Neo.ClientError.Request.Invalid` | `unsupported_version`, malformed `PULL`/`DISCARD` metadata |
| `Neo.DatabaseError.Execution.Failed` | `execution_failed` |
| `Neo.ClientError.Transaction.TransactionNotFound` | `COMMIT` or `ROLLBACK` without `BEGIN` |

After a failure, send `RESET` before continuing.

## Limits

| Limit | Value |
|---|---|
| Message size | 64 MiB |
| Rows buffered per `RUN` | 100,000; larger results fail. Page with `SKIP`/`LIMIT`. |
| Concurrent connections | 1,024 |
| Time to authenticate after connecting | 10 seconds |
| Idle read/write timeout | 60 seconds |

## Not supported

- TLS (`bolt+s://`, `neo4j+s://`) and cluster routing beyond a single-server
  table.
- Languages other than Cypher over Bolt.
- Vector, full-text, and native operations; use the [HTTP API](http-api.md).
- Multitenant servers (see the warning above).

## See also

- [openCypher](../languages/cypher.md)
- [Transactions](../query/transactions.md)
- [Authentication](authentication.md)
- [Error codes](../reference/error-codes.md)
