//! P00 baseline: incremental dataset generation + parent/child crash harness
//! (04-implementation-backlog.md P00). A child process (this same test binary
//! re-executed with `--p00-crash-child <db>`) writes an incremental dataset and
//! aborts; the parent reopens and verifies acknowledged commits are durable,
//! the logical log is dense/replayable, and record membership is intact. This
//! exercises crash recovery without a separate binary or killing the runner.

use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::resources::{Durability, StorageResources};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

const TENANT: TenantId = TenantId::from_ordinal(66);
const CRASH_COMMITS: u64 = 8;

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rushdb-p00-{name}-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ))
}

/// Locate the compiled `crash_child` example binary (built by the test build).
fn crash_child_bin() -> std::path::PathBuf {
    // current_exe is target/{debug,release}/deps/baseline-<hash>. The
    // example lives at target/{debug,release}/examples/crash_child.
    let mut path = std::env::current_exe().unwrap();
    if path.pop() && path.pop() {
        path.push("examples");
        path.push("crash_child");
    }
    path
}

fn state_fingerprint(store: &Store) -> u64 {
    let view = store.read_view().unwrap();
    let mut rows = view
        .range_prefix(&rushdb_types::keys::key(TENANT, &[]))
        .unwrap();
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for (key, value) in rows {
        key.hash(&mut hasher);
        value.hash(&mut hasher);
    }
    hasher.finish()
}

#[test]
fn child_crash_after_acknowledged_commits_reopens_with_exactly_those_commits() {
    // Re-exec entry: if this process IS the child, write and abort.

    let dir = path("crash");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");

    // Spawn the crash_child example binary as the crash child.
    let child = crash_child_bin();
    assert!(
        child.exists(),
        "crash_child example not built: {}",
        child.display()
    );
    let mut child_process = std::process::Command::new(&child)
        .arg(&db)
        .arg(CRASH_COMMITS.to_string())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child_process.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "crash child did not terminate");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        !status.success(),
        "child must crash (abort), not exit cleanly"
    );

    // Parent: reopen and verify every acknowledged record survived.
    let store = Arc::new(rushdb_lmpg::open_store(&db).unwrap());
    let view = store.read_view().unwrap();
    let members = rushdb_lmpg::reader::try_access::label_members(&view, TENANT, "Doc").unwrap();
    assert_eq!(
        members.len() as u64,
        CRASH_COMMITS,
        "all acknowledged records must survive the crash"
    );

    // The logical log is dense and replayable onto a fresh store (the expander
    // is wired by Lmpg::new, same as production).
    let log = store.log_bytes();
    assert_eq!(
        log.len() as u64,
        CRASH_COMMITS,
        "log dense: one entry per commit"
    );
    let replayed = Arc::new(Store::new_in_memory());
    let _rlmpg = Lmpg::new(Arc::clone(&replayed), TENANT);
    replayed.try_replay(&log).unwrap();
    let rview = replayed.read_view().unwrap();
    let rmembers = rushdb_lmpg::reader::try_access::label_members(&rview, TENANT, "Doc").unwrap();
    assert_eq!(
        rmembers.len() as u64,
        CRASH_COMMITS,
        "replay reconstructs the crashed prefix"
    );
    assert_eq!(
        state_fingerprint(&store),
        state_fingerprint(&replayed),
        "replay must preserve canonical record content, not only membership counts"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Spawn the crash child with extra arguments and wait for it (bounded).
fn run_crash_child(db: &Path, args: &[&str]) -> ExitStatus {
    let child = crash_child_bin();
    assert!(
        child.exists(),
        "crash_child example not built: {}",
        child.display()
    );
    let mut command = Command::new(&child);
    command.arg(db).args(args);
    let mut process = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = process.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "crash child did not terminate");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The reopened store must be dense (one log entry per acknowledged commit),
/// replay to an identical canonical fingerprint, and have exactly one
/// membership row per durable commit. Returns the durable commit prefix.
fn assert_dense_replayable(store: &Store) -> u64 {
    let committed = store.committed_version();
    let log = store.log_bytes();
    assert_eq!(
        log.len() as u64,
        committed,
        "log must be dense across the durable prefix"
    );
    let replayed = Arc::new(Store::new_in_memory());
    let _lmpg = Lmpg::new(Arc::clone(&replayed), TENANT);
    replayed.try_replay(&log).unwrap();
    assert_eq!(
        state_fingerprint(store),
        state_fingerprint(&replayed),
        "replay must reproduce the exact canonical state"
    );
    let view = store.read_view().unwrap();
    let members = rushdb_lmpg::reader::try_access::label_members(&view, TENANT, "Doc").unwrap();
    assert_eq!(
        members.len() as u64,
        committed,
        "one membership row per durable commit"
    );
    committed
}

#[test]
fn crash_child_multi_shard_reopen_is_consistent() {
    let dir = path("crash-shards");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let status = run_crash_child(&db, &["4", "--shards", "2"]);
    assert!(!status.success(), "child must crash, not exit cleanly");

    let config = StorageResources {
        shard_count: 2,
        experimental_multi_shard: true,
        ..Default::default()
    };
    let store = Arc::new(rushdb_lmpg::open_store_with_resources(&db, config).unwrap());
    assert_eq!(
        assert_dense_replayable(&store),
        4,
        "all acknowledged commits survive a multi-shard Local crash"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn crash_child_concurrent_burst_reopen_has_no_torn_state() {
    let dir = path("crash-burst");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    // Four writers, aborted 5 ms in so some commits are still in flight.
    let status = run_crash_child(&db, &["20", "--concurrent", "4", "--delay-ms", "5"]);
    assert!(!status.success(), "child must crash, not exit cleanly");

    let store = Arc::new(rushdb_lmpg::open_store(&db).unwrap());
    let committed = assert_dense_replayable(&store);
    assert!(
        committed <= 80,
        "durable prefix {committed} cannot exceed the 4x20 writes"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn crash_child_async_durability_leaves_a_consistent_prefix() {
    let dir = path("crash-async");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let status = run_crash_child(&db, &["50", "--async"]);
    assert!(!status.success(), "child must crash, not exit cleanly");

    let config = StorageResources {
        durability: Durability::Async,
        ..Default::default()
    };
    let store = Arc::new(rushdb_lmpg::open_store_with_resources(&db, config).unwrap());
    let committed = assert_dense_replayable(&store);
    assert!(
        committed <= 50,
        "async durability may lose the tail but never invents commits"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn incremental_dataset_generation_is_bounded_and_dense() {
    let dir = path("incremental");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let store = Arc::new(rushdb_lmpg::open_store(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TENANT);
    let mut expected = Vec::new();
    for i in 0..50u64 {
        let r = lmpg
            .create_record("Doc", None, vec![("i".into(), FieldChange::Set(json!(i)))])
            .unwrap();
        expected.push(r.record_id);
    }
    let ci = store.committed_version();
    let log = store.log_bytes();
    // Dense: one commit per record, so log length == committed version.
    assert_eq!(log.len() as u64, ci, "log must be dense (no stranded CI)");
    drop(lmpg);
    drop(store);

    let store2 = Arc::new(rushdb_lmpg::open_store(&db).unwrap());
    let view = store2.read_view().unwrap();
    let members = rushdb_lmpg::reader::try_access::label_members(&view, TENANT, "Doc").unwrap();
    assert_eq!(
        members.len(),
        expected.len(),
        "all incremental records persisted"
    );
    std::fs::remove_dir_all(&dir).ok();
}
