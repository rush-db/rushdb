//! Retyping relationships merges each endpoint pair into one edge of the
//! target type and keeps every relationship property from both sides
//! (REL-1, owner decision 2026-11-01). Under the default `mergeStrategy`
//! (`keep`) an existing target-type edge keeps its identity and values and
//! the retyped edges add what it lacks; `append` lets the retyped values win,
//! `rewrite` replaces the existing edge's properties, and `fail` refuses a
//! retype with any conflict. The result counts the edges retyped and merged.

use std::sync::Arc;

use rushdb_protocol::{EnvelopeHandler, ErrorCode, ProtocolError, RequestEnvelope};
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

fn try_call(handler: &NodeEnvelopeHandler, operation: &Value) -> Result<Value, ProtocolError> {
    let envelope: RequestEnvelope =
        serde_json::from_value(json!({ "protocolVersion": 1, "operation": operation }))
            .expect("envelope");
    let response = handler.execute(envelope, None)?;
    Ok(serde_json::to_value(response.response).expect("response json"))
}

fn call(handler: &NodeEnvelopeHandler, operation: &Value) -> Value {
    try_call(handler, operation).expect("request succeeds")
}

fn create(handler: &NodeEnvelopeHandler, label: &str, name: &str) -> String {
    call(
        handler,
        &json!({ "kind": "ingest", "ingest": { "kind": "documents", "label": label,
            "documents": [ { "name": name } ] } }),
    );
    let found = call(
        handler,
        &json!({ "kind": "query", "query": { "kind": "records", "query": {
            "labels": [label], "predicate": { "name": name } } } }),
    );
    found["records"][0]["id"].as_str().expect("id").to_string()
}

fn link(handler: &NodeEnvelopeHandler, rel_type: &str, from: &str, to: &str, properties: &Value) {
    call(
        handler,
        &json!({ "kind": "mutate", "mutation": { "kind": "linkRecordsPairs", "command": {
            "relationType": rel_type, "pairs": [ { "sourceId": from, "targetId": to } ],
            "properties": properties } } }),
    );
}

fn relationships(handler: &NodeEnvelopeHandler) -> Vec<Value> {
    let out = call(
        handler,
        &json!({ "kind": "query", "query": { "kind": "relationships", "query": {} } }),
    );
    out["relationships"].as_array().cloned().unwrap_or_default()
}

struct Fixture {
    handler: NodeEnvelopeHandler,
    ada: String,
    works_at_id: Value,
}

/// Ada already `WORKS_AT` Acme {since 2020, badge 7} and is also `EMPLOYED_BY`
/// it {since `ada_since`, role, `startedAt`}; Bo is only `EMPLOYED_BY` {role}.
fn fixture(ada_since: i64) -> Fixture {
    let handler = node();
    let ada = create(&handler, "PERSON", "Ada");
    let bo = create(&handler, "PERSON", "Bo");
    let acme = create(&handler, "COMPANY", "Acme");
    link(
        &handler,
        "WORKS_AT",
        &ada,
        &acme,
        &json!({ "since": 2020, "badge": 7 }),
    );
    link(
        &handler,
        "EMPLOYED_BY",
        &ada,
        &acme,
        &json!({ "since": ada_since, "role": "engineer", "startedAt": "2019-03-01T00:00:00Z" }),
    );
    link(
        &handler,
        "EMPLOYED_BY",
        &bo,
        &acme,
        &json!({ "role": "designer" }),
    );
    let works_at_id = relationships(&handler)
        .iter()
        .find(|r| r.to_string().contains("WORKS_AT"))
        .map(|r| r["id"].clone())
        .expect("WORKS_AT edge");
    Fixture {
        handler,
        ada,
        works_at_id,
    }
}

fn retype(handler: &NodeEnvelopeHandler, strategy: Option<&str>) -> Result<Value, ProtocolError> {
    let mut command = json!({ "sourceLabel": "PERSON", "targetLabel": "COMPANY",
        "sourceType": "EMPLOYED_BY", "targetType": "WORKS_AT" });
    if let Some(strategy) = strategy {
        command["mergeStrategy"] = json!(strategy);
    }
    try_call(
        handler,
        &json!({ "kind": "mutate", "mutation": { "kind": "retypeRelations", "command": command } }),
    )
}

/// After a successful retype: two `WORKS_AT` edges, the existing one kept by
/// id; returns (Ada's properties, Bo's properties).
fn merged_edges(f: &Fixture) -> (Value, Value) {
    let after = relationships(&f.handler);
    assert_eq!(after.len(), 2, "one WORKS_AT edge per pair: {after:#?}");
    assert!(
        after.iter().all(|r| !r.to_string().contains("EMPLOYED_BY")),
        "{after:#?}"
    );
    let ada = after
        .iter()
        .find(|r| r["id"] == f.works_at_id)
        .expect("the existing WORKS_AT edge keeps its identity");
    let bo = after
        .iter()
        .find(|r| r["id"] != f.works_at_id)
        .expect("Bo's edge");
    (ada["properties"].clone(), bo["properties"].clone())
}

#[test]
fn keep_is_the_default_and_the_existing_value_wins() {
    for strategy in [None, Some("keep")] {
        let f = fixture(2019);
        let out = retype(&f.handler, strategy).expect("retype");
        assert_eq!(out["affected"], 2, "{out}");
        // Ada's edge folds into her existing WORKS_AT; their `since` differs.
        assert_eq!(out["merged"], 1, "{out}");
        assert_eq!(out["propertyConflicts"], 1, "{out}");
        let (ada, bo) = merged_edges(&f);
        assert_eq!(ada["since"], 2020, "the existing edge's value wins: {ada}");
        assert_eq!(ada["badge"], 7, "{ada}");
        assert_eq!(ada["role"], "engineer", "{ada}");
        assert!(ada.get("startedAt").is_some(), "{ada}");
        assert_eq!(bo["role"], "designer", "{bo}");
    }
}

#[test]
fn append_lets_the_retyped_value_win() {
    let f = fixture(2019);
    let out = retype(&f.handler, Some("append")).expect("retype");
    assert_eq!(out["propertyConflicts"], 1, "{out}");
    let (ada, bo) = merged_edges(&f);
    assert_eq!(ada["since"], 2019, "the retyped edge's value wins: {ada}");
    assert_eq!(
        ada["badge"], 7,
        "properties only the existing edge has stay: {ada}"
    );
    assert_eq!(ada["role"], "engineer", "{ada}");
    assert!(ada.get("startedAt").is_some(), "{ada}");
    assert_eq!(bo["role"], "designer", "{bo}");
}

#[test]
fn rewrite_replaces_the_existing_edges_properties() {
    let f = fixture(2019);
    let out = retype(&f.handler, Some("rewrite")).expect("retype");
    assert_eq!(out["merged"], 1, "{out}");
    let (ada, bo) = merged_edges(&f);
    assert_eq!(ada["since"], 2019, "{ada}");
    assert_eq!(ada["role"], "engineer", "{ada}");
    assert!(ada.get("startedAt").is_some(), "{ada}");
    assert!(
        ada.get("badge").is_none(),
        "the existing edge's own properties are removed: {ada}"
    );
    assert_eq!(bo["role"], "designer", "{bo}");
}

#[test]
fn fail_refuses_a_conflict_and_writes_nothing() {
    let f = fixture(2019);
    let before = relationships(&f.handler);
    let error = retype(&f.handler, Some("fail")).expect_err("a conflict is refused");
    assert_eq!(error.code, ErrorCode::Conflict, "{error:?}");
    assert!(!error.retryable, "{error:?}");
    assert!(
        error.message.contains("1 property conflict") && error.message.contains("`since`"),
        "{error:?}"
    );
    assert!(error.message.contains(&f.ada), "names the pair: {error:?}");
    assert_eq!(relationships(&f.handler), before, "nothing was written");
}

#[test]
fn fail_without_conflicts_merges_as_keep() {
    let f = fixture(2020);
    let out = retype(&f.handler, Some("fail")).expect("no conflict, so the retype runs");
    assert_eq!(out["merged"], 1, "{out}");
    assert_eq!(out["propertyConflicts"], 0, "{out}");
    let (ada, _) = merged_edges(&f);
    assert_eq!(ada["since"], 2020, "{ada}");
    assert_eq!(ada["badge"], 7, "{ada}");
    assert_eq!(ada["role"], "engineer", "{ada}");
}

#[test]
fn an_unknown_strategy_is_refused_when_decoding() {
    let error = serde_json::from_value::<RequestEnvelope>(json!({ "protocolVersion": 1,
        "operation": { "kind": "mutate", "mutation": { "kind": "retypeRelations", "command": {
            "sourceLabel": "PERSON", "targetLabel": "COMPANY", "targetType": "WORKS_AT",
            "mergeStrategy": "drop" } } } }))
    .expect_err("unknown strategy");
    let message = error.to_string();
    assert!(
        message.contains("`keep`, `append`, `rewrite`, `fail`"),
        "{message}"
    );
}
