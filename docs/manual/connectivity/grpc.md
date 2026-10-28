# gRPC

RushDB exposes the Protocol v1 envelope over gRPC for service meshes and
HTTP/2 infrastructure. The gRPC service is a transport only: every operation
and every enabled query language works exactly as on the
[HTTP API](http-api.md). This page covers enabling the listener, the service
definition, authentication, tenant selection, and status codes.

## Enabling the listener

gRPC is off by default. Give it a port:

```bash
rushdb serve --data-dir ./data --grpc-port 50051
```

It binds the `--listen` address (default `127.0.0.1`) and serves plaintext
HTTP/2 (no TLS). The same [listener security](README.md#listener-security)
rules as the PostgreSQL wire and Bolt listeners apply. The port can also be set
with `server.grpc-port` in `config.yml`; `describe.buildInfo` reports it under
`transports`.

## Service definition

There is one unary method, `rushdb.v1.Database/Execute`. Its request and
response messages have a single `bytes` field (field number 1) that holds the
JSON-encoded Protocol v1 request or response envelope. The server does not
publish a `.proto` file or the gRPC reflection service. This definition is
wire-compatible:

```protobuf
syntax = "proto3";

package rushdb.v1;

// Field 1 carries the canonical JSON envelope (UTF-8 bytes).
message Envelope {
  bytes json = 1;
}

service Database {
  rpc Execute(Envelope) returns (Envelope);
}
```

Any other method path returns `UNIMPLEMENTED`. Streaming RPCs are not
offered.

## Authentication and tenant

| Metadata | Value |
|---|---|
| `authorization` | `Bearer <token>`: the bootstrap token (`--token`). Capability tokens are not accepted. |
| `x-rushdb-tenant` | Tenant scope, on a multitenant server. |

On a multitenant server, set the scope in **both** places: the
`x-rushdb-tenant` metadata and the envelope's `context.scope`. They must match,
otherwise the call fails with `UNAUTHENTICATED` (`scope_mismatch` or
`missing_scope`). On a single-tenant server, send neither: `x-rushdb-tenant`
there fails with `UNAUTHENTICATED` (`scope_mismatch`).

## Example with grpcurl

Save the definition above as `rushdb.proto`, then encode the JSON envelope in
base64 (the protobuf JSON mapping for `bytes`):

```bash
REQ=$(printf '%s' '{"protocolVersion":1,"operation":{"kind":"describe","describe":{"kind":"buildInfo"}}}' | base64 -w0)

grpcurl -plaintext \
  -import-path . -proto rushdb.proto \
  -H "authorization: Bearer $RUSHDB_TOKEN" \
  -d "{\"json\": \"$REQ\"}" \
  127.0.0.1:50051 rushdb.v1.Database/Execute \
  | jq -r .json | base64 -d
```

The decoded output is the same response envelope HTTP returns.

## Example with tonic (Rust)

This mirrors the client used by RushDB's own gRPC tests. It hand-encodes the
single-field message, so it needs no generated code:

```rust
use tonic::{codec::ProstCodec, Request};

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Envelope {
    #[prost(bytes = "vec", tag = "1")]
    pub json: Vec<u8>,
}

let channel = tonic::transport::Endpoint::from_static("http://127.0.0.1:50051")
    .connect()
    .await?;
let mut grpc = tonic::client::Grpc::new(channel);
grpc.ready().await?;

let envelope = serde_json::json!({
    "protocolVersion": 1,
    "operation": {
        "kind": "query",
        "query": { "kind": "statement", "language": "sql", "statement": "SELECT 1 AS x" }
    }
});
let mut request = Request::new(Envelope { json: serde_json::to_vec(&envelope)? });
request
    .metadata_mut()
    .insert("authorization", format!("Bearer {token}").parse()?);

let response = grpc
    .unary(
        request,
        http::uri::PathAndQuery::from_static("/rushdb.v1.Database/Execute"),
        ProstCodec::<Envelope, Envelope>::default(),
    )
    .await?;
let value: serde_json::Value = serde_json::from_slice(&response.into_inner().json)?;
assert_eq!(value["response"]["kind"], "rows");
```

## Status codes

Protocol errors are returned as gRPC statuses whose message is the engine's
error message:

| gRPC status | Engine error codes |
|---|---|
| `UNAUTHENTICATED` | missing or invalid bearer token; `unauthorized`, `missing_scope`, `scope_mismatch` |
| `INVALID_ARGUMENT` | malformed envelope JSON; `invalid_request`, `protocol_violation`, `unsupported_version`, `language_disabled`, `unsupported`, `execution_failed` |
| `NOT_FOUND` | `not_found` |
| `ALREADY_EXISTS` | `conflict` |
| `ABORTED` | `write_conflict` (retry) |
| `CANCELLED` | `cancelled` |
| `RESOURCE_EXHAUSTED` | `resource_exhausted` (retry later) |
| `UNAVAILABLE` | `unavailable` (retry later) |
| `UNIMPLEMENTED` | unknown method path |

## Limits

| Limit | Value |
|---|---|
| Message size (request and response) | the server body limit (default 16 MiB, `server.body-limit`) |
| Concurrent requests per connection | 64 |
| Concurrent HTTP/2 streams per connection | 64 |

Requests also share the node's request scheduler and per-tenant concurrency
cap with the other transports; when they are full, calls fail with
`UNAVAILABLE`.

## See also

- [HTTP API](http-api.md) for the envelope and operations
- [Authentication](authentication.md)
- [Error codes](../reference/error-codes.md)
