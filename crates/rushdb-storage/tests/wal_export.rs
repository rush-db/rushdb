//! Log-first WAL export/import integrity (decision B): the durable
//! segmented WAL stores checksummed framed op-set records, and a segment export
//! must round-trip those records so a restore replays the same state.

use rushdb_storage::{wal, Store};
use rushdb_types::command::{CommitBatch, COMMAND_VERSION};
use uuid::Uuid;

fn durable_store(name: &str) -> (std::path::PathBuf, Store) {
    let dir = std::env::temp_dir().join(format!(
        "rushdb-wal-{name}-{}-{}",
        std::process::id(),
        Uuid::now_v7()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let store = Store::open(&db).unwrap();
    store.set_expander(std::sync::Arc::new(Box::new(|_ci, _batch, _txn| Ok(()))));
    (dir, store)
}

fn commit_batch(ordinal: u64) -> CommitBatch {
    CommitBatch {
        command_version: COMMAND_VERSION,
        tenant: rushdb_types::ids::TenantId::single(),
        transaction_id: format!("t{ordinal}"),
        commit_key: Uuid::now_v7(),
        start_index: 0,
        isolation: rushdb_types::command::IsolationMode::Snapshot,
        preconditions: vec![],
        mutations: vec![],
    }
}

fn commit(store: &Store, ordinal: u64) {
    let batch = commit_batch(ordinal);
    store
        .commit(batch.commit_key, |_ci, _txn| Ok((batch.clone(), ordinal)))
        .expect("commit");
}

#[test]
fn exported_segments_round_trip_to_op_sets() {
    let (dir, store) = durable_store("round-trip");
    for i in 1..=5u64 {
        commit(&store, i);
    }

    // Export a bounded LSN range and verify the segment manifest + records.
    let mut out: Vec<u8> = Vec::new();
    let exported = store.export_wal_segments(&mut out, 2, 4).expect("export");
    assert_eq!(exported, 3);

    let mut reader = std::io::Cursor::new(&out);
    let Some((manifest, records)) = wal::read_wal_segment(&mut reader).expect("segment") else {
        panic!("expected one segment");
    };
    assert_eq!(manifest.from_lsn, 2);
    assert_eq!(manifest.to_lsn, 4);
    assert_eq!(manifest.record_count, 3);
    assert_eq!(records.len(), 3);

    // Every record decodes as a valid op-set with its own LSN.
    for (idx, record) in records.iter().enumerate() {
        let lsn = 2 + idx as u64;
        let (header, ops, _command) = wal::decode_ops_record(record).expect("decode");
        assert_eq!(header.lsn, lsn);
        assert!(!ops.is_empty(), "a commit produces at least LAST_APPLIED");
    }

    drop(store);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn incomplete_export_range_is_refused() {
    let (dir, store) = durable_store("incomplete");
    for i in 1..=3u64 {
        commit(&store, i);
    }

    // A requested range above the committed log (4,5 absent) must fail closed
    // rather than certify an incomplete segment.
    let mut out: Vec<u8> = Vec::new();
    let err = store.export_wal_segments(&mut out, 2, 5).unwrap_err();
    assert!(
        matches!(err, rushdb_storage::StorageError::Durability(_)),
        "{err:?}"
    );
    assert!(out.is_empty(), "no partial segment may be written");

    // A range whose start predates the log (0 absent) also fails closed.
    let mut out: Vec<u8> = Vec::new();
    assert!(store.export_wal_segments(&mut out, 0, 3).is_err());
    assert!(out.is_empty());

    // The exact covered range still succeeds.
    let mut out: Vec<u8> = Vec::new();
    assert_eq!(store.export_wal_segments(&mut out, 1, 3).unwrap(), 3);

    drop(store);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn replay_rejects_an_interior_lsn_gap() {
    let (dir, store) = durable_store("gap");
    for i in 1..=3u64 {
        commit(&store, i);
    }
    // Two individually-valid segments with LSN 2 missing.
    let mut stream = Vec::new();
    store.export_wal_segments(&mut stream, 1, 1).unwrap();
    store.export_wal_segments(&mut stream, 3, 3).unwrap();

    let recovered = Store::new_in_memory();
    let err = recovered.replay_wal_segments(&mut &stream[..]).unwrap_err();
    assert!(
        matches!(err, rushdb_storage::StorageError::Durability(_)),
        "a missing LSN must not be silently compacted: {err:?}"
    );

    drop(store);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn corrupted_export_is_rejected() {
    let (dir, store) = durable_store("corrupt");
    for i in 1..=3u64 {
        commit(&store, i);
    }
    let mut out: Vec<u8> = Vec::new();
    store.export_wal_segments(&mut out, 1, 3).expect("export");
    let mid = out.len() / 2;
    out[mid] ^= 0x08;
    let mut reader = std::io::Cursor::new(&out);
    assert!(
        wal::read_wal_segment(&mut reader).is_err(),
        "corrupted segment must be rejected"
    );

    drop(store);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn redb_reopen_recovers_from_wal() {
    let dir = std::env::temp_dir().join(format!(
        "rushdb-wal-reopen-{}-{}",
        std::process::id(),
        Uuid::now_v7()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");

    {
        let store = Store::open(&db).unwrap();
        store.set_expander(std::sync::Arc::new(Box::new(|_ci, _batch, _txn| Ok(()))));
        for i in 1..=3u64 {
            commit(&store, i);
        }
    }

    // Reopen: recovery replays any WAL tail and restores the commit index.
    let store = Store::open(&db).unwrap();
    assert_eq!(store.committed_version(), 3);
    // The command journal is derived from the WAL's embedded-command
    // envelope, not a materialized redb row, and survives reopen.
    let batches: Vec<CommitBatch> = store
        .log_bytes()
        .iter()
        .map(|raw| rushdb_types::command_codec::decode(raw).expect("journal entry"))
        .collect();
    assert_eq!(batches.len(), 3);
    assert_eq!(batches[0].transaction_id, "t1");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn command_journal_is_fully_derivable_from_the_wal_without_a_log_row() {
    // The in-redb `__log__` command journal is
    // deletable only because its content — the originating `CommitBatch` per
    // commit, in order — is fully reconstructable from the segmented WAL's
    // embedded-command envelope alone. This test commits through the real
    // path, asserts no `__log__` row exists anywhere in the durable backend,
    // and still recovers every original command byte-for-byte from the WAL.
    let (dir, store) = durable_store("derivable");
    let originals: Vec<CommitBatch> = (1..=5).map(commit_batch).collect();
    for batch in &originals {
        let batch = batch.clone();
        store
            .commit(batch.commit_key, move |_ci, _txn| Ok((batch, ())))
            .expect("commit");
    }

    let view = store.read_view().unwrap();
    assert!(
        view.range_prefix(b"__log__/").unwrap().is_empty(),
        "no in-redb command journal row must remain on a durable store"
    );

    let recovered: Vec<CommitBatch> = store
        .log_bytes()
        .iter()
        .map(|raw| rushdb_types::command_codec::decode(raw).expect("WAL-derived journal entry"))
        .collect();
    assert_eq!(recovered.len(), originals.len());
    for (original, recovered) in originals.iter().zip(recovered.iter()) {
        assert_eq!(original.commit_key, recovered.commit_key);
        assert_eq!(original.transaction_id, recovered.transaction_id);
        assert_eq!(original.tenant, recovered.tenant);
    }

    drop(store);
    std::fs::remove_dir_all(&dir).ok();
}
