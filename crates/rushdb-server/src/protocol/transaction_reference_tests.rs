//! Transaction stages name existing records by public id alone: the engine
//! resolves each id inside the transaction, so committed records and records
//! staged earlier in the same transaction can be updated, deleted and linked.

use super::*;

pub(super) fn run(
    node: &Node,
    operation: &serde_json::Value,
) -> Result<serde_json::Value, ProtocolError> {
    let envelope: RequestEnvelope = serde_json::from_value(serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "operation": operation,
    }))
    .unwrap();
    ProtocolExecutor::new(node, None)
        .execute(envelope)
        .map(|envelope| serde_json::to_value(envelope.response).unwrap())
}

fn begin(node: &Node) -> String {
    let begun = run(
        node,
        &serde_json::json!({"kind": "transaction", "transaction": {"kind": "begin"}}),
    )
    .unwrap();
    begun["transactionId"].as_str().unwrap().to_owned()
}

fn stage(
    node: &Node,
    tx: &str,
    mutation: &serde_json::Value,
) -> Result<serde_json::Value, ProtocolError> {
    run(
        node,
        &serde_json::json!({"kind": "transaction", "transaction": {
            "kind": "stage", "transactionId": tx, "mutation": mutation,
        }}),
    )
}

fn commit(node: &Node, tx: &str) -> serde_json::Value {
    run(
        node,
        &serde_json::json!({"kind": "transaction", "transaction": {
            "kind": "commit", "transactionId": tx,
        }}),
    )
    .unwrap()
}

fn record(node: &Node, id: &str) -> Option<serde_json::Value> {
    run(
        node,
        &serde_json::json!({"kind": "query", "query": {"kind": "record", "id": id}}),
    )
    .ok()
    .and_then(|response| response.get("record").cloned())
    .filter(|record| !record.is_null())
}

fn links(node: &Node, relation_type: &str) -> usize {
    let response = run(
        node,
        &serde_json::json!({"kind": "query", "query": {"kind": "relationships", "query": {
            "relationType": relation_type,
        }}}),
    )
    .unwrap();
    response["relationships"].as_array().unwrap().len()
}

fn committed_person(node: &Node, name: &str) -> String {
    let created = node
        .scope()
        .create_record(
            "Person",
            &serde_json::json!({ "name": name }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap();
    created["id"].as_str().unwrap().to_owned()
}

const STAGED: &str = "00000000-0000-0000-0000-0000000000aa";

#[test]
fn update_delete_and_link_committed_and_staged_records_by_id() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = committed_person(&node, "Ada");
    let alan = committed_person(&node, "Alan");
    let tx = begin(&node);

    stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "createRecord", "label": "Person", "id": STAGED,
            "properties": {"name": "Grace"}}),
    )
    .unwrap();
    stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "updateRecord", "record": {"id": ada},
            "properties": {"name": "Ada Lovelace"}}),
    )
    .unwrap();
    stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "createRelationship", "relationType": "KNOWS",
            "source": {"id": ada}, "target": {"id": STAGED}}),
    )
    .unwrap();
    stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "deleteRecord", "record": {"id": alan}}),
    )
    .unwrap();
    let read = stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "getRecord", "record": {"id": ada}}),
    )
    .unwrap();
    assert_eq!(read["record"]["properties"]["name"], "Ada Lovelace");

    // Nothing is visible before commit.
    assert_eq!(record(&node, &ada).unwrap()["properties"]["name"], "Ada");
    assert!(record(&node, &alan).is_some());
    assert_eq!(links(&node, "KNOWS"), 0);

    let outcome = commit(&node, &tx);
    assert_eq!(outcome["status"], "committed", "{outcome}");
    assert_eq!(
        record(&node, &ada).unwrap()["properties"]["name"],
        "Ada Lovelace"
    );
    assert_eq!(
        record(&node, STAGED).unwrap()["properties"]["name"],
        "Grace"
    );
    assert!(record(&node, &alan).is_none());
    assert_eq!(links(&node, "KNOWS"), 1);
}

#[test]
fn unknown_record_id_fails_the_stage_and_leaves_the_transaction_usable() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = committed_person(&node, "Ada");
    let tx = begin(&node);

    let missing = stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "updateRecord",
            "record": {"id": "00000000-0000-0000-0000-0000000000ff"},
            "properties": {"name": "nobody"}}),
    )
    .unwrap_err();
    assert!(
        missing
            .message
            .contains("record not found: 00000000-0000-0000-0000-0000000000ff"),
        "{}",
        missing.message
    );

    stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "updateRecord", "record": {"id": ada},
            "properties": {"name": "Ada Lovelace"}}),
    )
    .unwrap();
    let outcome = commit(&node, &tx);
    assert_eq!(outcome["status"], "committed", "{outcome}");
    assert_eq!(
        record(&node, &ada).unwrap()["properties"]["name"],
        "Ada Lovelace"
    );
}

#[test]
fn a_record_reference_takes_only_an_id() {
    let parsed =
        serde_json::from_value::<rushdb_protocol::TransactionMutation>(serde_json::json!({
            "kind": "deleteRecord",
            "record": {"id": STAGED, "ordinal": 0},
        }));
    let error = parsed.unwrap_err().to_string();
    assert!(error.contains("unknown field `ordinal`"), "{error}");
}

#[test]
fn staged_writes_type_values_like_put_record_and_replace_deletes_unnamed() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = committed_person(&node, "Ada");
    let tx = begin(&node);
    stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "createRecord", "label": "Person", "id": STAGED,
            "properties": {"name": "Grace", "born": "1906-12-09"},
            "options": {"suggestTypes": true}}),
    )
    .unwrap();
    stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "updateRecord", "record": {"id": ada},
            "properties": {"city": "London"}}),
    )
    .unwrap();
    stage(
        &node,
        &tx,
        &serde_json::json!({"kind": "updateRecord", "record": {"id": ada}, "replace": true,
            "properties": {"title": "Countess"}}),
    )
    .unwrap();
    let outcome = commit(&node, &tx);
    assert_eq!(outcome["status"], "committed", "{outcome}");

    let grace = record(&node, STAGED).unwrap();
    assert_eq!(grace["propertyTypes"]["born"], "datetime", "{grace}");
    let ada = record(&node, &ada).unwrap();
    assert_eq!(ada["properties"]["title"], "Countess");
    let properties = &ada["properties"];
    assert!(
        properties.get("name").is_none() && properties.get("city").is_none(),
        "{ada}"
    );
}
