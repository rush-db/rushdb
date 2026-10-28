//! Cross-request transaction contract (plan document 07): every outcome in
//! the set `committed / aborted / expired / unknown_retryable` is exercised,
//! commit retries are idempotent, and rollback durably records an aborted
//! outcome so a later commit never reports false success.

use std::sync::Arc;
use std::time::Duration;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::reader::try_access;
use rushdb_lmpg::BatchOp;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_tx::{TxCoordinator, TxOutcome};
use rushdb_types::ids::TenantId;

fn setup(ttl: Duration) -> (Lmpg, TxCoordinator) {
    let store = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(61);
    let lmpg = Lmpg::new(store.clone(), tenant);
    let coord = TxCoordinator::new(store, tenant, ttl).unwrap();
    (lmpg, coord)
}

fn create_op(label: &str, value: i64) -> BatchOp {
    BatchOp::CreateRecord {
        label: label.to_string(),
        explicit_id: None,
        fields: vec![("v".into(), FieldChange::Set(serde_json::json!(value)))],
    }
}

#[test]
fn multi_op_transaction_commits_atomically() {
    let (lmpg, coord) = setup(Duration::from_secs(60));
    let tx = coord.begin().unwrap();
    coord.stage(&tx, create_op("A", 1)).unwrap();
    coord.stage(&tx, create_op("B", 2)).unwrap();
    coord.stage(&tx, create_op("C", 3)).unwrap();

    match coord.commit(&tx) {
        TxOutcome::Committed(ci) => assert!(ci >= 1, "commit index is the batch's index"),
        other => panic!("expected committed, got {other:?}"),
    }

    let snap = lmpg.store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&snap, lmpg.tenant, "A")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        try_access::label_members(&snap, lmpg.tenant, "B")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        try_access::label_members(&snap, lmpg.tenant, "C")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn commit_retry_is_idempotent_and_discoverable() {
    let (lmpg, coord) = setup(Duration::from_secs(60));
    let tx = coord.begin().unwrap();
    coord.stage(&tx, create_op("A", 1)).unwrap();

    let first = coord.commit(&tx);
    let TxOutcome::Committed(first_ci) = first else {
        panic!("first commit failed: {first:?}")
    };

    // a retry with the SAME id must discover the durable outcome and not
    // re-execute (no duplicate records).
    let retry = coord.commit(&tx);
    assert_eq!(
        retry,
        TxOutcome::Committed(first_ci),
        "retry returns same index"
    );

    let snap = lmpg.store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&snap, lmpg.tenant, "A")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn rollback_records_aborted_outcome() {
    let (lmpg, coord) = setup(Duration::from_secs(60));
    let tx = coord.begin().unwrap();
    coord.stage(&tx, create_op("A", 1)).unwrap();
    coord.rollback(&tx).unwrap();

    // a later commit attempt reports the durable aborted outcome
    match coord.commit(&tx) {
        TxOutcome::Aborted(_) => {}
        other => panic!("expected aborted, got {other:?}"),
    }
    let snap = lmpg.store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&snap, lmpg.tenant, "A")
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn expired_transaction_never_commits() {
    let (lmpg, coord) = setup(Duration::from_millis(50));
    let tx = coord.begin().unwrap();
    coord.stage(&tx, create_op("A", 1)).unwrap();
    std::thread::sleep(Duration::from_millis(80));

    match coord.commit(&tx) {
        TxOutcome::Expired => {}
        other => panic!("expected expired, got {other:?}"),
    }
    let snap = lmpg.store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&snap, lmpg.tenant, "A")
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn unknown_id_is_retryable_not_success() {
    let (_lmpg, coord) = setup(Duration::from_secs(60));
    assert_eq!(coord.commit("never-begun"), TxOutcome::UnknownRetryable);
}

#[test]
fn conflicting_transaction_reports_aborted() {
    let (_lmpg, coord) = setup(Duration::from_secs(60));
    // A batch whose create_relationship references a nonexistent source
    // fails the source-endpoint precondition -> durable aborted outcome.
    let tx = coord.begin().unwrap();
    coord
        .stage(
            &tx,
            BatchOp::CreateRelationship {
                rel_type: "R".to_string(),
                source: rushdb_lmpg::RecordRef {
                    record_id: [0x01; 16],
                    ordinal: 0,
                },
                target: rushdb_lmpg::RecordRef {
                    record_id: [0x02; 16],
                    ordinal: 1,
                },
                fields: vec![],
            },
        )
        .unwrap();
    match coord.commit(&tx) {
        TxOutcome::Aborted(reason) => assert!(reason.contains("record"), "{reason}"),
        other => panic!("expected aborted, got {other:?}"),
    }
    // A retry discovers the durable aborted outcome (not success).
    assert!(matches!(coord.commit(&tx), TxOutcome::Aborted(_)));
}
