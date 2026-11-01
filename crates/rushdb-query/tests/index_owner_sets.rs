//! READ-2: an indexed leaf's owner set is cached per snapshot and shared by
//! its readers; a commit moves readers to a new snapshot, so a cached set
//! never hides a write, and a reader pinned to an older snapshot keeps its
//! own answer.

use std::sync::Arc;

use rushdb_lmpg::index::IndexStep;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_query::document::parse_document;
use rushdb_query::find_records_in_view;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use rushdb_types::index::IndexKind;
use serde_json::json;

fn total(lmpg: &Lmpg, view: &Arc<rushdb_storage::ReadView>, n: i64) -> u64 {
    let parsed =
        parse_document(&json!({"labels": ["Big"], "where": {"n": n, "m": {"$gte": 0}}})).unwrap();
    find_records_in_view(lmpg, &parsed.search, &|| false, Arc::clone(view))
        .unwrap()
        .total_matched
}

#[test]
fn cached_owner_sets_follow_the_snapshot() {
    let lmpg = Lmpg::new(Arc::new(Store::default()), TenantId::single());
    let record = |i: i64| {
        vec![
            ("n".into(), FieldChange::Set(json!(i % 10))),
            ("m".into(), FieldChange::Set(json!(i))),
        ]
    };
    for i in 0..300 {
        lmpg.create_record("Big", None, record(i)).unwrap();
    }
    for name in ["n", "m"] {
        let def = lmpg
            .define_index(&format!("big_{name}"), "Big", name, IndexKind::Ordered)
            .unwrap();
        while !matches!(
            lmpg.index_step(def.id, 1_000).unwrap(),
            IndexStep::Finished | IndexStep::Idle
        ) {}
    }
    let before = Arc::new(lmpg.store.read_view().unwrap());
    assert_eq!(total(&lmpg, &before, 5), 30);
    assert_eq!(total(&lmpg, &before, 5), 30, "a cached set answers alike");
    lmpg.create_record("Big", None, record(305)).unwrap();
    let after = Arc::new(lmpg.store.read_view().unwrap());
    assert_eq!(total(&lmpg, &after, 5), 31, "the write is seen");
    assert_eq!(
        total(&lmpg, &before, 5),
        30,
        "the older snapshot keeps its answer"
    );
}
