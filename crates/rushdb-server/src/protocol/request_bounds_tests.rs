//! QRY-3 on the wire: a filter without a usable index runs by default, like
//! a PostgreSQL sequential scan; a request may bound it with
//! `context.unindexedWalkMaxOwners` and `context.queryMemoryMaxBytes`, which
//! only tighten the server's ceilings.

use super::*;
use rushdb_protocol::{RequestContext, RequestEnvelope};

fn execute(
    node: &Node,
    context: RequestContext,
    operation: &serde_json::Value,
) -> Result<Response, ProtocolError> {
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context,
            operation: serde_json::from_value(operation.clone()).unwrap(),
        })
        .map(|envelope| envelope.response)
}

fn walk_limited(owners: u64) -> RequestContext {
    RequestContext {
        unindexed_walk_max_owners: Some(owners),
        ..Default::default()
    }
}

fn filtered_read() -> serde_json::Value {
    serde_json::json!({"kind": "query", "query": {"kind": "records", "query": {
        "labels": ["User"], "predicate": {"n": 1}}}})
}

fn sql_count() -> serde_json::Value {
    serde_json::json!({"kind": "query", "query": {"kind": "statement", "language": "sql",
        "statement": "SELECT count(*) AS c FROM \"user\" WHERE n = 1"}})
}

fn node_with_users(configure: impl FnOnce(&mut crate::ServerConfig)) -> Node {
    let mut config = super::tests::config(false);
    config.languages.sql = true;
    configure(&mut config);
    let node = Node::bootstrap(&mut config).unwrap();
    for n in 0..5 {
        execute(
            &node,
            RequestContext::default(),
            &serde_json::json!({"kind": "mutate", "mutation": {"kind": "putRecord",
                "mode": "create", "label": "User", "properties": {"n": n, "email": format!("u{n}")},
                "options": {}}}),
        )
        .unwrap();
    }
    node
}

fn assert_refused_walk(result: Result<Response, ProtocolError>) {
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidRequest, "{error:?}");
    assert!(
        error.message.contains("declare an ordered index"),
        "{error:?}"
    );
}

#[test]
fn an_unindexed_filter_runs_unless_the_request_bounds_it() {
    let node = node_with_users(|_| {});
    // The server sets no limit.
    assert!(execute(&node, RequestContext::default(), &filtered_read()).is_ok());
    assert!(execute(&node, RequestContext::default(), &sql_count()).is_ok());
    // The request refuses walks over 4 records; the label has 5.
    assert_refused_walk(execute(&node, walk_limited(4), &filtered_read()));
    assert_refused_walk(execute(&node, walk_limited(4), &sql_count()));
    assert!(execute(&node, walk_limited(5), &filtered_read()).is_ok());
}

#[test]
fn a_request_cannot_lift_the_servers_walk_limit() {
    let node = node_with_users(|config| config.resources.unindexed_walk_max_owners = 2);
    assert_refused_walk(execute(&node, walk_limited(1_000), &filtered_read()));
}

#[test]
fn a_request_memory_cap_must_be_positive_and_bounds_an_upsert() {
    let node = node_with_users(|_| {});
    let zero = RequestContext {
        query_memory_max_bytes: Some(0),
        ..Default::default()
    };
    let error = execute(&node, zero, &filtered_read()).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidRequest, "{error:?}");

    // An upsert on an unindexed key holds the label's key values.
    let upsert = serde_json::json!({"kind": "mutate", "mutation": {"kind": "putRecord",
        "mode": "create", "label": "User", "properties": {"email": "u1"},
        "options": {"mergeBy": ["email"]}}});
    let tiny = RequestContext {
        query_memory_max_bytes: Some(64),
        ..Default::default()
    };
    let error = execute(&node, tiny, &upsert).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceExhausted, "{error:?}");
    assert!(error.message.contains("`email`"), "{error:?}");
    assert!(execute(&node, RequestContext::default(), &upsert).is_ok());
}
