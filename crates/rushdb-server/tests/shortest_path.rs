//! `query.path`: the shortest path between two records, as the records on it
//! in order. The search follows one relationship type in one direction and
//! honours the enforced policy: it never follows a denied type and never
//! passes through a record whose label the policy hides.

use std::sync::Arc;

use rushdb_protocol::{EnvelopeHandler, RequestEnvelope};
use rushdb_server::{Node, NodeEnvelopeHandler, ServerConfig};
use serde_json::{json, Value};

fn node() -> NodeEnvelopeHandler {
    let mut cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "t".into(),
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
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    NodeEnvelopeHandler::new(node, &cfg)
}

fn try_call(handler: &NodeEnvelopeHandler, operation: &Value) -> Result<Value, String> {
    let envelope: RequestEnvelope =
        serde_json::from_value(json!({ "protocolVersion": 1, "operation": operation }))
            .expect("envelope");
    handler
        .execute(envelope, None)
        .map(|response| serde_json::to_value(response.response).expect("response json"))
        .map_err(|error| format!("{error:?}"))
}

fn call(handler: &NodeEnvelopeHandler, operation: &Value) -> Value {
    try_call(handler, operation).expect("request succeeds")
}

/// `A -LINK-> B -LINK-> C -LINK-> D`, a shortcut `A -LINK-> X -LINK-> D`
/// through `X` (label `SECRET`), and `D -AUDIT-> A`. Returns the handler and
/// each record's id by name.
fn graph() -> (NodeEnvelopeHandler, impl Fn(&str) -> String) {
    let handler = node();
    let mut ids = std::collections::HashMap::new();
    for (label, name) in [
        ("NODE", "A"),
        ("NODE", "B"),
        ("NODE", "C"),
        ("NODE", "D"),
        ("SECRET", "X"),
    ] {
        let out = call(
            &handler,
            &json!({ "kind": "ingest", "ingest": { "kind": "documents", "label": label,
                "documents": [{ "name": name }] } }),
        );
        let found = call(
            &handler,
            &json!({ "kind": "query", "query": { "kind": "records", "query": {
                "labels": [label], "predicate": { "name": name } } } }),
        );
        let id = found["records"][0]["id"].as_str().expect("id").to_string();
        assert!(!out.is_null());
        ids.insert(name.to_string(), id);
    }
    let link = |rel: &str, pairs: &[(&str, &str)]| {
        let pairs: Vec<Value> = pairs
            .iter()
            .map(|(a, b)| json!({ "sourceId": ids[*a], "targetId": ids[*b] }))
            .collect();
        call(
            &handler,
            &json!({ "kind": "mutate", "mutation": { "kind": "linkRecordsPairs",
                "command": { "relationType": rel, "pairs": pairs } } }),
        );
    };
    link(
        "LINK",
        &[("A", "B"), ("B", "C"), ("C", "D"), ("A", "X"), ("X", "D")],
    );
    link("AUDIT", &[("D", "A")]);
    (handler, move |name: &str| ids[name].clone())
}

fn path(handler: &NodeEnvelopeHandler, query: &Value) -> Result<Vec<String>, String> {
    let out = try_call(
        handler,
        &json!({ "kind": "query", "query": { "kind": "path", "query": query } }),
    )?;
    assert_eq!(
        out["total"].as_u64(),
        out["records"].as_array().map(|r| r.len() as u64)
    );
    Ok(out["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|r| {
            r["properties"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect())
}

#[test]
fn the_shortest_path_comes_back_as_its_records_in_order() {
    let (handler, id) = graph();
    let q = |from: &str, to: &str, extra: Value| {
        let mut q = json!({ "from": id(from), "to": id(to), "type": "LINK", "maxHops": 6 });
        q.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        q
    };
    assert_eq!(
        path(&handler, &q("A", "D", json!({}))).unwrap(),
        ["A", "X", "D"]
    );
    // `direction` defaults to both; `out` follows the relationships' way.
    assert_eq!(
        path(&handler, &q("D", "A", json!({}))).unwrap(),
        ["D", "X", "A"]
    );
    assert!(path(&handler, &q("D", "A", json!({ "direction": "out" })))
        .unwrap()
        .is_empty());
    assert_eq!(
        path(&handler, &q("D", "A", json!({ "direction": "in" }))).unwrap(),
        ["D", "X", "A"]
    );
    // `labels` keeps the path on records of those labels.
    assert_eq!(
        path(&handler, &q("A", "D", json!({ "labels": ["NODE"] }))).unwrap(),
        ["A", "B", "C", "D"]
    );
    // Beyond `maxHops` there is no path; a record is its own 0-hop path.
    assert!(path(
        &handler,
        &q("A", "D", json!({ "labels": ["NODE"], "maxHops": 2 }))
    )
    .unwrap()
    .is_empty());
    assert_eq!(
        path(&handler, &q("B", "B", json!({ "maxHops": 0 }))).unwrap(),
        ["B"]
    );
    // Another type, or every type.
    let audit = json!({ "from": id("A"), "to": id("D"), "type": "AUDIT", "maxHops": 6 });
    assert_eq!(path(&handler, &audit).unwrap(), ["A", "D"]);
    let any = json!({ "from": id("A"), "to": id("D"), "maxHops": 6 });
    assert_eq!(path(&handler, &any).unwrap(), ["A", "D"]);
    // An unknown record is not found.
    let missing =
        json!({ "from": "00000000-0000-0000-0000-000000000001", "to": id("D"), "maxHops": 6 });
    assert!(path(&handler, &missing).unwrap_err().contains("NotFound"));
}

#[test]
fn a_path_honours_the_enforced_policy() {
    let (handler, id) = graph();
    call(
        &handler,
        &json!({ "kind": "policy", "policy": { "kind": "put", "enforced": true, "policy": {
            "allowedLabels": ["NODE"], "deniedRelTypes": ["AUDIT"] } } }),
    );
    // The shortcut runs through a hidden record: the path goes around it.
    let link = json!({ "from": id("A"), "to": id("D"), "type": "LINK", "maxHops": 6 });
    assert_eq!(path(&handler, &link).unwrap(), ["A", "B", "C", "D"]);
    let short = json!({ "from": id("A"), "to": id("D"), "type": "LINK", "maxHops": 2 });
    assert!(path(&handler, &short).unwrap().is_empty());
    // A denied type, or no type when some are denied, is refused.
    let audit = json!({ "from": id("A"), "to": id("D"), "type": "AUDIT", "maxHops": 6 });
    assert!(path(&handler, &audit).unwrap_err().contains("Unauthorized"));
    let any = json!({ "from": id("A"), "to": id("D"), "maxHops": 6 });
    assert!(path(&handler, &any).unwrap_err().contains("Unauthorized"));
    // So is a hidden end.
    let hidden = json!({ "from": id("A"), "to": id("X"), "type": "LINK", "maxHops": 6 });
    assert!(path(&handler, &hidden)
        .unwrap_err()
        .contains("Unauthorized"));
}
