//! Planner differential oracle: every access path must return exactly what a
//! forced full scan returns, for eligible, residual, and mixed predicates.
//! `SearchRequest::force_scan` selects the path on identical input.

use std::collections::BTreeSet;
use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_query::document::parse_document;
use rushdb_query::{find_records, find_records_streaming};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

#[expect(
    clippy::cast_precision_loss,
    reason = "the fixture stores a small random integer divided by four as an f64 property value"
)]
fn setup(seed: u64) -> Lmpg {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(900 + seed));
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    for i in 0..300u64 {
        let label = if i % 3 == 0 { "B" } else { "A" };
        let mut fields: Vec<(String, FieldChange)> = vec![
            ("n".into(), FieldChange::Set(json!(next() % 20))),
            (
                "f".into(),
                FieldChange::Set(json!((next() % 100) as f64 / 4.0)),
            ),
            (
                "s".into(),
                FieldChange::Set(json!(format!("v{}", next() % 7))),
            ),
            ("flag".into(), FieldChange::Set(json!(next() % 2 == 0))),
        ];
        if i % 5 == 0 {
            // Array values (element postings; whole-array eval semantics).
            fields.push((
                "arr".into(),
                FieldChange::Set(json!([next() % 5, next() % 5])),
            ));
        }
        if i % 11 == 0 {
            fields.push(("nv".into(), FieldChange::ExplicitNull));
        }
        lmpg.create_record(label, None, fields).unwrap();
    }
    lmpg
}

fn ids_and_total(page: &rushdb_query::Page) -> (BTreeSet<String>, u64) {
    (
        page.records
            .iter()
            .filter_map(|r| r.get("__id").and_then(|v| v.as_str()).map(str::to_string))
            .collect(),
        page.total_matched,
    )
}

fn run(lmpg: &Lmpg, document: &Value, force_scan: bool, stream: bool) -> (BTreeSet<String>, u64) {
    let mut parsed = parse_document(document).expect("valid query document");
    parsed.search.force_scan = force_scan;
    let page = if stream {
        let snap = Arc::new(lmpg.store.read_view().unwrap());
        find_records_streaming(lmpg, &parsed.search, &|| false, snap).expect("streaming")
    } else {
        find_records(lmpg, &parsed.search).expect("scan")
    };
    ids_and_total(&page)
}

#[test]
fn signed_zero_and_integer_zero_index_matches_scan() {
    let store = Arc::new(rushdb_storage::Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::single());
    for value in [json!(-0.0), json!(0.0), json!(0)] {
        rushdb_lmpg::import::import_document(&lmpg, "Audit", &json!({"n": value})).unwrap();
    }
    for value in [json!(-0.0), json!(0.0), json!(0)] {
        let mut indexed = parse_document(&json!({"where": {"n": value}}))
            .unwrap()
            .search;
        let indexed_result = find_records(&lmpg, &indexed).unwrap();
        indexed.force_scan = true;
        let scanned_result = find_records(&lmpg, &indexed).unwrap();
        assert_eq!(indexed_result.total_matched, scanned_result.total_matched);
    }
}

/// Both executors, indexed and forced-scan, must agree exactly.
fn assert_paths(lmpg: &Lmpg, document: &Value, label: &str) {
    for stream in [false, true] {
        let indexed = run(lmpg, document, false, stream);
        let scanned = run(lmpg, document, true, stream);
        assert_eq!(
            indexed, scanned,
            "path mismatch ({label}, stream={stream}): {document}"
        );
    }
}

#[test]
fn eligible_residual_and_mixed_predicates_match_the_scan() {
    let lmpg = setup(0x5EED);
    let cases = vec![
        json!({"where": {"n": 3}}),
        json!({"where": {"n": {"$in": [1, 5, 9]}}}),
        json!({"where": {"n": {"$gte": 10}}}),
        json!({"where": {"n": {"$lt": 4}}}),
        json!({"where": {"f": {"$gt": 12.5}}}),
        json!({"where": {"s": "v2"}}),
        json!({"where": {"flag": true}}),
        json!({"where": {"arr": 2}}),
        json!({"where": {"nv": {"$eq": null}}}),
        json!({"where": {"n": {"$exists": true}}}),
        json!({"where": {"n": {"$exists": false}}}),
        // residual-only
        json!({"where": {"n": {"$ne": 3}}}),
        json!({"where": {"s": {"$contains": "v1"}}}),
        json!({"where": {"$or": [{"n": 1}, {"n": 2}]}}),
        // eligible + residual (partial-index planning)
        json!({"where": {"n": 3, "s": {"$contains": "v"}}}),
        json!({"where": {"flag": true, "n": {"$ne": 3}}}),
        json!({"where": {"n": {"$gte": 5}, "s": "v2", "flag": false}}),
        // eligible + eligible + residual
        json!({"where": {"n": {"$in": [1, 2, 3]}, "flag": true, "s": {"$contains": "v"}}}),
        // label scoped
        json!({"labels": ["A"], "where": {"n": 3}}),
        json!({"labels": ["A", "B"], "where": {"n": {"$exists": true}, "n": {"$lt": 5}}}),
        // empty match paths
        json!({"where": {"n": 999}}),
        json!({"where": {"nope": 1}}),
    ];
    for (i, case) in cases.iter().enumerate() {
        assert_paths(&lmpg, case, &format!("case {i}"));
    }
}

#[test]
#[expect(
    clippy::cast_precision_loss,
    reason = "the fixture stores small integers halved as f64 range values"
)]
fn range_seek_matches_scan_at_fractional_and_negative_bounds() {
    // The seek widens Int64/DateTime bounds to the nearest integer; verify the
    // exact filter still removes the widened extras and never drops a match.
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::single());
    for n in -6i64..=6 {
        lmpg.create_record(
            "R",
            None,
            vec![
                ("n".into(), FieldChange::Set(json!(n))),
                ("f".into(), FieldChange::Set(json!(n as f64 / 2.0))),
            ],
        )
        .unwrap();
    }
    for bound in [
        json!(-3),
        json!(-3.5),
        json!(-0.5),
        json!(0),
        json!(0.5),
        json!(2.25),
        json!(5.5),
        json!(6),
    ] {
        for op in ["$gt", "$gte", "$lt", "$lte"] {
            assert_paths(
                &lmpg,
                &json!({"labels": ["R"], "where": {"n": {op: bound.clone()}}}),
                &format!("n {op} {bound}"),
            );
            assert_paths(
                &lmpg,
                &json!({"labels": ["R"], "where": {"f": {op: bound.clone()}}}),
                &format!("f {op} {bound}"),
            );
        }
    }
}

#[test]
fn bounded_exists_count_plan_matches_the_scan_page_and_total() {
    // Single unscoped `$exists` with a finite limit uses the maintained
    // owner count for `total` and a bounded presence merge for the page.
    let lmpg = setup(0xB0A2D);
    for (skip, limit) in [(0u64, 1u64), (0, 7), (5, 7), (200, 50), (0, 1000)] {
        let document = json!({
            "where": {"n": {"$exists": true}},
            "skip": skip,
            "limit": limit
        });
        assert_paths(
            &lmpg,
            &document,
            &format!("exists skip={skip} limit={limit}"),
        );
    }
}

#[test]
fn label_scoped_exists_count_plan_matches_the_scan_page_and_total() {
    // Label-scoped single `$exists` uses the maintained per-label counter for
    // `total` and a label-filtered bounded presence merge for the page.
    let lmpg = setup(0x1ABE1);
    for labels in [vec!["A"], vec!["B"], vec!["A", "B"]] {
        for (skip, limit) in [(0u64, 1u64), (0, 5), (3, 4), (0, 1000)] {
            let document = json!({
                "labels": labels,
                "where": {"n": {"$exists": true}},
                "skip": skip,
                "limit": limit
            });
            assert_paths(
                &lmpg,
                &document,
                &format!("exists labels={labels:?} skip={skip} limit={limit}"),
            );
        }
    }
}

#[test]
fn id_predicates_resolve_through_the_id_index_like_the_scan() {
    // `$id` and `$id: {$in: [...]}` must serve from the record-id index
    // (selected via `IndexPlan.exact`) and agree with a forced full scan.
    let lmpg = setup(0x001D_5EED);
    let all = find_records(&lmpg, &parse_document(&json!({})).unwrap().search).unwrap();
    let ids: Vec<String> = all
        .records
        .iter()
        .filter_map(|r| r["__id"].as_str().map(str::to_string))
        .collect();
    assert!(ids.len() > 20, "fixture should have records");

    // Single id (hit and miss), including a dangling id.
    assert_paths(&lmpg, &json!({"where": {"$id": ids[3]}}), "id hit");
    assert_paths(
        &lmpg,
        &json!({"where": {"$id": "00000000000000000000000000000000"}}),
        "id miss",
    );
    // `$in` with hits, a duplicate, and a dangling id.
    let subset: Vec<String> = ids.iter().step_by(7).take(20).cloned().collect();
    let mut with_dangling = subset.clone();
    with_dangling.push("ffffffffffffffffffffffffffffffff".into());
    with_dangling.push(subset[0].clone());
    assert_paths(
        &lmpg,
        &json!({"where": {"$id": {"$in": with_dangling}}}),
        "id in",
    );
    // Id union combined with another indexed conjunct.
    assert_paths(
        &lmpg,
        &json!({"where": {"$id": {"$in": subset}, "flag": true}}),
        "id in + flag",
    );
}

#[test]
fn count_only_aggregate_equals_the_scan_count() {
    // `select: {$count: "*"}` must equal the matched-root total of the same
    // filter, whether the count is served by the label catalog, the index, or
    // the ordinal matcher.
    let lmpg = setup(0xC0DE);
    let cases = [
        json!({}),
        json!({"labels": ["A"]}),
        json!({"labels": ["A", "B"]}),
        json!({"where": {"n": 3}}),
        json!({"where": {"n": {"$gte": 10}}}),
        json!({"labels": ["A"], "where": {"flag": true}}),
        json!({"where": {"nope": 1}}),
    ];
    for (i, case) in cases.iter().enumerate() {
        let mut doc = case.clone();
        doc["select"] = json!({"total": {"$count": "*"}});
        let parsed = parse_document(&doc).unwrap();
        let expected = find_records(&lmpg, &parsed.search).unwrap().total_matched;
        let aggregate = parsed.aggregate.expect("select produces an aggregate");
        let (buckets, total) =
            rushdb_query::aggregate_with_cancel(&lmpg, &aggregate, &|| false).unwrap();
        assert_eq!(total, expected, "case {i}: {case}");
        if expected == 0 {
            assert!(
                buckets.is_empty(),
                "case {i}: zero matches yields no bucket"
            );
        } else {
            assert_eq!(buckets.len(), 1, "case {i}");
            assert_eq!(buckets[0].count_roots, expected, "case {i}: {case}");
        }
    }
}

#[test]
#[expect(
    clippy::cast_precision_loss,
    reason = "the fixture stores a small random integer divided by four as an f64 property value"
)]
fn randomized_predicates_never_diverge_from_the_scan() {
    let lmpg = setup(0x00C0_FFEE);
    let fields = ["n", "f", "s", "flag", "arr", "nv"];
    let mut state = 0x1234_5678_9abc_def0u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    for case in 0..400 {
        let leaves: usize = 1 + (next() % 3) as usize;
        let mut where_obj = serde_json::Map::new();
        for _ in 0..leaves {
            let field = fields[usize::try_from(next() % fields.len() as u64).unwrap_or(0)];
            let value = match field {
                "n" => json!(next() % 20),
                "f" => json!((next() % 100) as f64 / 4.0),
                "s" => json!(format!("v{}", next() % 7)),
                "flag" => json!(next() % 2 == 0),
                "arr" => json!(next() % 5),
                _ => json!(Value::Null),
            };
            let predicate = match next() % 6 {
                0 => value,
                1 => json!({"$ne": value}),
                2 => json!({"$in": [value]}),
                3 => json!({"$gte": (next() % 20)}),
                4 => json!({"$exists": true}),
                _ => json!({"$contains": "v"}),
            };
            where_obj.insert(field.to_string(), predicate);
        }
        let mut document = json!({"where": where_obj});
        if next() % 4 == 0 {
            document["labels"] = json!(["A"]);
        }
        assert_paths(&lmpg, &document, &format!("random {case}"));
    }
}

/// A range leaf now gets a bounded histogram estimate from
/// `leaf_estimate` (feeding the planner's own leaf-intersection order), but
/// `explain_cardinality` must still report `None` for it — the field is
/// contractually exact — while the separate `explain_range_selectivity`
/// channel reports a bounded `[0.0, 1.0]` fraction. The fixture's `n` field
/// is `next() % 20`, so `$gte: 10` is neither the full set nor empty.
#[test]
fn range_leaf_gets_an_estimate_but_never_through_the_exact_explain_channel() {
    let lmpg = setup(42);
    let view = lmpg.store.read_view().unwrap();
    let expr = rushdb_query::document::parse_where(&json!({"n": {"$gte": 10}})).unwrap();

    let exact = rushdb_query::explain_cardinality(&view, lmpg.tenant, &[], &expr).unwrap();
    assert_eq!(
        exact, None,
        "a lone range leaf has no exact index-eligible cardinality"
    );

    let estimate = rushdb_query::explain_range_selectivity(&view, lmpg.tenant, &expr)
        .unwrap()
        .expect("a usable histogram exists for `n`");
    assert!(
        (0.0..=1.0).contains(&estimate),
        "selectivity estimate must be a fraction, got {estimate}"
    );

    // The differential oracle: the index path (now using the histogram
    // estimate for leaf ordering) must still match a forced full scan.
    assert_paths(&lmpg, &json!({"where": {"n": {"$gte": 10}}}), "pln1 gte");
}

/// Range estimates load the value distribution once into the store's
/// planner-statistics cache and serve later plans from it.
#[test]
fn range_estimates_load_once_into_the_planner_stats_cache() {
    let lmpg = setup(42);
    let expr = rushdb_query::document::parse_where(&json!({"n": {"$gte": 10}})).unwrap();
    assert_eq!(lmpg.store.planner_stats().stats().entries, 0);
    let first = {
        let view = lmpg.store.read_view().unwrap();
        rushdb_query::explain_range_selectivity(&view, lmpg.tenant, &expr).unwrap()
    };
    let loaded = lmpg.store.planner_stats().stats();
    assert_eq!(loaded.entries, 1, "{loaded:?}");
    assert!(loaded.bytes > 0);
    let second = {
        let view = lmpg.store.read_view().unwrap();
        rushdb_query::explain_range_selectivity(&view, lmpg.tenant, &expr).unwrap()
    };
    assert_eq!(first, second);
    let after = lmpg.store.planner_stats().stats();
    assert_eq!(after.entries, 1);
    assert!(after.hits > loaded.hits, "the second plan hit the cache");
}

/// Distributions for 500 properties stay within the cache budget
/// (LRU under a tight one), and an idle sweep empties the cache.
#[test]
fn planner_stats_for_500_properties_stay_in_budget_and_evict_when_idle() {
    fn run(budget: usize) -> rushdb_storage::cache::CacheStats {
        let store = Arc::new(
            Store::new_with_resources(rushdb_storage::resources::StorageResources {
                planner_stats_bytes: budget,
                planner_stats_idle_seconds: 0,
                ..rushdb_storage::resources::StorageResources::default()
            })
            .unwrap(),
        );
        let lmpg = Lmpg::new(store, TenantId::from_ordinal(500));
        let mut batch = lmpg.begin_batch();
        for i in 0..2_000u64 {
            batch.ops.push(rushdb_lmpg::BatchOp::CreateRecord {
                label: "Wide".into(),
                explicit_id: None,
                fields: vec![(format!("p{}", i % 500), FieldChange::Set(json!(i)))],
            });
        }
        lmpg.run_batch(batch).unwrap();
        let view = lmpg.store.read_view().unwrap();
        for p in 0..500 {
            let expr =
                rushdb_query::document::parse_where(&json!({format!("p{p}"): {"$gte": 1000}}))
                    .unwrap();
            rushdb_query::explain_range_selectivity(&view, lmpg.tenant, &expr)
                .unwrap()
                .expect("an estimate");
        }
        let loaded = lmpg.store.planner_stats().stats();
        assert!(loaded.bytes <= budget as u64, "{loaded:?} over {budget}");
        lmpg.store.planner_stats().evict_idle();
        let idle = lmpg.store.planner_stats().stats();
        assert_eq!(
            (idle.entries, idle.bytes),
            (0, 0),
            "idle sweep empties the cache"
        );
        loaded
    }
    let roomy = run(32 * 1024 * 1024);
    assert_eq!(roomy.entries, 500, "{roomy:?}");
    eprintln!(
        "500 cached properties: {} bytes ({} per entry)",
        roomy.bytes,
        roomy.bytes / roomy.entries
    );
    let tight = run(64 * 1024);
    assert!(tight.evictions > 0 && tight.entries < 500, "{tight:?}");
}

fn declare_ready(lmpg: &Lmpg, label: &str, property: &str) {
    let def = lmpg
        .define_index(
            &format!("{label}_{property}"),
            label,
            property,
            rushdb_types::index::IndexKind::Ordered,
        )
        .unwrap();
    while lmpg.index_step(def.id, 64).unwrap() != rushdb_lmpg::index::IndexStep::Finished {}
}

/// With declared ordered indexes the indexed plan equals the scan for
/// equality, `$in`, ranges (int, float, fractional and negative bounds) and
/// string prefix, label-scoped, across arrays and mixed labels.
#[test]
fn ordered_index_answers_match_scan_oracle() {
    let lmpg = setup(0xF174);
    for label in ["A", "B"] {
        for property in ["n", "f", "s", "arr"] {
            declare_ready(&lmpg, label, property);
        }
    }
    let view = lmpg.store.read_view().unwrap();
    let explained = rushdb_query::explain_index(
        &view,
        lmpg.tenant,
        &["A".into()],
        &rushdb_query::document::parse_where(&json!({"n": 3})).unwrap(),
    )
    .unwrap();
    assert_eq!(explained.as_deref(), Some("A_n"));
    let cases = [
        json!({"n": 3}),
        json!({"n": {"$in": [1, 5, 9, 99]}}),
        json!({"n": {"$gte": 10}}),
        json!({"n": {"$gt": 9.5}}),
        json!({"n": {"$lt": 4}}),
        json!({"n": {"$lte": -1}}),
        json!({"f": {"$gt": 12.5}}),
        json!({"f": {"$lte": 0.25}}),
        json!({"f": 0}),
        json!({"s": "v2"}),
        json!({"s": {"$startsWith": "v"}}),
        json!({"s": {"$startsWith": "v3"}}),
        json!({"s": {"$startsWith": "w"}}),
        json!({"arr": 2}),
        json!({"arr": {"$gte": 3}}),
        json!({"n": {"$gte": 5}, "s": "v2"}),
        json!({"n": {"$in": [1, 2, 3]}, "flag": true}),
    ];
    for labels in [vec!["A"], vec!["B"], vec!["A", "B"]] {
        for case in &cases {
            let mut document = json!({"where": case});
            document["labels"] = json!(labels);
            assert_paths(&lmpg, &document, &format!("{labels:?}"));
        }
    }
}

#[test]
fn planner_uses_only_ready_indexes() {
    let lmpg = setup(0xF175);
    let def = lmpg
        .define_index("A_n", "A", "n", rushdb_types::index::IndexKind::Ordered)
        .unwrap();
    let expr = rushdb_query::document::parse_where(&json!({"n": 3})).unwrap();
    let explain = |lmpg: &Lmpg| {
        let view = lmpg.store.read_view().unwrap();
        rushdb_query::explain_index(&view, lmpg.tenant, &["A".into()], &expr).unwrap()
    };
    lmpg.index_step(def.id, 10).unwrap();
    assert_eq!(explain(&lmpg), None, "a building index is not a candidate");
    assert_paths(
        &lmpg,
        &json!({"labels": ["A"], "where": {"n": 3}}),
        "building",
    );
    while lmpg.index_step(def.id, 64).unwrap() != rushdb_lmpg::index::IndexStep::Finished {}
    assert_eq!(explain(&lmpg).as_deref(), Some("A_n"));
    // An index on one label does not serve a query over two.
    let view = lmpg.store.read_view().unwrap();
    assert_eq!(
        rushdb_query::explain_index(&view, lmpg.tenant, &["A".into(), "B".into()], &expr).unwrap(),
        None
    );
    drop(view);
    lmpg.drop_index("A_n").unwrap();
    assert_eq!(explain(&lmpg), None, "a dropping index is not a candidate");
}
