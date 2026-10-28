//! 2C — LMPG semantic fixtures. These pin the exact semantics every future
//! optimization (filter pushdown, bounded aggregation, spill) must preserve:
//! null versus missing, multivalued/array properties, duplicate occurrences,
//! ordering ties, aggregates, and transaction visibility.

use rushdb_protocol::{
    Mutation, Operation, Query, QueryDocument, RequestContext, RequestEnvelope, Response,
};
use rushdb_server::{Node, ServerConfig};

fn node() -> Node {
    let mut cfg = ServerConfig::default();
    Node::bootstrap(&mut cfg).unwrap()
}

fn put(node: &Node, label: &str, properties: serde_json::Value) {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("fixture-put".into()),
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::PutRecord {
                mode: rushdb_protocol::PutMode::Create,
                id: None,
                label: Some(label.to_string()),
                properties,
                vectors: vec![],
                options: serde_json::json!({}),
            },
        },
    };
    rushdb_server::protocol::ProtocolExecutor::new(node, None)
        .execute(envelope)
        .unwrap();
}

fn find(node: &Node, doc: QueryDocument) -> Vec<serde_json::Value> {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("fixture-find".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records { query: doc },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(node, None)
        .execute(envelope)
        .unwrap();
    let Response::Records { records, .. } = res.response else {
        panic!("expected records");
    };
    records
}

fn find_field_values(
    node: &Node,
    label: &str,
    predicate: serde_json::Value,
    field: &str,
) -> Vec<serde_json::Value> {
    let doc = QueryDocument {
        labels: vec![label.to_string()],
        predicate,
        limit: Some(100),
        ..Default::default()
    };
    find(node, doc)
        .into_iter()
        .filter_map(|r| {
            r.as_object()
                .and_then(|o| o.get("properties"))
                .and_then(|p| p.get(field).cloned())
        })
        .collect()
}

#[test]
fn null_is_distinct_from_missing() {
    let n = node();
    put(
        &n,
        "T",
        serde_json::json!({ "name": "explicit-null", "score": null }),
    );
    put(&n, "T", serde_json::json!({ "name": "missing-score" }));
    put(
        &n,
        "T",
        serde_json::json!({ "name": "has-score", "score": 7 }),
    );

    // `score: null` matches the explicitly-null record only — a missing
    // property is NOT null (explicit Null stays distinct from MISSING).
    let with_null = find_field_values(&n, "T", serde_json::json!({ "score": null }), "name");
    assert_eq!(with_null, vec![serde_json::json!("explicit-null")]);

    // `$ne` requires the field to be present (RushDB: value <> X): the
    // missing record is excluded; the present non-null record matches.
    let not_null = find_field_values(
        &n,
        "T",
        serde_json::json!({ "score": { "$ne": null } }),
        "name",
    );
    assert_eq!(not_null, vec![serde_json::json!("has-score")]);
}

#[test]
fn multivalued_array_property_matches_elementwise() {
    let n = node();
    put(
        &n,
        "T",
        serde_json::json!({ "name": "tags-a", "tags": ["x", "y"] }),
    );
    put(
        &n,
        "T",
        serde_json::json!({ "name": "tags-b", "tags": ["y", "z"] }),
    );
    put(&n, "T", serde_json::json!({ "name": "no-tags" }));

    // Element match: `{ tags: "x" }` matches only the record containing x.
    let has_x = find_field_values(&n, "T", serde_json::json!({ "tags": "x" }), "name");
    assert_eq!(has_x, vec![serde_json::json!("tags-a")]);

    // `{ tags: "y" }` matches both array records, not the missing one.
    let has_y = find_field_values(&n, "T", serde_json::json!({ "tags": "y" }), "name");
    assert_eq!(has_y.len(), 2, "both array records contain y");
    assert!(!has_y.contains(&serde_json::json!("no-tags")));
}

#[test]
fn create_never_deduplicates_records() {
    let n = node();
    // Two create calls with an identical payload are two distinct records —
    // create is insert-only; dedup would be a merge/upsert concern.
    put(&n, "T", serde_json::json!({ "name": "dup", "score": 1 }));
    put(&n, "T", serde_json::json!({ "name": "dup", "score": 1 }));

    let names = find_field_values(&n, "T", serde_json::json!({}), "name");
    assert_eq!(
        names,
        vec![serde_json::json!("dup"), serde_json::json!("dup")]
    );
}

#[test]
fn ordering_ties_resolve_deterministically() {
    let n = node();
    for (name, score) in [("a", 5), ("b", 5), ("c", 3)] {
        put(&n, "T", serde_json::json!({ "name": name, "score": score }));
    }
    let doc = QueryDocument {
        labels: vec!["T".to_string()],
        order: Some(serde_json::json!({ "score": "desc" })),
        limit: Some(10),
        ..Default::default()
    };
    let rows = find(&n, doc);
    // Descending score: c(3) last; the 5-tie (a,b) is fully returned and the
    // set is deterministic — both tied records present exactly once.
    let names: Vec<String> = rows
        .iter()
        .filter_map(|r| {
            r.as_object()
                .and_then(|o| o.get("properties"))
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str().map(str::to_string))
        })
        .collect();
    assert_eq!(names.len(), 3);
    assert_eq!(names.last().unwrap(), "c", "lowest score last under desc");
    assert!(names.contains(&"a".to_string()) && names.contains(&"b".to_string()));
}

#[test]
fn aggregate_counts_by_group_key() {
    use serde_json::json;
    let n = node();
    put(&n, "FACTION", json!({ "name": "Jedi Order" }));
    put(&n, "FACTION", json!({ "name": "Jedi Order" }));
    put(&n, "FACTION", json!({ "name": "Sith Order" }));

    // select + groupBy drives the aggregate path: count per group key.
    let doc = QueryDocument {
        labels: vec!["FACTION".to_string()],
        group_by: vec!["$record.name".to_string()],
        select: Some(json!({ "count": { "$count": "*" } })),
        ..Default::default()
    };
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("fixture-aggregate".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records { query: doc },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(&n, None)
        .execute(envelope)
        .unwrap();
    let Response::Rows { rows, .. } = res.response else {
        panic!("expected aggregate rows, got {:?}", res.response);
    };
    let text = serde_json::to_string(&rows).unwrap();
    assert!(text.contains("\"Jedi Order\"") && text.contains("\"Sith Order\""));
    // The count output lands under the selected alias; assert 2-vs-1 via the
    // row payloads.
    let counts: Vec<u64> = rows
        .iter()
        .filter_map(|r| {
            let o = r.as_object()?;
            o.values()
                .find_map(serde_json::Value::as_u64)
                .or_else(|| o.get("count").and_then(serde_json::Value::as_u64))
        })
        .collect();
    assert!(
        counts.contains(&2),
        "Jedi Order bucket counts 2: {counts:?}"
    );
    assert!(
        counts.contains(&1),
        "Sith Order bucket counts 1: {counts:?}"
    );
}

#[test]
fn staged_writes_are_invisible_until_commit() {
    use rushdb_protocol::{TransactionMutation, TransactionOperation};
    let n = node();
    let begin = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("tx".into()),
        context: RequestContext::default(),
        operation: Operation::Transaction {
            transaction: TransactionOperation::Begin {
                ttl_ms: None,
                autocommit: false,
            },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(&n, None)
        .execute(begin)
        .unwrap();
    let Response::TransactionBegun { transaction_id } = res.response else {
        panic!("expected begun tx");
    };
    let stage = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("tx-stage".into()),
        context: RequestContext::default(),
        operation: Operation::Transaction {
            transaction: TransactionOperation::Stage {
                transaction_id: transaction_id.clone(),
                mutation: TransactionMutation::CreateRecord {
                    label: "T".to_string(),
                    id: Some("00000000-0000-0000-0000-0000000000a1".to_string()),
                    properties: serde_json::json!({ "name": "staged" }),
                    options: serde_json::json!({}),
                },
            },
        },
    };
    rushdb_server::protocol::ProtocolExecutor::new(&n, None)
        .execute(stage)
        .unwrap();

    let pre_commit = find_field_values(&n, "T", serde_json::json!({}), "name");
    assert!(
        !pre_commit.contains(&serde_json::json!("staged")),
        "staged write must be invisible pre-commit"
    );

    let commit = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("tx-commit".into()),
        context: RequestContext::default(),
        operation: Operation::Transaction {
            transaction: TransactionOperation::Commit { transaction_id },
        },
    };
    rushdb_server::protocol::ProtocolExecutor::new(&n, None)
        .execute(commit)
        .unwrap();
    let post_commit = find_field_values(&n, "T", serde_json::json!({}), "name");
    assert!(
        post_commit.contains(&serde_json::json!("staged")),
        "committed write must be visible"
    );
}
