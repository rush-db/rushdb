//! Complex SQL aggregation and join qualification.
//!
//! Exercises multi-table joins over labels and edge tables, grouped
//! aggregations, HAVING, DISTINCT counts, window functions, CTEs, subqueries,
//! NULL semantics, arrays, and join fan-out. Expected values are computed by
//! hand from the fixture so a wrong result fails rather than merely differing.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{Lmpg, RecordRef};
use rushdb_sql::{execute_sql, Row};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

struct Fixture {
    graph: Lmpg,
    #[allow(dead_code)]
    persons: Vec<RecordRef>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "linear test-fixture builder; one argument per property keeps the table readable"
)]
fn person(
    graph: &Lmpg,
    name: &str,
    age: i64,
    salary: i64,
    dept: &str,
    active: bool,
    nick: Value,
    tags: Value,
    rating: f64,
) -> RecordRef {
    graph
        .create_record(
            "Person",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!(name))),
                ("age".into(), FieldChange::Set(json!(age))),
                ("salary".into(), FieldChange::Set(json!(salary))),
                ("department".into(), FieldChange::Set(json!(dept))),
                ("active".into(), FieldChange::Set(json!(active))),
                ("nickname".into(), FieldChange::Set(nick)),
                ("tags".into(), FieldChange::Set(tags)),
                ("rating".into(), FieldChange::Set(json!(rating))),
            ],
        )
        .unwrap()
}

#[expect(
    clippy::too_many_lines,
    reason = "single linear fixture builder; splitting it would obscure the graph shape"
)]
fn fixture() -> Fixture {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(7));

    let acme = graph
        .create_record(
            "Company",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!("Acme"))),
                ("industry".into(), FieldChange::Set(json!("tech"))),
            ],
        )
        .unwrap();
    let globex = graph
        .create_record(
            "Company",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!("Globex"))),
                ("industry".into(), FieldChange::Set(json!("tech"))),
            ],
        )
        .unwrap();
    // A company with no employees, for LEFT JOIN count semantics.
    let _initech = graph
        .create_record(
            "Company",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!("Initech"))),
                ("industry".into(), FieldChange::Set(json!("services"))),
            ],
        )
        .unwrap();
    let apollo = graph
        .create_record(
            "Project",
            None,
            vec![("name".into(), FieldChange::Set(json!("Apollo")))],
        )
        .unwrap();
    let gemini = graph
        .create_record(
            "Project",
            None,
            vec![("name".into(), FieldChange::Set(json!("Gemini")))],
        )
        .unwrap();

    let alice = person(
        &graph,
        "Alice",
        30,
        100_000,
        "engineering",
        true,
        json!("Al"),
        json!(["a", "b"]),
        4.5,
    );
    let bob = person(
        &graph,
        "Bob",
        25,
        90_000,
        "engineering",
        false,
        Value::Null,
        json!(["b"]),
        3.0,
    );
    let carol = person(
        &graph,
        "Carol",
        40,
        150_000,
        "research",
        true,
        json!("C"),
        json!(["c"]),
        4.9,
    );
    let dave = person(
        &graph,
        "Dave",
        35,
        120_000,
        "research",
        true,
        Value::Null,
        json!([]),
        3.5,
    );
    let eve = person(
        &graph,
        "Eve",
        28,
        95_000,
        "sales",
        false,
        json!("Ev"),
        json!(["a"]),
        2.5,
    );
    let frank = person(
        &graph,
        "Frank",
        50,
        200_000,
        "sales",
        true,
        Value::Null,
        json!(["d", "e", "f"]),
        5.0,
    );

    for (person_ref, company, since) in [
        (alice, acme, 2015),
        (bob, acme, 2018),
        (carol, globex, 2012),
        (dave, globex, 2019),
        (eve, acme, 2021),
    ] {
        graph
            .create_relationship(
                "WORKS_AT",
                person_ref,
                company,
                vec![("since".into(), FieldChange::Set(json!(since)))],
            )
            .unwrap();
    }
    for (from, to) in [(alice, bob), (bob, carol), (carol, dave), (alice, carol)] {
        graph
            .create_relationship("KNOWS", from, to, vec![])
            .unwrap();
    }
    for (person_ref, project) in [(alice, apollo), (carol, apollo), (dave, gemini)] {
        graph
            .create_relationship("ASSIGNED", person_ref, project, vec![])
            .unwrap();
    }

    Fixture {
        graph,
        persons: vec![alice, bob, carol, dave, eve, frank],
    }
}

fn rows(graph: &Lmpg, sql: &str) -> Vec<Row> {
    execute_sql(graph, sql).unwrap_or_else(|error| panic!("`{sql}` failed: {error}"))
}

fn one(graph: &Lmpg, sql: &str) -> Row {
    let mut result = rows(graph, sql);
    assert_eq!(result.len(), 1, "expected one row for `{sql}`: {result:?}");
    result.pop().unwrap()
}

#[test]
fn group_by_aggregates_are_exact() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT department, COUNT(*) AS c, SUM(salary) AS s, AVG(salary) AS a, MIN(age) AS mn, MAX(age) AS mx \
         FROM person GROUP BY department ORDER BY department",
    );
    assert_eq!(result.len(), 3);
    assert_eq!(result[0]["department"], json!("engineering"));
    assert_eq!(result[0]["c"], json!(2));
    assert_eq!(result[0]["s"], json!(190_000));
    assert_eq!(result[0]["a"], json!(95_000.0));
    assert_eq!(result[0]["mn"], json!(25));
    assert_eq!(result[0]["mx"], json!(30));
    assert_eq!(result[1]["department"], json!("research"));
    assert_eq!(result[1]["s"], json!(270_000));
    assert_eq!(result[2]["department"], json!("sales"));
    assert_eq!(result[2]["s"], json!(295_000));
}

#[test]
fn having_filters_groups() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT department FROM person GROUP BY department HAVING COUNT(*) > 2",
    );
    assert!(result.is_empty(), "no department has >2 people: {result:?}");
    let result = rows(
        &f.graph,
        "SELECT department FROM person GROUP BY department HAVING SUM(salary) > 250000 ORDER BY department",
    );
    let depts: Vec<&str> = result
        .iter()
        .map(|r| r["department"].as_str().unwrap())
        .collect();
    assert_eq!(depts, vec!["research", "sales"]);
}

#[test]
fn distinct_counts_and_nulls() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT COUNT(DISTINCT department) AS d FROM person",
    );
    assert_eq!(result["d"], json!(3));

    let result = one(
        &f.graph,
        "SELECT COUNT(nickname) AS named, COUNT(*) AS total FROM person",
    );
    assert_eq!(result["named"], json!(3), "three non-null nicknames");
    assert_eq!(result["total"], json!(6));

    let result = one(
        &f.graph,
        "SELECT SUM(CASE WHEN nickname IS NULL THEN 1 ELSE 0 END) AS missing FROM person",
    );
    assert_eq!(result["missing"], json!(3));
}

#[test]
fn float_aggregate_over_mixed_numeric_column() {
    let f = fixture();
    let result = one(&f.graph, "SELECT AVG(rating) AS avg_rating FROM person");
    let avg = result["avg_rating"].as_f64().unwrap();
    assert!((avg - 3.9).abs() < 1e-9, "expected 3.9, got {avg}");
}

#[test]
fn left_join_preserves_unmatched_companies() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT c.name AS company, COUNT(w.__id) AS n, COUNT(*) AS rows \
         FROM company c LEFT JOIN rushdb_edges.works_at w ON c.__id = w.__target_id \
         GROUP BY c.name ORDER BY c.name",
    );
    assert_eq!(result.len(), 3);
    assert_eq!(result[0]["company"], json!("Acme"));
    assert_eq!(result[0]["n"], json!(3));
    assert_eq!(result[0]["rows"], json!(3));
    assert_eq!(result[1]["company"], json!("Globex"));
    assert_eq!(result[1]["n"], json!(2));
    // Initech has no employees: COUNT(col) is 0 but COUNT(*) is 1 (the null row).
    assert_eq!(result[2]["company"], json!("Initech"));
    assert_eq!(result[2]["n"], json!(0));
    assert_eq!(result[2]["rows"], json!(1));
}

#[test]
fn multi_join_grouped_aggregate() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT c.name AS company, COUNT(*) AS n, AVG(p.salary) AS avg_salary \
         FROM person p \
         JOIN rushdb_edges.works_at w ON p.__id = w.__source_id \
         JOIN company c ON c.__id = w.__target_id \
         GROUP BY c.name ORDER BY c.name",
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["company"], json!("Acme"));
    assert_eq!(result[0]["n"], json!(3));
    assert_eq!(result[0]["avg_salary"], json!(95_000.0));
    assert_eq!(result[1]["company"], json!("Globex"));
    assert_eq!(result[1]["avg_salary"], json!(135_000.0));
}

#[test]
fn self_join_through_edges() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT a.name AS src, b.name AS dst \
         FROM person a \
         JOIN rushdb_edges.knows e ON a.__id = e.__source_id \
         JOIN person b ON b.__id = e.__target_id \
         ORDER BY a.name, b.name",
    );
    let pairs: Vec<(String, String)> = result
        .iter()
        .map(|r| {
            (
                r["src"].as_str().unwrap().to_string(),
                r["dst"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("Alice".to_string(), "Bob".to_string()),
            ("Alice".to_string(), "Carol".to_string()),
            ("Bob".to_string(), "Carol".to_string()),
            ("Carol".to_string(), "Dave".to_string()),
        ]
    );
}

#[test]
fn two_hop_join_through_edges() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT a.name AS a, c.name AS c \
         FROM person a \
         JOIN rushdb_edges.knows e1 ON a.__id = e1.__source_id \
         JOIN person b ON b.__id = e1.__target_id \
         JOIN rushdb_edges.knows e2 ON b.__id = e2.__source_id \
         JOIN person c ON c.__id = e2.__target_id \
         ORDER BY a.name, c.name",
    );
    let pairs: Vec<(String, String)> = result
        .iter()
        .map(|r| {
            (
                r["a"].as_str().unwrap().to_string(),
                r["c"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("Alice".to_string(), "Carol".to_string()),
            ("Alice".to_string(), "Dave".to_string()),
            ("Bob".to_string(), "Dave".to_string()),
        ]
    );
}

#[test]
fn cte_join_and_aggregate() {
    let f = fixture();
    let result = one(
        &f.graph,
        "WITH eng AS (SELECT * FROM person WHERE department = 'engineering') \
         SELECT COUNT(*) AS n, SUM(salary) AS s FROM eng",
    );
    assert_eq!(result["n"], json!(2));
    assert_eq!(result["s"], json!(190_000));
}

#[test]
fn window_rank_partitioned_by_department() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name, department, \
                RANK() OVER (PARTITION BY department ORDER BY salary DESC) AS r \
         FROM person ORDER BY department, r",
    );
    let ordered: Vec<(String, i64)> = result
        .iter()
        .map(|r| {
            (
                r["name"].as_str().unwrap().to_string(),
                r["r"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        ordered,
        vec![
            ("Alice".to_string(), 1),
            ("Bob".to_string(), 2),
            ("Carol".to_string(), 1),
            ("Dave".to_string(), 2),
            ("Frank".to_string(), 1),
            ("Eve".to_string(), 2),
        ]
    );
}

#[test]
fn subquery_in_and_exists() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name FROM person WHERE department IN \
         (SELECT department FROM person GROUP BY department HAVING AVG(salary) > 130000) \
         ORDER BY name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Carol", "Dave", "Eve", "Frank"]);

    let result = rows(
        &f.graph,
        "SELECT DISTINCT a.name FROM person a WHERE EXISTS \
         (SELECT 1 FROM rushdb_edges.knows e WHERE e.__source_id = a.__id) ORDER BY a.name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Alice", "Bob", "Carol"]);
}

#[test]
fn anti_join_not_exists_finds_isolated_person() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT p.name FROM person p WHERE NOT EXISTS \
         (SELECT 1 FROM rushdb_edges.works_at w WHERE w.__source_id = p.__id) ORDER BY p.name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Frank"]);
}

#[test]
fn left_join_null_extension_finds_unmatched() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT p.name FROM person p \
         LEFT JOIN rushdb_edges.works_at w ON p.__id = w.__source_id \
         WHERE w.__id IS NULL ORDER BY p.name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Frank"]);
}

#[test]
fn array_property_length() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name, array_length(tags, 1) AS n FROM person ORDER BY name",
    );
    let lengths: Vec<(String, i64)> = result
        .iter()
        .map(|r| {
            (
                r["name"].as_str().unwrap().to_string(),
                r["n"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        lengths,
        vec![
            ("Alice".to_string(), 2),
            ("Bob".to_string(), 1),
            ("Carol".to_string(), 1),
            ("Dave".to_string(), 0),
            ("Eve".to_string(), 1),
            ("Frank".to_string(), 3),
        ]
    );
}

#[test]
fn join_fan_out_count_distinct_is_correct() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT COUNT(DISTINCT p.name) AS distinct_sources \
         FROM person p JOIN rushdb_edges.knows e ON p.__id = e.__source_id",
    );
    assert_eq!(result["distinct_sources"], json!(3));
}

#[test]
fn edge_filtered_join_aggregate() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT c.name AS company, COUNT(*) AS n \
         FROM rushdb_edges.works_at w JOIN company c ON c.__id = w.__target_id \
         WHERE w.since >= 2018 GROUP BY c.name ORDER BY c.name",
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["company"], json!("Acme"));
    assert_eq!(result[0]["n"], json!(2));
    assert_eq!(result[1]["company"], json!("Globex"));
    assert_eq!(result[1]["n"], json!(1));
}

#[test]
fn cross_join_cardinality() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT COUNT(*) AS n FROM person CROSS JOIN company",
    );
    assert_eq!(result["n"], json!(18));
}

#[test]
fn aggregate_over_empty_input_is_zero_and_null() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT COUNT(*) AS n, COUNT(salary) AS ns, SUM(salary) AS s, AVG(salary) AS a \
         FROM person WHERE department = 'nonexistent'",
    );
    assert_eq!(result["n"], json!(0));
    assert_eq!(result["ns"], json!(0));
    assert_eq!(result["s"], Value::Null, "SUM over no rows is NULL");
    assert_eq!(result["a"], Value::Null, "AVG over no rows is NULL");
}

#[test]
fn join_fan_out_double_counts_sum() {
    let f = fixture();
    // Alice has two outgoing KNOWS edges, so her salary is summed twice.
    let result = one(
        &f.graph,
        "SELECT SUM(p.salary) AS s FROM person p \
         JOIN rushdb_edges.knows e ON p.__id = e.__source_id",
    );
    assert_eq!(result["s"], json!(100_000 * 2 + 90_000 + 150_000));
}

#[test]
fn window_running_total() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name, \
                SUM(salary) OVER (PARTITION BY department ORDER BY salary) AS running \
         FROM person ORDER BY department, salary",
    );
    let engineering: Vec<i64> = result
        .iter()
        .filter(|r| {
            let n = r["name"].as_str().unwrap();
            n == "Bob" || n == "Alice"
        })
        .map(|r| r["running"].as_i64().unwrap())
        .collect();
    // Ordered by salary: Bob(90000) then Alice(100000).
    assert_eq!(engineering, vec![90_000, 190_000]);
}

#[test]
fn group_by_expression() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT UPPER(department) AS d, COUNT(*) AS n FROM person \
         GROUP BY UPPER(department) ORDER BY d",
    );
    let rows: Vec<(String, i64)> = result
        .iter()
        .map(|r| {
            (
                r["d"].as_str().unwrap().to_string(),
                r["n"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("ENGINEERING".to_string(), 2),
            ("RESEARCH".to_string(), 2),
            ("SALES".to_string(), 2),
        ]
    );
}

#[test]
fn correlated_scalar_subquery_counts_out_degree() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT p.name, \
                (SELECT COUNT(*) FROM rushdb_edges.knows e WHERE e.__source_id = p.__id) AS out_degree \
         FROM person p ORDER BY p.name",
    );
    let degrees: Vec<(String, i64)> = result
        .iter()
        .map(|r| {
            (
                r["name"].as_str().unwrap().to_string(),
                r["out_degree"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        degrees,
        vec![
            ("Alice".to_string(), 2),
            ("Bob".to_string(), 1),
            ("Carol".to_string(), 1),
            ("Dave".to_string(), 0),
            ("Eve".to_string(), 0),
            ("Frank".to_string(), 0),
        ]
    );
}

#[test]
fn count_distinct_through_join() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT COUNT(DISTINCT c.name) AS companies \
         FROM person p JOIN rushdb_edges.works_at w ON p.__id = w.__source_id \
         JOIN company c ON c.__id = w.__target_id",
    );
    assert_eq!(result["companies"], json!(2));
}

#[test]
fn having_on_aggregate_not_in_select() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT department FROM person GROUP BY department HAVING AVG(salary) >= 135000 \
         ORDER BY department",
    );
    let depts: Vec<&str> = result
        .iter()
        .map(|r| r["department"].as_str().unwrap())
        .collect();
    assert_eq!(depts, vec!["research", "sales"]);
}

#[test]
fn order_by_nulls_last_on_nullable_column() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name, nickname FROM person ORDER BY nickname ASC NULLS LAST, name",
    );
    // Non-null nicknames first (Al, C, Ev), then nulls (Bob, Dave, Frank).
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Alice", "Carol", "Eve", "Bob", "Dave", "Frank"]);
}

#[test]
fn three_way_join_with_filter_and_having() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT c.name AS company, COUNT(*) AS n \
         FROM person p \
         JOIN rushdb_edges.works_at w ON p.__id = w.__source_id \
         JOIN company c ON c.__id = w.__target_id \
         WHERE p.active = true \
         GROUP BY c.name HAVING COUNT(*) >= 2 ORDER BY c.name",
    );
    // Active employees: Acme Alice only (1); Globex Carol+Dave (2).
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["company"], json!("Globex"));
    assert_eq!(result[0]["n"], json!(2));
}

#[test]
fn not_in_with_null_matches_sql_semantics() {
    let f = fixture();
    // `NOT IN` over a list containing NULL yields no rows in SQL (UNKNOWN).
    let result = rows(
        &f.graph,
        "SELECT name FROM person WHERE name NOT IN ('Alice', NULL) ORDER BY name",
    );
    assert!(
        result.is_empty(),
        "NOT IN with NULL must be empty, got {result:?}"
    );
}

#[test]
fn order_by_multiple_aggregates() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT department, COUNT(*) AS c FROM person \
         GROUP BY department ORDER BY c DESC, department ASC",
    );
    let depts: Vec<&str> = result
        .iter()
        .map(|r| r["department"].as_str().unwrap())
        .collect();
    // All counts are 2; the tie-break is department ascending.
    assert_eq!(depts, vec!["engineering", "research", "sales"]);
}

#[test]
fn project_assignment_aggregate_by_project() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT pr.name AS project, COUNT(*) AS n \
         FROM rushdb_edges.assigned a JOIN project pr ON pr.__id = a.__target_id \
         GROUP BY pr.name ORDER BY pr.name",
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["project"], json!("Apollo"));
    assert_eq!(result[0]["n"], json!(2));
    assert_eq!(result[1]["project"], json!("Gemini"));
    assert_eq!(result[1]["n"], json!(1));
}

// ---- adversarial / edge cases ----

fn tiny(store: Arc<Store>, tenant: u64) -> Lmpg {
    Lmpg::new(store, TenantId::from_ordinal(tenant))
}

#[test]
fn heterogeneous_int_float_property_aggregates() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 11);
    for value in [json!(1), json!(2.5), json!(3)] {
        graph
            .create_record(
                "Reading",
                None,
                vec![("value".into(), FieldChange::Set(value))],
            )
            .unwrap();
    }
    let result = one(
        &graph,
        "SELECT SUM(value) AS s, AVG(value) AS a FROM reading",
    );
    assert!((result["s"].as_f64().unwrap() - 6.5).abs() < 1e-9);
    assert!((result["a"].as_f64().unwrap() - 6.5 / 3.0).abs() < 1e-9);
}

#[test]
fn sparse_property_columns_are_nullable() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 12);
    graph
        .create_record(
            "Sparse",
            None,
            vec![("a".into(), FieldChange::Set(json!(1)))],
        )
        .unwrap();
    graph
        .create_record(
            "Sparse",
            None,
            vec![("b".into(), FieldChange::Set(json!("x")))],
        )
        .unwrap();
    let result = rows(&graph, "SELECT a, b FROM sparse ORDER BY a NULLS LAST");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["a"], json!(1));
    assert_eq!(result[0]["b"], Value::Null);
    assert_eq!(result[1]["a"], Value::Null);
    assert_eq!(result[1]["b"], json!("x"));
}

#[test]
fn same_property_name_different_types_across_labels() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 13);
    graph
        .create_record(
            "Alpha",
            None,
            vec![("code".into(), FieldChange::Set(json!(1)))],
        )
        .unwrap();
    graph
        .create_record(
            "Beta",
            None,
            vec![("code".into(), FieldChange::Set(json!("one")))],
        )
        .unwrap();
    let alpha = one(&graph, "SELECT code FROM alpha");
    assert_eq!(alpha["code"], json!(1));
    let beta = one(&graph, "SELECT code FROM beta");
    assert_eq!(beta["code"], json!("one"));
}

#[test]
fn unquoted_uppercase_table_name_resolves() {
    let f = fixture();
    let result = one(&f.graph, "SELECT COUNT(*) AS n FROM PERSON");
    assert_eq!(result["n"], json!(6));
}

#[test]
fn pushdown_disjunction_with_comparison() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name FROM person \
         WHERE salary > 100000 AND (department = 'sales' OR department = 'research') \
         ORDER BY name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Carol", "Dave", "Frank"]);
}

#[test]
fn pushdown_equality_excludes_nulls() {
    let f = fixture();
    let result = rows(&f.graph, "SELECT name FROM person WHERE nickname = 'Al'");
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Alice"]);
}

#[test]
fn pushdown_float_range() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name FROM person WHERE rating > 3.4 ORDER BY name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Alice", "Carol", "Dave", "Frank"]);
}

#[test]
fn pushdown_in_list() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name FROM person WHERE department IN ('sales', 'research') ORDER BY name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Carol", "Dave", "Eve", "Frank"]);
}

#[test]
fn pushdown_boolean() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name FROM person WHERE active = true ORDER BY name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Alice", "Carol", "Dave", "Frank"]);
}

#[test]
fn pushdown_string_range() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name FROM person WHERE name >= 'C' ORDER BY name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Carol", "Dave", "Eve", "Frank"]);
}

#[test]
fn large_int64_is_exact_not_rounded() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 14);
    let big = 9_000_000_000_000_000_001_i64;
    graph
        .create_record(
            "Big",
            None,
            vec![("v".into(), FieldChange::Set(json!(big)))],
        )
        .unwrap();
    let result = one(&graph, "SELECT v FROM big");
    assert_eq!(result["v"], json!(big));
    let result = one(&graph, "SELECT SUM(v) AS s FROM big");
    assert_eq!(result["s"], json!(big));
}

#[test]
fn join_with_no_matches_is_empty() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT COUNT(*) AS n FROM person p JOIN company c ON p.__id = c.__id",
    );
    assert_eq!(result["n"], json!(0));
}

#[test]
fn group_by_nullable_column_groups_nulls_together() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT nickname, COUNT(*) AS n FROM person GROUP BY nickname ORDER BY nickname NULLS LAST",
    );
    // Al, C, Ev, then the null group of 3.
    assert_eq!(result.len(), 4);
    let last = &result[3];
    assert_eq!(last["nickname"], Value::Null);
    assert_eq!(last["n"], json!(3));
}

#[test]
fn truly_mixed_scalar_types_in_one_property() {
    // The substrate allows heterogeneous types; SQL must surface the column
    // without failing the whole label. If arrow cannot unify int + string, the
    // engine should coerce to a string column (schema-on-write parity).
    let graph = tiny(Arc::new(Store::new_in_memory()), 15);
    graph
        .create_record(
            "Mixed",
            None,
            vec![("score".into(), FieldChange::Set(json!(1)))],
        )
        .unwrap();
    graph
        .create_record(
            "Mixed",
            None,
            vec![("score".into(), FieldChange::Set(json!("one")))],
        )
        .unwrap();
    let result = rows(&graph, "SELECT score FROM mixed ORDER BY score");
    assert_eq!(result.len(), 2);
}

#[test]
fn aggregate_after_sql_update_and_delete() {
    let f = fixture();
    // Raise every sales salary by 1000.
    rushdb_sql::execute_sql_mutation(
        &f.graph,
        "UPDATE person SET salary = salary + 1000 WHERE department = 'sales'",
        None,
    )
    .unwrap();
    let result = one(
        &f.graph,
        "SELECT SUM(salary) AS s FROM person WHERE department = 'sales'",
    );
    assert_eq!(result["s"], json!(295_000 + 2_000));

    rushdb_sql::execute_sql_mutation(
        &f.graph,
        "DELETE FROM person WHERE department = 'research'",
        None,
    )
    .unwrap();
    let result = one(
        &f.graph,
        "SELECT COUNT(*) AS n, SUM(salary) AS s FROM person",
    );
    assert_eq!(result["n"], json!(4));
    assert_eq!(result["s"], json!(100_000 + 90_000 + 96_000 + 201_000));
}

#[test]
fn join_on_string_key() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 16);
    graph
        .create_record(
            "Emp",
            None,
            vec![("dept".into(), FieldChange::Set(json!("eng")))],
        )
        .unwrap();
    graph
        .create_record(
            "Emp",
            None,
            vec![("dept".into(), FieldChange::Set(json!("ops")))],
        )
        .unwrap();
    graph
        .create_record(
            "Dept",
            None,
            vec![("name".into(), FieldChange::Set(json!("eng")))],
        )
        .unwrap();
    let result = one(
        &graph,
        "SELECT COUNT(*) AS n FROM emp e JOIN dept d ON e.dept = d.name",
    );
    assert_eq!(result["n"], json!(1));
}

#[test]
fn two_edge_tables_joined_and_grouped() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT p.name, COUNT(DISTINCT w.__target_id) AS companies, COUNT(DISTINCT k.__target_id) AS known \
         FROM person p \
         JOIN rushdb_edges.works_at w ON p.__id = w.__source_id \
         JOIN rushdb_edges.knows k ON p.__id = k.__source_id \
         GROUP BY p.name ORDER BY p.name",
    );
    // Alice works at Acme and knows Bob+Carol; Carol works at Globex and knows Dave.
    let alice = result.iter().find(|r| r["name"] == json!("Alice")).unwrap();
    assert_eq!(alice["companies"], json!(1));
    assert_eq!(alice["known"], json!(2));
    let carol = result.iter().find(|r| r["name"] == json!("Carol")).unwrap();
    assert_eq!(carol["known"], json!(1));
}

#[test]
fn string_agg_concatenates_group_members() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT string_agg(name, ',' ORDER BY name) AS names \
         FROM person WHERE department = 'engineering'",
    );
    let names = result["names"].as_str().unwrap();
    assert_eq!(names, "Alice,Bob");
}

#[test]
fn sum_distinct_over_fan_out() {
    let f = fixture();
    // Alice appears twice via KNOWS; SUM(DISTINCT salary) counts her once.
    let result = one(
        &f.graph,
        "SELECT SUM(DISTINCT p.salary) AS s FROM person p \
         JOIN rushdb_edges.knows e ON p.__id = e.__source_id",
    );
    assert_eq!(result["s"], json!(100_000 + 90_000 + 150_000));
}

#[test]
fn aggregate_filter_clause() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT COUNT(*) FILTER (WHERE active) AS active_n, COUNT(*) AS n FROM person",
    );
    assert_eq!(result["active_n"], json!(4));
    assert_eq!(result["n"], json!(6));
}

#[test]
fn union_all_of_group_aggregates() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT department, COUNT(*) AS n FROM person WHERE department = 'sales' GROUP BY department \
         UNION ALL \
         SELECT department, COUNT(*) AS n FROM person WHERE department = 'research' GROUP BY department \
         ORDER BY department",
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["department"], json!("research"));
    assert_eq!(result[1]["department"], json!("sales"));
}

#[test]
fn nonexistent_label_errors_not_empty() {
    let f = fixture();
    let error = execute_sql(&f.graph, "SELECT * FROM does_not_exist");
    assert!(error.is_err(), "unknown table must error: {error:?}");
}

#[test]
fn cancelled_sql_returns_a_cancelled_error() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 23);
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    // The predicate reports cancellation before any batch is consumed.
    let result = rushdb_sql::execute_sql_with_parameters_budget_in_view_cancellable(
        &graph,
        snapshot,
        "SELECT 1",
        None,
        None,
        &|| true,
        None,
    );
    assert!(
        matches!(result, Err(rushdb_sql::SqlError::Cancelled)),
        "expected SqlError::Cancelled, got {result:?}"
    );
}

#[test]
fn sum_overflow_is_reported_not_wrapped() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 17);
    let half = i64::MAX / 2 + 1;
    for _ in 0..2 {
        graph
            .create_record(
                "Over",
                None,
                vec![("v".into(), FieldChange::Set(json!(half)))],
            )
            .unwrap();
    }
    // Two halves exceed i64::MAX. `checked_sum` must report the overflow
    // instead of returning the wrapped i64::MIN; a silent Ok is a regression.
    let result = execute_sql(&graph, "SELECT SUM(v) AS s FROM over");
    assert!(
        result.is_err(),
        "SUM(Int64) must error on overflow, got {result:?}"
    );

    // A non-overflowing integer SUM still returns an exact i64 (the rewrite
    // does not change the result type).
    let graph = tiny(Arc::new(Store::new_in_memory()), 17);
    for value in 1..=4i64 {
        graph
            .create_record(
                "Ok",
                None,
                vec![("v".into(), FieldChange::Set(json!(value)))],
            )
            .unwrap();
    }
    let result = execute_sql(&graph, "SELECT SUM(v) AS s FROM ok").unwrap();
    assert_eq!(result[0]["s"].as_i64(), Some(10));
}

#[test]
fn order_by_aggregate_alias() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT department, SUM(salary) AS total FROM person \
         GROUP BY department ORDER BY total DESC",
    );
    let depts: Vec<&str> = result
        .iter()
        .map(|r| r["department"].as_str().unwrap())
        .collect();
    assert_eq!(depts, vec!["sales", "research", "engineering"]);
}

#[test]
fn join_with_or_in_on_clause() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 18);
    let a = graph
        .create_record("N", None, vec![("n".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    let _ = a;
    let b = graph
        .create_record("N", None, vec![("n".into(), FieldChange::Set(json!(2)))])
        .unwrap();
    let _ = b;
    let m = graph
        .create_record("M", None, vec![("k".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    let _ = m;
    let result = one(
        &graph,
        "SELECT COUNT(*) AS n FROM n x JOIN m y ON x.n = y.k OR x.n = 2",
    );
    // y.k = 1 matches x.n = 1; the OR x.n = 2 matches every x.n=2 row for each y.
    assert!(result["n"].as_i64().unwrap() >= 1);
}

#[test]
fn not_exists_correlated_on_property() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT p.name FROM person p WHERE NOT EXISTS \
         (SELECT 1 FROM person q WHERE q.department = p.department AND q.salary > p.salary) \
         ORDER BY p.name",
    );
    let names: Vec<&str> = result.iter().map(|r| r["name"].as_str().unwrap()).collect();
    // Highest earner per department: Alice (eng), Carol (research), Frank (sales).
    assert_eq!(names, vec!["Alice", "Carol", "Frank"]);
}

#[test]
fn count_distinct_multiple_columns_is_a_clear_unsupported_error() {
    // DataFusion does not implement multi-argument COUNT(DISTINCT). The
    // contract is a clear error, never a silently wrong count.
    let f = fixture();
    let error = execute_sql(
        &f.graph,
        "SELECT COUNT(DISTINCT department, active) AS pairs FROM person",
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("not implemented"),
        "expected an explicit unsupported error, got {error}"
    );
}

#[test]
fn left_join_where_on_right_behaves_as_inner() {
    let f = fixture();
    let result = one(
        &f.graph,
        "SELECT COUNT(*) AS n FROM company c \
         LEFT JOIN rushdb_edges.works_at w ON c.__id = w.__target_id \
         WHERE w.since > 2010",
    );
    assert_eq!(result["n"], json!(5));
}

#[test]
fn coalesce_fills_nulls() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT name, COALESCE(nickname, 'anon') AS display FROM person ORDER BY name",
    );
    let displays: Vec<(String, String)> = result
        .iter()
        .map(|r| {
            (
                r["name"].as_str().unwrap().to_string(),
                r["display"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        displays,
        vec![
            ("Alice".to_string(), "Al".to_string()),
            ("Bob".to_string(), "anon".to_string()),
            ("Carol".to_string(), "C".to_string()),
            ("Dave".to_string(), "anon".to_string()),
            ("Eve".to_string(), "Ev".to_string()),
            ("Frank".to_string(), "anon".to_string()),
        ]
    );
}

#[test]
fn case_expression_grouping() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT CASE WHEN salary >= 150000 THEN 'high' ELSE 'low' END AS band, COUNT(*) AS n \
         FROM person GROUP BY 1 ORDER BY 1",
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["band"], json!("high"));
    assert_eq!(result[0]["n"], json!(2));
    assert_eq!(result[1]["band"], json!("low"));
    assert_eq!(result[1]["n"], json!(4));
}

#[test]
fn order_by_aggregate_with_limit() {
    let f = fixture();
    let result = rows(
        &f.graph,
        "SELECT department, SUM(salary) AS total FROM person \
         GROUP BY department ORDER BY total DESC LIMIT 2",
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["department"], json!("sales"));
    assert_eq!(result[1]["department"], json!("research"));
}

#[test]
#[expect(
    clippy::cast_precision_loss,
    reason = "the test compares AVG against an f64 approximation of a large i64"
)]
fn avg_does_not_overflow_on_large_ints() {
    let graph = tiny(Arc::new(Store::new_in_memory()), 19);
    for _ in 0..2 {
        graph
            .create_record(
                "Over2",
                None,
                vec![("v".into(), FieldChange::Set(json!(i64::MAX / 2 + 1)))],
            )
            .unwrap();
    }
    let result = one(&graph, "SELECT AVG(v) AS a FROM over2");
    let avg = result["a"].as_f64().unwrap();
    assert!(
        (avg - (i64::MAX / 2 + 1) as f64).abs() < 1.0,
        "AVG must not wrap: {avg}"
    );
}
