# PostgreSQL wire protocol

RushDB speaks the PostgreSQL frontend/backend protocol (version 3) so that
PostgreSQL clients can run [SQL](../languages/sql.md) against it. This page
covers enabling the listener, connecting with `psql` and drivers, selecting a
tenant, how errors map to SQLSTATE codes, and what the listener deliberately
does not support. It is for anyone connecting SQL tools or PostgreSQL drivers.

> **Note:** The compatibility suite exercises the `tokio-postgres` 0.7 client
> (simple queries, extended queries, prepared statements with typed
> parameters, DML, and tenant isolation). `psql`, node-postgres (`pg` 8) and
> psycopg 3 were checked by hand against the examples on this page. Other
> drivers, ORMs, and GUI tools are not certified; anything that depends on
> PostgreSQL system catalogs will not work.

## Enabling the listener

The listener starts whenever the `sql` language is enabled. It binds the
`--listen` address on `--pgwire-port` (default `5432`):

```bash
rushdb serve --data-dir ./data --languages sql --token "$RUSHDB_TOKEN"
```

To change the port, pass `--pgwire-port` or set `server.pgwire-port` in
`config.yml`. With `sql` disabled, nothing listens.

The listener is cleartext. It refuses SSL and GSS encryption requests during
startup, so clients must allow a non-TLS connection (`sslmode=disable` or the
default `prefer`). Keep it on loopback; see
[Listener security](README.md#listener-security).

## Connecting

| Startup parameter | Meaning |
|---|---|
| `user` | Accepted for compatibility; any value. `current_user` reports `rushdb`. |
| password | The server's bootstrap token (`--token`). Sent as a cleartext password message. |
| `database` | Tenant scope on a multitenant server (required, non-empty). Ignored on a single-tenant server. |

### psql

```bash
PGPASSWORD="$RUSHDB_TOKEN" psql \
  "host=127.0.0.1 port=5432 user=rushdb dbname=rushdb sslmode=disable"
```

```text
rushdb=> SELECT name FROM NOTE WHERE position > 5;
 name
-------
 alpha
(1 row)
```

Meta-commands that read the PostgreSQL catalogs (`\d`, `\dt`, `\l`) do not
work, because RushDB has no `pg_catalog`. `information_schema.tables` and
`information_schema.columns` list the tables and their columns.

### node-postgres (JavaScript)

```javascript
import pg from 'pg'

const client = new pg.Client({
  host: '127.0.0.1',
  port: 5432,
  user: 'rushdb',
  database: 'rushdb',            // the tenant scope on a multitenant server
  password: process.env.RUSHDB_TOKEN
})
await client.connect()
const { rows } = await client.query(
  'SELECT title, estimate FROM task WHERE status = $1 ORDER BY estimate DESC',
  ['pending']
)
await client.query('INSERT INTO task (title, status, estimate) VALUES ($1, $2, $3)', ['Write docs', 'pending', 1])
await client.end()
```

node-postgres returns `INT8` values as strings, as it does for PostgreSQL;
register a parser (`pg.types.setTypeParser(20, BigInt)`) to change that.

### psycopg 3 (Python)

```python
import os
import psycopg

with psycopg.connect(
    host="127.0.0.1", port=5432, user="rushdb", dbname="rushdb",
    password=os.environ["RUSHDB_TOKEN"],
    autocommit=True,  # required: every statement commits on its own
) as conn:
    rows = conn.execute(
        "SELECT title, estimate FROM task WHERE status = %s ORDER BY estimate DESC", ("pending",)
    ).fetchall()
    conn.execute("UPDATE task SET status = %s WHERE title = %s", ("done", "Write docs"))
```

Without `autocommit=True`, psycopg opens every statement with `BEGIN`, which
fails with SQLSTATE `0A000`.

### tokio-postgres (Rust)

This is the tested client:

```rust
use tokio_postgres::{Config, NoTls};

let mut config = Config::new();
config
    .host("127.0.0.1")
    .port(5432)
    .user("rushdb")
    .dbname("rushdb")          // the tenant scope on a multitenant server
    .password(std::env::var("RUSHDB_TOKEN")?);
let (client, connection) = config.connect(NoTls).await?;
tokio::spawn(async move {
    let _ = connection.await;
});

// Extended protocol: typed parameters, planner-derived metadata
let rows = client
    .query("SELECT name FROM NOTE WHERE position > $1 AND active = $2", &[&6_i64, &true])
    .await?;
let name: String = rows[0].get("name");

// Prepared statement: parameter and column types come from the SQL planner
let statement = client
    .prepare("SELECT position, active FROM NOTE WHERE position > $1")
    .await?;
assert_eq!(statement.params(), &[tokio_postgres::types::Type::INT8]);

// DML returns the affected row count
let inserted = client
    .execute("INSERT INTO NOTE (name, position) VALUES ($1, $2)", &[&"through-pgwire", &7_i64])
    .await?;
assert_eq!(inserted, 1);
```

Integer properties are `INT8` (`BIGINT`): bind `i64` values for them.

## Selecting a tenant

On a multitenant server, the `database` startup parameter selects the tenant.
It is required; a connection without it fails with SQLSTATE `3D000`. Each
connection sees exactly one tenant:

```rust
let tenant_a = connect(port, "tenant-a", &token).await?;   // dbname = tenant-a
let tenant_b = connect(port, "tenant-b", &token).await?;   // dbname = tenant-b
// SELECT name FROM NOTE returns only that tenant's records on each connection
```

> **Warning:** The listener authenticates only the bootstrap token, which is
> not bound to a tenant: whoever holds it can open any tenant by changing
> `database`. Capability tokens are not accepted as passwords. On a
> multitenant server the listener therefore refuses to bind a non-loopback
> address; expose it only through a trusted proxy that authenticates callers
> and fixes the `database` value.

## Supported protocol features

- Protocol 3 startup.
- Cleartext password authentication against the bootstrap token.
- Startup responses: `AuthenticationOk`, `ParameterStatus` (`server_version`
  is `16.0-rushdb`), `BackendKeyData`, `ReadyForQuery`.
- Simple query flow.
- Extended query flow (Parse, Bind, Describe, Execute, Close, Flush, Sync),
  with parameter and result types derived from the SQL plan.
- Text and binary encodings for `bool`, `int2`/`int4`/`int8`,
  `float4`/`float8`, `text`, `bytea`, `date`, `time`, `timestamp`, and
  `json`/`jsonb`.
- `CommandComplete` tags with affected-row counts for `INSERT`, `UPDATE`, and
  `DELETE`, and `CREATE INDEX` / `DROP INDEX` tags for index DDL.
- Compatibility answers for `SELECT 1`, `SELECT version()`,
  `SHOW server_version`, `SELECT current_database()`, and
  `SELECT current_user`.

## Statements and transactions

Each message carries **one** SQL statement:

- Statements starting with `INSERT`, `UPDATE`, or `DELETE` run on the atomic
  SQL write path and commit on their own.
- Index DDL (`CREATE INDEX`, `CREATE VECTOR INDEX`, `DROP INDEX`) runs as a
  mutation, like `mutate.statement` over HTTP.
- Everything else runs as a read.

There are no explicit transactions: every DML statement is its own atomic
commit. `BEGIN`, `START TRANSACTION`, `COMMIT`, `END`, `ROLLBACK`, `ABORT`,
`SAVEPOINT` and `RELEASE` fail with SQLSTATE `0A000`, so run drivers in
autocommit mode. Multi-statement strings are not supported.

See [SQL](../languages/sql.md) for the table mapping, supported SQL, and row
limits.

## Session settings

A session can bound its own statements with four settings, named as in
PostgreSQL and CockroachDB. Each one can only make the server's bound
stricter ([request bounds](../architecture/resource-governance.md#request-bounds)):

| Setting | Meaning | Values |
|---|---|---|
| `statement_timeout` | Cancel a statement after this long (`57014`). | `'5s'`, `'500ms'`, `'2min'`; a bare number is milliseconds; `0` = the server's timeout only |
| `work_mem` | Working memory one statement may use (`53200` beyond it). It caps the whole statement, not one sort as in PostgreSQL. | `'64MB'`, `'1GB'`; a bare number is kilobytes |
| `disallow_full_table_scans` | Refuse a filter without a usable index (`42601`, naming the index to declare). | `on` / `off` |
| `large_full_scan_rows` | With `disallow_full_table_scans` on, still allow such a filter on a label of at most this many records. | whole number, default `0` |

```sql
SET statement_timeout = '30s';
SET disallow_full_table_scans = on;
SET large_full_scan_rows = 100000;
SHOW statement_timeout;   -- 30000ms
RESET ALL;
```

`SET name = DEFAULT` and `RESET name` clear one setting. The settings can
also be passed as startup parameters (for example `statement_timeout` in
the connection options); a malformed value fails each statement with
`22023`. Other `SET` statements are not supported.

## SQLSTATE mapping

Engine errors are reported as PostgreSQL `ErrorResponse` messages with these
SQLSTATE codes:

| SQLSTATE | Meaning | Engine error code |
|---|---|---|
| `28P01` | Wrong password (token) | (authentication) |
| `3D000` | Missing `database` on a multitenant server; scope missing or mismatched | `missing_scope`, `scope_mismatch` |
| `42601` | Statement could not be parsed or described during Parse | `invalid_request` |
| `42501` | Not authorized | `unauthorized` |
| `42P01` | Table or object not found | `not_found` |
| `23505` | Constraint or state conflict | `conflict` |
| `40001` | Serialization failure; retry | `write_conflict` |
| `57014` | Query cancelled or deadline elapsed | `cancelled` |
| `53200` | Resource budget exhausted | `resource_exhausted` |
| `22023` | Invalid value for a [session setting](#session-settings) | (session) |
| `57P03` | Server unavailable (capacity, maintenance, shutting down) | `unavailable` |
| `0A000` | Feature not supported (including transaction control) or language disabled | `unsupported`, `language_disabled`, `unsupported_version` |
| `08P01` | Protocol violation | `protocol_violation` |
| `22P02` | A result value cannot be encoded as its column type | (encoding) |
| `XX000` | Internal or unclassified execution failure | `execution_failed` |

## Limits

| Limit | Value |
|---|---|
| Concurrent sessions | 1,024; further connections are closed |
| Statement deadline | `--request-timeout-ms` (none by default), or a shorter `statement_timeout`; a timed-out statement fails with `57014` |
| Rows per result | 100,000 (SQL frontend limit) |

## Not supported

- TLS, SCRAM-SHA-256, channel binding, and certificate authentication.
- Explicit transactions, savepoints, cursors, and bounded portal fetches.
- `COPY`, `LISTEN`/`NOTIFY`, and replication connections.
- PostgreSQL system catalogs, type casts specific to PostgreSQL, and
  extensions.
- Capability tokens as passwords.
- Graph languages (use [Bolt](bolt.md) for Cypher, or the
  [HTTP API](http-api.md) for everything else).
- Vector and full-text search.

Unsupported requests fail with a SQLSTATE error; they are not emulated.

## See also

- [SQL](../languages/sql.md)
- [Authentication](authentication.md)
- [Tenants](../data-model/tenants.md)
- [Error codes](../reference/error-codes.md)
