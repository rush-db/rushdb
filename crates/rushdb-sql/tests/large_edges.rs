//! SQL-4: `rushdb_edges` tables past the old 100,000-relationship ceiling.
//! Edge tables stream from the relationship index instead of being built in
//! memory.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, BatchTx, Lmpg};
use rushdb_sql::{execute_sql, MAX_SCAN_ROWS};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

const PEOPLE: usize = 1_000;
const KNOWS: usize = MAX_SCAN_ROWS + 1;

#[test]
fn edge_tables_stream_past_the_old_ceiling() {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    let people = lmpg
        .run_batch(BatchTx {
            ops: (0..PEOPLE)
                .map(|i| BatchOp::CreateRecord {
                    label: "Person".into(),
                    explicit_id: None,
                    fields: vec![("n".into(), FieldChange::Set(json!(i)))],
                })
                .collect(),
        })
        .unwrap()
        .created_records;
    for chunk in (0..KNOWS).collect::<Vec<_>>().chunks(10_000) {
        let ops = chunk
            .iter()
            .map(|&i| BatchOp::CreateRelationship {
                rel_type: "KNOWS".into(),
                source: people[i % PEOPLE],
                target: people[(i * 7 + 1) % PEOPLE],
                fields: vec![("weight".into(), FieldChange::Set(json!(i % 10)))],
            })
            .collect();
        lmpg.run_batch(BatchTx { ops }).unwrap();
    }
    lmpg.run_batch(BatchTx {
        ops: vec![BatchOp::CreateRelationship {
            rel_type: "LIKES".into(),
            source: people[0],
            target: people[1],
            fields: Vec::new(),
        }],
    })
    .unwrap();
    let count = |sql: &str| execute_sql(&lmpg, sql).unwrap()[0]["c"].as_u64().unwrap();
    assert_eq!(
        count("SELECT COUNT(*) AS c FROM rushdb_edges.knows"),
        KNOWS as u64
    );
    assert_eq!(
        count("SELECT COUNT(*) AS c FROM rushdb_edges.knows WHERE weight = 3"),
        (KNOWS / 10) as u64
    );
    assert_eq!(count("SELECT COUNT(*) AS c FROM rushdb_edges.likes"), 1);
    // Joined with the endpoint table.
    assert_eq!(
        count(
            "SELECT COUNT(*) AS c FROM rushdb_edges.knows k \
             JOIN person p ON p.__id = k.__source_id WHERE p.n = 0"
        ),
        (KNOWS / PEOPLE + 1) as u64
    );
}

/// An edge table's columns and row count come from the relationship catalog
/// without reading a relationship: every property the type's relationships
/// carry is a column of its stored type (arrays as lists, int and float
/// together as float), only the type's own properties are columns, and the
/// rows decode into them.
#[test]
fn edge_columns_come_from_the_catalog() {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    let people = lmpg
        .run_batch(BatchTx {
            ops: (0..2)
                .map(|i| BatchOp::CreateRecord {
                    label: "Person".into(),
                    explicit_id: None,
                    fields: vec![("n".into(), FieldChange::Set(json!(i)))],
                })
                .collect(),
        })
        .unwrap()
        .created_records;
    let rated = |fields: serde_json::Value| BatchOp::CreateRelationship {
        rel_type: "RATED".into(),
        source: people[0],
        target: people[1],
        fields: fields
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| (name.clone(), FieldChange::Set(value.clone())))
            .collect(),
    };
    lmpg.run_batch(BatchTx {
        ops: vec![
            rated(json!({"stars": 4, "mixed": 1, "note": "good", "ok": true,
                         "tags": ["a", "b"], "at": "2024-01-02T03:04:05Z"})),
            rated(json!({"stars": 2, "mixed": 2.5})),
            BatchOp::CreateRelationship {
                rel_type: "LIKES".into(),
                source: people[1],
                target: people[0],
                fields: vec![("since".into(), FieldChange::Set(json!(2020)))],
            },
        ],
    })
    .unwrap();
    let columns = execute_sql(
        &lmpg,
        "SELECT column_name AS c, data_type AS t FROM information_schema.columns \
         WHERE table_schema = 'rushdb_edges' AND table_name = 'rated' ORDER BY c",
    )
    .unwrap();
    let columns: Vec<(String, String)> = columns
        .iter()
        .map(|row| {
            (
                row["c"].as_str().unwrap().to_string(),
                row["t"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let expected = [
        ("__id", "Utf8"),
        ("__source_id", "Utf8"),
        ("__target_id", "Utf8"),
        ("__type", "Utf8"),
        ("at", "Utf8"),
        ("mixed", "Float64"),
        ("note", "Utf8"),
        ("ok", "Boolean"),
        ("stars", "Int64"),
    ];
    for (name, data_type) in expected {
        assert!(
            columns.iter().any(|(c, t)| c == name && t == data_type),
            "{name} {data_type}: {columns:?}"
        );
    }
    assert!(
        columns
            .iter()
            .any(|(c, t)| c == "tags" && t.starts_with("List")),
        "{columns:?}"
    );
    assert!(!columns.iter().any(|(c, _)| c == "since"), "{columns:?}");
    let rows = execute_sql(
        &lmpg,
        "SELECT stars, mixed, note, ok, tags, at FROM rushdb_edges.rated ORDER BY stars",
    )
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["stars"], json!(2));
    assert_eq!(rows[0]["mixed"], json!(2.5));
    assert_eq!(rows[0]["note"], json!(null));
    assert_eq!(rows[1]["mixed"], json!(1.0));
    assert_eq!(rows[1]["tags"], json!(["a", "b"]));
    assert_eq!(rows[1]["at"], json!("2024-01-02T03:04:05Z"));
    let types = execute_sql(&lmpg, "SELECT DISTINCT __type AS t FROM rushdb_edges.rated").unwrap();
    assert_eq!(types[0]["t"], json!("RATED"));
    let count = execute_sql(&lmpg, "SELECT COUNT(*) AS c FROM rushdb_edges.rated").unwrap();
    assert_eq!(count[0]["c"], json!(2));
}
