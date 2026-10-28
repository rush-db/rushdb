use super::*;
use std::time::Duration;

fn batch(commit_key: Uuid) -> CommitBatch {
    CommitBatch {
        command_version: rushdb_types::command::COMMAND_VERSION,
        tenant: TenantId::from_ordinal(1),
        transaction_id: "writer-test".into(),
        commit_key,
        start_index: 0,
        isolation: rushdb_types::command::IsolationMode::ReadCommitted,
        preconditions: vec![],
        mutations: vec![],
    }
}

#[test]
fn fallible_crash_log_export_preserves_decoded_batches() {
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"log-test".to_vec(), vec![1]);
        Ok(())
    })));
    let key = Uuid::new_v4();
    store
        .commit(key, |_, _| Ok((batch(key), ())))
        .expect("commit");
    assert_eq!(
        store.try_take_log_for_crash().unwrap(),
        store.take_log_for_crash()
    );
}

#[test]
fn tenant_routed_commit_rejects_batch_for_another_tenant() {
    let store = Store::new_in_memory();
    let routed = TenantId::from_ordinal(2);
    let key = Uuid::new_v4();
    let error = store
        .commit_for_tenant(&routed, key, |_, _| Ok((batch(key), ())))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("batch tenant does not match the routed tenant"));
    assert_eq!(store.committed_version(), 0);
}

#[test]
fn expansion_failures_never_consume_commit_indices_in_any_entry_point() {
    for mode in 0..4 {
        let store = Store::new_in_memory();
        for missing in [true, false] {
            if !missing {
                store.set_expander(Arc::new(Box::new(|_, _, _| Err("bad expansion".into()))));
            }
            let key = Uuid::new_v4();
            let failed = match mode {
                0 => store.commit(key, |_, _| Ok((batch(key), ()))).map(|_| ()),
                1 => store
                    .commit_serializable(key, 0, DependencySet::default(), |_| Ok((batch(key), ())))
                    .map(|_| ()),
                2 => store.apply_replicated_batch(&batch(key)).map(|_| ()),
                // The transaction commit path must reserve its commit index
                // only after expansion succeeds; otherwise a user-reachable
                // expansion error strands the contiguous publisher forever.
                _ => store
                    .tx_commit(key, &TxAccumulator::new(TenantId::from_ordinal(1), 0))
                    .map(|_| ()),
            };
            assert!(failed.is_err());
            assert_eq!(store.next_commit_index.load(Ordering::SeqCst), 1);
            assert_eq!(store.committed_version(), 0);
        }
        store.set_expander(Arc::new(Box::new(|_, _, txn| {
            txn.put(b"a".to_vec(), vec![1]);
            Ok(())
        })));
        let key = Uuid::new_v4();
        assert!(matches!(
            store.commit(key, |_, _| Ok((batch(key), ()))),
            Ok(CommitOutcome::Committed {
                commit_index: 1,
                ..
            })
        ));
    }
}

#[test]
fn transaction_reads_resolve_at_their_pinned_snapshot() {
    // A cross-request transaction must not observe commits that land after
    // its BEGIN. The accumulator pins the base view; `tx_read` must resolve
    // through it instead of current committed state.
    let store = Store::new_in_memory();
    let base = Arc::new(store.read_at(0).unwrap());
    let mut acc = TxAccumulator::new(TenantId::from_ordinal(1), 0);
    acc.base_view = Some(base);

    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"k".to_vec(), vec![1]);
        Ok(())
    })));
    store
        .commit(Uuid::new_v4(), |_, _| Ok((batch(Uuid::new_v4()), ())))
        .unwrap();
    assert_eq!(store.try_get(b"k").unwrap(), Some(vec![1]));

    let seen = store
        .tx_read(&mut acc, |txn| txn.get(b"k").map_err(|e| e.to_string()))
        .unwrap();
    assert_eq!(
        seen, None,
        "transaction must read its pinned snapshot, not the later commit"
    );
}

#[test]
fn two_transactions_writing_one_key_cannot_both_commit() {
    // D01 regression: transaction validation must be serialized with
    // publication. Both accumulators share a write key and the same base
    // snapshot; exactly one may commit.
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"k".to_vec(), vec![1]);
        Ok(())
    })));
    let mut first = TxAccumulator::new(TenantId::from_ordinal(1), 0);
    first.buffer.insert(b"k".to_vec(), Some(vec![1]));
    let mut second = TxAccumulator::new(TenantId::from_ordinal(1), 0);
    second.buffer.insert(b"k".to_vec(), Some(vec![1]));

    assert!(store.tx_commit(Uuid::new_v4(), &first).is_ok());
    let second_outcome = store.tx_commit(Uuid::new_v4(), &second);
    assert!(
        second_outcome.is_err(),
        "second writer must observe the first commit as a conflict: {second_outcome:?}"
    );
    assert_eq!(store.committed_version(), 1);
    assert_eq!(store.next_commit_index.load(Ordering::SeqCst), 2);
}

#[test]
fn replication_watermark_is_optional_and_exposed_to_retention() {
    struct Watermark;
    impl Replicator for Watermark {
        fn replicate(&self, _batch: &CommitBatch, _payload: &[u8]) -> Result<(), String> {
            Ok(())
        }

        fn acknowledged_commit_index(&self) -> Option<u64> {
            Some(17)
        }
    }

    let store = Store::new_in_memory();
    assert_eq!(store.replication_acknowledged_commit(), None);
    store.set_replicator(Some(Arc::new(Watermark)));
    assert_eq!(store.replication_acknowledged_commit(), Some(17));
}

#[test]
fn persistence_failure_completes_all_waiters_and_fences_new_writes() {
    let store = Arc::new(Store::new_in_memory());
    store.fail_next_persist.store(true, Ordering::SeqCst);
    let (later_tx, later_rx) = mpsc::channel();
    store.pending.lock().unwrap().insert(
        2,
        PendingCommit {
            ci: 2,
            shard: 0,
            ops: vec![],
            prepared: PreparedCommand::from_ops(2, &[]),
            command_payload: Vec::new(),
            ack: later_tx,
        },
    );
    let (done_tx, done_rx) = mpsc::channel();
    let worker_store = store.clone();
    let worker = std::thread::spawn(move || {
        done_tx
            .send(worker_store.enqueue_and_publish(
                1,
                0,
                vec![Op::Put(b"a".to_vec(), vec![1])],
                vec![],
            ))
            .unwrap();
    });
    assert!(matches!(
        done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("publisher stranded a waiter"),
        Err(StorageError::Durability(_))
    ));
    assert!(matches!(
        later_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        Err(StorageError::Durability(_))
    ));
    worker.join().unwrap();
    assert_eq!(store.committed_version(), 0);
    assert!(matches!(
        store.try_get(b"a"),
        Err(StorageError::Durability(_))
    ));
    assert!(store.pending.lock().unwrap().is_empty());
    assert!(store.publisher_failure().is_some());
    store.begin_shutdown();
    assert!(store.maintenance_aborted());
    assert!(matches!(
        store.commit::<(), _>(Uuid::new_v4(), |_, _| panic!("fenced writer ran a builder")),
        Err(StorageError::Durability(_))
    ));
    assert!(matches!(
        store.read_view(),
        Err(StorageError::Durability(_))
    ));
}

#[test]
fn temporary_nonpublisher_lock_holder_does_not_strand_a_commit() {
    let store = Arc::new(Store::new_in_memory());
    let guard = store.publish_lock.lock().unwrap();
    let (tx, rx) = mpsc::channel();
    let worker_store = store.clone();
    let worker = std::thread::spawn(move || {
        tx.send(worker_store.enqueue_and_publish(1, 0, vec![], vec![]))
            .unwrap();
    });
    let mut pending = store.pending.lock().unwrap();
    while !pending.contains_key(&1) {
        let (next, timeout) = store
            .pending_cv
            .wait_timeout(pending, Duration::from_secs(2))
            .unwrap();
        pending = next;
        assert!(!timeout.timed_out(), "worker failed to enqueue");
    }
    drop(pending);
    // No artificial notification when this read/maintenance lock is released.
    drop(guard);
    rx.recv_timeout(Duration::from_secs(2))
        .expect("non-publisher lock stranded commit")
        .unwrap();
    worker.join().unwrap();
}

#[test]
fn actual_write_summaries_detect_conflicts_without_reexpansion() {
    let store = Store::new_in_memory();
    let expansions = Arc::new(AtomicU64::new(0));
    let observed = expansions.clone();
    store.set_expander(Arc::new(Box::new(move |_, _, txn| {
        observed.fetch_add(1, Ordering::SeqCst);
        // Re-expanding against current state would incorrectly emit no write.
        if txn.get(b"a").map_err(|e| e.to_string())?.is_none() {
            txn.put(b"a".to_vec(), vec![1]);
        }
        Ok(())
    })));
    let first = Uuid::new_v4();
    store.commit(first, |_, _| Ok((batch(first), ()))).unwrap();
    let mut deps = DependencySet::default();
    deps.record(b"a".to_vec(), b"a".to_vec());
    let next = Uuid::new_v4();
    assert!(matches!(
        store.commit_serializable(next, 0, deps, |_| Ok((batch(next), ()))),
        Ok(CommitOutcome::Conflicted(_))
    ));
    assert_eq!(expansions.load(Ordering::SeqCst), 1);
    assert_eq!(store.outcome_for(next), Some(OutcomeKind::Aborted));
}

#[test]
fn prepared_commands_detect_read_write_and_write_write_overlap() {
    let mut reader = PreparedCommand::from_ops(1, &[Op::Put(b"out".to_vec(), vec![1])]);
    reader.reads.record(b"input".to_vec(), b"input".to_vec());
    let writer = PreparedCommand::from_ops(2, &[Op::Set(b"input".to_vec(), vec![2])]);
    assert!(reader.conflicts_with(&writer));

    let same_key = PreparedCommand::from_ops(3, &[Op::Del(b"out".to_vec())]);
    assert!(reader.conflicts_with(&same_key));

    let disjoint = PreparedCommand::from_ops(4, &[Op::Put(b"other".to_vec(), vec![3])]);
    assert!(!reader.conflicts_with(&disjoint));
    assert_eq!(reader.commit_index, 1);

    let mut whole = PreparedCommand::from_ops(5, &[]);
    whole.reads.record_prefix(b"");
    assert!(whole.conflicts_with(&disjoint));
}

#[test]
fn fallible_replay_rejects_corrupt_input_without_advancing_state() {
    let store = Store::new_in_memory();
    let error = store.try_replay(&[b"not-json".to_vec()]).unwrap_err();
    assert!(matches!(error, StorageError::CorruptLog));
    assert_eq!(store.committed_version(), 0);
    assert!(store
        .try_get(b"__log__/00000000000000000001")
        .unwrap()
        .is_none());
}

#[test]
fn fallible_replay_prepares_all_entries_before_publishing_expansion_failure() {
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"should-not-publish".to_vec(), vec![1]);
        Err("replay expansion failed".into())
    })));
    let first = batch(Uuid::new_v4());
    let second = batch(Uuid::new_v4());
    let log = vec![
        rushdb_types::command_codec::encode(&first),
        rushdb_types::command_codec::encode(&second),
    ];
    assert!(matches!(
        store.try_replay(&log),
        Err(StorageError::Builder(_))
    ));
    assert_eq!(store.committed_version(), 0);
    assert!(store.try_get(b"should-not-publish").unwrap().is_none());
}

#[test]
fn evicted_conflict_summary_is_not_treated_as_no_conflict() {
    // C4: a write set evicted from the bounded in-memory window must surface
    // as the retryable `ConflictHistoryUnavailable`, never as "no conflict".
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, _| Ok(()))));
    let first = Uuid::new_v4();
    store.commit(first, |_, _| Ok((batch(first), ()))).unwrap();
    store.conflicts.lock().unwrap().forget(1);
    let next = Uuid::new_v4();
    assert!(matches!(
        store.commit_serializable(next, 0, DependencySet::default(), |_| Ok((batch(next), ()))),
        Err(StorageError::ConflictHistoryUnavailable { commit_index: 1 })
    ));
    assert_eq!(store.next_commit_index.load(Ordering::SeqCst), 2);
}

#[test]
fn cross_tenant_expansion_observes_the_actual_global_predecessor() {
    let store = Arc::new(Store::new_in_memory());
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        let previous = txn
            .get(b"global-counter")
            .map_err(|e| e.to_string())?
            .map_or(0, |raw| {
                u64::from_be_bytes(raw.as_slice().try_into().unwrap())
            });
        txn.put(
            b"global-counter".to_vec(),
            (previous + 1).to_be_bytes().to_vec(),
        );
        Ok(())
    })));
    let mut workers = Vec::new();
    for tenant in 0..16 {
        let store = store.clone();
        workers.push(std::thread::spawn(move || {
            for _ in 0..10 {
                let key = Uuid::new_v4();
                store
                    .commit_for_tenant(&TenantId::from_ordinal(tenant), key, |_, _| {
                        let mut batch = batch(key);
                        batch.tenant = TenantId::from_ordinal(tenant);
                        Ok((batch, ()))
                    })
                    .unwrap();
            }
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(store.try_get_u64(b"global-counter").unwrap(), Some(160));
    assert_eq!(store.committed_version(), 160);
}

#[test]
fn get_u64_rejects_short_corrupt_values_without_panicking() {
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"short-counter".to_vec(), vec![1, 2, 3]);
        txn.put(b"wide-counter".to_vec(), vec![0; 9]);
        Ok(())
    })));
    let key = Uuid::new_v4();
    store.commit(key, |_, _| Ok((batch(key), ()))).unwrap();
    assert!(matches!(
        store.try_get_u64(b"short-counter"),
        Err(StorageError::CorruptLog)
    ));
    assert!(matches!(
        store.try_get_u64(b"wide-counter"),
        Err(StorageError::CorruptLog)
    ));
    assert!(matches!(
        store.try_get_u64(b"short-counter"),
        Err(StorageError::CorruptLog)
    ));
    assert!(matches!(
        store.try_get_u64(b"wide-counter"),
        Err(StorageError::CorruptLog)
    ));
}

#[test]
fn replicated_apply_cannot_overwrite_a_terminal_abort() {
    let store = Store::new_in_memory();
    let key = Uuid::new_v4();
    store.record_aborted_outcome(key).unwrap();
    assert!(store.apply_replicated_batch(&batch(key)).is_err());
    assert_eq!(store.outcome_for(key), Some(OutcomeKind::Aborted));
    assert_eq!(store.committed_version(), 0);
}

#[test]
fn bounded_writer_splits_large_bursts_into_multiple_groups() {
    // P04 bounded writer: a configured max_group_count must split a large
    // contiguous burst into multiple group-commit transactions while every
    // commit still lands exactly once and the log stays dense.
    let config = resources::StorageResources {
        managed_limit_bytes: 64 * 1024 * 1024,
        page_cache_bytes: 1024 * 1024,
        transaction_pool_bytes: 1024 * 1024,
        durability: resources::Durability::Local,
        shard_count: 1,
        experimental_multi_shard: false,
        max_group_count: 4,
        max_group_bytes: 0,
        max_group_delay_ms: 0,
        logical_cache_bytes: 32 * 1024 * 1024,
        phase_budget_bytes: 64 * 1024 * 1024,
        query_memory_max_bytes: 0,
        lock_acquisition_timeout_ms: 0,
        log_segment_bytes: 64 * 1024 * 1024,
        max_expanded_commit_bytes: 512 * 1024 * 1024,
        ..resources::StorageResources::default()
    };
    let store = Store::new_with_resources(config).unwrap();
    store.set_expander(Arc::new(Box::new(|_, _, _| Ok(()))));
    let n = 20usize;
    for _ in 0..n {
        store
            .commit(Uuid::new_v4(), |_, _| Ok((batch(Uuid::new_v4()), ())))
            .unwrap();
    }
    // All commits landed, contiguous, and the log is dense (length == CI).
    assert_eq!(store.committed_version(), n as u64);
    assert_eq!(store.log_bytes().len(), n);
}

#[test]
fn bounded_writer_byte_cap_splits_groups() {
    // P04 bounded writer byte cap: a max_group_bytes that fits ~2 ops per
    // group must still deliver every commit with a dense log.
    let config = resources::StorageResources {
        managed_limit_bytes: 64 * 1024 * 1024,
        page_cache_bytes: 1024 * 1024,
        transaction_pool_bytes: 1024 * 1024,
        durability: resources::Durability::Local,
        shard_count: 1,
        experimental_multi_shard: false,
        max_group_count: 0,
        max_group_bytes: 128,
        max_group_delay_ms: 0,
        logical_cache_bytes: 32 * 1024 * 1024,
        phase_budget_bytes: 64 * 1024 * 1024,
        query_memory_max_bytes: 0,
        lock_acquisition_timeout_ms: 0,
        log_segment_bytes: 64 * 1024 * 1024,
        max_expanded_commit_bytes: 512 * 1024 * 1024,
        ..resources::StorageResources::default()
    };
    let store = Store::new_with_resources(config).unwrap();
    store.set_expander(Arc::new(Box::new(|_, _, _| Ok(()))));
    let n = 20usize;
    for _ in 0..n {
        store
            .commit(Uuid::new_v4(), |_, _| Ok((batch(Uuid::new_v4()), ())))
            .unwrap();
    }
    assert_eq!(store.committed_version(), n as u64);
    assert_eq!(store.log_bytes().len(), n);
}

#[test]
fn multi_shard_persist_fans_out_to_both_backends_in_parallel() {
    // One drain group spanning two physical shards must persist each
    // shard's op-sets to its own backend file (parallel since disjoint).
    // Deterministic at the persist layer: no thread scheduling involved.
    let dir = std::env::temp_dir().join(format!(
        "rushdb-shard-persist-{}-{}",
        std::process::id(),
        Uuid::now_v7()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let config = resources::StorageResources {
        managed_limit_bytes: 64 * 1024 * 1024,
        page_cache_bytes: 1024 * 1024,
        transaction_pool_bytes: 1024 * 1024,
        durability: resources::Durability::Local,
        shard_count: 2,
        experimental_multi_shard: true,
        max_group_count: 0,
        max_group_bytes: 0,
        max_group_delay_ms: 0,
        logical_cache_bytes: 32 * 1024 * 1024,
        phase_budget_bytes: 64 * 1024 * 1024,
        query_memory_max_bytes: 0,
        lock_acquisition_timeout_ms: 0,
        log_segment_bytes: 64 * 1024 * 1024,
        max_expanded_commit_bytes: 512 * 1024 * 1024,
        ..resources::StorageResources::default()
    };
    let store = Store::new_with_resources(config).unwrap();
    store.attach_redb(&db).unwrap();
    let (ack0, _rx0) = mpsc::channel();
    let (ack1, _rx1) = mpsc::channel();
    let batch = vec![
        PendingCommit {
            ci: 1,
            shard: 0,
            ops: vec![Op::Put(b"shard0-key".to_vec(), b"v0".to_vec())],
            prepared: PreparedCommand::from_ops(
                1,
                &[Op::Put(b"shard0-key".to_vec(), b"v0".to_vec())],
            ),
            command_payload: Vec::new(),
            ack: ack0,
        },
        PendingCommit {
            ci: 2,
            shard: 1,
            ops: vec![Op::Put(b"shard1-key".to_vec(), b"v1".to_vec())],
            prepared: PreparedCommand::from_ops(
                2,
                &[Op::Put(b"shard1-key".to_vec(), b"v1".to_vec())],
            ),
            command_payload: Vec::new(),
            ack: ack1,
        },
    ];
    let pairs: Vec<(u64, &[Op])> = batch.iter().map(|pc| (pc.ci, pc.ops.as_slice())).collect();
    store.persist_many_sharded(&pairs, &batch).unwrap();
    let get = |shard: usize, key: &[u8]| {
        store
            .backend(shard)
            .unwrap()
            .read_view()
            .unwrap()
            .read_value(
                key,
                0,
                true,
                ReadLimits::default(),
                ScanProjection::KeyValue,
            )
            .unwrap()
    };
    assert_eq!(get(0, b"shard0-key"), Some(b"v0".to_vec()));
    assert_eq!(get(1, b"shard1-key"), Some(b"v1".to_vec()));
    // Disjoint files: neither shard sees the other's keys.
    assert_eq!(get(0, b"shard1-key"), None);
    assert_eq!(get(1, b"shard0-key"), None);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn bounded_keys_prefix_stops_at_limit_and_skips_tombstones() {
    // Chunked purge enumerates keys with `keys_prefix_limited`: ascending,
    // bounded, tombstone-aware, and (unlike `keys_prefix`) not subject to the
    // 64 MiB materialization ceiling.
    let store = Store::new_in_memory();
    {
        let mut data = store.data.write().unwrap();
        for i in 0..10u8 {
            data.insert(vec![b'k', i], vec![i]);
        }
    }
    let data = store.data.read().unwrap();
    let mut txn = Txn {
        base: TxnBase::Live(&data),
        buffer: BTreeMap::new(),
        purge_keys: BTreeSet::default(),
        set_keys: BTreeSet::default(),
        deps: None,
    };
    // A staged replacement stays; a staged tombstone drops out.
    txn.put(vec![b'k', 2], vec![99]);
    txn.del(vec![b'k', 3]);
    assert_eq!(
        txn.keys_prefix_limited(b"k", 4).unwrap(),
        vec![vec![b'k', 0], vec![b'k', 1], vec![b'k', 2], vec![b'k', 4]]
    );
    assert_eq!(txn.keys_prefix_limited(b"k", 1).unwrap().len(), 1);
    assert!(txn.keys_prefix_limited(b"k", 0).unwrap().is_empty());
}

#[test]
fn purge_op_erases_value_and_history_without_a_new_version() {
    // Tenant purge erases keys destructively: no superseded version is
    // retained for a value that is intentionally destroyed.
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"k".to_vec(), vec![1]);
        Ok(())
    })));
    store
        .commit(Uuid::new_v4(), |_, _| Ok((batch(Uuid::new_v4()), ())))
        .unwrap();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"k".to_vec(), vec![2]);
        Ok(())
    })));
    store
        .commit(Uuid::new_v4(), |_, _| Ok((batch(Uuid::new_v4()), ())))
        .unwrap();
    assert_eq!(
        store
            .history
            .read()
            .unwrap()
            .get(b"k".as_slice())
            .map(Vec::len),
        Some(1),
        "an overwrite retains one version"
    );
    store
        .persist_ops(&[Op::Purge(b"k".to_vec())], 3, None)
        .unwrap();
    assert!(store.data.read().unwrap().get(b"k".as_slice()).is_none());
    assert!(
        store.history.read().unwrap().get(b"k".as_slice()).is_none(),
        "purge must drop retained versions"
    );
}

#[test]
fn set_op_overwrites_without_retaining_history() {
    // Op::Set is the history-free overwrite used for internal bookkeeping:
    // repeated overwrites must not accumulate MVCC versions, unlike Op::Put.
    let store = Store::new_in_memory();
    store
        .persist_ops(&[Op::Set(b"meta/alloc/x".to_vec(), vec![1])], 1, None)
        .unwrap();
    store
        .persist_ops(&[Op::Set(b"meta/alloc/x".to_vec(), vec![2])], 2, None)
        .unwrap();
    assert_eq!(
        store.data.read().unwrap().get(b"meta/alloc/x".as_slice()),
        Some(&vec![2])
    );
    assert!(
        store
            .history
            .read()
            .unwrap()
            .get(b"meta/alloc/x".as_slice())
            .is_none(),
        "Set must not retain versions"
    );
    store
        .persist_ops(&[Op::Put(b"k".to_vec(), vec![1])], 3, None)
        .unwrap();
    store
        .persist_ops(&[Op::Put(b"k".to_vec(), vec![2])], 4, None)
        .unwrap();
    assert_eq!(
        store
            .history
            .read()
            .unwrap()
            .get(b"k".as_slice())
            .map(Vec::len),
        Some(1),
        "Put still retains a version"
    );
    // A Set after Puts drops the accumulated history for that key.
    store
        .persist_ops(&[Op::Set(b"k".to_vec(), vec![3])], 5, None)
        .unwrap();
    assert!(
        store.history.read().unwrap().get(b"k".as_slice()).is_none(),
        "Set drops pre-existing history"
    );
    assert_eq!(
        store.data.read().unwrap().get(b"k".as_slice()),
        Some(&vec![3])
    );
}

#[test]
fn cache_budget_is_created_from_config_and_charges_the_governor() {
    let config = resources::StorageResources {
        logical_cache_bytes: 1 << 20,
        ..Default::default()
    };
    let store = Store::new_with_resources(config).unwrap();
    let budget = store.cache_budget().expect("budget when enabled");
    assert_eq!(budget.snapshot().limit, 1 << 20);
    let mut cache = crate::cache::LogicalCache::<u64, u64>::new(budget).unwrap();
    assert!(cache.insert(1, 1, 4096));
    assert_eq!(cache.stats().bytes, 4096);
    let shared = store.cache_budget().unwrap();
    let first = shared.try_reserve(700_000).unwrap();
    assert!(shared.try_reserve(700_000).is_err());
    drop(first);
    assert!(shared.try_reserve(700_000).is_ok());

    let zero = resources::StorageResources {
        logical_cache_bytes: 0,
        ..Default::default()
    };
    assert!(
        Store::new_with_resources(zero)
            .unwrap()
            .cache_budget()
            .is_none(),
        "0 disables logical caches"
    );
}

#[test]
fn txn_reads_through_the_sequencer_overlay() {
    // The sequencer expands a commit against committed state plus prior pending
    // ops. That overlay must sit between this transaction's own buffer (highest
    // precedence) and the committed base, for point reads and every scan.
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"k/a".to_vec(), b"base-a".to_vec());
        txn.put(b"k/b".to_vec(), b"base-b".to_vec());
        txn.put(b"k/c".to_vec(), b"base-c".to_vec());
        Ok(())
    })));
    let key = Uuid::new_v4();
    store
        .commit(key, |_, _| Ok((batch(key), ())))
        .expect("commit");

    let base = Arc::new(store.read_view().unwrap());
    // Pending overlay: rewrite `b`, tombstone `c`, add `d`.
    let mut overlay = OverlayView::new(Arc::clone(&base));
    overlay.put(b"k/b".to_vec(), b"pending-b".to_vec());
    overlay.delete(b"k/c".to_vec());
    overlay.put(b"k/d".to_vec(), b"pending-d".to_vec());

    let mut txn = Txn {
        base: TxnBase::Overlay(Arc::new(overlay)),
        buffer: BTreeMap::new(),
        purge_keys: BTreeSet::default(),
        set_keys: BTreeSet::default(),
        deps: None,
    };

    // Point reads: overlay over base; a staged buffer write wins over both.
    assert_eq!(txn.get(b"k/a").unwrap(), Some(b"base-a".to_vec()));
    assert_eq!(txn.get(b"k/b").unwrap(), Some(b"pending-b".to_vec()));
    assert_eq!(
        txn.get(b"k/c").unwrap(),
        None,
        "an overlay tombstone hides the committed row"
    );
    assert_eq!(txn.get(b"k/d").unwrap(), Some(b"pending-d".to_vec()));
    txn.put(b"k/b".to_vec(), b"buffer-b".to_vec());
    assert_eq!(txn.get(b"k/b").unwrap(), Some(b"buffer-b".to_vec()));

    // Scans merge buffer over overlay over base, dropping tombstones.
    assert_eq!(
        txn.range(b"k/", b"k0").unwrap(),
        vec![
            (b"k/a".to_vec(), b"base-a".to_vec()),
            (b"k/b".to_vec(), b"buffer-b".to_vec()),
            (b"k/d".to_vec(), b"pending-d".to_vec()),
        ]
    );
    assert_eq!(
        txn.keys_prefix(b"k/").unwrap(),
        vec![b"k/a".to_vec(), b"k/b".to_vec(), b"k/d".to_vec()]
    );
    assert_eq!(
        txn.keys_prefix_limited(b"k/", 2).unwrap(),
        vec![b"k/a".to_vec(), b"k/b".to_vec()],
        "bounded enumeration stops at the limit across the overlay merge"
    );
}

#[test]
fn committed_mutation_is_idempotent_on_commit_key_retry() {
    // RM-2: the committed/unknown-outcome contract. When a mutation committed
    // but its response was dropped, a retry with the same commit key must
    // discover the recorded outcome instead of re-executing.
    let store = Store::new_in_memory();
    let executions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let exec = Arc::clone(&executions);
    store.set_expander(Arc::new(Box::new(move |_, _, txn| {
        exec.fetch_add(1, Ordering::SeqCst);
        txn.put(b"k".to_vec(), vec![1]);
        Ok(())
    })));
    let key = Uuid::new_v4();
    let first = store.commit(key, |_, _| Ok((batch(key), 7u64))).unwrap();
    let ci = match first {
        CommitOutcome::Committed {
            commit_index,
            payload,
        } => {
            assert_eq!(payload, 7);
            commit_index
        }
        CommitOutcome::Conflicted(reason) => panic!("first commit must succeed: {reason}"),
    };
    // The response is lost; the client retries the same commit key.
    let second = store.commit(key, |_, _| Ok((batch(key), 7u64))).unwrap();
    match second {
        CommitOutcome::Committed {
            commit_index,
            payload,
        } => {
            assert_eq!(
                commit_index, ci,
                "retry must return the original commit index"
            );
            assert_eq!(payload, 7, "retry must return the original payload");
        }
        CommitOutcome::Conflicted(reason) => {
            panic!("retry must discover the committed outcome: {reason}")
        }
    }
    assert_eq!(
        executions.load(Ordering::SeqCst),
        1,
        "the expander must not re-execute on a commit-key retry"
    );
    // Unknown outcome: a never-proposed key is safe to (re)try.
    assert!(store.try_outcome_for(Uuid::new_v4()).unwrap().is_none());
}

#[test]
fn the_conflict_window_is_charged_to_the_managed_budget() {
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"T/some-key".to_vec(), b"v".to_vec());
        Ok(())
    })));
    let before = store.resource_budget().snapshot().used;
    for _ in 0..3 {
        let key = Uuid::new_v4();
        store.commit(key, |_, _| Ok((batch(key), ()))).unwrap();
    }
    let window = store.conflict_window_bytes();
    assert!(window > 0);
    assert!(
        store.resource_budget().snapshot().used >= before + window,
        "the window's bytes are part of the managed budget"
    );
}
