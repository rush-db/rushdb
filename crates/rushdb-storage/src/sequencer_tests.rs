//! Differential safety net for the C-1 sequencer.
//!
//! The sequencer's premise is that a command can be expanded against the
//! committed base plus the *pending* ops of lower-index commits (through
//! `Txn`'s overlay) instead of waiting for those commits to be published. These
//! tests assert that claim byte-for-byte: the overlay expansion must equal the
//! serialized forward path, which expands each command only after its
//! predecessors are committed, and the accumulated overlay must match the
//! really committed state.

use super::*;
use rushdb_types::command::Mutation;

/// Read-modify-write expander. Each `PutRawKey { key, value }` adds the
/// big-endian `i64` delta in `value` to whichever value the transaction view
/// currently holds for `key` (committed base overlaid with prior pending ops)
/// and writes the sum. Reading before writing is exactly the shape the
/// sequencer's overlay has to satisfy.
fn summing_expander() -> Arc<ExpandFn> {
    Arc::new(Box::new(|_ci, batch, txn| {
        for mutation in &batch.mutations {
            let Mutation::PutRawKey { key, value } = mutation else {
                continue;
            };
            let delta = i64::from_be_bytes(
                value
                    .as_slice()
                    .try_into()
                    .map_err(|_| "delta must be 8 bytes".to_string())?,
            );
            let current = txn
                .get(key)
                .map_err(|error| error.to_string())?
                .map_or(0, |raw| {
                    raw.as_slice().try_into().map_or(0, i64::from_be_bytes)
                });
            txn.put(key.clone(), (current + delta).to_be_bytes().to_vec());
        }
        Ok(())
    }))
}

fn delta_batch(commit_key: Uuid, tenant: TenantId, deltas: &[(Vec<u8>, i64)]) -> CommitBatch {
    CommitBatch {
        command_version: rushdb_types::command::COMMAND_VERSION,
        tenant,
        transaction_id: format!("seq-{commit_key}"),
        commit_key,
        start_index: 0,
        isolation: rushdb_types::command::IsolationMode::Snapshot,
        preconditions: vec![],
        mutations: deltas
            .iter()
            .map(|(key, delta)| Mutation::PutRawKey {
                key: key.clone(),
                value: delta.to_be_bytes().to_vec(),
            })
            .collect(),
    }
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// A deterministic mixed workload: a fixed read-modify-write prefix over a
/// small account set followed by a seeded pseudo-random sequence. Repeated keys
/// within one command and overlapping accounts across commands force the
/// overlay to carry state forward correctly.
fn workload() -> Vec<Vec<(Vec<u8>, i64)>> {
    let mut steps: Vec<Vec<(Vec<u8>, i64)>> = vec![
        vec![(b"acct/a".to_vec(), 5)],
        vec![(b"acct/a".to_vec(), -2), (b"acct/b".to_vec(), 7)],
        vec![(b"acct/b".to_vec(), 3), (b"acct/b".to_vec(), 1)],
        vec![(b"acct/c".to_vec(), 9)],
    ];
    let accounts = [b"acct/a".to_vec(), b"acct/b".to_vec(), b"acct/c".to_vec()];
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    for _ in 0..28 {
        let count = 1 + usize::try_from(xorshift(&mut state) % 3).unwrap_or(0);
        let mut step = Vec::with_capacity(count);
        for _ in 0..count {
            let index = usize::try_from(xorshift(&mut state) % 3).unwrap_or(0);
            let delta = i64::try_from(xorshift(&mut state) % 7).unwrap_or(0) - 3;
            step.push((accounts[index].clone(), delta));
        }
        steps.push(step);
    }
    steps
}

fn op_key(op: &Op) -> &[u8] {
    match op {
        Op::Put(key, _) | Op::Set(key, _) | Op::Del(key) | Op::Purge(key) => key,
    }
}

/// Fold the data (non-coordination) effect of an op-set into the pending
/// overlay, exactly as the sequencer will between commands.
fn apply_net(pending: &mut BTreeMap<Vec<u8>, Option<Vec<u8>>>, ops: &[Op]) {
    for op in ops {
        if is_coordination_write(op_key(op)) {
            continue;
        }
        match op {
            Op::Put(key, value) | Op::Set(key, value) => {
                pending.insert(key.clone(), Some(value.clone()));
            }
            Op::Del(key) | Op::Purge(key) => {
                pending.insert(key.clone(), None);
            }
        }
    }
}

/// Commit index of the newest in-flight layer that writes `key`.
fn latest_writer(state: &SequencerState, key: &[u8]) -> Option<u64> {
    state
        .layers
        .iter()
        .rev()
        .find(|layer| layer.get(key).is_some())
        .map(|layer| layer.ci())
}

#[test]
fn sequencer_buffer_trims_published_and_tracks_in_flight() {
    let store = Store::new_in_memory();
    store.set_expander(summing_expander());
    let tenant = TenantId::from_ordinal(1);

    let first = Uuid::new_v4();
    let batch = delta_batch(first, tenant, &[(b"acct".to_vec(), 5)]);
    store
        .commit(first, |_, _| Ok((batch.clone(), ())))
        .expect("first commit");
    // The just-published entry stays in the buffer until the next critical
    // section trims it (the base then already contains it).
    {
        let state = store.admission_lock.lock().unwrap();
        assert_eq!(latest_writer(&state, b"acct"), Some(1));
    }

    let second = Uuid::new_v4();
    let batch = delta_batch(second, tenant, &[(b"acct".to_vec(), 3)]);
    store
        .commit(second, |_, _| Ok((batch.clone(), ())))
        .expect("second commit");
    {
        let state = store.admission_lock.lock().unwrap();
        assert_eq!(
            latest_writer(&state, b"acct"),
            Some(2),
            "the published entry is trimmed and the new in-flight one recorded"
        );
        assert_eq!(state.layers.len(), 1, "one layer per in-flight commit");
    }
}

#[test]
fn concurrent_commits_coalesce_into_group_commits() {
    // C-1 payoff: releasing admission/shard before the durability wait lets
    // concurrent commits accumulate in `pending` so one `drain_and_publish`
    // persists many of them. A linger widens the join window deterministically.
    let config = resources::StorageResources {
        max_group_delay_ms: 20,
        ..Default::default()
    };
    let store = Arc::new(Store::new_with_resources(config).unwrap());
    // Each commit writes a distinct key (so the publisher's write-write barrier
    // does not split the group) and reads nothing (dependency-free).
    store.set_expander(Arc::new(Box::new(|_ci, batch, txn| {
        let mut key = b"key/".to_vec();
        key.extend_from_slice(batch.commit_key.as_bytes());
        txn.put(key, vec![1]);
        Ok(())
    })));
    let tenant = TenantId::from_ordinal(1);
    let threads = 8usize;
    let per_thread = 50usize;
    let barrier = Arc::new(std::sync::Barrier::new(threads));
    let mut handles = Vec::new();
    for _ in 0..threads {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            for _ in 0..per_thread {
                let commit_key = Uuid::new_v4();
                let batch = delta_batch(commit_key, tenant, &[]);
                store
                    .commit(commit_key, |_, _| Ok((batch.clone(), ())))
                    .expect("concurrent commit");
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let total = (threads * per_thread) as u64;
    assert_eq!(store.committed_version(), total);
    let groups = store.group_commits.load(Ordering::Relaxed);
    assert!(
        groups < total,
        "expected coalescing: {groups} group-commits for {total} commits"
    );
}

/// Read-dependent commits of one tenant (every one reads and
/// rewrites the same counter) expanded against the sequencer overlay must
/// still coalesce into shared publish groups — before, the publisher barrier
/// gave each its own fsync — and the counter must stay exact.
#[test]
fn dependent_sequenced_commits_share_publish_groups_and_stay_exact() {
    let config = resources::StorageResources {
        max_group_delay_ms: 20,
        ..Default::default()
    };
    let store = Arc::new(Store::new_with_resources(config).unwrap());
    store.set_expander(summing_expander());
    let tenant = TenantId::from_ordinal(1);
    let threads = 8usize;
    let per_thread = 50usize;
    let barrier = Arc::new(std::sync::Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let (store, barrier) = (Arc::clone(&store), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..per_thread {
                    let commit_key = Uuid::new_v4();
                    let batch = delta_batch(commit_key, tenant, &[(b"acct/hot".to_vec(), 1)]);
                    store
                        .commit(commit_key, |_, _| Ok((batch.clone(), ())))
                        .expect("dependent commit");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let total = (threads * per_thread) as u64;
    assert_eq!(store.committed_version(), total);
    let hot = store.try_get(b"acct/hot").unwrap().unwrap();
    assert_eq!(
        i64::from_be_bytes(hot.as_slice().try_into().unwrap()),
        i64::try_from(total).unwrap(),
        "every dependent increment must land exactly once"
    );
    let groups = store.group_commits.load(Ordering::Relaxed);
    assert!(
        groups * 2 < total,
        "dependent commits must coalesce: {groups} groups for {total} commits"
    );
}

#[test]
fn overlay_expansion_matches_the_committed_forward_path() {
    let store = Store::new_in_memory();
    store.set_expander(summing_expander());
    // Receipts stamp their creation time; the two expansions of one commit
    // must agree on it to be byte-identical.
    store.set_receipt_clock(Arc::new(|| 1_000));
    let tenant = TenantId::from_ordinal(7);
    let workload = workload();
    let accounts: [&[u8]; 3] = [b"acct/a", b"acct/b", b"acct/c"];

    // Forward path: commit every command for real, recording its ci/batch.
    let mut committed: Vec<(u64, CommitBatch, String)> = Vec::new();
    for deltas in &workload {
        let commit_key = Uuid::new_v4();
        let batch = delta_batch(commit_key, tenant, deltas);
        let payload = format!("payload-{}", committed.len());
        let outcome = store
            .commit(commit_key, |_, _| Ok((batch.clone(), payload.clone())))
            .expect("commit");
        let ci = match outcome {
            CommitOutcome::Committed { commit_index, .. } => commit_index,
            CommitOutcome::Conflicted(reason) => panic!("unexpected conflict: {reason}"),
        };
        committed.push((ci, batch, payload));
    }
    assert_eq!(committed.len(), workload.len());

    // Sequencer path: start from the pre-sequence base and carry each command's
    // expanded ops forward as the pending overlay, exactly as the sequencer
    // will. Compare against the committed predecessor and the real state.
    let base_ci = committed.first().map_or(0, |(ci, ..)| ci - 1);
    let mut pending: BTreeMap<Vec<u8>, Option<Vec<u8>>> = BTreeMap::new();
    let mut layers: Vec<Arc<overlay_view::CommitLayer>> = Vec::new();
    for (step, (ci, batch, payload)) in committed.iter().enumerate() {
        let overlay = Arc::new(OverlayView::with_buffer(
            Arc::new(store.read_at(base_ci).expect("base view")),
            pending.clone(),
        ));
        let overlay_ops = store
            .expand_against_overlay(*ci, batch, payload, Arc::clone(&overlay))
            .expect("overlay expansion");

        // The serialized forward path expands each command against its single
        // committed predecessor only.
        let predecessor = Arc::new(OverlayView::new(Arc::new(
            store.read_at(ci - 1).expect("predecessor view"),
        )));
        let forward_ops = store
            .expand_against_overlay(*ci, batch, payload, predecessor)
            .expect("forward expansion");

        assert_eq!(
            overlay_ops, forward_ops,
            "overlay expansion diverged from the committed forward path at step {step} (ci {ci})"
        );

        // The sequencer's shared per-commit layers expand identically.
        let layered = Arc::new(OverlayView::with_layers(
            Arc::new(store.read_at(base_ci).expect("base view")),
            layers.clone(),
        ));
        let layered_ops = store
            .expand_against_overlay(*ci, batch, payload, Arc::clone(&layered))
            .expect("layered expansion");
        assert_eq!(
            layered_ops, forward_ops,
            "layered expansion diverged from the committed forward path at step {step} (ci {ci})"
        );
        layers.push(Arc::new(overlay_view::CommitLayer::new(
            *ci,
            Arc::new(layered_ops),
        )));

        apply_net(&mut pending, &overlay_ops);

        // After folding this command's ops, the accumulated overlay must match
        // the really committed state for every account the workload touched.
        let predicted = OverlayView::with_buffer(
            Arc::new(store.read_at(base_ci).expect("base view")),
            pending.clone(),
        );
        let real = store.read_at(*ci).expect("real view");
        for account in accounts {
            assert_eq!(
                predicted.get(account).unwrap(),
                real.get(account).unwrap(),
                "overlay state diverged from committed state for {:?} at ci {ci}",
                String::from_utf8_lossy(account)
            );
        }
    }
}

#[test]
fn commit_layer_keeps_the_last_write_per_key_in_key_order() {
    let layer = overlay_view::CommitLayer::new(
        7,
        Arc::new(vec![
            Op::Put(b"k/b".to_vec(), b"1".to_vec()),
            Op::Put(b"k/a".to_vec(), b"1".to_vec()),
            Op::Del(b"k/b".to_vec()),
            Op::Set(b"k/c".to_vec(), b"3".to_vec()),
            Op::Put(b"k/a".to_vec(), b"2".to_vec()),
        ]),
    );
    assert_eq!(layer.ci(), 7);
    assert_eq!(layer.len(), 3);
    assert_eq!(layer.get(b"k/a"), Some(Some(b"2".as_slice())));
    assert_eq!(layer.get(b"k/b"), Some(None), "a later delete wins");
    assert_eq!(layer.get(b"k/c"), Some(Some(b"3".as_slice())));
    assert_eq!(layer.get(b"k/d"), None);
    assert_eq!(
        layer.keys().collect::<Vec<_>>(),
        vec![b"k/a".as_slice(), b"k/b", b"k/c"]
    );
}

#[test]
fn layered_overlay_reads_newest_layer_then_buffer_over_base() {
    let store = Store::new_in_memory();
    store.set_expander(Arc::new(Box::new(|_, _, txn| {
        txn.put(b"k/a".to_vec(), b"base-a".to_vec());
        txn.put(b"k/b".to_vec(), b"base-b".to_vec());
        txn.put(b"k/c".to_vec(), b"base-c".to_vec());
        Ok(())
    })));
    let key = Uuid::new_v4();
    let tenant = TenantId::from_ordinal(1);
    store
        .commit(key, |_, _| Ok((delta_batch(key, tenant, &[]), ())))
        .expect("commit");
    let base = Arc::new(store.read_view().unwrap());
    let older = Arc::new(overlay_view::CommitLayer::new(
        10,
        Arc::new(vec![
            Op::Put(b"k/b".to_vec(), b"older-b".to_vec()),
            Op::Put(b"k/d".to_vec(), b"older-d".to_vec()),
            Op::Put(b"k/e".to_vec(), b"older-e".to_vec()),
        ]),
    ));
    let newer = Arc::new(overlay_view::CommitLayer::new(
        11,
        Arc::new(vec![
            Op::Del(b"k/c".to_vec()),
            Op::Put(b"k/d".to_vec(), b"newer-d".to_vec()),
            Op::Del(b"k/e".to_vec()),
        ]),
    ));
    let mut overlay = OverlayView::with_layers(base, vec![older, newer]);
    overlay.put(b"k/b".to_vec(), b"own-b".to_vec());
    overlay.put(b"k/e".to_vec(), b"own-e".to_vec());

    assert_eq!(overlay.get(b"k/a").unwrap(), Some(b"base-a".to_vec()));
    assert_eq!(overlay.get(b"k/b").unwrap(), Some(b"own-b".to_vec()));
    assert_eq!(
        overlay.get(b"k/c").unwrap(),
        None,
        "a layer tombstone hides base"
    );
    assert_eq!(overlay.get(b"k/d").unwrap(), Some(b"newer-d".to_vec()));
    assert_eq!(overlay.get(b"k/e").unwrap(), Some(b"own-e".to_vec()));
    let expected = vec![
        (b"k/a".to_vec(), b"base-a".to_vec()),
        (b"k/b".to_vec(), b"own-b".to_vec()),
        (b"k/d".to_vec(), b"newer-d".to_vec()),
        (b"k/e".to_vec(), b"own-e".to_vec()),
    ];
    assert_eq!(overlay.range_prefix(b"k/").unwrap(), expected);
    assert_eq!(overlay.range(b"k/", b"k0").unwrap(), expected);
    assert_eq!(
        overlay.range(b"k/b", b"k/e").unwrap(),
        expected[1..3].to_vec(),
        "range bounds apply to staged rows too"
    );
    assert_eq!(
        overlay.range_keys(b"k/", b"k0").unwrap(),
        expected
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>()
    );

    let mut txn = Txn {
        base: TxnBase::Overlay(Arc::new(overlay)),
        buffer: BTreeMap::new(),
        purge_keys: BTreeSet::default(),
        set_keys: BTreeSet::default(),
        watermark_keys: BTreeSet::default(),
        ids: &store.ids,
        deps: None,
    };
    txn.del(b"k/a".to_vec());
    assert_eq!(
        txn.keys_prefix_limited(b"k/", 10).unwrap(),
        vec![b"k/b".to_vec(), b"k/d".to_vec(), b"k/e".to_vec()]
    );
}

#[test]
fn admitted_commit_shares_its_ops_with_the_sequencer_layer() {
    // The pending entry the publisher drains and the layer later commands
    // read are one allocation: admission copies no key or value.
    let store = Arc::new(Store::new_in_memory());
    store.set_expander(summing_expander());
    let publishing = store.publish_lock.lock().unwrap();
    let worker = {
        let store = Arc::clone(&store);
        std::thread::spawn(move || {
            let key = Uuid::new_v4();
            let batch = delta_batch(key, TenantId::from_ordinal(1), &[(b"acct".to_vec(), 4)]);
            store.commit(key, |_, _| Ok((batch.clone(), ()))).unwrap();
        })
    };
    let mut pending = store.pending.lock().unwrap();
    while !pending.contains_key(&1) {
        pending = store
            .pending_cv
            .wait_timeout(pending, std::time::Duration::from_secs(2))
            .unwrap()
            .0;
    }
    {
        let state = store.admission_lock.lock().unwrap();
        assert_eq!(state.layers.len(), 1);
        assert!(Arc::ptr_eq(&pending[&1].ops, state.layers[0].ops()));
    }
    drop(pending);
    drop(publishing);
    worker.join().unwrap();
    assert_eq!(
        store.try_get(b"acct").unwrap(),
        Some(4_i64.to_be_bytes().to_vec())
    );
}
