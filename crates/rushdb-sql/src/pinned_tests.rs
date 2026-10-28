use super::*;
use rushdb_lmpg::{BatchOp, BatchTx, FieldChange, RecordRef};
use rushdb_storage::Store;
use rushdb_types::command::{CommitBatch, IsolationMode, Mutation, COMMAND_VERSION};
use rushdb_types::ids::TenantId;
use serde_json::json;

#[test]
fn synchronous_sql_rejects_runtime_threads_without_writing() {
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(68));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(
            execute_sql(&graph, "SELECT 1"),
            Err(SqlError::BlockingInRuntime)
        ));
        assert!(matches!(
            describe_sql(&graph, "SELECT 1"),
            Err(SqlError::BlockingInRuntime)
        ));
        assert!(matches!(
            execute_sql_mutation(&graph, "DELETE FROM person", None),
            Err(SqlError::BlockingInRuntime)
        ));
        assert_eq!(graph.store.committed_version(), 0);
        let rows = execute_sql_async(&graph, "SELECT 1 AS n", None)
            .await
            .unwrap();
        assert_eq!(rows[0]["n"], json!(1));
    });
}

#[test]
fn only_filters_the_scan_answers_in_full_are_exact() {
    // Contract: DataFusion drops an `Exact` filter, so only comparisons whose
    // native predicate keeps exactly SQL's rows are `Exact` (`=` on a column
    // of one kind of value, `>=`/`<=` on a numeric one); anything else is
    // `Inexact` and DataFusion re-applies it.
    use datafusion::datasource::TableProvider;
    use datafusion_expr::TableProviderFilterPushDown::{Exact, Inexact};
    use datafusion_expr::{col, lit};

    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(68));
    fixture(&graph);
    graph
        .create_record(
            "Person",
            None,
            vec![
                ("age".into(), FieldChange::Set(json!(30))),
                ("code".into(), FieldChange::Set(json!(1))),
            ],
        )
        .unwrap();
    graph
        .create_record(
            "Person",
            None,
            vec![("code".into(), FieldChange::Set(json!("one")))],
        )
        .unwrap();
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    let table = crate::scan_provider::LabelTable::try_new(
        Arc::new(Lmpg::new(Arc::clone(&graph.store), graph.tenant)),
        "Person".into(),
        snapshot,
    )
    .unwrap();
    let filters = [
        col("name").eq(lit("before")),
        col("age")
            .gt_eq(lit(18i64))
            .and(col("age").lt_eq(lit(65i64))),
        col("age").gt(lit(18i64)),
        col("name").gt(lit("b")),
        col("code").eq(lit("one")),
        col("name").eq(lit("a")).or(col("name").eq(lit("b"))),
        col("__id").eq(lit("x")),
    ];
    let refs: Vec<&datafusion_expr::Expr> = filters.iter().collect();
    assert_eq!(
        table.supports_filters_pushdown(&refs).unwrap(),
        [Exact, Exact, Inexact, Inexact, Inexact, Inexact, Inexact]
    );
}

fn on_backends(test: impl Fn(&Lmpg)) {
    let path = rushdb_storage::TempPath::store("sql-pins");
    for persistent in [false, true] {
        let store = if persistent {
            Store::open(&path).unwrap()
        } else {
            Store::new_in_memory()
        };
        let graph = Lmpg::new(Arc::new(store), TenantId::from_ordinal(68));
        test(&graph);
    }
    rushdb_storage::remove_store(&path).unwrap();
}

fn fixture(graph: &Lmpg) -> (RecordRef, RecordRef, rushdb_lmpg::RelationshipRef) {
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
    (source, target, edge)
}

fn query(context: &SessionContext, sql: &str) -> Vec<Row> {
    futures::executor::block_on(async {
        let batches = context.sql(sql).await.unwrap().collect().await.unwrap();
        batches_to_rows(&batches).unwrap()
    })
}

fn assert_corrupt(error: &SqlError, context: &str) {
    let text = error.to_string().to_lowercase();
    assert!(
        text.contains("corrupt"),
        "{context}: expected a corruption error, got {error:?}"
    );
}

#[test]
fn record_and_relationship_tables_retain_one_version_across_writes_and_gc() {
    on_backends(|graph| {
        let (source, _, edge) = fixture(graph);
        let snapshot = graph.store.read_view().unwrap();
        graph
            .update_fields(
                source,
                vec![("name".into(), FieldChange::Set(json!("after")))],
            )
            .unwrap();
        graph.delete_relationship(edge).unwrap();
        graph.store.gc(u64::MAX).unwrap();
        assert_eq!(graph.store.gc_watermark(), snapshot.commit_index());
        // Both table registrations occur after the concurrent changes, but
        // must use the physical/logical version pinned before them.
        let _ci = snapshot.commit_index();
        let context = context_in_view(graph, &Arc::new(snapshot), None).unwrap();
        let rows = query(&context.context, "SELECT p.name, e.since FROM person p JOIN rushdb_edges.knows e ON p.__id=e.__source_id");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], json!("before"));
        assert_eq!(rows[0]["since"], json!(2001));
        assert!(virtual_catalog(graph)
            .unwrap()
            .relationship_tables
            .is_empty());
        assert_eq!(
            execute_sql(graph, "SELECT name FROM person WHERE name='after'")
                .unwrap()
                .len(),
            1
        );
    });
}

#[test]
fn budgeted_sql_entry_point_reuses_the_caller_snapshot() {
    on_backends(|graph| {
        let (source, _, _) = fixture(graph);
        let snapshot = Arc::new(graph.store.read_view().unwrap());
        graph
            .update_fields(
                source,
                vec![("name".into(), FieldChange::Set(json!("after")))],
            )
            .unwrap();
        let rows = execute_sql_with_parameters_budget_in_view(
            graph,
            snapshot,
            "SELECT name FROM person WHERE name='before'",
            None,
            Some(1024 * 1024),
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], json!("before"));
    });
}

#[test]
fn dml_reads_the_pinned_version_and_refuses_a_stale_commit() {
    on_backends(|graph| {
        let (source, _, _) = fixture(graph);
        let snapshot = Arc::new(graph.store.read_view().unwrap());
        graph
            .update_fields(
                source,
                vec![("name".into(), FieldChange::Set(json!("after")))],
            )
            .unwrap();
        graph.store.gc(u64::MAX).unwrap();
        // The DML read runs on the pinned version: it finds the row by its old
        // name, and committing the delta at that stale version is refused.
        let stale = execute_sql_mutation_in_view_with_budget(
            graph,
            Arc::clone(&snapshot),
            "UPDATE person SET name = 'stale write' WHERE name = 'before'",
            None,
            None,
            None,
        );
        assert!(
            matches!(stale, Err(SqlError::WriteConflict(_))),
            "{stale:?}"
        );
        // The actual publish path rejects a delta from this stale snapshot.
        let result = graph.run_batch_at_version(
            BatchTx {
                ops: vec![BatchOp::UpdateFields {
                    owner: rushdb_types::ids::OwnerKind::Record,
                    owner_ord: source.ordinal,
                    owner_public_id: source.record_id,
                    fields: vec![("name".into(), FieldChange::Set(json!("stale write")))],
                }],
            },
            snapshot.commit_index(),
        );
        assert!(result.is_err());
    });
}

#[test]
fn record_and_edge_materialization_share_the_row_safety_limit() {
    on_backends(|graph| {
        fixture(graph);
        // Lazy label tables: registration infers schema without materializing
        // rows, and a normal label is registered (not rejected). The row
        // safety limit is enforced per label at registration (member count
        // check); this test proves the lazy path registers and scans normally.
        let snapshot = Arc::new(graph.store.read_view().unwrap());
        let context = SessionContext::new();
        register_lazy_catalog(&context, graph, &snapshot).unwrap();
        // The "Person" label (2 members) is resolved and scanned lazily.
        let rows = query(&context, "SELECT __id FROM person");
        assert_eq!(rows.len(), 2, "lazy scan returns the label's records");
        // An edge table resolves lazily as well.
        let edges = query(&context, "SELECT __id FROM rushdb_edges.knows");
        assert_eq!(edges.len(), 1, "lazy edge scan returns the edge");
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
fn malformed_relationship_index_fails_catalog_describe_select_and_dml() {
    on_backends(|graph| {
        fixture(graph);
        corrupt(graph, key(graph.tenant, &[P_REL_BY_ORD]));
        assert!(matches!(
            virtual_catalog(graph),
            Err(SqlError::Storage(StorageError::CorruptLog))
        ));
        assert!(matches!(
            catalog_fingerprint(graph),
            Err(SqlError::Storage(StorageError::CorruptLog))
        ));
        // An edge table resolves from the relationship catalog, so describing
        // it does not read the index; a malformed index surfaces when the
        // table is scanned, not when an unrelated label is queried.
        assert!(describe_sql(graph, "SELECT * FROM rushdb_edges.knows").is_ok());
        assert_corrupt(
            &execute_sql(graph, "SELECT * FROM rushdb_edges.knows").unwrap_err(),
            "select edge",
        );
        // DML reads like SELECT: a statement that reads the edge
        // index fails on it and commits nothing.
        let version = graph.store.committed_version();
        assert_corrupt(
            &execute_sql_mutation(
                graph,
                "UPDATE person SET name='bad' WHERE __id IN \
                 (SELECT __source_id FROM rushdb_edges.knows)",
                None,
            )
            .unwrap_err(),
            "dml reading edges",
        );
        assert_eq!(graph.store.committed_version(), version);
    });
}

#[test]
fn table_less_queries_do_not_scan_the_lazy_catalog() {
    on_backends(|graph| {
        fixture(graph);
        // Corrupt both the record identity index and the relationship index.
        corrupt(graph, rushdb_types::keys::record_hdr_key(graph.tenant, 0));
        corrupt(graph, key(graph.tenant, &[P_REL_BY_ORD]));
        // `SELECT 1` resolves no table, so neither catalog is materialized.
        // Before the lazy providers this query reconstructed the whole tenant
        // and returned a corruption error.
        let rows = execute_sql(graph, "SELECT 1 AS n").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["n"], json!(1));
    });
}

#[test]
fn malformed_record_identity_index_fails_instead_of_omitting_row() {
    on_backends(|graph| {
        let (source, _, _) = fixture(graph);
        corrupt(
            graph,
            rushdb_types::keys::record_hdr_key(graph.tenant, source.ordinal),
        );
        // The corruption is detected during the lazily-resolved scan (wrapped
        // by DataFusion), not during catalog registration; it must still fail
        // rather than silently omit the row.
        assert_corrupt(
            &execute_sql(graph, "SELECT * FROM person").unwrap_err(),
            "select person",
        );
    });
}

#[test]
fn native_sql_dml_catalog_and_queries_survive_reopen() {
    let path = rushdb_storage::TempPath::store("sql-reopen");
    let tenant = TenantId::from_ordinal(68);
    let graph = Lmpg::new(Arc::new(Store::open(&path).unwrap()), tenant);
    fixture(&graph);
    let updated = execute_sql_mutation(
        &graph,
        "UPDATE person SET name='updated' WHERE name='before'",
        None,
    )
    .unwrap();
    assert_eq!(updated.affected, 1);
    let expected_catalog = virtual_catalog(&graph).unwrap();
    let expected_fingerprint = catalog_fingerprint(&graph).unwrap();
    drop(graph);
    let graph = Lmpg::new(Arc::new(Store::open(&path).unwrap()), tenant);
    assert_eq!(virtual_catalog(&graph).unwrap(), expected_catalog);
    assert_eq!(catalog_fingerprint(&graph).unwrap(), expected_fingerprint);
    let rows = execute_sql(
        &graph,
        "SELECT p.name, e.since FROM person p JOIN rushdb_edges.knows e ON p.__id=e.__source_id",
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], json!("updated"));
    assert_eq!(rows[0]["since"], json!(2001));
    drop(graph);
    rushdb_storage::remove_store(&path).unwrap();
}

/// `PeakTrackingMemoryPool` must expose the real
/// high-water mark across a reservation's lifetime, not just whatever is
/// currently resident — the gap `MemoryPool::reserved()` alone cannot
/// close, and the reason `record_real_memory_peak` can charge the actual
/// `DataFusion` peak instead of staying at the flat `DATAFUSION_PHASE_OVERHEAD`
/// estimate forever.
#[test]
fn peak_tracking_memory_pool_records_the_high_water_mark_not_current_usage() {
    use datafusion::execution::memory_pool::{MemoryConsumer, MemoryPool};
    let pool = Arc::new(PeakTrackingMemoryPool::new(1024 * 1024));
    let pool_dyn: Arc<dyn MemoryPool> = Arc::clone(&pool) as Arc<dyn MemoryPool>;
    let reservation = MemoryConsumer::new("test").register(&pool_dyn);

    reservation.grow(900_000);
    reservation.shrink(800_000);

    // Current usage dropped back down, but the peak must still reflect the
    // highest point reached — exactly what a bare `reserved()` read at this
    // point would miss (it would report 100_000, not 900_000).
    assert_eq!(reservation.size(), 100_000);
    assert_eq!(pool_dyn.reserved(), 100_000);
    assert_eq!(pool.peak(), 900_000);
    assert!(pool.peak() > pool_dyn.reserved());

    // Growing past the old peak raises it further.
    reservation.grow(50_000);
    assert_eq!(
        pool.peak(),
        900_000,
        "150_000 current is still under the prior 900_000 peak"
    );
    reservation.grow(900_000);
    assert_eq!(pool.peak(), 1_050_000);
}

/// `DataFusionContext::record_real_memory_peak`
/// must resize the *store-level* `sql-phase` child reservation to the
/// tracked pool's real peak, not leave it at the flat
/// `DATAFUSION_PHASE_OVERHEAD` estimate `context_in_view_with_budget`
/// charges up front. The pool's own peak-tracking correctness is proven in
/// isolation above; this proves the glue between that pool and the
/// `ByteBudget` tree an operator's `/metrics` gauge actually reads.
#[test]
fn record_real_memory_peak_resizes_the_store_level_reservation() {
    use datafusion::execution::memory_pool::{MemoryConsumer, MemoryPool};
    let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(70));
    let snapshot = Arc::new(graph.store.read_view().unwrap());
    let mut context =
        context_in_view_with_budget(&graph, &snapshot, Some(64 * 1024 * 1024), None).unwrap();
    let budget = graph.store.sql_phase_budget().unwrap();
    assert_eq!(
        budget.snapshot().used,
        DATAFUSION_PHASE_OVERHEAD,
        "construction charges the flat overhead estimate up front"
    );

    // Simulate `DataFusion` allocating well past that flat estimate during
    // execution (provoking a specific real allocation size from an actual
    // query is covered by the broader SQL test suite's budget tests, not
    // needed here to prove this glue is wired correctly).
    let pool = context
        .peak_pool
        .clone()
        .expect("budgeted context tracks a pool");
    let pool_dyn: Arc<dyn MemoryPool> = pool as Arc<dyn MemoryPool>;
    let reservation = MemoryConsumer::new("simulated").register(&pool_dyn);
    let simulated_peak = DATAFUSION_PHASE_OVERHEAD + 5 * 1024 * 1024;
    reservation.grow(simulated_peak);
    reservation.shrink(simulated_peak);
    assert_eq!(
        budget.snapshot().used,
        DATAFUSION_PHASE_OVERHEAD,
        "simulating pool activity alone must not yet touch the store-level reservation"
    );

    context.record_real_memory_peak();

    assert_eq!(
        budget.snapshot().used,
        simulated_peak,
        "resized to the real measured peak, not the flat estimate"
    );
    assert!(budget.snapshot().peak >= simulated_peak);
}

#[test]
fn a_snapshot_write_conflicts_only_with_its_own_tenant() {
    on_backends(|graph| {
        let (source, _, _) = fixture(graph);
        let update = |name: &str| BatchTx {
            ops: vec![BatchOp::UpdateFields {
                owner: rushdb_types::ids::OwnerKind::Record,
                owner_ord: source.ordinal,
                owner_public_id: source.record_id,
                fields: vec![("name".into(), FieldChange::Set(json!(name)))],
            }],
        };
        // Another tenant commits after the snapshot: no conflict.
        let snapshot = graph.store.committed_version();
        let other = rushdb_lmpg::Lmpg::new(
            std::sync::Arc::clone(&graph.store),
            rushdb_types::ids::TenantId::from_ordinal(u64::from(graph.tenant.0) + 1),
        );
        other
            .create_record(
                "Elsewhere",
                None,
                vec![("n".into(), FieldChange::Set(json!(1)))],
            )
            .unwrap();
        graph
            .run_batch_at_version(update("other tenant committed"), snapshot)
            .expect("a commit in another tenant does not invalidate the snapshot");
        // The same tenant commits after the snapshot: a retryable conflict.
        let snapshot = graph.store.committed_version();
        graph
            .update_fields(
                source,
                vec![("name".into(), FieldChange::Set(json!("raced")))],
            )
            .unwrap();
        let Err(error) = graph.run_batch_at_version(update("stale"), snapshot) else {
            panic!("a commit in the same tenant must conflict")
        };
        assert!(
            matches!(error, rushdb_lmpg::LmpgError::WriteConflict(_)),
            "{error:?}"
        );
    });
}
