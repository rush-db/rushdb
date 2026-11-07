//! Grouped counts answered from the value dictionary must equal the record
//! loop's answer for every value shape: strings, integers, floats, booleans,
//! datetimes, explicit nulls, records without the property, a second label
//! sharing the property, and the shapes that send the request back to the
//! record loop (arrays, long values, type drift).

use std::collections::BTreeMap;
use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_query::{aggregate_with_cancel_in_view, AggregateRequest, GroupBucket};
use rushdb_storage::read_accounting;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

fn store() -> (Arc<Store>, Lmpg) {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store.clone(), TenantId::single());
    (store, lmpg)
}

fn put(lmpg: &Lmpg, label: &str, fields: Vec<(&str, Value)>) {
    lmpg.create_record(
        label,
        None,
        fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), FieldChange::Set(value)))
            .collect(),
    )
    .unwrap();
}

/// `groupBy [property]` + `$count: "*"`: the dictionary shape.
fn grouped_count(property: &str) -> AggregateRequest {
    AggregateRequest {
        labels: vec!["Trip".into()],
        group_by: vec![property.into()],
        counts: vec![("n".into(), "*".into())],
        ..Default::default()
    }
}

/// The same grouping with a second, field-counting metric, which the
/// dictionary cannot answer: the record loop's answer.
fn grouped_count_by_records(property: &str) -> AggregateRequest {
    AggregateRequest {
        counts: vec![("n".into(), "*".into()), ("m".into(), "zzz".into())],
        ..grouped_count(property)
    }
}

fn by_key(buckets: &[GroupBucket], key: &str) -> BTreeMap<String, u64> {
    buckets
        .iter()
        .map(|b| (b.keys[key].to_string(), b.count_roots))
        .collect()
}

fn run(store: &Arc<Store>, lmpg: &Lmpg, request: &AggregateRequest) -> (Vec<GroupBucket>, u64) {
    let view = Arc::new(store.read_view().unwrap());
    aggregate_with_cancel_in_view(lmpg, request, &|| false, view).unwrap()
}

#[test]
fn grouped_counts_from_the_dictionary_equal_the_record_loop() {
    let (store, lmpg) = store();
    for i in 0..60u32 {
        let mut fields: Vec<(&str, Value)> = vec![
            ("vendor", json!(if i % 3 == 0 { "A" } else { "B" })),
            ("hour", json!(i % 5)),
            ("fare", json!(f64::from(i % 4) * 2.5)),
            ("paid", json!(i % 2 == 0)),
            ("at", json!(format!("2026-01-{:02}T00:00:00Z", i % 3 + 1))),
        ];
        if i % 7 == 0 {
            fields.push(("tip", Value::Null));
        } else if i % 7 < 4 {
            fields.push(("tip", json!(i % 2)));
        }
        put(&lmpg, "Trip", fields);
    }
    // Another label holding the same properties must not leak into Trip.
    for i in 0..9 {
        put(
            &lmpg,
            "Other",
            vec![("vendor", json!("C")), ("hour", json!(i))],
        );
    }
    for property in ["vendor", "hour", "fare", "paid", "at", "tip", "nothing"] {
        let (fast, fast_total) = run(&store, &lmpg, &grouped_count(property));
        let (slow, slow_total) = run(&store, &lmpg, &grouped_count_by_records(property));
        assert_eq!(fast_total, 60, "{property}");
        assert_eq!(slow_total, 60, "{property}");
        assert_eq!(
            by_key(&fast, property),
            by_key(&slow, property),
            "{property}: dictionary {fast:?} vs records {slow:?}"
        );
    }
    // `tip`: 9 explicit nulls (i % 7 == 0), 27 valued, 24 without the property.
    let (tip, _) = run(&store, &lmpg, &grouped_count("tip"));
    let tip = by_key(&tip, "tip");
    assert_eq!(tip["null"], 9);
    assert_eq!(tip["\"__missing__\""], 24);
    assert_eq!(tip["0"] + tip["1"], 27);
}

#[test]
fn a_grouped_count_reads_no_record() {
    let (store, lmpg) = store();
    for i in 0..200u32 {
        put(
            &lmpg,
            "Trip",
            vec![
                ("vendor", json!(if i % 2 == 0 { "A" } else { "B" })),
                ("payload", json!("x".repeat(512))),
            ],
        );
    }
    let view = Arc::new(store.read_view().unwrap());
    read_accounting::enable(true);
    let _ = read_accounting::take_local();
    let (buckets, _) =
        aggregate_with_cancel_in_view(&lmpg, &grouped_count("vendor"), &|| false, view).unwrap();
    let reads: u64 = read_accounting::take_local().values().sum();
    read_accounting::enable(false);
    // A handful of catalog rows and the two dictionary entries, never a
    // read per record.
    assert!(reads < 50, "{reads} backend reads for 200 records");
    assert_eq!(by_key(&buckets, "vendor")["\"A\""], 100);
}

#[test]
fn arrays_long_values_and_type_drift_fall_back_to_the_record_loop() {
    let (store, lmpg) = store();
    put(
        &lmpg,
        "Trip",
        vec![("tags", json!(["x", "y"])), ("mixed", json!(1))],
    );
    put(
        &lmpg,
        "Trip",
        vec![("tags", json!(["x"])), ("mixed", json!("one"))],
    );
    put(&lmpg, "Trip", vec![("long", json!("l".repeat(4096)))]);
    put(&lmpg, "Trip", vec![("long", json!("short"))]);
    for property in ["tags", "mixed", "long"] {
        let (fast, _) = run(&store, &lmpg, &grouped_count(property));
        let (slow, _) = run(&store, &lmpg, &grouped_count_by_records(property));
        assert_eq!(
            by_key(&fast, property),
            by_key(&slow, property),
            "{property}"
        );
    }
    let (tags, _) = run(&store, &lmpg, &grouped_count("tags"));
    assert_eq!(by_key(&tags, "tags")["[\"x\",\"y\"]"], 1);
}

#[test]
fn a_grouped_count_orders_and_pages_its_rows() {
    let (store, lmpg) = store();
    for i in 0..30u32 {
        put(&lmpg, "Trip", vec![("hour", json!(i % 3))]);
    }
    let request = AggregateRequest {
        order_by: vec![("hour".into(), true)],
        skip: 1,
        limit: Some(1),
        ..grouped_count("hour")
    };
    let (rows, total) = run(&store, &lmpg, &request);
    assert_eq!(total, 30);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].keys["hour"], json!(1));
    assert_eq!(rows[0].count_roots, 10);
}
