//! Streaming replication. A follower polls a leader's
//! internal WAL-tail endpoint and replays each commit batch locally; this
//! proves it converges on pre-existing data and stays converged under
//! concurrent writes on the leader.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rushdb_server::{http, Node, ServerConfig};
use serde_json::json;

fn config(dir: &std::path::Path, token: &str) -> ServerConfig {
    let mut cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: token.to_string(),
        data_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    // These followers start empty and tail the leader from LSN 1. With the
    // default retention (keep only what a registered consumer needs) the
    // leader may prune that history before a follower's first poll, so the
    // leader keeps a window, as an operator would for bootstrap-from-empty.
    // Attaching to a leader that has pruned is the base-backup path
    // (`a_restored_base_backup_lets_a_follower_attach_mid_stream`).
    cfg.resources.tx_log_keep_commits = 100_000;
    cfg
}

fn write_range(node: &Node, from: u64, to: u64) {
    for i in from..to {
        node.scope()
            .create_record("S", &json!({ "u": i }), &[], &json!({}))
            .unwrap();
    }
}

/// Decode an `export_snapshot` stream into its tenant JSONL lines, sorted —
/// matching this project's own memory-vs-disk differential comparison
/// (`memory_disk_differential.rs::normalize`), since row export order is a
/// scan artifact, not a content guarantee.
fn normalized_lines(snapshot: &[u8]) -> Vec<String> {
    use rushdb_storage::wal::{decode_tenant_chunk, read_framed_chunk, read_snapshot_manifest};
    let mut cursor = std::io::Cursor::new(snapshot);
    let manifest = read_snapshot_manifest(&mut cursor).unwrap();
    let mut lines = Vec::new();
    for _ in 0..manifest.tenant_count {
        let raw = read_framed_chunk(&mut cursor).unwrap().unwrap();
        let chunk = decode_tenant_chunk(&raw).unwrap();
        let jsonl = String::from_utf8(chunk.jsonl).unwrap();
        lines.extend(jsonl.lines().map(str::to_string));
    }
    lines.sort();
    lines
}

fn wait_for_commit(node: &Node, target: u64, deadline: std::time::Instant) {
    while node.committed_version() < target {
        assert!(
            std::time::Instant::now() <= deadline,
            "follower did not catch up: follower={} leader={}",
            node.committed_version(),
            target
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn streaming_replication_follower_converges_under_concurrent_writes() {
    const TOKEN: &str = "shared-replication-test-token";
    let root = rushdb_storage::TempPath::dir("replication");
    let leader_dir = root.join("leader");
    let follower_dir = root.join("follower");
    std::fs::create_dir_all(&leader_dir).unwrap();
    std::fs::create_dir_all(&follower_dir).unwrap();

    // Pre-existing data: the follower must bootstrap-replicate this before
    // it ever sees a "live" write.
    let leader_node = Arc::new(Node::bootstrap(&mut config(&leader_dir, TOKEN)).unwrap());
    write_range(&leader_node, 0, 50);

    let (mut leader_router, leader_port) =
        http::bind_router(config(&leader_dir, TOKEN), Arc::clone(&leader_node)).unwrap();
    let _leader_thread = std::thread::spawn(move || {
        let _ = leader_router.serve_forever();
    });

    let mut follower_cfg = config(&follower_dir, TOKEN);
    follower_cfg.resources.replica_of = Some(format!("http://127.0.0.1:{leader_port}"));
    follower_cfg.resources.replica_id = "f1".into();
    let follower_node = Arc::new(Node::bootstrap(&mut follower_cfg).unwrap());
    let (mut follower_router, _follower_port) =
        http::bind_router(follower_cfg, Arc::clone(&follower_node)).unwrap();
    let _follower_thread = std::thread::spawn(move || {
        let _ = follower_router.serve_forever();
    });

    // The follower must catch up on data that existed before it ever started
    // following.
    wait_for_commit(
        &follower_node,
        leader_node.committed_version(),
        std::time::Instant::now() + Duration::from_secs(10),
    );

    // Now race a concurrent writer on the leader against the follower's poll
    // loop — the same shape as the online-backup concurrency test.
    let stop = Arc::new(AtomicBool::new(false));
    let writer_leader = Arc::clone(&leader_node);
    let writer_stop = Arc::clone(&stop);
    let writer = std::thread::spawn(move || {
        let mut i = 50u64;
        while !writer_stop.load(Ordering::Relaxed) {
            writer_leader
                .scope()
                .create_record("S", &json!({ "u": i }), &[], &json!({}))
                .unwrap();
            i += 1;
        }
    });
    std::thread::sleep(Duration::from_millis(300));
    stop.store(true, Ordering::Relaxed);
    writer.join().unwrap();

    let target = leader_node.committed_version();
    assert!(
        target > 50,
        "the concurrent writer must have committed something"
    );
    wait_for_commit(
        &follower_node,
        target,
        std::time::Instant::now() + Duration::from_secs(10),
    );

    let mut leader_snapshot = Vec::new();
    leader_node
        .export_snapshot(&mut leader_snapshot, &[])
        .unwrap();
    let mut follower_snapshot = Vec::new();
    follower_node
        .export_snapshot(&mut follower_snapshot, &[])
        .unwrap();
    assert_eq!(
        normalized_lines(&leader_snapshot),
        normalized_lines(&follower_snapshot),
        "follower state matches the leader's state"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// A follower rejects local writes while following, and
/// promotion (`Node::request_promotion`) both stops the replication task
/// and starts allowing writes — but not before the task actually stops,
/// which this proves by showing further leader writes no longer reach the
/// promoted node.
#[test]
fn promoted_follower_accepts_writes_and_stops_tailing_the_leader() {
    use rushdb_protocol::{Mutation, Operation, PutMode, RequestContext, RequestEnvelope};
    use rushdb_server::protocol::ProtocolExecutor;

    const TOKEN: &str = "shared-promotion-test-token";
    let root = rushdb_storage::TempPath::dir("promotion");
    let leader_dir = root.join("leader");
    let follower_dir = root.join("follower");
    std::fs::create_dir_all(&leader_dir).unwrap();
    std::fs::create_dir_all(&follower_dir).unwrap();

    let leader_node = Arc::new(Node::bootstrap(&mut config(&leader_dir, TOKEN)).unwrap());
    write_range(&leader_node, 0, 10);
    let (mut leader_router, leader_port) =
        http::bind_router(config(&leader_dir, TOKEN), Arc::clone(&leader_node)).unwrap();
    let _leader_thread = std::thread::spawn(move || {
        let _ = leader_router.serve_forever();
    });

    let mut follower_cfg = config(&follower_dir, TOKEN);
    follower_cfg.resources.replica_of = Some(format!("http://127.0.0.1:{leader_port}"));
    follower_cfg.resources.replica_id = "f1".into();
    let follower_node = Arc::new(Node::bootstrap(&mut follower_cfg).unwrap());
    assert!(
        follower_node.is_following(),
        "configured with --replica-of, must start read-only"
    );
    let (mut follower_router, follower_port) =
        http::bind_router(follower_cfg, Arc::clone(&follower_node)).unwrap();
    let _follower_thread = std::thread::spawn(move || {
        let _ = follower_router.serve_forever();
    });

    wait_for_commit(
        &follower_node,
        leader_node.committed_version(),
        std::time::Instant::now() + Duration::from_secs(10),
    );

    let write_envelope = || RequestEnvelope {
        protocol_version: 1,
        request_id: Some("promotion-test".into()),
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::PutRecord {
                mode: PutMode::Create,
                id: None,
                label: Some("S".into()),
                properties: json!({ "u": 999 }),
                vectors: vec![],
                options: json!({}),
            },
        },
    };

    let rejected = ProtocolExecutor::new(&follower_node, None).execute(write_envelope());
    assert!(
        rejected.is_err(),
        "a following node must reject a local write, got {rejected:?}"
    );
    let message = format!("{:?}", rejected.unwrap_err());
    assert!(
        message.contains("follower"),
        "rejection should name the reason: {message}"
    );

    // Promote through the operator endpoint, which also persists the role.
    let promoted = {
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", follower_port)).unwrap();
        write!(
            stream,
            "POST /internal/replication/promote HTTP/1.1\r\nHost: x\r\nX-RushDB-Token: {TOKEN}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        reply
    };
    assert!(promoted.contains(r#""persisted":true"#), "{promoted}");
    assert!(!follower_node.is_following());
    let stored = rushdb_server::config::ConfigFile::load(&follower_dir)
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.resources.and_then(|resources| resources.replica_of),
        None,
        "a restart must not follow the former leader again"
    );

    let accepted = ProtocolExecutor::new(&follower_node, None).execute(write_envelope());
    assert!(
        accepted.is_ok(),
        "a promoted node must accept a local write, got {accepted:?}"
    );
    let promoted_version = follower_node.committed_version();

    // Further leader writes must no longer reach the promoted node — the
    // replication task has actually stopped, not just "writes happen to
    // be allowed now while it keeps running in the background".
    write_range(&leader_node, 10, 20);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        follower_node.committed_version(),
        promoted_version,
        "a promoted follower must not keep tailing its former leader"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// Streaming replication with a multi-shard config (S-2, `--experimental-sharding`):
/// replication is shard-count-agnostic by construction — `Store`'s WAL is
/// one global log regardless of `shards`, and `apply_replicated_batch_with_payload`
/// re-derives the target shard from the batch's tenant
/// (`physical_shard_for`) rather than from anything baked into the
/// replicated record, so a follower configured with the same `shards`
/// count as its leader should converge the same way a single-shard one
/// does. This only proves the *mechanism* tolerates a multi-shard config
/// without breaking (single-tenant mode still routes every record to one
/// fixed shard, so it does not exercise data actually landing on more
/// than one shard, or `shard_count` mismatches between leader and
/// follower) — real cross-shard routing diversity needs multitenancy,
/// which this file's helpers do not set up. Multi-shard commit already
/// carries its own known tearing risk independent of replication;
/// this test does not change that.
#[test]
fn streaming_replication_tolerates_a_multi_shard_config() {
    const TOKEN: &str = "shared-multishard-test-token";
    let root = rushdb_storage::TempPath::dir("multishard");
    let leader_dir = root.join("leader");
    let follower_dir = root.join("follower");
    std::fs::create_dir_all(&leader_dir).unwrap();
    std::fs::create_dir_all(&follower_dir).unwrap();

    let sharded_config = |dir: &std::path::Path| {
        let mut cfg = config(dir, TOKEN);
        cfg.resources.shards = 2;
        cfg.resources.experimental_sharding = true;
        cfg
    };

    let leader_node = Arc::new(Node::bootstrap(&mut sharded_config(&leader_dir)).unwrap());
    write_range(&leader_node, 0, 30);
    let (mut leader_router, leader_port) =
        http::bind_router(sharded_config(&leader_dir), Arc::clone(&leader_node)).unwrap();
    let _leader_thread = std::thread::spawn(move || {
        let _ = leader_router.serve_forever();
    });

    let mut follower_cfg = sharded_config(&follower_dir);
    follower_cfg.resources.replica_of = Some(format!("http://127.0.0.1:{leader_port}"));
    follower_cfg.resources.replica_id = "f1".into();
    let follower_node = Arc::new(Node::bootstrap(&mut follower_cfg).unwrap());
    let (mut follower_router, _follower_port) =
        http::bind_router(follower_cfg, Arc::clone(&follower_node)).unwrap();
    let _follower_thread = std::thread::spawn(move || {
        let _ = follower_router.serve_forever();
    });

    wait_for_commit(
        &follower_node,
        leader_node.committed_version(),
        std::time::Instant::now() + Duration::from_secs(10),
    );

    let mut leader_snapshot = Vec::new();
    leader_node
        .export_snapshot(&mut leader_snapshot, &[])
        .unwrap();
    let mut follower_snapshot = Vec::new();
    follower_node
        .export_snapshot(&mut follower_snapshot, &[])
        .unwrap();
    assert_eq!(
        normalized_lines(&leader_snapshot),
        normalized_lines(&follower_snapshot),
        "follower state matches the leader's state under a multi-shard config"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// Mid-stream attach: `run_replication_follower` already
/// resumes from `committed_version() + 1`, whatever that already is — it
/// never assumed 0. What a *new* follower actually needs to not start
/// from empty is a non-zero, leader-consistent starting commit index,
/// which Phase 3's existing online base backup + restore already
/// provides. No new replication-specific code is needed: this composes
/// two already-proven mechanisms (`Node::backup_base_online` /
/// `archive::restore_base_into`, proven by `pitr.rs`, and the Phase 4
/// poll loop, proven by the tests above) rather than building a parallel
/// network-fetch path.
#[test]
fn a_restored_base_backup_lets_a_follower_attach_mid_stream() {
    const TOKEN: &str = "shared-midstream-test-token";
    let root = rushdb_storage::TempPath::dir("midstream");
    let leader_dir = root.join("leader");
    let base_dir = root.join("base");
    let follower_dir = root.join("follower");
    std::fs::create_dir_all(&leader_dir).unwrap();

    let leader_node = Arc::new(Node::bootstrap(&mut config(&leader_dir, TOKEN)).unwrap());
    write_range(&leader_node, 0, 50);

    // Take an online base backup partway through the leader's history —
    // the follower attaches starting from THIS point, not from LSN 1.
    let manifest = leader_node.backup_base_online(&base_dir).unwrap();
    assert!(manifest.commit_index >= 50);

    let (mut leader_router, leader_port) =
        http::bind_router(config(&leader_dir, TOKEN), Arc::clone(&leader_node)).unwrap();
    let _leader_thread = std::thread::spawn(move || {
        let _ = leader_router.serve_forever();
    });

    // More writes on the leader *after* the base was taken and *before*
    // the follower ever starts — a genuine mid-stream attach, not a base
    // restore immediately followed by catching up on nothing new.
    write_range(&leader_node, 50, 80);

    rushdb_server::archive::restore_base_into(&base_dir, &follower_dir).unwrap();
    let mut follower_cfg = config(&follower_dir, TOKEN);
    follower_cfg.resources.replica_of = Some(format!("http://127.0.0.1:{leader_port}"));
    follower_cfg.resources.replica_id = "f1".into();
    let follower_node = Arc::new(Node::bootstrap(&mut follower_cfg).unwrap());
    assert_eq!(
        follower_node.committed_version(),
        manifest.commit_index,
        "restoring the base must resume exactly where it was taken, not from empty"
    );
    let (mut follower_router, _follower_port) =
        http::bind_router(follower_cfg, Arc::clone(&follower_node)).unwrap();
    let _follower_thread = std::thread::spawn(move || {
        let _ = follower_router.serve_forever();
    });

    wait_for_commit(
        &follower_node,
        leader_node.committed_version(),
        std::time::Instant::now() + Duration::from_secs(10),
    );

    let mut leader_snapshot = Vec::new();
    leader_node
        .export_snapshot(&mut leader_snapshot, &[])
        .unwrap();
    let mut follower_snapshot = Vec::new();
    follower_node
        .export_snapshot(&mut follower_snapshot, &[])
        .unwrap();
    assert_eq!(
        normalized_lines(&leader_snapshot),
        normalized_lines(&follower_snapshot),
        "mid-stream-attached follower converges to the leader's full state"
    );

    std::fs::remove_dir_all(&root).ok();
}
