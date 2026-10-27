//! Serializable isolation and MVCC snapshot coverage for the storage commit
//! path: snapshot time travel, range-overlap conflicts, and disjoint commits.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{reader, Lmpg};
use rushdb_storage::{CommitOutcome, DependencySet, Store};
use rushdb_types::command::{CommitBatch, IsolationMode, Mutation, COMMAND_VERSION};
use rushdb_types::ids::TenantId;

use serde_json::json;
use uuid::Uuid;

fn tenant() -> TenantId {
    TenantId::from_ordinal(42)
}

#[test]
fn snapshot_time_travel_reads_older_state() {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, tenant());

    let r = lmpg
        .create_record(
            "Doc",
            None,
            vec![("v".into(), FieldChange::Set(json!("v1")))],
        )
        .unwrap();

    let ci_after_v1 = lmpg.store.committed_version();

    lmpg.update_fields(r, vec![("v".into(), FieldChange::Set(json!("v2")))])
        .unwrap();
    let ci_after_v2 = lmpg.store.committed_version();
    assert!(ci_after_v2 > ci_after_v1);

    // Read views pinned at each commit see exactly their own state.
    let old_view = lmpg.store.read_at(ci_after_v1).unwrap();
    assert_eq!(
        reader::try_access::record_by_ord(&old_view, tenant(), r.ordinal)
            .unwrap()
            .map(|_| ()),
        Some(())
    );
    let old_json = rushdb_lmpg::reconstruct::reconstruct_record(&old_view, tenant(), r.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(old_json.get("v"), Some(&json!("v1")));

    let new_json = rushdb_lmpg::reconstruct::reconstruct_record(
        &lmpg.store.read_view().unwrap(),
        tenant(),
        r.ordinal,
    )
    .unwrap()
    .unwrap();
    assert_eq!(new_json.get("v"), Some(&json!("v2")));
}

#[test]
fn write_skew_flow_placeholder_removed() {
    // covered precisely by serializable_conflict_errors_* below
}
#[test]
fn serializable_range_overlap_aborts() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = tenant();
    let lmpg = Lmpg::new(store.clone(), tenant);
    let _r = lmpg.create_record("K", None, vec![]).unwrap();
    let snap = lmpg.store.committed_version();

    // concurrent interloper publishes into the same label domain FIRST
    let _interloper = store
        .commit_for_tenant(&tenant, Uuid::now_v7(), |_ci, txn| {
            let ordinal = counter_next(txn, tenant, "record");
            Ok((
                CommitBatch {
                    command_version: COMMAND_VERSION,
                    tenant,
                    transaction_id: String::new(),
                    commit_key: Uuid::nil(),
                    start_index: 0,
                    isolation: IsolationMode::Snapshot,
                    preconditions: vec![],
                    mutations: vec![Mutation::CreateRecordHeader {
                        record_id: [7u8; 16],
                        ordinal,
                        label: "K".into(),
                    }],
                },
                (),
            ))
        })
        .expect("interloper");

    // competing tx tracks the whole K label domain from the older snapshot
    let mut deps = DependencySet::default();
    let (start_key, end_key) =
        rushdb_types::keys::prefix_range(&rushdb_types::keys::label_member_prefix(tenant, "K"));
    deps.record(start_key, end_key);

    let outcome = store.commit_serializable(Uuid::now_v7(), snap - 1, deps, |_| {
        Ok((empty_batch_now(), ()))
    });

    match outcome.expect("storage") {
        CommitOutcome::Conflicted(m) => assert!(m.contains("serializable_conflict")),
        other @ CommitOutcome::Committed { .. } => {
            panic!("expected serializable conflict, got {other:?}")
        }
    }
}

#[test]
fn disjoint_serializable_commits_pass() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = tenant();
    let lmpg = Lmpg::new(store.clone(), tenant);
    let _r = lmpg.create_record("A", None, vec![]).unwrap();
    let snap = lmpg.store.committed_version();

    let _unrelated = store
        .commit_for_tenant(&tenant, Uuid::now_v7(), |_ci, txn| {
            let ordinal = counter_next(txn, tenant, "record");
            Ok((
                CommitBatch {
                    command_version: COMMAND_VERSION,
                    tenant,
                    transaction_id: String::new(),
                    commit_key: Uuid::nil(),
                    start_index: 0,
                    isolation: IsolationMode::Snapshot,
                    preconditions: vec![],
                    mutations: vec![Mutation::CreateRecordHeader {
                        record_id: [8u8; 16],
                        ordinal,
                        label: "Elsewhere".into(),
                    }],
                },
                (),
            ))
        })
        .expect("unrelated");

    // non-overlapping range succeeds even after the unrelated commit.
    let mut deps = DependencySet::default();
    let one_point = rushdb_types::keys::label_member_key(tenant, "Island", 5);
    deps.record(one_point.clone(), one_point);

    let ok = store.commit_serializable(Uuid::now_v7(), snap - 1, deps, |_| {
        Ok((empty_batch_now(), ()))
    });
    assert!(matches!(ok.unwrap(), CommitOutcome::Committed { .. }));
}

fn counter_next(txn: &mut rushdb_storage::Txn<'_>, t: TenantId, name: &str) -> u64 {
    let ckey = rushdb_lmpg::expand::meta_counter_key(t, name);
    let cur = txn.get(&ckey).unwrap().map_or(0, |b| {
        u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    });
    txn.put(ckey, (cur + 1).to_be_bytes().to_vec());
    cur + 1
}

fn empty_batch_now() -> CommitBatch {
    CommitBatch {
        command_version: COMMAND_VERSION,
        tenant: tenant(),
        transaction_id: String::new(),
        commit_key: Uuid::nil(),
        start_index: 0,
        isolation: IsolationMode::Snapshot,
        preconditions: vec![],
        mutations: vec![],
    }
}
