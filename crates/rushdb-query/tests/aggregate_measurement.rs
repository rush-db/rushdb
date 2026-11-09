//! Measurement, not a gate: grouped aggregation over many records of one
//! label, the shapes of the benchmark suite's `aggregations` area. Run with
//! `cargo test -p rushdb-query --release --test aggregate_measurement -- --ignored --nocapture`
//! and `AGGREGATE_MEASUREMENT_RECORDS` (default 1,000,000),
//! `AGGREGATE_MEASUREMENT_THREADS` (default `1,8`) and
//! `AGGREGATE_MEASUREMENT_DIR` (a redb store in that empty directory).

use std::sync::Arc;
use std::time::Instant;

use rushdb_lmpg::Lmpg;
use rushdb_query::document::parse_document;
use rushdb_query::{aggregate_with_cancel_in_view, AggregateRequest};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

#[test]
#[ignore = "measurement: prints grouped aggregation latency at scale"]
#[expect(
    clippy::too_many_lines,
    reason = "the seven benchmark shapes are listed in one place, as the suite runs them"
)]
fn grouped_aggregation_latency_at_scale() {
    let records: u32 = std::env::var("AGGREGATE_MEASUREMENT_RECORDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    // `AGGREGATE_MEASUREMENT_DIR` names an empty directory for a redb store
    // (in memory without it).
    let store = Arc::new(match std::env::var("AGGREGATE_MEASUREMENT_DIR") {
        Ok(dir) => Store::open(&std::path::Path::new(&dir).join("store.redb")).unwrap(),
        Err(_) => Store::new_in_memory(),
    });
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    let options = rushdb_lmpg::import::ImportOptions::default();
    let started = Instant::now();
    for batch in 0..records.div_ceil(10_000) {
        let rows: Vec<serde_json::Value> = (batch * 10_000..((batch + 1) * 10_000).min(records))
            .map(|i| {
                let payment = ["card", "cash", "other"][(i % 3) as usize];
                json!({
                    "vendor": if i % 3 == 0 { "A" } else { "B" },
                    "payment": payment,
                    "pickup_hour": i % 24,
                    "pickup_zone": i % 263,
                    "dropoff_zone": (i * 7) % 263,
                    "passengers": i % 6 + 1,
                    "distance_km": f64::from(i % 300) / 10.0,
                    "fare": f64::from(i % 700) / 10.0 + 3.0,
                    "tip": f64::from(i % 50) / 10.0,
                })
            })
            .collect();
        let plan = rushdb_lmpg::import::serialize_bfs(&json!(rows), "trips", &options);
        rushdb_lmpg::import::import_bfs(&lmpg, &plan, &options).unwrap();
    }
    println!("seeded {records} records in {:.1?}", started.elapsed());

    let where_card = parse_document(&json!({
        "labels": ["trips"],
        "where": {"payment": "card", "passengers": {"$gte": 2}}
    }))
    .unwrap()
    .search
    .where_;
    let shapes: Vec<(&str, AggregateRequest)> = vec![
        (
            "groupBy payment: count",
            AggregateRequest {
                labels: vec!["trips".into()],
                group_by: vec!["payment".into()],
                counts: vec![("n".into(), "*".into())],
                ..Default::default()
            },
        ),
        (
            "groupBy pickup_zone (263): count",
            AggregateRequest {
                labels: vec!["trips".into()],
                group_by: vec!["pickup_zone".into()],
                counts: vec![("n".into(), "*".into())],
                ..Default::default()
            },
        ),
        (
            "groupBy payment: count + sum fare",
            AggregateRequest {
                labels: vec!["trips".into()],
                group_by: vec!["payment".into()],
                counts: vec![("n".into(), "*".into())],
                sums: vec![("fare".into(), "fare".into())],
                ..Default::default()
            },
        ),
        (
            "groupBy vendor: avg distance + max fare",
            AggregateRequest {
                labels: vec!["trips".into()],
                group_by: vec!["vendor".into()],
                avg_terms: vec![("dist".into(), "distance_km".into())],
                maxs: vec![("max_fare".into(), "fare".into())],
                ..Default::default()
            },
        ),
        (
            "groupBy pickup_hour: avg tip",
            AggregateRequest {
                labels: vec!["trips".into()],
                group_by: vec!["pickup_hour".into()],
                avg_terms: vec![("tip".into(), "tip".into())],
                ..Default::default()
            },
        ),
        (
            "where payment=card, passengers>=2; groupBy vendor: count + sum fare",
            AggregateRequest {
                labels: vec!["trips".into()],
                where_: where_card,
                group_by: vec!["vendor".into()],
                counts: vec![("n".into(), "*".into())],
                sums: vec![("fare".into(), "fare".into())],
                ..Default::default()
            },
        ),
        (
            "groupBy vendor, payment: count + min fare",
            AggregateRequest {
                labels: vec!["trips".into()],
                group_by: vec!["vendor".into(), "payment".into()],
                counts: vec![("n".into(), "*".into())],
                mins: vec![("min_fare".into(), "fare".into())],
                ..Default::default()
            },
        ),
    ];
    // Each shape at each thread budget (`AGGREGATE_MEASUREMENT_THREADS`,
    // default `1,8`); the answers must not differ.
    let threads: Vec<usize> = std::env::var("AGGREGATE_MEASUREMENT_THREADS")
        .unwrap_or_else(|_| "1,8".into())
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    for (name, request) in &shapes {
        let mut answers = Vec::new();
        for &n in &threads {
            rushdb_storage::parallel::set_threads(n);
            let view = Arc::new(store.read_view().unwrap());
            let started = Instant::now();
            let (buckets, total) =
                aggregate_with_cancel_in_view(&lmpg, request, &|| false, view).unwrap();
            println!(
                "{name} [{n} threads]: {:.1?} ({} groups, {total} matched)",
                started.elapsed(),
                buckets.len()
            );
            answers.push(serde_json::to_string(&buckets).unwrap());
        }
        assert!(answers.windows(2).all(|w| w[0] == w[1]), "{name}");
    }
}
