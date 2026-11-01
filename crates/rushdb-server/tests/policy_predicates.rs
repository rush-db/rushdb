//! An enforced policy bounds what a query may use, not only what it returns
//! (POL-1, found verifying the blog's access-control example 2026-10-31):
//! filtering, sorting or aggregating on a hidden property, or traversing a
//! denied relationship type, is refused instead of answering from the hidden
//! data while masking the output.

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
    NodeEnvelopeHandler::new(Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap")))
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

/// Worker (salary 100) `-REPORTS_TO->` Boss (salary 200), and Peer (120), with
/// the blog's policy: `EMP` only, `name` visible, `REPORTS_TO` denied.
fn company() -> NodeEnvelopeHandler {
    let handler = node();
    call(
        &handler,
        &json!({ "kind": "ingest", "ingest": { "kind": "documents", "label": "EMP",
            "documents": [
                { "name": "Worker", "salary": 100, "EMP": { "name": "Boss", "salary": 200 } },
                { "name": "Peer", "salary": 120 } ],
            "options": { "relationshipTypes": { "EMP": "REPORTS_TO" } } } }),
    );
    call(
        &handler,
        &json!({ "kind": "policy", "policy": { "kind": "put", "enforced": true, "policy": {
            "allowedLabels": ["EMP"], "deniedRelTypes": ["REPORTS_TO"],
            "visibleProperties": { "EMP": ["name"] } } } }),
    );
    handler
}

fn records(query: &Value) -> Value {
    json!({ "kind": "query", "query": { "kind": "records", "query": query } })
}

fn names(out: &Value) -> Vec<String> {
    out["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|r| {
            r["properties"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

fn refused(handler: &NodeEnvelopeHandler, query: &Value, why: &str) {
    match try_call(handler, &records(query)) {
        Err(error) => assert!(error.contains(why), "{error}"),
        Ok(out) => panic!("expected a refusal naming `{why}`, got {out}"),
    }
}

#[test]
fn a_filter_on_a_hidden_property_is_refused() {
    let handler = company();
    refused(
        &handler,
        &json!({ "labels": ["EMP"], "predicate": { "salary": { "$gt": 150 } } }),
        "salary",
    );
    refused(
        &handler,
        &json!({ "labels": ["EMP"], "predicate": { "$or": [ { "name": "Peer" }, { "salary": 200 } ] } }),
        "salary",
    );
    // Through a related block too.
    refused(
        &handler,
        &json!({ "labels": ["EMP"], "predicate": { "EMP": {
            "$relation": { "type": "WORKS_WITH", "direction": "out" }, "salary": 200 } } }),
        "salary",
    );
}

#[test]
fn sorting_by_a_hidden_property_is_refused() {
    let handler = company();
    refused(
        &handler,
        &json!({ "labels": ["EMP"], "order": { "salary": "desc" } }),
        "salary",
    );
}

#[test]
fn aggregating_a_hidden_property_is_refused() {
    let handler = company();
    refused(
        &handler,
        &json!({ "labels": ["EMP"], "select": { "top": { "$max": "$record.salary" } } }),
        "salary",
    );
    refused(
        &handler,
        &json!({ "labels": ["EMP"], "select": { "n": { "$count": "*" } }, "groupBy": ["salary"] }),
        "salary",
    );
}

#[test]
fn traversing_a_denied_relationship_type_is_refused() {
    let handler = company();
    refused(
        &handler,
        &json!({ "labels": ["EMP"], "predicate": { "EMP": {
            "$relation": { "type": "REPORTS_TO", "direction": "out" }, "name": "Boss" } } }),
        "REPORTS_TO",
    );
    // An untyped traversal could cross REPORTS_TO, so it must name a type.
    refused(
        &handler,
        &json!({ "labels": ["EMP"], "predicate": { "EMP": { "name": "Boss" } } }),
        "relationship type",
    );
}

#[test]
fn reads_on_visible_properties_still_work() {
    let handler = company();
    let out = call(
        &handler,
        &records(
            &json!({ "labels": ["EMP"], "predicate": { "name": { "$in": ["Peer", "Boss"] } },
            "order": { "name": "asc" } }),
        ),
    );
    assert_eq!(names(&out), ["Boss", "Peer"]);
    let props = &out["records"][0]["properties"];
    assert!(props.get("salary").is_none(), "{props}");
    let out = call(
        &handler,
        &records(&json!({ "labels": ["EMP"], "select": { "n": { "$count": "*" } } })),
    );
    assert!(out.to_string().contains('3'), "{out}");
}

#[test]
fn without_a_policy_the_same_reads_answer() {
    let handler = node();
    call(
        &handler,
        &json!({ "kind": "ingest", "ingest": { "kind": "documents", "label": "EMP",
            "documents": [ { "name": "Worker", "salary": 100, "EMP": { "name": "Boss", "salary": 200 } } ],
            "options": { "relationshipTypes": { "EMP": "REPORTS_TO" } } } }),
    );
    let out = call(
        &handler,
        &records(&json!({ "labels": ["EMP"], "predicate": { "salary": { "$gt": 150 } } })),
    );
    assert_eq!(names(&out), ["Boss"]);
    let out = call(
        &handler,
        &records(&json!({ "labels": ["EMP"], "predicate": { "EMP": {
            "$relation": { "type": "REPORTS_TO", "direction": "out" }, "name": "Boss" } } })),
    );
    assert_eq!(names(&out), ["Worker"]);
}

#[test]
fn sql_cannot_filter_or_sort_on_a_hidden_property() {
    let handler = company();
    let sql = |statement: &str| {
        try_call(
            &handler,
            &json!({ "kind": "query", "query": { "kind": "statement", "language": "sql",
                "statement": statement } }),
        )
    };
    for statement in [
        "SELECT name FROM emp WHERE salary > 150",
        "SELECT name FROM emp ORDER BY salary DESC",
        "SELECT max(salary) FROM emp",
    ] {
        // SQL's schema leaves hidden columns out, so they do not plan.
        assert!(sql(statement).is_err(), "{statement} must be refused");
    }
    match sql("SELECT * FROM rushdb_edges.reports_to") {
        Err(_) => {}
        Ok(out) => assert!(!out.to_string().contains("__"), "denied edges read: {out}"),
    }
    let out = sql("SELECT name FROM emp ORDER BY name").expect("visible column");
    assert!(out.to_string().contains("Peer"), "{out}");
}
