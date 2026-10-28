//! Every reference query of the pinned LDBC SNB Interactive v1 Cypher
//! implementation is classified for the published GQL profile
//! (`conformance/gql-ldbc-snb-v1-mapping.json`): accepted cases run against
//! the manifest's fixture and must return their hand-derived rows (reads) or
//! commit their effect (writes); the rest name why they are not mapped.

use std::collections::BTreeSet;
use std::sync::Arc;

use rushdb_language::{execute, execute_mutation, QueryLanguage};
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::Value;

const MAPPING: &str = include_str!("../conformance/gql-ldbc-snb-v1-mapping.json");

fn fixture(mapping: &Value) -> Lmpg {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(1));
    execute_mutation(
        &graph,
        QueryLanguage::Gql,
        mapping["fixture"].as_str().expect("fixture script"),
        None,
    )
    .expect("fixture loads");
    graph
}

#[test]
fn gql_profile_classifies_and_executes_the_ldbc_corpus() {
    let mapping: Value = serde_json::from_str(MAPPING).expect("valid mapping JSON");
    assert_eq!(
        mapping["source"]["commit"],
        "11db98cc2ba14c33492f6c0c34e68c8be7e22e5f"
    );
    let cases = mapping["cases"].as_array().expect("mapping cases");
    // 7 short, 14 complex and 8 update reference queries, each exactly once.
    let sources: BTreeSet<&str> = cases
        .iter()
        .map(|case| case["sourcePath"].as_str().expect("source path"))
        .collect();
    assert_eq!(sources.len(), 29);
    assert_eq!(cases.len(), 29);

    let reads = fixture(&mapping);
    for case in cases {
        let id = &case["id"];
        match case["status"].as_str() {
            Some("typedUnsupported") => {
                assert!(
                    case["reason"].as_str().is_some_and(|r| !r.is_empty()),
                    "{id}"
                );
                continue;
            }
            Some("accepted") => {}
            other => panic!("{id}: unknown status {other:?}"),
        }
        let statement = case["statement"].as_str().expect("mapped statement");
        let parameters = case["parameters"].clone();
        if case["kind"] == "write" {
            let graph = fixture(&mapping);
            execute_mutation(&graph, QueryLanguage::Gql, statement, Some(&parameters))
                .unwrap_or_else(|error| panic!("{id}: {error}"));
            let check = case["check"].as_str().expect("write check");
            let rows = execute(&graph, QueryLanguage::Gql, check, None)
                .unwrap_or_else(|error| panic!("{id} check: {error}"))
                .rows;
            assert_eq!(Value::Array(rows), case["expectedCheckRows"], "write {id}");
        } else {
            let rows = execute(&reads, QueryLanguage::Gql, statement, Some(&parameters))
                .unwrap_or_else(|error| panic!("{id}: {error}"))
                .rows;
            assert_eq!(Value::Array(rows), case["expectedRows"], "read {id}");
        }
    }
}
