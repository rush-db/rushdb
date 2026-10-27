# Native client examples

Minimal clients for each transport, runnable against a `rushdb serve` node; no
other service is needed. Start the engine with every transport and language:

```bash
cargo run -p rushdb-server --bin rushdb -- serve \
  --port 3000 --pgwire-port 5432 --grpc-port 50051 --bolt-port 7687 \
  --token local-token --languages sql,gql,cypher,gremlin,sql-pgq,mongodb
```

All transports authenticate with the same boot token
(`--token` / `RUSHDB_TOKEN`).

## HTTP — native protocol

```bash
curl -s -X POST http://127.0.0.1:3000/protocol/v1/execute \
  -H 'X-RushDB-Token: local-token' \
  -H 'Content-Type: application/json' \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "describe",
      "describe": { "kind": "buildInfo" }
    }
  }'
```

## gRPC — `Database.Execute` unary

The `rushdb-grpc` crate exposes `BytesEnvelope` (raw envelope JSON). A tonic
client calls `/rushdb.v1.Database/Execute` with an `authorization: Bearer
<token>` metadata header and the envelope as the message body. See
`crates/rushdb-server/tests/grpc.rs` for a full round-trip client.

## pgwire — PostgreSQL wire

```bash
PGPASSWORD=local-token psql -h 127.0.0.1 -p 5432 -U rushdb -c \
  'SELECT name FROM FACTION LIMIT 3'
```

Or from Rust with `tokio_postgres`:

```rust
let (client, connection) = tokio_postgres::connect(
    "host=127.0.0.1 port=5432 user=rushdb password=local-token dbname=rushdb",
    tokio_postgres::NoTls,
).await?;
tokio::spawn(async move { connection.await.unwrap() });
let rows = client.query("SELECT name FROM FACTION", &[]).await?;
```

pgwire also runs single-statement `INSERT`/`UPDATE`/`DELETE`, each committed
atomically; there are no explicit transactions (`BEGIN`/`COMMIT`). See
[PostgreSQL wire](../docs/manual/connectivity/postgresql-wire.md).

## Bolt — Neo4j wire

A minimal Bolt 5 client sends the handshake, HELLO (token auth), then
RUN/PULL for read/write Cypher. See `crates/rushdb-server/tests/bolt.rs` for a
hand-rolled client that exercises read + write + transactions. Values are
plain JSON maps (no GraphBinary).

## Negotiation

Every client should start from `Describe::BuildInfo` (version, transport
bindings, limits) + `Describe::queryLanguages` (enabled profiles with
`status`/`features`/`limitations`) instead of guessing from a version string.