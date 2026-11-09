//! Cost follows the result, not the label: each query below reads about the
//! same backend rows whether the label holds 1,000 or 10,000 records, as
//! long as the records it matches are the same. A query whose reads grow
//! with the label (a walk where a lookup was planned, an early stop that no
//! longer stops) fails here, before a benchmark finds it. Read counts are
//! deterministic, so the bounds are tight.

use std::sync::Arc;

use rushdb_lmpg::index::IndexStep;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, Lmpg};
use rushdb_query::document::parse_document;
use rushdb_query::{aggregate, find_records};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use rushdb_types::index::IndexKind;
use serde_json::{json, Value};

/// `records` records of label `Item`; the first 50 have `group: "hit"`,
/// every other `group: "miss-<n % 100>"`. `group` and `score` are indexed.
fn store(records: u64) -> Lmpg {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(5));
    for chunk in (0..records).collect::<Vec<_>>().chunks(2_000) {
        let mut tx = lmpg.begin_batch();
        for &n in chunk {
            let group = if n < 50 {
                "hit".to_owned()
            } else {
                format!("miss-{}", n % 100)
            };
            tx.ops.push(BatchOp::CreateRecord {
                label: "Item".into(),
                explicit_id: None,
                fields: vec![
                    ("group".into(), FieldChange::Set(json!(group))),
                    ("score".into(), FieldChange::Set(json!(n))),
                ],
            });
        }
        lmpg.run_batch(tx).unwrap();
    }
    for property in ["group", "score"] {
        let def = lmpg
            .define_index(
                &format!("item_{property}"),
                "Item",
                property,
                IndexKind::Ordered,
            )
            .unwrap();
        while !matches!(
            lmpg.index_step(def.id, 10_000).unwrap(),
            IndexStep::Finished | IndexStep::Idle
        ) {}
    }
    lmpg
}

/// Backend reads of `run` on this thread, after one warm-up run (planner
/// statistics load on first use and are cached; a steady query does not
/// pay for them).
fn reads(run: impl Fn()) -> u64 {
    run();
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    run();
    rushdb_storage::read_accounting::take_local().values().sum()
}

fn records_reads(lmpg: &Lmpg, document: &Value) -> u64 {
    let search = parse_document(document).unwrap().search;
    reads(|| {
        find_records(lmpg, &search).unwrap();
    })
}

fn aggregate_reads(lmpg: &Lmpg, document: &Value) -> u64 {
    let request = parse_document(document).unwrap().aggregate.unwrap();
    reads(|| {
        aggregate(lmpg, &request).unwrap();
    })
}

/// Reads at 1,000 and at 10,000 records; the larger may cost at most
/// `slack` more (a deeper index or counter lookup), never ten times more.
fn assert_flat(name: &str, measure: impl Fn(&Lmpg) -> u64) {
    let small = measure(&store(1_000));
    let large = measure(&store(10_000));
    assert!(
        large <= small + small / 5 + 20,
        "{name}: {small} reads at 1,000 records, {large} at 10,000"
    );
}

#[test]
fn an_indexed_equality_page_reads_its_matches_only() {
    assert_flat("records where group = hit", |lmpg| {
        records_reads(
            lmpg,
            &json!({"labels": ["Item"], "where": {"group": "hit"}}),
        )
    });
}

#[test]
fn an_indexed_range_page_reads_its_matches_only() {
    assert_flat("records where score < 50", |lmpg| {
        records_reads(
            lmpg,
            &json!({"labels": ["Item"], "where": {"score": {"$lt": 50}}}),
        )
    });
}

#[test]
fn an_unfiltered_page_stops_at_its_limit() {
    assert_flat("records limit 10", |lmpg| {
        records_reads(lmpg, &json!({"labels": ["Item"], "limit": 10}))
    });
}

#[test]
fn an_indexed_count_reads_its_matches_only() {
    assert_flat("count where group = hit", |lmpg| {
        aggregate_reads(
            lmpg,
            &json!({"labels": ["Item"], "where": {"group": "hit"},
                    "select": {"n": {"$count": "*"}}}),
        )
    });
}

/// Reads of `run` by key family (one warm-up run first).
fn family_reads(run: impl Fn()) -> std::collections::BTreeMap<(&'static str, String), u64> {
    run();
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    run();
    rushdb_storage::read_accounting::take_local()
}

#[test]
fn an_indexed_grouped_metric_aggregate_reads_its_matches_only() {
    rushdb_storage::parallel::set_threads(1);
    assert_flat(
        "groupBy group: count + sum score where group = hit",
        |lmpg| {
            aggregate_reads(
                lmpg,
                &json!({"labels": ["Item"], "where": {"group": "hit"}, "groupBy": ["group"],
                    "select": {"n": {"$count": "*"}, "total": {"$sum": "$record.score"}}}),
            )
        },
    );
}

#[test]
fn a_grouped_metric_aggregate_reads_each_record_once() {
    // Parts run on this thread, so its read counts see every read.
    rushdb_storage::parallel::set_threads(1);
    let document = json!({"labels": ["Item"], "groupBy": ["group"],
        "select": {"n": {"$count": "*"}, "total": {"$sum": "$record.score"}}});
    let request = parse_document(&document).unwrap().aggregate.unwrap();
    let per_record = |records: u64| {
        let lmpg = store(records);
        let reads = family_reads(|| {
            aggregate(&lmpg, &request).unwrap();
        });
        let header: u64 = reads
            .iter()
            .filter(|((_, family), _)| family == "record-header")
            .map(|(_, n)| n)
            .sum();
        let occurrences: u64 = reads
            .iter()
            .filter(|((_, family), _)| family == "occurrence")
            .map(|(_, n)| n)
            .sum();
        // No record header per candidate (the label scan only yields live
        // records), and one point read per property per record.
        assert_eq!(header, 0, "{records}: {reads:?}");
        assert_eq!(occurrences, 2 * records, "{records}: {reads:?}");
        (reads.values().sum::<u64>(), records)
    };
    let ((small, small_records), (large, large_records)) = (per_record(1_000), per_record(10_000));
    // Linear in the records: ten times the records cost at most ten times
    // the reads (plus a few for a deeper counter or catalog lookup).
    assert!(
        large * small_records <= (small + 20) * large_records,
        "{small} reads at {small_records} records, {large} at {large_records}"
    );
}
