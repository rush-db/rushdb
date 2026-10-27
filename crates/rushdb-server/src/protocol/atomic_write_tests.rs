//! Multi-step writes commit once (all or nothing): `putRecord` replace,
//! query/ID link and unlink, `retypeRelations` and Mongo `*Many`. Each test
//! caps `max-expanded-commit-bytes` between one item's commit and the whole
//! operation's, so the operation is refused partway through its items and
//! must leave nothing applied.

use super::*;
use crate::resource_config::ByteSize;
use rushdb_protocol::{
    Direction, RelationshipIdsMutation, RelationshipMutation, RelationshipQuery,
    RelationshipSelector, RequestContext, RequestEnvelope, RetypeRelationsMutation,
    StatementLanguage,
};

/// Items per operation; each test's cap fits one item's commit, not all.
const ITEMS: usize = 60;
/// Fits one write of a few hundred bytes, not [`ITEMS`] of them.
const WRITE_CAP: usize = 24 * 1024;
/// Fits one item's setup commit, not [`ITEMS`] relationship deletes.
const DELETE_CAP: usize = 8 * 1024;
/// Fits one small record, not [`ITEMS`] record updates or deletes.
const RECORD_CAP: usize = 4 * 1024;

fn capped_node(cap: usize) -> Node {
    let mut cfg = super::tests::config(false);
    cfg.resources.max_expanded_commit_bytes = ByteSize(cap);
    Node::bootstrap(&mut cfg).unwrap()
}

/// Distinct per item: identical values share one dictionary entry, so they
/// would not grow the commit.
fn filler(i: usize) -> String {
    format!("{i:04}{}", "x".repeat(508))
}

fn properties_of(node: &Node, id: &str) -> serde_json::Value {
    let view = node.store().read_view().unwrap();
    let id = rushdb_types::ids::RecordId(*uuid::Uuid::parse_str(id).unwrap().as_bytes());
    let header = rushdb_lmpg::reader::try_access::record_by_id(&view, node.scope().tenant(), &id)
        .unwrap()
        .unwrap();
    rushdb_lmpg::reconstruct::reconstruct_record(&view, node.scope().tenant(), header.ordinal)
        .unwrap()
        .unwrap()
}

fn mutate(node: &Node, mutation: Mutation) -> Result<Response, ProtocolError> {
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Mutate { mutation },
        })
        .map(|envelope| envelope.response)
}

fn assert_refused_whole(result: Result<Response, ProtocolError>) {
    let error = result.expect_err("an operation over the commit cap is refused");
    assert!(
        error.message.contains("expanded mutation batch exceeds"),
        "refused for the commit size, not something else: {}",
        error.message
    );
}

fn create(node: &Node, label: &str, properties: &serde_json::Value) -> String {
    node.scope()
        .create_record(label, properties, &[], &serde_json::json!({}))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn relationship_count(node: &Node, relation_type: &str) -> usize {
    let rows = node
        .scope()
        .scan_relationships(&RelationshipQuery {
            source_labels: Vec::new(),
            target_labels: Vec::new(),
            relation_type: Some(relation_type.into()),
            predicate: serde_json::json!({}),
            offset: 0,
            source_predicate: None,
            target_predicate: None,
            limit: None,
        })
        .unwrap();
    rows["data"].as_array().map_or(0, Vec::len)
}

fn label_count(node: &Node, label: &str) -> usize {
    let view = node.store().read_view().unwrap();
    super::test_access::label_members(&view, node.scope().tenant(), label).len()
}

/// `ITEMS` sources keyed `k` = 0.., one target keyed `k` = every value.
fn link_fixture(node: &Node) -> (Vec<String>, String) {
    let sources = (0..ITEMS)
        .map(|k| create(node, "Src", &serde_json::json!({"k": k})))
        .collect();
    let keys: Vec<usize> = (0..ITEMS).collect();
    let target = create(node, "Dst", &serde_json::json!({"k": keys}));
    (sources, target)
}

fn query_link(properties: serde_json::Value) -> RelationshipMutation {
    RelationshipMutation {
        relation_type: "HAS".into(),
        source: RelationshipSelector {
            label: "Src".into(),
            property: Some("k".into()),
            predicate: serde_json::json!({}),
        },
        target: RelationshipSelector {
            label: "Dst".into(),
            property: Some("k".into()),
            predicate: serde_json::json!({}),
        },
        direction: Direction::Out,
        allow_cartesian: false,
        properties,
    }
}

/// Links every fixture source to the target one commit at a time.
fn link_one_by_one(node: &Node, sources: &[String], target: &str) {
    for source in sources {
        mutate(
            node,
            Mutation::LinkRecordsById {
                command: RelationshipIdsMutation {
                    relation_type: "HAS".into(),
                    direction: Direction::Out,
                    source_ids: vec![source.clone()],
                    target_ids: vec![target.to_owned()],
                    properties: serde_json::json!({}),
                },
            },
        )
        .unwrap();
    }
    assert_eq!(relationship_count(node, "HAS"), sources.len());
}

#[test]
fn replace_over_the_cap_leaves_the_record_as_it_was() {
    let node = capped_node(WRITE_CAP);
    let id = create(&node, "Doc", &serde_json::json!({"keep": 1, "old": "a"}));
    let big: serde_json::Map<String, serde_json::Value> = (0..ITEMS)
        .map(|i| (format!("p{i}"), serde_json::json!(filler(i))))
        .collect();
    assert_refused_whole(mutate(
        &node,
        Mutation::PutRecord {
            mode: PutMode::Replace,
            id: Some(id.clone()),
            label: None,
            properties: serde_json::Value::Object(big),
            vectors: Vec::new(),
            options: serde_json::json!({}),
        },
    ));
    let record = properties_of(&node, &id);
    assert_eq!(record["keep"], 1, "the old properties were not deleted");
    assert_eq!(record["old"], "a");

    // Under the cap: replaced in one commit, old properties gone.
    let before = node.store().committed_version();
    mutate(
        &node,
        Mutation::PutRecord {
            mode: PutMode::Replace,
            id: Some(id.clone()),
            label: None,
            properties: serde_json::json!({"keep": 2, "new": true}),
            vectors: Vec::new(),
            options: serde_json::json!({}),
        },
    )
    .unwrap();
    assert_eq!(node.store().committed_version(), before + 1, "one commit");
    let record = properties_of(&node, &id);
    assert_eq!(record["keep"], 2);
    assert_eq!(record["new"], true);
    assert!(record.get("old").is_none());
}

#[test]
fn query_link_over_the_cap_creates_nothing() {
    let node = capped_node(WRITE_CAP);
    link_fixture(&node);
    assert_refused_whole(mutate(
        &node,
        Mutation::LinkRecords {
            command: query_link(serde_json::json!({})),
        },
    ));
    assert_eq!(relationship_count(&node, "HAS"), 0);

    // Uncapped: every edge in one commit.
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    link_fixture(&node);
    let before = node.store().committed_version();
    mutate(
        &node,
        Mutation::LinkRecords {
            command: query_link(serde_json::json!({})),
        },
    )
    .unwrap();
    assert_eq!(node.store().committed_version(), before + 1, "one commit");
    assert_eq!(relationship_count(&node, "HAS"), ITEMS);
}

#[test]
fn query_unlink_over_the_cap_deletes_nothing() {
    let node = capped_node(DELETE_CAP);
    let (sources, target) = link_fixture(&node);
    link_one_by_one(&node, &sources, &target);
    assert_refused_whole(mutate(
        &node,
        Mutation::UnlinkRecords {
            command: query_link(serde_json::json!({})),
        },
    ));
    assert_eq!(relationship_count(&node, "HAS"), ITEMS);
}

#[test]
fn unlink_by_id_over_the_cap_deletes_nothing() {
    let node = capped_node(DELETE_CAP);
    let (sources, target) = link_fixture(&node);
    link_one_by_one(&node, &sources, &target);
    assert_refused_whole(mutate(
        &node,
        Mutation::UnlinkRecordsById {
            command: rushdb_protocol::UnlinkRecordsMutation {
                relation_types: vec!["HAS".into()],
                direction: Some(Direction::Out),
                source_ids: sources,
                target_ids: vec![target],
            },
        },
    ));
    assert_eq!(relationship_count(&node, "HAS"), ITEMS);
}

#[test]
fn retype_over_the_cap_changes_nothing() {
    let node = capped_node(DELETE_CAP);
    let (sources, target) = link_fixture(&node);
    link_one_by_one(&node, &sources, &target);
    assert_refused_whole(mutate(
        &node,
        Mutation::RetypeRelations {
            command: RetypeRelationsMutation {
                source_label: "Src".into(),
                target_label: "Dst".into(),
                source_type: "HAS".into(),
                target_type: "OWNS".into(),
                direction: Direction::Out,
            },
        },
    ));
    assert_eq!(relationship_count(&node, "HAS"), ITEMS);
    assert_eq!(relationship_count(&node, "OWNS"), 0);
}

fn mongo(node: &Node, statement: &serde_json::Value) -> Result<Response, ProtocolError> {
    mutate(
        node,
        Mutation::Statement {
            language: StatementLanguage::Mongo,
            statement: statement.to_string(),
            parameters: None,
        },
    )
}

#[test]
fn mongo_insert_many_over_the_cap_inserts_nothing() {
    let node = capped_node(WRITE_CAP);
    let documents: Vec<serde_json::Value> = (0..ITEMS)
        .map(|i| serde_json::json!({"i": i, "body": filler(i)}))
        .collect();
    assert_refused_whole(mongo(
        &node,
        &serde_json::json!({"insertMany": {"collection": "M", "documents": documents}}),
    ));
    assert_eq!(label_count(&node, "M"), 0);
}

#[test]
fn mongo_update_and_delete_many_over_the_cap_change_nothing() {
    let node = capped_node(RECORD_CAP);
    for i in 0..ITEMS {
        create(&node, "M", &serde_json::json!({"i": i, "v": 0}));
    }
    assert_refused_whole(mongo(
        &node,
        &serde_json::json!({"updateMany": {"collection": "M", "filter": {},
            "update": {"$inc": {"v": 1}}}}),
    ));
    let view = node.store().read_view().unwrap();
    let unchanged = super::test_access::label_members(&view, node.scope().tenant(), "M")
        .into_iter()
        .all(|ord| {
            rushdb_lmpg::reconstruct::reconstruct_record(&view, node.scope().tenant(), ord)
                .unwrap()
                .is_some_and(|record| record["v"] == 0)
        });
    assert!(unchanged, "no document was updated");

    assert_refused_whole(mongo(
        &node,
        &serde_json::json!({"deleteMany": {"collection": "M", "filter": {}}}),
    ));
    assert_eq!(label_count(&node, "M"), ITEMS, "no document was deleted");
}
