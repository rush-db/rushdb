//! #148: a unique index refuses a second record of its label holding a
//! value, with a 409 `conflict` naming the label, the property and the
//! record holding it, on every write path; it is refused on data that
//! already repeats a value.

use super::transaction_reference_tests::run;
use super::*;
use serde_json::{json, Value};

fn node() -> Node {
    let mut cfg = super::tests::config(false);
    cfg.languages = rushdb_protocol::LanguageSet {
        sql: true,
        gql: true,
        cypher: true,
        gremlin: true,
        sql_pgq: true,
        mongodb: true,
    };
    Node::bootstrap(&mut cfg).unwrap()
}

fn unique(node: &Node, label: &str, property: &str) -> Result<Value, ProtocolError> {
    let created = run(
        node,
        &json!({"kind": "index", "index": {"kind": "create", "definition": {
            "name": format!("{label}_{property}"), "label": label, "property": property,
            "kind": "unique"}}}),
    )?;
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
    Ok(created)
}

fn create(node: &Node, label: &str, properties: &Value) -> Result<String, ProtocolError> {
    let created = run(
        node,
        &json!({"kind": "mutate", "mutation": {"kind": "putRecord", "mode": "create",
            "label": label, "properties": properties}}),
    )?;
    Ok(created["record"]["id"].as_str().unwrap().to_owned())
}

fn assert_unique_conflict(error: &ProtocolError, label: &str, property: &str, holder: &str) {
    assert_eq!(
        error.code,
        rushdb_protocol::ErrorCode::Conflict,
        "{error:?}"
    );
    assert_eq!(
        error.details,
        Some(json!({"label": label, "property": property, "recordId": holder})),
        "{error:?}"
    );
    assert!(
        !error.message.contains("a@x"),
        "the value is not told: {error:?}"
    );
}

#[test]
fn a_unique_index_refuses_a_second_holder_and_releases_on_change() {
    let node = node();
    unique(&node, "User", "email").unwrap();
    let ada = create(&node, "User", &json!({"email": "a@x"})).unwrap();
    let refused = create(&node, "User", &json!({"email": "a@x"})).unwrap_err();
    assert_unique_conflict(&refused, "User", "email", &ada);
    // Another label may hold it.
    create(&node, "Admin", &json!({"email": "a@x"})).unwrap();
    // Rewriting the same value keeps it; changing it releases the old one.
    run(
        &node,
        &json!({"kind": "mutate", "mutation": {"kind": "putRecord", "mode": "merge",
            "label": "User", "id": ada, "properties": {"email": "a@x", "n": 1}}}),
    )
    .unwrap();
    run(
        &node,
        &json!({"kind": "mutate", "mutation": {"kind": "putRecord", "mode": "merge",
            "label": "User", "id": ada, "properties": {"email": "b@x"}}}),
    )
    .unwrap();
    let grace = create(&node, "User", &json!({"email": "a@x"})).unwrap();
    // A deleted record releases its values.
    run(
        &node,
        &json!({"kind": "mutate", "mutation": {"kind": "deleteRecord", "id": ada}}),
    )
    .unwrap();
    create(&node, "User", &json!({"email": "b@x"})).unwrap();
    // Statement languages and ingest meet the same rule.
    for (kind, statement) in [
        ("sql", "INSERT INTO \"User\" (email) VALUES ('a@x')"),
        ("cypher", "CREATE (:User {email: 'a@x'})"),
        (
            "mongo",
            r#"{"insertOne": {"collection": "User", "document": {"email": "a@x"}}}"#,
        ),
    ] {
        let error = run(
            &node,
            &json!({"kind": "mutate", "mutation": {"kind": "statement", "language": kind,
                "statement": statement}}),
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            rushdb_protocol::ErrorCode::Conflict,
            "{kind}: {error:?}"
        );
    }
    let ingest = run(
        &node,
        &json!({"kind": "ingest", "ingest": {"kind": "documents", "label": "User",
            "documents": [{"email": "c@x"}, {"email": "c@x"}]}}),
    )
    .unwrap_err();
    assert_eq!(
        ingest.code,
        rushdb_protocol::ErrorCode::Conflict,
        "{ingest:?}"
    );
    let _ = grace;
}

#[test]
#[expect(
    clippy::needless_collect,
    reason = "every writer starts before any is joined"
)]
fn of_concurrent_writers_of_one_value_exactly_one_commits() {
    let node = std::sync::Arc::new(node());
    unique(&node, "User", "email").unwrap();
    let writers: Vec<_> = (0..8)
        .map(|_| {
            let node = std::sync::Arc::clone(&node);
            std::thread::spawn(move || create(&node, "User", &json!({"email": "same@x"})))
        })
        .collect();
    let outcomes: Vec<_> = writers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(
        outcomes.iter().filter(|o| o.is_ok()).count(),
        1,
        "{outcomes:?}"
    );
    for refused in outcomes.iter().filter_map(|o| o.as_ref().err()) {
        assert_eq!(
            refused.code,
            rushdb_protocol::ErrorCode::Conflict,
            "{refused:?}"
        );
    }
}

#[test]
fn a_unique_index_on_repeated_values_is_refused_and_dropping_frees_them() {
    let node = node();
    let first = create(&node, "Dup", &json!({"code": "X"})).unwrap();
    let second = create(&node, "Dup", &json!({"code": "X"})).unwrap();
    let refused = unique(&node, "Dup", "code").unwrap_err();
    assert_eq!(
        refused.code,
        rushdb_protocol::ErrorCode::Conflict,
        "{refused:?}"
    );
    assert!(
        refused.message.contains(&first) && refused.message.contains(&second),
        "{refused:?}"
    );
    // On distinct values it builds; dropping it lets a value repeat again.
    create(&node, "Solo", &json!({"code": "Y"})).unwrap();
    unique(&node, "Solo", "code").unwrap();
    assert!(create(&node, "Solo", &json!({"code": "Y"})).is_err());
    run(
        &node,
        &json!({"kind": "index", "index": {"kind": "drop", "name": "Solo_code"}}),
    )
    .unwrap();
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
    create(&node, "Solo", &json!({"code": "Y"})).unwrap();
}

/// Each statement language declares a unique index: SQL `CREATE UNIQUE
/// INDEX`, Cypher `CREATE CONSTRAINT … REQUIRE n.p IS UNIQUE`, Mongo
/// `createIndex` with `"unique": true`.
#[test]
fn statement_languages_declare_unique_indexes() {
    let node = node();
    for (label, language, statement) in [
        (
            "Sku",
            "sql",
            "CREATE UNIQUE INDEX sku_code ON \"Sku\" (code)",
        ),
        (
            "Part",
            "cypher",
            "CREATE CONSTRAINT part_code FOR (p:Part) REQUIRE p.code IS UNIQUE",
        ),
        (
            "Item",
            "mongo",
            r#"{"createIndex": {"collection": "Item", "property": "code", "unique": true}}"#,
        ),
    ] {
        create(&node, label, &json!({"code": "A"})).unwrap();
        run(
            &node,
            &json!({"kind": "mutate", "mutation": {"kind": "statement", "language": language,
                "statement": statement}}),
        )
        .unwrap_or_else(|error| panic!("{language}: {error:?}"));
        while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
        let refused = create(&node, label, &json!({"code": "A"})).unwrap_err();
        assert_eq!(
            refused.code,
            rushdb_protocol::ErrorCode::Conflict,
            "{language}"
        );
    }
}

/// A transaction meets the rule too, at the latest when it commits.
#[test]
fn a_transaction_writing_a_held_value_does_not_commit() {
    let node = node();
    unique(&node, "User", "email").unwrap();
    create(&node, "User", &json!({"email": "a@x"})).unwrap();
    let begun = run(
        &node,
        &json!({"kind": "transaction", "transaction": {"kind": "begin"}}),
    )
    .unwrap();
    let tx = begun["transactionId"].as_str().unwrap().to_owned();
    let staged = run(
        &node,
        &json!({"kind": "transaction", "transaction": {"kind": "stage", "transactionId": tx,
            "mutation": {"kind": "createRecord", "label": "User",
                "properties": {"email": "a@x"}}}}),
    );
    let outcome = staged.and_then(|_| {
        run(
            &node,
            &json!({"kind": "transaction", "transaction": {"kind": "commit", "transactionId": tx}}),
        )
    });
    let error = outcome.unwrap_err();
    assert_eq!(
        error.code,
        rushdb_protocol::ErrorCode::Conflict,
        "{error:?}"
    );
    assert!(
        error
            .message
            .starts_with("unique constraint conflict on `User`.`email`"),
        "{error:?}"
    );
    let users = run(
        &node,
        &json!({"kind": "query", "query": {"kind": "records", "query": {"labels": ["User"]}}}),
    )
    .unwrap();
    assert_eq!(users["total"], json!(1), "{users}");
}

/// A unique index claims every stored value when it is declared: a write
/// sent while its build is still covering the label's records is refused,
/// and the index builds.
#[test]
fn a_write_during_a_multi_batch_build_is_refused_and_the_index_stays() {
    let node = node();
    let rows: Vec<Value> = (0..2_500)
        .map(|n| json!({"email": format!("u{n}@x")}))
        .collect();
    run(
        &node,
        &json!({"kind": "ingest", "ingest": {"kind": "documents", "label": "Bulk", "documents": rows}}),
    )
    .unwrap();
    let holder = create(&node, "Bulk", &json!({"email": "a@x"})).unwrap();
    run(
        &node,
        &json!({"kind": "index", "index": {"kind": "create", "definition": {
            "name": "Bulk_email", "label": "Bulk", "property": "email", "kind": "unique"}}}),
    )
    .unwrap();
    let state = |node: &Node| {
        let listed = run(
            node,
            &json!({"kind": "describe", "describe": {"kind": "indexes"}}),
        )
        .unwrap();
        listed["indexes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|index| index["name"] == "Bulk_email")
            .map(|index| index["state"].as_str().unwrap().to_owned())
    };
    assert_eq!(state(&node).as_deref(), Some("building"));
    let refused = create(&node, "Bulk", &json!({"email": "a@x"})).unwrap_err();
    assert_unique_conflict(&refused, "Bulk", "email", &holder);
    let refused = create(&node, "Bulk", &json!({"email": "u7@x"})).unwrap_err();
    assert_eq!(
        refused.code,
        rushdb_protocol::ErrorCode::Conflict,
        "{refused:?}"
    );
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
    assert_eq!(state(&node).as_deref(), Some("ready"));
    assert!(create(&node, "Bulk", &json!({"email": "a@x"})).is_err());
    create(&node, "Bulk", &json!({"email": "new@x"})).unwrap();
}

/// A value longer than the value dictionary holds (96 bytes) that repeats
/// is found when the index is declared, not by its build.
#[test]
fn a_repeated_long_value_is_refused_at_declaration() {
    let node = node();
    let long = "x".repeat(200);
    create(&node, "Long", &json!({"code": long})).unwrap();
    let second = create(&node, "Long", &json!({"code": long})).unwrap();
    let refused = unique(&node, "Long", "code").unwrap_err();
    assert_eq!(
        refused.code,
        rushdb_protocol::ErrorCode::Conflict,
        "{refused:?}"
    );
    assert!(refused.message.contains(&second), "{refused:?}");
    assert!(!refused.message.contains(&long), "{refused:?}");
    let listed = run(
        &node,
        &json!({"kind": "describe", "describe": {"kind": "indexes"}}),
    )
    .unwrap();
    assert!(listed["indexes"].as_array().unwrap().is_empty(), "{listed}");
}
