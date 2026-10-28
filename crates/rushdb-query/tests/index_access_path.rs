//! Equality/In index access path: the maintained postings must return the
//! exact same match sets as the full-scan fallback, including numeric
//! int/float union, arrays, missing fields, label scoping, and shapes the
//! index declines (`$ne`, `$or`).

use std::collections::BTreeSet;
use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_query::document::parse_document;
use rushdb_query::{find_records, find_records_streaming};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

fn setup() -> Lmpg {
    let store = Arc::new(Store::new_in_memory());
    Lmpg::new(store, TenantId::from_ordinal(77))
}

fn seed(lmpg: &Lmpg) {
    let set = |fields: Vec<(&str, Value)>| -> Vec<(String, FieldChange)> {
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), FieldChange::Set(v)))
            .collect()
    };
    // string / int / float / bool / array / missing
    lmpg.create_record(
        "A",
        None,
        set(vec![
            ("s", json!("alpha")),
            ("n", json!(7)),
            ("flag", json!(true)),
            ("tags", json!(["red", "blue"])),
        ]),
    )
    .unwrap();
    lmpg.create_record(
        "A",
        None,
        set(vec![("s", json!("beta")), ("n", json!(7.0))]),
    )
    .unwrap();
    lmpg.create_record(
        "A",
        None,
        set(vec![("s", json!("gamma")), ("n", json!(7.5))]),
    )
    .unwrap();
    lmpg.create_record(
        "A",
        None,
        set(vec![("s", json!("alpha")), ("tags", json!(["green"]))]),
    )
    .unwrap();
    // Same value on a second label for scoping.
    lmpg.create_record("B", None, set(vec![("s", json!("alpha"))]))
        .unwrap();
    // Explicit null occurrence (present, null-valued).
    lmpg.create_record(
        "A",
        None,
        vec![("nv".to_string(), FieldChange::ExplicitNull)],
    )
    .unwrap();
}

fn collect(page: &rushdb_query::Page) -> BTreeSet<String> {
    page.records
        .iter()
        .filter_map(|r| r.get("__id").and_then(|v| v.as_str()).map(str::to_string))
        .collect()
}

/// Run one query through both the full and streaming executors against the
/// same dataset and return their id sets.
fn both(document: &Value) -> (BTreeSet<String>, BTreeSet<String>) {
    let lmpg = setup();
    seed(&lmpg);
    let parsed = parse_document(document).expect("valid query");
    let full = collect(&find_records(&lmpg, &parsed.search).expect("search"));
    let snap = Arc::new(lmpg.store.read_view().unwrap());
    let streamed = collect(
        &find_records_streaming(&lmpg, &parsed.search, &|| false, snap).expect("streaming search"),
    );
    (full, streamed)
}

fn assert_both(document: &Value, expected: usize, label: &str) {
    let (full, streamed) = both(document);
    assert_eq!(full.len(), expected, "full path {label}: {full:?}");
    assert_eq!(
        streamed, full,
        "streaming path differs from full for {label}"
    );
}

#[test]
fn eq_string_index_matches_scan() {
    assert_both(&json!({"where": {"s": "alpha"}}), 3, "eq string");
    assert_both(&json!({"where": {"s": "zeta"}}), 0, "eq absent value");
    assert_both(&json!({"where": {"missing": "x"}}), 0, "eq absent field");
}

#[test]
fn eq_number_unions_int_and_float_definitions() {
    // 7 (int) and 7.0 (float) must both match either form.
    assert_both(&json!({"where": {"n": 7}}), 2, "eq int");
    assert_both(&json!({"where": {"n": 7.0}}), 2, "eq float integral");
    assert_both(&json!({"where": {"n": 7.5}}), 1, "eq float fractional");
    assert_both(&json!({"where": {"n": 8}}), 0, "eq absent number");
}

#[test]
fn eq_bool_and_array_membership() {
    assert_both(&json!({"where": {"flag": true}}), 1, "eq bool");
    // Array values match element-wise, matching `any_element` semantics.
    assert_both(&json!({"where": {"tags": "red"}}), 1, "eq array element");
    assert_both(
        &json!({"where": {"tags": "green"}}),
        1,
        "eq array element 2",
    );
    assert_both(
        &json!({"where": {"tags": "purple"}}),
        0,
        "eq absent element",
    );
}

#[test]
fn in_filter_matches_scan() {
    assert_both(
        &json!({"where": {"s": {"$in": ["alpha", "gamma"]}}}),
        4,
        "in strings",
    );
    assert_both(&json!({"where": {"n": {"$in": [7, 7.5]}}}), 3, "in numbers");
    assert_both(
        &json!({"where": {"s": {"$in": ["nope"]}}}),
        0,
        "in empty match",
    );
}

#[test]
fn label_scoped_index_matches_scan() {
    assert_both(
        &json!({"labels": ["A"], "where": {"s": "alpha"}}),
        2,
        "label scoped",
    );
    assert_both(
        &json!({"labels": ["B"], "where": {"s": "alpha"}}),
        1,
        "label scoped single",
    );
    assert_both(
        &json!({"labels": ["A", "B"], "where": {"s": "alpha"}}),
        3,
        "label union",
    );
}

#[test]
fn unsupported_shapes_fall_back_and_stay_correct() {
    // $ne requires presence and must not be served by the index.
    assert_both(&json!({"where": {"s": {"$ne": "alpha"}}}), 2, "ne fallback");
    // $or is not a conjunction; whole expression must fall back.
    assert_both(
        &json!({"where": {"$or": [{"s": "beta"}, {"s": "gamma"}]}}),
        2,
        "or fallback",
    );
    // Conjunction with a range leaf falls back entirely and stays exact.
    assert_both(
        &json!({"where": {"s": "alpha", "n": {"$gte": 7}}}),
        1,
        "and with range fallback",
    );
    assert_both(&json!({"where": {"n": {"$gte": 7.0}}}), 3, "range only");
}

#[test]
fn conjunction_of_equalities_intersects() {
    assert_both(
        &json!({"where": {"s": "alpha", "tags": "red"}}),
        1,
        "and eq eq",
    );
    assert_both(
        &json!({"where": {"s": "alpha", "tags": "red", "flag": true}}),
        1,
        "and eq eq eq",
    );
}

#[test]
fn exists_and_null_use_the_presence_index() {
    // `$exists: true` (any occurrence) and `$eq: null` (explicit null) resolve
    // through presence keys; `$exists: false` is not indexable and must fall
    // back exactly.
    assert_both(&json!({"where": {"n": {"$exists": true}}}), 3, "exists n");
    assert_both(
        &json!({"where": {"n": {"$exists": false}}}),
        3,
        "exists false fallback",
    );
    assert_both(
        &json!({"where": {"nv": {"$eq": null}}}),
        1,
        "eq explicit null",
    );
}

#[test]
fn exists_count_is_exact_without_reconstruction() {
    // Dense `$exists` via the exact fast path: total must equal the number of
    // carriers (here every seeded record except the null-only one carries `s`).
    let (full, streamed) = both(&json!({"where": {"s": {"$exists": true}}}));
    assert_eq!(full.len(), 5);
    assert_eq!(streamed.len(), 5);
}
