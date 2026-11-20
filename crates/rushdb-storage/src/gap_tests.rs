//! PUB-1 differential tests: reads served from a gap (published, not yet
//! materialized commits) answer byte for byte what the same reads answer
//! once the commits are in redb, at every snapshot, for point reads, scans
//! in both projections, ranges and `last_key`.

use super::*;
use rushdb_types::command::CommitBatch;
use rushdb_types::ids::TenantId;
use rushdb_types::keys::{tenant_prefix, usage_bytes_key};
use std::ops::Bound;

/// `(kind, key, value)`: `P`ut, `D`el, `S`et, `X` purge.
type RawOp = (char, Vec<u8>, Vec<u8>);

fn configure(store: &Store) {
    store.set_expander(Arc::new(Box::new(|_, batch, txn| {
        let ops: Vec<RawOp> = serde_json::from_str(&batch.transaction_id).unwrap();
        for (kind, key, value) in ops {
            match kind {
                'P' => txn.put(key, value),
                'D' => txn.del(key),
                'S' => txn.set(key, value),
                _ => txn.purge(key),
            }
        }
        Ok(())
    })));
}

fn commit(store: &Store, tenant: u32, ops: &[RawOp]) -> u64 {
    let key = Uuid::new_v4();
    match store
        .commit(key, |_, _| {
            Ok((
                CommitBatch {
                    command_version: rushdb_types::command::COMMAND_VERSION,
                    tenant: TenantId::from_ordinal(u64::from(tenant)),
                    transaction_id: serde_json::to_string(ops).unwrap(),
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
        CommitOutcome::Conflicted(reason) => panic!("unexpected conflict: {reason}"),
    }
}

/// xorshift64*: deterministic per seed, no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const TENANTS: u32 = 2;
const KEYS: u64 = 12;

fn key(tenant: u32, n: u64) -> Vec<u8> {
    let mut key = tenant_prefix(TenantId::from_ordinal(u64::from(tenant))).to_vec();
    key.extend_from_slice(format!("k{n:02}").as_bytes());
    key
}

fn random_ops(rng: &mut Rng, tenant: u32) -> Vec<RawOp> {
    (0..=rng.below(4))
        .map(|_| {
            let key = if rng.below(10) == 0 {
                // A key outside every tenant (no usage accounting).
                format!("g/{}", rng.below(3)).into_bytes()
            } else {
                key(tenant, rng.below(KEYS))
            };
            let value = vec![
                u8::try_from(rng.below(250)).unwrap();
                1 + usize::try_from(rng.below(40)).unwrap()
            ];
            let kind = match rng.below(10) {
                0..=4 => 'P',
                5 | 6 => 'D',
                7 | 8 => 'S',
                _ => 'X',
            };
            (kind, key, value)
        })
        .collect()
}

fn scan(view: &ReadView, bounds: ScanBounds, projection: ScanProjection) -> Vec<ScanRow> {
    let mut cursor = view.scan(bounds, projection).unwrap();
    let mut rows = Vec::new();
    loop {
        let batch = cursor.next_batch(3, 1 << 20).unwrap();
        rows.extend(batch.rows);
        if batch.done {
            return rows;
        }
    }
}

/// Everything the tests compare, read through `view`.
#[derive(Debug, PartialEq, Eq)]
struct Answers {
    ci: u64,
    gets: Vec<Option<Vec<u8>>>,
    scans: Vec<Vec<ScanRow>>,
    ranges: Vec<Vec<(Vec<u8>, Vec<u8>)>>,
    last: Vec<Option<Vec<u8>>>,
}

fn answers(view: &ReadView) -> Answers {
    let mut keys: Vec<Vec<u8>> = (1..=TENANTS)
        .flat_map(|tenant| {
            (0..KEYS)
                .map(move |n| key(tenant, n))
                .chain([usage_bytes_key(TenantId::from_ordinal(u64::from(tenant)))])
        })
        .collect();
    keys.extend((0..3).map(|n| format!("g/{n}").into_bytes()));
    let prefixes: Vec<Vec<u8>> = (1..=TENANTS)
        .map(|tenant| tenant_prefix(TenantId::from_ordinal(u64::from(tenant))).to_vec())
        .chain([b"g/".to_vec()])
        .collect();
    Answers {
        ci: view.commit_index(),
        gets: keys.iter().map(|key| view.get(key).unwrap()).collect(),
        scans: prefixes
            .iter()
            .flat_map(|prefix| {
                [ScanProjection::KeyValue, ScanProjection::Keys]
                    .map(|projection| scan(view, ScanBounds::prefix(prefix), projection))
            })
            // Ranges with every bound kind, starting and ending on keys
            // that exist, that are gone and that never existed.
            .chain(
                (1..=TENANTS)
                    .flat_map(|tenant| {
                        (0..KEYS).step_by(3).flat_map(move |n| {
                            [
                                (
                                    Bound::Included(key(tenant, n)),
                                    Bound::Excluded(key(tenant, n + 5)),
                                ),
                                (
                                    Bound::Excluded(key(tenant, n)),
                                    Bound::Included(key(tenant, n + 4)),
                                ),
                                (
                                    Bound::Excluded(key(tenant, n)),
                                    Bound::Excluded(b"receipt".to_vec()),
                                ),
                            ]
                        })
                    })
                    .map(|(start, end)| {
                        scan(view, ScanBounds { start, end }, ScanProjection::KeyValue)
                    }),
            )
            // Every row up to the receipts (keyed by random commit keys).
            .chain([scan(
                view,
                ScanBounds {
                    start: Bound::Unbounded,
                    end: Bound::Excluded(b"receipt".to_vec()),
                },
                ScanProjection::KeyValue,
            )])
            .collect(),
        ranges: prefixes
            .iter()
            .map(|prefix| view.range_prefix(prefix).unwrap())
            .collect(),
        last: prefixes
            .iter()
            .map(|prefix| view.last_key(ScanBounds::prefix(prefix)).unwrap())
            .collect(),
    }
}

/// Assert two answer lists match, reporting the first difference.
#[track_caller]
fn assert_same(got: &[Answers], want: &[Answers], context: &str) {
    assert_eq!(got.len(), want.len(), "{context}: snapshot count");
    for (got, want) in got.iter().zip(want) {
        let ci = want.ci;
        assert_eq!(got.ci, ci, "{context}");
        for (field, a, b) in [
            (
                "gets",
                format!("{:?}", got.gets),
                format!("{:?}", want.gets),
            ),
            (
                "last",
                format!("{:?}", got.last),
                format!("{:?}", want.last),
            ),
            (
                "ranges",
                format!("{:?}", got.ranges),
                format!("{:?}", want.ranges),
            ),
        ] {
            assert!(a == b, "{context}: ci {ci} {field}\n got {a}\nwant {b}");
        }
        for (index, (a, b)) in got.scans.iter().zip(&want.scans).enumerate() {
            assert!(
                a == b,
                "{context}: ci {ci} scan {index}\n got {a:?}\nwant {b:?}"
            );
        }
    }
}

fn open(name: &str) -> (crate::TempPath, Store) {
    let path = crate::TempPath::store(name);
    let store = Store::open(&path).unwrap();
    configure(&store);
    (path, store)
}

/// Answers at every snapshot `1..=published`, and at the current one.
fn every_snapshot(store: &Store) -> Vec<Answers> {
    let published = store.committed_version();
    (1..=published)
        .map(|ci| answers(&store.read_at(ci).unwrap()))
        .chain([answers(&store.read_view().unwrap())])
        .collect()
}

#[test]
fn gap_reads_equal_materialized_reads_at_every_snapshot() {
    for seed in 1..=6u64 {
        let (path, store) = open(&format!("gap-diff-{seed}"));
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        // Views opened along the way, read again once all is materialized.
        let mut held: Vec<(ReadView, Answers)> = Vec::new();
        for round in 0..6 {
            store.pause_materialization(true);
            for _ in 0..12 {
                let tenant = 1 + u32::try_from(rng.below(u64::from(TENANTS))).unwrap();
                commit(&store, tenant, &random_ops(&mut rng, tenant));
                if rng.below(4) == 0 {
                    let view = store.read_view().unwrap();
                    let seen = answers(&view);
                    held.push((view, seen));
                }
            }
            // Some rounds leave part of the gap materialized.
            if round % 2 == 1 {
                store.pause_materialization(false);
            }
            let from_gap = every_snapshot(&store);
            store.pause_materialization(false);
            store.wait_materialized().unwrap();
            assert_eq!(store.unmaterialized_bytes(), 0);
            assert_eq!(
                from_gap,
                every_snapshot(&store),
                "seed {seed} round {round}"
            );
        }
        for (view, seen) in &held {
            assert_eq!(&answers(view), seen, "seed {seed}: a held view changed");
        }
        drop(held);
        drop(store);
        crate::remove_store(&path).unwrap();
    }
}

#[test]
fn gap_reads_equal_a_store_that_materializes_every_commit() {
    let (gap_path, gap) = open("gap-vs-inline-gap");
    let (inline_path, inline) = open("gap-vs-inline-inline");
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    gap.pause_materialization(true);
    for step in 0..120 {
        let tenant = 1 + u32::try_from(rng.below(u64::from(TENANTS))).unwrap();
        let ops = random_ops(&mut rng, tenant);
        let a = commit(&gap, tenant, &ops);
        let b = commit(&inline, tenant, &ops);
        inline.wait_materialized().unwrap();
        assert_eq!(a, b);
        if step % 10 == 9 {
            assert_eq!(
                answers(&gap.read_view().unwrap()),
                answers(&inline.read_view().unwrap()),
                "step {step}"
            );
        }
        if step == 60 {
            gap.pause_materialization(false);
        }
        if step == 80 {
            gap.pause_materialization(true);
        }
    }
    assert_same(&every_snapshot(&gap), &every_snapshot(&inline), "");
    gap.pause_materialization(false);
    drop((gap, inline));
    crate::remove_store(&gap_path).unwrap();
    crate::remove_store(&inline_path).unwrap();
}

#[test]
fn concurrent_groups_read_the_same_from_the_gap_and_from_redb() {
    // Concurrent committers form multi-commit publish groups, whose derive
    // and usage rows read earlier commits of the same group.
    let (path, store) = open("gap-groups");
    let store = Arc::new(store);
    store.pause_materialization(true);
    let writers: Vec<_> = (0..4u64)
        .map(|writer| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                let mut rng = Rng(0xA076_1D64_78BD_642F ^ (writer + 1));
                for _ in 0..40 {
                    let tenant = 1 + u32::try_from(rng.below(u64::from(TENANTS))).unwrap();
                    commit(&store, tenant, &random_ops(&mut rng, tenant));
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let from_gap = every_snapshot(&store);
    store.pause_materialization(false);
    store.wait_materialized().unwrap();
    assert_same(&from_gap, &every_snapshot(&store), "");
    drop(store);
    crate::remove_store(&path).unwrap();
}

#[test]
fn publishers_wait_at_the_gap_bound_instead_of_growing_it() {
    let path = crate::TempPath::store("gap-bound");
    // A 64 MiB managed budget: a 16 MiB gap bound.
    let store = Store::open_with_resources(
        &path,
        resources::StorageResources {
            managed_limit_bytes: 64 << 20,
            page_cache_bytes: 1 << 20,
            transaction_pool_bytes: 1 << 20,
            logical_cache_bytes: 1 << 20,
            vector_cache_bytes: 0,
            graph_cache_bytes: 0,
            owner_set_cache_bytes: 0,
            column_cache_bytes: 0,
            phase_budget_bytes: 8 << 20,
            ..resources::StorageResources::default()
        },
    )
    .unwrap();
    configure(&store);
    let store = Arc::new(store);
    store.pause_materialization(true);
    let value = vec![9u8; 1 << 20];
    let writer = {
        let store = Arc::clone(&store);
        std::thread::spawn(move || {
            for n in 0..40u64 {
                commit(&store, 1, &[('P', key(1, n), value.clone())]);
            }
        })
    };
    // The writer stalls once the paused gap is full; nothing is lost and
    // reads keep answering meanwhile.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while store.unmaterialized_bytes() < 14 << 20 {
        assert!(std::time::Instant::now() < deadline, "the gap never filled");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    let stalled = store.committed_version();
    assert!(store.unmaterialized_bytes() <= 16 << 20);
    assert!(!writer.is_finished(), "the writer must wait at the bound");
    assert!(store
        .read_view()
        .unwrap()
        .get(&key(1, 0))
        .unwrap()
        .is_some());
    store.pause_materialization(false);
    writer.join().unwrap();
    assert!(store.committed_version() > stalled);
    store.wait_materialized().unwrap();
    assert_eq!(store.unmaterialized_bytes(), 0);
    drop(store);
    crate::remove_store(&path).unwrap();
}

/// The deterministic commit sequence of the crash tests: step `i` (1-based)
/// is commit index `i` of a fresh store written by one thread.
fn crash_steps(count: u64) -> Vec<(u32, Vec<RawOp>)> {
    let mut rng = Rng(0x5851_F42D_4C95_7F2D);
    (0..count)
        .map(|_| {
            let tenant = 1 + u32::try_from(rng.below(u64::from(TENANTS))).unwrap();
            let mut ops = random_ops(&mut rng, tenant);
            // Some larger values, so materializer batches and checkpoints
            // have real pages to write.
            if rng.below(3) == 0 {
                ops.push(('P', key(tenant, rng.below(KEYS)), vec![7; 64 * 1024]));
            }
            (tenant, ops)
        })
        .collect()
}

const CRASH_PATH: &str = "RUSHDB_GAP_CRASH_PATH";
const CRASH_SCENARIO: &str = "RUSHDB_GAP_CRASH_SCENARIO";

/// The child of [`killed_at`]: a no-op unless it runs as one. Commits the
/// crash sequence, recording how many were acknowledged, until a kill point
/// fires (or the scenario kills it).
#[test]
fn crash_child() {
    let (Ok(path), Ok(scenario)) = (std::env::var(CRASH_PATH), std::env::var(CRASH_SCENARIO))
    else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let store = Arc::new(Store::open(&path).unwrap());
    configure(&store);
    let acked = path.with_extension("acked");
    // A kill can land mid-write: write a temporary file and rename it over
    // the count, so the parent reads the previous count or the new one,
    // never a truncated file.
    let acked_next = path.with_extension("acked.next");
    let steps = crash_steps(40);
    let paused_from = match scenario.as_str() {
        "paused" => 0,
        "checkpoint" => 20,
        _ => usize::MAX,
    };
    let mut compaction = None;
    for (step, (tenant, ops)) in steps.iter().enumerate() {
        if step == paused_from {
            store.pause_materialization(true);
        }
        // Compact online while the rest of the commits land.
        if scenario == "online-compact" && step == 10 {
            let store = Arc::clone(&store);
            compaction = Some(std::thread::spawn(move || store.compact_online()));
        }
        commit(&store, *tenant, ops);
        std::fs::write(&acked_next, (step + 1).to_string()).unwrap();
        std::fs::rename(&acked_next, &acked).unwrap();
    }
    if let Some(compaction) = compaction {
        compaction.join().unwrap().unwrap();
        // Reached only when no kill point stopped it.
        std::fs::write(path.with_extension("compacted"), "").unwrap();
    }
    if scenario == "checkpoint" {
        // The checkpoint finds 20 commits in the gap, waits for them, and
        // is killed inside its write transaction.
        let release = Arc::clone(&store);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            release.pause_materialization(false);
        });
        let _ = store.checkpoint();
    }
    crate::kill_point::kill();
}

/// Run [`crash_child`] under `scenario` (with the kill-point environment
/// `env`), reopen what it left and check it against a store that applied
/// the same commits without interruption, at every snapshot.
/// Returns how many commits were acknowledged and how many recovered.
fn killed_at(scenario: &str, env: &[(&str, &str)]) -> (u64, u64) {
    let path = crate::TempPath::store(&format!("gap-crash-{scenario}"));
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "gap_tests::crash_child", "--nocapture"])
        .env(CRASH_PATH, path.path())
        .env(CRASH_SCENARIO, scenario)
        .envs(env.iter().copied())
        .status()
        .unwrap();
    assert!(!status.success(), "{scenario}: the child must be killed");
    // A kill point inside the online compaction must have stopped it.
    let compacted = path.path().with_extension("compacted");
    let finished = compacted.exists();
    let _ = std::fs::remove_file(&compacted);
    if env
        .iter()
        .any(|(name, point)| *name == "RUSHDB_TEST_KILL_AT" && point.starts_with("online-compact"))
    {
        assert!(!finished, "{scenario}: the kill point was never reached");
    }
    let acked: u64 = std::fs::read_to_string(path.path().with_extension("acked"))
        .map_or(0, |count| count.trim().parse().unwrap());
    let reopened = Store::open(&path).unwrap();
    configure(&reopened);
    let recovered = reopened.committed_version();
    assert!(
        recovered >= acked,
        "{scenario}: {acked} acknowledged, {recovered} recovered"
    );
    let (reference_path, reference) = open(&format!("gap-crash-{scenario}-reference"));
    for (tenant, ops) in crash_steps(recovered) {
        commit(&reference, tenant, &ops);
    }
    reference.wait_materialized().unwrap();
    assert_same(
        &every_snapshot(&reopened),
        &every_snapshot(&reference),
        &format!("{scenario}: {acked} acknowledged, {recovered} recovered"),
    );
    // The recovered store keeps working: one more commit, materialized.
    let (tenant, ops) = &crash_steps(recovered + 1)[usize::try_from(recovered).unwrap()];
    assert_eq!(commit(&reopened, *tenant, ops), recovered + 1);
    reopened.wait_materialized().unwrap();
    drop((reopened, reference));
    let _ = std::fs::remove_file(path.path().with_extension("acked"));
    let _ = std::fs::remove_file(path.path().with_extension("acked.next"));
    crate::remove_store(&path).unwrap();
    crate::remove_store(&reference_path).unwrap();
    (acked, recovered)
}

#[test]
fn a_kill_after_the_wal_fsync_before_publication_recovers_the_commit() {
    let (acked, recovered) = killed_at(
        "before-publish",
        &[
            ("RUSHDB_TEST_KILL_AT", "before-publish"),
            ("RUSHDB_TEST_KILL_AFTER", "25"),
        ],
    );
    // The 25th commit was durable, never acknowledged, and is recovered.
    assert_eq!((acked, recovered), (24, 25));
}

#[test]
fn a_kill_with_every_commit_published_and_none_materialized_loses_none() {
    assert_eq!(killed_at("paused", &[]), (40, 40));
}

#[test]
fn a_kill_between_a_materialized_batch_and_its_trim_recovers_exactly() {
    for after in ["1", "7", "30"] {
        let _ = killed_at(
            "materialized",
            &[
                ("RUSHDB_TEST_KILL_AT", "materialized"),
                ("RUSHDB_TEST_KILL_AFTER", after),
            ],
        );
    }
}

#[test]
fn a_kill_during_a_checkpoint_with_a_gap_open_recovers_exactly() {
    assert_eq!(
        killed_at("checkpoint", &[("RUSHDB_TEST_KILL_AT", "checkpoint")]),
        (40, 40)
    );
}

fn batch_of(tenant: u32, ops: &[RawOp]) -> CommitBatch {
    CommitBatch {
        command_version: rushdb_types::command::COMMAND_VERSION,
        tenant: TenantId::from_ordinal(u64::from(tenant)),
        transaction_id: serde_json::to_string(ops).unwrap(),
        commit_key: Uuid::new_v4(),
        start_index: 0,
        isolation: rushdb_types::command::IsolationMode::ReadCommitted,
        preconditions: vec![],
        mutations: vec![],
    }
}

#[test]
fn a_follower_with_its_gap_open_answers_like_its_leader() {
    let (leader_path, leader) = open("gap-leader");
    let (follower_path, follower) = open("gap-follower");
    follower.pause_materialization(true);
    for (step, (tenant, ops)) in crash_steps(60).into_iter().enumerate() {
        let ci = commit(&leader, tenant, &ops);
        assert_eq!(
            follower
                .apply_replicated_batch(&batch_of(tenant, &ops))
                .unwrap(),
            ci
        );
        if step == 30 {
            follower.pause_materialization(false);
        }
        if step == 45 {
            follower.pause_materialization(true);
        }
    }
    leader.wait_materialized().unwrap();
    assert!(follower.unmaterialized_bytes() > 0);
    assert_same(&every_snapshot(&follower), &every_snapshot(&leader), "");
    follower.pause_materialization(false);
    follower.wait_materialized().unwrap();
    assert_same(&every_snapshot(&follower), &every_snapshot(&leader), "");
    drop((leader, follower));
    crate::remove_store(&leader_path).unwrap();
    crate::remove_store(&follower_path).unwrap();
}

#[test]
fn a_wal_replay_onto_a_store_with_its_gap_open_waits_for_it() {
    let (source_path, source) = open("gap-replay-source");
    let (target_path, target) = open("gap-replay-target");
    let target = Arc::new(target);
    let steps = crash_steps(40);
    // Both stores share the first 20 commits; the target's are still in
    // its gap when the source's later 20 are replayed onto it.
    target.pause_materialization(true);
    for (tenant, ops) in &steps[..20] {
        commit(&source, *tenant, ops);
        commit(&target, *tenant, ops);
    }
    for (tenant, ops) in &steps[20..] {
        commit(&source, *tenant, ops);
    }
    let mut delta = Vec::new();
    source.export_wal_segments(&mut delta, 21, 40).unwrap();
    assert!(target.unmaterialized_bytes() > 0);
    let release = {
        let target = Arc::clone(&target);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            target.pause_materialization(false);
        })
    };
    assert_eq!(target.replay_wal_segments(&mut &delta[..]).unwrap(), 40);
    release.join().unwrap();
    target.wait_materialized().unwrap();
    source.wait_materialized().unwrap();
    assert_eq!(
        answers(&target.read_view().unwrap()),
        answers(&source.read_view().unwrap())
    );
    drop((source, target));
    crate::remove_store(&source_path).unwrap();
    crate::remove_store(&target_path).unwrap();
}

#[test]
fn a_consistent_file_copy_waits_for_the_gap_to_drain() {
    let (path, store) = open("gap-backup");
    let store = Arc::new(store);
    store.pause_materialization(true);
    for (tenant, ops) in crash_steps(20) {
        commit(&store, tenant, &ops);
    }
    assert!(store.unmaterialized_bytes() > 0);
    let release = {
        let store = Arc::clone(&store);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            store.pause_materialization(false);
        })
    };
    let copied = store
        .with_quiescent_files(|ci| Ok((ci, store.unmaterialized_bytes())))
        .unwrap()
        .unwrap();
    release.join().unwrap();
    // The files hold every published commit when the copy runs.
    assert_eq!(copied, (20, 0));
    drop(store);
    crate::remove_store(&path).unwrap();
}

/// VEC-4/VEC-5: request tokens and retained caches (an ANN build's
/// vector-cache reservation) could fill the managed budget to its last
/// byte, and every commit then failed with `resource exhausted`, a rollback
/// included. They now leave a headroom commits can use: commits land.
#[test]
fn commits_land_while_caches_fill_the_managed_budget() {
    let path = crate::TempPath::store("gap-floor");
    let store = Store::open_with_resources(
        &path,
        resources::StorageResources {
            managed_limit_bytes: 64 << 20,
            page_cache_bytes: 1 << 20,
            transaction_pool_bytes: 1 << 20,
            logical_cache_bytes: 1 << 20,
            vector_cache_bytes: 64 << 20,
            graph_cache_bytes: 0,
            owner_set_cache_bytes: 0,
            column_cache_bytes: 0,
            phase_budget_bytes: 64 << 20,
            ..resources::StorageResources::default()
        },
    )
    .unwrap();
    configure(&store);
    // Request tokens and the vector cache take everything the managed
    // budget lets them.
    let mut held = Vec::new();
    for budget in [
        store.phase_budget().unwrap(),
        store.vector_cache_budget().unwrap(),
    ] {
        for chunk in [1 << 20, 64 << 10, 4 << 10, 256] {
            while let Ok(reservation) = budget.try_reserve(chunk) {
                held.push(reservation);
            }
        }
    }
    let value = vec![7u8; 256 << 10];
    for n in 0..8 {
        commit(&store, 1, &[('P', key(1, n), value.clone())]);
    }
    assert_eq!(store.committed_version(), 8);
    drop(held);
}

/// #427: a store killed while it compacts online, before the copy replaces
/// the file or right after, reopens with every acknowledged commit and
/// every snapshot as a store that never compacted.
#[test]
fn a_kill_during_an_online_compaction_recovers_exactly() {
    for point in ["online-compact-before-swap", "online-compact-renamed"] {
        let (acked, recovered) = killed_at("online-compact", &[("RUSHDB_TEST_KILL_AT", point)]);
        assert!(recovered >= acked, "{point}");
    }
    // Not killed: it finishes, and the child is killed after the commits.
    assert_eq!(killed_at("online-compact", &[]), (40, 40));
}

/// #427: an online compaction serves reads and commits while it runs,
/// shrinks a file with free pages, and leaves every snapshot as a store
/// that never compacted has it.
#[test]
fn online_compaction_serves_while_it_shrinks_the_file() {
    let (path, store) = open("online-compact");
    let store = Arc::new(store);
    let big = vec![5u8; 32 << 10];
    let mut steps: Vec<(u32, Vec<RawOp>)> = (0..200)
        .map(|n| (1, vec![('P', key(1, n), big.clone())]))
        .collect();
    // Purged keys leave free pages behind.
    steps.extend((0..150).map(|n| (1, vec![('X', key(1, n), Vec::new())])));
    for (tenant, ops) in &steps {
        commit(&store, *tenant, ops);
    }
    store.wait_materialized().unwrap();
    store.checkpoint().unwrap();
    let before = store.storage_file_bytes().unwrap();
    // Commits land between the copy and the swap: the copy catches up in
    // its loop (more than the swap's lag) and again inside the swap.
    let during: Vec<(u32, Vec<RawOp>)> = (0..400)
        .map(|n| (3, vec![('P', key(3, n), vec![3u8; 1 << 10])]))
        .collect();
    {
        let (store, during) = (Arc::clone(&store), during.clone());
        *crate::redb_backend::AFTER_COPY.lock().unwrap() = Some(Box::new(move || {
            for (tenant, ops) in &during {
                commit(&store, *tenant, ops);
            }
            store.wait_materialized().unwrap();
        }));
    }
    // Reads go on throughout.
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = {
        let (store, done) = (Arc::clone(&store), Arc::clone(&done));
        std::thread::spawn(move || {
            let mut reads = 0u64;
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                let view = store.read_view().unwrap();
                assert!(view.get(&key(1, 199)).unwrap().is_some());
                reads += 1;
            }
            reads
        })
    };
    let catch_ups = crate::redb_backend::CATCH_UPS.load(std::sync::atomic::Ordering::SeqCst);
    let outcome = store.compact_online().unwrap();
    *crate::redb_backend::AFTER_COPY.lock().unwrap() = None;
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let reads = reader.join().unwrap();
    assert!(
        crate::redb_backend::CATCH_UPS.load(std::sync::atomic::Ordering::SeqCst) > catch_ups,
        "the copy caught up from the WAL"
    );
    assert!(
        matches!(outcome, crate::OnlineCompaction::Compacted(_)),
        "{outcome:?}"
    );
    assert!(reads > 0);
    store.wait_materialized().unwrap();
    let after = store.storage_file_bytes().unwrap();
    assert!(after < before, "{before} bytes before, {after} after");
    let (reference_path, reference) = open("online-compact-reference");
    for (tenant, ops) in steps.iter().chain(&during) {
        commit(&reference, *tenant, ops);
    }
    reference.wait_materialized().unwrap();
    assert_same(
        &every_snapshot(&store),
        &every_snapshot(&reference),
        "online compaction",
    );
    // And as reopened.
    let store = Arc::try_unwrap(store).ok().unwrap();
    drop(store);
    let reopened = Store::open(&path).unwrap();
    configure(&reopened);
    assert_same(
        &every_snapshot(&reopened),
        &every_snapshot(&reference),
        "online compaction, reopened",
    );
    drop((reopened, reference));
    crate::remove_store(&path).unwrap();
    crate::remove_store(&reference_path).unwrap();
}
