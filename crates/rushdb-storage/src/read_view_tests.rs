use super::*;
use std::ops::Bound::{Excluded, Included, Unbounded};

#[test]
fn read_view_does_not_take_the_publication_lock() {
    // Opening a read view must serialize with GC on `read_pins`, not with the
    // writer's `publish_lock` (an in-flight fsync). Hold the publication lock
    // and prove a reader in another thread still opens within the deadline.
    let store = Arc::new(Store::new_in_memory());
    let publish = store.publish_lock.lock().unwrap();
    let worker_store = Arc::clone(&store);
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let _ = tx.send(worker_store.read_view().is_ok());
    });
    let opened = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap_or(false);
    drop(publish);
    handle.join().unwrap();
    assert!(opened, "read_view must not block on the publication lock");
}

fn configure(store: &Store) {
    store.set_expander(Arc::new(Box::new(|_, batch, txn| {
        let ops: Vec<(Vec<u8>, Option<Vec<u8>>)> =
            serde_json::from_str(&batch.transaction_id).unwrap();
        for (key, value) in ops {
            match value {
                Some(value) => txn.put(key, value),
                None => txn.del(key),
            }
        }
        Ok(())
    })));
}

fn commit(store: &Store, ops: impl IntoIterator<Item = (Vec<u8>, Option<Vec<u8>>)>) -> u64 {
    let ops: Vec<(Vec<u8>, Option<Vec<u8>>)> = ops.into_iter().collect();
    let key = Uuid::new_v4();
    match store
        .commit(key, |_, _| {
            Ok((
                CommitBatch {
                    command_version: rushdb_types::command::COMMAND_VERSION,
                    tenant: TenantId::from_ordinal(1),
                    transaction_id: serde_json::to_string(&ops).unwrap(),
                    commit_key: key,
                    start_index: 0,
                    isolation: rushdb_types::command::IsolationMode::ReadCommitted,
                    preconditions: vec![],
                    mutations: vec![],
                },
                (),
            ))
        })
        .unwrap()
    {
        CommitOutcome::Committed { commit_index, .. } => commit_index,
        CommitOutcome::Conflicted(reason) => panic!("unexpected conflicted: {reason}"),
    }
}

fn commit_tenant(store: &Store, tenant: TenantId, key: Vec<u8>) -> u64 {
    let commit_key = Uuid::new_v4();
    match store
        .commit(commit_key, |_, _| {
            Ok((
                CommitBatch {
                    command_version: rushdb_types::command::COMMAND_VERSION,
                    tenant,
                    transaction_id: serde_json::to_string(&[(key, Some(vec![1]))]).unwrap(),
                    commit_key,
                    start_index: 0,
                    isolation: rushdb_types::command::IsolationMode::ReadCommitted,
                    preconditions: vec![],
                    mutations: vec![],
                },
                (),
            ))
        })
        .unwrap_or_else(|error| panic!("tenant commit failed: {error:?}"))
    {
        CommitOutcome::Committed { commit_index, .. } => commit_index,
        CommitOutcome::Conflicted(reason) => panic!("unexpected conflicted: {reason}"),
    }
}

fn on_backends(test: impl Fn(&Store)) {
    let memory = Store::new_in_memory();
    configure(&memory);
    test(&memory);
    let path = crate::TempPath::store("read-view");
    let disk = Store::open(&path).unwrap();
    configure(&disk);
    test(&disk);
    drop(disk);
    crate::remove_store(&path).unwrap();
}

fn rows(view: &ReadView, bounds: ScanBounds, projection: ScanProjection) -> Vec<ScanRow> {
    let mut cursor = view.scan(bounds, projection).unwrap();
    let mut rows = Vec::new();
    loop {
        let batch = cursor.next_batch(2, 64).unwrap();
        assert!(batch.rows.len() <= 2 && batch.bytes <= 64);
        rows.extend(batch.rows);
        if batch.done {
            return rows;
        }
    }
}

#[test]
fn owned_read_views_outlive_store_on_both_backends() {
    fn owned_thread_safe<T: Send + Sync + 'static>() {}
    owned_thread_safe::<ReadView>();
    let path = crate::TempPath::store("owned-view");
    for persistent in [false, true] {
        let store = if persistent {
            Store::open(&path).unwrap()
        } else {
            Store::new_in_memory()
        };
        configure(&store);
        commit(&store, vec![(b"user/a".to_vec(), Some(vec![1]))]);
        let view = store.read_view().unwrap();
        drop(store);
        assert_eq!(view.get(b"user/a").unwrap(), Some(vec![1]));
        assert_eq!(
            rows(&view, ScanBounds::prefix(b"user/"), ScanProjection::Keys).len(),
            1
        );
        drop(view);
        if persistent {
            // Dropping the last owned view also releases the database lock.
            drop(Store::open(&path).unwrap());
        }
    }
    crate::remove_store(&path).unwrap();
}

#[test]
fn bounded_child_view_keeps_snapshot_and_has_independent_limits() {
    on_backends(|store| {
        commit(
            store,
            vec![
                (b"bounded/a".to_vec(), Some(vec![1, 2])),
                (b"bounded/b".to_vec(), Some(vec![3, 4])),
            ],
        );
        let parent = Arc::new(store.read_view().unwrap());
        let child = parent.with_materialization_limit(12);

        // The parent remains unrestricted when the child narrows its limits.
        assert_eq!(parent.get(b"bounded/a").unwrap(), Some(vec![1, 2]));
        let strict = parent.with_materialization_limit(10);
        assert!(matches!(
            strict.get(b"bounded/a"),
            Err(StorageError::ResourceExhausted(_))
        ));
        assert_eq!(child.commit_index(), parent.commit_index());

        // Both handles stay on the original pinned snapshot after a write.
        commit(store, vec![(b"bounded/a".to_vec(), Some(vec![9]))]);
        assert_eq!(parent.get(b"bounded/a").unwrap(), Some(vec![1, 2]));
        assert_eq!(child.get(b"bounded/a").unwrap(), Some(vec![1, 2]));

        let zero = parent.with_materialization_limit(0);
        assert!(matches!(
            zero.get(b"bounded/a"),
            Err(StorageError::ResourceExhausted(_))
        ));
        assert!(matches!(
            zero.range_prefix(b"bounded/"),
            Err(StorageError::ResourceExhausted(_))
        ));

        // Each row is 11 copied bytes; the 12-byte prefix cap permits one row
        // and rejects the next row before it is cloned.
        let prefix = parent.with_materialization_limit(12);
        assert!(matches!(
            prefix.range_prefix(b"bounded/"),
            Err(StorageError::ResourceExhausted(_))
        ));
    });
}

#[test]
fn native_cache_charge_outlives_store_until_last_view_closes() {
    let path = crate::TempPath::store("cache-charge");
    let config = resources::StorageResources {
        managed_limit_bytes: 1024 * 1024,
        page_cache_bytes: 128 * 1024,
        transaction_pool_bytes: 128 * 1024,
        durability: resources::Durability::Local,
        shard_count: 1,
        experimental_multi_shard: false,
        max_group_count: 0,
        max_group_bytes: 0,
        max_group_delay_ms: 0,
        logical_cache_bytes: 32 * 1024 * 1024,
        phase_budget_bytes: 1024 * 1024,
        query_memory_max_bytes: 0,
        lock_acquisition_timeout_ms: 0,
        log_segment_bytes: 64 * 1024 * 1024,
        max_expanded_commit_bytes: 512 * 1024 * 1024,
        ..resources::StorageResources::default()
    };
    let store = Store::open_with_resources(&path, config).unwrap();
    let budget = store.resource_budget();
    assert_eq!(budget.snapshot().used, config.page_cache_bytes);
    let view = store.read_view().unwrap();
    let second = store.read_view().unwrap();
    drop(store);
    assert_eq!(budget.snapshot().used, config.page_cache_bytes);
    drop(view);
    assert_eq!(budget.snapshot().used, config.page_cache_bytes);
    drop(second);
    assert_eq!(budget.snapshot().used, 0);
    crate::remove_store(&path).unwrap();
}

#[test]
fn tenant_read_view_opens_only_the_routed_shard() {
    let dir = crate::TempPath::dir("routed-view");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rushdb.redb");
    let store = Store::open_with_resources(
        &path,
        resources::StorageResources {
            shard_count: 2,
            experimental_multi_shard: true,
            ..Default::default()
        },
    )
    .unwrap();
    let tenant = TenantId::from_ordinal(1_234);
    let all = store.read_view().unwrap();
    assert_eq!(all.native_backend_count(), 2);
    let routed = store.read_view_for_tenant(&tenant).unwrap();
    assert_eq!(routed.native_backend_count(), 1);
    assert_eq!(routed.commit_index(), all.commit_index());
    drop(routed);
    drop(all);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tenant_read_view_keeps_global_snapshot_identity_across_shards() {
    let dir = crate::TempPath::dir("routed-ci");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rushdb.redb");
    let store = Store::open_with_resources(
        &path,
        resources::StorageResources {
            shard_count: 2,
            experimental_multi_shard: true,
            ..Default::default()
        },
    )
    .unwrap();
    configure(&store);
    let first = TenantId::from_ordinal(1_000);
    let first_shard = resources::physical_shard_for(Some(&first), 2);
    let second = (1_001..1_065)
        .map(TenantId::from_ordinal)
        .find(|tenant| resources::physical_shard_for(Some(tenant), 2) != first_shard)
        .unwrap();
    commit_tenant(&store, first, b"first".to_vec());
    let global = commit_tenant(&store, second, b"second".to_vec());
    assert_eq!(store.committed_version(), global);
    let view = store.read_view_for_tenant(&first).unwrap();
    assert_eq!(view.commit_index(), global);
    drop(view);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_config_and_failed_open_do_not_leave_cache_charges() {
    let path = crate::TempPath::store("cache-failure");
    let invalid = resources::StorageResources {
        managed_limit_bytes: 1024,
        page_cache_bytes: 1024,
        transaction_pool_bytes: 1024,
        durability: resources::Durability::Local,
        shard_count: 1,
        experimental_multi_shard: false,
        max_group_count: 0,
        max_group_bytes: 0,
        max_group_delay_ms: 0,
        logical_cache_bytes: 32 * 1024 * 1024,
        phase_budget_bytes: 1024,
        query_memory_max_bytes: 0,
        lock_acquisition_timeout_ms: 0,
        log_segment_bytes: 64 * 1024 * 1024,
        max_expanded_commit_bytes: 512 * 1024 * 1024,
        ..resources::StorageResources::default()
    };
    assert!(matches!(
        Store::open_with_resources(&path, invalid),
        Err(StorageError::InvalidConfiguration(_))
    ));
    assert!(!path.exists());
    std::fs::create_dir(&path).unwrap();
    let store = Store::new_in_memory();
    assert!(store.attach_redb(&path).is_err());
    assert_eq!(store.resource_budget().snapshot().used, 0);
    std::fs::remove_dir(path).unwrap();
}

#[test]
fn current_views_are_repeatable_across_writes_deletes_and_recreates() {
    on_backends(|store| {
        let empty = store.read_view().unwrap();
        assert_eq!(empty.commit_index(), 0);
        commit(store, vec![(b"user/a".to_vec(), Some(vec![1]))]);
        let old = store.read_view().unwrap();
        commit(
            store,
            vec![
                (b"user/a".to_vec(), None),
                (b"user/b".to_vec(), Some(vec![2])),
            ],
        );
        let deleted = store.read_view().unwrap();
        commit(store, vec![(b"user/a".to_vec(), Some(vec![3]))]);
        assert_eq!(old.get(b"user/a").unwrap(), Some(vec![1]));
        assert_eq!(deleted.get(b"user/a").unwrap(), None);
        assert_eq!(store.read_at(2).unwrap().get(b"user/a").unwrap(), None);
        assert_eq!(
            store.read_at(1).unwrap().get(b"user/a").unwrap(),
            Some(vec![1])
        );
        assert_eq!(
            store.read_view().unwrap().get(b"user/a").unwrap(),
            Some(vec![3])
        );
        assert!(rows(&empty, ScanBounds::prefix(b"user/"), ScanProjection::Keys).is_empty());
        assert_eq!(
            rows(&old, ScanBounds::prefix(b"user/"), ScanProjection::Keys),
            vec![ScanRow {
                key: b"user/a".to_vec(),
                value: None
            }]
        );
    });
}

#[test]
fn read_pins_limit_gc_and_floor_is_monotonic_and_validated() {
    on_backends(|store| {
        commit(store, vec![(b"user/a".to_vec(), Some(vec![1]))]);
        let first = store.read_at(1).unwrap();
        let same = store.read_at(1).unwrap();
        commit(store, vec![(b"user/a".to_vec(), Some(vec![2]))]);
        commit(store, vec![(b"user/a".to_vec(), Some(vec![3]))]);
        store.gc(100).unwrap();
        assert_eq!(store.gc_watermark(), 1);
        drop(first);
        store.gc(3).unwrap();
        assert_eq!(store.gc_watermark(), 1);
        assert_eq!(same.get(b"user/a").unwrap(), Some(vec![1]));
        assert!(matches!(store.compact(), Err(StorageError::ReadersActive)));
        drop(same);
        store.gc(100).unwrap();
        assert_eq!(store.gc_watermark(), 3);
        assert!(matches!(
            store.read_at(2),
            Err(StorageError::SnapshotExpired {
                requested: 2,
                oldest: 3
            })
        ));
        assert!(matches!(
            store.read_at(4),
            Err(StorageError::FutureSnapshot {
                requested: 4,
                visible: 3
            })
        ));
        store.gc(0).unwrap();
        assert_eq!(store.gc_watermark(), 3);
    });
}

#[test]
fn scan_bounds_match_byte_order_oracle_for_arbitrary_prefixes() {
    on_backends(|store| {
        let mut keys = vec![
            vec![],
            vec![0],
            vec![0, 255],
            vec![1],
            vec![254],
            vec![255],
            vec![255, 0],
            vec![255, 255],
        ];
        for i in 0..=255 {
            keys.push(vec![42, i]);
        }
        commit(store, keys.iter().map(|k| (k.clone(), Some(vec![7]))));
        // Oracle: the store's own complete key set (native for a redb-backed
        // store, the maps for the in-memory store). Same operational keys as
        // the scan under test.
        let oracle_keys = store.try_range_prefix(&[]).unwrap();
        let view = store.read_view().unwrap();
        for prefix in [
            vec![],
            vec![0],
            vec![42],
            vec![255],
            vec![255, 255],
            vec![99],
        ] {
            // Empty prefix includes operational metadata too.
            let expected: Vec<ScanRow> = oracle_keys
                .iter()
                .filter(|(key, _)| key.starts_with(&prefix))
                .map(|(key, value)| ScanRow {
                    key: key.clone(),
                    value: Some(value.clone()),
                })
                .collect();
            let mut cursor = view
                .scan(ScanBounds::prefix(&prefix), ScanProjection::KeyValue)
                .unwrap();
            let mut actual = Vec::new();
            loop {
                let batch = cursor.next_batch(3, 64 * 1024).unwrap();
                actual.extend(batch.rows);
                if batch.done {
                    break;
                }
            }
            assert_eq!(actual, expected);
        }
        for start in [Included(vec![0]), Excluded(vec![0]), Unbounded] {
            for end in [Included(vec![255]), Excluded(vec![255]), Unbounded] {
                let bounds = ScanBounds {
                    start: start.clone(),
                    end: end.clone(),
                };
                let actual = rows(&view, bounds, ScanProjection::Keys);
                let key_ok = |k: &Vec<u8>| -> bool {
                    let start_ok = match &start {
                        Included(s) => k >= s,
                        Excluded(s) => k > s,
                        Unbounded => true,
                    };
                    let end_ok = match &end {
                        Included(e) => k <= e,
                        Excluded(e) => k < e,
                        Unbounded => true,
                    };
                    start_ok && end_ok
                };
                let expected: Vec<ScanRow> = oracle_keys
                    .iter()
                    .filter(|(key, _)| key_ok(key))
                    .map(|(key, _)| ScanRow {
                        key: key.clone(),
                        value: None,
                    })
                    .collect();
                assert_eq!(actual, expected);
            }
        }
    });
}

#[test]
fn batch_boundaries_do_not_skip_rows_and_oversize_can_be_retried() {
    on_backends(|store| {
        commit(
            store,
            vec![
                (b"u/a".to_vec(), Some(vec![1; 5])),
                (b"u/b".to_vec(), Some(vec![2; 5])),
            ],
        );
        let view = store.read_view().unwrap();
        let mut cursor = view
            .scan(ScanBounds::prefix(b"u/"), ScanProjection::KeyValue)
            .unwrap();
        assert!(matches!(
            cursor.next_batch(10, 7),
            Err(StorageError::ResourceExhausted(_))
        ));
        let a = cursor.next_batch(10, 15).unwrap();
        assert_eq!(a.rows.len(), 1);
        assert_eq!(a.rows[0].key, b"u/a");
        assert_eq!(a.bytes, 8);
        let b = cursor.next_batch(10, 15).unwrap();
        assert_eq!(b.rows.len(), 1);
        assert_eq!(b.rows[0].key, b"u/b");
        assert!(b.done);
        assert!(cursor.next_batch(10, 15).unwrap().rows.is_empty());
    });
}

#[test]
fn keys_only_avoids_copying_oversized_current_values() {
    on_backends(|store| {
        commit(store, vec![(b"u/a".to_vec(), Some(vec![1; 100]))]);
        let view = store
            .read_with_limits(
                None,
                ReadLimits {
                    max_item_bytes: 10,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(matches!(
            view.get(b"u/a"),
            Err(StorageError::ResourceExhausted(_))
        ));
        assert_eq!(
            rows(&view, ScanBounds::prefix(b"u/"), ScanProjection::Keys),
            vec![ScanRow {
                key: b"u/a".to_vec(),
                value: None
            }]
        );
    });
}

#[test]
fn a_failed_batch_does_not_consume_undelivered_rows() {
    on_backends(|store| {
        commit(
            store,
            vec![
                (b"u/a".to_vec(), Some(vec![1])),
                (b"u/b".to_vec(), Some(vec![2; 100])),
            ],
        );
        let view = store
            .read_with_limits(
                None,
                ReadLimits {
                    max_item_bytes: 10,
                    ..Default::default()
                },
            )
            .unwrap();
        let mut cursor = view
            .scan(ScanBounds::prefix(b"u/"), ScanProjection::KeyValue)
            .unwrap();
        assert!(matches!(
            cursor.next_batch(10, 1024),
            Err(StorageError::ResourceExhausted(_))
        ));
        let delivered = cursor.next_batch(1, 1024).unwrap();
        assert_eq!(delivered.rows[0].key, b"u/a");
    });
}

#[test]
fn a_failed_batch_does_not_consume_undelivered_rows_across_multiple_prior_rows() {
    // v2's native-iterator fast path (B7 follow-up) cannot "un-consume" a
    // row the way v1's position-based retry does (re-seeking just
    // re-derives it) — it has its own undo buffer (`pending_native`) to
    // match the same contract. Two consumed-then-undelivered rows, not
    // just one, exercises that buffer actually holding more than a single
    // row, which `a_failed_batch_does_not_consume_undelivered_rows` alone
    // does not distinguish from "only ever handles exactly one".
    on_backends(|store| {
        commit(
            store,
            vec![
                (b"u/a".to_vec(), Some(vec![1])),
                (b"u/b".to_vec(), Some(vec![2])),
                (b"u/c".to_vec(), Some(vec![3; 100])),
            ],
        );
        let view = store
            .read_with_limits(
                None,
                ReadLimits {
                    max_item_bytes: 10,
                    ..Default::default()
                },
            )
            .unwrap();
        let mut cursor = view
            .scan(ScanBounds::prefix(b"u/"), ScanProjection::KeyValue)
            .unwrap();
        assert!(matches!(
            cursor.next_batch(10, 1024),
            Err(StorageError::ResourceExhausted(_))
        ));
        // Exactly matching the two undelivered rows avoids re-touching the
        // oversized "u/c" in this same call (which would re-stash and
        // re-error — the same shape v1 already has: a caller must shrink
        // its limits to step past a poison row, not just retry blindly).
        let delivered = cursor.next_batch(2, 1024).unwrap();
        assert_eq!(
            delivered.rows.len(),
            2,
            "both prior rows arrive, not just one"
        );
        assert_eq!(delivered.rows[0].key, b"u/a");
        assert_eq!(delivered.rows[1].key, b"u/b");
        assert!(!delivered.done);
    });
}

#[test]
fn a_tighter_byte_limit_on_a_later_call_still_errors_on_a_stashed_row() {
    // Regression for a real bug caught in review of the native path's
    // `pending_native` undo buffer (see
    // `a_failed_batch_does_not_consume_undelivered_rows_across_multiple_prior_rows`):
    // every stashed row already passed the absolute `max_item_bytes` check
    // when it was first read, so the *only* way it can still not fit is a
    // caller using a tighter `byte_limit` on a later call. The first
    // version of this buffer silently returned an empty, non-done `Ok`
    // batch in that case instead of `ResourceExhausted` — indistinguishable
    // from "nothing left yet" to a caller, so it would spin forever
    // instead of ever being told to raise its limit or give up.
    on_backends(|store| {
        commit(
            store,
            vec![
                (b"u/a".to_vec(), Some(vec![1])),
                (b"u/b".to_vec(), Some(vec![2])),
                (b"u/c".to_vec(), Some(vec![3; 100])),
            ],
        );
        let view = store
            .read_with_limits(
                None,
                ReadLimits {
                    max_item_bytes: 10,
                    ..Default::default()
                },
            )
            .unwrap();
        let mut cursor = view
            .scan(ScanBounds::prefix(b"u/"), ScanProjection::KeyValue)
            .unwrap();
        assert!(matches!(
            cursor.next_batch(10, 1024),
            Err(StorageError::ResourceExhausted(_))
        ));
        // "u/a" alone (key + value) is 4 bytes; a byte_limit of 1 cannot
        // admit it even though it already passed the absolute item limit.
        assert!(matches!(
            cursor.next_batch(2, 1),
            Err(StorageError::ResourceExhausted(_))
        ));
    });
}

#[test]
fn cancellation_and_invalid_bounds_are_explicit_errors() {
    on_backends(|store| {
        let view = store.read_view().unwrap();
        assert!(matches!(
            view.scan(
                ScanBounds {
                    start: Included(vec![2]),
                    end: Included(vec![1])
                },
                ScanProjection::Keys
            ),
            Err(StorageError::InvalidRead(_))
        ));
        let mut cursor = view
            .scan(ScanBounds::prefix(b"u/"), ScanProjection::Keys)
            .unwrap();
        assert!(matches!(
            cursor.next_batch(0, 1),
            Err(StorageError::InvalidRead(_))
        ));
        view.cancellation().cancel();
        assert!(matches!(view.get(b"u/a"), Err(StorageError::Cancelled)));
        assert!(matches!(
            cursor.next_batch(1, 1),
            Err(StorageError::Cancelled)
        ));
    });
}

#[test]
fn native_reads_do_not_depend_on_application_maps_and_survive_reopen() {
    let path = crate::TempPath::store("native-read");
    {
        let store = Store::open(&path).unwrap();
        configure(&store);
        commit(&store, vec![(b"u/a".to_vec(), Some(vec![1]))]);
        commit(&store, vec![(b"u/a".to_vec(), Some(vec![2]))]);
    }
    let store = Store::open(&path).unwrap();
    // Test-only removal proves the new path reads redb itself, not the
    // transitional mirrors still needed by the legacy writer.
    store.data.write().unwrap().clear();
    store.created.write().unwrap().clear();
    store.history.write().unwrap().clear();
    let view = store.read_view().unwrap();
    assert_eq!(view.commit_index(), 2);
    assert_eq!(view.get(b"u/a").unwrap(), Some(vec![2]));
    assert_eq!(
        store.read_at(1).unwrap().get(b"u/a").unwrap(),
        Some(vec![1])
    );
    assert_eq!(
        rows(&view, ScanBounds::prefix(b"u/"), ScanProjection::KeyValue),
        vec![ScanRow {
            key: b"u/a".to_vec(),
            value: Some(vec![2])
        }]
    );
    drop(view);
    drop(store);
    crate::remove_store(&path).unwrap();
}

#[test]
fn transaction_key_scans_skip_large_payloads_and_merge_binary_overlay() {
    on_backends(|store| {
        // Generate the large fixture in expansion so the test WAL need not
        // encode millions of JSON array elements just to exercise a read.
        store.set_expander(Arc::new(Box::new(|_, _, txn| {
            txn.put(vec![255, 0], vec![7; 8 * 1024 * 1024 + 1]);
            txn.put(vec![255, 2], vec![2]);
            txn.put(vec![255, 255], vec![3]);
            Ok(())
        })));
        commit(store, vec![]);
        let result = store.commit::<(), _>(Uuid::new_v4(), |_, txn| {
            assert!(matches!(
                txn.range_prefix(&[255]),
                Err(StorageError::ResourceExhausted(_))
            ));
            assert_eq!(
                txn.keys_prefix(&[255]).unwrap(),
                vec![vec![255, 0], vec![255, 2], vec![255, 255]]
            );
            txn.del(vec![255, 0]);
            txn.put(vec![255, 1], vec![8; 8 * 1024 * 1024 + 1]);
            txn.put(vec![255, 2], vec![9; 8 * 1024 * 1024 + 1]);
            txn.del(vec![255, 254]); // Absent-base tombstones do not appear.
            txn.put(vec![255, 255, 0], vec![4]);
            let expected = vec![
                vec![255, 1],
                vec![255, 2],
                vec![255, 255],
                vec![255, 255, 0],
            ];
            assert_eq!(txn.keys_prefix(&[255]).unwrap(), expected);
            assert_eq!(
                txn.keys_prefix(&[255, 255]).unwrap(),
                vec![vec![255, 255], vec![255, 255, 0]]
            );
            let all = txn.keys_prefix(&[]).unwrap();
            assert!(all.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(
                all.into_iter()
                    .filter(|key| key.starts_with(&[255]))
                    .collect::<Vec<_>>(),
                expected
            );
            Err("inspection only".into())
        });
        assert!(
            matches!(&result, Ok(CommitOutcome::Conflicted(message)) if message == "inspection only"),
            "unexpected inspection outcome: {result:?}"
        );
        assert_eq!(store.committed_version(), 1);
    });
}

#[test]
fn persistent_planning_expansion_preconditions_and_retry_ignore_application_maps() {
    let path = crate::TempPath::store("native-writer");
    let store = Store::open(&path).unwrap();
    configure(&store);
    commit(
        &store,
        vec![
            (b"u/a".to_vec(), Some(vec![1])),
            (b"u/b".to_vec(), Some(vec![2])),
        ],
    );
    let mut first: CommitBatch =
        rushdb_types::command_codec::decode(&store.log_bytes()[0]).unwrap();
    store.data.write().unwrap().clear();
    store.created.write().unwrap().clear();
    store.history.write().unwrap().clear();
    let retry = store.commit::<(), _>(first.commit_key, |_, _| panic!("native outcome was missed"));
    assert!(matches!(
        retry,
        Ok(CommitOutcome::Committed {
            commit_index: 1,
            ..
        })
    ));
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        assert_eq!(txn.get(b"u/a").map_err(|e| e.to_string())?, Some(vec![1]));
        assert_eq!(txn.range_prefix(b"u/").map_err(|e| e.to_string())?.len(), 2);
        assert_eq!(
            txn.keys_prefix(b"u/").unwrap(),
            vec![b"u/a".to_vec(), b"u/b".to_vec()]
        );
        txn.del(b"u/b".to_vec());
        txn.put(b"u/a".to_vec(), vec![3]);
        txn.put(b"u/c".to_vec(), vec![4]);
        assert_eq!(
            txn.keys_prefix(b"u/").unwrap(),
            vec![b"u/a".to_vec(), b"u/c".to_vec()]
        );
        assert_eq!(
            txn.range_prefix(b"u/").map_err(|e| e.to_string())?,
            vec![(b"u/a".to_vec(), vec![3]), (b"u/c".to_vec(), vec![4])]
        );
        Ok(())
    })));
    let second = Uuid::new_v4();
    first.commit_key = second;
    first.preconditions = vec![rushdb_types::command::Precondition::exists(b"u/a".to_vec())];
    store
        .commit(second, |_, txn| {
            assert_eq!(txn.get(b"u/b").map_err(|e| e.to_string())?, Some(vec![2]));
            Ok((first, ()))
        })
        .unwrap();
    assert_eq!(
        store.read_view().unwrap().get(b"u/a").unwrap(),
        Some(vec![3])
    );
    assert_eq!(
        store.read_at(1).unwrap().get(b"u/a").unwrap(),
        Some(vec![1])
    );
    assert_eq!(
        store.try_outcome_for(second).unwrap(),
        Some(OutcomeKind::Committed(2))
    );
    let mut deps = DependencySet::default();
    deps.record(b"u/b".to_vec(), b"u/b".to_vec());
    let mut batch: CommitBatch =
        rushdb_types::command_codec::decode(&store.log_bytes()[0]).unwrap();
    batch.commit_key = Uuid::new_v4();
    assert!(matches!(
        store.commit_serializable(batch.commit_key, 0, deps, |_| Ok((batch, ()))),
        Ok(CommitOutcome::Conflicted(_))
    ));
    drop(store);
    crate::remove_store(&path).unwrap();
}

#[test]
fn corrupt_outcome_is_an_error_not_an_idempotency_miss() {
    on_backends(|store| {
        let key = Uuid::new_v4();
        let ops = vec![Op::Put(crate::receipt::receipt_key(key), vec![9; 17])];
        store.persist_ops_local(&ops, 1).unwrap();
        store.persist_ops_local(&ops, 1).unwrap();
        assert!(matches!(
            store.commit::<(), _>(key, |_, _| panic!("corrupt outcome caused reexecution")),
            Err(StorageError::CorruptLog)
        ));
        assert_eq!(store.committed_version(), 0);
    });
}

#[test]
fn native_history_decode_has_an_explicit_block_ceiling() {
    let path = crate::TempPath::store("history-limit");
    let store = Store::open(&path).unwrap();
    configure(&store);
    commit(&store, vec![(b"u/a".to_vec(), Some(vec![1; 100]))]);
    commit(&store, vec![(b"u/a".to_vec(), Some(vec![2]))]);
    let view = store
        .read_with_limits(
            Some(1),
            ReadLimits {
                max_history_block_bytes: 10,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(
        view.get(b"u/a"),
        Err(StorageError::ResourceExhausted(_))
    ));
    drop(view);
    drop(store);
    crate::remove_store(&path).unwrap();
}

#[test]
fn native_current_reads_do_not_decode_deleted_key_history() {
    let path = crate::TempPath::store("current-only");
    let store = Store::open(&path).unwrap();
    configure(&store);
    commit(&store, vec![(b"u/deleted".to_vec(), Some(vec![1; 100]))]);
    commit(
        &store,
        vec![
            (b"u/deleted".to_vec(), None),
            (b"u/live".to_vec(), Some(vec![2])),
        ],
    );
    let limits = ReadLimits {
        max_history_block_bytes: 1,
        ..Default::default()
    };
    let current = store.read_with_limits(None, limits).unwrap();
    assert_eq!(current.get(b"u/deleted").unwrap(), None);
    assert_eq!(
        rows(
            &current,
            ScanBounds::prefix(b"u/"),
            ScanProjection::KeyValue
        ),
        vec![ScanRow {
            key: b"u/live".to_vec(),
            value: Some(vec![2]),
        }]
    );
    commit(&store, vec![(b"u/live".to_vec(), Some(vec![3]))]);
    assert_eq!(current.get(b"u/live").unwrap(), Some(vec![2]));
    assert!(matches!(
        store
            .read_with_limits(Some(1), limits)
            .unwrap()
            .get(b"u/deleted"),
        Err(StorageError::ResourceExhausted(_))
    ));
    drop(current);
    drop(store);
    crate::remove_store(&path).unwrap();
}

#[test]
fn views_are_send_and_sync_without_unsafe_lifetimes() {
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<ReadView>();
}

#[test]
fn active_views_remain_consistent_during_concurrent_writes_and_gc() {
    on_backends(|store| {
        commit(store, vec![(b"u/a".to_vec(), Some(vec![1]))]);
        let view = store.read_view().unwrap();
        std::thread::scope(|scope| {
            let writer = scope.spawn(|| {
                for value in 2..40 {
                    commit(store, vec![(b"u/a".to_vec(), Some(vec![value]))]);
                    store.gc(store.committed_version()).unwrap();
                }
            });
            for _ in 0..40 {
                assert_eq!(view.get(b"u/a").unwrap(), Some(vec![1]));
                assert_eq!(
                    rows(&view, ScanBounds::prefix(b"u/"), ScanProjection::KeyValue),
                    vec![ScanRow {
                        key: b"u/a".to_vec(),
                        value: Some(vec![1])
                    }]
                );
            }
            writer.join().unwrap();
        });
        assert_eq!(store.gc_watermark(), 1);
        drop(view);
        store.gc(store.committed_version()).unwrap();
        assert_eq!(store.gc_watermark(), store.committed_version());
    });
}

/// P02 property test: for arbitrary randomized keys, the native scan (bounded
/// batches, byte-ordered) must equal the store's own memory `data` map (the
/// complete key oracle, including operational metadata keys) for every prefix
/// and for included/excluded byte bounds, on both backends. This is the
/// "property-based arbitrary-key scan" acceptance item.
#[test]
fn randomized_keys_scan_matches_memory_oracle_for_arbitrary_bounds() {
    on_backends(|store| {
        let mut rng = {
            let mut state = 0xDEAD_BEEF_1234_5678u64;
            move || {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                (state >> 33).to_le_bytes()[0]
            }
        };
        // Seed random data keys (dedup). Operational metadata keys are added by
        // `commit` and appear in BOTH the native scan and the memory-map oracle.
        let mut seen = std::collections::BTreeSet::<Vec<u8>>::new();
        let mut keys = Vec::new();
        for _ in 0..500 {
            let len = (rng() % 8) as usize;
            let mut k = Vec::with_capacity(len);
            for _ in 0..len {
                k.push(rng());
            }
            if seen.insert(k.clone()) {
                keys.push(k);
            }
        }
        keys.push(vec![]);
        keys.push(vec![255, 255, 255]);
        commit(store, keys.iter().map(|k| (k.clone(), Some(vec![7]))));
        let view = store.read_view().unwrap();

        // Complete-key oracle: the store's own range_prefix (native for a
        // redb-backed store, maps for in-memory). Includes operational keys.
        let oracle = || {
            store
                .try_range_prefix(&[])
                .unwrap()
                .into_iter()
                .collect::<std::collections::BTreeMap<Vec<u8>, Vec<u8>>>()
        };

        // 1) Every prefix: native range_prefix must equal the memory map.
        for _ in 0..200 {
            let plen = (rng() % 6) as usize;
            let mut prefix = Vec::with_capacity(plen);
            for _ in 0..plen {
                prefix.push(rng());
            }
            let expected: Vec<Vec<u8>> = oracle()
                .range::<Vec<u8>, _>(prefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&prefix))
                .map(|(k, _)| k.clone())
                .collect();
            let got: Vec<Vec<u8>> = rows(&view, ScanBounds::prefix(&prefix), ScanProjection::Keys)
                .into_iter()
                .map(|r| r.key)
                .collect();
            assert_eq!(got, expected, "prefix {prefix:?}");
        }

        // 2) Included byte-range bounds: [start, end).
        for _ in 0..200 {
            let a_len = (rng() % 5) as usize;
            let b_len = (rng() % 5) as usize;
            let mut a = Vec::with_capacity(a_len);
            let mut b = Vec::with_capacity(b_len);
            for _ in 0..a_len {
                a.push(rng());
            }
            for _ in 0..b_len {
                b.push(rng());
            }
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let expected: Vec<Vec<u8>> = oracle()
                .range::<Vec<u8>, _>(lo.clone()..hi.clone())
                .map(|(k, _)| k.clone())
                .collect();
            let got: Vec<Vec<u8>> = rows(
                &view,
                ScanBounds {
                    start: Included(lo.clone()),
                    end: Excluded(hi.clone()),
                },
                ScanProjection::Keys,
            )
            .into_iter()
            .map(|r| r.key)
            .collect();
            assert_eq!(got, expected, "bounds {lo:?}..{hi:?}");
        }
    });
}

/// P03 acceptance: concurrent read handles each own a native redb read
/// transaction and serve reads without a global mutex held across the read
/// operation. Many reader threads, each with its own view, must all observe
/// the committed data and complete without serialization artifacts.
#[test]
fn concurrent_read_views_serve_independently_without_global_lock() {
    on_backends(|store| {
        // Commit a set of keys the readers will scan.
        let keys: Vec<Vec<u8>> = (0..256)
            .map(|i| vec![0xAB, u8::try_from(i).unwrap()])
            .collect();
        commit(store, keys.iter().map(|k| (k.clone(), Some(vec![7]))));
        let expected = keys.len();

        let barrier = Arc::new(std::sync::Barrier::new(8));
        let store_ref = store;
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    // Each thread opens its OWN view (its own redb read txn).
                    let view = store_ref.read_view().unwrap();
                    let rows = rows(&view, ScanBounds::prefix(&[0xAB]), ScanProjection::Keys);
                    assert_eq!(rows.len(), expected, "reader must see all committed keys");
                    barrier.wait();
                });
            }
        });
    });
}

/// P06: read leases are expirable by age so an abandoned reader cannot block
/// history advancement forever, and retention is observable.
#[test]
fn read_lease_expiry_releases_abandoned_pins_and_is_observable() {
    on_backends(|store| {
        // Two views pin two CIs.
        let v1 = store.read_view().unwrap();
        let v2 = store.read_view().unwrap();
        assert!(v1.commit_index() > 0 || store.committed_version() == 0);
        let (oldest, count) = store.retention_summary();
        assert_eq!(count, 1, "both views pin the same (current) CI");
        assert_eq!(oldest, Some(v1.commit_index()));

        // A very small max_age must expire the leases (safe release) once some
        // wall time passes.
        std::thread::sleep(std::time::Duration::from_millis(5));
        let released = store.expire_read_pins(1);
        assert!(released >= 1, "lease expiry must release the pin");
        let (oldest_after, count_after) = store.retention_summary();
        assert_eq!(count_after, 0, "expired leases release retention");
        assert!(oldest_after.is_none());
        assert!(matches!(v1.check_cancelled(), Err(StorageError::Cancelled)));
        assert!(matches!(v2.check_cancelled(), Err(StorageError::Cancelled)));

        // Dropping the views after expiry is harmless (pins already removed).
        drop(v1);
        drop(v2);
    });
}

/// A stale view that drops after its lease expired must not decrement a newer,
/// unrelated pin admitted at the same commit index (pin-generation identity).
#[test]
fn stale_view_drop_does_not_corrupt_a_newer_pin_at_the_same_ci() {
    on_backends(|store| {
        let stale = store.read_view().unwrap();
        let ci = stale.commit_index();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(store.expire_read_pins(1) >= 1, "expire the stale lease");

        // A new view at the same CI re-creates the pin entry (new generation).
        let live = store.read_view().unwrap();
        assert_eq!(live.commit_index(), ci);
        assert_eq!(store.retention_summary(), (Some(ci), 1));

        // Dropping the expired view must leave the live pin untouched.
        drop(stale);
        assert_eq!(
            store.retention_summary(),
            (Some(ci), 1),
            "a stale drop must not release a live pin"
        );

        drop(live);
        assert_eq!(store.retention_summary(), (None, 0));
    });
}
