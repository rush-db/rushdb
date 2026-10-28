//! The Gremlin traversal profile (`conformance/gremlin-profile-v1.json`):
//! every traversal runs on the engine with the expected row count or
//! effect, and every bytecode case translates to its pinned traversal. CI
//! (`gremlin-grammar`) checks the same traversals against the official
//! `TinkerPop` grammar.

use std::sync::Arc;

use rushdb_language::{execute, execute_mutation, gremlin_bytecode, QueryLanguage};
use rushdb_lmpg::{plan::FieldChange, Lmpg, RecordRef};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

const PROFILE: &str = include_str!("../conformance/gremlin-profile-v1.json");

/// The profile's fixture graph.
fn fixture() -> Lmpg {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(9));
    let person = |name: &str, age: i64| -> RecordRef {
        graph
            .create_record(
                "Person",
                None,
                vec![
                    ("name".into(), FieldChange::Set(json!(name))),
                    ("age".into(), FieldChange::Set(json!(age))),
                ],
            )
            .unwrap()
    };
    let alice = person("alice", 29);
    let bob = person("bob", 27);
    let carol = person("carol", 32);
    let dave = person("dave", 35);
    let lop = graph
        .create_record(
            "Software",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!("lop"))),
                ("lang".into(), FieldChange::Set(json!("java"))),
            ],
        )
        .unwrap();
    for (rel, a, b) in [
        ("KNOWS", alice, bob),
        ("KNOWS", alice, carol),
        ("KNOWS", carol, dave),
        ("CREATED", alice, lop),
        ("CREATED", carol, lop),
    ] {
        graph.create_relationship(rel, a, b, vec![]).unwrap();
    }
    graph
}

fn profile() -> Value {
    serde_json::from_str(PROFILE).expect("valid profile JSON")
}

fn rows(graph: &Lmpg, traversal: &str) -> usize {
    execute(graph, QueryLanguage::Gremlin, traversal, None)
        .unwrap_or_else(|error| panic!("{traversal}: {error}"))
        .rows
        .len()
}

#[test]
fn every_profile_read_runs_with_its_row_count() {
    let graph = fixture();
    for case in profile()["reads"].as_array().unwrap() {
        let traversal = case["traversal"].as_str().unwrap();
        assert_eq!(
            rows(&graph, traversal) as u64,
            case["rows"].as_u64().unwrap(),
            "{traversal}"
        );
    }
}

#[test]
fn every_profile_write_commits_its_effect() {
    for case in profile()["writes"].as_array().unwrap() {
        let graph = fixture();
        let traversal = case["traversal"].as_str().unwrap();
        let result = execute_mutation(&graph, QueryLanguage::Gremlin, traversal, None)
            .unwrap_or_else(|error| panic!("{traversal}: {error}"));
        assert_eq!(
            result.affected,
            case["affected"].as_u64().unwrap(),
            "{traversal}"
        );
    }
}

#[test]
fn every_profile_bytecode_translates_to_its_traversal_and_runs() {
    let graph = fixture();
    for case in profile()["bytecode"].as_array().unwrap() {
        let traversal = case["traversal"].as_str().unwrap();
        assert_eq!(
            gremlin_bytecode::bytecode_to_gremlin(&case["bytecode"]).unwrap(),
            traversal
        );
        assert_eq!(
            rows(&graph, traversal) as u64,
            case["rows"].as_u64().unwrap(),
            "{traversal}"
        );
    }
}
