//! Durability A: a physical base backup plus continuously archived WAL
//! restores any point in the retained window (base + PITR), so the store is no
//! longer "one file or nothing".

use rushdb_server::archive;
use rushdb_server::{Node, ServerConfig};
use serde_json::json;

fn config(dir: &std::path::Path) -> ServerConfig {
    ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        ..Default::default()
    }
}

fn snapshot_bytes(node: &Node) -> Vec<u8> {
    let mut out = Vec::new();
    node.export_snapshot(&mut out, &[]).unwrap();
    out
}

fn write_range(node: &Node, from: u64, to: u64) {
    for i in from..to {
        node.scope()
            .create_record("S", &json!({ "u": i }), &[], &json!({}))
            .unwrap();
    }
}

#[test]
fn base_backup_plus_archived_wal_restores_to_a_point_in_time() {
    let root = std::env::temp_dir().join(format!(
        "rushdb-pitr-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let src_dir = root.join("src");
    let archive_dir = root.join("archive");
    let base_dir = root.join("base");
    let restored60 = root.join("restored60");
    let restored45 = root.join("restored45");
    std::fs::create_dir_all(&src_dir).unwrap();

    let (snap30, snap45, snap60);
    {
        let node = Node::bootstrap(&mut config(&src_dir)).unwrap();
        write_range(&node, 0, 30);
        assert_eq!(node.committed_version(), 30);
        snap30 = snapshot_bytes(&node);
        // Archive the first range, then take the physical base at commit 30.
        // Log-first: checkpoint the materialization before the offline base
        // copy so the copied store file is durable at commit 30.
        node.archive_wal(&archive_dir).unwrap();
        node.checkpoint().unwrap();
        archive::backup_base(&src_dir, &base_dir, 30).unwrap();

        write_range(&node, 30, 45);
        snap45 = snapshot_bytes(&node);
        node.archive_wal(&archive_dir).unwrap();

        write_range(&node, 45, 60);
        snap60 = snapshot_bytes(&node);
        node.archive_wal(&archive_dir).unwrap();
        assert!(snap30 != snap45 && snap45 != snap60);
    }

    // Restore the base at 30 and replay the archive to the latest (60).
    archive::restore_base_into(&base_dir, &restored60).unwrap();
    assert!(archive::restore_base_into(&base_dir, &restored60).is_err());
    {
        let node = Node::bootstrap(&mut config(&restored60)).unwrap();
        assert_eq!(node.committed_version(), 30, "base pinned at 30");
        let mut stream = archive::segment_stream(&archive_dir).unwrap();
        let ci = node.restore_pitr_to(&mut stream, Some(30), None).unwrap();
        assert_eq!(ci, 60);
        // G2: the live replay derives every commit's counters, presence and
        // dictionary rows; `verify` compares them with a from-scratch
        // derivation of the restored base rows.
        assert_eq!(node.verify().unwrap(), 0, "restored graph is consistent");
        assert_eq!(snapshot_bytes(&node), snap60, "state matches source at 60");
    }

    // Point-in-time restore to 45 from the same base + archive.
    archive::restore_base_into(&base_dir, &restored45).unwrap();
    {
        let node = Node::bootstrap(&mut config(&restored45)).unwrap();
        let mut stream = archive::segment_stream(&archive_dir).unwrap();
        let ci = node
            .restore_pitr_to(&mut stream, Some(30), Some(45))
            .unwrap();
        assert_eq!(ci, 45);
        assert_eq!(node.verify().unwrap(), 0, "derived rows exact at 45");
        assert_eq!(snapshot_bytes(&node), snap45, "state matches source at 45");
    }

    // The base manifest verifies, and a corrupted base file is rejected.
    assert_eq!(archive::verify_base(&base_dir).unwrap(), 30);
    let base_file = base_dir.join("base").join("rushdb.redb");
    let mut bytes = std::fs::read(&base_file).unwrap();
    if let Some(byte) = bytes.last_mut() {
        *byte ^= 0xFF;
    }
    std::fs::write(&base_file, &bytes).unwrap();
    assert!(
        archive::verify_base(&base_dir).is_err(),
        "corruption detected"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn online_backup_under_concurrent_writes_restores_consistently() {
    // An online base backup, taken while the
    // server keeps accepting writes, must still restore to a consistent
    // state exactly at the commit index it reports — unlike
    // `--backup-base`, nothing stops the writer for this to hold.
    let root = rushdb_storage::TempPath::dir("online-backup");
    let src_dir = root.join("src");
    let base_dir = root.join("base");
    let restored_dir = root.join("restored");
    std::fs::create_dir_all(&src_dir).unwrap();

    let node = std::sync::Arc::new(Node::bootstrap(&mut config(&src_dir)).unwrap());
    write_range(&node, 0, 50);

    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let writer_node = node.clone();
    let writer_stop = stop.clone();
    let writer = std::thread::spawn(move || {
        let mut i = 50u64;
        while !writer_stop.load(std::sync::atomic::Ordering::Relaxed) {
            writer_node
                .scope()
                .create_record("S", &json!({ "u": i }), &[], &json!({}))
                .unwrap();
            i += 1;
        }
    });

    // Let the writer get going, then take the online backup while it keeps
    // racing commits in — including through the copy itself.
    std::thread::sleep(std::time::Duration::from_millis(20));
    let manifest = node.backup_base_online(&base_dir).unwrap();
    assert!(manifest.commit_index >= 50);

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    writer.join().unwrap();

    // The manifest's own CRCs verify: no torn copy.
    assert_eq!(
        archive::verify_base(&base_dir).unwrap(),
        manifest.commit_index
    );

    archive::restore_base_into(&base_dir, &restored_dir).unwrap();
    let restored = Node::bootstrap(&mut config(&restored_dir)).unwrap();
    assert_eq!(restored.committed_version(), manifest.commit_index);
    assert_eq!(
        restored.verify().unwrap(),
        0,
        "restored graph is consistent"
    );

    // Not just "looks ok": byte-identical to the source's own state at
    // exactly the commit index the backup pinned.
    let mut source_snapshot = Vec::new();
    node.export_snapshot_at(&mut source_snapshot, manifest.commit_index, &[])
        .unwrap();
    let mut restored_snapshot = Vec::new();
    restored
        .export_snapshot(&mut restored_snapshot, &[])
        .unwrap();
    assert_eq!(source_snapshot, restored_snapshot);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn restore_rejects_unsafe_or_duplicate_manifest_paths() {
    let root = rushdb_storage::TempPath::dir("pitr-manifest");
    let base = root.join("base");
    let payload = base.join("base");
    std::fs::create_dir_all(&payload).unwrap();
    let manifest = rushdb_server::archive::BaseManifest {
        format_version: 1,
        command_version: rushdb_types::command::COMMAND_VERSION,
        protocol_version: rushdb_protocol::PROTOCOL_VERSION,
        commit_index: 1,
        files: vec![rushdb_server::archive::BaseFile {
            name: "../escape.redb".into(),
            bytes: 0,
            crc32: 0,
        }],
    };
    std::fs::write(
        base.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(rushdb_server::archive::verify_base(&base).is_err());
    assert!(rushdb_server::archive::restore_base_into(&base, &root.join("out")).is_err());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn segment_stream_rejects_wal_gaps_and_overlaps() {
    let root = rushdb_storage::TempPath::dir("pitr-gap");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("wal-00000000000000000000-00000000000000000010.seg"),
        [],
    )
    .unwrap();
    std::fs::write(
        root.join("wal-00000000000000000012-00000000000000000020.seg"),
        [],
    )
    .unwrap();
    assert!(rushdb_server::archive::segment_stream(&root).is_err());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn pitr_within_window_succeeds_outside_fails_cleanly() {
    let root = rushdb_storage::TempPath::dir("pitr-window");
    let src_dir = root.join("src");
    let archive_dir = root.join("archive");
    std::fs::create_dir_all(&src_dir).unwrap();
    let day = std::time::Duration::from_hours(24);
    let now = rushdb_server::retention::unix_ms();
    let age = |path: &std::path::Path, days: u32| {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - day * days)
            .unwrap();
    };
    let base_dir = root.join("base");
    let snap60;
    {
        let node = Node::bootstrap(&mut config(&src_dir)).unwrap();
        write_range(&node, 0, 30);
        node.archive_wal(&archive_dir).unwrap();
        node.checkpoint().unwrap();
        archive::backup_base(&src_dir, &base_dir, 30).unwrap();
        // Taken 9 days ago, before a 7-day window's start.
        archive::register_base(&archive_dir, 30, now - 9 * 24 * 3600 * 1000).unwrap();
        write_range(&node, 30, 60);
        node.archive_wal(&archive_dir).unwrap();
        snap60 = snapshot_bytes(&node);
    }
    let mut segments: Vec<std::path::PathBuf> = std::fs::read_dir(&archive_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "seg"))
        .collect();
    segments.sort();
    assert_eq!(segments.len(), 2);
    for segment in &segments {
        age(segment, 10);
    }

    // Inside the window: the base at 30 covers its start, so the segment
    // after it survives pruning and the restore reaches commit 60.
    let pruned = archive::prune_archive(&archive_dir, now, day * 7, None).unwrap();
    assert_eq!((pruned.segments, pruned.forced), (1, 0));
    let restored = root.join("restored");
    archive::restore_base_into(&base_dir, &restored).unwrap();
    {
        let node = Node::bootstrap(&mut config(&restored)).unwrap();
        let mut stream = archive::segment_stream(&archive_dir).unwrap();
        assert_eq!(
            node.restore_pitr_to(&mut stream, Some(30), None).unwrap(),
            60
        );
        assert_eq!(snapshot_bytes(&node), snap60);
    }

    // Outside: a byte cap removed the segment the base needs. The restore
    // reports the gap instead of producing a store that never existed.
    let forced = archive::prune_archive(&archive_dir, now, day * 7, Some(1)).unwrap();
    assert_eq!(forced.forced, 1);
    let gapped = root.join("gapped");
    archive::restore_base_into(&base_dir, &gapped).unwrap();
    {
        let node = Node::bootstrap(&mut config(&gapped)).unwrap();
        let mut stream = archive::segment_stream(&archive_dir).unwrap();
        let replayed = node.restore_pitr_to(&mut stream, Some(30), Some(60));
        assert!(
            replayed.is_err(),
            "restore past the archive must fail: {replayed:?}"
        );
        assert_eq!(node.committed_version(), 30);
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn offline_base_backup_after_an_unclean_stop_holds_every_commit() {
    let root = rushdb_storage::TempPath::dir("offline-base");
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    {
        let node = Node::bootstrap(&mut config(&data)).unwrap();
        write_range(&node, 0, 40);
        // Dropped without the server's final checkpoint: the latest commits
        // may live only in the WAL.
    }
    let node = Node::bootstrap(&mut config(&data)).unwrap();
    let expected = snapshot_bytes(&node);
    let committed = node.committed_version();
    let manifest = node.backup_base_offline(&root.join("base")).unwrap();
    assert_eq!(manifest.commit_index, committed);
    drop(node);
    let restored = root.join("restored");
    archive::restore_base_into(&root.join("base"), &restored).unwrap();
    let restored_node = Node::bootstrap(&mut config(&restored)).unwrap();
    assert_eq!(snapshot_bytes(&restored_node), expected);
    drop(restored_node);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn operators_can_take_a_base_backup_of_a_running_server() {
    use std::io::{Read, Write};
    let root = rushdb_storage::TempPath::dir("online-base");
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let mut cfg = ServerConfig {
        token: "operator-token".into(),
        // An ephemeral port: the default can be taken on a developer host.
        port: 0,
        ..config(&data)
    };
    let node = std::sync::Arc::new(Node::bootstrap(&mut cfg).unwrap());
    write_range(&node, 0, 25);
    let expected = snapshot_bytes(&node);
    let (mut router, port) =
        rushdb_server::http::bind_router(cfg, std::sync::Arc::clone(&node)).unwrap();
    std::thread::spawn(move || {
        let _ = router.serve_forever();
    });
    let base = root.join("base");
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "POST /internal/backup/base?out={} HTTP/1.1\r\nHost: x\r\nX-RushDB-Token: operator-token\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        base.display()
    )
    .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    let restored = root.join("restored");
    archive::restore_base_into(&base, &restored).unwrap();
    let restored_node = Node::bootstrap(&mut config(&restored)).unwrap();
    assert_eq!(snapshot_bytes(&restored_node), expected);
    drop(restored_node);
    let _ = std::fs::remove_dir_all(root);
}
