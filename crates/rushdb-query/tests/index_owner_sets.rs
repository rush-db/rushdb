//! READ-2: an indexed leaf's owner set is cached and shared by its readers.
//! A commit that writes one of its indexes makes later readers compute it
//! again, so a cached set never hides a write; a commit that does not (another
//! tenant, another property) leaves it valid; a reader pinned to an older
//! snapshot keeps its own answer.

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

/// Index-entry reads of `total` at the store's current snapshot.
fn index_entry_reads(lmpg: &Lmpg, n: i64) -> (u64, u64) {
    let view = Arc::new(lmpg.store.read_view().unwrap());
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    let matched = total(lmpg, &view, n);
    let reads = rushdb_storage::read_accounting::take_local()
        .into_iter()
        .filter(|((_, family), _)| family == "index-entry")
        .map(|(_, count)| count)
        .sum();
    (matched, reads)
}

#[test]
fn cached_owner_sets_survive_commits_that_do_not_write_their_indexes() {
    let store = Arc::new(Store::default());
    let lmpg = Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(1));
    let other = Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(2));
    let record = |i: i64| {
        vec![
            ("n".into(), FieldChange::Set(json!(i % 10))),
            ("m".into(), FieldChange::Set(json!(i))),
        ]
    };
    for i in 0..300 {
        lmpg.create_record("Big", None, record(i)).unwrap();
        other.create_record("Big", None, record(i)).unwrap();
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
    let (matched, first) = index_entry_reads(&lmpg, 5);
    assert_eq!(matched, 30);
    assert!(first > 0, "the first query reads its index entries");
    // Another tenant's write, and a write to an unindexed property of this
    // tenant, leave the cached sets valid: no index entry is read again.
    other.create_record("Big", None, record(301)).unwrap();
    lmpg.create_record(
        "Note",
        None,
        vec![("text".into(), FieldChange::Set(json!("x")))],
    )
    .unwrap();
    assert_eq!(index_entry_reads(&lmpg, 5), (30, 0));
    // A write to an indexed property invalidates them, and the new record
    // is found.
    lmpg.create_record("Big", None, record(305)).unwrap();
    let (matched, reread) = index_entry_reads(&lmpg, 5);
    assert_eq!(matched, 31);
    assert!(
        reread > 0,
        "the sets are read again after their index changed"
    );
    assert_eq!(index_entry_reads(&lmpg, 5), (31, 0));
}
