//! Disk planning and native read/write parity tests: fallible reconstruction
//! and scan projection must preserve payloads across store reopen.

use rushdb_lmpg::{plan::FieldChange, reader::keys_ext, Lmpg};
use rushdb_storage::{ScanBounds, ScanProjection, Store};
use rushdb_types::ids::TenantId;
use serde_json::json;
use std::sync::Arc;

#[test]
fn fallible_reconstruction_preserves_record_and_relationship_payloads_after_reopen() {
    let path = rushdb_storage::TempPath::store("native-rebuild");
    let tenant = TenantId::from_ordinal(66);
    let store = Arc::new(Store::open(&path).unwrap());
    let lmpg = Lmpg::new(store, tenant);
    let document = json!({"name": "Ada", "score": 99, "ratio": 0.5, "active": true,
        "nickname": null, "tags": ["a", "b", "a"], "nums": [1, 2]});
    let root = rushdb_lmpg::import::import_document(&lmpg, "Person", &document).unwrap();
    let other = lmpg.create_record("Person", None, vec![]).unwrap();
    let rel = lmpg
        .create_relationship(
            "KNOWS",
            root,
            other,
            vec![
                ("since".into(), FieldChange::Set(json!(2001))),
                ("tier".into(), FieldChange::ExplicitNull),
                ("tags".into(), FieldChange::Set(json!(["work", "friend"]))),
            ],
        )
        .unwrap();
    let expected_record = rushdb_lmpg::reconstruct::reconstruct_record(
        &lmpg.store.read_view().unwrap(),
        tenant,
        root.ordinal,
    )
    .unwrap()
    .unwrap();
    let expected_rel = rushdb_lmpg::reconstruct::reconstruct_relationship(
        &lmpg.store.read_view().unwrap(),
        tenant,
        rel.ordinal,
    )
    .unwrap()
    .unwrap();
    drop(lmpg);
    let reopened = Store::open(&path).unwrap();
    let view = reopened.read_view().unwrap();
    let actual = rushdb_lmpg::reconstruct::reconstruct_record(&view, tenant, root.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(actual, expected_record);
    for (name, value) in document.as_object().unwrap() {
        assert_eq!(&actual[name], value);
    }
    assert_eq!(
        rushdb_lmpg::reconstruct::reconstruct_relationship(&view, tenant, rel.ordinal)
            .unwrap()
            .unwrap(),
        expected_rel
    );
    drop(view);
    drop(reopened);
    rushdb_storage::remove_store(&path).unwrap();
}

#[test]
fn native_planning_updates_allocators_and_cascades_survive_reopen() {
    let path = rushdb_storage::TempPath::store("native-lmpg");
    let store = Arc::new(Store::open(&path).unwrap());
    let tenant = TenantId::from_ordinal(66);
    let lmpg = Lmpg::new(store.clone(), tenant);
    let first = lmpg
        .create_record(
            "User",
            None,
            vec![("name".into(), FieldChange::Set(json!("first")))],
        )
        .unwrap();
    lmpg.update_fields(
        first,
        vec![("name".into(), FieldChange::Set(json!("updated")))],
    )
    .unwrap();
    let second = lmpg
        .create_record(
            "User",
            None,
            vec![("name".into(), FieldChange::Set(json!("second")))],
        )
        .unwrap();
    assert_eq!(second.ordinal, first.ordinal + 1);
    let relation = lmpg
        .create_relationship("KNOWS", first, second, vec![])
        .unwrap();
    let before = store.committed_version();
    lmpg.delete_record(first).unwrap();
    assert!(store
        .read_view()
        .unwrap()
        .get(&rushdb_types::keys::record_id_key(tenant, &first.record_id))
        .unwrap()
        .is_none());
    assert!(store
        .read_view()
        .unwrap()
        .get(&keys_ext::rel_row_key(tenant, &relation.relationship_id))
        .unwrap()
        .is_none());
    drop(lmpg);
    drop(store);
    let reopened = Store::open(&path).unwrap();
    let current = reopened.read_view().unwrap();
    assert!(current
        .get(&rushdb_types::keys::record_id_key(
            tenant,
            &second.record_id
        ))
        .unwrap()
        .is_some());
    assert!(reopened
        .read_at(before)
        .unwrap()
        .get(&rushdb_types::keys::record_id_key(tenant, &first.record_id))
        .unwrap()
        .is_some());
    let mut cursor = current
        .scan(
            ScanBounds::prefix(&rushdb_types::keys::label_member_prefix(tenant, "User")),
            ScanProjection::Keys,
        )
        .unwrap();
    assert_eq!(cursor.next_batch(10, 1024).unwrap().rows.len(), 1);
    drop(cursor);
    drop(current);
    drop(reopened);
    rushdb_storage::remove_store(&path).unwrap();
}

#[test]
fn malformed_durable_allocator_rejects_planning_without_allocating_duplicate_ids() {
    use rushdb_types::command::{CommitBatch, IsolationMode, Mutation, COMMAND_VERSION};
    let path = rushdb_storage::TempPath::store("corrupt-allocator");
    let store = Arc::new(Store::open(&path).unwrap());
    let tenant = TenantId::from_ordinal(66);
    let lmpg = Lmpg::new(store.clone(), tenant);
    let commit_key = uuid::Uuid::now_v7();
    // Deliberately malformed canonical metadata, injected through the real
    // durable command path. The chosen tenant's binary key is valid UTF-8.
    let counter =
        String::from_utf8(rushdb_lmpg::expand::meta_counter_key(tenant, "record")).unwrap();
    store
        .commit(commit_key, |_, _| {
            Ok((
                CommitBatch {
                    command_version: COMMAND_VERSION,
                    tenant,
                    transaction_id: String::new(),
                    commit_key,
                    start_index: 0,
                    isolation: IsolationMode::ReadCommitted,
                    preconditions: vec![],
                    mutations: vec![Mutation::SetMetaBlob {
                        key: counter,
                        value: vec![1],
                    }],
                },
                (),
            ))
        })
        .unwrap();
    let error = lmpg.create_record("User", None, vec![]).unwrap_err();
    assert!(error.to_string().contains("invalid u64 metadata"));
    assert_eq!(store.committed_version(), 1);
    let view = store.read_view().unwrap();
    let mut scan = view
        .scan(
            ScanBounds::prefix(&rushdb_types::keys::label_member_prefix(tenant, "User")),
            ScanProjection::Keys,
        )
        .unwrap();
    assert!(scan.next_batch(10, 1024).unwrap().rows.is_empty());
    drop(scan);
    drop(view);
    drop(lmpg);
    drop(store);
    rushdb_storage::remove_store(&path).unwrap();
}
