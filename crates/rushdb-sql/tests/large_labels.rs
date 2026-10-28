//! SQL-2: SQL statements on a label with millions of records. A label
//! table's schema comes from the property catalog, its rows stream, and a
//! filtered scan reads its candidates the way records reads do: from the
//! index plan when an index answers a condition, from a walk of the label
//! otherwise, refused when an unindexed filter would check more records than
//! the unindexed-walk limit. The request's own bounds reach the scan.

use std::sync::Arc;

use rushdb_lmpg::index::IndexStep;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{Lmpg, RequestLimits};
use rushdb_sql::{
    execute_sql_with_parameters_budget_in_view, reset_work_counters, take_work_counters, SqlError,
};
use rushdb_storage::resources::StorageResources;
use rushdb_storage::{ReadView, Store};
use rushdb_types::ids::TenantId;
use rushdb_types::index::IndexKind;
use serde_json::json;

const RECORDS: i64 = 300;

/// `Big` records: `n` = i mod 10, `code` an integer except one text.
fn engine(walk_limit: u64) -> Lmpg {
    engine_of(walk_limit, RECORDS)
}

fn engine_of(walk_limit: u64, records: i64) -> Lmpg {
    let store = Store::new_with_resources(StorageResources {
        unindexed_walk_max_owners: walk_limit,
        ..StorageResources::default()
    })
    .unwrap();
    let lmpg = Lmpg::new(Arc::new(store), TenantId::single());
    for i in 0..records {
        let code = if i == 7 { json!("x") } else { json!(i) };
        lmpg.create_record(
            "Big",
            None,
            vec![
                ("n".into(), FieldChange::Set(json!(i % 10))),
                ("code".into(), FieldChange::Set(code)),
            ],
        )
        .unwrap();
    }
    lmpg
}

fn index(lmpg: &Lmpg, property: &str) {
    let def = lmpg
        .define_index(
            &format!("big_{property}"),
            "Big",
            property,
            IndexKind::Ordered,
        )
        .unwrap();
    while !matches!(
        lmpg.index_step(def.id, 1_000).unwrap(),
        IndexStep::Finished | IndexStep::Idle
    ) {}
}

fn sql_in(lmpg: &Lmpg, view: Arc<ReadView>, sql: &str) -> Result<Vec<serde_json::Value>, SqlError> {
    execute_sql_with_parameters_budget_in_view(lmpg, view, sql, None, None).map(|rows| {
        rows.into_iter()
            .map(|row| serde_json::Value::Object(row.into_iter().collect()))
            .collect()
    })
}

fn sql(lmpg: &Lmpg, sql: &str) -> Result<Vec<serde_json::Value>, SqlError> {
    sql_in(lmpg, Arc::new(lmpg.store.read_view().unwrap()), sql)
}

#[test]
fn a_label_past_the_materialization_ceiling_streams() {
    // At 10M records every statement failed: the table listed its label's
    // members, and rebuilt every one to infer its schema, through a scan
    // capped at 64 MiB (`scan row exceeds batch byte limit`). Here the
    // ceiling is 16 KiB, under the 3,000 records' member keys.
    let lmpg = engine_of(0, 3_000);
    index(&lmpg, "n");
    let view = Arc::new(lmpg.store.read_view().unwrap()).with_materialization_limit(16 * 1024);
    assert!(
        view.range_prefix(&rushdb_types::keys::label_member_prefix(lmpg.tenant, "Big"))
            .is_err(),
        "the ceiling must bind a collected scan"
    );
    let count = |sql: &str| sql_in(&lmpg, Arc::clone(&view), sql).unwrap();
    // Planned on the index.
    assert_eq!(
        count("SELECT count(*) AS c FROM big WHERE n = 5"),
        vec![json!({"c": 300})]
    );
    // Walked: no condition the scan can state.
    assert_eq!(
        count("SELECT count(*) AS c FROM big WHERE n + 1 = 6"),
        vec![json!({"c": 300})]
    );
    assert_eq!(count("SELECT n FROM big LIMIT 3").len(), 3);
}

#[test]
fn an_indexed_condition_reads_only_its_candidates() {
    let lmpg = engine(0);
    index(&lmpg, "n");
    reset_work_counters();
    let rows = sql(&lmpg, "SELECT count(*) AS c FROM big WHERE n = 5").unwrap();
    assert_eq!(rows, vec![json!({"c": 30})]);
    assert_eq!(take_work_counters().candidate_rows, 30);

    // A condition the scan cannot state leaves the indexed one planned.
    reset_work_counters();
    let rows = sql(
        &lmpg,
        "SELECT count(*) AS c FROM big WHERE n = 5 AND (code = 5 OR code = 15)",
    )
    .unwrap();
    assert_eq!(rows, vec![json!({"c": 2})]);
    assert_eq!(take_work_counters().candidate_rows, 30);

    // Ranges widen to inclusive bounds and are re-checked exactly.
    let rows = sql(&lmpg, "SELECT count(*) AS c FROM big WHERE n > 7").unwrap();
    assert_eq!(rows, vec![json!({"c": 60})]);

    // `__id` is one id lookup.
    let id = sql(&lmpg, "SELECT __id FROM big WHERE n = 3 LIMIT 1").unwrap()[0]["__id"]
        .as_str()
        .unwrap()
        .to_owned();
    reset_work_counters();
    let rows = sql(&lmpg, &format!("SELECT n FROM big WHERE __id = '{id}'")).unwrap();
    assert_eq!(rows, vec![json!({"n": 3})]);
    assert_eq!(take_work_counters().candidate_rows, 1);
}

#[test]
fn a_text_comparison_on_a_widened_column_keeps_every_matching_row() {
    // `code` holds integers and one text value, so its column is text and
    // `code = '5'` matches the integer 5. An index probe for the text "5"
    // would miss it, so the condition is not planned on the index.
    let lmpg = engine(0);
    index(&lmpg, "code");
    let rows = sql(&lmpg, "SELECT n FROM big WHERE code = '5'").unwrap();
    assert_eq!(rows, vec![json!({"n": 5})]);
}

#[test]
fn an_unindexed_filter_over_the_walk_limit_is_refused() {
    let lmpg = engine(100);
    let err = sql(&lmpg, "SELECT n FROM big WHERE n = 5").unwrap_err();
    let SqlError::UnindexedWalk(message) = &err else {
        panic!("expected an unindexed-walk refusal, got {err:?}");
    };
    assert!(
        message.contains("`n`") && message.contains("300"),
        "{message}"
    );
    // A filter with no condition the scan can state walks the whole label.
    assert!(matches!(
        sql(&lmpg, "SELECT n FROM big WHERE n + 1 = 6"),
        Err(SqlError::UnindexedWalk(_))
    ));
    // Unfiltered reads walk nothing they do not return.
    assert_eq!(
        sql(&lmpg, "SELECT count(*) AS c FROM big").unwrap(),
        vec![json!({"c": RECORDS})]
    );
    assert_eq!(sql(&lmpg, "SELECT n FROM big LIMIT 2").unwrap().len(), 2);
    // With the index the same filter runs.
    index(&lmpg, "n");
    assert_eq!(
        sql(&lmpg, "SELECT count(*) AS c FROM big WHERE n = 5").unwrap(),
        vec![json!({"c": 30})]
    );
}

#[test]
fn an_unfiltered_limit_stops_after_its_rows() {
    let lmpg = engine(0);
    reset_work_counters();
    assert_eq!(sql(&lmpg, "SELECT n FROM big LIMIT 2").unwrap().len(), 2);
    let read = take_work_counters().candidate_rows;
    assert!(
        read < u64::try_from(RECORDS).unwrap(),
        "read {read} candidates"
    );
}

#[test]
fn a_count_the_index_answers_exactly_rebuilds_no_record() {
    let lmpg = engine(0);
    reset_work_counters();
    assert_eq!(
        sql(&lmpg, "SELECT count(*) AS c FROM big").unwrap(),
        vec![json!({"c": RECORDS})]
    );
    assert_eq!(take_work_counters().reconstructed_rows, 0);

    // Walked: the `Exact` filters are checked strictly per row.
    let counts = |lmpg: &Lmpg| {
        [
            "SELECT count(*) AS c FROM big WHERE n = 5",
            "SELECT count(*) AS c FROM big WHERE n >= 5 AND n <= 6",
            "SELECT count(*) AS c FROM big WHERE n = 5 AND code >= 100",
        ]
        .map(|q| sql(lmpg, q).unwrap()[0]["c"].clone())
    };
    let expected = [json!(30), json!(60), json!(20)];
    assert_eq!(counts(&lmpg), expected);

    // Planned: the index answers `n` exactly, so nothing is rebuilt.
    index(&lmpg, "n");
    reset_work_counters();
    assert_eq!(
        sql(&lmpg, "SELECT count(*) AS c FROM big WHERE n = 5").unwrap(),
        vec![json!({"c": 30})]
    );
    let work = take_work_counters();
    assert_eq!((work.candidate_rows, work.reconstructed_rows), (30, 0));
    assert_eq!(counts(&lmpg), expected);
}

#[test]
fn point_reads_return_the_values_a_rebuilt_record_holds() {
    // `n, code` are scalar properties, read by point reads; adding `__id`
    // rebuilds each record instead. Both must give the same values.
    let lmpg = engine(0);
    let pairs = |q: &str| {
        let mut pairs: Vec<String> = sql(&lmpg, q)
            .unwrap()
            .iter()
            .map(|row| format!("{}|{}", row["n"], row["code"]))
            .collect();
        pairs.sort();
        pairs
    };
    let by_points = pairs("SELECT n, code FROM big");
    assert_eq!(by_points.len(), usize::try_from(RECORDS).unwrap());
    assert_eq!(by_points, pairs("SELECT __id, n, code FROM big"));
}

#[test]
fn the_request_bounds_stop_a_walk_the_server_allows() {
    // `disallow_full_table_scans` with `large_full_scan_rows = 100`.
    let lmpg = engine(0).with_request_limits(RequestLimits {
        unindexed_walk_max_owners: Some(100),
        ..RequestLimits::default()
    });
    assert!(matches!(
        sql(&lmpg, "SELECT n FROM big WHERE n = 5"),
        Err(SqlError::UnindexedWalk(_))
    ));

    // A `count(*)` yields no batch until its walk ends, so the scan itself
    // checks the deadline.
    let lmpg = engine(0).with_request_limits(RequestLimits {
        deadline: Some(std::time::Instant::now()),
        ..RequestLimits::default()
    });
    let err = sql(&lmpg, "SELECT count(*) AS c FROM big WHERE n + 1 = 6").unwrap_err();
    assert!(matches!(err, SqlError::Cancelled), "{err:?}");

    // `work_mem`: a sort of every row needs more than 1 KiB.
    let lmpg = engine(0).with_request_limits(RequestLimits {
        query_memory_max_bytes: Some(1024),
        ..RequestLimits::default()
    });
    let err = sql(&lmpg, "SELECT n, code FROM big ORDER BY code, n").unwrap_err();
    assert!(matches!(err, SqlError::MemoryLimit(1024)), "{err:?}");

    // Unbounded, the same statements run.
    let lmpg = engine(0);
    assert_eq!(
        sql(&lmpg, "SELECT count(*) AS c FROM big WHERE n + 1 = 6").unwrap(),
        vec![json!({"c": 30})]
    );
    assert_eq!(
        sql(&lmpg, "SELECT n, code FROM big ORDER BY code, n")
            .unwrap()
            .len(),
        300
    );
}
