//! Text search end to end over HTTP, with the server's own background
//! index builder and applier: what a client does, in the order it does it.
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use rushdb_server::{new_router, ServerConfig};
use serde_json::{json, Value};

fn execute(port: u16, operation: &Value) -> Value {
    let body = json!({"protocolVersion": 1, "context": {}, "operation": operation}).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request = format!(
        "POST /protocol/v1/execute HTTP/1.1\r\nHost: localhost\r\nX-RushDB-Token: test-token\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let (head, payload) = raw.split_once("\r\n\r\n").expect("a response");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}\n{payload}");
    serde_json::from_str(payload).expect("JSON")
}

fn serve() -> u16 {
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
    port
}

/// Wait for every index to be ready, then search until nothing is pending.
fn search_settled(port: u16, index: &str, query: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let indexes = execute(
            port,
            &json!({"kind": "describe", "describe": {"kind": "indexes"}}),
        );
        let ready = indexes["response"]["indexes"]
            .as_array()
            .is_some_and(|all| all.iter().all(|i| i["state"] == "ready"));
        if ready {
            let hits = execute(
                port,
                &json!({"kind": "query", "query": {"kind": "text", "index": index, "query": query, "limit": 5}}),
            );
            let response = hits["response"].clone();
            if response["pending"] == 0 || Instant::now() > deadline {
                return response;
            }
        }
        assert!(Instant::now() < deadline, "{index} never settled");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn documents_ingested_then_indexed_are_found() {
    // A text index created over existing records lost them when the
    // applier, woken by the index's own creation, judged coverage by the
    // definition from before the build: the search settled with no hits
    // and nothing pending. The timing varies, so the sequence is repeated.
    let port = serve();
    for round in 0..4 {
        for (property, index_first) in [("body", false), ("*", false), ("title", true)] {
            let label = format!("docs_{round}_{}", property.replace('*', "all"));
            let index = format!("{label}_text");
            let create = json!({"kind": "index", "index": {"kind": "create", "definition": {
                "name": index, "label": label, "property": property, "kind": "text"}}});
            if index_first {
                execute(port, &create);
            }
            execute(
                port,
                &json!({"kind": "ingest", "ingest": {"kind": "documents", "label": label, "documents": [
                    {"doc_id": 1, "title": "quantum world", "body": "machines learn quantum mechanics"},
                    {"doc_id": 2, "title": "dragons", "body": "magic dragon call"}
                ], "options": {"returnResult": true}}}),
            );
            if !index_first {
                execute(port, &create);
            }
            let found = search_settled(port, &index, "quantum");
            assert_eq!(
                found["rows"].as_array().map(Vec::len),
                Some(1),
                "round {round}, property {property}, index first {index_first}: {found}"
            );
        }
    }
}
