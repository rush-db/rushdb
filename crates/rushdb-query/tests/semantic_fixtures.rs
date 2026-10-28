//! P00 frozen semantic fixtures (04-implementation-backlog.md P00): duplicate
//! property values, null/missing, multivalues, sort ties, exact totals, and
//! text-bearing determinism. These pin the correctness contract so later query /
//! index / storage changes cannot silently change ordering, counts, or
//! missing/null behavior (plan execution rules).

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

const TENANT: TenantId = TenantId::from_ordinal(77);

fn setup() -> Lmpg {
    let store = Arc::new(Store::new_in_memory());
    Lmpg::new(store, TENANT)
}

fn match_count(lmpg: &Lmpg, document: &Value) -> usize {
    let parsed = rushdb_query::document::parse_document(document).unwrap();
    usize::try_from(
        rushdb_query::find_records(lmpg, &parsed.search)
            .unwrap()
            .total_matched,
    )
    .unwrap_or(usize::MAX)
}

#[test]
fn duplicate_property_values_are_all_matched() {
    // Three records share the same `v` value; the index must not collapse them.
    let lmpg = setup();
    for _ in 0..3 {
        lmpg.create_record("D", None, vec![("v".into(), FieldChange::Set(json!(7)))])
            .unwrap();
    }
    assert_eq!(
        match_count(
            &lmpg,
            &json!({ "labels": ["D"], "where": { "v": { "$eq": 7 } } })
        ),
        3,
        "duplicate-valued records must each match"
    );
}

#[test]
fn null_missing_and_absent_properties_are_distinct() {
    let lmpg = setup();
    // a: has v=1; b: has v=null (explicit); c: has no v at all.
    lmpg.create_record("D", None, vec![("v".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    lmpg.create_record("D", None, vec![("v".into(), FieldChange::ExplicitNull)])
        .unwrap();
    lmpg.create_record("D", None, vec![]).unwrap();

    let all = match_count(&lmpg, &json!({ "labels": ["D"] }));
    assert_eq!(all, 3);

    // Explicit null value vs missing key are different states: only the record
    // with v=null matches an $eq:null predicate; the absent-key record matches
    // $exists:false. They must be disjoint and together cover the 2 records.
    let null_present = match_count(
        &lmpg,
        &json!({ "labels": ["D"], "where": { "v": { "$eq": null } } }),
    );
    let absent = match_count(
        &lmpg,
        &json!({ "labels": ["D"], "where": { "v": { "$exists": false } } }),
    );
    assert_eq!(
        null_present, 1,
        "explicit null matches exactly the null record"
    );
    assert_eq!(absent, 1, "missing key matches exactly the absent record");
}

#[test]
fn multivalue_matches_any_element_but_not_empty() {
    let lmpg = setup();
    lmpg.create_record(
        "D",
        None,
        vec![("tags".into(), FieldChange::Set(json!(["a", "b"])))],
    )
    .unwrap();
    lmpg.create_record(
        "D",
        None,
        vec![("tags".into(), FieldChange::Set(json!([])))],
    )
    .unwrap();
    lmpg.create_record(
        "D",
        None,
        vec![("tags".into(), FieldChange::Set(json!(["b", "c"])))],
    )
    .unwrap();

    // Element match: both records containing "b".
    assert_eq!(
        match_count(
            &lmpg,
            &json!({ "labels": ["D"], "where": { "tags": { "$eq": "b" } } })
        ),
        2
    );
    // Empty array contains no element.
    assert_eq!(
        match_count(
            &lmpg,
            &json!({ "labels": ["D"], "where": { "tags": { "$eq": "a" } } })
        ),
        1
    );
}

#[test]
fn sort_ties_have_deterministic_order() {
    let lmpg = setup();
    // Three records with identical sort keys; ordering must be stable (not
    // nondeterministic), so repeated queries return the same sequence.
    for i in 0..3 {
        lmpg.create_record(
            "D",
            None,
            vec![
                ("k".into(), FieldChange::Set(json!(10))),
                ("id".into(), FieldChange::Set(json!(i))),
            ],
        )
        .unwrap();
    }
    let doc = json!({ "labels": ["D"], "orderBy": { "k": "desc" }, "limit": 3 });
    let parsed = rushdb_query::document::parse_document(&doc).unwrap();
    let ids = |parsed: &rushdb_query::SearchRequest| -> Vec<Value> {
        rushdb_query::find_records(&lmpg, parsed)
            .unwrap()
            .records
            .iter()
            .map(|r| r.get("id").cloned().unwrap_or(Value::Null))
            .collect()
    };
    let first = ids(&parsed.search);
    let second = ids(&parsed.search);
    assert_eq!(
        first, second,
        "tie ordering must be deterministic across runs"
    );
    assert_eq!(first.len(), 3);
}

#[test]
fn exact_totals_are_reported_not_truncated() {
    let lmpg = setup();
    for i in 0..25 {
        lmpg.create_record("D", None, vec![("v".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }
    // A small page must report the true total, not the page size.
    let doc = json!({ "labels": ["D"], "limit": 5 });
    let parsed = rushdb_query::document::parse_document(&doc).unwrap();
    let page = rushdb_query::find_records(&lmpg, &parsed.search).unwrap();
    assert_eq!(page.records.len(), 5);
    assert_eq!(
        page.total_matched, 25,
        "exact total must not be truncated to the page"
    );
}

#[test]
fn text_bearing_queries_are_deterministic() {
    // A filtered label query over text-bearing records must be finite and
    // deterministic (prerequisite for any filtered-scoring work; the `$text`
    // operator lives in the text engine, not the query-document grammar).
    let lmpg = setup();
    for i in 0..10 {
        lmpg.create_record(
            "D",
            None,
            vec![(
                "t".into(),
                FieldChange::Set(json!(format!("graph database topic {i}"))),
            )],
        )
        .unwrap();
    }
    let doc = json!({ "labels": ["D"], "limit": 10 });
    let parsed = rushdb_query::document::parse_document(&doc).unwrap();
    let a = rushdb_query::find_records(&lmpg, &parsed.search).unwrap();
    let b = rushdb_query::find_records(&lmpg, &parsed.search).unwrap();
    assert_eq!(
        a.records, b.records,
        "query over text-bearing records is deterministic"
    );
    assert_eq!(a.total_matched, 10);
}
