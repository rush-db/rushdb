//! Request contracts: the query
//! document is exact and `putRecord` honours an explicit id, with
//! merge upserting.

use super::*;
use rushdb_protocol::{PutMode, Query, RequestContext, RequestEnvelope};
use serde_json::json;

fn run(node: &Node, operation: Operation) -> Result<Response, rushdb_protocol::ProtocolError> {
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext::default(),
            operation,
        })
        .map(|executed| executed.response)
}

fn node() -> Node {
    let mut cfg = super::tests::config(false);
    Node::bootstrap(&mut cfg).unwrap()
}

fn put(mode: PutMode, id: Option<&str>, label: Option<&str>, v: i64) -> Operation {
    Operation::Mutate {
        mutation: rushdb_protocol::Mutation::PutRecord {
            mode,
            id: id.map(str::to_string),
            label: label.map(str::to_string),
            properties: json!({"v": v}),
            vectors: Vec::new(),
            options: json!({}),
        },
    }
}

fn records(node: &Node, predicate: serde_json::Value) -> Vec<serde_json::Value> {
    let Ok(Response::Records { records, .. }) = run(
        node,
        Operation::Query {
            query: Query::Records {
                query: QueryDocument {
                    predicate,
                    ..QueryDocument::default()
                },
            },
        },
    ) else {
        panic!("expected records")
    };
    records
}

#[test]
fn an_unknown_query_document_key_is_rejected_by_name() {
    let node = node();
    node.scope()
        .create_record("Doc", &json!({"v": 1}), &[], &json!({}))
        .unwrap();
    let body = json!({
        "protocolVersion": 1,
        "operation": {"kind": "query", "query": {"kind": "records",
            "query": {"labels": ["Doc"], "where": {"v": 2}}}},
    })
    .to_string();
    let Err(crate::ApiError::Protocol(status, error)) =
        crate::protocol_http::execute_cancellable(&node, None, body.as_bytes(), &|| false, None)
    else {
        panic!("an unknown key must not execute")
    };
    assert_eq!(status, 400);
    assert_eq!(error.code, ErrorCode::InvalidRequest);
    assert!(error.message.contains("`where`"), "{}", error.message);
}

#[test]
fn a_predicate_on_a_missing_property_matches_nothing() {
    let node = node();
    node.scope()
        .create_record("Doc", &json!({"v": 1}), &[], &json!({}))
        .unwrap();
    assert_eq!(records(&node, json!({"v": 1})).len(), 1);
    assert!(records(&node, json!({"colour": "red"})).is_empty());
}

#[test]
fn create_honours_an_explicit_id_and_refuses_an_existing_one() {
    let node = node();
    let id = "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
    let Ok(Response::Record { record }) =
        run(&node, put(PutMode::Create, Some(id), Some("Doc"), 1))
    else {
        panic!("create with an id")
    };
    assert_eq!(record["id"], json!(id));
    let duplicate = run(&node, put(PutMode::Create, Some(id), Some("Doc"), 2)).unwrap_err();
    assert_eq!(duplicate.code, ErrorCode::Conflict);
    let invalid = run(
        &node,
        put(PutMode::Create, Some("not-an-id"), Some("Doc"), 3),
    )
    .unwrap_err();
    assert_eq!(invalid.code, ErrorCode::InvalidRequest);
    assert_eq!(records(&node, json!({})).len(), 1);
}

#[test]
fn merge_updates_an_existing_record_and_creates_a_missing_one() {
    let node = node();
    let id = "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5c";
    // Missing and unlabeled: nothing to create it as.
    let unlabeled = run(&node, put(PutMode::Merge, Some(id), None, 1)).unwrap_err();
    assert!(unlabeled.message.contains("label"), "{}", unlabeled.message);
    // Missing with a label: created with that id.
    let Ok(Response::Record { record }) = run(&node, put(PutMode::Merge, Some(id), Some("Doc"), 1))
    else {
        panic!("merge creates")
    };
    assert_eq!(record["id"], json!(id));
    // Present: updated in place.
    let Ok(Response::Record { record }) = run(&node, put(PutMode::Merge, Some(id), Some("Doc"), 2))
    else {
        panic!("merge updates")
    };
    assert_eq!(record["properties"]["v"], json!(2));
    assert_eq!(records(&node, json!({})).len(), 1);
}
