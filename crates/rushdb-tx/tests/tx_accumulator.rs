//! Storage-level coverage for the cross-service transaction accumulator:
//! read-your-writes within a tx, atomic commit, conflict detection against
//! concurrent writers (read-set and write-set), and rollback semantics.

use std::sync::Arc;

use rushdb_lmpg::expand::meta_counter_key;
use rushdb_lmpg::Lmpg;
use rushdb_storage::{Store, TxAccumulator};
use rushdb_types::command::{CommitBatch, IsolationMode, Mutation, COMMAND_VERSION};
use rushdb_types::ids::TenantId;
use uuid::Uuid;

fn tenant() -> TenantId {
    TenantId::from_ordinal(42)
}

/// Creating an Lmpg installs the deterministic expander on the store (a
/// prerequisite for `tx_apply`/`tx_commit` expansion).
fn with_expander(store: &Arc<Store>, tenant: TenantId) -> Lmpg {
    Lmpg::new(Arc::clone(store), tenant)
}

fn counter_next(txn: &mut rushdb_storage::Txn<'_>, t: TenantId, name: &str) -> u64 {
    let ckey = meta_counter_key(t, name);
    let cur = txn.get(&ckey).unwrap().map_or(0, |b| {
        u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    });
    txn.put(ckey, (cur + 1).to_be_bytes().to_vec());
    cur + 1
}

fn create_record_batch(
    tenant: TenantId,
    txn: &mut rushdb_storage::Txn<'_>,
    record_id: [u8; 16],
    label: &str,
) -> CommitBatch {
    let ordinal = counter_next(txn, tenant, "record");
    CommitBatch {
        command_version: COMMAND_VERSION,
        tenant,
        transaction_id: String::new(),
        commit_key: Uuid::nil(),
        start_index: 0,
        isolation: IsolationMode::Snapshot,
        preconditions: vec![],
        mutations: vec![Mutation::CreateRecordHeader {
            record_id,
            ordinal,
            label: label.into(),
        }],
    }
}

#[test]
fn tx_apply_does_not_commit_and_reads_own_writes() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = tenant();
    let _lmpg = with_expander(&store, tenant);
    let base_ci = store.committed_version();
    let mut tx = TxAccumulator::new(tenant, base_ci);

    // Statement 1 allocates the record counter and creates a header.
    store
        .tx_apply(&mut tx, Uuid::now_v7(), |_ci, txn| {
            Ok((create_record_batch(tenant, txn, [1u8; 16], "Doc"), ()))
        })
        .unwrap();
    assert_eq!(
        store.committed_version(),
        base_ci,
        "tx_apply must not commit"
    );

    // Statement 2 re-reads the counter: it must see statement 1's bump
    // (read-your-writes through the tx overlay).
    let seen = store
        .tx_apply(&mut tx, Uuid::now_v7(), |_ci, txn| {
            let cur = txn
                .get(&meta_counter_key(tenant, "record"))
                .unwrap()
                .map_or(0, |b| u64::from_be_bytes(b.as_slice().try_into().unwrap()));
            Ok((create_record_batch(tenant, txn, [2u8; 16], "Doc"), cur))
        })
        .unwrap();
    assert!(
        seen >= 1,
        "counter read inside tx must see its own bump, got {seen}"
    );
    assert_eq!(store.committed_version(), base_ci);

    // Commit publishes everything in ONE atomic commit.
    let ci = store.tx_commit(Uuid::now_v7(), &tx).unwrap();
    assert_eq!(ci, base_ci + 1);
    let snap = store.read_view().unwrap();
    assert!(
        rushdb_lmpg::reader::try_access::record_by_ord(&snap, tenant, 1)
            .unwrap()
            .is_some()
    );
    // Statement 1's expansion bumps the record counter to 2, so statement 2
    // allocates ordinal 3.
    assert!(
        rushdb_lmpg::reader::try_access::record_by_ord(&snap, tenant, 3)
            .unwrap()
            .is_some()
    );
}

#[test]
fn disjoint_tx_and_autocommit_both_commit() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = tenant();
    let _lmpg = with_expander(&store, tenant);
    let base_ci = store.committed_version();

    // Concurrent autocommit writes meta key "unrelated" (disjoint from the tx).
    store
        .commit_for_tenant(&tenant, Uuid::now_v7(), |_ci, _txn| {
            use rushdb_types::command::Mutation;
            let mut batch = CommitBatch {
                command_version: COMMAND_VERSION,
                tenant,
                transaction_id: String::new(),
                commit_key: Uuid::nil(),
                start_index: 0,
                isolation: IsolationMode::Snapshot,
                preconditions: vec![],
                mutations: vec![],
            };
            batch.mutations.push(Mutation::SetMetaValue {
                key: "unrelated".into(),
                value: 1,
            });
            Ok((batch, ()))
        })
        .unwrap();

    let mut tx = TxAccumulator::new(tenant, base_ci);
    store
        .tx_apply(&mut tx, Uuid::now_v7(), |_ci, _txn| {
            use rushdb_types::command::Mutation;
            let mut batch = CommitBatch {
                command_version: COMMAND_VERSION,
                tenant,
                transaction_id: String::new(),
                commit_key: Uuid::nil(),
                start_index: 0,
                isolation: IsolationMode::Snapshot,
                preconditions: vec![],
                mutations: vec![],
            };
            batch.mutations.push(Mutation::SetMetaValue {
                key: "island".into(),
                value: 2,
            });
            Ok((batch, ()))
        })
        .unwrap();
    let ci = store.tx_commit(Uuid::now_v7(), &tx).unwrap();
    assert!(ci > base_ci);
}

#[test]
fn a_transaction_and_a_concurrent_create_get_distinct_ordinals_and_both_commit() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = tenant();
    let lmpg = with_expander(&store, tenant);
    let base_ci = store.committed_version();

    // The transaction plans a record: its ordinal is reserved, not read from
    // the counter as a dependency.
    let mut tx = TxAccumulator::new(tenant, base_ci);
    let staged = lmpg
        .create_record_tx("Doc", None, vec![], Some(&mut tx))
        .unwrap();
    // A concurrent autocommit creates a record before the transaction
    // commits. It must not reuse the reserved ordinal.
    let committed = lmpg.create_record("Doc", None, vec![]).unwrap();
    assert_ne!(staged.ordinal, committed.ordinal);

    // Both advanced the same counter; that is not a conflict.
    store.tx_commit(Uuid::now_v7(), &tx).unwrap();
    let snap = store.read_view().unwrap();
    for ordinal in [staged.ordinal, committed.ordinal] {
        assert!(
            rushdb_lmpg::reader::try_access::record_by_ord(&snap, tenant, ordinal)
                .unwrap()
                .is_some()
        );
    }
    let counter = snap
        .get(&meta_counter_key(tenant, "record"))
        .unwrap()
        .unwrap();
    assert_eq!(
        u64::from_be_bytes(counter.try_into().unwrap()),
        staged.ordinal.max(committed.ordinal) + 1,
        "the counter covers both ordinals"
    );
}

#[test]
fn rollback_discards_without_committing() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = tenant();
    let _lmpg = with_expander(&store, tenant);
    let base_ci = store.committed_version();
    let mut tx = TxAccumulator::new(tenant, base_ci);
    store
        .tx_apply(&mut tx, Uuid::now_v7(), |_ci, txn| {
            Ok((create_record_batch(tenant, txn, [4u8; 16], "Doc"), ()))
        })
        .unwrap();
    // Rollback = drop the accumulator; nothing was committed.
    drop(tx);
    assert_eq!(store.committed_version(), base_ci);
    let snap = store.read_view().unwrap();
    assert!(
        rushdb_lmpg::reader::try_access::record_by_ord(&snap, tenant, 0)
            .unwrap()
            .is_none()
    );
}

#[test]
fn expansion_reads_are_retained_for_read_write_conflict_validation() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = tenant();
    let tx_key = Uuid::now_v7();
    let writer_key = Uuid::now_v7();

    // The transaction's expansion only reads `guard`; the later writer is the
    // only command that writes it. This specifically exercises a dependency
    // that is absent from the transaction's output/write set.
    store.set_expander(Arc::new(Box::new(move |_, batch, txn| {
        let _ = txn.get(b"guard").map_err(|e| e.to_string())?;
        if batch.commit_key == writer_key {
            txn.put(b"guard".to_vec(), vec![1]);
        }
        Ok(())
    })));

    let mut tx = TxAccumulator::new(tenant, store.committed_version());
    store
        .tx_apply_batch(
            &mut tx,
            tx_key,
            &CommitBatch {
                command_version: COMMAND_VERSION,
                tenant,
                transaction_id: String::new(),
                commit_key: tx_key,
                start_index: 0,
                isolation: IsolationMode::Snapshot,
                preconditions: vec![],
                mutations: vec![],
            },
        )
        .unwrap();

    // A concurrent command changes exactly the key observed by the pending
    // transaction's expansion.
    store
        .commit(writer_key, |_, _| {
            Ok((
                CommitBatch {
                    command_version: COMMAND_VERSION,
                    tenant,
                    transaction_id: String::new(),
                    commit_key: writer_key,
                    start_index: 0,
                    isolation: IsolationMode::Snapshot,
                    preconditions: vec![],
                    mutations: vec![],
                },
                (),
            ))
        })
        .unwrap();

    let error = store.tx_commit(tx_key, &tx).unwrap_err();
    assert!(
        error.to_string().contains("transaction_conflict"),
        "expansion read must conflict with a later write, got {error}"
    );
}
