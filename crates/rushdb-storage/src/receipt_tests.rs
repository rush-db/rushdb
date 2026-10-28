//! Retry receipts answer a repeated commit key for the idempotency
//! window, then expire and are swept in bounded batches.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rushdb_types::command::{CommitBatch, IsolationMode, COMMAND_VERSION};
use rushdb_types::ids::TenantId;
use uuid::Uuid;

use crate::{resources, CommitOutcome, Store};

const WINDOW_SECONDS: u64 = 60;

/// A durable store with a 60 s window and a mock receipt clock.
fn store(name: &str) -> (Store, Arc<AtomicU64>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("rushdb-receipts-{name}-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open_with_resources(
        &dir.join("rushdb.redb"),
        resources::StorageResources {
            idempotency_window_seconds: WINDOW_SECONDS,
            ..Default::default()
        },
    )
    .unwrap();
    store.set_expander(Arc::new(Box::new(|_, batch, txn| {
        txn.put(format!("u/{}", batch.transaction_id).into_bytes(), vec![1]);
        Ok(())
    })));
    let now = Arc::new(AtomicU64::new(1_000_000));
    let clock = Arc::clone(&now);
    store.set_receipt_clock(Arc::new(move || clock.load(Ordering::SeqCst)));
    (store, now, dir)
}

fn commit(store: &Store, commit_key: Uuid, body: &str) -> (u64, String) {
    match store
        .commit(commit_key, |_, _| {
            Ok((
                CommitBatch {
                    command_version: COMMAND_VERSION,
                    tenant: TenantId::single(),
                    transaction_id: body.to_owned(),
                    commit_key,
                    start_index: 0,
                    isolation: IsolationMode::ReadCommitted,
                    preconditions: vec![],
                    mutations: vec![],
                },
                body.to_owned(),
            ))
        })
        .unwrap()
    {
        CommitOutcome::Committed {
            commit_index,
            payload,
        } => (commit_index, payload),
        CommitOutcome::Conflicted(reason) => panic!("conflicted: {reason}"),
    }
}

#[test]
fn retry_within_window_is_exactly_once() {
    let (store, now, dir) = store("within");
    let key = Uuid::now_v7();
    let first = commit(&store, key, "first");
    let version = store.committed_version();
    now.fetch_add(WINDOW_SECONDS * 1000, Ordering::SeqCst); // the window's last instant
                                                            // The retry's own body is never executed: the stored answer comes back.
    assert_eq!(commit(&store, key, "retry"), first);
    assert_eq!(store.committed_version(), version);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn retry_after_window_is_a_new_commit() {
    let (store, now, dir) = store("after");
    let key = Uuid::now_v7();
    let (first_ci, _) = commit(&store, key, "first");
    now.fetch_add(WINDOW_SECONDS * 1000 + 1, Ordering::SeqCst);
    let (second_ci, payload) = commit(&store, key, "second");
    assert!(second_ci > first_ci);
    assert_eq!(payload, "second");
    // The new receipt answers retries for its own window, and the sweep of
    // the old one must not take the new one with it.
    assert_eq!(store.sweep_receipts(100).unwrap(), 1);
    assert_eq!(
        commit(&store, key, "third"),
        (second_ci, "second".to_owned())
    );
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn receipt_sweep_is_bounded_per_tick() {
    let (store, now, dir) = store("sweep");
    for index in 0..25 {
        commit(&store, Uuid::now_v7(), &format!("old-{index}"));
    }
    now.fetch_add(10_000, Ordering::SeqCst);
    let fresh = Uuid::now_v7();
    let fresh_outcome = commit(&store, fresh, "fresh");
    let rows = |store: &Store| store.storage_stats().unwrap().receipt_keys;
    // Two rows per receipt (the receipt and its expiry index entry).
    assert_eq!(rows(&store), 52);
    // Nothing is past the window yet.
    assert_eq!(store.sweep_receipts(10).unwrap(), 0);
    now.fetch_add(WINDOW_SECONDS * 1000 - 5_000, Ordering::SeqCst);
    // The 25 old receipts are expired, the fresh one is not; each call
    // removes at most `max_keys`.
    assert_eq!(store.sweep_receipts(10).unwrap(), 10);
    assert_eq!(store.sweep_receipts(10).unwrap(), 10);
    assert_eq!(store.sweep_receipts(10).unwrap(), 5);
    assert_eq!(store.sweep_receipts(10).unwrap(), 0);
    assert_eq!(rows(&store), 2);
    assert_eq!(commit(&store, fresh, "retry"), fresh_outcome);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn receipt_sweep_drains_a_backlog_within_its_budget() {
    let (store, now, dir) = store("sweep-drain");
    for index in 0..25 {
        commit(&store, Uuid::now_v7(), &format!("old-{index}"));
    }
    now.fetch_add(WINDOW_SECONDS * 1000 + 1_000, Ordering::SeqCst);
    let fresh = Uuid::now_v7();
    commit(&store, fresh, "fresh");
    // Batches of 10 until the 25 expired receipts are gone; the fresh one
    // stays.
    let budget = std::time::Duration::from_secs(60);
    assert_eq!(store.sweep_expired_receipts(10, budget).unwrap(), 25);
    assert_eq!(store.storage_stats().unwrap().receipt_keys, 2);
    assert_eq!(store.sweep_expired_receipts(10, budget).unwrap(), 0);
    // A spent budget stops after one batch.
    for index in 0..25 {
        commit(&store, Uuid::now_v7(), &format!("older-{index}"));
    }
    now.fetch_add(WINDOW_SECONDS * 1000 + 1_000, Ordering::SeqCst);
    assert_eq!(
        store
            .sweep_expired_receipts(10, std::time::Duration::ZERO)
            .unwrap(),
        10
    );
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}
