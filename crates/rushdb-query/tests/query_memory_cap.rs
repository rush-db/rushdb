//! Per-query working-set cap (config #3): a query that would materialize more
//! than the configured budget returns `resource exhausted` instead of
//! ballooning RSS. The default (0) is off, so existing behavior is unchanged.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_query::document::parse_document;
use rushdb_query::{find_records_in_view, QueryError};
use rushdb_storage::resources::StorageResources;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

fn store_with_cap(cap: u64) -> Arc<Store> {
    Arc::new(
        Store::new_with_resources(StorageResources {
            query_memory_max_bytes: cap,
            ..StorageResources::default()
        })
        .unwrap(),
    )
}

fn seed(lmpg: &Lmpg) {
    for i in 0..300 {
        lmpg.create_record("Big", None, vec![("n".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }
}

#[test]
fn query_memory_cap_rejects_an_oversized_working_set() {
    let store = store_with_cap(1024);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    seed(&lmpg);
    let parsed = parse_document(&json!({ "labels": ["Big"] })).unwrap();
    let view = Arc::new(store.read_view().unwrap());
    let err = find_records_in_view(&lmpg, &parsed.search, &|| false, view).unwrap_err();
    assert!(
        matches!(err, QueryError::ResourceLimit(_)),
        "expected a resource cap, got {err:?}"
    );
}

#[test]
fn cancellation_interrupts_candidate_discovery() {
    // The unscoped candidate scan must poll cancellation between batches, not
    // only per-candidate during predicate evaluation.
    let store = store_with_cap(0);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    for i in 0..5000u64 {
        lmpg.create_record("Big", None, vec![("n".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }
    let view = Arc::new(store.read_view().unwrap());
    let parsed = parse_document(&json!({})).unwrap();
    let polls = std::sync::atomic::AtomicUsize::new(0);
    let cancelled = || {
        // First poll (the pre-dispatch check) passes; the next (inside the
        // candidate scan) cancels.
        polls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1 >= 2
    };
    let err = find_records_in_view(&lmpg, &parsed.search, &cancelled, view).unwrap_err();
    assert!(matches!(err, QueryError::Cancelled), "{err:?}");
}

#[test]
fn query_memory_cap_is_off_by_default() {
    let store = store_with_cap(0);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    seed(&lmpg);
    let parsed = parse_document(&json!({ "labels": ["Big"] })).unwrap();
    let view = Arc::new(store.read_view().unwrap());
    let page = find_records_in_view(&lmpg, &parsed.search, &|| false, view).unwrap();
    assert_eq!(page.total_matched, 300);
}

#[test]
fn record_collect_respects_the_configured_working_set_cap() {
    use rushdb_query::{CollectSpec, Direction};
    let store = store_with_cap(64 * 1024);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    let root = lmpg.create_record("Root", None, vec![]).unwrap();
    for _ in 0..4 {
        let child = lmpg
            .create_record(
                "Child",
                None,
                vec![(
                    "payload".into(),
                    FieldChange::Set(json!("x".repeat(32 * 1024))),
                )],
            )
            .unwrap();
        lmpg.create_relationship("HAS", root, child, vec![])
            .unwrap();
    }
    let mut parsed = parse_document(&json!({ "labels": ["Root"] })).unwrap();
    parsed.search.collects = vec![CollectSpec {
        steps: vec![],
        alias: "children".into(),
        rel_type: Some("HAS".into()),
        direction: Direction::Outgoing,
        target_labels: vec![],
        fields: vec![],
        nested: vec![],
        filter: None,
        order_by: vec![],
        skip: 0,
        unique: true,
        limit: None,
    }];
    let view = Arc::new(store.read_view().unwrap());
    let err = find_records_in_view(&lmpg, &parsed.search, &|| false, view).unwrap_err();
    assert!(
        matches!(err, QueryError::ResourceLimit(_) | QueryError::Storage(_)),
        "collect expansion must be charged against the working-set cap: {err:?}"
    );
}

#[test]
fn records_explicit_zero_budget_does_not_mean_unlimited() {
    // An exhausted request budget arrives as `Some(0)`. It must be a hard zero
    // cap, not silently disable the limit and admit the whole result set.
    let store = store_with_cap(0);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    seed(&lmpg);
    let parsed = parse_document(&json!({ "labels": ["Big"] })).unwrap();
    let view = Arc::new(store.read_view().unwrap());
    let err = rushdb_query::find_records_in_view_with_budget(
        &lmpg,
        &parsed.search,
        &|| false,
        view,
        Some(0),
    )
    .unwrap_err();
    assert!(
        matches!(err, QueryError::ResourceLimit(_)),
        "explicit zero budget must reject, got {err:?}"
    );
}

#[test]
fn collect_expansion_is_cumulative_across_page_records() {
    // `$collect` must be charged cumulatively: two roots whose collected
    // children together exceed the budget must fail even though each root
    // alone would fit the full remaining budget.
    use rushdb_query::{CollectSpec, Direction};
    let store = store_with_cap(96 * 1024);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    for _ in 0..2 {
        let root = lmpg.create_record("Root", None, vec![]).unwrap();
        let child = lmpg
            .create_record(
                "Child",
                None,
                vec![(
                    "payload".into(),
                    FieldChange::Set(json!("x".repeat(64 * 1024))),
                )],
            )
            .unwrap();
        lmpg.create_relationship("HAS", root, child, vec![])
            .unwrap();
    }
    let mut parsed = parse_document(&json!({ "labels": ["Root"] })).unwrap();
    parsed.search.collects = vec![CollectSpec {
        steps: vec![],
        alias: "children".into(),
        rel_type: Some("HAS".into()),
        direction: Direction::Outgoing,
        target_labels: vec![],
        fields: vec![],
        nested: vec![],
        filter: None,
        order_by: vec![],
        skip: 0,
        unique: true,
        limit: None,
    }];
    let view = Arc::new(store.read_view().unwrap());
    let err = find_records_in_view(&lmpg, &parsed.search, &|| false, view).unwrap_err();
    assert!(
        matches!(err, QueryError::ResourceLimit(_) | QueryError::Storage(_)),
        "collect expansion must be cumulative across records: {err:?}"
    );
}

#[test]
fn aggregate_explicit_zero_budget_does_not_mean_unlimited() {
    let store = store_with_cap(0);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    let result = rushdb_query::aggregate_with_cancel_in_view_with_budget(
        &lmpg,
        &rushdb_query::AggregateRequest::default(),
        &|| false,
        Arc::new(store.read_view().unwrap()),
        Some(0),
    );
    assert!(matches!(result, Err(QueryError::ResourceLimit(_))));
}

#[test]
fn aggregate_applies_the_stricter_configured_limit() {
    let store = store_with_cap(128);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    let result = rushdb_query::aggregate_with_cancel_in_view_with_budget(
        &lmpg,
        &rushdb_query::AggregateRequest::default(),
        &|| false,
        Arc::new(store.read_view().unwrap()),
        Some(1024 * 1024),
    );
    assert!(matches!(result, Err(QueryError::ResourceLimit(_))));
}

#[test]
fn aggregate_indexed_candidates_respect_work_budget() {
    let store = store_with_cap(0);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    for _ in 0..200 {
        lmpg.create_record(
            "Shared",
            None,
            vec![("n".into(), FieldChange::Set(json!(1)))],
        )
        .unwrap();
    }
    let parsed = parse_document(&json!({"labels": ["Shared"], "where": {"n": 1}})).unwrap();
    let request = rushdb_query::AggregateRequest {
        labels: parsed.search.labels,
        where_: parsed.search.where_,
        sums: vec![("sum".into(), "n".into())],
        ..Default::default()
    };
    let view = Arc::new(store.read_view().unwrap());
    let result = rushdb_query::aggregate_with_cancel_in_view_with_budget(
        &lmpg,
        &request,
        &|| false,
        view.clone(),
        Some(32 * 1024),
    );
    assert!(
        matches!(result, Err(QueryError::ResourceLimit(_))),
        "{result:?}"
    );
    let (buckets, _) = rushdb_query::aggregate_with_cancel_in_view_with_budget(
        &lmpg,
        &request,
        &|| false,
        view,
        Some(1024 * 1024),
    )
    .unwrap();
    assert_eq!(buckets[0].sums["sum"], json!(200.0));
}

#[test]
fn aggregate_collect_budget_covers_work_discarded_by_child_pagination() {
    use rushdb_query::{AggregateRequest, CollectSpec, Direction};
    let store = store_with_cap(0);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    let root = rushdb_lmpg::import::import_document(&lmpg, "Root", &json!({"n": 0})).unwrap();
    let child = rushdb_lmpg::import::import_document(
        &lmpg,
        "Child",
        &json!({"payload": "x".repeat(128 * 1024)}),
    )
    .unwrap();
    lmpg.create_relationship("HAS", root, child, vec![])
        .unwrap();
    let request = AggregateRequest {
        labels: vec!["Root".into()],
        collects: vec![CollectSpec {
            steps: vec![],
            alias: "children".into(),
            rel_type: Some("HAS".into()),
            direction: Direction::Outgoing,
            target_labels: vec![],
            fields: vec![],
            nested: vec![],
            filter: None,
            order_by: vec![],
            skip: 1,
            unique: true,
            limit: None,
        }],
        ..Default::default()
    };
    let view = Arc::new(store.read_view().unwrap());
    let (buckets, _) = rushdb_query::aggregate_in_view(&lmpg, &request, view.clone()).unwrap();
    let result = rushdb_query::aggregate_to_rows_in_view_with_budget(
        &lmpg,
        &view,
        &buckets,
        &request,
        Some(64 * 1024),
    );
    assert!(
        matches!(result, Err(QueryError::ResourceLimit(_))),
        "{result:?}"
    );
    let rows = rushdb_query::aggregate_to_rows_in_view_with_budget(
        &lmpg,
        &view,
        &buckets,
        &request,
        Some(8 * 1024 * 1024),
    )
    .unwrap();
    assert_eq!(rows[0]["children"], json!([]));
}

#[test]
fn the_response_budget_caps_the_page_not_the_working_set() {
    // READ-1: the request's response byte budget (the HTTP body limit) used
    // to cap the candidate and match ordinals too, so a label over ~2M
    // records refused even a 1-row page. The working set is capped by
    // `query-memory-max` alone, and it holds only the matches a page keeps:
    // candidates stream past and the total comes from the label counter.
    let read = |cap: u64, response_budget: usize, limit: u64| {
        let store = store_with_cap(cap);
        let lmpg = Lmpg::new(store.clone(), TenantId::single());
        seed(&lmpg);
        let parsed = parse_document(&json!({ "labels": ["Big"], "limit": limit })).unwrap();
        let view = Arc::new(store.read_view().unwrap());
        rushdb_query::find_records_in_view_with_budget(
            &lmpg,
            &parsed.search,
            &|| false,
            view,
            Some(response_budget),
        )
    };
    // 300 candidates, a 1-row page: fits a 1 KiB working set and a 1 KiB
    // response budget, with the exact total.
    let page = read(1024, 1024, 1).unwrap();
    assert_eq!(page.total_matched, 300);
    assert_eq!(page.records.len(), 1);
    // `query-memory-max` still binds what a page keeps: 300 ordinals are
    // 2,400 bytes.
    assert!(matches!(
        read(1024, 1024 * 1024, 1_000),
        Err(QueryError::ResourceLimit(_))
    ));
}
