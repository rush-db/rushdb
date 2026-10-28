//! Growth accounting: reconcile per-family durable bytes with the file and
//! the commit count, so operators can see where growth comes from.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::{CompactOutcome, Store};
use rushdb_types::ids::TenantId;
use serde_json::json;

const TENANT: TenantId = TenantId::from_ordinal(77);
const COMMITS: u64 = 2_000;

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rushdb-stats-{name}-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ))
}

#[test]
fn storage_stats_account_for_every_stored_byte() {
    let dir = path("account");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let store = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TENANT);
    for i in 0..COMMITS {
        lmpg.create_record("S", None, vec![("u".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }

    let stats = store.storage_stats().unwrap();
    let file = store.storage_file_bytes().unwrap();
    eprintln!("stats = {stats:?}");
    eprintln!("file  = {file} bytes");

    // One retry receipt plus its expiry row per commit. The command
    // journal itself is not a store row (it is derived from the
    // WAL), so `stats.log_keys`/`log_bytes` are always 0 for a durable store.
    assert_eq!(stats.receipt_keys, 2 * COMMITS, "one receipt per commit");
    assert_eq!(stats.log_keys, 0, "no in-redb command journal rows remain");
    // Canonical data dominates key count (record header, postings, …).
    assert!(stats.data_keys > stats.receipt_keys);
    assert!(stats.data_bytes > 0);

    let live = stats.data_bytes + stats.history_bytes + stats.log_bytes + stats.receipt_bytes;
    assert!(
        file < live * 8,
        "file {file} grew more than 8x live data {live}"
    );
    store.compact_if_idle().unwrap();
    let compacted = store.storage_file_bytes().unwrap();
    eprintln!("compaction: {file} -> {compacted} bytes (live {live})");
    assert!(
        compacted < live * 3,
        "compaction left excessive slack: {compacted} vs live {live}"
    );
    {
        // The file is an uncompressed copy-on-write high-water mark, at
        // least the live rows, and idle compaction only shrinks it.
        assert!(file >= live, "file {file} < live {live}");
        assert!(
            compacted >= live,
            "compaction dropped below live bytes: {compacted} < {live}"
        );
        assert!(
            compacted <= file,
            "compaction grew the file: {file} -> {compacted}"
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn compaction_is_gated_by_quiescence_and_growth() {
    use std::time::Duration;

    let dir = path("gate");
    std::fs::create_dir_all(&dir).unwrap();
    let store = Arc::new(Store::open(&dir.join("rushdb.redb")).unwrap());
    let lmpg = Lmpg::new(store.clone(), TENANT);
    for i in 0..500u64 {
        lmpg.create_record("S", None, vec![("u".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }

    // Not quiescent: a commit just happened, so compaction defers.
    assert_eq!(
        store
            .compact_if_needed(Duration::from_secs(600), 100)
            .unwrap(),
        CompactOutcome::CommitsRecent
    );

    // Quiescent and never compacted: it runs.
    let ran = store.compact_if_needed(Duration::ZERO, 100).unwrap();
    assert!(
        matches!(ran, CompactOutcome::Compacted(_)),
        "first quiescent compaction should run: {ran:?}"
    );

    // Growth gate: with no new growth, a 200% threshold defers.
    assert_eq!(
        store.compact_if_needed(Duration::ZERO, 200).unwrap(),
        CompactOutcome::BelowThreshold
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn tenant_purge_triggers_compaction_without_file_growth() {
    // L4: a purge frees pages but never grows the file, so the
    // growth gate alone would never give a purged tenant's space back.
    use std::time::Duration;

    let dir = path("purge-compact");
    std::fs::create_dir_all(&dir).unwrap();
    let store = Arc::new(Store::open(&dir.join("rushdb.redb")).unwrap());
    let keep = Lmpg::new(store.clone(), TENANT);
    let gone = Lmpg::new(store.clone(), TenantId::from_ordinal(42));
    for i in 0..300u64 {
        keep.create_record("S", None, vec![("u".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }
    for i in 0..1_500u64 {
        gone.create_record(
            "S",
            None,
            vec![(
                "u".into(),
                FieldChange::Set(json!(format!("{i}-{}", "x".repeat(200)))),
            )],
        )
        .unwrap();
    }
    assert!(matches!(
        store.compact_if_needed(Duration::ZERO, 100).unwrap(),
        CompactOutcome::Compacted(_)
    ));
    let before = store.storage_file_bytes().unwrap();
    // No growth since that compaction: the growth gate defers.
    assert_eq!(
        store.compact_if_needed(Duration::ZERO, 150).unwrap(),
        CompactOutcome::BelowThreshold
    );
    gone.purge_tenant().unwrap();
    // The purge alone qualifies, and the file shrinks.
    assert!(matches!(
        store.compact_if_needed(Duration::ZERO, 150).unwrap(),
        CompactOutcome::Compacted(_)
    ));
    let after = store.storage_file_bytes().unwrap();
    assert!(after * 10 < before * 6, "file {before} -> {after} bytes");
    // Consumed: without a new purge or growth it defers again.
    assert_eq!(
        store.compact_if_needed(Duration::ZERO, 150).unwrap(),
        CompactOutcome::BelowThreshold
    );
    std::fs::remove_dir_all(&dir).ok();
}

fn padded_resources() -> rushdb_storage::resources::StorageResources {
    rushdb_storage::resources::StorageResources {
        log_segment_bytes: 64 * 1024,
        ..Default::default()
    }
}

/// A field large enough that a modest commit volume rotates several WAL
/// segments at the minimum (64 KiB) segment size.
fn padding() -> serde_json::Value {
    json!("x".repeat(2000))
}

#[test]
fn byte_target_prunes_sealed_wal_segments() {
    // The byte-target floor walks WAL segments (the log's actual
    // reclamation unit), not row-granular `__log__` bytes. Reclaiming them
    // needs retention (which advances the cursor `truncate_log` clamps to,
    // A4) and a checkpoint — the only thing that truncates segments.
    let dir = path("bytes");
    std::fs::create_dir_all(&dir).unwrap();
    let store =
        Arc::new(Store::open_with_resources(&dir.join("rushdb.redb"), padded_resources()).unwrap());
    let lmpg = Lmpg::new(store.clone(), TENANT);
    for i in 0..500u64 {
        lmpg.create_record(
            "S",
            None,
            vec![
                ("u".into(), FieldChange::Set(json!(i))),
                ("pad".into(), FieldChange::Set(padding())),
            ],
        )
        .unwrap();
    }
    let total = store.retained_log_bytes().unwrap();
    assert!(total > 0);

    let target = total / 2;
    let floor = store.log_byte_floor(target).unwrap();
    assert!(floor > 0, "byte floor must be positive when over target");
    loop {
        if store
            .retire_commit_metadata_before(floor, 1_000_000)
            .unwrap()
            == 0
        {
            break;
        }
    }
    store.checkpoint().unwrap();

    let after = store.retained_log_bytes().unwrap();
    assert!(
        after < total,
        "retained WAL bytes {after} did not shrink from {total}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn retained_log_bytes_tracks_commits_pruning_and_reopen() {
    // `retained_log_bytes` now sums the segmented WAL's own bytes
    // instead of a durable per-row running total. It still follows
    // retention + checkpoint-driven pruning and survives reopen.
    let dir = path("logtotal");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let config = padded_resources();
    {
        let store = Arc::new(Store::open_with_resources(&db, config).unwrap());
        let lmpg = Lmpg::new(store.clone(), TENANT);
        for i in 0..500u64 {
            lmpg.create_record(
                "S",
                None,
                vec![
                    ("u".into(), FieldChange::Set(json!(i))),
                    ("pad".into(), FieldChange::Set(padding())),
                ],
            )
            .unwrap();
        }
        let total = store.retained_log_bytes().unwrap();
        assert!(total > 0);

        let target = total / 2;
        let floor = store.log_byte_floor(target).unwrap();
        assert!(floor > 0, "over target must produce a floor");
        loop {
            if store
                .retire_commit_metadata_before(floor, 1_000_000)
                .unwrap()
                == 0
            {
                break;
            }
        }
        store.checkpoint().unwrap();

        let after = store.retained_log_bytes().unwrap();
        assert!(after < total, "pruned WAL bytes {after} not below {total}");
        assert_eq!(
            store.log_byte_floor(target).unwrap(),
            0,
            "within budget => no floor"
        );
    }

    let reopened = Store::open_with_resources(&db, config).unwrap();
    let after_reopen = reopened.retained_log_bytes().unwrap();
    assert!(after_reopen > 0, "WAL segments are restored on reopen");
    std::fs::remove_dir_all(&dir).ok();
}
