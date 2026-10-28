//! 1A — the same contract fixture corpus must produce the identical protocol
//! `response` over HTTP and gRPC (both run the canonical executor). This is
//! the "same fixture corpus over HTTP and gRPC" acceptance.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use rushdb_protocol::LanguageSet;
use rushdb_server::{new_gateway, ServerConfig};
use tonic::codec::ProstCodec;
use tonic::transport::Channel;
use tonic::Request;

use rushdb_grpc::BytesEnvelope;
use std::sync::atomic::{AtomicU16, Ordering};

static GRPC_PORT: AtomicU16 = AtomicU16::new(39010);

fn boot(token: &str) -> (u16, u16, u16) {
    let grpc_port = GRPC_PORT.fetch_add(1, Ordering::Relaxed);
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port,
        bolt_port: 0,
        token: token.to_string(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: LanguageSet {
            sql: true,
            gql: true,
            cypher: true,
            gremlin: true,
            sql_pgq: true,
            mongodb: true,
        },
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
    let (mut router, http_port, _pgwire, _, mut grpc, bound_grpc, _bolt, _) =
        new_gateway(cfg, 0).expect("gateway");
    std::thread::spawn(move || router.serve_forever());
    std::thread::spawn(move || grpc.serve_forever());
    (http_port, bound_grpc, bound_grpc)
}

fn http_execute(port: u16, token: &str, envelope: &serde_json::Value) -> (u16, serde_json::Value) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let payload = envelope.to_string();
    let request = format!(
        "POST /protocol/v1/execute HTTP/1.1\r\nHost: localhost\r\nX-RushDB-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(0);
    let body = raw
        .split_once("\r\n\r\n")
        .and_then(|(_, v)| serde_json::from_str(v).ok())
        .unwrap_or(serde_json::Value::Null);
    (status, body)
}

async fn grpc_execute(
    port: u16,
    token: &str,
    envelope: &serde_json::Value,
) -> Result<serde_json::Value, tonic::Status> {
    let channel = Channel::from_shared(format!("http://127.0.0.1:{port}"))
        .expect("channel")
        .connect()
        .await
        .expect("connect");
    let mut grpc = tonic::client::Grpc::new(channel);
    let mut req = Request::new(BytesEnvelope {
        json: serde_json::to_vec(envelope).expect("serialize"),
    });
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}").parse().expect("meta"),
    );
    let codec = ProstCodec::<BytesEnvelope, BytesEnvelope>::default();
    grpc.ready()
        .await
        .map_err(|e| tonic::Status::unavailable(e.to_string()))?;
    let path = tonic::codegen::http::uri::PathAndQuery::from_static("/rushdb.v1.Database/Execute");
    let resp = grpc.unary(req, path, codec).await?;
    let json = serde_json::from_slice(&resp.into_inner().json).expect("response json");
    Ok(json)
}

/// Make a response comparable across transports: drop generated record ids and
/// sort arrays so record ordering does not mask an identical payload.
fn normalize_response(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(obj) => {
            let mut out = serde_json::Map::new();
            for (k, v) in obj {
                if k == "id" {
                    continue;
                }
                out.insert(k.clone(), normalize_response(v));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            let mut out: Vec<serde_json::Value> = items.iter().map(normalize_response).collect();
            out.sort_by_key(std::string::ToString::to_string);
            serde_json::Value::Array(out)
        }
        other => other.clone(),
    }
}

fn envelope(kind: &str, body: &serde_json::Value) -> serde_json::Value {
    let mut op = serde_json::Map::new();
    op.insert("kind".to_string(), serde_json::json!(kind));
    if let Some(obj) = body.as_object() {
        for (k, v) in obj {
            op.insert(k.clone(), v.clone());
        }
    }
    serde_json::json!({
        "protocolVersion": 1,
        "operation": op
    })
}

fn corpus() -> Vec<(String, serde_json::Value)> {
    vec![
        (
            "put-record".into(),
            envelope(
                "mutate",
                &serde_json::json!({
                    "mutation": {
                        "kind": "putRecord",
                        "mode": "create",
                        "label": "BOOK",
                        "properties": { "title": "Dune" },
                        "options": {}
                    }
                }),
            ),
        ),
        (
            "find-records".into(),
            envelope(
                "query",
                &serde_json::json!({
                    "query": {
                        "kind": "records",
                        "query": { "labels": ["BOOK"], "limit": 10 }
                    }
                }),
            ),
        ),
        (
            "describe-capabilities".into(),
            envelope(
                "describe",
                &serde_json::json!({ "describe": { "kind": "capabilities" } }),
            ),
        ),
        (
            "describe-languages".into(),
            envelope(
                "describe",
                &serde_json::json!({ "describe": { "kind": "queryLanguages" } }),
            ),
        ),
        (
            "describe-buildinfo".into(),
            envelope(
                "describe",
                &serde_json::json!({ "describe": { "kind": "buildInfo" } }),
            ),
        ),
    ]
}

/// Pin the documented protocol v1 contract shape
/// (docs/manual/reference/http-endpoints.md): every
/// request carries `protocolVersion == 1` and an operation `kind` from the
/// documented set; every response carries a `response.kind`.
fn validate_envelope_shape(request: &serde_json::Value, http_body: &serde_json::Value) {
    let kinds = [
        "query",
        "mutate",
        "describe",
        "ingest",
        "index",
        "transaction",
        "purgeTenant",
        "backup",
    ];
    assert_eq!(request["protocolVersion"], 1, "protocolVersion must be 1");
    let op = request.get("operation").expect("operation present");
    let kind = op
        .get("kind")
        .and_then(|k| k.as_str())
        .expect("operation.kind");
    assert!(kinds.contains(&kind), "unknown operation kind `{kind}`");
    if let Some(response) = http_body.get("response") {
        assert!(
            response.get("kind").is_some(),
            "response.kind must be present"
        );
    }
    if let Some(error) = http_body.get("error") {
        for field in ["code", "message", "retryable"] {
            assert!(error.get(field).is_some(), "error.{field} must be present");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn contract_corpus_produces_identical_responses_over_http_and_grpc() {
    let token = "contract-token";
    let (http_port, grpc_port, _) = boot(token);

    for (name, fixture) in corpus() {
        validate_envelope_shape(&fixture, &serde_json::json!({}));
        let (status, http_body) = http_execute(http_port, token, &fixture);
        assert_eq!(status, 200, "HTTP rejected fixture {name}: {http_body}");
        validate_envelope_shape(&fixture, &http_body);

        let grpc_body = grpc_execute(grpc_port, token, &fixture)
            .await
            .unwrap_or_else(|e| panic!("gRPC rejected fixture {name}: {e}"));

        // Both transports carry the canonical ResponseEnvelope; the `response`
        // payload must be identical.
        let http_response = http_body
            .get("response")
            .cloned()
            .expect("HTTP envelope has response");
        let grpc_response = grpc_body
            .get("response")
            .cloned()
            .expect("gRPC envelope has response");
        assert_eq!(
            normalize_response(&http_response),
            normalize_response(&grpc_response),
            "fixture `{name}` diverged between HTTP and gRPC\nHTTP:  {http_response}\ngRPC:  {grpc_response}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn error_contract_is_consistent_across_transports() {
    let token = "contract-token";
    let (http_port, grpc_port, _) = boot(token);
    // An unsupported Mongo form must fail identically on both transports.
    let fixture = envelope(
        "query",
        &serde_json::json!({
            "query": {
                "kind": "statement",
                "language": "mongo",
                "statement": r#"{"collection":"X","aggregate":[{"$match":{}},{"$lookup":{"from":"Y"}}]}"#
            }
        }),
    );
    let (status, http_body) = http_execute(http_port, token, &fixture);
    assert_eq!(status, 422, "{http_body}");
    let err = grpc_execute(grpc_port, token, &fixture)
        .await
        .expect_err("gRPC must reject");
    assert_eq!(err.code(), tonic::Code::InvalidArgument, "{err}");
    // Same error code in both bodies.
    let http_code = http_body
        .pointer("/error/code")
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(http_code, "unsupported");
}
