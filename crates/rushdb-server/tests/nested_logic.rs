//! Logical operators inside nested label blocks (Q-1, found verifying the
//! blog's clinical-ontology example 2026-10-31): `$or`/`$and`/`$not` at any
//! block depth, with or without `$relation`, mean what they mean at the root.

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

/// PT-001 has a diagnosis coded as hypertension; PT-002 one coded as type 2
/// diabetes; PT-003 one coded as asthma.
fn clinic() -> NodeEnvelopeHandler {
    let handler = node();
    let patient = |id: &str, concept: &str| json!({ "id": id, "DIAGNOSIS": { "onset": 2020, "CONCEPT": { "name": concept } } });
    try_call(
        &handler,
        &json!({ "kind": "ingest", "ingest": { "kind": "documents", "label": "PATIENT",
            "documents": [
                patient("PT-001", "Essential hypertension"),
                patient("PT-002", "Type 2 diabetes mellitus"),
                patient("PT-003", "Asthma") ],
            "options": { "relationshipTypes": { "DIAGNOSIS": "HAS_DIAGNOSIS", "CONCEPT": "CODED_AS" } } } }),
    )
    .expect("ingest");
    handler
}

fn ids(handler: &NodeEnvelopeHandler, predicate: &Value) -> Vec<String> {
    let out = try_call(
        handler,
        &json!({ "kind": "query", "query": { "kind": "records", "query": {
            "labels": ["PATIENT"], "predicate": predicate, "order": { "id": "asc" } } } }),
    )
    .unwrap_or_else(|error| panic!("{predicate}: {error}"));
    let mut ids: Vec<String> = out["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|r| {
            r["properties"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    ids.sort();
    ids
}

const T1: &str = "Type 1 diabetes mellitus";
const T2: &str = "Type 2 diabetes mellitus";

#[test]
fn or_two_blocks_deep_matches_like_a_root_or() {
    let handler = clinic();
    let root_or = json!({ "$or": [
        { "DIAGNOSIS": { "CONCEPT": { "name": T1 } } },
        { "DIAGNOSIS": { "CONCEPT": { "name": T2 } } } ] });
    assert_eq!(ids(&handler, &root_or), ["PT-002"]);
    let nested =
        json!({ "DIAGNOSIS": { "CONCEPT": { "$or": [ { "name": T1 }, { "name": T2 } ] } } });
    assert_eq!(ids(&handler, &nested), ["PT-002"]);
    let nested = json!({ "DIAGNOSIS": { "CONCEPT": { "$or": [
        { "name": "Asthma" }, { "name": T2 } ] } } });
    assert_eq!(ids(&handler, &nested), ["PT-002", "PT-003"]);
}

#[test]
fn nested_or_beside_a_relation() {
    let handler = clinic();
    let nested = json!({ "DIAGNOSIS": { "CONCEPT": {
        "$relation": { "type": "CODED_AS", "direction": "out" },
        "$or": [ { "name": T1 }, { "name": T2 } ] } } });
    assert_eq!(ids(&handler, &nested), ["PT-002"]);
}

#[test]
fn nested_and_and_not() {
    let handler = clinic();
    let and = json!({ "DIAGNOSIS": { "CONCEPT": { "$and": [
        { "name": { "$contains": "diabetes" } }, { "name": { "$startsWith": "Type 2" } } ] } } });
    assert_eq!(ids(&handler, &and), ["PT-002"]);
    let not = json!({ "DIAGNOSIS": { "CONCEPT": { "$not": { "name": "Asthma" } } } });
    assert_eq!(ids(&handler, &not), ["PT-001", "PT-002"]);
    let one_level = json!({ "DIAGNOSIS": { "$or": [ { "onset": 1999 }, { "onset": 2020 } ] } });
    assert_eq!(ids(&handler, &one_level), ["PT-001", "PT-002", "PT-003"]);
}
