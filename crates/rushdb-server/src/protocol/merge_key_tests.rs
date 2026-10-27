//! MERGE-1 (b) on the wire: a reserved (`__`) property name or `mergeBy` key, and
//! an upsert on a key without an index over `unindexed-walk-max-owners`, are
//! the client's to fix: `invalid_request`, not retryable.

use super::*;
use rushdb_protocol::{RequestContext, RequestEnvelope};

fn put(node: &Node, mutation: &serde_json::Value) -> Result<Response, ProtocolError> {
    let operation: Operation =
        serde_json::from_value(serde_json::json!({ "kind": "mutate", "mutation": mutation }))
            .unwrap();
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext {
                scope: None,
                deadline_ms: None,
                ..Default::default()
            },
            operation,
        })
        .map(|envelope| envelope.response)
}

fn create(properties: &serde_json::Value, merge_by: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "kind": "putRecord", "mode": "create", "label": "User",
        "properties": properties, "options": { "mergeBy": merge_by }
    })
}

fn assert_invalid(error: &ProtocolError, expected: &str) {
    assert_eq!(error.code, ErrorCode::InvalidRequest, "{error:?}");
    assert!(!error.retryable, "{error:?}");
    assert!(error.message.contains(expected), "{error:?}");
}

#[test]
fn reserved_names_and_unindexed_upserts_over_the_limit_are_invalid_requests() {
    let mut config = super::tests::config(false);
    config.resources.unindexed_walk_max_owners = 2;
    let node = Node::bootstrap(&mut config).unwrap();

    for name in ["__score", "__label"] {
        let error = put(&node, &create(&serde_json::json!({ name: 1 }), &[])).unwrap_err();
        assert_invalid(&error, "invalid property name");
    }
    let error = put(&node, &create(&serde_json::json!({ "a": 1 }), &["__a"])).unwrap_err();
    assert_invalid(&error, "invalid merge key");

    for i in 0..3 {
        put(
            &node,
            &create(&serde_json::json!({ "email": format!("u{i}") }), &[]),
        )
        .unwrap();
    }
    let error = put(
        &node,
        &create(&serde_json::json!({ "email": "u1" }), &["email"]),
    )
    .unwrap_err();
    assert_invalid(&error, "declare an ordered index on `email`");
}

#[test]
fn a_user_underscore_id_property_never_replaces_the_record_id() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let Response::Record { record } = put(
        &node,
        &create(&serde_json::json!({ "_id": "ext-1", "name": "Ada" }), &[]),
    )
    .unwrap() else {
        panic!("expected a record");
    };
    assert!(
        uuid::Uuid::parse_str(record["id"].as_str().unwrap()).is_ok(),
        "{record}"
    );
    assert_eq!(record["properties"]["_id"], "ext-1", "{record}");
    assert_eq!(record["propertyTypes"]["_id"], "string", "{record}");
}
