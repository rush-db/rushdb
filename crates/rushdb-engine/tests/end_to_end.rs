//! End-to-end tests for the in-process [`Engine`] facade.

use rushdb_engine::Engine;
use rushdb_engine::OperationClass;
use rushdb_lmpg::plan::{FieldChange, PlanError};
use rushdb_query::{Direction, Expr, FieldPred, Pagination, RelatedSpec, SearchRequest};
use rushdb_tx::Transaction;
use rushdb_types::command::IsolationMode;
use rushdb_types::ids::RecordId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

const SCOPE: &str = "engine-test-scope";

#[test]
fn fallible_replay_rejects_malformed_log_before_publishing() {
    let Err(error) = Engine::try_open_recovered(&[b"not-json".to_vec()], Some(SCOPE)) else {
        panic!("malformed replay unexpectedly succeeded")
    };
    assert!(matches!(error, rushdb_storage::StorageError::CorruptLog));
}

#[test]
fn execution_context_pins_reads_and_enforces_response_budget() {
    let engine = Engine::new_in_memory();
    engine
        .import_document(Some(SCOPE), "Employee", &json!({"name": "Ada"}))
        .unwrap();
    let mut context = engine
        .context(Some(SCOPE), OperationClass::Read, None, Some(1))
        .unwrap();
    let request = SearchRequest {
        labels: vec!["Employee".into()],
        ..Default::default()
    };
    let error = engine
        .search_with_context(&mut context, &request)
        .unwrap_err();
    assert!(matches!(error, rushdb_query::QueryError::ResourceLimit(_)));

    let mut pinned = engine
        .context(Some(SCOPE), OperationClass::Read, None, None)
        .unwrap();
    engine
        .import_document(Some(SCOPE), "Employee", &json!({"name": "Grace"}))
        .unwrap();
    let page = engine.search_with_context(&mut pinned, &request).unwrap();
    assert_eq!(page.total_matched, 1);
}

#[test]
fn context_aware_import_honors_deadline_before_mutation() {
    let engine = Engine::new_in_memory();
    let mut context = engine
        .context(
            Some(SCOPE),
            OperationClass::Write,
            Some(std::time::Duration::ZERO),
            None,
        )
        .unwrap();
    let error = engine
        .import_document_with_context(&mut context, "Employee", &json!({"name": "Ada"}))
        .unwrap_err();
    assert!(error.to_string().contains("deadline"));
    let page = engine
        .search(
            Some(SCOPE),
            &SearchRequest {
                labels: vec!["Employee".into()],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(page.total_matched, 0);
}

#[test]
fn context_aware_import_rejects_cancelled_and_foreign_contexts() {
    let source = Engine::new_in_memory();
    let destination = Engine::new_in_memory();
    let mut cancelled = source
        .context(None, OperationClass::Write, None, None)
        .unwrap();
    cancelled.read_view().cancellation().cancel();
    assert!(destination
        .import_document_with_context(&mut cancelled, "Audit", &json!({"value": 1}))
        .is_err());

    let mut foreign = source
        .context(None, OperationClass::Write, None, None)
        .unwrap();
    assert!(destination
        .import_document_with_context(&mut foreign, "Audit", &json!({"value": 1}))
        .is_err());
}

#[test]
fn context_aware_import_admits_staging_before_nested_allocations() {
    let engine = Engine::new_in_memory();
    let mut context = engine
        .context(None, OperationClass::Write, None, Some(8 * 1024))
        .unwrap();
    let nested = json!({"child": {"payload": "x".repeat(2048)}});
    assert!(engine
        .import_document_with_context(&mut context, "Audit", &nested)
        .is_err());
    let page = engine
        .search(
            None,
            &SearchRequest {
                labels: vec!["Audit".into()],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(page.total_matched, 0);
}

#[test]
fn context_aware_import_allows_bounded_nested_documents() {
    let engine = Engine::new_in_memory();
    let mut context = engine
        .context(None, OperationClass::Write, None, Some(256 * 1024))
        .unwrap();
    let nested = json!({
        "child": {"payload": "x".repeat(256)}
    });
    engine
        .import_document_with_context(&mut context, "Audit", &nested)
        .unwrap();
    let page = engine
        .search(
            None,
            &SearchRequest {
                labels: vec!["Audit".into()],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(page.total_matched, 1);
}

#[test]
fn rejected_import_restores_context_charge_for_reuse() {
    let engine = Engine::new_in_memory();
    let mut context = engine
        .context(None, OperationClass::Write, None, Some(32 * 1024))
        .unwrap();
    context.charge_bytes(1).unwrap();
    let before = context.charged_bytes();
    let rejected = json!({"child": {"payload": "x".repeat(16 * 1024)}});
    assert!(engine
        .import_document_with_context(&mut context, "Audit", &rejected)
        .is_err());
    assert_eq!(context.charged_bytes(), before);

    engine
        .import_document_with_context(&mut context, "Audit", &json!({"value": 1}))
        .unwrap();
    assert_eq!(context.charged_bytes(), before);
}

#[test]
fn execution_context_budget_covers_candidate_reconstruction_work() {
    let engine = Engine::new_in_memory();
    engine
        .import_document(Some(SCOPE), "Employee", &json!({"name": "Ada"}))
        .unwrap();
    let mut context = engine
        .context(Some(SCOPE), OperationClass::Read, None, Some(8))
        .unwrap();
    let request = SearchRequest {
        labels: vec!["Employee".into()],
        where_: Some(Expr::Field(FieldPred::Contains("name".into(), "A".into()))),
        ..Default::default()
    };
    let error = engine
        .search_with_context(&mut context, &request)
        .unwrap_err();
    assert!(matches!(error, rushdb_query::QueryError::ResourceLimit(_)));
    assert_eq!(
        context.charged_bytes(),
        0,
        "failed work admission is atomic"
    );
}

#[test]
fn end_to_end_import_search_and_aggregate() {
    let engine = Engine::new_in_memory();
    let scope = SCOPE;

    import_employees(&engine, scope);
    assert_exists_search(&engine, scope);
    let lmpg = create_department_and_edge(&engine, scope);
    assert_related_search(&engine, scope);
    assert_collect_search(&engine, scope);
    assert_aggregate(&lmpg);
}

fn import_employees(engine: &Engine, scope: &str) {
    engine
        .import_document(
            Some(scope),
            "Employee",
            &json!({"name": "Ada", "salary": 120_000, "status": "active"}),
        )
        .unwrap();
    engine
        .import_document(
            Some(scope),
            "Employee",
            &json!({"name": "Grace", "salary": 150_500, "status": null}),
        )
        .unwrap();
    engine
        .import_document(Some(scope), "Employee", &json!({"name": "Linus"}))
        .unwrap();
}

fn assert_exists_search(engine: &Engine, scope: &str) {
    // SearchQuery find with $exists distinction.
    let req = SearchRequest {
        labels: vec!["Employee".into()],
        where_: Some(Expr::Field(FieldPred::Exists("status".into(), true))),
        related: Vec::new(),
        collects: Vec::new(),
        pagination: Pagination {
            skip: 0,
            limit: Some(2),
        },
        order_by: Vec::new(),
        force_scan: false,
    };
    let page = engine.search(Some(scope), &req).unwrap();
    assert_eq!(page.total_matched, 2);
    assert_eq!(page.records.len(), 2);
}

fn create_department_and_edge(engine: &Engine, scope: &str) -> rushdb_lmpg::Lmpg {
    let dept = engine
        .import_document(Some(scope), "Department", &json!({"d": "eng"}))
        .unwrap();
    let lmpg = engine.tenant(Some(scope)).unwrap();
    let emp = rushdb_query::find_records(
        &lmpg,
        &SearchRequest {
            labels: vec!["Employee".into()],
            where_: Some(Expr::Field(FieldPred::Eq("name".into(), json!("Ada")))),
            ..Default::default()
        },
    )
    .unwrap();
    let ada_id = emp.records[0]["__id"].as_str().unwrap().to_string();
    let ada = lmpg_record_by_id_hex(&lmpg, &ada_id).expect("ada");

    let created = Transaction::begin(&lmpg)
        .create_relationship(
            "WORKS_IN",
            ada,
            dept,
            vec![("since".into(), FieldChange::Set(json!(2024)))],
        )
        .commit()
        .unwrap();
    assert_eq!(created.created_relationships.len(), 1);
    lmpg
}

fn assert_related_search(engine: &Engine, scope: &str) {
    // Traversal filter via RelatedSpec (induced outgoing edges).
    let req_related = SearchRequest {
        labels: vec!["Employee".into()],
        where_: None,
        related: vec![RelatedSpec {
            steps: Vec::new(),
            alias: Some("dept".into()),
            rel_type: Some("WORKS_IN".into()),
            direction: Direction::Outgoing,
            target_labels: vec!["Department".into()],
        }],
        collects: Vec::new(),
        pagination: Pagination::default(),
        order_by: Vec::new(),
        force_scan: false,
    };
    let page2 = engine.search(Some(scope), &req_related).unwrap();
    assert_eq!(page2.total_matched, 1);
}

fn assert_collect_search(engine: &Engine, scope: &str) {
    // $collect at the binding grain: batched adjacency collection projected.
    let req_collect = SearchRequest {
        labels: vec!["Employee".into()],
        related: Vec::new(),
        collects: vec![rushdb_query::CollectSpec {
            steps: Vec::new(),
            alias: "departments".into(),
            rel_type: Some("WORKS_IN".into()),
            direction: rushdb_query::Direction::Outgoing,
            target_labels: vec!["Department".into()],
            fields: vec![("d".into(), "d".into())],
            nested: vec![],
            filter: None,
            order_by: vec![],
            skip: 0,
            unique: true,
            limit: Some(5),
        }],
        ..Default::default()
    };
    let page3 = engine.search(Some(scope), &req_collect).unwrap();
    let ada_row = page3
        .records
        .iter()
        .find(|r| r.get("name").and_then(|v| v.as_str()) == Some("Ada"))
        .expect("ada row");
    assert_eq!(ada_row.pointer("/departments/0/d"), Some(&json!("eng")));
}

fn assert_aggregate(lmpg: &rushdb_lmpg::Lmpg) {
    // Aggregate: distinct-root count + sum at root grain.
    let agg = rushdb_query::aggregate(
        lmpg,
        &rushdb_query::AggregateRequest {
            labels: vec!["Employee".into()],
            group_by: vec!["status".into()],
            sums: vec![("payroll".into(), "salary".into())],
            counts: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(agg.len(), 3); // active / null / missing statuses distinct
    let total_payroll: f64 = agg
        .iter()
        .filter_map(|b| b.sums.get("payroll").and_then(serde_json::Value::as_f64))
        .sum();
    assert!((total_payroll - 270_500.0).abs() < f64::EPSILON);

    // Counting '$count' roots ignores paging multiplicity.
    let all = aggregate_count_roots(lmpg);
    assert_eq!(all, 3);
}

fn aggregate_count_roots(lmpg: &rushdb_lmpg::Lmpg) -> u64 {
    let buckets = rushdb_query::aggregate(
        lmpg,
        &rushdb_query::AggregateRequest {
            labels: vec!["Employee".into()],
            ..Default::default()
        },
    )
    .unwrap();
    buckets.iter().map(|b| b.count_roots).sum()
}

fn lmpg_record_by_id_hex(lmpg: &rushdb_lmpg::Lmpg, hex: &str) -> Option<rushdb_lmpg::RecordRef> {
    let bytes = rushdb_query::hex_decode_16(hex)?;
    let snap = lmpg.store.read_view().unwrap();
    let hdr = rushdb_lmpg::reader::try_access::record_by_id(&snap, lmpg.tenant, &RecordId(bytes))
        .ok()??;
    Some(rushdb_lmpg::RecordRef {
        record_id: bytes,
        ordinal: hdr.ordinal,
    })
}

#[test]
fn multi_statement_batch_is_atomic() {
    let engine = Engine::new_in_memory();
    let lmpg = engine.tenant(Some(SCOPE)).unwrap();

    let a = lmpg.create_record("A", None, vec![]).unwrap();
    let b = lmpg.create_record("B", None, vec![]).unwrap();

    // One atomic batch creating two records and a relationship between them.
    let tx = Transaction::begin(&lmpg)
        .create_record("C", None, vec![("v".into(), FieldChange::Set(json!(1)))])
        .create_record("D", None, vec![("v".into(), FieldChange::Set(json!(2)))]);
    let committed = tx.commit().unwrap();
    assert_eq!(committed.created_records.len(), 2);
    let c = committed.created_records[0];
    let d = committed.created_records[1];

    let done = Transaction::begin(&lmpg)
        .create_relationship("LINKS", c, d, vec![])
        .delete_record(a)
        .commit()
        .unwrap();
    assert_eq!(done.created_relationships.len(), 1);

    let snap = lmpg.store.read_view().unwrap();
    assert!(
        rushdb_lmpg::reader::try_access::record_by_ord(&snap, lmpg.tenant, b.ordinal)
            .unwrap()
            .is_some()
    );
    assert!(
        rushdb_lmpg::reader::try_access::record_by_ord(&snap, lmpg.tenant, a.ordinal)
            .unwrap()
            .is_none(),
        "batch delete applied atomically"
    );
}

#[test]
fn crash_recovery_preserves_tenant_scoped_graph() {
    let primary_engine = Engine::new_in_memory();
    primary_engine
        .import_document(
            Some(SCOPE),
            "Note",
            &json!({"body": "hello", "tags": ["a"]}),
        )
        .unwrap();

    let log = primary_engine.store.take_log_for_crash();
    let recovered = Engine::open_recovered(&log, Some(SCOPE));

    let page = recovered
        .search(Some(SCOPE), &SearchRequest::default())
        .unwrap();
    assert_eq!(page.total_matched, 1);
    let body = page.records[0].get("body").and_then(|v| v.as_str());
    assert_eq!(body, Some("hello"));
}

#[test]
fn idempotent_outcome_discovery_via_commit_key() {
    let engine = Engine::new_in_memory();
    let lmpg = engine.tenant(Some(SCOPE)).unwrap();

    let key = Uuid::now_v7();
    let _committed_ordinal = simulate_commit_with_key(&lmpg, key).expect("keyed commit");

    assert!(matches!(
        rushdb_tx::discover_outcome(&engine.store, key),
        rushdb_tx::KnownOutcome::Committed(_)
    ));

    // Unknown key reports never-proposed rather than guessing success.
    assert!(matches!(
        rushdb_tx::discover_outcome(&engine.store, Uuid::now_v7()),
        rushdb_tx::KnownOutcome::NeverProposed
    ));
    let _ = IsolationMode::Snapshot;
    let _: Result<(), PlanError> = Ok(());
}

#[derive(Serialize, Deserialize)]
struct CommitPayload {
    ordinal: u64,
}

fn simulate_commit_with_key(lmpg: &rushdb_lmpg::Lmpg, commit_key: Uuid) -> Result<u64, String> {
    let commit_result = lmpg.store.commit(commit_key, |_ci, txn| {
        use rushdb_types::command::Mutation;
        let tenant = lmpg.tenant;
        let mut pl = rushdb_lmpg::plan::Planner::new(tenant, txn);
        let rid = *Uuid::now_v7().as_bytes();
        let ordinal = pl.next_alloc("record").map_err(|e| e.to_string())?;
        pl.mutations.push(Mutation::CreateRecordHeader {
            record_id: rid,
            ordinal,
            label: "KEYED".into(),
        });
        let mutations = std::mem::take(&mut pl.mutations);
        let preconditions = std::mem::take(&mut pl.preconditions);
        let mut batch = rushdb_types::command::CommitBatch {
            command_version: rushdb_types::command::COMMAND_VERSION,
            tenant,
            transaction_id: String::new(),
            commit_key,
            start_index: 0,
            isolation: IsolationMode::Snapshot,
            preconditions,
            mutations,
        };
        batch.commit_key = commit_key;
        let payload = CommitPayload { ordinal };
        Ok((batch, payload))
    });
    match commit_result {
        Ok(rushdb_storage::CommitOutcome::Committed { commit_index, .. }) => Ok(commit_index),
        Ok(rushdb_storage::CommitOutcome::Conflicted(reason)) => Err(reason),
        Err(e) => Err(e.to_string()),
    }
}
