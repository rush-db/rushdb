//! USE-1: `POST /internal/usage` reports per-tenant usage for a list of
//! tenant scopes plus their total, takes the operator token, and never
//! registers a scope it is asked about.

use std::io::{Read, Write};
use std::sync::Arc;

use rushdb_protocol::VectorIndexDefinition;
use rushdb_server::{Node, RequestScope, ServerConfig};
use serde_json::{json, Value};

fn serve(node: &Arc<Node>, cfg: ServerConfig) -> u16 {
    let (mut router, port) = rushdb_server::http::bind_router(cfg, Arc::clone(node)).unwrap();
    std::thread::spawn(move || {
        let _ = router.serve_forever();
    });
    port
}

fn post(port: u16, token: &str, body: &str) -> (u16, Value) {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "POST /internal/usage HTTP/1.1\r\nHost: x\r\nX-RushDB-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response[9..12].parse().unwrap();
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default();
    // Chunked bodies carry a size line before the JSON.
    let json_start = body.find('{').unwrap_or(0);
    let json_end = body.rfind('}').map_or(body.len(), |end| end + 1);
    (
        status,
        serde_json::from_str(&body[json_start..json_end]).unwrap(),
    )
}

fn seed(node: &Node, scope: &str, records: u16, with_vectors: bool) {
    let request = RequestScope::new(node, Some(scope.to_string()));
    if with_vectors {
        request
            .create_vector_index(&VectorIndexDefinition {
                name: Some("doc_embedding".into()),
                label: "DOC".into(),
                property: "embedding".into(),
                dimensions: 2,
                similarity: "cosine".into(),
                model: None,
                owner: None,
            })
            .unwrap();
    }
    let mut previous: Option<String> = None;
    for i in 0..records {
        let properties = json!({ "n": i, "name": format!("r{i}") });
        let created = request
            .create_record("DOC", &properties, &[], &json!({}))
            .unwrap();
        let id = created["id"].as_str().unwrap().to_string();
        if with_vectors {
            request
                .upsert_vector_index(
                    "doc_embedding",
                    &[rushdb_protocol::VectorItem {
                        record_id: id.clone(),
                        vector: vec![1.0, f32::from(i)],
                        source_hash: None,
                        index_generation: None,
                    }],
                )
                .unwrap();
        }
        if let Some(previous) = previous {
            request
                .link_records_by_id(&rushdb_protocol::RelationshipIdsMutation {
                    relation_type: "NEXT".into(),
                    direction: rushdb_protocol::Direction::default(),
                    source_ids: vec![previous],
                    target_ids: vec![id.clone()],
                    properties: json!({}),
                })
                .unwrap();
        }
        previous = Some(id);
    }
}

#[test]
fn usage_reports_each_tenant_and_their_total() {
    let mut cfg = ServerConfig {
        multitenant: true,
        token: "operator-token".into(),
        port: 0,
        ..Default::default()
    };
    let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
    seed(&node, "ws-a", 3, true);
    seed(&node, "ws-b", 2, false);
    let port = serve(&node, cfg);

    let (status, usage) = post(
        port,
        "operator-token",
        r#"{"tenants":["ws-a","ws-b","never-seen"]}"#,
    );
    assert_eq!(status, 200, "{usage}");
    let rows = usage["tenants"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let a = &rows[0];
    assert_eq!(a["tenant"], "ws-a");
    assert_eq!(a["exists"], true);
    assert_eq!(a["records"], 3);
    assert_eq!(a["relationships"], 2);
    assert_eq!(a["labels"], 1);
    assert!(a["properties"].as_u64().unwrap() >= 2, "{a}");
    assert!(a["logicalBytes"].as_u64().unwrap() > 0);
    assert_eq!(a["vectors"], 3);
    assert_eq!(
        a["vectorIndexes"],
        json!([{ "index": "doc_embedding", "vectors": 3 }])
    );
    let b = &rows[1];
    assert_eq!(
        (b["records"].as_u64(), b["relationships"].as_u64()),
        (Some(2), Some(1))
    );
    assert_eq!(b["vectors"], 0);
    let unknown = &rows[2];
    assert_eq!(unknown["exists"], false);
    assert_eq!(unknown["records"], 0);
    assert_eq!(unknown["logicalBytes"], 0);

    let total = &usage["total"];
    for metric in [
        "records",
        "relationships",
        "labels",
        "properties",
        "logicalBytes",
        "vectors",
    ] {
        assert_eq!(
            total[metric].as_u64().unwrap(),
            a[metric].as_u64().unwrap() + b[metric].as_u64().unwrap(),
            "total {metric}"
        );
    }
    // Asking about a scope does not register it: a second read still finds
    // no such tenant.
    let (_, again) = post(port, "operator-token", r#"{"tenants":["never-seen"]}"#);
    assert_eq!(again["tenants"][0]["exists"], false);
}

#[test]
fn usage_requires_the_operator_token_and_a_valid_request() {
    let mut cfg = ServerConfig {
        multitenant: true,
        token: "operator-token".into(),
        port: 0,
        ..Default::default()
    };
    let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
    let port = serve(&node, cfg);
    assert_eq!(post(port, "wrong", r#"{"tenants":["a"]}"#).0, 401);
    assert_eq!(post(port, "operator-token", r#"{"tenants":[]}"#).0, 400);
    assert_eq!(post(port, "operator-token", r#"{"tenant":"a"}"#).0, 400);
}

#[test]
fn single_tenant_usage_reports_the_one_tenant() {
    let mut cfg = ServerConfig {
        token: "operator-token".into(),
        port: 0,
        ..Default::default()
    };
    let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
    node.scope()
        .create_record("A", &json!({ "x": 1 }), &[], &json!({}))
        .unwrap();
    let port = serve(&node, cfg);
    let (status, usage) = post(port, "operator-token", "{}");
    assert_eq!(status, 200, "{usage}");
    assert_eq!(usage["tenants"][0]["tenant"], Value::Null);
    assert_eq!(usage["tenants"][0]["records"], 1);
    assert_eq!(usage["total"]["records"], 1);
    assert_eq!(post(port, "operator-token", r#"{"tenants":["a"]}"#).0, 400);
}
