//! PostgreSQL clients and BI tools list a database through `pg_catalog` and
//! `information_schema`. The statements below are the ones the PostgreSQL
//! JDBC driver and Metabase send (as captured from Metabase 0.65 syncing a
//! `RushDB` database), with their parameters inlined and the session's
//! `current_database()` (bound by the PostgreSQL wire) written as its value.

use std::collections::BTreeMap;
use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, BatchTx, Lmpg};
use rushdb_sql::{execute_sql, execute_sql_under_policy_in_view};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use rushdb_types::security::{ActivePolicy, AuthorizationMode, DataAccessPolicy};
use serde_json::{json, Value};

/// Two `Customer` records and one `Secret`, linked by `OWNS`.
fn shop() -> Lmpg {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    let records = lmpg
        .run_batch(BatchTx {
            ops: vec![
                BatchOp::CreateRecord {
                    label: "Customer".into(),
                    explicit_id: None,
                    fields: vec![
                        ("name".into(), FieldChange::Set(json!("Ann"))),
                        ("age".into(), FieldChange::Set(json!(34))),
                        ("balance".into(), FieldChange::Set(json!(1.5))),
                    ],
                },
                BatchOp::CreateRecord {
                    label: "Customer".into(),
                    explicit_id: None,
                    fields: vec![("name".into(), FieldChange::Set(json!("Bob")))],
                },
                BatchOp::CreateRecord {
                    label: "Secret".into(),
                    explicit_id: None,
                    fields: vec![("code".into(), FieldChange::Set(json!("x")))],
                },
            ],
        })
        .unwrap()
        .created_records;
    lmpg.run_batch(BatchTx {
        ops: vec![BatchOp::CreateRelationship {
            rel_type: "OWNS".into(),
            source: records[0],
            target: records[2],
            fields: Vec::new(),
        }],
    })
    .unwrap();
    lmpg
}

fn column(rows: &[serde_json::Map<String, Value>], name: &str) -> Vec<Value> {
    rows.iter().map(|row| row[name].clone()).collect()
}

#[test]
fn jdbc_lists_the_schemas() {
    // `DatabaseMetaData.getSchemas()`.
    let rows = execute_sql(
        &shop(),
        "SELECT nspname AS \"TABLE_SCHEM\", 'rushdb' AS \"TABLE_CATALOG\" \
         FROM pg_catalog.pg_namespace  WHERE nspname <> 'pg_toast' AND (nspname !~ '^pg_temp_' \
         OR nspname = (pg_catalog.current_schemas(true))[1]) AND (nspname !~ '^pg_toast_temp_' \
         OR nspname = replace((pg_catalog.current_schemas(true))[1], 'pg_temp_', 'pg_toast_temp_')) \
         ORDER BY \"TABLE_SCHEM\"",
    )
    .unwrap();
    assert_eq!(
        column(&rows, "TABLE_SCHEM"),
        vec![
            json!("information_schema"),
            json!("pg_catalog"),
            json!("public"),
            json!("rushdb_edges")
        ]
    );
}

#[test]
fn metabase_lists_the_tables_with_their_sizes() {
    let rows = execute_sql(
        &shop(),
        "SELECT \"n\".\"nspname\" AS \"schema\", \"c\".\"relname\" AS \"name\", \
         CASE \"c\".\"relkind\" WHEN 'r' THEN 'TABLE' WHEN 'p' THEN 'PARTITIONED TABLE' \
         WHEN 'v' THEN 'VIEW' WHEN 'f' THEN 'FOREIGN TABLE' WHEN 'm' THEN 'MATERIALIZED VIEW' \
         ELSE NULL END AS \"type\", \"d\".\"description\" AS \"description\", \
         NULLIF(\"stat\".\"n_live_tup\", 0) AS \"estimated_row_count\" \
         FROM \"pg_catalog\".\"pg_class\" AS \"c\" INNER JOIN \"pg_catalog\".\"pg_namespace\" AS \"n\" \
         ON \"c\".\"relnamespace\" = \"n\".\"oid\" LEFT JOIN \"pg_catalog\".\"pg_description\" AS \"d\" \
         ON (\"c\".\"oid\" = \"d\".\"objoid\") AND (\"d\".\"objsubid\" = 0) \
         AND (\"d\".\"classoid\" = 'pg_class'::regclass) \
         LEFT JOIN \"pg_stat_user_tables\" AS \"stat\" ON (\"n\".\"nspname\" = \"stat\".\"schemaname\") \
         AND (\"c\".\"relname\" = \"stat\".\"relname\") \
         WHERE (\"c\".\"relnamespace\" = \"n\".\"oid\") AND (\"n\".\"nspname\" !~ '^information_schema|catalog_history|pg_') \
         AND (\"n\".\"nspname\" <> 'information_schema') AND c.relkind in ('r', 'p', 'v', 'f', 'm') \
         AND (\"n\".\"nspname\" IN ('public', 'rushdb_edges')) \
         ORDER BY \"type\" ASC, \"schema\" ASC, \"name\" ASC",
    )
    .unwrap();
    let tables: Vec<(Value, Value, Value)> = rows
        .iter()
        .map(|row| {
            (
                row["schema"].clone(),
                row["name"].clone(),
                row["estimated_row_count"].clone(),
            )
        })
        .collect();
    assert_eq!(
        tables,
        vec![
            (json!("public"), json!("Customer"), json!(2)),
            (json!("public"), json!("Secret"), json!(1)),
            (json!("rushdb_edges"), json!("owns"), json!(1)),
        ]
    );
}

#[test]
fn metabase_reads_each_column_with_its_postgresql_type() {
    // The `information_schema` half of Metabase's field sync; its
    // `pg_catalog` half lists the same columns from `pg_attribute`.
    let lmpg = shop();
    let rows = execute_sql(
        &lmpg,
        "SELECT \"c\".\"column_name\" AS \"name\", CASE WHEN \"c\".\"udt_schema\" IN ('public', 'pg_catalog') \
         THEN FORMAT('%s', \"c\".\"udt_name\") ELSE FORMAT('\"%s\".\"%s\"', \"c\".\"udt_schema\", \"c\".\"udt_name\") END \
         AS \"database-type\", \"c\".\"ordinal_position\" - 1 AS \"database-position\", \
         \"pk\".\"column_name\" IS NOT NULL AS \"pk?\", \
         COL_DESCRIPTION(CAST(CAST(FORMAT('%I.%I', CAST(\"c\".\"table_schema\" AS TEXT), CAST(\"c\".\"table_name\" AS TEXT)) AS REGCLASS) AS OID), \"c\".\"ordinal_position\") AS \"field-comment\", \
         \"is_generated\" = 'ALWAYS' AS \"database-is-generated\", \"is_nullable\" = 'YES' AS \"database-is-nullable\" \
         FROM \"information_schema\".\"columns\" AS \"c\" LEFT JOIN (SELECT \"tc\".\"table_schema\", \"tc\".\"table_name\", \"kc\".\"column_name\" \
         FROM \"information_schema\".\"table_constraints\" AS \"tc\" INNER JOIN \"information_schema\".\"key_column_usage\" AS \"kc\" \
         ON (\"tc\".\"constraint_name\" = \"kc\".\"constraint_name\") AND (\"tc\".\"table_schema\" = \"kc\".\"table_schema\") \
         AND (\"tc\".\"table_name\" = \"kc\".\"table_name\") WHERE \"tc\".\"constraint_type\" = 'PRIMARY KEY') AS \"pk\" \
         ON (\"c\".\"table_schema\" = \"pk\".\"table_schema\") AND (\"c\".\"table_name\" = \"pk\".\"table_name\") \
         AND (\"c\".\"column_name\" = \"pk\".\"column_name\") \
         WHERE c.table_schema !~ '^information_schema|catalog_history|pg_' AND (\"c\".\"table_schema\" IN ('public')) \
         AND \"c\".\"table_name\" = 'Customer' \
         UNION ALL SELECT \"pa\".\"attname\" AS \"name\", FORMAT('%s', \"pt\".\"typname\") AS \"database-type\", \
         \"pa\".\"attnum\" - 1 AS \"database-position\", FALSE AS \"pk?\", NULL AS \"field-comment\", \
         FALSE AS \"database-is-generated\", FALSE AS \"database-is-nullable\" \
         FROM \"pg_catalog\".\"pg_class\" AS \"pc\" INNER JOIN \"pg_catalog\".\"pg_namespace\" AS \"pn\" ON \"pn\".\"oid\" = \"pc\".\"relnamespace\" \
         INNER JOIN \"pg_catalog\".\"pg_attribute\" AS \"pa\" ON \"pa\".\"attrelid\" = \"pc\".\"oid\" \
         INNER JOIN \"pg_catalog\".\"pg_type\" AS \"pt\" ON \"pt\".\"oid\" = \"pa\".\"atttypid\" \
         WHERE \"pc\".\"relkind\" = 'm' ORDER BY \"database-position\"",
    )
    .unwrap();
    let types: BTreeMap<String, Value> = rows
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap().to_owned(),
                row["database-type"].clone(),
            )
        })
        .collect();
    assert_eq!(
        types,
        BTreeMap::from([
            ("__id".to_owned(), json!("text")),
            ("age".to_owned(), json!("int8")),
            ("balance".to_owned(), json!("float8")),
            ("name".to_owned(), json!("text")),
        ])
    );
    assert!(rows.iter().all(|row| row["pk?"] == json!(false)));
    // `data_type` is spelled as PostgreSQL spells it.
    let spelled = execute_sql(
        &lmpg,
        "SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_name = 'Customer' ORDER BY column_name",
    )
    .unwrap();
    assert_eq!(
        column(&spelled, "data_type"),
        vec![
            json!("text"),
            json!("bigint"),
            json!("double precision"),
            json!("text")
        ]
    );
}

#[test]
fn jdbc_reads_primary_and_foreign_keys_of_which_there_are_none() {
    let lmpg = shop();
    // `DatabaseMetaData.getPrimaryKeys()`.
    let keys = execute_sql(
        &lmpg,
        "SELECT result.TABLE_CAT AS \"TABLE_CAT\", result.TABLE_SCHEM AS \"TABLE_SCHEM\", \
         result.TABLE_NAME AS \"TABLE_NAME\", result.COLUMN_NAME AS \"COLUMN_NAME\", \
         result.KEY_SEQ AS \"KEY_SEQ\", result.PK_NAME AS \"PK_NAME\" FROM (SELECT 'rushdb' AS TABLE_CAT, \
         n.nspname AS TABLE_SCHEM, ct.relname AS TABLE_NAME, a.attname AS COLUMN_NAME, \
         (information_schema._pg_expandarray(con.conkey)).n AS KEY_SEQ, con.conname AS PK_NAME, \
         information_schema._pg_expandarray(con.conkey) AS KEYS, a.attnum AS A_ATTNUM \
         FROM pg_catalog.pg_constraint con JOIN pg_catalog.pg_class ct ON (con.conrelid = ct.oid) \
         JOIN pg_catalog.pg_namespace n ON (ct.relnamespace = n.oid) \
         JOIN pg_catalog.pg_attribute a ON (a.attrelid = ct.oid) WHERE con.contype = 'p' \
         AND n.nspname = 'public' AND ct.relname = 'Customer') result \
         where result.A_ATTNUM = (result.KEYS).x ORDER BY result.table_name, result.pk_name, result.key_seq",
    )
    .unwrap();
    assert!(keys.is_empty());
    // Metabase's foreign keys.
    let foreign = execute_sql(
        &lmpg,
        "SELECT \"fk_ns\".\"nspname\" AS \"fk-table-schema\", \"fk_table\".\"relname\" AS \"fk-table-name\" \
         FROM \"pg_constraint\" AS \"c\" INNER JOIN \"pg_class\" AS \"fk_table\" ON \"c\".\"conrelid\" = \"fk_table\".\"oid\" \
         INNER JOIN \"pg_namespace\" AS \"fk_ns\" ON \"c\".\"connamespace\" = \"fk_ns\".\"oid\" \
         INNER JOIN \"pg_attribute\" AS \"fk_column\" ON \"c\".\"conrelid\" = \"fk_column\".\"attrelid\" \
         WHERE (\"c\".\"contype\" = 'f'::char) AND (\"fk_column\".\"attnum\" = ANY(c.conkey))",
    )
    .unwrap();
    assert!(foreign.is_empty());
}

#[test]
fn the_catalogs_show_what_the_policy_shows() {
    let lmpg = shop();
    let policy = Arc::new(ActivePolicy {
        generation: 1,
        policy: DataAccessPolicy {
            allowed_labels: vec!["Customer".into()],
            denied_rel_types: vec!["OWNS".into()],
            visible_properties: BTreeMap::from([("Customer".into(), vec!["name".into()])]),
            max_results: 100,
        },
        mode: AuthorizationMode::PolicyEnforced,
        fingerprint: 0,
        min_count: 0,
    });
    let snapshot = Arc::new(lmpg.store.read_view().unwrap());
    let read = |sql: &str| {
        execute_sql_under_policy_in_view(
            &lmpg,
            Arc::clone(&snapshot),
            sql,
            None,
            None,
            &|| false,
            None,
            Arc::clone(&policy),
        )
        .unwrap()
    };
    let tables = read("SELECT relname FROM pg_catalog.pg_class ORDER BY relname");
    assert_eq!(column(&tables, "relname"), vec![json!("Customer")]);
    let listed = read("SELECT table_name FROM information_schema.tables ORDER BY table_name");
    assert_eq!(column(&listed, "table_name"), vec![json!("Customer")]);
    let columns = read(
        "SELECT a.attname FROM pg_catalog.pg_attribute a \
         JOIN pg_catalog.pg_class c ON a.attrelid = c.oid ORDER BY a.attname",
    );
    assert_eq!(
        column(&columns, "attname"),
        vec![json!("__id"), json!("name")]
    );
}

#[test]
fn format_quotes_as_postgresql_does() {
    let rows = execute_sql(
        &shop(),
        "SELECT format('%s|%I|%I|%L|%L|%%', 'a', 'plain', 'Mixed Case', 'it''s', NULL) AS f",
    )
    .unwrap();
    assert_eq!(rows[0]["f"], json!("a|plain|\"Mixed Case\"|'it''s'|NULL|%"));
}
