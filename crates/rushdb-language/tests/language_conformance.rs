//! End-to-end conformance tests for the GQL, Cypher, Gremlin, and SQL/PGQ
//! frontends, exercised against a real LMPG tenant.

use std::sync::Arc;

use rushdb_language::{execute, execute_mutation, QueryLanguage};
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
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
                ("age".into(), FieldChange::Set(json!(34))),
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
                ("age".into(), FieldChange::Set(json!(29))),
            ],
        )
        .unwrap();
    graph
        .create_record(
            "Person",
            None,
            vec![
                (
                    "name".into(),
                    FieldChange::Set(json!(format!("Carol{suffix}"))),
                ),
                ("age".into(), FieldChange::Set(json!(22))),
            ],
        )
        .unwrap();
    graph
        .create_relationship(
            "KNOWS",
            alice,
            bob,
            vec![("since".into(), FieldChange::Set(json!(2024)))],
        )
        .unwrap();
    graph
}

#[test]
fn cancelled_language_query_returns_a_cancelled_error() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 77, "cancel");
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    let result = rushdb_language::execute_in_view_with_budget_cancellable(
        &snapshot,
        TenantId::from_ordinal(77),
        QueryLanguage::Cypher,
        "MATCH (p:Person) RETURN p.name AS name",
        None,
        None,
        &|| true,
        None,
    );
    assert!(
        matches!(result, Err(rushdb_language::LanguageError::Cancelled)),
        "expected LanguageError::Cancelled, got {result:?}"
    );
}

#[test]
fn gql_cypher_gremlin_and_sql_pgq_share_one_lmpg_graph() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");

    let cypher = execute(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (p:Person)-[:KNOWS]->(f:Person) RETURN p.name AS source, f.name AS target",
        None,
    )
    .unwrap();
    assert_eq!(
        cypher.rows,
        vec![json!({"source": "Alice", "target": "Bob"})]
    );

    let gql = execute(
        &graph,
        QueryLanguage::Gql,
        "MATCH (p:Person)-[:KNOWS]->(f:Person) RETURN p.name AS source, f.name AS target",
        None,
    )
    .unwrap();
    assert_eq!(gql.rows, cypher.rows);

    let gremlin = execute(
        &graph,
        QueryLanguage::Gremlin,
        "g.V().hasLabel('Person').has('age', gt(30)).values('name')",
        None,
    )
    .unwrap();
    assert!(
        gremlin.rows.iter().any(|row| row
            .as_object()
            .unwrap()
            .values()
            .any(|value| value == "Alice")),
        "unexpected Gremlin rows: {:?}",
        gremlin.rows
    );

    let sql = execute(
        &graph,
        QueryLanguage::SqlPgq,
        "SELECT * FROM GRAPH_TABLE (MATCH (p:Person)-[:KNOWS]->(f:Person) COLUMNS (p.name AS source, f.name AS target)) AS g",
        None,
    )
    .unwrap();
    assert_eq!(sql.rows, cypher.rows);
}

#[test]
fn every_language_is_tenant_scoped() {
    let store = Arc::new(Store::new_in_memory());
    let first = fixture(Arc::clone(&store), 10, "-one");
    let second = fixture(store, 20, "-two");

    let cases = [
        (
            QueryLanguage::Cypher,
            "MATCH (n:Person) RETURN n.name AS name ORDER BY name",
        ),
        (
            QueryLanguage::Gql,
            "MATCH (n:Person) RETURN n.name AS name ORDER BY name",
        ),
        (
            QueryLanguage::Gremlin,
            "g.V().hasLabel('Person').values('name').order()",
        ),
        (
            QueryLanguage::SqlPgq,
            "SELECT * FROM GRAPH_TABLE (MATCH (n:Person) COLUMNS (n.name AS name)) AS g ORDER BY name",
        ),
    ];

    for (language, statement) in cases {
        let first_rows = execute(&first, language, statement, None).unwrap().rows;
        let first_json = serde_json::to_string(&first_rows).unwrap();
        assert!(first_json.contains("-one"), "{language:?}: {first_json}");
        assert!(!first_json.contains("-two"), "{language:?}: {first_json}");

        let second_rows = execute(&second, language, statement, None).unwrap().rows;
        let second_json = serde_json::to_string(&second_rows).unwrap();
        assert!(second_json.contains("-two"), "{language:?}: {second_json}");
        assert!(!second_json.contains("-one"), "{language:?}: {second_json}");
    }
}

#[test]
fn write_statements_fail_closed_on_the_read_authorized_query_path() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    for (language, statement) in [
        (QueryLanguage::Gql, "INSERT (:Person {name: 'Eve'})"),
        (QueryLanguage::Cypher, "CREATE (:Person {name: 'Eve'})"),
        (
            QueryLanguage::Gremlin,
            "g.addV('Person').property('name', 'Eve')",
        ),
    ] {
        let error = execute(&graph, language, statement, None).unwrap_err();
        let message = error.to_string().to_lowercase();
        assert!(
            message.contains("read-only")
                || message.contains("write")
                || message.contains("mutable"),
            "{language:?} did not fail as read-only: {error}"
        );
    }

    let after = execute(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (n:Person) RETURN count(n) AS count",
        None,
    )
    .unwrap();
    assert_eq!(after.rows, vec![json!({"count": 3})]);
}

#[test]
fn gql_and_cypher_cover_parameters_nulls_aggregation_unwind_and_slicing() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");

    for language in [QueryLanguage::Gql, QueryLanguage::Cypher] {
        let filtered = execute(
            &graph,
            language,
            "MATCH (p:Person) WHERE p.age >= $minAge RETURN p.name AS name ORDER BY name",
            Some(&json!({"minAge": 30})),
        )
        .unwrap();
        assert_eq!(filtered.rows, vec![json!({"name": "Alice"})]);

        let optional = execute(
            &graph,
            language,
            "MATCH (p:Person) OPTIONAL MATCH (p)-[:KNOWS]->(f:Person) RETURN p.name AS source, f.name AS target ORDER BY source",
            None,
        )
        .unwrap();
        assert_eq!(
            optional.rows,
            vec![
                json!({"source": "Alice", "target": "Bob"}),
                json!({"source": "Bob", "target": null}),
                json!({"source": "Carol", "target": null}),
            ],
            "{language:?} OPTIONAL MATCH/null semantics"
        );

        let aggregate = execute(
            &graph,
            language,
            "MATCH (p:Person) RETURN count(p) AS total",
            None,
        )
        .unwrap();
        assert_eq!(aggregate.rows, vec![json!({"total": 3})]);

        let sliced = execute(
            &graph,
            language,
            "MATCH (p:Person) RETURN p.name AS name ORDER BY name SKIP 1 LIMIT 1",
            None,
        )
        .unwrap();
        assert_eq!(sliced.rows, vec![json!({"name": "Bob"})]);

        let unwound = execute(
            &graph,
            language,
            "UNWIND [3, 1, 2] AS value RETURN value ORDER BY value",
            None,
        )
        .unwrap();
        assert_eq!(
            unwound.rows,
            vec![
                json!({"value": 1}),
                json!({"value": 2}),
                json!({"value": 3})
            ]
        );
    }
}

#[test]
fn parameters_reject_unsigned_integers_outside_graph_integer_domain() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let error = execute(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (p:Person) WHERE p.age >= $minimum RETURN p.name",
        Some(&json!({"minimum": u64::MAX})),
    )
    .expect_err("lossy unsigned parameter must fail closed");
    assert!(matches!(
        error,
        rushdb_language::LanguageError::InvalidParameters
    ));
}

#[test]
fn gremlin_named_parameters_are_bound_by_the_community_engine() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let result = execute(
        &graph,
        QueryLanguage::Gremlin,
        "g.V().hasLabel('Person').has('name', $name).values('age')",
        Some(&json!({"name": "Alice"})),
    )
    .unwrap();
    assert!(
        serde_json::to_string(&result.rows).unwrap().contains("34"),
        "unexpected parameterized Gremlin result: {result:?}"
    );
}

#[test]
fn gremlin_read_profile_covers_filter_traversal_order_range_and_count() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");

    let traversed = execute(
        &graph,
        QueryLanguage::Gremlin,
        "g.V().hasLabel('Person').has('name', 'Alice').out('KNOWS').values('name')",
        None,
    )
    .unwrap();
    let encoded = serde_json::to_string(&traversed.rows).unwrap();
    assert!(encoded.contains("Bob"), "{encoded}");

    let sliced = execute(
        &graph,
        QueryLanguage::Gremlin,
        "g.V().hasLabel('Person').values('name').order().range(1, 2)",
        None,
    )
    .unwrap();
    assert_eq!(sliced.rows.len(), 1);
    assert!(serde_json::to_string(&sliced.rows).unwrap().contains("Bob"));

    let counted = execute(
        &graph,
        QueryLanguage::Gremlin,
        "g.V().hasLabel('Person').count()",
        None,
    )
    .unwrap();
    assert!(serde_json::to_string(&counted.rows).unwrap().contains('3'));
}

#[test]
fn sql_pgq_read_profile_covers_projection_filter_and_order() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let rows = execute(
        &graph,
        QueryLanguage::SqlPgq,
        "SELECT name FROM GRAPH_TABLE (MATCH (p:Person) COLUMNS (p.name AS name, p.age AS age)) AS g WHERE age >= 29 ORDER BY name DESC",
        None,
    )
    .unwrap()
    .rows;
    assert_eq!(rows, vec![json!({"name": "Bob"}), json!({"name": "Alice"})]);
}

#[test]
fn graph_language_creates_commit_to_lmpg_atomically() {
    for (language, statement) in [
        (
            QueryLanguage::Cypher,
            "CREATE (n:Person {name: 'Eve', age: 40}) RETURN n.name AS name",
        ),
        (
            QueryLanguage::Gql,
            "INSERT (:Person {name: 'Eve', age: 40})",
        ),
        (
            QueryLanguage::Gremlin,
            "g.addV('Person').property('name', 'Eve').property('age', 40)",
        ),
    ] {
        let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
        let before = graph.store.committed_version();
        let mutation = execute_mutation(&graph, language, statement, None).unwrap();
        assert!(mutation.commit_index > before, "{language:?}");
        assert!(mutation.affected > 0, "{language:?}: {mutation:?}");

        let after = execute(
            &graph,
            QueryLanguage::Cypher,
            "MATCH (n:Person) WHERE n.name = 'Eve' RETURN n.age AS age",
            None,
        )
        .unwrap();
        assert_eq!(after.rows, vec![json!({"age": 40})], "{language:?}");
    }
}

#[test]
fn cypher_updates_relationship_creation_and_detach_delete_are_atomic() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");

    let updated = execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (n:Person {name: 'Alice'}) SET n.age = 35, n.city = 'London' RETURN n.age AS age",
        None,
    )
    .unwrap();
    assert_eq!(updated.result.rows, vec![json!({"age": 35})]);

    let related = execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (a:Person {name: 'Alice'}), (c:Person {name: 'Carol'}) CREATE (a)-[r:LIKES {weight: 2}]->(c) RETURN r",
        None,
    )
    .unwrap();
    assert_eq!(related.result.rows.len(), 1);
    assert_eq!(
        related.result.rows[0]["r"]["__type"], "LIKES",
        "{related:?}"
    );

    let relation = execute(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (a:Person)-[r:LIKES]->(c:Person) RETURN a.name AS source, c.name AS target, r.weight AS weight",
        None,
    )
    .unwrap();
    assert_eq!(
        relation.rows,
        vec![json!({"source": "Alice", "target": "Carol", "weight": 2})]
    );

    execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (n:Person {name: 'Bob'}) DETACH DELETE n",
        None,
    )
    .unwrap();
    let after = execute(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (n:Person) RETURN n.name AS name ORDER BY name",
        None,
    )
    .unwrap();
    assert_eq!(
        after.rows,
        vec![json!({"name": "Alice"}), json!({"name": "Carol"})]
    );
}

#[test]
fn gql_and_gremlin_updates_and_deletes_commit_through_the_same_delta_boundary() {
    for (language, update, delete) in [
        (
            QueryLanguage::Gql,
            "MATCH (n:Person) WHERE n.name = 'Alice' SET n.age = 36",
            "MATCH (n:Person) WHERE n.name = 'Carol' DELETE n",
        ),
        (
            QueryLanguage::Gremlin,
            "g.V().hasLabel('Person').has('name', 'Alice').property('age', 36)",
            "g.V().hasLabel('Person').has('name', 'Carol').drop()",
        ),
    ] {
        let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
        execute_mutation(&graph, language, update, None).unwrap();
        execute_mutation(&graph, language, delete, None).unwrap();

        let rows = execute(
            &graph,
            QueryLanguage::Cypher,
            "MATCH (n:Person) RETURN n.name AS name, n.age AS age ORDER BY name",
            None,
        )
        .unwrap()
        .rows;
        assert_eq!(
            rows,
            vec![
                json!({"name": "Alice", "age": 36}),
                json!({"name": "Bob", "age": 29}),
            ],
            "{language:?}"
        );
    }
}

#[test]
fn gql_and_gremlin_relationship_create_and_delete_reach_lmpg() {
    let gql_graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(1));
    let created = execute_mutation(
        &gql_graph,
        QueryLanguage::Gql,
        "INSERT (:Person {name: 'Alice'})-[:LIKES {weight: 2}]->(:Person {name: 'Bob'})",
        None,
    )
    .unwrap();
    assert_eq!(created.affected, 3);
    assert_eq!(
        execute(
            &gql_graph,
            QueryLanguage::Cypher,
            "MATCH ()-[r:LIKES]->() RETURN r.weight AS weight",
            None,
        )
        .unwrap()
        .rows,
        vec![json!({"weight": 2})]
    );
    execute_mutation(
        &gql_graph,
        QueryLanguage::Gql,
        "MATCH ()-[r:LIKES]->() DELETE r",
        None,
    )
    .unwrap();

    let gremlin_graph = fixture(Arc::new(Store::new_in_memory()), 2, "");
    execute_mutation(
        &gremlin_graph,
        QueryLanguage::Gremlin,
        "g.V().has('name', 'Alice').as('a').V().has('name', 'Bob').addE('LIKES').from('a').property('weight', 3)",
        None,
    )
    .unwrap();
    assert_eq!(
        execute(
            &gremlin_graph,
            QueryLanguage::Cypher,
            "MATCH ()-[r:LIKES]->() RETURN r.weight AS weight",
            None,
        )
        .unwrap()
        .rows,
        vec![json!({"weight": 3})]
    );
    execute_mutation(
        &gremlin_graph,
        QueryLanguage::Gremlin,
        "g.E().hasLabel('LIKES').drop()",
        None,
    )
    .unwrap();
    assert!(execute(
        &gremlin_graph,
        QueryLanguage::Cypher,
        "MATCH ()-[r:LIKES]->() RETURN r",
        None,
    )
    .unwrap()
    .rows
    .is_empty());
}

#[test]
fn one_cypher_statement_can_atomically_create_both_endpoints_and_the_relationship() {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(1));
    let mutation = execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        "CREATE (a:Person {name: 'Alice'})-[r:KNOWS {since: 2025}]->(b:Person {name: 'Bob'}) RETURN a.name AS source, b.name AS target",
        None,
    )
    .unwrap();
    assert_eq!(mutation.affected, 3);
    assert_eq!(
        mutation.result.rows,
        vec![json!({"source": "Alice", "target": "Bob"})]
    );

    let rows = execute(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a.name AS source, b.name AS target, r.since AS since",
        None,
    )
    .unwrap()
    .rows;
    assert_eq!(
        rows,
        vec![json!({"source": "Alice", "target": "Bob", "since": 2025})]
    );
}

#[test]
fn unsupported_multi_label_mutation_publishes_nothing() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let before = graph.store.committed_version();
    let error = execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        "CREATE (:Person:Employee {name: 'Eve'})",
        None,
    )
    .unwrap_err();
    assert!(error.to_string().contains("exactly one label"));
    assert_eq!(graph.store.committed_version(), before);
    let rows = execute(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (n:Person) WHERE n.name = 'Eve' RETURN n",
        None,
    )
    .unwrap()
    .rows;
    assert!(rows.is_empty());
}

/// Label writes that leave a record's one primary label as it is (setting
/// the label it has, removing one it lacks) are within the model: they
/// succeed and commit nothing. Any other label change is refused whole.
#[test]
fn label_writes_that_keep_the_primary_label_succeed_and_others_are_refused() {
    let graph = fixture(Arc::new(Store::new_in_memory()), 1, "");
    let before = graph.store.committed_version();
    for statement in [
        "MATCH (n:Person) SET n:Person RETURN n.name AS name",
        "MATCH (n:Person) REMOVE n:Employee RETURN n.name AS name",
    ] {
        let result = execute_mutation(&graph, QueryLanguage::Cypher, statement, None)
            .unwrap_or_else(|error| panic!("{statement}: {error}"));
        assert_eq!(result.affected, 0, "{statement}");
        assert!(!result.result.rows.is_empty(), "{statement}");
    }
    for statement in [
        "MATCH (n:Person) SET n:Employee",
        "MATCH (n:Person) REMOVE n:Person",
    ] {
        let error = execute_mutation(&graph, QueryLanguage::Cypher, statement, None).unwrap_err();
        assert!(
            error.to_string().contains("retypeRecord"),
            "{statement}: {error}"
        );
    }
    assert_eq!(graph.store.committed_version(), before);
}

/// The bounded-mutation budget counts the records a statement touches or
/// creates, not the tenant: a one-node create fits a budget of one in a
/// larger tenant, while an update of every `Person` does not.
#[test]
fn bounded_mutation_counts_the_touched_working_set_not_the_tenant() {
    use rushdb_language::{execute_mutation_with_budget, LanguageError};

    let graph = fixture(Arc::new(Store::new_in_memory()), 2, "");
    let ok = execute_mutation_with_budget(
        &graph,
        QueryLanguage::Cypher,
        "CREATE (n:Extra {name: 'x'})",
        None,
        1,
    )
    .unwrap();
    assert!(ok.commit_index > 0);

    let err = execute_mutation_with_budget(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (n:Person) SET n.flag = true",
        None,
        1,
    )
    .unwrap_err();
    assert!(
        matches!(err, LanguageError::GraphTooLarge(_)),
        "a write touching more records than the budget is refused, got {err:?}"
    );
}
