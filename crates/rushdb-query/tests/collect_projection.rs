//! A `$collect` with a `fields` projection decodes only what it uses: the
//! projected sources, its filter's fields and its ordering's. The answer is
//! the one a whole-child rebuild gives.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_query::document::parse_document;
use rushdb_query::{find_records, CollectSpec, Direction, SearchRequest};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

#[test]
fn a_projected_collect_filters_and_orders_on_fields_it_does_not_return() {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::single());
    let root = lmpg
        .create_record(
            "Root",
            None,
            vec![("name".into(), FieldChange::Set(json!("r")))],
        )
        .unwrap();
    for (name, rank) in [("a", 3), ("b", 1), ("c", 2)] {
        let child = lmpg
            .create_record(
                "Child",
                None,
                vec![
                    ("name".into(), FieldChange::Set(json!(name))),
                    ("rank".into(), FieldChange::Set(json!(rank))),
                    ("payload".into(), FieldChange::Set(json!("x".repeat(4096)))),
                ],
            )
            .unwrap();
        lmpg.create_relationship("HAS", root, child, vec![])
            .unwrap();
    }
    let filter = parse_document(&json!({ "where": { "rank": { "$gte": 2 } } }))
        .unwrap()
        .search
        .where_;
    let request = SearchRequest {
        labels: vec!["Root".into()],
        collects: vec![CollectSpec {
            steps: vec![],
            alias: "children".into(),
            rel_type: Some("HAS".into()),
            direction: Direction::Outgoing,
            target_labels: vec![],
            fields: vec![("n".into(), "name".into())],
            nested: vec![],
            filter,
            order_by: vec![("rank".into(), true)],
            skip: 0,
            unique: true,
            limit: None,
        }],
        ..Default::default()
    };
    let page = find_records(&lmpg, &request).unwrap();
    assert_eq!(page.records.len(), 1);
    let children = page.records[0]["children"].as_array().unwrap();
    // `b` (rank 1) is filtered out; `a` (3) sorts before `c` (2); only `n`
    // is returned.
    assert_eq!(children.len(), 2, "{children:?}");
    assert_eq!(children[0], json!({ "n": "a" }));
    assert_eq!(children[1], json!({ "n": "c" }));
}
