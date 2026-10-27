//! Integration tests for the `rushdb-sql` read and mutation entry points.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_sql::{
    describe_sql, execute_sql, execute_sql_mutation, execute_sql_mutation_in_view_with_budget,
    execute_sql_with_parameters, execute_sql_with_parameters_budget, SqlError, SqlType,
};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

fn fixture(store: Arc<Store>, tenant: u64, suffix: &str) -> Lmpg {
    let graph = Lmpg::new(store, TenantId::from_ordinal(tenant));
    let alice = graph
        .create_record(
            "Person",
            None,
            vec![
                (
                    "name".into(),
                    FieldChange::Set(json!(format!("Alice{suffix}"))),
                ),
                ("department".into(), FieldChange::Set(json!("engineering"))),
                ("salary".into(), FieldChange::Set(json!(150_000))),
            ],
        )
        .unwrap();
    let bob = graph
        .create_record(
            "Person",
            None,
            vec![
                (
                    "name".into(),
                    FieldChange::Set(json!(format!("Bob{suffix}"))),
                ),
                ("department".into(), FieldChange::Set(json!("engineering"))),
                ("salary".into(), FieldChange::Set(json!(120_000))),
            ],
        )
        .unwrap();
    let carol = graph
        .create_record(
            "Person",
            None,
            vec![
                (
                    "name".into(),
                    FieldChange::Set(json!(format!("Carol{suffix}"))),
                ),
                ("department".into(), FieldChange::Set(json!("research"))),
                ("salary".into(), FieldChange::Set(json!(170_000))),
            ],
        )
        .unwrap();
    graph
        .create_relationship(
            "KNOWS",
            alice,
            bob,
            vec![("since".into(), FieldChange::Set(json!(2020)))],
        )
        .unwrap();
    graph
        .create_relationship(
            "KNOWS",
            bob,
            carol,
            vec![("since".into(), FieldChange::Set(json!(2022)))],
        )
        .unwrap();
    graph
}

#[test]
fn mature_sql_features_are_executed_by_datafusion() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let rows = execute_sql(
        &graph,
        "WITH ranked AS (
           SELECT name, department, salary,
                  ROW_NUMBER() OVER (PARTITION BY department ORDER BY salary DESC) AS rank
           FROM person
         )
         SELECT name, salary, rank
         FROM ranked
         WHERE rank = 1
         ORDER BY salary DESC",
    )
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "Carol");
    assert_eq!(rows[0]["rank"], 1);
    assert_eq!(rows[1]["name"], "Alice");
}

#[test]
fn pushed_filters_are_inexact_and_sql_returns_only_matching_rows() {
    // The provider's in-scan translation is all-or-nothing, so a pushed
    // conjunct must be advertised `Inexact`: DataFusion has to re-apply the
    // predicate even when the provider filtered. If it were `Exact`,
    // `eval_pushdown`'s `unwrap_or(true)` fallback for an unmapped type would
    // let non-matching rows through. This exercises the disjunction shape
    // (`translate_filters` returns `None`, so the scan applies nothing) and
    // asserts the correct rows survive.
    let store = Arc::new(Store::new_in_memory());
    let graph = Lmpg::new(store, TenantId::from_ordinal(9));
    graph
        .create_record(
            "Metric",
            None,
            vec![("value".into(), FieldChange::Set(json!(150_000)))],
        )
        .unwrap();
    graph
        .create_record(
            "Metric",
            None,
            vec![("value".into(), FieldChange::Set(json!(1)))],
        )
        .unwrap();

    let rows = execute_sql(&graph, "SELECT value FROM metric WHERE value = 150000").unwrap();
    assert_eq!(rows.len(), 1, "only the matching row: {rows:?}");

    let rows = execute_sql(
        &graph,
        "SELECT value FROM metric WHERE value = 150000 AND (value = 150000 OR value = 1)",
    )
    .unwrap();
    assert_eq!(rows.len(), 1, "disjunction must not leak rows: {rows:?}");
}

#[test]
fn relationship_types_are_sql_schema_tables() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let rows = execute_sql(
        &graph,
        "SELECT p.name, e.since
         FROM person p
         JOIN rushdb_edges.knows e ON p.__id = e.__source_id
         WHERE e.since >= 2020
         ORDER BY e.since",
    )
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "Alice");
    assert_eq!(rows[1]["name"], "Bob");
}

#[test]
fn sql_schema_includes_fields_introduced_by_later_members() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 17, "");
    graph
        .create_record(
            "Person",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!("Dana"))),
                ("age".into(), FieldChange::Set(json!(42))),
            ],
        )
        .unwrap();

    let rows = execute_sql(&graph, "SELECT name, age FROM person ORDER BY name").unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0]["age"], serde_json::Value::Null);
    assert_eq!(rows[3]["age"], json!(42));
}

#[test]
fn explain_reports_scan_filter_and_projection_without_materializing_rows() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 18, "");
    let rows = execute_sql(
        &graph,
        "EXPLAIN SELECT name FROM person WHERE salary >= 150000",
    )
    .unwrap();
    let plan = serde_json::to_string(&rows).unwrap();
    assert!(
        plan.contains("person"),
        "EXPLAIN must identify the scanned table"
    );
    assert!(
        plan.contains("salary"),
        "EXPLAIN must retain the pushed predicate"
    );
    assert!(
        plan.contains("name"),
        "EXPLAIN must retain the selected projection"
    );
}

#[test]
fn sql_catalog_and_execution_are_tenant_scoped() {
    let store = Arc::new(Store::new_in_memory());
    let first = fixture(Arc::clone(&store), 10, "-one");
    let second = fixture(store, 20, "-two");

    let first_rows = execute_sql(&first, "SELECT name FROM person ORDER BY name").unwrap();
    let first_json = serde_json::to_string(&first_rows).unwrap();
    assert!(first_json.contains("-one"));
    assert!(!first_json.contains("-two"));

    let second_rows = execute_sql(&second, "SELECT name FROM person ORDER BY name").unwrap();
    let second_json = serde_json::to_string(&second_rows).unwrap();
    assert!(second_json.contains("-two"));
    assert!(!second_json.contains("-one"));
}

#[test]
fn sql_mutations_fail_closed() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    for statement in [
        "INSERT INTO person (name) VALUES ('Eve')",
        "UPDATE person SET salary = 0",
        "DELETE FROM person",
        "CREATE TABLE leaked (id BIGINT)",
    ] {
        assert!(matches!(
            execute_sql(&graph, statement),
            Err(SqlError::ReadOnly(_))
        ));
    }
    assert_eq!(
        execute_sql(&graph, "SELECT COUNT(*) AS count FROM person").unwrap()[0]["count"],
        3
    );
}

#[test]
fn datafusion_dml_is_published_as_atomic_lmpg_changes() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");

    let updated = execute_sql_mutation(
        &graph,
        "UPDATE person SET salary = salary + $increment WHERE name = $name",
        Some(&json!({"increment": 5_000, "name": "Alice"})),
    )
    .unwrap();
    assert_eq!(updated.affected, 1);
    assert!(updated.commit_index > 0);

    let inserted = execute_sql_mutation(
        &graph,
        "INSERT INTO person (name, department, salary) VALUES ('Dora', 'research', 90000)",
        None,
    )
    .unwrap();
    assert_eq!(inserted.affected, 1);

    let deleted =
        execute_sql_mutation(&graph, "DELETE FROM person WHERE salary < 100000", None).unwrap();
    assert_eq!(deleted.affected, 1);

    let rows = execute_sql(&graph, "SELECT name, salary FROM person ORDER BY name").unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["name"], "Alice");
    assert_eq!(rows[0]["salary"], 155_000);
    assert!(!rows.iter().any(|row| row["name"] == "Dora"));
}

#[test]
fn sql_dml_preserves_tenant_and_record_identity_boundaries() {
    let store = Arc::new(Store::new_in_memory());
    let first = fixture(Arc::clone(&store), 10, "-one");
    let second = fixture(store, 20, "-two");

    execute_sql_mutation(
        &first,
        "UPDATE person SET department = 'platform' WHERE name = 'Alice-one'",
        None,
    )
    .unwrap();
    assert_eq!(
        execute_sql(
            &second,
            "SELECT COUNT(*) AS count FROM person WHERE department = 'platform'",
        )
        .unwrap()[0]["count"],
        0
    );

    let identity_change = execute_sql_mutation(
        &first,
        "UPDATE person SET __id = '00000000-0000-7000-8000-000000000001' WHERE name = 'Bob-one'",
        None,
    );
    assert!(matches!(identity_change, Err(SqlError::InvalidIdentity(_))));
    assert_eq!(
        execute_sql(&first, "SELECT COUNT(*) AS count FROM person").unwrap()[0]["count"],
        3
    );
}

#[test]
fn sql_mutation_rejects_non_dml_and_relationship_table_writes() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    for statement in [
        "SELECT * FROM person",
        "CREATE TABLE leaked (id BIGINT)",
        "DELETE FROM rushdb_edges.knows",
    ] {
        assert!(matches!(
            execute_sql_mutation(&graph, statement, None),
            Err(SqlError::UnsupportedMutation(_))
        ));
    }
    assert_eq!(
        execute_sql(&graph, "SELECT COUNT(*) AS count FROM person").unwrap()[0]["count"],
        3
    );
}

#[test]
fn positional_and_named_parameters_are_bound_by_datafusion() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let positional = execute_sql_with_parameters(
        &graph,
        "SELECT name FROM person WHERE salary > $1 ORDER BY name",
        Some(&json!([130_000])),
    )
    .unwrap();
    assert_eq!(positional.len(), 2);

    let named = execute_sql_with_parameters(
        &graph,
        "SELECT name FROM person WHERE department = $department ORDER BY name",
        Some(&json!({"department": "research"})),
    )
    .unwrap();
    assert_eq!(
        named,
        vec![serde_json::from_value(json!({"name": "Carol"})).unwrap()]
    );
}

#[test]
fn sql_result_budget_is_rejected_before_rows_are_retained() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let error = execute_sql_with_parameters_budget(
        &graph,
        "SELECT name, department FROM person ORDER BY name",
        None,
        Some(1),
    )
    .expect_err("tiny SQL result budget");
    assert!(matches!(error, SqlError::ResourceLimit(_)));
}

#[test]
fn sql_mutation_result_budget_is_rejected_before_commit() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    let error = execute_sql_mutation_in_view_with_budget(
        &graph,
        snapshot,
        "UPDATE person SET department = 'research'",
        None,
        Some(1),
        None,
    )
    .expect_err("tiny SQL mutation result budget");
    assert!(matches!(error, SqlError::ResourceLimit(_)));
}

#[test]
fn prepared_statement_metadata_comes_from_the_datafusion_plan() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let description = describe_sql(
        &graph,
        "SELECT name, salary FROM person WHERE salary > $1 AND department = $department",
    )
    .unwrap();
    assert_eq!(
        description.parameters,
        vec![
            ("$1".into(), SqlType::Int64),
            ("$department".into(), SqlType::Utf8),
        ]
    );
    assert_eq!(description.columns[0].name, "name");
    assert_eq!(description.columns[0].data_type, SqlType::Utf8);
    assert_eq!(description.columns[1].data_type, SqlType::Int64);
}
