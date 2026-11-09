//! Aggregation over typed rows, split into fixed-size parts: the answer is
//! the same whatever the threads lent, candidates are trusted to be live
//! (a deleted record never appears, and no record header is read per
//! candidate), and a property stored under two types is read in the order
//! most records hold it.
#![expect(
    clippy::cast_precision_loss,
    reason = "the fixtures' integers are small enough for f64"
)]

use std::sync::{Arc, Mutex};

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, Lmpg};
use rushdb_query::document::parse_document;
use rushdb_query::{
    aggregate, aggregate_to_rows, aggregate_with_cancel, find_records, AggregateRequest,
    OutputExpr, QueryError, TimeBucketSpec, TimeUnit,
};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

/// Tests that set the process-wide thread budget run one at a time.
static THREADS: Mutex<()> = Mutex::new(());

/// More records than three parts hold.
const TRIPS: u64 = 20_000;

fn engine(cap: u64) -> Lmpg {
    let store = Store::new_with_resources(rushdb_storage::resources::StorageResources {
        query_memory_max_bytes: cap,
        ..rushdb_storage::resources::StorageResources::default()
    })
    .unwrap();
    Lmpg::new(Arc::new(store), TenantId::from_ordinal(9))
}

fn trip(i: u64) -> Vec<(String, FieldChange)> {
    let set = |name: &str, value: Value| (name.to_owned(), FieldChange::Set(value));
    let mut fields = vec![
        set("vendor", json!(if i.is_multiple_of(3) { "A" } else { "B" })),
        set(
            "payment",
            json!(["card", "cash", "other"][(i % 3) as usize]),
        ),
        set("zone", json!(i % 37)),
        set("fare", json!((i % 700) as f64 / 10.0 + 3.0)),
        // Stored under both Int64 and Float64.
        set(
            "tip",
            if i.is_multiple_of(4) {
                json!(i % 9)
            } else {
                json!((i % 50) as f64 / 10.0)
            },
        ),
        set("ts", json!(1_700_000_000_000u64 + i * 60_000)),
        set("region", json!(i / 20)),
    ];
    if i.is_multiple_of(11) {
        fields.push(set("note", json!(format!("n{}", i % 5))));
    }
    fields
}

fn seed(lmpg: &Lmpg, records: u64) {
    for chunk in (0..records).collect::<Vec<_>>().chunks(2_000) {
        let mut tx = lmpg.begin_batch();
        for &i in chunk {
            tx.ops.push(BatchOp::CreateRecord {
                label: "Trip".into(),
                explicit_id: None,
                fields: trip(i),
            });
        }
        lmpg.run_batch(tx).unwrap();
    }
}

fn request(doc: &Value) -> AggregateRequest {
    parse_document(doc).unwrap().aggregate.unwrap()
}

fn shapes() -> Vec<(&'static str, AggregateRequest)> {
    vec![
        (
            "groupBy payment: count + sum fare",
            request(&json!({"labels": ["Trip"], "groupBy": ["payment"],
                "select": {"n": {"$count": "*"}, "fare": {"$sum": "$record.fare"}}})),
        ),
        (
            "groupBy vendor, payment: min fare, max tip, avg tip",
            request(
                &json!({"labels": ["Trip"], "groupBy": ["vendor", "payment"],
                "select": {"n": {"$count": "*"}, "lo": {"$min": "$record.fare"},
                           "hi": {"$max": "$record.tip"}, "tip": {"$avg": "$record.tip"}}}),
            ),
        ),
        (
            "filtered: payment = card, zone >= 10; groupBy vendor",
            request(&json!({"labels": ["Trip"],
                "where": {"payment": "card", "zone": {"$gte": 10}},
                "groupBy": ["vendor"],
                "select": {"n": {"$count": "*"}, "fare": {"$sum": "$record.fare"}}})),
        ),
        (
            "global rollup: count note, min note, sum tip, avg fare",
            request(&json!({"labels": ["Trip"],
                "select": {"notes": {"$count": "$record.note"}, "first": {"$min": "$record.note"},
                           "tip": {"$sum": "$record.tip"}, "fare": {"$avg": "$record.fare"}}})),
        ),
        (
            "groupBy tip (two types)",
            request(&json!({"labels": ["Trip"], "groupBy": ["tip"],
                "select": {"n": {"$count": "*"}, "zones": {"$sum": "$record.zone"}}})),
        ),
        ("time bucket by day, expression sum", {
            let mut req = request(&json!({"labels": ["Trip"],
                "select": {"n": {"$count": "*"}}}));
            req.time_bucket = Some(TimeBucketSpec::fixed("ts", TimeUnit::Days));
            req.expr_sums = vec![(
                "double".into(),
                OutputExpr::Multiply(
                    Box::new(OutputExpr::Field("fare".into())),
                    Box::new(OutputExpr::Literal(2.0)),
                ),
            )];
            req.outputs = vec![("double_out".into(), OutputExpr::Ref("double".into()))];
            req
        }),
    ]
}

fn rows(lmpg: &Lmpg, req: &AggregateRequest) -> Result<(Vec<Value>, u64), QueryError> {
    let (buckets, total) = aggregate_with_cancel(lmpg, req, &|| false)?;
    Ok((aggregate_to_rows(lmpg, &buckets, req)?, total))
}

#[test]
fn parts_give_the_same_answer_whatever_the_threads() {
    let _serial = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let lmpg = engine(0);
    seed(&lmpg, TRIPS);
    for (name, req) in shapes() {
        rushdb_storage::parallel::set_threads(1);
        let one = rows(&lmpg, &req).unwrap();
        rushdb_storage::parallel::set_threads(8);
        let eight = rows(&lmpg, &req).unwrap();
        assert_eq!(one, eight, "{name}");
        assert!(!one.0.is_empty(), "{name}");
    }
    rushdb_storage::parallel::set_threads(1);

    // Exact values across parts: counts, minima and maxima.
    rushdb_storage::parallel::set_threads(8);
    let (by_payment, total) = rows(&lmpg, &shapes()[0].1).unwrap();
    assert_eq!(total, TRIPS);
    let card = by_payment.iter().find(|r| r["payment"] == "card").unwrap();
    assert_eq!(card["n"], json!(TRIPS.div_ceil(3)));
    let (extremes, _) = rows(&lmpg, &shapes()[1].1).unwrap();
    let lo = extremes
        .iter()
        .map(|r| r["lo"].as_f64().unwrap())
        .fold(f64::MAX, f64::min);
    let hi = extremes
        .iter()
        .map(|r| r["hi"].as_f64().unwrap())
        .fold(f64::MIN, f64::max);
    assert_eq!((lo, hi), (3.0, 8.0));
    let (rollup, _) = rows(&lmpg, &shapes()[3].1).unwrap();
    assert_eq!(rollup[0]["notes"], json!(TRIPS.div_ceil(11)));
    // The first value seen (record 0's) sets a text minimum.
    assert_eq!(rollup[0]["first"], json!("n0"));
    rushdb_storage::parallel::set_threads(1);
}

#[test]
fn integer_and_float_values_group_apart() {
    let lmpg = engine(0);
    for amount in [json!(15), json!(15.0), json!(15), json!(2.5)] {
        rushdb_lmpg::import::import_document(&lmpg, "Pay", &json!({"amount": amount})).unwrap();
    }
    let req = request(&json!({"labels": ["Pay"], "groupBy": ["amount"],
        "select": {"n": {"$count": "*"}, "total": {"$sum": "$record.amount"}}}));
    let (rows, total) = rows(&lmpg, &req).unwrap();
    assert_eq!(total, 4);
    let n = |key: &Value| {
        rows.iter()
            .find(|r| r["amount"] == *key)
            .map(|r| r["n"].clone())
    };
    assert_eq!(n(&json!(15)), Some(json!(2)));
    assert_eq!(n(&json!(15.0)), Some(json!(1)));
    assert_eq!(n(&json!(2.5)), Some(json!(1)));
    assert_eq!(rows.len(), 3);
}

#[test]
fn the_caller_is_polled_between_parts() {
    let _serial = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    rushdb_storage::parallel::set_threads(1);
    let lmpg = engine(0);
    seed(&lmpg, TRIPS);
    let req = &shapes()[0].1;
    let probes = std::cell::Cell::new(0usize);
    aggregate_with_cancel(&lmpg, req, &|| {
        probes.set(probes.get() + 1);
        false
    })
    .unwrap();
    // The last probe comes before the last part: stopping there cancels.
    let last = probes.get();
    let seen = std::cell::Cell::new(0usize);
    let stopped = aggregate_with_cancel(&lmpg, req, &|| {
        seen.set(seen.get() + 1);
        seen.get() >= last
    });
    assert!(matches!(stopped, Err(QueryError::Cancelled)), "{stopped:?}");

    // A passed deadline stops the parts too.
    let late = lmpg
        .handle()
        .with_request_limits(rushdb_lmpg::RequestLimits {
            deadline: Some(std::time::Instant::now()),
            ..Default::default()
        });
    rushdb_storage::parallel::set_threads(8);
    assert!(matches!(aggregate(&late, req), Err(QueryError::Cancelled)));
    rushdb_storage::parallel::set_threads(1);
}

#[test]
fn every_part_counts_toward_the_working_set() {
    let _serial = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // 1,000 regions: each part holds ~410 groups, all parts together
    // 1,000 groups three times over.
    let req = request(&json!({"labels": ["Trip"], "groupBy": ["region"],
        "select": {"n": {"$count": "*"}, "fare": {"$sum": "$record.fare"}}}));
    for (cap, fits) in [(600_000u64, false), (4_000_000, true)] {
        let lmpg = engine(cap);
        seed(&lmpg, TRIPS);
        let mut outcomes = Vec::new();
        for threads in [1, 8] {
            rushdb_storage::parallel::set_threads(threads);
            let outcome = rows(&lmpg, &req);
            match &outcome {
                Ok((rows, _)) => assert!(fits && rows.len() == 1_000, "cap {cap}"),
                Err(QueryError::ResourceLimit(message)) => {
                    assert!(!fits && message.contains("query-memory-max"), "{message}");
                }
                Err(other) => panic!("cap {cap}: {other:?}"),
            }
            outcomes.push(outcome.map_err(|e| e.to_string()));
        }
        assert_eq!(outcomes[0], outcomes[1], "cap {cap}");
    }
    rushdb_storage::parallel::set_threads(1);
}

#[test]
fn the_group_cap_holds_across_parts() {
    let _serial = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let lmpg = engine(0);
    // 1,050 regions over 21,000 records: no part holds more than 1,000.
    seed(&lmpg, 21_000);
    let req = request(&json!({"labels": ["Trip"], "groupBy": ["region"],
        "select": {"n": {"$count": "*"}, "fare": {"$sum": "$record.fare"}}}));
    for threads in [1, 8] {
        rushdb_storage::parallel::set_threads(threads);
        let refused = rows(&lmpg, &req);
        assert!(
            matches!(&refused, Err(QueryError::ResourceLimit(m)) if m.contains("distinct groups")),
            "{refused:?}"
        );
    }
    rushdb_storage::parallel::set_threads(1);
}

/// Backend reads of `run` on this thread, by key family.
fn reads(run: impl Fn()) -> std::collections::BTreeMap<(&'static str, String), u64> {
    run();
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    run();
    rushdb_storage::read_accounting::take_local()
}

fn family(reads: &std::collections::BTreeMap<(&'static str, String), u64>, name: &str) -> u64 {
    reads
        .iter()
        .filter(|((_, family), _)| family == name)
        .map(|(_, n)| n)
        .sum()
}

#[test]
fn deleted_records_never_appear_and_live_ones_are_not_rechecked() {
    let _serial = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    rushdb_storage::parallel::set_threads(1);
    let lmpg = engine(0);
    let mut refs = Vec::new();
    for i in 0..40u64 {
        let doc = json!({"kind": if i % 2 == 0 { "even" } else { "odd" }, "n": i});
        refs.push(rushdb_lmpg::import::import_document(&lmpg, "Item", &doc).unwrap());
    }
    for r in refs.iter().step_by(4) {
        lmpg.delete_record(*r).unwrap();
    }
    let grouped = request(&json!({"labels": ["Item"], "groupBy": ["kind"],
        "select": {"c": {"$count": "*"}, "sum": {"$sum": "$record.n"}}}));
    let (rows_out, total) = rows(&lmpg, &grouped).unwrap();
    assert_eq!(total, 30);
    let even = rows_out.iter().find(|r| r["kind"] == "even").unwrap();
    // Evens left: 2, 6, 10, ..., 38 (0, 4, 8, ... deleted).
    assert_eq!(even["c"], json!(10));
    assert_eq!(
        even["sum"],
        json!((0..40).filter(|i| i % 4 == 2).sum::<u64>() as f64)
    );
    let filtered = request(&json!({"labels": ["Item"], "where": {"n": {"$lt": 9}},
        "select": {"c": {"$count": "*"}}}));
    assert_eq!(aggregate(&lmpg, &filtered).unwrap()[0].count_roots, 6);
    let search = parse_document(&json!({"labels": ["Item"], "where": {"kind": "even"}}))
        .unwrap()
        .search;
    let page = find_records(&lmpg, &search).unwrap();
    assert_eq!(page.total_matched, 10);
    assert!(page
        .records
        .iter()
        .all(|r| r["n"].as_u64().unwrap() % 4 == 2));

    // Neither the walk nor the filter reads a record header per candidate.
    let walked = reads(|| {
        aggregate(&lmpg, &grouped).unwrap();
    });
    assert_eq!(family(&walked, "record-header"), 0, "{walked:?}");
    let checked = reads(|| {
        aggregate(&lmpg, &filtered).unwrap();
    });
    assert_eq!(family(&checked, "record-header"), 0, "{checked:?}");
}

#[test]
fn a_property_stored_under_two_types_is_read_in_the_order_most_records_hold_it() {
    let _serial = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    rushdb_storage::parallel::set_threads(1);
    let lmpg = engine(0);
    // The Int64 definition comes first (the lower id) and holds 90 of 100.
    for i in 0..100u64 {
        let amount = if i < 90 {
            json!(i)
        } else {
            json!(i as f64 + 0.5)
        };
        rushdb_lmpg::import::import_document(&lmpg, "Pay", &json!({"amount": amount})).unwrap();
    }
    let sum = request(&json!({"labels": ["Pay"],
        "select": {"total": {"$sum": "$record.amount"}, "hi": {"$max": "$record.amount"}}}));
    let (rows_out, _) = rows(&lmpg, &sum).unwrap();
    let expected = (0..90).sum::<u64>() as f64 + (90..100).map(|i| f64::from(i) + 0.5).sum::<f64>();
    assert_eq!(rows_out[0]["total"], json!(expected));
    assert_eq!(rows_out[0]["hi"], json!(99.5));
    let ordered = parse_document(&json!({"labels": ["Pay"], "orderBy": {"amount": "desc"},
        "limit": 3}))
    .unwrap()
    .search;
    let page = find_records(&lmpg, &ordered).unwrap();
    let top: Vec<Value> = page.records.iter().map(|r| r["amount"].clone()).collect();
    assert_eq!(top, vec![json!(99.5), json!(98.5), json!(97.5)]);

    // One occurrence read per record that holds the common type, two for
    // the ten that hold the other: probing the newer (Float64) definition
    // first read 190.
    let measured = reads(|| {
        aggregate(&lmpg, &sum).unwrap();
    });
    assert_eq!(family(&measured, "occurrence"), 110, "{measured:?}");
}

#[test]
fn a_batch_writing_a_property_twice_keeps_one_value() {
    let lmpg = engine(0);
    let mut tx = lmpg.begin_batch();
    tx.ops.push(BatchOp::CreateRecord {
        label: "T".into(),
        explicit_id: None,
        fields: vec![
            ("x".into(), FieldChange::Set(json!(1))),
            ("X".into(), FieldChange::Set(json!(2.5))),
            ("tags".into(), FieldChange::Set(json!(["a", "b", "c"]))),
            ("tags".into(), FieldChange::Set(json!(["d"]))),
        ],
    });
    let created = lmpg.run_batch(tx).unwrap().created_records[0];
    let mut tx = lmpg.begin_batch();
    for value in [json!("text"), json!(7)] {
        tx.ops.push(BatchOp::UpdateFields {
            owner: rushdb_types::ids::OwnerKind::Record,
            owner_ord: created.ordinal,
            owner_public_id: created.record_id,
            fields: vec![("y".into(), FieldChange::Set(value))],
        });
    }
    lmpg.run_batch(tx).unwrap();
    let snap = lmpg.store.read_view().unwrap();
    let occurrences = rushdb_lmpg::reader::try_access::occurrences_of_owner(
        &snap,
        lmpg.tenant,
        rushdb_types::ids::OwnerKind::Record,
        created.ordinal,
    )
    .unwrap();
    // x, one tag, y.
    assert_eq!(occurrences.len(), 3, "{occurrences:?}");
    let record = rushdb_lmpg::reconstruct::reconstruct_record(&snap, lmpg.tenant, created.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(record["x"], json!(2.5));
    assert_eq!(record["tags"], json!(["d"]));
    assert_eq!(record["y"], json!(7));
}

#[test]
fn a_walk_reads_each_member_once_across_labels_and_ranges() {
    let _serial = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let lmpg = engine(0);
    // Two labels interleaved over the ordinals, and a sparse third one.
    for chunk in (0..30_000u64).collect::<Vec<_>>().chunks(3_000) {
        let mut tx = lmpg.begin_batch();
        for &i in chunk {
            let label = match i % 100 {
                0 => "Rare",
                n if n % 2 == 0 => "Even",
                _ => "Odd",
            };
            tx.ops.push(BatchOp::CreateRecord {
                label: label.into(),
                explicit_id: None,
                fields: vec![("n".into(), FieldChange::Set(json!(i)))],
            });
        }
        lmpg.run_batch(tx).unwrap();
    }
    let sum_of = |pred: &dyn Fn(u64) -> bool| (0..30_000u64).filter(|&i| pred(i)).sum::<u64>();
    let cases: Vec<(Value, u64, u64)> = vec![
        (json!(["Rare"]), 300, sum_of(&|i| i % 100 == 0)),
        (json!(["Even", "Rare"]), 15_000, sum_of(&|i| i % 2 == 0)),
        (json!(["Odd", "Even", "Rare"]), 30_000, sum_of(&|_| true)),
        (json!([]), 30_000, sum_of(&|_| true)),
    ];
    for (labels, count, sum) in cases {
        let req = request(&json!({"labels": labels,
            "select": {"c": {"$count": "*"}, "s": {"$sum": "$record.n"}}}));
        let mut answers = Vec::new();
        for threads in [1, 8] {
            rushdb_storage::parallel::set_threads(threads);
            let (out, total) = rows(&lmpg, &req).unwrap();
            assert_eq!((total, &out[0]["c"]), (count, &json!(count)), "{labels}");
            assert_eq!(out[0]["s"], json!(sum as f64), "{labels}");
            answers.push(out);
        }
        assert_eq!(answers[0], answers[1], "{labels}");
    }
    rushdb_storage::parallel::set_threads(1);
}
