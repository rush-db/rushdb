//! Multi-operation batch tests for query-language mutation adapters:
//! `run_batch` must apply every staged op atomically and in order.

use std::sync::Arc;

use rushdb_lmpg::reader::try_access;
use rushdb_lmpg::{BatchOp, BatchTx, FieldChange, Lmpg};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;
use uuid::Uuid;

#[test]
fn relationship_can_reference_records_created_in_the_same_atomic_batch() {
    let store = Arc::new(Store::new_in_memory());
    let graph = Lmpg::new(store, TenantId::from_ordinal(7));
    let source_id = *Uuid::now_v7().as_bytes();
    let target_id = *Uuid::now_v7().as_bytes();

    let committed = graph
        .run_batch(BatchTx {
            ops: vec![
                BatchOp::CreateRecord {
                    label: "Person".into(),
                    explicit_id: Some(source_id),
                    fields: vec![("name".into(), FieldChange::Set(json!("Alice")))],
                },
                BatchOp::CreateRecord {
                    label: "Person".into(),
                    explicit_id: Some(target_id),
                    fields: vec![("name".into(), FieldChange::Set(json!("Bob")))],
                },
                BatchOp::CreateRelationshipById {
                    rel_type: "KNOWS".into(),
                    source_id,
                    target_id,
                    fields: vec![("since".into(), FieldChange::Set(json!(2024)))],
                },
            ],
        })
        .unwrap();

    assert_eq!(committed.created_records.len(), 2);
    assert_eq!(committed.created_relationships.len(), 1);
    let snapshot = graph.store.read_view().unwrap();
    let relationship = try_access::relationship_by_ord(
        &snapshot,
        graph.tenant,
        committed.created_relationships[0].ordinal,
    )
    .unwrap();
    let relationship = relationship.unwrap();
    assert_eq!(relationship.source_id, source_id);
    assert_eq!(relationship.target_id, target_id);
}

#[test]
fn derived_batch_rejects_a_stale_snapshot_version() {
    let store = Arc::new(Store::new_in_memory());
    let graph = Lmpg::new(store, TenantId::from_ordinal(9));
    let snapshot_version = graph.store.committed_version();

    graph
        .create_record(
            "Person",
            None,
            vec![("name".into(), FieldChange::Set(json!("concurrent")))],
        )
        .unwrap();

    let result = graph.run_batch_at_version(
        BatchTx {
            ops: vec![BatchOp::CreateRecord {
                label: "Person".into(),
                explicit_id: None,
                fields: vec![("name".into(), FieldChange::Set(json!("stale")))],
            }],
        },
        snapshot_version,
    );
    assert!(result.is_err());
    assert_eq!(
        try_access::label_members(&graph.store.read_view().unwrap(), graph.tenant, "Person")
            .unwrap()
            .len(),
        1
    );
}
