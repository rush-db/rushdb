//! SQL-3: `INSERT`/`UPDATE`/`DELETE` on a tenant past the old 100,000-row
//! materialization ceiling. DML reads its rows the way `SELECT` does (streamed
//! label tables) and commits only the touched records.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, BatchTx, Lmpg};
use rushdb_sql::{
    execute_sql, execute_sql_mutation, execute_sql_mutation_in_view_with_budget, SqlError,
    MAX_SCAN_ROWS,
};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

/// One record past the old ceiling, in a label that also has a small sibling.
const RECORDS: usize = MAX_SCAN_ROWS + 1;

fn engine() -> Lmpg {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    for chunk in (0..RECORDS).collect::<Vec<_>>().chunks(10_000) {
        let ops = chunk
            .iter()
            .map(|&i| BatchOp::CreateRecord {
                label: "Big".into(),
                explicit_id: None,
                fields: vec![
                    ("n".into(), FieldChange::Set(json!(i % 10))),
                    ("Code Name".into(), FieldChange::Set(json!(format!("c{i}")))),
                ],
            })
            .collect();
        lmpg.run_batch(BatchTx { ops }).unwrap();
    }
    lmpg.create_record(
        "Small",
        None,
        vec![("n".into(), FieldChange::Set(json!(3)))],
    )
    .unwrap();
    lmpg
}

fn count(lmpg: &Lmpg, sql: &str) -> u64 {
    execute_sql(lmpg, sql).unwrap()[0]["c"].as_u64().unwrap()
}

#[test]
fn dml_on_a_label_past_the_old_ceiling() {
    let lmpg = engine();
    let tenth = (RECORDS / 10) as u64; // rows with n = 3 (RECORDS - 1 ends in 0)

    // UPDATE by predicate, with a parameter and a quoted mixed-case column.
    let updated = execute_sql_mutation(
        &lmpg,
        "UPDATE big SET \"Code Name\" = 'hit' WHERE n = $1",
        Some(&json!([3])),
    )
    .unwrap();
    assert_eq!(updated.affected, tenth);
    assert_eq!(
        count(
            &lmpg,
            "SELECT COUNT(*) AS c FROM big WHERE \"Code Name\" = 'hit'"
        ),
        tenth
    );

    // An UPDATE touching every row, the whole label in one commit.
    let all = execute_sql_mutation(&lmpg, "UPDATE big SET n = n + 10", None).unwrap();
    assert_eq!(all.affected, RECORDS as u64);
    assert_eq!(
        count(&lmpg, "SELECT COUNT(*) AS c FROM big WHERE n < 10"),
        0
    );

    // A row that already holds the value is matched but not rewritten.
    let same = execute_sql_mutation(&lmpg, "UPDATE small SET n = 3", None).unwrap();
    assert_eq!((same.affected, same.commit_index), (1, all.commit_index));

    // DELETE by predicate.
    let deleted = execute_sql_mutation(&lmpg, "DELETE FROM big WHERE n = 13", None).unwrap();
    assert_eq!(deleted.affected, tenth);
    assert_eq!(
        count(&lmpg, "SELECT COUNT(*) AS c FROM big"),
        RECORDS as u64 - tenth
    );

    // INSERT, with and without a column list, into the large tenant.
    let inserted =
        execute_sql_mutation(&lmpg, "INSERT INTO small (n) VALUES (7), (8)", None).unwrap();
    assert_eq!(inserted.affected, 2);
    // Without a column list, values follow the table's column order (as
    // `SELECT *` shows it): properties, then `__id`.
    let id = "00000000-0000-7000-8000-000000000042";
    execute_sql_mutation(
        &lmpg,
        &format!("INSERT INTO small VALUES (9, '{id}')"),
        None,
    )
    .unwrap();
    assert_eq!(count(&lmpg, "SELECT COUNT(*) AS c FROM small"), 4);
    assert!(matches!(
        execute_sql_mutation(
            &lmpg,
            &format!("INSERT INTO small VALUES (9, '{id}')"),
            None
        ),
        Err(SqlError::InvalidIdentity(_))
    ));

    // UPDATE ... FROM another table.
    let from = execute_sql_mutation(
        &lmpg,
        "UPDATE small SET n = s.n + 100 FROM small s WHERE small.__id = s.__id AND s.n = 9",
        None,
    )
    .unwrap();
    assert_eq!(from.affected, 1);
    assert_eq!(
        count(&lmpg, "SELECT COUNT(*) AS c FROM small WHERE n = 109"),
        1
    );
}

#[test]
fn dml_write_set_is_charged_to_the_request_budget() {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    for i in 0..100 {
        lmpg.create_record("T", None, vec![("n".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }
    let version = lmpg.store.committed_version();
    let snapshot = Arc::new(lmpg.store.read_view().unwrap());
    let error = execute_sql_mutation_in_view_with_budget(
        &lmpg,
        snapshot,
        "UPDATE t SET n = n + 1",
        None,
        Some(2_000),
        None,
    )
    .expect_err("100 planned writes exceed 2,000 bytes");
    assert!(matches!(error, SqlError::ResourceLimit(_)), "{error:?}");
    assert_eq!(lmpg.store.committed_version(), version);
}

#[test]
fn unsupported_dml_shapes_fail_closed() {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    lmpg.create_record("T", None, vec![("n".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    for statement in [
        "UPDATE t SET n = 2 RETURNING n",
        "DELETE FROM t RETURNING n",
        "INSERT INTO t (n) VALUES (1) ON CONFLICT DO NOTHING",
        "UPDATE missing SET n = 1",
        "INSERT INTO t (n) VALUES (1, 2)",
    ] {
        assert!(
            matches!(
                execute_sql_mutation(&lmpg, statement, None),
                Err(SqlError::UnsupportedMutation(_))
            ),
            "{statement}"
        );
    }
    assert_eq!(
        execute_sql(&lmpg, "SELECT COUNT(*) AS c FROM t").unwrap()[0]["c"],
        1
    );
}

#[test]
fn unknown_tables_suggest_the_closest_names() {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    lmpg.create_record(
        "Patient",
        None,
        vec![("birth_date".into(), FieldChange::Set(json!("1990-01-01")))],
    )
    .unwrap();
    let read = execute_sql(&lmpg, "SELECT * FROM patients")
        .unwrap_err()
        .to_string();
    assert!(read.contains("Did you mean 'Patient'?"), "{read}");
    // DataFusion suggests close column names itself.
    let column = execute_sql(&lmpg, "SELECT birthdate FROM patient")
        .unwrap_err()
        .to_string();
    assert!(column.contains("birth_date"), "{column}");
    let write = execute_sql_mutation(&lmpg, "DELETE FROM patients", None)
        .unwrap_err()
        .to_string();
    assert!(write.contains("did you mean `Patient`?"), "{write}");
}
