//! Parity verification against the actual rushdb-repo `SearchQuery` test
//! syntax (platform/core/src/core/search/parser/tests/*.spec.ts). These
//! cases are ported verbatim from the TypeScript suite to confirm the Rust
//! query-document parser accepts the same documented syntax.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{import, Lmpg};
use rushdb_query::document::parse_document;
use rushdb_query::{aggregate, find_records, find_records_in_view, find_records_streaming};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

fn setup() -> Lmpg {
    let store = Arc::new(Store::new_in_memory());
    Lmpg::new(store, TenantId::from_ordinal(81))
}

fn match_count(lmpg: &Lmpg, document: &Value) -> usize {
    let q = parse_document(document).expect("valid query document");
    if let Some(agg) = q.aggregate {
        aggregate(lmpg, &agg).expect("aggregate").len()
    } else {
        usize::try_from(find_records(lmpg, &q.search).expect("search").total_matched)
            .unwrap_or(usize::MAX)
    }
}

/// parseQuery.spec.ts "parses subQueries correctly 1":
/// { tag: 'top-sellers', AUTHOR: { name: 'James Brown', $relation: {...} } }
#[test]
fn ported_parse_query_1() {
    let lmpg = setup();
    let book = import::import_document(&lmpg, "BOOK", &json!({"title": "T"})).unwrap();
    let tag = import::import_document(&lmpg, "BOOK", &json!({"tag": "top-sellers"})).unwrap();
    let author = import::import_document(&lmpg, "AUTHOR", &json!({"name": "James Brown"})).unwrap();
    import::import_document(&lmpg, "AUTHOR", &json!({"name": "Someone Else"})).unwrap();
    // top-seller tagged book authored by James Brown
    lmpg.create_relationship("AUTHORED", author, tag, vec![])
        .unwrap();
    let _ = book;

    let n = match_count(
        &lmpg,
        &json!({"labels": ["BOOK"], "where": {
            "tag": "top-sellers",
            "AUTHOR": {"name": "James Brown", "$relation": {"type": "AUTHORED", "direction": "in"}}
        }}),
    );
    assert_eq!(n, 1);
}

/// parseQuery.spec.ts "parses subQueries correctly 2": logical ops inside a
/// field block, datetime components, nested label blocks.
#[test]
fn ported_parse_query_2_field_logical_and_datetime() {
    let lmpg = setup();
    // rating in [4.5..6] or ==3 or not >=4; created 2011-11-11
    lmpg.create_record(
        "POST",
        None,
        vec![
            (
                "created".into(),
                FieldChange::Set(json!(1_320_969_600_000i64)),
            ), // 2011-11-11
            ("rating".into(), FieldChange::Set(json!(5))),
        ],
    )
    .unwrap();
    // rating 3, created 2012
    lmpg.create_record(
        "POST",
        None,
        vec![
            (
                "created".into(),
                FieldChange::Set(json!(1_336_176_000_000i64)),
            ), // 2012-05-05
            ("rating".into(), FieldChange::Set(json!(3))),
        ],
    )
    .unwrap();
    // rating 2 (not >=4), created 2011
    lmpg.create_record(
        "POST",
        None,
        vec![
            (
                "created".into(),
                FieldChange::Set(json!(1_320_969_600_000i64)),
            ),
            ("rating".into(), FieldChange::Set(json!(2))),
        ],
    )
    .unwrap();

    let n = match_count(
        &lmpg,
        &json!({"labels": ["POST"], "where": {
            "created": {"$year": 2011, "$month": 11, "$day": 11},
            "rating": {"$or": [
                {"$and": [{"$gt": 4.5}, {"$lt": 6}]},
                {"$ne": 3},
                {"$not": {"$gte": 4}}
            ]}
        }}),
    );
    // created exactly 2011-11-11 AND rating satisfies the OR group:
    // record0: 5 ∈ (4.5,6) ✓ ; record2: 2 (not>=4) ✓
    assert_eq!(n, 2);
}

/// compileExpr.spec.ts select scenarios: $count '*', $sum of nested math,
/// $avg with precision, $ref derived output, $record.__id.
#[test]
fn ported_compile_select_scenarios() {
    let lmpg = setup();
    import::import_document(
        &lmpg,
        "ORDER",
        &json!({"price": 10, "qty": 2, "amount": 20}),
    )
    .unwrap();
    import::import_document(
        &lmpg,
        "ORDER",
        &json!({"price": 15, "qty": 1, "amount": 15}),
    )
    .unwrap();

    // $count '*' + $sum over $record.amount
    let q = parse_document(&json!({
        "labels": ["ORDER"],
        "select": {"count": {"$count": "*"}, "revenue": {"$sum": "$record.amount"}}
    }))
    .unwrap();
    let buckets = aggregate(&lmpg, q.aggregate.as_ref().unwrap()).unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].count_roots, 2);
    assert_eq!(buckets[0].sums.get("revenue"), Some(&json!(35.0)));

    // $sum over nested math ($multiply price*qty)
    let q = parse_document(&json!({
        "labels": ["ORDER"],
        "select": {"lineTotal": {"$sum": {"$multiply": ["$record.price", "$record.qty"]}}}
    }))
    .unwrap();
    let buckets = aggregate(&lmpg, q.aggregate.as_ref().unwrap()).unwrap();
    assert_eq!(buckets[0].sums.get("lineTotal"), Some(&json!(35.0))); // 10*2 + 15*1

    // $avg with $precision
    let q = parse_document(&json!({
        "labels": ["ORDER"],
        "select": {"avgAmount": {"$avg": "$record.amount", "$precision": 1}}
    }))
    .unwrap();
    let buckets = aggregate(&lmpg, q.aggregate.as_ref().unwrap()).unwrap();
    assert_eq!(buckets[0].sums.get("avgAmount"), Some(&json!(17.5))); // (20+15)/2

    // $subtract with $ref operands: revenue - cost
    let q = parse_document(&json!({
        "labels": ["ORDER"],
        "select": {
            "revenue": {"$sum": "$record.amount"},
            "cost": {"$sum": "$record.qty"},
            "margin": {"$subtract": [{"$ref": "revenue"}, {"$ref": "cost"}]}
        }
    }))
    .unwrap();
    let buckets = aggregate(&lmpg, q.aggregate.as_ref().unwrap()).unwrap();
    assert_eq!(buckets[0].sums.get("margin"), Some(&json!(32.0))); // 35 - 3
}

/// Array-element semantics: { tags: 'typescript' } matches arrays containing it.
#[test]
fn array_element_semantics() {
    let lmpg = setup();
    lmpg.create_record(
        "ITEM",
        None,
        vec![(
            "tags".into(),
            FieldChange::Set(json!(["javascript", "typescript", "react"])),
        )],
    )
    .unwrap();
    lmpg.create_record(
        "ITEM",
        None,
        vec![("tags".into(), FieldChange::Set(json!(["python", "django"])))],
    )
    .unwrap();

    let n = match_count(
        &lmpg,
        &json!({"labels": ["ITEM"], "where": {"tags": "typescript"}}),
    );
    assert_eq!(n, 1);

    // $in over arrays: any element in list
    let n = match_count(
        &lmpg,
        &json!({"labels": ["ITEM"], "where": {"tags": {"$in": ["python"]}}}),
    );
    assert_eq!(n, 1);

    // $contains on array element
    let n = match_count(
        &lmpg,
        &json!({"labels": ["ITEM"], "where": {"tags": {"$contains": "react"}}}),
    );
    assert_eq!(n, 1);
}

/// Array-literal equality `{ tags: ["a", "b"] }` lowers to element-match
/// ($in) semantics instead of a label-key relationship block, matching the
/// baseline behavior of wrapping single values into IN predicates.
#[test]
fn array_literal_value_is_element_match() {
    let lmpg = setup();
    lmpg.create_record(
        "ITEM",
        None,
        vec![(
            "tags".into(),
            FieldChange::Set(json!(["javascript", "typescript"])),
        )],
    )
    .unwrap();
    lmpg.create_record(
        "ITEM",
        None,
        vec![("tags".into(), FieldChange::Set(json!(["python"])))],
    )
    .unwrap();
    lmpg.create_record(
        "ITEM",
        None,
        vec![("tags".into(), FieldChange::Set(json!("single")))],
    )
    .unwrap();

    // { tags: ["typescript"] } matches the record containing that element
    let n = match_count(
        &lmpg,
        &json!({"labels": ["ITEM"], "where": {"tags": ["typescript"]}}),
    );
    assert_eq!(n, 1);

    // { tags: ["python", "ruby"] } matches any element in the list
    let n = match_count(
        &lmpg,
        &json!({"labels": ["ITEM"], "where": {"tags": ["python", "ruby"]}}),
    );
    assert_eq!(n, 1);

    // scalar field values are matched by the same element semantics
    let n = match_count(
        &lmpg,
        &json!({"labels": ["ITEM"], "where": {"tags": ["single", "missing"]}}),
    );
    assert_eq!(n, 1);

    // $nin over an array literal list: records with no element in the list
    let n = match_count(
        &lmpg,
        &json!({"labels": ["ITEM"], "where": {"tags": {"$nin": ["javascript", "python", "single"]}}}),
    );
    assert_eq!(n, 0);
}

/// `$type` is cardinality-aware: `string[]`/`number[]` match array-typed
/// columns, `array` matches any array, and the bare type matches both scalar
/// and array values (baseline stores array primitives as plain string).
#[test]
fn type_operator_is_array_cardinality_aware() {
    let lmpg = setup();
    lmpg.create_record(
        "ITEM",
        None,
        vec![
            ("tags".into(), FieldChange::Set(json!(["a", "b"]))),
            ("score".into(), FieldChange::Set(json!([1, 2]))),
        ],
    )
    .unwrap();
    lmpg.create_record(
        "ITEM",
        None,
        vec![
            ("tags".into(), FieldChange::Set(json!("c"))),
            ("score".into(), FieldChange::Set(json!(3))),
        ],
    )
    .unwrap();

    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"tags": {"$type": "string[]"}}})
        ),
        1
    );
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"tags": {"$type": "array"}}})
        ),
        1
    );
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"score": {"$type": "number[]"}}})
        ),
        1
    );
    // bare type matches both scalar and array values (baseline compatibility)
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"tags": {"$type": "string"}}})
        ),
        2
    );
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"score": {"$type": "number"}}})
        ),
        2
    );
}

/// `$type` boolean alias: the client surface operates with `boolean`, while
/// legacy queries may still send `bool`. Both must match boolean-typed
/// properties (declared) and boolean-shaped values (fallback).
#[test]
fn type_operator_boolean_alias_accepts_bool_and_boolean() {
    let lmpg = setup();
    lmpg.create_record(
        "ITEM",
        None,
        vec![
            ("active".into(), FieldChange::Set(json!(true))),
            ("flagged".into(), FieldChange::Set(json!(false))),
            ("score".into(), FieldChange::Set(json!(7))),
        ],
    )
    .unwrap();

    // canonical client-facing name
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"active": {"$type": "boolean"}}})
        ),
        1
    );
    // legacy alias still works
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"active": {"$type": "bool"}}})
        ),
        1
    );
    // array cardinality alias: boolean[] matches array-typed boolean columns
    lmpg.create_record(
        "ITEM",
        None,
        vec![("flags".into(), FieldChange::Set(json!([true, false])))],
    )
    .unwrap();
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"flags": {"$type": "boolean[]"}}})
        ),
        1
    );
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"flags": {"$type": "bool[]"}}})
        ),
        1
    );
    // non-boolean field must not match
    assert_eq!(
        match_count(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {"score": {"$type": "boolean"}}})
        ),
        0
    );
}

/// $ne on a missing field must NOT match (`RushDB` semantics).
#[test]
fn ne_requires_presence() {
    let lmpg = setup();
    lmpg.create_record(
        "DOC",
        None,
        vec![("status".into(), FieldChange::Set(json!("active")))],
    )
    .unwrap();
    lmpg.create_record("DOC", None, vec![]).unwrap();

    // { status: { $ne: 'deleted' } } → only the record WITH status active
    let n = match_count(
        &lmpg,
        &json!({"labels": ["DOC"], "where": {"status": {"$ne": "deleted"}}}),
    );
    assert_eq!(n, 1);

    // $exists false still works
    let n = match_count(
        &lmpg,
        &json!({"labels": ["DOC"], "where": {"status": {"$exists": false}}}),
    );
    assert_eq!(n, 1);
}
/// Regression: query-document `labels` must scope the aggregate path (multi-label
/// stores must not leak unrelated roots into COUNT/* or groupBy).
#[test]
fn query_document_labels_scope_aggregates() {
    let lmpg = setup();
    import::import_document(&lmpg, "BOOK", &json!({"amount": 20})).unwrap();
    import::import_document(&lmpg, "BOOK", &json!({"amount": 15})).unwrap();
    import::import_document(&lmpg, "BOOK", &json!({"amount": 5})).unwrap();
    import::import_document(&lmpg, "AUTHOR", &json!({"name": "J"})).unwrap();

    let q = parse_document(&json!({
        "labels": ["BOOK"],
        "select": {"count": {"$count": "*"}, "revenue": {"$sum": "$record.amount"}}
    }))
    .unwrap();
    let buckets = aggregate(&lmpg, q.aggregate.as_ref().unwrap()).unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(
        buckets[0].count_roots, 3,
        "AUTHOR must not leak into BOOK count"
    );
    assert_eq!(buckets[0].sums.get("revenue"), Some(&json!(40.0)));
}

/// P08: streaming bounded execution returns the same page + exact total as the
/// non-streaming path, but retains only the page (not all matched ordinals).
#[test]
fn streaming_bounded_query_matches_nonstreaming_exact_total() {
    let lmpg = setup();
    for i in 0..100 {
        import::import_document(&lmpg, "D", &json!({"n": i, "v": i % 7})).unwrap();
    }
    let doc = json!({ "labels": ["D"], "where": { "v": { "$eq": 3 } }, "limit": 5 });
    let parsed = parse_document(&doc).unwrap();
    let snap = Arc::new(lmpg.store.read_view().unwrap());

    // Non-streaming reference.
    let page = find_records_in_view(&lmpg, &parsed.search, &|| false, snap.clone()).unwrap();
    assert_eq!(page.records.len(), 5);
    assert_eq!(page.total_matched, 14, "v % 7 == 3 over 100 records = 14");

    // Streaming bounded: same page + exact total.
    let streamed = find_records_streaming(&lmpg, &parsed.search, &|| false, snap).unwrap();
    assert_eq!(streamed.records.len(), 5, "streaming returns one page");
    assert_eq!(streamed.total_matched, 14, "streaming total is exact");
    assert_eq!(
        streamed.records, page.records,
        "streaming page matches reference"
    );
}

/// Ordered streaming retains the best global candidates, rather than merely
/// sorting the first page-sized prefix of the scan.
#[test]
fn streaming_ordered_page_matches_materialized_global_order() {
    let lmpg = setup();
    for n in (0..100).rev() {
        import::import_document(&lmpg, "D", &json!({ "n": n })).unwrap();
    }
    let doc = json!({
        "labels": ["D"],
        "orderBy": { "n": "asc" },
        "skip": 7,
        "limit": 5
    });
    let parsed = parse_document(&doc).unwrap();
    let snap = Arc::new(lmpg.store.read_view().unwrap());

    let materialized =
        find_records_in_view(&lmpg, &parsed.search, &|| false, snap.clone()).unwrap();
    let streamed = find_records_streaming(&lmpg, &parsed.search, &|| false, snap).unwrap();
    assert_eq!(streamed.total_matched, 100);
    assert_eq!(streamed.records, materialized.records);
    assert_eq!(
        streamed
            .records
            .iter()
            .map(|record| record["n"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![7, 8, 9, 10, 11]
    );
}

/// Equality with ISO-8601 text matches a datetime at that instant and a
/// string equal to the text, through the index path and a forced scan alike.
#[test]
fn equality_on_iso_text_matches_datetimes_and_strings() {
    let lmpg = setup();
    // 2026-07-23 (midnight UTC) and 2026-07-24T10:00Z as datetimes.
    for (date, kind) in [
        (1_784_764_800_000i64, "start"),
        (1_784_887_200_000i64, "end"),
    ] {
        lmpg.create_record(
            "EV",
            None,
            vec![
                ("date".into(), FieldChange::SetDatetime(date)),
                ("kind".into(), FieldChange::Set(json!(kind))),
            ],
        )
        .unwrap();
    }
    // The same text held as a plain string.
    lmpg.create_record(
        "EV",
        None,
        vec![("code".into(), FieldChange::Set(json!("2026-07-23")))],
    )
    .unwrap();

    for (predicate, expected) in [
        (json!({"date": {"$eq": "2026-07-23"}}), 1),
        (json!({"date": "2026-07-23"}), 1),
        (json!({"date": {"$eq": "2026-07-23T00:00:00.000Z"}}), 1),
        (
            json!({"date": {"$in": ["2026-07-24T10:00:00Z", "1999-01-01"]}}),
            1,
        ),
        (json!({"date": {"$ne": "2026-07-23"}}), 1),
        (json!({"date": {"$nin": ["2026-07-23"]}}), 1),
        // A datetime also equals its epoch milliseconds.
        (json!({"date": 1_784_764_800_000i64}), 1),
        (json!({"code": {"$eq": "2026-07-23"}}), 1),
        (json!({"code": "2026-07-23"}), 1),
        (json!({"code": {"$in": ["2026-07-23"]}}), 1),
    ] {
        let document = json!({"labels": ["EV"], "where": predicate});
        let mut search = parse_document(&document).expect("valid").search;
        let indexed = find_records(&lmpg, &search).expect("search").total_matched;
        search.force_scan = true;
        let scanned = find_records(&lmpg, &search).expect("search").total_matched;
        assert_eq!(indexed, expected, "index path: {predicate}");
        assert_eq!(scanned, expected, "scan: {predicate}");
    }
}
