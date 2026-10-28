//! HTTP protocol adapter end-to-end tests (raw envelope, scoping, and shaping).
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use rushdb_server::{new_router, ServerConfig};

fn call(
    port: u16,
    path: &str,
    body: &serde_json::Value,
    scope: Option<&str>,
) -> (u16, serde_json::Value) {
    call_with_token(port, path, body, scope, "test-token")
}

fn call_with_token(
    port: u16,
    path: &str,
    body: &serde_json::Value,
    scope: Option<&str>,
    token: &str,
) -> (u16, serde_json::Value) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let payload = body.to_string();
    let scope_header = scope
        .map(|value| format!("X-RushDB-Tenant: {value}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nX-RushDB-Token: {token}\r\nContent-Type: application/json\r\n{scope_header}Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let response = raw
        .split_once("\r\n\r\n")
        .and_then(|(_, value)| serde_json::from_str(value).ok())
        .unwrap_or(serde_json::Value::Null);
    (status, response)
}

#[test]
fn http_capability_cannot_select_a_different_tenant() {
    const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "boot-token".into(),
        data_dir: None,
        multitenant: true,
        capability_secret: Some(SECRET.to_vec()),
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet {
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));
    let token = rushdb_server::capability::issue_for_tenant(
        SECRET,
        "service-a",
        "write",
        Some("tenant-a"),
        60,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );

    let request = put_request("tenant-a", "NOTE", &serde_json::json!({ "text": "safe" }));
    let (accepted, _) = call_with_token(
        port,
        "/protocol/v1/execute",
        &request,
        Some("tenant-a"),
        &token,
    );
    assert_eq!(accepted, 200);

    let cross_tenant = put_request("tenant-b", "NOTE", &serde_json::json!({ "text": "leak" }));
    let (rejected, response) = call_with_token(
        port,
        "/protocol/v1/execute",
        &cross_tenant,
        Some("tenant-b"),
        &token,
    );
    assert_eq!(rejected, 401, "{response}");
}

#[test]
fn tenant_capability_cannot_export_another_tenant() {
    const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "boot-token".into(),
        data_dir: None,
        multitenant: true,
        capability_secret: Some(SECRET.to_vec()),
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet::default(),
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));

    for (scope, text) in [("tenant-a", "a-secret"), ("tenant-b", "b-secret")] {
        let (status, response) = call_with_token(
            port,
            "/protocol/v1/execute",
            &put_request(scope, "NOTE", &serde_json::json!({ "text": text })),
            Some(scope),
            "boot-token",
        );
        assert_eq!(status, 200, "{response}");
    }

    let token = rushdb_server::capability::issue_for_tenant(
        SECRET,
        "service-a",
        "read",
        Some("tenant-a"),
        60,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    let cross_tenant_backup = serde_json::json!({
        "protocolVersion": 1,
        "context": { "scope": "tenant-b" },
        "operation": { "kind": "backup", "request": { "scopes": ["tenant-b"] } }
    });
    let (status, response) = call_with_token(
        port,
        "/protocol/v1/execute",
        &cross_tenant_backup,
        Some("tenant-b"),
        &token,
    );
    assert_eq!(status, 401, "{response}");

    let own_backup = serde_json::json!({
        "protocolVersion": 1,
        "context": { "scope": "tenant-a" },
        "operation": { "kind": "backup", "request": { "scopes": [] } }
    });
    let (status, response) = call_with_token(
        port,
        "/protocol/v1/execute",
        &own_backup,
        Some("tenant-a"),
        &token,
    );
    assert_eq!(status, 200, "{response}");
    let backup = response["response"]["tenants"][0]["jsonl"]
        .as_str()
        .expect("backup JSONL");
    assert!(backup.contains("a-secret"));
    assert!(!backup.contains("b-secret"));
}

fn join_request(scope: &str) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "requestId": "http-join-1",
        "context": { "scope": scope },
        "operation": {
            "kind": "query",
            "query": {
                "kind": "joinPairs",
                "left": { "label": "SOURCE", "filter": {} },
                "right": { "label": "TARGET", "filter": {} },
                "predicate": {
                    "kind": "propertyOverlap",
                    "leftProperty": "targetId",
                    "rightProperty": "id"
                },
                "excludeSameRecord": false,
                "projection": "count",
                "limit": 100
            }
        }
    })
}

fn put_request(scope: &str, label: &str, properties: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "context": { "scope": scope },
        "operation": {
            "kind": "mutate",
            "mutation": {
                "kind": "putRecord",
                "mode": "create",
                "label": label,
                "properties": properties,
                "options": {}
            }
        }
    })
}

fn records_request(scope: &str, label: &str) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "context": { "scope": scope },
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

fn ingest_request_with_options(
    documents: &serde_json::Value,
    options: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "context": {},
        "operation": {
            "kind": "ingest",
            "ingest": {
                "kind": "documents",
                "documents": documents,
                "options": options
            }
        }
    })
}

fn ingest_request(documents: &serde_json::Value) -> serde_json::Value {
    ingest_request_with_options(documents, &serde_json::json!({ "returnResult": true }))
}

fn labels_request() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "context": {},
        "operation": {
            "kind": "describe",
            "describe": {
                "kind": "labels",
                "query": {
                    "labels": [],
                    "predicate": {},
                    "offset": 0
                }
            }
        }
    })
}

fn statement_mutation(language: &str, statement: &str) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": 1,
        "context": {},
        "operation": {
            "kind": "mutate",
            "mutation": {
                "kind": "statement",
                "language": language,
                "statement": statement
            }
        }
    })
}

#[test]
fn http_carries_native_snapshot_cursor_envelopes() {
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "test-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet::default(),
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));
    for (id, rank) in [("h1", 1), ("h2", 2), ("h3", 3)] {
        let request = serde_json::json!({
            "protocolVersion": 1, "context": {}, "operation": { "kind": "mutate", "mutation": {
                "kind": "putRecord", "mode": "create", "label": "CURSOR_ITEM",
                "properties": {"name": id, "rank": rank}, "options": {}
            }}
        });
        assert_eq!(call(port, "/protocol/v1/execute", &request, None).0, 200);
    }
    let first = serde_json::json!({
        "protocolVersion": 1, "context": {}, "operation": { "kind": "query", "query": {
            "kind": "records", "query": {"labels":["CURSOR_ITEM"], "predicate":{}, "order":{"rank":"asc"}, "limit":2, "cursorTtlMs":1000}
        }}
    });
    let (status, first) = call(port, "/protocol/v1/execute", &first, None);
    assert_eq!(status, 200, "{first}");
    let cursor = first["response"]["cursorId"].as_str().unwrap().to_string();
    assert_eq!(first["response"]["records"].as_array().unwrap().len(), 2);
    let next = serde_json::json!({
        "protocolVersion": 1, "context": {}, "operation": { "kind": "query", "query": {
            "kind": "cursor", "cursorId": cursor, "limit": 2
        }}
    });
    let (status, next) = call(port, "/protocol/v1/execute", &next, None);
    assert_eq!(status, 200, "{next}");
    assert_eq!(next["response"]["total"], 3);
    assert_eq!(next["response"]["records"].as_array().unwrap().len(), 1);
    assert!(next["response"]["cursorId"].is_null());
}

#[test]
fn http_executes_all_writable_query_language_profiles() {
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "test-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet {
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));

    for (language, statement) in [
        ("cypher", "CREATE (:Person {name: 'Cypher'})"),
        ("gql", "INSERT (:Person {name: 'GQL'})"),
        ("gremlin", "g.addV('Person').property('name', 'Gremlin')"),
    ] {
        let (status, response) = call(
            port,
            "/protocol/v1/execute",
            &statement_mutation(language, statement),
            None,
        );
        assert_eq!(status, 200, "{language}: {response}");
        assert_eq!(response["response"]["kind"], "statementMutation");
        assert!(response["response"]["affected"].as_u64().unwrap() > 0);
    }

    let (status, response) = call(
        port,
        "/protocol/v1/execute",
        &statement_mutation("sql", "INSERT INTO person (name) VALUES ('DataFusion')"),
        None,
    );
    assert_eq!(status, 200, "sql: {response}");
    assert_eq!(response["response"]["kind"], "statementMutation");
    assert_eq!(response["response"]["affected"], 1);

    let query = serde_json::json!({
        "protocolVersion": 1,
        "context": {},
        "operation": {
            "kind": "query",
            "query": {
                "kind": "statement",
                "language": "sql",
                "statement": "SELECT name FROM Person ORDER BY name"
            }
        }
    });
    let (status, response) = call(port, "/protocol/v1/execute", &query, None);
    assert_eq!(status, 200, "{response}");
    assert_eq!(response["response"]["total"], 4);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
)]
fn protocol_http_preserves_raw_envelope_and_fails_closed_on_scope_mismatch() {
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "test-token".into(),
        data_dir: None,
        multitenant: true,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet {
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));

    let (source_status, source_response) = call(
        port,
        "/protocol/v1/execute",
        &put_request(
            "tenant-a",
            "SOURCE",
            &serde_json::json!({ "targetId": ["target-1", "target-1"] }),
        ),
        Some("tenant-a"),
    );
    assert_eq!(source_status, 200);
    assert_eq!(source_response["response"]["kind"], "record");
    assert_eq!(source_response["response"]["record"]["label"], "SOURCE");
    assert!(
        source_response["response"]["record"]
            .get("properties")
            .and_then(|p| p.get("tenantKey"))
            .is_none(),
        "multitenant records must not expose a tenant discriminator property: {source_response}"
    );
    assert!(
        source_response["response"]["record"].get("__id").is_none(),
        "canonical records must not expose SDK aliases: {source_response}"
    );
    let (target_status, _) = call(
        port,
        "/protocol/v1/execute",
        &put_request(
            "tenant-a",
            "TARGET",
            &serde_json::json!({ "id": "target-1" }),
        ),
        Some("tenant-a"),
    );
    assert_eq!(target_status, 200);

    let (status, response) = call(
        port,
        "/protocol/v1/execute",
        &join_request("tenant-a"),
        Some("tenant-a"),
    );
    assert_eq!(status, 200, "{response}");
    assert_eq!(response["protocolVersion"], 1);
    assert_eq!(response["requestId"], "http-join-1");
    assert_eq!(response["response"]["kind"], "count");
    assert_eq!(response["response"]["value"], 1);
    assert!(response.get("data").is_none(), "{response}");

    let (other_scope_status, _) = call(
        port,
        "/protocol/v1/execute",
        &put_request(
            "tenant-b",
            "SOURCE",
            &serde_json::json!({ "targetId": "target-1" }),
        ),
        Some("tenant-b"),
    );
    assert_eq!(other_scope_status, 200);
    let (query_status, query_response) = call(
        port,
        "/protocol/v1/execute",
        &records_request("tenant-a", "SOURCE"),
        Some("tenant-a"),
    );
    assert_eq!(query_status, 200, "{query_response}");
    assert_eq!(query_response["response"]["kind"], "records");
    assert_eq!(query_response["response"]["total"], 1);
    assert!(
        query_response["response"]["records"][0]["properties"]
            .get("tenantKey")
            .is_none(),
        "records must not expose the tenant discriminator: {query_response}"
    );

    let (mismatch_status, mismatch) = call(
        port,
        "/protocol/v1/execute",
        &join_request("tenant-b"),
        Some("tenant-a"),
    );
    assert_eq!(mismatch_status, 403, "{mismatch}");
    assert!(
        mismatch["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("authenticated transport scope"),
        "{mismatch}"
    );

    let (removed_route_status, _) = call(
        port,
        "/api/v1/records",
        &serde_json::json!({}),
        Some("tenant-a"),
    );
    assert_eq!(removed_route_status, 404);
}

#[test]
fn single_tenant_mode_has_no_scope_or_discriminator() {
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "test-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet {
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));

    let request = serde_json::json!({
        "protocolVersion": 1,
        "context": {},
        "operation": {
            "kind": "mutate",
            "mutation": {
                "kind": "putRecord",
                "mode": "create",
                "label": "NOTE",
                "properties": { "text": "standalone" },
                "options": {}
            }
        }
    });
    let (status, response) = call(port, "/protocol/v1/execute", &request, None);
    assert_eq!(status, 200, "{response}");
    assert_eq!(
        response["response"]["record"]["properties"],
        serde_json::json!({ "text": "standalone" })
    );

    let scoped_request = put_request("unexpected", "NOTE", &serde_json::json!({ "text": "bad" }));
    let (scoped_status, scoped_response) =
        call(port, "/protocol/v1/execute", &scoped_request, None);
    assert_eq!(scoped_status, 403, "{scoped_response}");
    assert_eq!(scoped_response["error"]["code"], "scope_mismatch");
}

#[test]
fn label_less_ingest_derives_labels_from_complex_container_keys() {
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "test-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet {
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));

    let (status, response) = call(
        port,
        "/protocol/v1/execute",
        &ingest_request_with_options(
            &serde_json::json!({
                "battle": [{ "name": "Battle of Yavin" }],
                "planet": { "name": "Alderaan" },
                "starship": [{ "name": "Millennium Falcon" }],
                "faction": { "name": "Rebel Alliance" },
                "character": [{ "name": "Leia Organa" }]
            }),
            &serde_json::json!({
                "returnResult": true,
                "capitalizeLabels": true
            }),
        ),
        None,
    );
    assert_eq!(status, 200, "{response}");
    assert_eq!(response["response"]["kind"], "ingested");
    assert_eq!(response["response"]["count"], 5);
    let labels = response["response"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|record| record["label"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec!["BATTLE", "PLANET", "STARSHIP", "FACTION", "CHARACTER"]
    );

    let (labels_status, labels_response) =
        call(port, "/protocol/v1/execute", &labels_request(), None);
    assert_eq!(labels_status, 200, "{labels_response}");
    assert_eq!(labels_response["response"]["kind"], "labels");
    assert_eq!(
        labels_response["response"]["labels"],
        serde_json::json!(["BATTLE", "PLANET", "STARSHIP", "FACTION", "CHARACTER"])
    );

    let (invalid_status, invalid_response) = call(
        port,
        "/protocol/v1/execute",
        &ingest_request(&serde_json::json!({ "name": "Leia Organa" })),
        None,
    );
    assert_eq!(invalid_status, 400, "{invalid_response}");
    assert_eq!(invalid_response["error"]["code"], "invalid_request");
    assert!(invalid_response["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("label is required"));
}

#[test]
fn default_ingest_options_return_ingested_without_inline_records() {
    // `returnResult` defaults to false, so the engine reports its success
    // shape as `data: true` (or omits `data` above the inline bound). The
    // protocol must accept that as a successful ingest with no inline
    // records instead of rejecting it as a malformed engine response.
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "test-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet::default(),
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));

    let (status, response) = call(
        port,
        "/protocol/v1/execute",
        &serde_json::json!({
            "protocolVersion": 1,
            "context": {},
            "operation": {
                "kind": "ingest",
                "ingest": {
                    "kind": "documents",
                    "label": "DEFAULT_OPTS",
                    "documents": [{ "name": "ok" }],
                    "options": {}
                }
            }
        }),
        None,
    );
    assert_eq!(status, 200, "{response}");
    assert_eq!(response["response"]["kind"], "ingested");
    assert_eq!(response["response"]["count"], 1);
    assert_eq!(response["response"]["records"], serde_json::json!([]));
}

/// The client-facing property type name for booleans is `boolean` (`NestJS`/
/// UI contract), never `bool`. `describe.properties` must surface it.
#[test]
fn describe_properties_exposes_boolean_type_name() {
    let cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "test-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet {
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
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));

    let (status, response) = call(
        port,
        "/protocol/v1/execute",
        &serde_json::json!({
            "protocolVersion": 1,
            "context": {},
            "operation": {
                "kind": "mutate",
                "mutation": {
                    "kind": "putRecord",
                    "mode": "create",
                    "label": "FLAG",
                    "properties": { "active": true, "count": 3 },
                    "options": {}
                }
            }
        }),
        None,
    );
    assert_eq!(status, 200, "{response}");

    let (describe_status, describe_response) = call(
        port,
        "/protocol/v1/execute",
        &serde_json::json!({
            "protocolVersion": 1,
            "context": {},
            "operation": {
                "kind": "describe",
                "describe": {
                    "kind": "properties",
                    "query": {
                        "labels": ["FLAG"],
                        "predicate": {},
                        "offset": 0
                    }
                }
            }
        }),
        None,
    );
    assert_eq!(describe_status, 200, "{describe_response}");
    assert_eq!(describe_response["response"]["kind"], "properties");
    let properties = describe_response["response"]["properties"]
        .as_array()
        .unwrap()
        .clone();
    let active = properties
        .iter()
        .find(|p| p["name"] == serde_json::json!("active"))
        .expect("active property");
    assert_eq!(active["type"], serde_json::json!("boolean"));
    let count = properties
        .iter()
        .find(|p| p["name"] == serde_json::json!("count"))
        .expect("count property");
    assert_eq!(count["type"], serde_json::json!("number"));
}

#[test]
fn early_http_refusals_use_the_status_of_their_error_code() {
    let cfg = ServerConfig {
        port: 0,
        token: "test-token".into(),
        multitenant: true,
        ..ServerConfig::default()
    };
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));
    let (status, body) = call(port, "/protocol/v1/execute", &labels_request(), None);
    assert_eq!(body["error"]["code"], "missing_scope", "{body}");
    assert_eq!(status, 403, "missing_scope maps to 403 everywhere");
}

#[test]
fn only_the_boot_token_may_change_a_tenant_policy() {
    const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";
    let cfg = ServerConfig {
        port: 0,
        token: "boot-token".into(),
        multitenant: true,
        capability_secret: Some(SECRET.to_vec()),
        ..ServerConfig::default()
    };
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(40));
    let capability = rushdb_server::capability::issue_for_tenant(
        SECRET,
        "service-a",
        "*",
        Some("tenant-a"),
        60,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    let put = serde_json::json!({
        "protocolVersion": 1,
        "context": { "scope": "tenant-a" },
        "operation": { "kind": "policy", "policy": {
            "kind": "put", "policy": { "allowedLabels": [] }, "enforced": false } }
    });
    let (status, _) = call_with_token(
        port,
        "/protocol/v1/execute",
        &put,
        Some("tenant-a"),
        &capability,
    );
    assert_eq!(
        status, 401,
        "a capability token, even scoped `*`, cannot change the policy"
    );
    let (status, body) = call_with_token(
        port,
        "/protocol/v1/execute",
        &put,
        Some("tenant-a"),
        "boot-token",
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["response"]["kind"], "policy", "{body}");
}
