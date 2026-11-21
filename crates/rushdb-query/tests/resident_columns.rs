//! Grouped aggregates read their cells from resident columns once the
//! columns are built, and answer exactly as reading storage does: the same
//! groups, the same values bit for bit, the same errors. Columns built for
//! one snapshot are not used after a write to what they reflect, are
//! refused when the budget cannot hold them, and are never shared across
//! tenants.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    reason = "small fixture integers"
)]

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, Lmpg};
use rushdb_query::document::parse_document;
use rushdb_query::{aggregate, aggregate_to_rows, AggregateRequest};
use rushdb_storage::resources::StorageResources;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

const ROWS: u64 = 3_000;

fn store(column_cache_bytes: usize) -> Arc<Store> {
    Arc::new(
        Store::new_with_resources(StorageResources {
            column_cache_bytes,
            ..StorageResources::default()
        })
        .unwrap(),
    )
}

/// Every kind of cell a column holds, plus a property stored as an array
/// (no column: it is read from storage).
fn row(i: u64) -> Vec<(String, FieldChange)> {
    let set = |name: &str, value: Value| (name.to_owned(), FieldChange::Set(value));
    let mut fields = Vec::new();
    if !i.is_multiple_of(13) {
        fields.push(set(
            "k",
            json!(["a", "b", "c", "d", "e", "f", "g"][(i % 7) as usize]),
        ));
    }
    fields.push(set(
        "i",
        json!(match i % 5 {
            0 => i64::MAX / 4,
            1 => -(i as i64),
            2 => 9_007_199_254_740_993_i64,
            _ => i as i64,
        }),
    ));
    fields.push(set(
        "f",
        json!(match i % 4 {
            0 => -0.0,
            1 => 1.0e300,
            2 => (i as f64) / 7.0,
            _ => -1.5,
        }),
    ));
    // One property, three stored types.
    fields.push(set(
        "mixed",
        match i % 3 {
            0 => json!(i % 40),
            1 => json!((i % 40) as f64 + 0.25),
            _ => json!(format!("s{}", i % 4)),
        },
    ));
    fields.push(set("b", json!(i.is_multiple_of(2))));
    if i.is_multiple_of(10) {
        fields.push(("n".to_owned(), FieldChange::ExplicitNull));
    } else {
        fields.push(set("n", json!(i % 3)));
    }
    fields.push((
        "d".to_owned(),
        FieldChange::Set(rushdb_types::types::to_doc_json(
            &rushdb_types::value::Scalar::DateTime(
                rushdb_types::types::temporal::Instant::from_millis(
                    1_700_000_000_000 + (i as i64) * 86_400_000,
                )
                .unwrap(),
            ),
        )),
    ));
    if i.is_multiple_of(9) {
        fields.push(set("tags", json!(["x", "y"])));
    } else {
        fields.push(set("tags", json!("x")));
    }
    fields
}

fn seed(lmpg: &Lmpg) {
    seed_rows(lmpg, ROWS);
}

fn seed_rows(lmpg: &Lmpg, rows: u64) {
    for chunk in (0..rows).collect::<Vec<_>>().chunks(1_000) {
        let mut tx = lmpg.begin_batch();
        for &i in chunk {
            tx.ops.push(BatchOp::CreateRecord {
                label: "Row".into(),
                explicit_id: None,
                fields: row(i),
            });
        }
        lmpg.run_batch(tx).unwrap();
    }
}

fn request(doc: &Value) -> AggregateRequest {
    parse_document(doc).unwrap().aggregate.unwrap()
}

fn shapes() -> Vec<AggregateRequest> {
    [
        json!({"labels": ["Row"], "groupBy": ["k"],
            "select": {"n": {"$count": "*"}, "f": {"$sum": "$record.f"}, "lo": {"$min": "$record.f"},
                       "hi": {"$max": "$record.f"}, "avg": {"$avg": "$record.f"}}}),
        json!({"labels": ["Row"], "groupBy": ["b", "k"],
            "select": {"n": {"$count": "*"}, "mixed_lo": {"$min": "$record.mixed"},
                       "mixed_hi": {"$max": "$record.mixed"}, "mixed_avg": {"$avg": "$record.mixed"}}}),
        json!({"labels": ["Row"], "groupBy": ["mixed"], "select": {"n": {"$count": "*"}}}),
        json!({"labels": ["Row"], "groupBy": ["n"],
            "select": {"rows": {"$count": "*"}, "d": {"$max": "$record.d"}, "f": {"$avg": "$record.f"}}}),
        json!({"labels": ["Row"], "where": {"b": true, "f": {"$lt": 100}}, "groupBy": ["k"],
            "select": {"n": {"$count": "*"}, "f": {"$sum": "$record.f"}}}),
        json!({"labels": ["Row"], "where": {"mixed": {"$gte": 20}}, "groupBy": ["b"],
            "select": {"n": {"$count": "*"}, "lo": {"$min": "$record.mixed"}}}),
        json!({"labels": ["Row"], "where": {"d": {"$gte": "2024-01-01T00:00:00Z"}}, "groupBy": ["k"],
            "select": {"n": {"$count": "*"}}}),
        // Read from storage: a property stored as an array somewhere.
        json!({"labels": ["Row"], "groupBy": ["tags"], "select": {"n": {"$count": "*"}}}),
        // An integer sum that overflows: the same error either way.
        json!({"labels": ["Row"], "groupBy": ["b"], "select": {"i": {"$sum": "$record.i"}}}),
        // Counts and sums over text and mixed cells (text is counted, never
        // summed), extremes of text.
        json!({"labels": ["Row"], "groupBy": ["b"],
            "select": {"k": {"$count": "$record.k"}, "mixed": {"$count": "$record.mixed"},
                       "mixed_sum": {"$sum": "$record.mixed"}, "k_lo": {"$min": "$record.k"},
                       "k_hi": {"$max": "$record.k"}, "n": {"$count": "$record.n"}}}),
        // Four keys fold by code; five fold built rows.
        json!({"labels": ["Row"], "groupBy": ["b", "k", "n", "mixed"],
            "select": {"c": {"$count": "*"}, "f": {"$sum": "$record.f"}}}),
        json!({"labels": ["Row"], "groupBy": ["b", "k", "n", "mixed", "d"],
            "select": {"c": {"$count": "*"}, "f": {"$avg": "$record.f"}}}),
    ]
    .iter()
    .map(request)
    .collect()
}

/// The answer to `req`, rendered, or the error.
fn answer(lmpg: &Lmpg, req: &AggregateRequest) -> String {
    match aggregate(lmpg, req).and_then(|buckets| aggregate_to_rows(lmpg, &buckets, req)) {
        Ok(rows) => serde_json::to_string(&rows).unwrap(),
        Err(error) => format!("error: {error}"),
    }
}

fn reference(lmpg: &Lmpg) -> Vec<String> {
    let residents = lmpg.store.residents();
    residents.set_enabled(false);
    let answers = shapes().iter().map(|req| answer(lmpg, req)).collect();
    residents.set_enabled(rushdb_storage::resident::residents_enabled());
    answers
}

fn column_counters(lmpg: &Lmpg) -> (u64, u64, u64) {
    lmpg.store
        .residents()
        .snapshots()
        .into_iter()
        .find(|layer| layer.layer == "column")
        .map_or((0, 0, 0), |layer| {
            (layer.builds, layer.hits, layer.build_refused)
        })
}

/// Every shape answers like storage: before the columns exist (the reads
/// pay toward them), and after they are built.
fn assert_answers_like_storage(lmpg: &Lmpg, expected: &[String]) {
    for _ in 0..2 {
        for (req, want) in shapes().iter().zip(expected) {
            assert_eq!(&answer(lmpg, req), want, "{req:?}");
        }
        lmpg.store.residents().wait_idle();
    }
}

#[test]
fn grouped_aggregates_answer_from_columns_like_storage() {
    let lmpg = Lmpg::new(store(256 << 20), TenantId::from_ordinal(170));
    seed(&lmpg);
    let expected = reference(&lmpg);
    assert!(
        expected.iter().any(|a| a.starts_with("error:")),
        "the overflow shape errs"
    );
    assert!(expected.iter().filter(|a| !a.starts_with("error:")).count() >= 8);
    assert_answers_like_storage(&lmpg, &expected);
    let (builds, hits, _) = column_counters(&lmpg);
    if rushdb_storage::resident::residents_enabled() {
        assert!(builds >= 6, "columns were built: {builds}");
        assert!(hits > 0, "and answered: {hits}");
    } else {
        assert_eq!((builds, hits), (0, 0));
    }

    // A value change of existing records only (no new member, property or
    // type): their columns are not used after it.
    let snap = lmpg.store.read_view().unwrap();
    for ord in rushdb_lmpg::reader::try_access::label_members(&snap, lmpg.tenant, "Row")
        .unwrap()
        .into_iter()
        .take(40)
    {
        let header = rushdb_lmpg::reader::try_access::record_by_ord(&snap, lmpg.tenant, ord)
            .unwrap()
            .unwrap();
        let record = rushdb_lmpg::RecordRef {
            record_id: header.record_id,
            ordinal: ord,
        };
        lmpg.update_fields(record, vec![("f".into(), FieldChange::Set(json!(7.0e299)))])
            .unwrap();
    }
    let expected = reference(&lmpg);
    assert_answers_like_storage(&lmpg, &expected);

    // A new member and a new property type: columns of the old snapshot are
    // not used.
    let mut tx = lmpg.begin_batch();
    tx.ops.push(BatchOp::CreateRecord {
        label: "Row".into(),
        explicit_id: None,
        fields: vec![
            ("k".into(), FieldChange::Set(json!("z"))),
            ("f".into(), FieldChange::Set(json!(2.5))),
            ("mixed".into(), FieldChange::Set(json!(true))),
        ],
    });
    lmpg.run_batch(tx).unwrap();
    let expected = reference(&lmpg);
    assert_answers_like_storage(&lmpg, &expected);
}

#[test]
fn grouped_aggregates_over_several_parts_answer_like_storage() {
    // More members than one part holds (8,192): the parts fold by code on
    // their own threads, each with its own code cache, and merge in part
    // order, as storage's parts do.
    let lmpg = Lmpg::new(store(256 << 20), TenantId::from_ordinal(176));
    seed_rows(&lmpg, 20_000);
    let expected = reference(&lmpg);
    assert_answers_like_storage(&lmpg, &expected);
}

#[test]
fn columns_over_the_budget_are_refused_and_storage_answers() {
    let lmpg = Lmpg::new(store(4 * 1024), TenantId::from_ordinal(171));
    seed(&lmpg);
    let expected = reference(&lmpg);
    assert_answers_like_storage(&lmpg, &expected);
    let (builds, hits, refused) = column_counters(&lmpg);
    assert_eq!(hits, 0, "nothing fits 4 KiB");
    if rushdb_storage::resident::residents_enabled() {
        assert!(
            refused > 0,
            "builds were refused: {builds} built, {refused} refused"
        );
    }
}

#[test]
fn a_column_is_never_read_for_another_tenant() {
    let store = store(256 << 20);
    let mine = Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(172));
    let other = Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(173));
    seed(&mine);
    // The other tenant: same label and properties, different values.
    let mut tx = other.begin_batch();
    for i in 0..50 {
        tx.ops.push(BatchOp::CreateRecord {
            label: "Row".into(),
            explicit_id: None,
            fields: vec![
                ("k".into(), FieldChange::Set(json!("other"))),
                ("f".into(), FieldChange::Set(json!(i))),
            ],
        });
    }
    other.run_batch(tx).unwrap();
    let expected_mine = reference(&mine);
    let expected_other = reference(&other);
    assert_answers_like_storage(&mine, &expected_mine);
    // Columns of `mine` are built; the other tenant still answers its own.
    for (req, want) in shapes().iter().zip(&expected_other) {
        assert_eq!(&answer(&other, req), want, "{req:?}");
    }
}

/// Reads `run` makes on this thread (helpers it lends work to included).
fn reads(run: impl Fn()) -> u64 {
    run();
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    run();
    rushdb_storage::read_accounting::take_local().values().sum()
}

#[test]
fn a_filter_over_resident_columns_reads_no_storage_rows() {
    rushdb_storage::parallel::set_threads(1);
    let lmpg = Lmpg::new(store(256 << 20), TenantId::from_ordinal(174));
    seed(&lmpg);
    let filtered = request(
        &json!({"labels": ["Row"], "where": {"b": true, "n": {"$gte": 1}},
        "groupBy": ["k"], "select": {"n": {"$count": "*"}, "f": {"$sum": "$record.f"}}}),
    );
    let residents = lmpg.store.residents();
    residents.set_enabled(false);
    let expected = answer(&lmpg, &filtered);
    let from_storage = reads(|| {
        answer(&lmpg, &filtered);
    });
    residents.set_enabled(rushdb_storage::resident::residents_enabled());
    // The planner's candidates pay toward the predicate's columns, the
    // rows' reads toward theirs.
    for _ in 0..4 {
        assert_eq!(answer(&lmpg, &filtered), expected);
        residents.wait_idle();
    }
    let from_columns = reads(|| {
        assert_eq!(answer(&lmpg, &filtered), expected);
    });
    if rushdb_storage::resident::residents_enabled() {
        assert!(
            from_columns * 20 < from_storage,
            "{from_columns} reads from columns, {from_storage} from storage"
        );
    } else {
        assert_eq!(from_columns, from_storage);
    }
}

#[test]
fn tight_query_budgets_never_refuse_what_storage_answers() {
    // Resident columns never refuse an aggregate storage answers within the
    // same query budget; they may answer one storage refuses (they hold
    // less), and then the answer is the unbounded one.
    let unbounded = Lmpg::new(store(256 << 20), TenantId::from_ordinal(176));
    seed_rows(&unbounded, 400);
    let full: Vec<String> = shapes().iter().map(|req| answer(&unbounded, req)).collect();
    let mut refused = 0;
    // Fine steps, so some budget is decided by the transient row itself.
    for budget in (2_048..24_576).step_by(256) {
        let lmpg = Lmpg::new(
            Arc::new(
                Store::new_with_resources(StorageResources {
                    query_memory_max_bytes: budget,
                    ..StorageResources::default()
                })
                .unwrap(),
            ),
            TenantId::from_ordinal(175),
        );
        seed_rows(&lmpg, 400);
        let expected = reference(&lmpg);
        refused += expected
            .iter()
            .filter(|a| a.contains("query-memory-max"))
            .count();
        for _ in 0..2 {
            for ((req, want), unbounded) in shapes().iter().zip(&expected).zip(&full) {
                let got = answer(&lmpg, req);
                if want.contains("query-memory-max") {
                    assert!(
                        &got == want || &got == unbounded,
                        "{budget}: {req:?}: {got}"
                    );
                } else {
                    assert_eq!(&got, want, "{budget}: {req:?}");
                }
            }
            lmpg.store.residents().wait_idle();
        }
    }
    assert!(refused > 0, "some budget refuses an aggregate");
}

/// L8: a listing whose filter no declared index answers reads the matches
/// from the resident columns of the properties it filters on, once its
/// walks have paid for them, and lists what storage lists: the same
/// records, total and order, page by page.
#[test]
fn unindexed_listing_filters_answer_from_columns_like_storage() {
    rushdb_storage::parallel::set_threads(1);
    let lmpg = Lmpg::new(store(256 << 20), TenantId::from_ordinal(176));
    seed(&lmpg);
    let filters = [
        json!({"k": "c"}),
        json!({"k": {"$in": ["a", "g"]}, "b": true}),
        json!({"i": {"$gt": 1000}, "f": {"$lt": 0}}),
        json!({"mixed": {"$gte": 10}}),
        json!({"mixed": "s2"}),
        json!({"n": null}),
        json!({"n": {"$exists": true}, "k": {"$nin": ["a"]}}),
        json!({"$or": [{"k": "b"}, {"i": {"$lt": 0}}]}),
        json!({"k": {"$not": {"$in": ["a", "b", "c"]}}}),
        json!({"mixed": {"$type": "string"}}),
        json!({"k": {"$contains": "e"}}),
        json!({"k": {"$exists": false}}),
        json!({"d": {"$gte": "2024-01-01T00:00:00Z"}}),
    ];
    let documents: Vec<Value> = filters
        .iter()
        .flat_map(|filter| {
            [
                json!({"labels": ["Row"], "where": filter, "limit": 40}),
                json!({"labels": ["Row"], "where": filter, "orderBy": {"i": "desc"},
                    "skip": 10, "limit": 25}),
            ]
        })
        .collect();
    let list = |doc: &Value| {
        let page = rushdb_query::find_records(&lmpg, &parse_document(doc).unwrap().search).unwrap();
        let ids: Vec<Value> = page.records.iter().map(|r| r["__id"].clone()).collect();
        (page.total_matched, ids)
    };
    let residents = lmpg.store.residents();
    residents.set_enabled(false);
    let expected: Vec<_> = documents.iter().map(list).collect();
    let from_storage = reads(|| {
        list(&documents[0]);
    });
    residents.set_enabled(rushdb_storage::resident::residents_enabled());
    for _ in 0..4 {
        for (doc, want) in documents.iter().zip(&expected) {
            assert_eq!(&list(doc), want, "{doc}");
        }
        residents.wait_idle();
    }
    let from_columns = reads(|| {
        assert_eq!(list(&documents[0]), expected[0]);
    });
    if rushdb_storage::resident::residents_enabled() {
        assert!(
            from_columns * 2 < from_storage,
            "{from_columns} reads with columns, {from_storage} from storage"
        );
    }
}
