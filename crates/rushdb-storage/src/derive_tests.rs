//! Apply-time derivation at the storage layer, with a test deriver
//! that keeps one counter row per base key prefix.

use super::*;
use rushdb_types::command::Mutation;

const COUNT_PREFIX: &[u8] = b"count/";

/// Counts live `base/<group>/…` keys per group in `count/<group>`: a
/// read-before-write derivation, like presence and dictionary refcounts.
struct CountingDeriver;

fn group_of(key: &[u8]) -> Option<&[u8]> {
    let rest = key.strip_prefix(b"base/")?;
    let end = rest.iter().position(|byte| *byte == b'/')?;
    Some(&rest[..end])
}

impl Deriver for CountingDeriver {
    fn derive(
        &self,
        _ci: u64,
        ops: &[Op],
        before: &dyn DeriveRead,
    ) -> Result<Vec<Op>, StorageError> {
        let mut seen: BTreeMap<Vec<u8>, Option<Vec<u8>>> = BTreeMap::new();
        let mut deltas: BTreeMap<Vec<u8>, i64> = BTreeMap::new();
        for op in ops {
            let (key, live) = match op {
                Op::Put(key, _) | Op::Set(key, _) => (key, true),
                Op::Del(key) | Op::Purge(key) => (key, false),
            };
            let Some(group) = group_of(key) else {
                continue;
            };
            let existed = match seen.get(key) {
                Some(state) => state.is_some(),
                None => before.get(key)?.is_some(),
            };
            seen.insert(key.clone(), live.then(Vec::new));
            let delta = i64::from(live) - i64::from(existed);
            if delta != 0 {
                let mut counter = COUNT_PREFIX.to_vec();
                counter.extend_from_slice(group);
                *deltas.entry(counter).or_default() += delta;
            }
        }
        let mut out = Vec::new();
        for (counter, delta) in deltas {
            let current = before.get(&counter)?.map_or(0, |raw| {
                i64::from_be_bytes(raw.as_slice().try_into().unwrap_or([0; 8]))
            });
            out.push(Op::Put(counter, (current + delta).to_be_bytes().to_vec()));
        }
        Ok(out)
    }

    fn is_derived_key(&self, key: &[u8]) -> bool {
        key.starts_with(COUNT_PREFIX)
    }

    fn prefix_reaches_derived(&self, prefix: &[u8]) -> bool {
        COUNT_PREFIX.starts_with(prefix) || prefix.starts_with(COUNT_PREFIX)
    }
}

fn put_expander() -> Arc<ExpandFn> {
    Arc::new(Box::new(|_ci, batch, txn| {
        for mutation in &batch.mutations {
            match mutation {
                Mutation::PutRawKey { key, value } => txn.put(key.clone(), value.clone()),
                Mutation::DeleteRawKey { key } => txn.del(key.clone()),
                _ => {}
            }
        }
        Ok(())
    }))
}

fn raw_batch(commit_key: Uuid, puts: &[Vec<u8>], deletes: &[Vec<u8>]) -> CommitBatch {
    let mut mutations: Vec<Mutation> = puts
        .iter()
        .map(|key| Mutation::PutRawKey {
            key: key.clone(),
            value: vec![1],
        })
        .collect();
    mutations.extend(
        deletes
            .iter()
            .map(|key| Mutation::DeleteRawKey { key: key.clone() }),
    );
    CommitBatch {
        command_version: rushdb_types::command::COMMAND_VERSION,
        tenant: TenantId::from_ordinal(1),
        transaction_id: format!("derive-{commit_key}"),
        commit_key,
        start_index: 0,
        isolation: rushdb_types::command::IsolationMode::Snapshot,
        preconditions: vec![],
        mutations,
    }
}

fn commit(store: &Store, puts: &[Vec<u8>], deletes: &[Vec<u8>]) -> u64 {
    let commit_key = Uuid::new_v4();
    let batch = raw_batch(commit_key, puts, deletes);
    match store
        .commit(commit_key, |_, _| Ok((batch.clone(), ())))
        .expect("commit")
    {
        CommitOutcome::Committed { commit_index, .. } => commit_index,
        CommitOutcome::Conflicted(reason) => panic!("unexpected conflict: {reason}"),
    }
}

fn count(view: &ReadView, group: &str) -> i64 {
    let mut key = COUNT_PREFIX.to_vec();
    key.extend_from_slice(group.as_bytes());
    view.get(&key).unwrap().map_or(0, |raw| {
        i64::from_be_bytes(raw.as_slice().try_into().unwrap())
    })
}

fn base_key(group: &str, id: usize) -> Vec<u8> {
    format!("base/{group}/{id:06}").into_bytes()
}

fn durable_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rushdb-derive-{label}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn durable_store(dir: &std::path::Path) -> Store {
    let store = Store::open_with_deriver(
        &dir.join("rushdb.redb"),
        resources::StorageResources::default(),
        Some(Arc::new(CountingDeriver)),
    )
    .unwrap();
    store.set_expander(put_expander());
    store
}

#[test]
fn derived_ops_publish_atomically_with_base_at_the_same_ci() {
    let store = Store::new_in_memory();
    store.set_expander(put_expander());
    store.set_deriver(Arc::new(CountingDeriver));
    commit(
        &store,
        &[base_key("a", 1), base_key("a", 2), base_key("b", 1)],
        &[],
    );
    let after_first = store.read_view().unwrap();
    commit(&store, &[base_key("a", 3)], &[base_key("a", 1)]);
    let after_second = store.read_view().unwrap();
    assert_eq!((count(&after_first, "a"), count(&after_first, "b")), (2, 1));
    // Derived rows advance with their base at the same commit index, and a
    // view pinned before the second commit keeps the earlier counts.
    assert_eq!(
        (count(&after_second, "a"), count(&after_second, "b")),
        (2, 1)
    );
    commit(&store, &[], &[base_key("a", 2), base_key("a", 3)]);
    assert_eq!(count(&store.read_view().unwrap(), "a"), 0);
    assert_eq!(count(&after_first, "a"), 2, "pinned view is snapshot-exact");
}

#[test]
fn derived_counters_stay_exact_under_concurrent_grouped_commits() {
    let config = resources::StorageResources {
        max_group_delay_ms: 5,
        ..Default::default()
    };
    let store = Arc::new(Store::new_with_resources(config).unwrap());
    store.set_expander(put_expander());
    store.set_deriver(Arc::new(CountingDeriver));
    let threads = 16usize;
    let per_thread = 40usize;
    let handles: Vec<_> = (0..threads)
        .map(|thread| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                for i in 0..per_thread {
                    commit(&store, &[base_key("hot", thread * per_thread + i)], &[]);
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(
        count(&store.read_view().unwrap(), "hot"),
        i64::try_from(threads * per_thread).unwrap()
    );
}

#[test]
fn wal_contains_base_ops_only_and_replay_rederives_identical_state() {
    let dir = durable_dir("replay");
    let mut expected = Vec::new();
    {
        let store = durable_store(&dir);
        for round in 0..20 {
            commit(&store, &[base_key("g", round), base_key("h", round)], &[]);
            if round % 3 == 0 {
                commit(&store, &[], &[base_key("h", round)]);
            }
        }
        let view = store.read_view().unwrap();
        expected.push(count(&view, "g"));
        expected.push(count(&view, "h"));
        let log = store.log.lock().unwrap();
        let records = log.as_ref().unwrap().read_from(1).unwrap();
        let mut assembler = wal::ChainAssembler::default();
        for (_, raw) in records {
            let wal::ChainFeed::Complete(header, body) = assembler.feed(&raw, 0).unwrap() else {
                continue;
            };
            let (_, ops, _) = wal::decode_logical_body(header, &body).unwrap();
            assert!(
                ops.iter().all(|op| !match op {
                    Op::Put(key, _) | Op::Set(key, _) | Op::Del(key) | Op::Purge(key) =>
                        key.starts_with(COUNT_PREFIX),
                }),
                "derived rows must never be logged"
            );
        }
    }
    // Recover from the WAL alone: a fresh materialization next to the same
    // log replays every commit and re-derives the counters.
    let replay_dir = durable_dir("replay-copy");
    let wal_from = dir.join("rushdb.redb.wal");
    let wal_to = replay_dir.join("rushdb.redb.wal");
    std::fs::create_dir_all(&wal_to).unwrap();
    for entry in std::fs::read_dir(&wal_from).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), wal_to.join(entry.file_name())).unwrap();
    }
    let recovered = durable_store(&replay_dir);
    let view = recovered.read_view().unwrap();
    assert_eq!(vec![count(&view, "g"), count(&view, "h")], expected);
    drop(view);
    drop(recovered);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&replay_dir);
}

#[test]
fn replay_without_a_deriver_fences_the_store_once_one_is_installed() {
    let dir = durable_dir("underived");
    {
        let store = durable_store(&dir);
        for round in 0..5 {
            commit(&store, &[base_key("g", round)], &[]);
        }
    }
    // Recover a copy from the WAL alone without a deriver: base rows are
    // applied but their derived rows are missing.
    let replay_dir = durable_dir("underived-copy");
    let wal_from = dir.join("rushdb.redb.wal");
    let wal_to = replay_dir.join("rushdb.redb.wal");
    std::fs::create_dir_all(&wal_to).unwrap();
    for entry in std::fs::read_dir(&wal_from).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), wal_to.join(entry.file_name())).unwrap();
    }
    let store = Store::open_with_deriver(
        &replay_dir.join("rushdb.redb"),
        resources::StorageResources::default(),
        None,
    )
    .unwrap();
    store.health.check().unwrap();
    // Installing the deriver afterwards cannot repair the missing rows, so
    // the store refuses to serve until reopened with the deriver.
    store.set_deriver(Arc::new(CountingDeriver));
    assert!(store.health.check().is_err());
    drop(store);
    let recovered = durable_store(&replay_dir);
    assert_eq!(count(&recovered.read_view().unwrap(), "g"), 0);
    drop(recovered);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&replay_dir);
}
