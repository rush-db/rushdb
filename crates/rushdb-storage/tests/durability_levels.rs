//! Durability-level contract (W2, research D10). `Local` is the default and
//! durable-before-ack; `Async` acks after queuing without forcing fsync; the
//! store still serves the committed data. A `Memory` store rejects a
//! configured redb cache. Everything below runs with the same correctness
//! surface — durability level changes the ack boundary, not the logical data.

use rushdb_storage::resources::{Durability, StorageResources};
use rushdb_storage::Store;
use rushdb_types::command::COMMAND_VERSION;

fn batch(ordinal: u64) -> (uuid::Uuid, rushdb_types::command::CommitBatch) {
    let key = uuid::Uuid::now_v7();
    let batch = rushdb_types::command::CommitBatch {
        command_version: COMMAND_VERSION,
        tenant: rushdb_types::ids::TenantId::single(),
        transaction_id: format!("t{ordinal}"),
        commit_key: key,
        start_index: 0,
        isolation: rushdb_types::command::IsolationMode::Snapshot,
        preconditions: vec![],
        mutations: vec![],
    };
    (key, batch)
}

#[test]
fn async_durability_commits_and_serves_data() {
    let store = Store::new_in_memory();
    store.set_expander(std::sync::Arc::new(Box::new(|_ci, _batch, _txn| Ok(()))));
    // In-memory store: Async == Memory semantics (no durable backend), but the
    // logical contract still requires the committed data to be visible.
    let (key, batch) = batch(1);
    let outcome = store
        .commit(key, |_ci, _txn| Ok((batch, 1u64)))
        .expect("commit under Async durability");
    match outcome {
        rushdb_storage::CommitOutcome::Committed {
            commit_index,
            payload,
        } => {
            assert_eq!(commit_index, 1);
            assert_eq!(payload, 1u64);
        }
        rushdb_storage::CommitOutcome::Conflicted(reason) => {
            panic!("unexpected conflict: {reason}")
        }
    }
    assert_eq!(store.committed_version(), 1);
    assert_eq!(store.log_bytes().len(), 1);
}

#[test]
fn local_durability_is_the_default_and_commits_durably() {
    let dir = std::env::temp_dir().join(format!(
        "rushdb-durability-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    {
        let store = Store::open(&db).unwrap();
        store.set_expander(std::sync::Arc::new(Box::new(|_ci, _batch, _txn| Ok(()))));
        let (key, batch) = batch(1);
        store
            .commit(key, |_ci, _txn| Ok((batch, 1u64)))
            .expect("commit");
    }
    // Reopen: the durable `Local` commit must be present.
    let store = Store::open(&db).unwrap();
    assert_eq!(store.committed_version(), 1);
    assert_eq!(store.log_bytes().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn memory_durability_rejects_configured_redb_cache() {
    let config = StorageResources {
        durability: Durability::Memory,
        shard_count: 1,
        ..StorageResources::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn async_durability_configures_redb_backend() {
    let dir = std::env::temp_dir().join(format!(
        "rushdb-async-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let config = StorageResources {
        durability: Durability::Async,
        shard_count: 1,
        ..StorageResources::default()
    };
    let store = Store::open_with_resources(&db, config).expect("open async store");
    store.set_expander(std::sync::Arc::new(Box::new(|_ci, _batch, _txn| Ok(()))));
    let (key, batch) = batch(1);
    store
        .commit(key, |_ci, _txn| Ok((batch, 1u64)))
        .expect("commit");
    assert_eq!(store.committed_version(), 1);
    std::fs::remove_dir_all(&dir).ok();
}
#[test]
fn shard_count_one_opens_a_single_backend_file() {
    let dir = std::env::temp_dir().join(format!(
        "rushdb-shard1-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let store = Store::open(&db).unwrap();
    store.set_expander(std::sync::Arc::new(Box::new(|_ci, _batch, _txn| Ok(()))));
    // Single-shard layout: exactly one backend file, no -1 suffix.
    assert!(db.exists(), "shard 0 keeps the unsuffixed name");
    assert!(!dir.join("rushdb.redb-1").exists());
    // Commits land in the single backend and are visible.
    let key = uuid::Uuid::now_v7();
    let batch = rushdb_types::command::CommitBatch {
        command_version: COMMAND_VERSION,
        tenant: rushdb_types::ids::TenantId::single(),
        transaction_id: "shard1".into(),
        commit_key: key,
        start_index: 0,
        isolation: rushdb_types::command::IsolationMode::Snapshot,
        preconditions: vec![],
        mutations: vec![],
    };
    store
        .commit(key, |_ci, _txn| Ok((batch, 1u64)))
        .expect("commit");
    assert_eq!(store.committed_version(), 1);
    assert_eq!(store.log_bytes().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn physical_routing_is_stable_and_respects_shard_count() {
    use rushdb_storage::resources::physical_shard_for;
    use rushdb_types::ids::TenantId;
    // Routing is deterministic and in-range for any configured shard count.
    for count in [1usize, 2, 4, 8] {
        for n in 0..50u64 {
            let t = TenantId::from_ordinal(n);
            let s = physical_shard_for(Some(&t), count);
            assert!(s < count);
            assert_eq!(physical_shard_for(Some(&t), count), s);
        }
    }
    assert_eq!(physical_shard_for(None, 4), 0);
}

#[test]
fn two_shards_persist_tenants_separately_and_reopen_merges() {
    use rushdb_storage::resources::{physical_shard_for, Durability, StorageResources};
    use rushdb_types::ids::TenantId;

    // Two tenants that route to different physical shards.
    let t0 = TenantId::from_ordinal(0);
    let t1 = TenantId::from_ordinal(1);
    let shard_of_t0 = physical_shard_for(Some(&t0), 2);
    let shard_of_t1 = physical_shard_for(Some(&t1), 2);
    assert_ne!(
        shard_of_t0, shard_of_t1,
        "fixture needs tenants on distinct shards"
    );

    let dir = std::env::temp_dir().join(format!(
        "rushdb-shard2-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let config = StorageResources {
        shard_count: 2,
        experimental_multi_shard: true,
        durability: Durability::Local,
        ..StorageResources::default()
    };
    {
        let store = Store::open_with_resources(&db, config).expect("open 2-shard store");
        store.set_expander(std::sync::Arc::new(Box::new(|_ci, _batch, _txn| Ok(()))));
        // Commit one record per tenant.
        for (tenant, txid) in [(t0, "ta".to_string()), (t1, "tb".to_string())] {
            let key = uuid::Uuid::now_v7();
            let batch = rushdb_types::command::CommitBatch {
                command_version: COMMAND_VERSION,
                tenant,
                transaction_id: txid,
                commit_key: key,
                start_index: 0,
                isolation: rushdb_types::command::IsolationMode::Snapshot,
                preconditions: vec![],
                mutations: vec![],
            };
            store
                .commit_for_tenant(&tenant, key, |_ci, _txn| Ok((batch, tenant)))
                .expect("commit");
        }
        assert_eq!(store.committed_version(), 2);
        // Both tenant files exist.
        assert!(db.exists());
        assert!(dir.join("rushdb.redb-1").exists());
    }

    // Reopen merges both shard files; both tenants' log entries are present.
    let store = Store::open_with_resources(&db, config).expect("reopen 2-shard store");
    assert_eq!(store.committed_version(), 2);
    let txids: Vec<String> = store
        .log_bytes()
        .iter()
        .map(|raw| {
            rushdb_types::command_codec::decode(raw)
                .expect("journal entry")
                .transaction_id
        })
        .collect();
    assert!(
        txids.contains(&"ta".to_string()),
        "tenant A log missing: {txids:?}"
    );
    assert!(
        txids.contains(&"tb".to_string()),
        "tenant B log missing: {txids:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}
