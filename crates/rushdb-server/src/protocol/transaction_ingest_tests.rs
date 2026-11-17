//! An import staged into a transaction (JSON documents or CSV) creates its
//! records and relationships on commit, and not before.

use super::transaction_reference_tests::run;
use super::*;

fn begin(node: &Node) -> String {
    let begun = run(
        node,
        &serde_json::json!({"kind": "transaction", "transaction": {"kind": "begin"}}),
    )
    .unwrap();
    begun["transactionId"].as_str().unwrap().to_owned()
}

fn transaction(node: &Node, tx: &str, kind: &str, extra: &serde_json::Value) -> serde_json::Value {
    let mut transaction = serde_json::json!({"kind": kind, "transactionId": tx});
    transaction
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    run(
        node,
        &serde_json::json!({"kind": "transaction", "transaction": transaction}),
    )
    .unwrap()
}

fn stage_ingest(
    node: &Node,
    tx: &str,
    ingest: &serde_json::Value,
) -> Result<serde_json::Value, ProtocolError> {
    run(
        node,
        &serde_json::json!({"kind": "transaction", "transaction": {
            "kind": "stage", "transactionId": tx,
            "mutation": {"kind": "ingest", "ingest": ingest},
        }}),
    )
}

fn count(node: &Node, label: &str) -> usize {
    let response = run(
        node,
        &serde_json::json!({"kind": "query", "query": {"kind": "records", "query": {"labels": [label]}}}),
    )
    .unwrap();
    response["records"].as_array().unwrap().len()
}

fn links(node: &Node) -> usize {
    let response = run(
        node,
        &serde_json::json!({"kind": "query", "query": {"kind": "relationships", "query": {}}}),
    )
    .unwrap();
    response["relationships"].as_array().unwrap().len()
}

#[test]
fn a_staged_json_import_lands_on_commit_with_its_relationships() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let tx = begin(&node);
    let staged = stage_ingest(
        &node,
        &tx,
        &serde_json::json!({"kind": "documents", "label": "Author", "documents": [
            {"name": "Herbert", "book": {"title": "Dune"}},
            {"name": "Le Guin", "book": [{"title": "Earthsea"}, {"title": "Lathe"}]},
        ]}),
    )
    .unwrap();
    assert_eq!(staged["kind"], "transactionIngested");
    assert_eq!(staged["count"], 5);
    assert_eq!(staged["relationships"], 3);
    assert_eq!(count(&node, "Author"), 0);
    assert_eq!(links(&node), 0);

    assert_eq!(
        transaction(&node, &tx, "commit", &serde_json::json!({}))["status"],
        "committed"
    );
    assert_eq!(count(&node, "Author"), 2);
    assert_eq!(count(&node, "book"), 3);
    assert_eq!(links(&node), 3);
}

#[test]
fn a_staged_csv_import_rolls_back_with_its_transaction() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let tx = begin(&node);
    let staged = stage_ingest(
        &node,
        &tx,
        &serde_json::json!({"kind": "csv", "label": "Book", "data": "title,pages\nDune,412\nEmma,474\n",
            "options": {"suggestTypes": true}}),
    )
    .unwrap();
    assert_eq!(staged["count"], 2);
    transaction(&node, &tx, "rollback", &serde_json::json!({}));
    assert_eq!(count(&node, "Book"), 0);
}

fn ingest(node: &Node, ingest: &serde_json::Value) {
    run(
        node,
        &serde_json::json!({"kind": "ingest", "ingest": ingest}),
    )
    .unwrap();
}

fn merge_books(documents: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({"kind": "documents", "label": "Book", "documents": documents,
        "options": {"mergeBy": ["title"]}})
}

#[test]
fn a_staged_merge_matches_committed_and_staged_records() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    ingest(
        &node,
        &merge_books(&serde_json::json!([{"title": "Dune", "pages": 1}])),
    );
    let tx = begin(&node);
    // Updates the committed Dune, creates Emma.
    let staged = stage_ingest(
        &node,
        &tx,
        &merge_books(&serde_json::json!([
            {"title": "Dune", "pages": 412, "author": {"name": "Herbert"}},
            {"title": "Emma", "pages": 474},
        ])),
    )
    .unwrap();
    assert_eq!(staged["count"], 3);
    assert_eq!(staged["relationships"], 1);
    // Matches the Emma this transaction staged (read-your-writes).
    stage_ingest(
        &node,
        &tx,
        &merge_books(&serde_json::json!([{"title": "Emma", "year": 1815}])),
    )
    .unwrap();
    assert_eq!(count(&node, "Book"), 1, "nothing staged is visible yet");
    assert_eq!(
        transaction(&node, &tx, "commit", &serde_json::json!({}))["status"],
        "committed"
    );
    assert_eq!(count(&node, "Book"), 2);
    assert_eq!(count(&node, "author"), 1);
    assert_eq!(links(&node), 1);
    let emma = run(
        &node,
        &serde_json::json!({"kind": "query", "query": {"kind": "records", "query": {
            "labels": ["Book"], "predicate": {"title": "Emma"}}}}),
    )
    .unwrap();
    assert_eq!(emma["records"][0]["properties"]["pages"], 474);
    assert_eq!(emma["records"][0]["properties"]["year"], 1815);
}

#[test]
fn a_staged_merge_rolls_back_with_its_transaction() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let tx = begin(&node);
    stage_ingest(
        &node,
        &tx,
        &merge_books(&serde_json::json!([{"title": "Dune", "author": {"name": "Herbert"}}])),
    )
    .unwrap();
    transaction(&node, &tx, "rollback", &serde_json::json!({}));
    assert_eq!(count(&node, "Book"), 0);
    assert_eq!(count(&node, "author"), 0);
    assert_eq!(links(&node), 0);
}

#[test]
fn a_staged_merge_conflicts_with_a_concurrent_merge_of_its_keys() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let tx = begin(&node);
    stage_ingest(
        &node,
        &tx,
        &merge_books(&serde_json::json!([{"title": "Dune", "pages": 1}])),
    )
    .unwrap();
    // Another writer creates the same key after the transaction looked.
    ingest(
        &node,
        &merge_books(&serde_json::json!([{"title": "Dune", "pages": 2}])),
    );
    let outcome = transaction(&node, &tx, "commit", &serde_json::json!({}));
    assert_eq!(outcome["status"], "aborted", "{outcome}");
    assert_eq!(outcome["retryable"], true, "{outcome}");
    assert_eq!(count(&node, "Book"), 1, "the key exists once");
}

#[test]
fn inline_vectors_are_not_staged() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let tx = begin(&node);
    let vectors = stage_ingest(
        &node,
        &tx,
        &serde_json::json!({"kind": "documents", "label": "Book", "documents": [
            {"title": "Dune", "$vectors": [{"propertyName": "title", "vector": [1, 0]}]},
        ]}),
    )
    .unwrap_err();
    assert_eq!(vectors.code, ErrorCode::Unsupported);
    assert!(
        vectors.message.contains("inline vectors"),
        "{}",
        vectors.message
    );
    assert_eq!(
        transaction(&node, &tx, "commit", &serde_json::json!({}))["status"],
        "committed"
    );
    assert_eq!(count(&node, "Book"), 0);
}

#[test]
fn a_merge_call_over_the_commit_cap_is_refused_whole_and_not_retryable() {
    let mut cfg = super::tests::config(false);
    cfg.resources.max_expanded_commit_bytes = crate::resource_config::ByteSize(64 * 1024);
    let node = Node::bootstrap(&mut cfg).unwrap();
    // Far more than 64 KiB once stored: many records, each with a nested
    // author and a relationship.
    let books: Vec<serde_json::Value> = (0..400)
        .map(|i| serde_json::json!({"title": format!("Book {i}"), "author": {"name": format!("A{i}")}}))
        .collect();
    let error = run(
        &node,
        &serde_json::json!({"kind": "ingest", "ingest": merge_books(&serde_json::json!(books))}),
    )
    .expect_err("over the commit cap");
    assert_eq!(
        error.code,
        rushdb_protocol::ErrorCode::ResourceExhausted,
        "{}",
        error.message
    );
    assert!(
        !error.retryable,
        "the same request fails the same way: split it"
    );
    assert!(
        error.message.contains("expanded mutation batch exceeds"),
        "{}",
        error.message
    );
    assert_eq!(
        count(&node, "Book"),
        0,
        "nothing of a refused merge is written"
    );
    assert_eq!(count(&node, "author"), 0);
}
