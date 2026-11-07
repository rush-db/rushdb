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
    boot_with(token, languages, false, None, None)
}

/// A self-signed certificate and its key as PEM files (alive as long as the
/// returned directory handle), and the certificate's PEM for the client's
/// trust store.
fn self_signed_pem() -> (
    (std::path::PathBuf, std::path::PathBuf),
    Vec<u8>,
    rushdb_storage::TempPath,
) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let dir = rushdb_storage::TempPath::dir("grpc-tls");
    std::fs::create_dir_all(&dir).unwrap();
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    let cert_pem = cert.cert.pem();
    std::fs::write(&cert_path, &cert_pem).unwrap();
    std::fs::write(&key_path, cert.key_pair.serialize_pem()).unwrap();
    ((cert_path, key_path), cert_pem.into_bytes(), dir)
}

/// A TLS gateway: `(port, certificate PEM, the PEM files' directory)`.
fn boot_tls(
    token: &str,
    multitenant: bool,
    capability_secret: Option<&[u8]>,
) -> (u16, Vec<u8>, rushdb_storage::TempPath) {
    let (tls, cert_pem, dir) = self_signed_pem();
    let port = boot_with(
        token,
        LanguageSet::default(),
        multitenant,
        capability_secret,
        Some(tls),
    );
    (port, cert_pem, dir)
}

fn boot_with(
    token: &str,
    languages: LanguageSet,
    multitenant: bool,
    capability_secret: Option<&[u8]>,
    tls: Option<(std::path::PathBuf, std::path::PathBuf)>,
) -> u16 {
    let (tls_cert, tls_key) = tls.map_or((None, None), |(cert, key)| (Some(cert), Some(key)));
    let mut cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: token.to_string(),
        data_dir: None,
        multitenant,
        capability_secret: capability_secret.map(<[u8]>::to_vec),
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages,
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert,
        tls_key,
        maintenance: false,
        allow_insecure_listen: false,
        require_index_warmup: false,
        request_timeout_ms: 0,
        max_concurrent_requests_per_tenant: 0,
        explicit_flags: std::collections::BTreeSet::new(),
    };
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    let handler: Arc<dyn rushdb_grpc::EnvelopeHandler> =
        Arc::new(NodeEnvelopeHandler::new(node, &cfg));
    let tls = match (&cfg.tls_cert, &cfg.tls_key) {
        (Some(cert), Some(key)) => Some(rushdb_grpc::GrpcTls {
            cert_pem: std::fs::read(cert).unwrap(),
            key_pem: std::fs::read(key).unwrap(),
        }),
        _ => None,
    };
    let (mut server, port) =
        GrpcServer::bind(handler, "127.0.0.1", 0, 64 * 1024 * 1024, tls).expect("grpc bind");
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

/// A TLS channel trusting `cert_pem` for `localhost`.
async fn connect_tls(port: u16, cert_pem: &[u8]) -> tonic::client::Grpc<Channel> {
    // The test binary links two rustls providers; tonic's client builder
    // needs the process default to be one of them.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let tls = tonic::transport::ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(cert_pem))
        .domain_name("localhost");
    let channel = Channel::from_shared(format!("https://localhost:{port}"))
        .expect("channel uri")
        .tls_config(tls)
        .expect("tls config")
        .connect()
        .await
        .expect("connect over TLS");
    tonic::client::Grpc::new(channel)
}

async fn execute(
    grpc: &mut tonic::client::Grpc<Channel>,
    token: &str,
    envelope: serde_json::Value,
) -> Result<BytesEnvelope, tonic::Status> {
    execute_scoped(grpc, token, None, envelope).await
}

/// One call with `x-rushdb-tenant` and the envelope's `context.scope` set
/// to `tenant` when given.
async fn execute_scoped(
    grpc: &mut tonic::client::Grpc<Channel>,
    token: &str,
    tenant: Option<&str>,
    mut envelope: serde_json::Value,
) -> Result<BytesEnvelope, tonic::Status> {
    if let Some(tenant) = tenant {
        envelope["context"]["scope"] = serde_json::json!(tenant);
    }
    let mut req = Request::new(BytesEnvelope {
        json: serde_json::to_vec(&envelope).expect("serialize envelope"),
    });
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}").parse().expect("token metadata"),
    );
    if let Some(tenant) = tenant {
        req.metadata_mut()
            .insert("x-rushdb-tenant", tenant.parse().expect("tenant metadata"));
    }
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

#[tokio::test]
async fn grpc_tls_channel_works_and_plaintext_is_refused() {
    let (port, cert_pem, _dir) = boot_tls("grpc-token", false, None);
    let mut grpc = connect_tls(port, &cert_pem).await;
    let resp = execute(&mut grpc, "grpc-token", query_envelope("Artist"))
        .await
        .expect("query over TLS");
    let value: serde_json::Value = serde_json::from_slice(&resp.json).unwrap();
    assert_eq!(value["response"]["kind"], "records", "{value}");
    // A plaintext channel to a TLS listener never completes a call.
    let channel = Channel::from_shared(format!("http://127.0.0.1:{port}"))
        .unwrap()
        .connect_lazy();
    let mut plain = tonic::client::Grpc::new(channel);
    assert!(execute(&mut plain, "grpc-token", query_envelope("Artist"))
        .await
        .is_err());
}

#[tokio::test]
async fn grpc_capabilities_are_tenant_bound_bearer_tokens() {
    let secret = b"grpc-capability-secret-0123456789";
    let (port, cert_pem, _dir) = boot_tls("grpc-token", true, Some(secret));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let issue = |scope: &str, tenant: &str| {
        rushdb_server::capability::issue_for_tenant(secret, "svc", scope, Some(tenant), 60, now)
    };
    let mut grpc = connect_tls(port, &cert_pem).await;
    // The boot token seeds both tenants.
    for tenant in ["tenant-a", "tenant-b"] {
        execute_scoped(
            &mut grpc,
            "grpc-token",
            Some(tenant),
            put_envelope("Artist", &format!("{tenant}-1")),
        )
        .await
        .unwrap();
    }
    // A read capability reads its own tenant ...
    let reader = issue("read", "tenant-a");
    let resp = execute_scoped(
        &mut grpc,
        &reader,
        Some("tenant-a"),
        query_envelope("Artist"),
    )
    .await
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&resp.json).unwrap();
    assert_eq!(value["response"]["total"], 1, "{value}");
    // ... not another, and nothing it was not granted.
    let err = execute_scoped(
        &mut grpc,
        &reader,
        Some("tenant-b"),
        query_envelope("Artist"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated, "{err}");
    let err = execute_scoped(
        &mut grpc,
        &reader,
        Some("tenant-a"),
        put_envelope("Artist", "tenant-a-2"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), tonic::Code::PermissionDenied, "{err}");
    // Expired and tampered tokens are refused.
    let expired = rushdb_server::capability::issue_for_tenant(
        secret,
        "svc",
        "read",
        Some("tenant-a"),
        1,
        now - 100,
    );
    let err = execute_scoped(
        &mut grpc,
        &expired,
        Some("tenant-a"),
        query_envelope("Artist"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated, "{err}");
    let err = execute_scoped(
        &mut grpc,
        &format!("{reader}x"),
        Some("tenant-a"),
        query_envelope("Artist"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated, "{err}");
    // A write capability writes.
    let writer = issue("read write", "tenant-a");
    execute_scoped(
        &mut grpc,
        &writer,
        Some("tenant-a"),
        put_envelope("Artist", "tenant-a-2"),
    )
    .await
    .unwrap();
    let resp = execute_scoped(
        &mut grpc,
        &writer,
        Some("tenant-a"),
        query_envelope("Artist"),
    )
    .await
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&resp.json).unwrap();
    assert_eq!(value["response"]["total"], 2, "{value}");
}
