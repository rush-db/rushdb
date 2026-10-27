//! SCALE-3, QRY-3: an unindexed-walk limit refuses a filter without a
//! usable index whose record population exceeds it, at planning, with the
//! property and the remedy named; indexed filters and populations within the
//! limit run. The server sets none by default (like PostgreSQL); a request
//! may set one, and may only tighten the server's. The request's deadline
//! stops a walk whatever probe the caller passes.

use std::sync::Arc;

use rushdb_lmpg::index::IndexStep;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{Lmpg, RequestLimits};
use rushdb_query::document::parse_document;
use rushdb_query::{find_records_in_view, QueryError};
use rushdb_storage::resources::StorageResources;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use rushdb_types::index::IndexKind;
use serde_json::json;

fn engine(limit: u64) -> Lmpg {
    let store = Store::new_with_resources(StorageResources {
        unindexed_walk_max_owners: limit,
        ..StorageResources::default()
    })
    .unwrap();
    let lmpg = Lmpg::new(Arc::new(store), TenantId::single());
    for i in 0..300 {
        lmpg.create_record(
            "Big",
            None,
            vec![("n".into(), FieldChange::Set(json!(i % 10)))],
        )
        .unwrap();
    }
    lmpg
}

fn find(lmpg: &Lmpg, document: &serde_json::Value) -> Result<u64, QueryError> {
    let parsed = parse_document(document).unwrap();
    let view = Arc::new(lmpg.store.read_view().unwrap());
    find_records_in_view(lmpg, &parsed.search, &|| false, view).map(|page| page.total_matched)
}

#[test]
fn a_walk_over_the_limit_is_refused_with_the_index_to_declare() {
    let lmpg = engine(100);
    let err = find(&lmpg, &json!({"labels": ["Big"], "where": {"n": 5}})).unwrap_err();
    let QueryError::UnindexedWalk(message) = &err else {
        panic!("expected an unindexed-walk refusal, got {err:?}");
    };
    assert!(
        message.contains("`n`") && message.contains("300"),
        "{message}"
    );
    // No walkable condition: the whole label is the population.
    assert!(matches!(
        find(
            &lmpg,
            &json!({"labels": ["Big"], "where": {"n": {"$ne": 5}}})
        ),
        Err(QueryError::UnindexedWalk(_))
    ));
    // No filter: nothing is walked.
    assert_eq!(find(&lmpg, &json!({"labels": ["Big"]})).unwrap(), 300);
}

#[test]
fn an_indexed_filter_or_a_walk_within_the_limit_runs() {
    assert_eq!(
        find(
            &engine(1_000),
            &json!({"labels": ["Big"], "where": {"n": 5}})
        )
        .unwrap(),
        30
    );
    assert_eq!(
        find(&engine(0), &json!({"labels": ["Big"], "where": {"n": 5}})).unwrap(),
        30,
        "0 = off"
    );
    let lmpg = engine(100);
    let def = lmpg
        .define_index("big_n", "Big", "n", IndexKind::Ordered)
        .unwrap();
    while !matches!(
        lmpg.index_step(def.id, 1_000).unwrap(),
        IndexStep::Finished | IndexStep::Idle
    ) {}
    assert_eq!(
        find(&lmpg, &json!({"labels": ["Big"], "where": {"n": 5}})).unwrap(),
        30
    );
}

#[test]
fn a_request_limit_refuses_a_walk_the_server_allows() {
    let lmpg = engine(0).with_request_limits(RequestLimits {
        unindexed_walk_max_owners: Some(100),
        ..RequestLimits::default()
    });
    assert!(matches!(
        find(&lmpg, &json!({"labels": ["Big"], "where": {"n": 5}})),
        Err(QueryError::UnindexedWalk(_))
    ));
    // `0` refuses every walk, as `disallow_full_table_scans` does.
    let strict = engine(0).with_request_limits(RequestLimits {
        unindexed_walk_max_owners: Some(0),
        ..RequestLimits::default()
    });
    assert!(matches!(
        find(&strict, &json!({"labels": ["Big"], "where": {"n": 5}})),
        Err(QueryError::UnindexedWalk(_))
    ));
    assert_eq!(find(&strict, &json!({"labels": ["Big"]})).unwrap(), 300);
}

#[test]
fn a_request_cannot_lift_the_server_limit() {
    let lmpg = engine(100).with_request_limits(RequestLimits {
        unindexed_walk_max_owners: Some(1_000),
        ..RequestLimits::default()
    });
    assert!(matches!(
        find(&lmpg, &json!({"labels": ["Big"], "where": {"n": 5}})),
        Err(QueryError::UnindexedWalk(_))
    ));
}

#[test]
fn a_passed_deadline_stops_a_walk_the_caller_never_cancels() {
    let lmpg = engine(0).with_request_limits(RequestLimits {
        deadline: Some(std::time::Instant::now()),
        ..RequestLimits::default()
    });
    assert!(matches!(
        find(&lmpg, &json!({"labels": ["Big"], "where": {"n": 5}})),
        Err(QueryError::Cancelled)
    ));
}
