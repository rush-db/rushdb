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
fn aggregate_applies_the_configured_limit() {
    let store = store_with_cap(128);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    let result = rushdb_query::aggregate_with_cancel_in_view(
        &lmpg,
        &rushdb_query::AggregateRequest::default(),
        &|| false,
        Arc::new(store.read_view().unwrap()),
    );
    assert!(matches!(result, Err(QueryError::ResourceLimit(_))));
}

/// Seeds `count` trips with a few scalar columns in one commit.
fn seed_trips(lmpg: &Lmpg, count: u32) {
    let rows: Vec<serde_json::Value> = (0..count)
        .map(|i| {
            json!({
                "vendor": if i % 3 == 0 { "A" } else { "B" },
                "hour": i % 24,
                "fare": f64::from(i % 50) + 0.5,
                "distance": f64::from(i % 7),
                "note": format!("trip {i}"),
            })
        })
        .collect();
    let options = rushdb_lmpg::import::ImportOptions::default();
    let plan = rushdb_lmpg::import::serialize_bfs(&json!(rows), "Trip", &options);
    rushdb_lmpg::import::import_bfs(lmpg, &plan, &options).unwrap();
}

#[test]
fn grouped_averages_over_many_records_fit_query_memory_max() {
    // The aggregate working set is the match ordinals, the row being
    // aggregated and the buckets, under `query-memory-max`. Before, the
    // candidate vector's *capacity* was charged twice over (16 B then 8 B
    // per slot) and tripled again for an `$avg`, so 30,000 matches needed
    // over 768 KiB and every row was rebuilt in full (the `note` column
    // included).
    let store = store_with_cap(600_000);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    seed_trips(&lmpg, 30_000);
    let request = rushdb_query::AggregateRequest {
        labels: vec!["Trip".into()],
        group_by: vec!["vendor".into()],
        avg_terms: vec![("dist".into(), "distance".into())],
        maxs: vec![("max_fare".into(), "fare".into())],
        counts: vec![("n".into(), "*".into())],
        ..Default::default()
    };
    let view = Arc::new(store.read_view().unwrap());
    let (buckets, total) =
        rushdb_query::aggregate_with_cancel_in_view(&lmpg, &request, &|| false, view).unwrap();
    assert_eq!(total, 30_000);
    assert_eq!(buckets.len(), 2);
    let a = buckets
        .iter()
        .find(|b| b.keys["vendor"] == json!("A"))
        .unwrap();
    assert_eq!(a.count_roots, 10_000);
    assert_eq!(a.maxs["max_fare"], json!(49.5));
    let expected_distance = (0..30_000u32)
        .filter(|i| i % 3 == 0)
        .map(|i| f64::from(i % 7))
        .sum::<f64>()
        / 10_000.0;
    assert_eq!(a.sums["dist"], json!(expected_distance));

    // A walk of the label holds no list of every member: each part reads
    // its own range of ordinals, so 30,000 records aggregate within 200 KB.
    // The cap still binds what a part holds: one part's members (8,192
    // ordinals, 64 KiB) do not fit 40 KB.
    for (cap, fits) in [(200_000, true), (40_000, false)] {
        let store = store_with_cap(cap);
        let lmpg = Lmpg::new(store.clone(), TenantId::single());
        seed_trips(&lmpg, 30_000);
        let view = Arc::new(store.read_view().unwrap());
        let outcome = rushdb_query::aggregate_with_cancel_in_view(&lmpg, &request, &|| false, view);
        match outcome {
            Ok((buckets, total)) => assert!(fits && total == 30_000 && buckets.len() == 2),
            Err(error) => assert!(
                !fits
                    && matches!(&error, QueryError::ResourceLimit(message)
                        if message.contains("query-memory-max")),
                "{error:?}"
            ),
        }
    }
}

#[test]
fn aggregate_limits_retained_groups_under_query_memory_max() {
    // Buckets are what an aggregate retains: 100 groups with 1 KiB keys do
    // not fit a 64 KiB working set, whether or not `skip` discards them.
    let store = store_with_cap(64 * 1024);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    for i in 0..100 {
        lmpg.create_record(
            "Groups",
            None,
            vec![(
                "group".into(),
                FieldChange::Set(json!(format!("{i}{}", "x".repeat(1024)))),
            )],
        )
        .unwrap();
    }
    let request = rushdb_query::AggregateRequest {
        labels: vec!["Groups".into()],
        group_by: vec!["group".into()],
        skip: 100,
        ..Default::default()
    };
    let view = Arc::new(store.read_view().unwrap());
    let result = rushdb_query::aggregate_with_cancel_in_view(&lmpg, &request, &|| false, view);
    assert!(
        matches!(result, Err(QueryError::ResourceLimit(_))),
        "{result:?}"
    );
}

#[test]
fn a_filtered_grouped_aggregate_reads_only_the_fields_it_names() {
    // The residual filter and the metrics read their properties by point
    // reads; a 64 KiB payload column is never decoded, so the working set
    // stays near the ordinals.
    let store = store_with_cap(96 * 1024);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    for i in 0..40 {
        lmpg.create_record(
            "Wide",
            None,
            vec![
                ("kind".into(), FieldChange::Set(json!(i % 2))),
                ("n".into(), FieldChange::Set(json!(i))),
                (
                    "payload".into(),
                    FieldChange::Set(json!("x".repeat(64 * 1024))),
                ),
            ],
        )
        .unwrap();
    }
    let parsed =
        parse_document(&json!({"labels": ["Wide"], "where": {"n": {"$gte": 20}}})).unwrap();
    let request = rushdb_query::AggregateRequest {
        labels: parsed.search.labels,
        where_: parsed.search.where_,
        group_by: vec!["kind".into()],
        sums: vec![("sum".into(), "n".into())],
        ..Default::default()
    };
    let view = Arc::new(store.read_view().unwrap());
    let (buckets, total) =
        rushdb_query::aggregate_with_cancel_in_view(&lmpg, &request, &|| false, view).unwrap();
    assert_eq!(total, 20);
    let even = buckets.iter().find(|b| b.keys["kind"] == json!(0)).unwrap();
    assert_eq!(
        even.sums["sum"],
        json!(f64::from((20..40).step_by(2).sum::<i32>()))
    );
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

/// Seeds `count` `Doc` records (`doc_id`, `tenant`, a short `body`) in one
/// commit.
fn seed_docs(lmpg: &Lmpg, count: u32) {
    let rows: Vec<serde_json::Value> = (0..count)
        .map(|i| json!({ "doc_id": i, "tenant": i % 500, "body": format!("doc {i}") }))
        .collect();
    let options = rushdb_lmpg::import::ImportOptions::default();
    let plan = rushdb_lmpg::import::serialize_bfs(&json!(rows), "Doc", &options);
    rushdb_lmpg::import::import_bfs(lmpg, &plan, &options).unwrap();
}

/// What a transport does with a query document: an aggregate renders its
/// buckets, anything else is a records page; both under a `response`-byte
/// budget (the body limit), on a store whose `query-memory-max` is `cap`.
fn run_document(
    cap: u64,
    docs: u32,
    document: &serde_json::Value,
    response: usize,
) -> Result<(u64, Vec<serde_json::Value>), QueryError> {
    let store = store_with_cap(cap);
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    seed_docs(&lmpg, docs);
    run_in(&lmpg, &store, document, Some(response))
}

fn run_in(
    lmpg: &Lmpg,
    store: &Store,
    document: &serde_json::Value,
    response: Option<usize>,
) -> Result<(u64, Vec<serde_json::Value>), QueryError> {
    let parsed = parse_document(document).unwrap();
    let view = Arc::new(store.read_view().unwrap());
    if let Some(aggregate) = parsed.aggregate {
        let (buckets, total) = rushdb_query::aggregate_with_cancel_in_view(
            lmpg,
            &aggregate,
            &|| false,
            Arc::clone(&view),
        )?;
        let rows = rushdb_query::aggregate_to_rows_in_view_with_budget(
            lmpg, &view, &buckets, &aggregate, response,
        )?;
        return Ok((total, rows));
    }
    let page = rushdb_query::find_records_in_view_with_budget(
        lmpg,
        &parsed.search,
        &|| false,
        view,
        response,
    )?;
    Ok((page.total_matched, page.records))
}

const DEFAULT_QUERY_MEMORY_MAX: u64 = 256 * 1024 * 1024;

fn resource_message(result: Result<(u64, Vec<serde_json::Value>), QueryError>) -> String {
    match result {
        Err(QueryError::ResourceLimit(message)) => message,
        other => panic!("expected a resource limit, got {other:?}"),
    }
}

#[test]
fn a_projection_returns_every_row_its_real_bytes_allow() {
    // BENCH-9: a select of plain fields went to the aggregate executor,
    // which materialized every match and refused its rows on an estimate
    // of ~770 bytes per row, ~40 times their JSON. 20,000 `{"doc_id": n}`
    // rows are ~330 KB; they fit a 2 MiB response budget that the estimate
    // (15 MB) did not.
    let tenants: Vec<u32> = (0..500).collect();
    let document = json!({
        "labels": ["Doc"],
        "where": { "tenant": { "$in": tenants } },
        "select": { "doc_id": "$record.doc_id" },
        "limit": 100_000
    });
    let parsed = parse_document(&document).unwrap();
    assert!(parsed.aggregate.is_none(), "a projection is a listing");
    assert_eq!(
        parsed.search.projection,
        vec![("doc_id".to_owned(), "doc_id".to_owned())]
    );
    let (total, rows) =
        run_document(DEFAULT_QUERY_MEMORY_MAX, 20_000, &document, 2 * 1024 * 1024).unwrap();
    assert_eq!(total, 20_000);
    assert_eq!(rows.len(), 20_000);
    assert_eq!(rows[7], json!({ "doc_id": 7 }));
    let real: usize = rows.iter().map(|row| row.to_string().len()).sum();
    assert!(real < 512 * 1024, "{real} bytes of rows");
}

#[test]
fn an_aggregate_renders_rows_charged_by_their_real_bytes() {
    // A per-root aggregate (a metric beside a field, no `groupBy`) is one
    // row per record: its render is charged by each row's real JSON, not by
    // an estimate of every bucket up front.
    let document = json!({
        "labels": ["Doc"],
        "select": { "doc_id": "$record.doc_id", "n": { "$count": "*" } },
        "limit": 100_000
    });
    assert!(parse_document(&document).unwrap().aggregate.is_some());
    let (total, rows) =
        run_document(DEFAULT_QUERY_MEMORY_MAX, 20_000, &document, 2 * 1024 * 1024).unwrap();
    assert_eq!(total, 20_000);
    assert_eq!(rows.len(), 20_000);
    assert_eq!(rows[3], json!({ "doc_id": 3, "n": 1 }));
}

#[test]
fn a_budget_error_names_the_limit_that_tripped() {
    let projection = json!({
        "labels": ["Doc"],
        "select": { "doc_id": "$record.doc_id" },
        "limit": 100_000
    });
    let aggregate = json!({
        "labels": ["Doc"],
        "select": { "doc_id": "$record.doc_id", "n": { "$count": "*" } },
        "limit": 100_000
    });
    for document in [&projection, &aggregate] {
        // The rows (~40 KB) do not fit a 16 KiB response.
        let response = resource_message(run_document(
            DEFAULT_QUERY_MEMORY_MAX,
            2_000,
            document,
            16 * 1024,
        ));
        assert!(
            response.contains("response size limit (16384 bytes)")
                && response.contains("--body-limit")
                && !response.contains("query-memory-max"),
            "{response}"
        );
        // 2,000 match ordinals do not fit a 4 KiB working set.
        let working = resource_message(run_document(4096, 2_000, document, 1024 * 1024));
        assert!(
            working.contains("query-memory-max (4096 bytes)")
                && working.contains("--query-memory-max")
                && !working.contains("body-limit"),
            "{working}"
        );
    }
}

#[test]
fn a_projection_page_stops_reading_after_its_rows() {
    // Without an order, a projection reads its page and stops: the total
    // comes from the label counter, so 20 times the matches costs about the
    // same reads. The aggregate executor read every match.
    let reads = |docs: u32| -> u64 {
        let document = json!({
            "labels": ["Doc"],
            "select": { "doc_id": "$record.doc_id" },
            "limit": 10
        });
        // Reads are counted by the durable backend.
        let path = rushdb_storage::TempPath::store("projection-reads");
        let store = Arc::new(Store::open(&path).unwrap());
        let lmpg = Lmpg::new(store.clone(), TenantId::single());
        seed_docs(&lmpg, docs);
        // Accounting is switched on and left on: other tests in this binary
        // may flip the global flag, and this thread's own tally is checked.
        rushdb_storage::read_accounting::enable(true);
        let _ = rushdb_storage::read_accounting::take_local();
        let (total, rows) = run_in(&lmpg, &store, &document, None).unwrap();
        let reads = rushdb_storage::read_accounting::take_local().values().sum();
        assert_eq!(total, u64::from(docs));
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0], json!({ "doc_id": 0 }));
        drop((lmpg, store));
        rushdb_storage::remove_store(&path).unwrap();
        reads
    };
    let small = reads(1_000);
    let large = reads(20_000);
    assert!(small > 0);
    assert!(
        large < small.saturating_mul(2),
        "1K matches: {small} reads; 20K matches: {large} reads"
    );
}
