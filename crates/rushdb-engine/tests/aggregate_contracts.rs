//! Independent quality audit probes.
use rushdb_engine::{Engine, OperationClass};
use rushdb_query::{AggregateRequest, QueryError};
use serde_json::json;

#[test]
fn aggregate_charges_work_even_when_pagination_discards_result() {
    let engine = Engine::new_in_memory();
    for i in 0..20 {
        engine
            .import_document(
                None,
                "Audit",
                &json!({"group": i, "payload": "x".repeat(4096)}),
            )
            .unwrap();
    }
    let mut context = engine
        .context(None, OperationClass::Read, None, Some(2))
        .unwrap();
    let result = engine.aggregate_with_context(
        &mut context,
        &AggregateRequest {
            labels: vec!["Audit".into()],
            group_by: vec!["group".into()],
            skip: 20,
            ..Default::default()
        },
    );
    assert!(
        matches!(result, Err(QueryError::ResourceLimit(_))),
        "aggregate work bypassed 2-byte budget: {result:?}; charged={}",
        context.charged_bytes()
    );
}

#[test]
fn interleaved_group_keys_accumulate_in_matching_bucket() {
    let engine = Engine::new_in_memory();
    for (group, value) in [("a", 1), ("b", 10), ("a", 2)] {
        engine
            .import_document(None, "Audit", &json!({"group":group,"value":value}))
            .unwrap();
    }
    let buckets = engine
        .aggregate(
            None,
            &AggregateRequest {
                labels: vec!["Audit".into()],
                group_by: vec!["group".into()],
                sums: vec![("total".into(), "value".into())],
                ..Default::default()
            },
        )
        .unwrap();
    let a = buckets
        .iter()
        .find(|bucket| bucket.keys["group"] == "a")
        .unwrap();
    assert_eq!(a.sums["total"], json!(3.0), "buckets={buckets:?}");
}

#[test]
fn aggregation_preserves_all_metrics_for_interleaved_groups() {
    let engine = Engine::new_in_memory();
    for (group, value) in [("a", 4), ("b", 100), ("a", 2), ("b", 10), ("a", 4)] {
        engine
            .import_document(None, "Metrics", &json!({"group": group, "value": value}))
            .unwrap();
    }
    let buckets = engine
        .aggregate(
            None,
            &AggregateRequest {
                labels: vec!["Metrics".into()],
                group_by: vec!["group".into()],
                sums: vec![("sum".into(), "value".into())],
                counts: vec![("count".into(), "value".into())],
                mins: vec![("min".into(), "value".into())],
                maxs: vec![("max".into(), "value".into())],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(buckets.len(), 2);
    for (group, count, sum, min, max) in [("a", 3, 10.0, 2.0, 4.0), ("b", 2, 110.0, 10.0, 100.0)] {
        let bucket = buckets.iter().find(|b| b.keys["group"] == group).unwrap();
        assert_eq!(bucket.count_roots, count);
        assert_eq!(bucket.sums["sum"].as_f64(), Some(sum));
        assert_eq!(bucket.counts["count"].as_u64(), Some(count));
        assert_eq!(bucket.mins["min"].as_f64(), Some(min));
        assert_eq!(bucket.maxs["max"].as_f64(), Some(max));
    }
}

#[test]
fn aggregate_rejects_large_record_before_discarding_output() {
    let engine = Engine::new_in_memory();
    engine
        .import_document(
            None,
            "Large",
            &json!({"group": "a", "payload": "x".repeat(128 * 1024)}),
        )
        .unwrap();
    let mut context = engine
        .context(None, OperationClass::Read, None, Some(64 * 1024))
        .unwrap();
    let result = engine.aggregate_with_context(
        &mut context,
        &AggregateRequest {
            labels: vec!["Large".into()],
            group_by: vec!["group".into()],
            skip: 1,
            ..Default::default()
        },
    );
    assert!(
        matches!(result, Err(QueryError::ResourceLimit(_))),
        "{result:?}"
    );
}

#[test]
fn aggregate_reuses_temporary_record_budget() {
    let engine = Engine::new_in_memory();
    for _ in 0..100 {
        engine
            .import_document(
                None,
                "Repeat",
                &json!({"value": 1, "payload": "x".repeat(1024)}),
            )
            .unwrap();
    }
    let mut context = engine
        .context(None, OperationClass::Read, None, Some(256 * 1024))
        .unwrap();
    let result = engine
        .aggregate_with_context(
            &mut context,
            &AggregateRequest {
                labels: vec!["Repeat".into()],
                sums: vec![("sum".into(), "value".into())],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result[0].sums["sum"], json!(100.0));
}

#[test]
fn aggregate_limits_retained_groups_even_when_skip_discards_them() {
    let engine = Engine::new_in_memory();
    for i in 0..100 {
        engine
            .import_document(
                None,
                "Groups",
                &json!({"group": format!("{i}{}", "x".repeat(1024))}),
            )
            .unwrap();
    }
    let mut context = engine
        .context(None, OperationClass::Read, None, Some(64 * 1024))
        .unwrap();
    let result = engine.aggregate_with_context(
        &mut context,
        &AggregateRequest {
            labels: vec!["Groups".into()],
            group_by: vec!["group".into()],
            skip: 100,
            ..Default::default()
        },
    );
    assert!(
        matches!(result, Err(QueryError::ResourceLimit(_))),
        "{result:?}"
    );
}
