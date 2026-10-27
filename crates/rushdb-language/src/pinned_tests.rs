use super::*;
use rushdb_lmpg::FieldChange;
use rushdb_storage::{ReadLimits, Store};
use rushdb_types::command::{CommitBatch, IsolationMode, Mutation, COMMAND_VERSION};
use serde_json::json;

#[test]
fn mutation_byte_limit_rejects_fat_properties_and_edges_without_committing() {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(69));
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    let params = json!({"payload": "x".repeat(8192)});
    let error = execute_mutation_in_view_with_limits(
        &graph,
        &snapshot,
        QueryLanguage::Cypher,
        "CREATE (a:Person {payload: $payload}) RETURN a",
        Some(&params),
        100,
        4096,
        &|| false,
        None,
    )
    .unwrap_err();
    assert!(
        matches!(error, LanguageError::MutationBytesExceeded(4096)),
        "{error}"
    );
    assert_eq!(graph.store.committed_version(), 0);

    execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        "CREATE (:Person {name: 'a'})",
        None,
    )
    .unwrap();
    let before = graph.store.committed_version();
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    let error = execute_mutation_in_view_with_limits(
        &graph,
        &snapshot,
        QueryLanguage::Cypher,
        "MATCH (a:Person) CREATE (a)-[:KNOWS {payload: $payload}]->(a)",
        Some(&params),
        100,
        4096,
        &|| false,
        None,
    )
    .unwrap_err();
    assert!(
        matches!(error, LanguageError::MutationBytesExceeded(4096)),
        "{error}"
    );
    assert_eq!(graph.store.committed_version(), before);
}

#[test]
fn mutation_byte_limit_counts_only_what_the_statement_touches() {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(69));
    graph
        .create_record(
            "Person",
            None,
            vec![("payload".into(), FieldChange::Set(json!("x".repeat(8192))))],
        )
        .unwrap();
    // The fat record is not copied for a write that does not touch it.
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    execute_mutation_in_view_with_limits(
        &graph,
        &snapshot,
        QueryLanguage::Cypher,
        "CREATE (:Other)",
        None,
        100,
        4096,
        &|| false,
        None,
    )
    .unwrap();
    // A write that changes it holds it, and is refused over the limit.
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    let error = execute_mutation_in_view_with_limits(
        &graph,
        &snapshot,
        QueryLanguage::Cypher,
        "MATCH (p:Person) SET p.flag = true",
        None,
        100,
        4096,
        &|| false,
        None,
    )
    .unwrap_err();
    assert!(matches!(error, LanguageError::MutationBytesExceeded(4096)));
    assert_eq!(graph.store.committed_version(), snapshot.commit_index());
}

fn on_backends(test: impl Fn(&Lmpg)) {
    let path = rushdb_storage::TempPath::store("grafeo-pins");
    for persistent in [false, true] {
        let store = if persistent {
            Store::open(&path).unwrap()
        } else {
            Store::new_in_memory()
        };
        test(&Lmpg::new(Arc::new(store), TenantId::from_ordinal(69)));
    }
    rushdb_storage::remove_store(&path).unwrap();
}

fn fixture(graph: &Lmpg) -> (rushdb_lmpg::RecordRef, rushdb_lmpg::RelationshipRef) {
    let source = graph
        .create_record(
            "Person",
            None,
            vec![("name".into(), FieldChange::Set(json!("before")))],
        )
        .unwrap();
    let target = graph
        .create_record(
            "Person",
            None,
            vec![("name".into(), FieldChange::Set(json!("target")))],
        )
        .unwrap();
    let edge = graph
        .create_relationship(
            "KNOWS",
            source,
            target,
            vec![("since".into(), FieldChange::Set(json!(2001)))],
        )
        .unwrap();
    (source, edge)
}

#[test]
fn pinned_adapter_and_result_reconstruction_survive_writes_deletion_and_gc() {
    on_backends(|graph| {
        let (source, edge) = fixture(graph);
        let view = Arc::new(graph.store.read_view().unwrap());
        let adapter =
            LmpgGraphStore::new(Arc::new(OverlayView::new(view.clone())), graph.tenant, None);
        graph
            .update_fields(
                source,
                vec![("name".into(), FieldChange::Set(json!("after")))],
            )
            .unwrap();
        graph.delete_relationship(edge).unwrap();
        graph.store.gc(u64::MAX).unwrap();
        assert_eq!(
            adapter.current_epoch(),
            grafeo_common::types::EpochId::new(view.commit_index())
        );
        assert_eq!(adapter.out_degree(NodeId::new(source.ordinal)), 1);
        adapter.check().unwrap();
        for language in [QueryLanguage::Cypher, QueryLanguage::Gql] {
            let result = execute_in_view(
                &view,
                graph.tenant,
                language,
                "MATCH (p:Person)-[e:KNOWS]->(f:Person) RETURN p, e",
                None,
            )
            .unwrap();
            assert_eq!(result.rows.len(), 1);
            assert_eq!(result.rows[0]["p"]["name"], json!("before"));
            assert_eq!(result.rows[0]["e"]["since"], json!(2001));
        }
        for (language, query) in [
            (QueryLanguage::Gremlin, "g.V().hasLabel('Person').has('name', 'before').values('name')"),
            (QueryLanguage::SqlPgq, "SELECT * FROM GRAPH_TABLE (MATCH (p:Person)-[:KNOWS]->(f:Person) COLUMNS (p.name AS source)) AS g"),
        ] {
            let result = execute_in_view(&view, graph.tenant, language, query, None).unwrap();
            assert_eq!(result.rows.len(), 1);
            assert!(result.rows[0].as_object().unwrap().values().any(|value| value == "before"));
        }
        assert_eq!(
            execute(
                graph,
                QueryLanguage::Cypher,
                "MATCH (p:Person)-[:KNOWS]->(f) RETURN p",
                None
            )
            .unwrap()
            .rows
            .len(),
            0
        );
    });
}

#[test]
fn graph_result_budget_rejects_before_retaining_rows() {
    on_backends(|graph| {
        fixture(graph);
        let view = Arc::new(graph.store.read_view().unwrap());
        let error = execute_in_view_with_budget(
            &view,
            graph.tenant,
            QueryLanguage::Cypher,
            "MATCH (p:Person) RETURN p",
            None,
            Some(1),
        )
        .expect_err("tiny graph result budget");
        assert!(error.to_string().contains("execution byte budget"));
    });
}

#[test]
fn graph_mutation_in_view_does_not_observe_post_pin_records() {
    on_backends(|graph| {
        fixture(graph);
        let view = Arc::new(graph.store.read_view().unwrap());
        graph
            .create_record(
                "AfterPin",
                None,
                vec![("name".into(), FieldChange::Set(json!("late")))],
            )
            .unwrap();
        // The pinned view has no `AfterPin` record: nothing matches, so the
        // write changes nothing and commits nothing.
        let before = graph.store.committed_version();
        let result = execute_mutation_in_view_with_budget(
            graph,
            &view,
            QueryLanguage::Cypher,
            "MATCH (p:AfterPin) SET p.name='changed'",
            None,
            MUTATION_MAX_NODES,
        )
        .unwrap();
        assert_eq!(result.affected, 0);
        assert_eq!(result.commit_index, view.commit_index());
        assert_eq!(graph.store.committed_version(), before);
    });
}

#[test]
fn graph_mutation_reads_one_pinned_input_view() {
    on_backends(|graph| {
        let (source, edge) = fixture(graph);
        let view = Arc::new(graph.store.read_view().unwrap());
        graph
            .update_fields(
                source,
                vec![("name".into(), FieldChange::Set(json!("after")))],
            )
            .unwrap();
        graph.delete_relationship(edge).unwrap();
        graph.store.gc(u64::MAX).unwrap();
        // Staged against the pinned view: the record still has its old name
        // and the relationship still exists there.
        let staged = stage_statement_mutation_in_overlay(
            graph,
            QueryLanguage::Cypher,
            "MATCH (p:Person {name: 'before'})-[e:KNOWS]->(:Person) SET e.seen = true",
            None,
            MUTATION_MAX_NODES,
            &Arc::new(OverlayView::new(view)),
            None,
        )
        .unwrap();
        assert!(
            matches!(
                staged.ops.as_slice(),
                [rushdb_lmpg::BatchOp::UpdateFields {
                    owner: rushdb_types::ids::OwnerKind::Relationship,
                    owner_ord,
                    ..
                }] if *owner_ord == edge.ordinal
            ),
            "{} ops",
            staged.ops.len()
        );
    });
}

fn corrupt(graph: &Lmpg, storage_key: Vec<u8>) {
    // Malformed base rows are refused by apply-time derivation, so plant them
    // with derivation suspended to prove readers fail on them.
    let commit_key = uuid::Uuid::now_v7();
    graph.store.inject_without_derivation(|| {
        graph
            .store
            .commit(commit_key, |_, _| {
                Ok((
                    CommitBatch {
                        command_version: COMMAND_VERSION,
                        tenant: graph.tenant,
                        transaction_id: String::new(),
                        commit_key,
                        start_index: 0,
                        isolation: IsolationMode::ReadCommitted,
                        preconditions: vec![],
                        mutations: vec![Mutation::PutRawKey {
                            key: storage_key,
                            value: vec![1],
                        }],
                    },
                    (),
                ))
            })
            .unwrap()
    });
}

#[test]
fn corrupt_reads_fail_every_language_instead_of_returning_partial_results() {
    on_backends(|graph| {
        let (source, _) = fixture(graph);
        corrupt(
            graph,
            rushdb_types::keys::record_hdr_key(graph.tenant, source.ordinal),
        );
        for (language, query) in [
            (
                QueryLanguage::Cypher,
                "MATCH (p:Person) RETURN p.name AS name",
            ),
            (QueryLanguage::Gql, "MATCH (p:Person) RETURN p.name AS name"),
            (
                QueryLanguage::Gremlin,
                "g.V().hasLabel('Person').values('name')",
            ),
            (
                QueryLanguage::SqlPgq,
                "SELECT * FROM GRAPH_TABLE (MATCH (p:Person) COLUMNS (p.name AS name)) AS g",
            ),
        ] {
            assert!(
                matches!(
                    execute(graph, language, query, None),
                    Err(LanguageError::Storage(StorageError::CorruptLog))
                ),
                "{language:?}"
            );
        }
        let ci = graph.store.committed_version();
        assert!(matches!(
            execute_mutation(
                graph,
                QueryLanguage::Cypher,
                "MATCH (p:Person) SET p.name='bad'",
                None
            ),
            Err(LanguageError::Storage(StorageError::CorruptLog))
        ));
        assert_eq!(graph.store.committed_version(), ci);
    });
}

#[test]
fn first_storage_failure_is_sticky_and_stops_further_reads() {
    on_backends(|graph| {
        fixture(graph);
        let view = Arc::new(
            graph
                .store
                .read_with_limits(
                    None,
                    ReadLimits {
                        max_item_bytes: 1,
                        ..Default::default()
                    },
                )
                .unwrap(),
        );
        let adapter = LmpgGraphStore::new(Arc::new(OverlayView::new(view)), graph.tenant, None);
        assert!(adapter.node_ids().is_empty());
        assert!(matches!(
            adapter.check(),
            Err(StorageError::ResourceExhausted(_))
        ));
        adapter.read::<()>(|| panic!("poisoned adapter performed more storage work"));
        assert!(matches!(
            adapter.check(),
            Err(StorageError::ResourceExhausted(_))
        ));
        let view = Arc::new(
            graph
                .store
                .read_with_limits(
                    None,
                    ReadLimits {
                        max_item_bytes: 1,
                        ..Default::default()
                    },
                )
                .unwrap(),
        );
        assert!(matches!(
            execute_in_view(
                &view,
                graph.tenant,
                QueryLanguage::Cypher,
                "MATCH (p:Person) RETURN p",
                None
            ),
            Err(LanguageError::Storage(StorageError::ResourceExhausted(_)))
        ));
        let view = Arc::new(graph.store.read_view().unwrap());
        view.cancellation().cancel();
        assert!(matches!(
            execute_in_view(
                &view,
                graph.tenant,
                QueryLanguage::Cypher,
                "MATCH (p:Person) RETURN p",
                None
            ),
            Err(LanguageError::Storage(StorageError::Cancelled))
        ));
    });
}

#[test]
fn malformed_ordinal_index_fails_without_suffix_slice_panic() {
    on_backends(|graph| {
        fixture(graph);
        corrupt(
            graph,
            rushdb_types::keys::key(graph.tenant, &[rushdb_types::keys::P_REL_BY_ORD]),
        );
        let view = Arc::new(OverlayView::new(Arc::new(graph.store.read_view().unwrap())));
        assert!(matches!(
            LmpgGraphStore::new(view, graph.tenant, None).try_edge_ids(),
            Err(StorageError::CorruptLog)
        ));
    });
}

#[test]
fn native_graph_mutation_and_pinned_queries_survive_reopen() {
    let path = rushdb_storage::TempPath::store("grafeo-reopen");
    let tenant = TenantId::from_ordinal(69);
    let graph = Lmpg::new(Arc::new(Store::open(&path).unwrap()), tenant);
    fixture(&graph);
    execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (p:Person) WHERE p.name='before' SET p.name='updated' RETURN p",
        None,
    )
    .unwrap();
    drop(graph);
    let graph = Lmpg::new(Arc::new(Store::open(&path).unwrap()), tenant);
    let result = execute(
        &graph,
        QueryLanguage::Cypher,
        "MATCH (p:Person)-[e:KNOWS]->(f:Person) RETURN p, e",
        None,
    )
    .unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0]["p"]["name"], json!("updated"));
    assert_eq!(result.rows[0]["e"]["since"], json!(2001));
    drop(graph);
    rushdb_storage::remove_store(&path).unwrap();
}

#[test]
fn planner_statistics_come_from_the_catalog_counters() {
    on_backends(|graph| {
        fixture(graph);
        graph.create_record("City", None, vec![]).unwrap();
        let view = Arc::new(OverlayView::new(Arc::new(graph.store.read_view().unwrap())));
        let store = LmpgGraphStore::new(view, graph.tenant, None);
        let statistics = store.statistics();
        assert_eq!((statistics.total_nodes, statistics.total_edges), (3, 1));
        assert_eq!(statistics.labels["Person"].node_count, 2);
        assert_eq!(statistics.labels["City"].node_count, 1);
        assert_eq!(statistics.edge_types["KNOWS"].edge_count, 1);
        assert_eq!(store.node_count(), 3);
        assert_eq!(store.edge_count(), 1);
        assert_eq!(store.nodes_by_label_count("Person"), 2);
        assert!((store.estimate_avg_degree("KNOWS", true) - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(store.all_edge_types(), vec!["KNOWS".to_string()]);
    });
}

/// Equality through declared ordered indexes returns exactly what a label
/// scan returns, across integer/float coercion, signed zeros, strings,
/// booleans and arrays, for reads and for writes.
#[test]
fn indexed_equality_matches_the_label_scan() {
    let values = [
        json!(1),
        json!(1.0),
        json!(2),
        json!("1"),
        json!(-0.0),
        json!(0),
        json!(true),
        json!([1]),
        json!(5.5),
        json!(0.999_999_999_999_999_9),
    ];
    let seeded = |indexed: bool| {
        let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(70));
        for (i, value) in values.iter().enumerate() {
            for label in ["User", "Other"] {
                graph
                    .create_record(
                        label,
                        None,
                        vec![
                            (
                                "name".into(),
                                FieldChange::Set(json!(format!("{label}{i}"))),
                            ),
                            ("v".into(), FieldChange::Set(value.clone())),
                        ],
                    )
                    .unwrap();
            }
        }
        if indexed {
            for label in ["User", "Other"] {
                let def = graph
                    .define_index(
                        &format!("{label}_v"),
                        label,
                        "v",
                        rushdb_types::index::IndexKind::Ordered,
                    )
                    .unwrap();
                while graph.index_step(def.id, 64).unwrap() != rushdb_lmpg::index::IndexStep::Idle {
                }
            }
        }
        graph
    };
    let scanned = seeded(false);
    let indexed = seeded(true);
    let adapter = |graph: &Lmpg| {
        LmpgGraphStore::new(
            Arc::new(OverlayView::new(Arc::new(graph.store.read_view().unwrap()))),
            graph.tenant,
            None,
        )
    };
    assert!(!adapter(&scanned).has_property_index("v"));
    assert!(adapter(&indexed).has_property_index("v"));

    for literal in ["1", "1.0", "2", "'1'", "-0.0", "0", "true", "5.5", "3"] {
        let names = |graph: &Lmpg| {
            execute(
                graph,
                QueryLanguage::Cypher,
                &format!(
                    "MATCH (n:User) WHERE n.v = {literal} RETURN n.name AS name ORDER BY name"
                ),
                None,
            )
            .unwrap()
            .rows
        };
        assert_eq!(names(&scanned), names(&indexed), "n.v = {literal}");
    }
    let touched = |graph: &Lmpg| {
        execute_mutation(
            graph,
            QueryLanguage::Cypher,
            "MATCH (n:User) WHERE n.v = 1 SET n.hit = true RETURN n.name AS name ORDER BY name",
            None,
        )
        .unwrap()
        .result
        .rows
    };
    let hit = touched(&scanned);
    assert!(!hit.is_empty());
    assert_eq!(hit, touched(&indexed));
}

#[test]
fn long_statements_run_and_overlong_ones_are_refused_not_crashing() {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(71));
    let script = |n: usize| -> String {
        use std::fmt::Write as _;
        let mut script = String::new();
        for i in 0..n {
            writeln!(script, "CREATE (n{i}:P {{i: {i}}})").unwrap();
        }
        script
    };
    // A long setup script (the TCK's movie graph is ~500 clauses).
    let created = execute_mutation(&graph, QueryLanguage::Cypher, &script(2_000), None).unwrap();
    assert_eq!(created.affected, 2_000);
    let refused = execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        &script(MAX_STATEMENT_CLAUSES + 1),
        None,
    )
    .unwrap_err();
    assert!(
        matches!(refused, LanguageError::StatementTooLong { .. }),
        "{refused}"
    );
}

/// Grafeo cannot interrupt an operator, so a statement that would never
/// finish (an unbounded variable-length match around a self-loop) ends
/// because its reads fail once the request is cancelled.
#[test]
fn a_statement_that_never_ends_is_cancelled_through_its_reads() {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(72));
    execute_mutation(
        &graph,
        QueryLanguage::Cypher,
        "CREATE (s:Single), (a:A {num: 42}), (b:B {num: 46}), (c:C) \
         CREATE (s)-[:REL]->(a), (s)-[:REL]->(b), (a)-[:REL]->(c), (b)-[:LOOP]->(b)",
        None,
    )
    .unwrap();
    let view = Arc::new(graph.store.read_view().unwrap());
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_millis(300);
    let result = execute_in_view_with_budget_cancellable(
        &view,
        graph.tenant,
        QueryLanguage::Cypher,
        "MATCH (a:Single) OPTIONAL MATCH (a)-[*3..]-(b) RETURN count(b)",
        None,
        None,
        &|| std::time::Instant::now() > deadline,
        None,
    );
    assert!(
        matches!(result, Err(LanguageError::Cancelled)),
        "{result:?}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "cancelled promptly, not after the statement ran out: {:?}",
        started.elapsed()
    );
}

#[test]
fn entities_inside_untyped_lists_and_maps_return_as_records() {
    // `tail(nodes(p))` is typed `Any`: its nodes arrive as Grafeo's internal
    // maps and must still return as records with public ids.
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(70));
    fixture(&graph);
    let statement = "MATCH p = (a:Person {name: 'before'})-[*1]->(x) \
                     WITH tail(nodes(p)) AS ns RETURN ns, {first: ns[0]} AS m";
    let check = |rows: &[Json]| {
        assert_eq!(rows.len(), 1);
        let node = &rows[0]["ns"][0];
        assert_eq!(node["__label"], "Person");
        assert_eq!(node["name"], "target");
        assert!(uuid::Uuid::parse_str(node["__id"].as_str().unwrap()).is_ok());
        assert!(node.get("_labels").is_none());
        assert_eq!(rows[0]["m"]["first"], *node);
    };
    check(
        &execute(&graph, QueryLanguage::Cypher, statement, None)
            .unwrap()
            .rows,
    );
    let write = format!("CREATE (:Marker) WITH 1 AS one {statement}");
    check(
        &execute_mutation(&graph, QueryLanguage::Cypher, &write, None)
            .unwrap()
            .result
            .rows,
    );
}
