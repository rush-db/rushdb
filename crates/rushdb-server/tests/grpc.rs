//! gRPC transport integration tests: real HTTP/2 wire round-trips through the
//! `Database.Execute` unary RPC, plus auth, tenant scope, error mapping, and
//! language gating.

use std::sync::Arc;

use rushdb_protocol::LanguageSet;
use rushdb_server::{Node, NodeEnvelopeHandler, ServerConfig};
use tonic::codec::ProstCodec;
use tonic::{transport::Channel, Request};

use rushdb_grpc::{BytesEnvelope, GrpcServer};

fn boot(token: &str, languages: LanguageSet) -> u16 {
    let mut cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: token.to_string(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages,
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        maintenance: false,
        allow_insecure_listen: false,
        require_index_warmup: false,
        request_timeout_ms: 0,
        max_concurrent_requests_per_tenant: 0,
        explicit_flags: std::collections::BTreeSet::new(),
    };
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    let handler: Arc<dyn rushdb_grpc::EnvelopeHandler> = Arc::new(NodeEnvelopeHandler::new(node));
    let (mut server, port) =
        GrpcServer::bind(handler, token.to_string(), "127.0.0.1", 0, 64 * 1024 * 1024)
            .expect("grpc bind");
    std::thread::spawn(move || server.serve_forever());
    port
}

async fn connect(port: u16, _token: &str) -> tonic::client::Grpc<Channel> {
    let channel = Channel::from_shared(format!("http://127.0.0.1:{port}"))
        .expect("channel uri")
        .connect()
        .await
        .expect("connect");
    tonic::client::Grpc::new(channel)
}

async fn execute(
    grpc: &mut tonic::client::Grpc<Channel>,
    token: &str,
    envelope: serde_json::Value,
) -> Result<BytesEnvelope, tonic::Status> {
    let mut req = Request::new(BytesEnvelope {
        json: serde_json::to_vec(&envelope).expect("serialize envelope"),
    });
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}").parse().expect("token metadata"),
    );
    let codec = ProstCodec::<BytesEnvelope, BytesEnvelope>::default();
    grpc.ready()
        .await
        .map_err(|e| tonic::Status::unavailable(e.to_string()))?;
    let path = tonic::codegen::http::uri::PathAndQuery::from_static("/rushdb.v1.Database/Execute");
    let resp = grpc.unary(req, path, codec).await?;
    Ok(resp.into_inner())
}

fn put_envelope(label: &str, id: &str) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "operation": {
            "kind": "mutate",
            "mutation": {
                "kind": "putRecord",
                "mode": "create",
                "label": label,
                "properties": { "name": id },
                "options": {}
            }
        }
    })
}

fn query_envelope(label: &str) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "operation": {
            "kind": "query",
            "query": {
                "kind": "records",
                "query": {
                    "labels": [label],
                    "predicate": {},
                    "offset": 0,
                    "limit": 100
                }
            }
        }
    })
}

fn ranked_put_envelope(id: &str, rank: u64) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "operation": {
            "kind": "mutate",
            "mutation": {
                "kind": "putRecord", "mode": "create", "label": "CURSOR_ITEM",
                "properties": { "name": id, "rank": rank }, "options": {}
            }
        }
    })
}

#[tokio::test]
async fn grpc_round_trip_insert_and_query() {
    let port = boot("grpc-token", LanguageSet::default());
    let mut grpc = connect(port, "grpc-token").await;
    let resp = execute(&mut grpc, "grpc-token", put_envelope("Artist", "a1"))
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&resp.json).unwrap();
    assert_eq!(value["response"]["kind"], "record");
    assert_eq!(value["response"]["record"]["properties"]["name"], "a1");

    let resp = execute(&mut grpc, "grpc-token", query_envelope("Artist"))
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&resp.json).unwrap();
    assert_eq!(value["response"]["kind"], "records");
    let records = value["response"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["properties"]["name"], "a1");
}

#[tokio::test]
async fn grpc_carries_native_snapshot_cursor_envelopes() {
    let port = boot("grpc-token", LanguageSet::default());
    let mut grpc = connect(port, "grpc-token").await;
    for (id, rank) in [("c1", 1), ("c2", 2), ("c3", 3)] {
        execute(&mut grpc, "grpc-token", ranked_put_envelope(id, rank))
            .await
            .unwrap();
    }
    let first = execute(
        &mut grpc,
        "grpc-token",
        serde_json::json!({
            "protocolVersion": 1,
            "operation": { "kind": "query", "query": { "kind": "records", "query": {
                "labels": ["CURSOR_ITEM"], "predicate": {}, "order": {"rank":"asc"},
                "limit": 2, "cursorTtlMs": 1000
            }}}
        }),
    )
    .await
    .unwrap();
    let first: serde_json::Value = serde_json::from_slice(&first.json).unwrap();
    assert_eq!(first["response"]["records"].as_array().unwrap().len(), 2);
    let cursor = first["response"]["cursorId"].as_str().unwrap().to_string();

    let next = execute(&mut grpc, "grpc-token", serde_json::json!({
        "protocolVersion": 1,
        "operation": { "kind": "query", "query": { "kind": "cursor", "cursorId": cursor, "limit": 2 }}
    })).await.unwrap();
    let next: serde_json::Value = serde_json::from_slice(&next.json).unwrap();
    assert_eq!(next["response"]["total"], 3);
    assert_eq!(next["response"]["records"].as_array().unwrap().len(), 1);
    assert!(next["response"]["cursorId"].is_null());
}

#[tokio::test]
async fn grpc_requires_valid_bearer_token() {
    let port = boot("grpc-token", LanguageSet::default());
    let mut grpc = connect(port, "wrong").await;
    let err = execute(&mut grpc, "wrong", query_envelope("Artist"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn grpc_invalid_envelope_is_invalid_argument() {
    let port = boot("grpc-token", LanguageSet::default());
    let mut grpc = connect(port, "grpc-token").await;
    let mut req = Request::new(BytesEnvelope {
        json: b"not json".to_vec(),
    });
    req.metadata_mut()
        .insert("authorization", "Bearer grpc-token".parse().unwrap());
    let codec = ProstCodec::<BytesEnvelope, BytesEnvelope>::default();
    grpc.ready().await.unwrap();
    let path = tonic::codegen::http::uri::PathAndQuery::from_static("/rushdb.v1.Database/Execute");
    let err = grpc.unary(req, path, codec).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn grpc_disabled_language_is_invalid_argument() {
    // No statement languages enabled: a SQL statement must be refused with
    // LanguageDisabled (mapped to InvalidArgument).
    let port = boot("grpc-token", LanguageSet::NONE);
    let mut grpc = connect(port, "grpc-token").await;
    let envelope = serde_json::json!({
        "protocolVersion": 1,
        "operation": {
            "kind": "query",
            "query": { "kind": "statement", "language": "sql", "statement": "SELECT 1" }
        }
    });
    let err = execute(&mut grpc, "grpc-token", envelope)
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn grpc_language_gated_statement_works_when_enabled() {
    let lang = LanguageSet {
        sql: true,
        ..Default::default()
    };
    let port = boot("grpc-token", lang);
    let mut grpc = connect(port, "grpc-token").await;
    let envelope = serde_json::json!({
        "protocolVersion": 1,
        "operation": {
            "kind": "query",
            "query": { "kind": "statement", "language": "sql", "statement": "SELECT 1 AS x" }
        }
    });
    let resp = execute(&mut grpc, "grpc-token", envelope).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&resp.json).unwrap();
    assert_eq!(value["response"]["kind"], "rows");
    assert_eq!(value["response"]["rows"][0]["x"], 1);
}

#[tokio::test]
async fn grpc_unknown_path_is_not_found() {
    let port = boot("grpc-token", LanguageSet::default());
    let mut grpc = connect(port, "grpc-token").await;
    let mut req = Request::new(BytesEnvelope { json: b"".to_vec() });
    req.metadata_mut()
        .insert("authorization", "Bearer grpc-token".parse().unwrap());
    let codec = ProstCodec::<BytesEnvelope, BytesEnvelope>::default();
    grpc.ready().await.unwrap();
    let path = tonic::codegen::http::uri::PathAndQuery::from_static("/rushdb.v1.Database/Unknown");
    let err = grpc.unary(req, path, codec).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn grpc_tenant_metadata_is_refused_on_a_single_tenant_server() {
    let port = boot("grpc-token", LanguageSet::default());
    let mut grpc = connect(port, "grpc-token").await;
    let mut req = Request::new(BytesEnvelope {
        json: serde_json::to_vec(&query_envelope("Artist")).unwrap(),
    });
    req.metadata_mut()
        .insert("authorization", "Bearer grpc-token".parse().unwrap());
    req.metadata_mut()
        .insert("x-rushdb-tenant", "acme".parse().unwrap());
    let codec = ProstCodec::<BytesEnvelope, BytesEnvelope>::default();
    grpc.ready().await.unwrap();
    let path = tonic::codegen::http::uri::PathAndQuery::from_static("/rushdb.v1.Database/Execute");
    let err = grpc.unary(req, path, codec).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated, "{err:?}");
    assert!(err.message().contains("single-tenant"), "{err:?}");
}
