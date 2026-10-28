use super::*;
use crate::equality::strings_equal;
use crate::planner::*;

/// Test-only convenience over [`index_candidates_with_budget`] (no budget, no
/// cancellation) — the production paths now always carry an explicit budget
/// and cancellation probe, so the unparameterized form only exists for these
/// planner unit tests.
fn index_candidates(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    req: &SearchRequest,
) -> Result<Option<IndexPlan>, QueryError> {
    index_candidates_with_budget(lmpg, snap, req, None, &|| false)
}

#[test]
fn aggregate_numeric_conversion_rejects_lossy_integers() {
    assert_eq!(exact_f64(&serde_json::json!(42i64)), Some(42.0));
    assert_eq!(
        exact_f64(&serde_json::json!(9_007_199_254_740_993i64)),
        None
    );
}

#[test]
fn equality_preserves_large_integer_precision() {
    let exact = serde_json::json!(9_007_199_254_740_993i64);
    let rounded = serde_json::json!(9_007_199_254_740_992.0f64);
    assert!(!strings_equal(&exact, &rounded));
    assert!(strings_equal(
        &serde_json::json!(42),
        &serde_json::json!(42.0)
    ));
}

#[test]
fn equality_index_variants_do_not_round_large_integers() {
    let variants = scalar_variants(&serde_json::json!(9_007_199_254_740_993i64))
        .expect("integer is indexable");
    assert!(variants
        .iter()
        .any(|(_, scalar)| matches!(scalar, Scalar::Int64(9_007_199_254_740_993))));
    assert!(!variants
        .iter()
        .any(|(_, scalar)| matches!(scalar, Scalar::Float64(_))));
}

#[test]
fn equality_index_variants_do_not_saturate_out_of_domain_float() {
    let variants = scalar_variants(&serde_json::json!(9_223_372_036_854_775_808.0f64))
        .expect("finite float remains indexable");
    assert!(!variants
        .iter()
        .any(|(_, scalar)| matches!(scalar, Scalar::Int64(i64::MAX))));
}

#[test]
fn range_parser_rejects_imprecise_integer_bounds() {
    let exact = crate::document::parse_document(&serde_json::json!({
        "where": {"n": {"$gt": 9_007_199_254_740_992i64}}
    }));
    assert!(exact.is_ok());
    let imprecise = crate::document::parse_document(&serde_json::json!({
        "where": {"n": {"$gt": 9_007_199_254_740_993i64}}
    }));
    let Err(error) = imprecise else {
        panic!("imprecise bound was accepted")
    };
    assert!(error.to_string().contains("not exactly representable"));
}

#[test]
fn derived_numeric_literals_reject_lossy_integer_widening() {
    let result = crate::document::parse_document(&serde_json::json!({
        "select": {
            "value": {"$add": [9_007_199_254_740_993i64, 1]}
        }
    }));
    let Err(error) = result else {
        panic!("derived literal must not round through f64")
    };
    assert!(error.to_string().contains("not exactly representable"));
}

#[test]
fn range_index_comparison_preserves_large_integer_ordering() {
    let value = Scalar::Int64(9_007_199_254_740_993);
    let bound = 9_007_199_254_740_992.0;
    assert!(range_scalar_matches(&value, RangeOp::Gt, bound));
    assert!(!range_scalar_matches(&value, RangeOp::Lte, bound));
}

#[test]
fn scan_numeric_comparison_preserves_large_integer_ordering() {
    let value = serde_json::json!(9_007_199_254_740_993i64);
    assert!(numeric_cmp(
        Some(&value),
        9_007_199_254_740_992.0,
        |ordering| { ordering == std::cmp::Ordering::Greater }
    ));
}

#[test]
fn aggregate_minmax_preserves_large_integer_json_values() {
    let mut bucket = GroupBucket {
        keys: Map::new(),
        count_roots: 0,
        sums: Map::new(),
        counts: Map::new(),
        mins: Map::new(),
        maxs: Map::new(),
        derived_outputs: std::collections::BTreeMap::new(),
    };
    let first = serde_json::json!(9_007_199_254_740_993i64);
    let rounded = serde_json::json!(9_007_199_254_740_992.0f64);
    minmax(&mut bucket, "value".into(), &rounded, false);
    minmax(&mut bucket, "value".into(), &first, false);
    assert_eq!(bucket.maxs["value"], first);
}

#[test]
fn derived_field_expression_rejects_lossy_integer_widening() {
    let mut row = Map::new();
    row.insert("value".into(), serde_json::json!(9_007_199_254_740_993i64));
    let expr = OutputExpr::Field("value".into());
    assert_eq!(eval_row_expr(&expr, &row), None);
}

fn lmpg() -> Lmpg {
    Lmpg::new(
        Arc::new(rushdb_storage::Store::new_in_memory()),
        rushdb_types::ids::TenantId::from_ordinal(1),
    )
}

/// An engine whose query working set is capped at `cap` bytes
/// (`query-memory-max`).
fn lmpg_with_cap(cap: u64) -> Lmpg {
    Lmpg::new(
        Arc::new(
            rushdb_storage::Store::new_with_resources(
                rushdb_storage::resources::StorageResources {
                    query_memory_max_bytes: cap,
                    ..rushdb_storage::resources::StorageResources::default()
                },
            )
            .unwrap(),
        ),
        rushdb_types::ids::TenantId::from_ordinal(1),
    )
}

fn request(lmpg: &Lmpg, where_: &serde_json::Value) -> SearchRequest {
    let parsed = crate::document::parse_document(&serde_json::json!({ "where": where_.clone() }))
        .expect("valid document");
    let _ = lmpg;
    parsed.search
}

#[test]
fn eligible_returns_plan_residual_only_returns_none_and_force_scan_bypasses() {
    let l = lmpg();
    l.create_record(
        "A",
        None,
        vec![(
            "n".to_string(),
            rushdb_lmpg::plan::FieldChange::Set(serde_json::json!(1)),
        )],
    )
    .unwrap();
    let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());

    // Eligible-only: exact plan with the matching owner.
    let plan = index_candidates(&l, &snap, &request(&l, &serde_json::json!({"n": 1})))
        .unwrap()
        .expect("index plan");
    assert!(plan.exact);
    assert_eq!(plan.ords.len(), 1);

    // Eligible + residual: partial prefilter, not exact.
    let plan = index_candidates(
        &l,
        &snap,
        &request(&l, &serde_json::json!({"n": 1, "s": {"$contains": "x"}})),
    )
    .unwrap()
    .expect("partial plan");
    assert!(!plan.exact);
    assert_eq!(plan.ords.len(), 1);

    // Residual only: no plan.
    assert!(index_candidates(
        &l,
        &snap,
        &request(&l, &serde_json::json!({"n": {"$ne": 1}}))
    )
    .unwrap()
    .is_none());

    // Escape hatch for the differential oracle.
    let mut forced = request(&l, &serde_json::json!({"n": 1}));
    forced.force_scan = true;
    assert!(index_candidates(&l, &snap, &forced).unwrap().is_none());

    // Bounded presence merge equals the first k of the full presence set.
    for extra in 0..20u64 {
        l.create_record(
            "A",
            None,
            vec![(
                "m".to_string(),
                rushdb_lmpg::plan::FieldChange::Set(serde_json::json!(extra)),
            )],
        )
        .unwrap();
    }
    let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());
    let full = owners_with_presence(&snap, l.tenant, "m", false, &[], usize::MAX)
        .unwrap()
        .expect("presence");
    let expected: Vec<u64> = full.iter().take(7).collect();
    let bounded = owners_with_presence_bounded(&snap, l.tenant, "m", &[], 7).unwrap();
    assert_eq!(bounded, expected);
    assert_eq!(total_support(&snap, l.tenant, "m", &[]).unwrap(), Some(20));

    // Label-scoped presence counts only the requested labels.
    l.create_record(
        "B",
        None,
        vec![(
            "m".to_string(),
            rushdb_lmpg::plan::FieldChange::Set(serde_json::json!(0)),
        )],
    )
    .unwrap();
    let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());
    let only_a = ["A".to_string()];
    assert_eq!(total_support(&snap, l.tenant, "m", &[]).unwrap(), Some(21));
    assert_eq!(
        total_support(&snap, l.tenant, "m", &only_a).unwrap(),
        Some(20)
    );
    assert_eq!(
        owners_with_presence_bounded(&snap, l.tenant, "m", &only_a, usize::MAX)
            .unwrap()
            .len(),
        20
    );

    // Range planning.
    let plan = index_candidates(
        &l,
        &snap,
        &request(&l, &serde_json::json!({"n": {"$gte": 1}})),
    )
    .unwrap()
    .expect("range plan");
    assert_eq!(plan.ords.len(), 1);
    let plan = index_candidates(
        &l,
        &snap,
        &request(&l, &serde_json::json!({"n": {"$gt": 1}})),
    )
    .unwrap()
    .expect("range plan");
    assert!(plan.ords.is_empty());
}

#[test]
fn related_traversal_honors_cancellation() {
    // Q-6r: predicate traversal (`eval_related`) must observe the cancellation
    // probe, not run to completion after a stop is requested.
    let store = std::sync::Arc::new(rushdb_storage::Store::new_in_memory());
    let tenant = rushdb_types::ids::TenantId::from_ordinal(1);
    let lmpg = rushdb_lmpg::Lmpg::new(store, tenant);
    let a = lmpg.create_record("A", None, vec![]).unwrap();
    let b = lmpg.create_record("B", None, vec![]).unwrap();
    lmpg.create_relationship("R", a, b, vec![]).unwrap();

    let snap = std::sync::Arc::new(lmpg.store.read_view().unwrap());
    let cancelled = || true;
    let ctx = EvalCtx {
        lmpg: &lmpg,
        snap,
        is_cancelled: Some(&cancelled),
        cycles: crate::cycle::CycleGraphs::default(),
        declared_types: std::cell::RefCell::default(),
    };
    let expr = Expr::Related(RelatedExpr {
        target_labels: Vec::new(),
        alias: None,
        rel_type: Some("R".into()),
        direction: Direction::Outgoing,
        hops: Some((1, 1)),
        cycle: false,
        filter: None,
    });
    let id_hex = a.record_id.iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
        out
    });
    let mut root = serde_json::Map::new();
    root.insert("__id".into(), serde_json::json!(id_hex));
    assert!(matches!(
        eval_expr_ctx(&ctx, &expr, &root),
        Err(QueryError::Cancelled)
    ));
}

/// `count` records labelled `A` with `fields` (`seq` added per record).
fn seed(l: &Lmpg, count: u64, fields: &dyn Fn(u64) -> serde_json::Value) {
    for seq in 0..count {
        let mut changes = vec![(
            "seq".to_string(),
            rushdb_lmpg::plan::FieldChange::Set(serde_json::json!(seq)),
        )];
        for (name, value) in fields(seq).as_object().unwrap() {
            changes.push((
                name.clone(),
                rushdb_lmpg::plan::FieldChange::Set(value.clone()),
            ));
        }
        l.create_record("A", None, changes).unwrap();
    }
}

#[test]
fn a_broad_leaf_is_held_as_a_bitmap_within_the_budget() {
    let l = lmpg_with_cap(128 * 1024);
    seed(&l, 5_000, &|_| serde_json::json!({"s": "a"}));
    let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());
    let req = request(&l, &serde_json::json!({"s": "a"}));
    // 128 KiB holds the 5K-owner bitmap (32K owners at 4 B; 256 at the old
    // 512 B) and the 8-byte candidate and match ordinals behind it.
    let ords = matching_ordinals(&l, &req, &|| false, Arc::clone(&snap)).unwrap();
    assert_eq!(ords.len(), 5_000);
    // The budget still binds: 4 KiB holds 1K owners.
    assert!(matches!(
        index_candidates_with_budget(&l, &snap, &req, Some(4 * 1024), &|| false),
        Err(QueryError::ResourceLimit(_))
    ));
}

#[test]
fn later_leaves_are_checked_on_the_candidates_earlier_leaves_left() {
    let l = lmpg();
    seed(&l, 2_000, &|seq| {
        if seq % 200 == 0 {
            serde_json::json!({"s": "a", "t": "x"})
        } else if seq % 2 == 0 {
            serde_json::json!({"s": "a", "t": "y"})
        } else {
            serde_json::json!({"s": "b", "t": "y"})
        }
    });
    // A record with `t = x` but `s = b` must not survive the second leaf.
    l.create_record(
        "A",
        None,
        vec![
            (
                "s".to_string(),
                rushdb_lmpg::plan::FieldChange::Set(serde_json::json!("b")),
            ),
            (
                "t".to_string(),
                rushdb_lmpg::plan::FieldChange::Set(serde_json::json!("x")),
            ),
        ],
    )
    .unwrap();
    let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());
    let req = request(&l, &serde_json::json!({"s": "a", "t": "x"}));
    // `t = x` (11 owners) is materialized first; `s = a` is then checked on
    // those 11 candidates instead of walking its 1,000-owner population,
    // and the plan stays exact.
    // Accounting is switched on and left on: other tests in this binary may
    // flip the global flag, and this thread's own tally is what is checked.
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    let plan = index_candidates(&l, &snap, &req).unwrap().expect("plan");
    let counts = rushdb_storage::read_accounting::take_local();
    assert!(plan.exact);
    assert_eq!(plan.ords.len(), 10);
    let occurrence_reads: u64 = counts
        .iter()
        .filter(|((_, family), _)| family == "occurrence")
        .map(|(_, count)| count)
        .sum();
    assert!(
        occurrence_reads < 2_000,
        "walked a population instead of the candidates: {occurrence_reads} occurrence reads"
    );
    assert_eq!(
        matching_ordinals(&l, &req, &|| false, snap).unwrap(),
        plan.ords.into_iter().collect::<Vec<u64>>()
    );
}

#[test]
fn records_checked_by_the_predicate_are_not_summed_into_the_working_set() {
    let padding = "p".repeat(200);
    let run = |cap: u64| {
        let l = lmpg_with_cap(cap);
        seed(
            &l,
            2_000,
            &|_| serde_json::json!({"s": "a", "pad": padding}),
        );
        let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());
        // `$ne` is residual: every record is rebuilt and checked, ~0.5 MB in all.
        let req = request(&l, &serde_json::json!({"s": {"$ne": "b"}}));
        assert!(index_candidates(&l, &snap, &req).unwrap().is_none());
        matching_ordinals(&l, &req, &|| false, snap)
    };
    // 20 KiB holds the 2K match ordinals (16 KB) plus one record; the
    // candidates are streamed, not held.
    assert_eq!(run(20 * 1024).unwrap().len(), 2_000);
    // The match ordinals alone must still fit.
    assert!(matches!(run(8 * 1024), Err(QueryError::ResourceLimit(_))));
}

#[test]
fn ordering_values_read_by_point_order_like_the_rebuilt_records() {
    // READ-1: an ordered page reads each match's ordering fields with point
    // reads instead of rebuilding it. Mixed types, nulls and missing values
    // must order exactly as the rebuilt records do, across the cuts that
    // keep only the best `offset + limit` rows (200 matches, cut at 64).
    let l = lmpg();
    seed(&l, 200, &|seq| match seq % 6 {
        0 => serde_json::json!({"k": seq % 7}),
        1 => serde_json::json!({"k": f64::from(u32::try_from(seq % 5).unwrap()) + 0.5}),
        2 => serde_json::json!({"k": format!("s{}", seq % 4)}),
        3 => serde_json::json!({"k": seq % 2 == 0}),
        4 => serde_json::json!({"k": null}),
        _ => serde_json::json!({}),
    });
    let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());
    let all = find_records_in_view(&l, &SearchRequest::default(), &|| false, Arc::clone(&snap))
        .unwrap()
        .records;
    for order in [
        vec![("k".to_string(), false)],
        vec![("k".to_string(), true), ("seq".to_string(), true)],
    ] {
        let mut rows: Vec<(OrderRow, ())> = all
            .iter()
            .map(|record| {
                let row = record.as_object().unwrap().clone();
                let seq = row["seq"].as_u64().unwrap();
                ((seq, row), ())
            })
            .collect();
        let req = SearchRequest {
            order_by: order,
            pagination: Pagination {
                skip: 7,
                limit: Some(20),
            },
            ..SearchRequest::default()
        };
        sort_ordered_rows(&mut rows, &req);
        let expected: Vec<u64> = rows
            .iter()
            .skip(7)
            .take(20)
            .map(|((seq, _), ())| *seq)
            .collect();
        let page = find_records_in_view(&l, &req, &|| false, Arc::clone(&snap)).unwrap();
        let got: Vec<u64> = page
            .records
            .iter()
            .map(|record| record["seq"].as_u64().unwrap())
            .collect();
        assert_eq!(got, expected, "{:?}", req.order_by);
        assert_eq!(page.total_matched, 200);
    }
}

#[test]
fn a_page_stops_after_its_rows_when_the_total_is_known() {
    // READ-1: an unfiltered or label-only page streams candidates in ordinal
    // order, takes its rows, and reads the total from counters instead of
    // walking the rest (the probe runs once per candidate visited).
    let l = lmpg();
    seed(&l, 300, &|_| serde_json::json!({}));
    let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());
    for labels in [vec![], vec!["A".to_string()]] {
        let probes = std::cell::Cell::new(0u32);
        let req = SearchRequest {
            labels,
            pagination: Pagination {
                skip: 3,
                limit: Some(5),
            },
            ..SearchRequest::default()
        };
        let page = find_records_in_view(
            &l,
            &req,
            &|| {
                probes.set(probes.get() + 1);
                false
            },
            Arc::clone(&snap),
        )
        .unwrap();
        assert_eq!(page.total_matched, 300);
        let seqs: Vec<u64> = page
            .records
            .iter()
            .map(|record| record["seq"].as_u64().unwrap())
            .collect();
        assert_eq!(seqs, vec![3, 4, 5, 6, 7]);
        assert!(probes.get() < 40, "visited {} candidates", probes.get());
    }
}

#[test]
fn predicates_read_by_point_match_the_rebuilt_records() {
    // READ-1: a residual predicate reads only the properties it names, by
    // point reads, instead of rebuilding each candidate. Every operator must
    // answer as it does on the rebuilt record, over mixed types, nulls,
    // missing values, and an array property (which takes the rebuild path).
    let l = lmpg();
    seed(&l, 120, &|seq| {
        let v = match seq % 6 {
            0 => serde_json::json!(seq % 7),
            1 => serde_json::json!(f64::from(u32::try_from(seq % 5).unwrap()) + 0.5),
            2 => serde_json::json!(format!("s{}", seq % 4)),
            3 => serde_json::json!(seq % 2 == 0),
            _ => serde_json::Value::Null,
        };
        let mut doc = serde_json::json!({"w": seq % 3});
        if seq % 6 != 5 {
            doc["v"] = v;
        }
        if seq % 4 == 0 {
            doc["tags"] = serde_json::json!(["a", format!("t{}", seq % 3)]);
        }
        doc
    });
    let snap: QuerySnapshot = Arc::new(l.store.read_view().unwrap());
    let all = find_records_in_view(&l, &SearchRequest::default(), &|| false, Arc::clone(&snap))
        .unwrap()
        .records;
    let ctx = EvalCtx::new(&l).unwrap();
    for predicate in [
        serde_json::json!({"v": {"$gt": 2}}),
        serde_json::json!({"v": {"$lte": 1.5}}),
        serde_json::json!({"v": null}),
        serde_json::json!({"v": {"$exists": false}}),
        serde_json::json!({"v": {"$ne": 3}}),
        serde_json::json!({"v": {"$in": [1, "s2", true]}}),
        serde_json::json!({"v": {"$nin": [0, 1]}}),
        serde_json::json!({"v": {"$type": "number"}}),
        serde_json::json!({"v": {"$contains": "s"}}),
        serde_json::json!({"v": {"$startsWith": "s1"}}),
        serde_json::json!({"$or": [{"v": {"$lt": 1}}, {"w": 2}]}),
        serde_json::json!({"$not": {"v": {"$gte": 3}}}),
        serde_json::json!({"tags": "a", "v": {"$exists": true}}),
    ] {
        let req = request(&l, &predicate);
        let expected: Vec<u64> = all
            .iter()
            .filter(|record| {
                eval_expr_ctx(
                    &ctx,
                    req.where_.as_ref().unwrap(),
                    record.as_object().unwrap(),
                )
                .unwrap()
            })
            .map(|record| record["seq"].as_u64().unwrap())
            .collect();
        let page = find_records_in_view(&l, &req, &|| false, Arc::clone(&snap)).unwrap();
        let got: Vec<u64> = page
            .records
            .iter()
            .map(|record| record["seq"].as_u64().unwrap())
            .collect();
        assert_eq!(got, expected, "{predicate}");
        assert_eq!(page.total_matched, expected.len() as u64, "{predicate}");
    }
}
