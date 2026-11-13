//! C6 (cloud-capacity F5): a signalled shutdown must close redb cleanly, even
//! while maintenance and writes are in flight, so the next open needs no
//! repair (which is O(file) on a large store).

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rushdb_server::http::new_router;
use rushdb_server::ServerConfig;

fn post(port: u16, body: &str) {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let request = format!(
        "POST /protocol/v1/execute HTTP/1.1\r\nhost: x\r\nauthorization: Bearer local-token\r\n\
         content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
}

#[test]
fn signalled_shutdown_leaves_redb_without_repair() {
    let dir = rushdb_storage::TempPath::dir("clean-shutdown");
    let mut cfg = ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        port: 0,
        pgwire_port: 0,
        ..Default::default()
    };
    // Tick maintenance (GC, retention, checkpoint) constantly during the test.
    cfg.resources.checkpoint_interval_seconds = 1;
    let (mut router, port) = new_router(cfg).unwrap();
    let stop = router.shutdown_handle();
    let server = std::thread::spawn(move || router.serve_forever());

    let writing = Arc::new(AtomicBool::new(true));
    let writer = {
        let writing = Arc::clone(&writing);
        std::thread::spawn(move || {
            let docs: Vec<String> = (0..200).map(|i| format!(r#"{{"n":{i}}}"#)).collect();
            let body = format!(
                r#"{{"protocolVersion":1,"context":{{}},"operation":{{"kind":"ingest","ingest":{{"kind":"documents","label":"Doc","documents":[{}],"options":{{}}}}}}}}"#,
                docs.join(",")
            );
            while writing.load(Ordering::Acquire) {
                post(port, &body);
            }
        })
    };
    std::thread::sleep(Duration::from_millis(2_500));
    stop();
    writing.store(false, Ordering::Release);
    writer.join().unwrap();
    server
        .join()
        .unwrap()
        .expect("shutdown must report success");

    let repaired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&repaired);
    let db = redb::Database::builder()
        .set_repair_callback(move |_| flag.store(true, Ordering::SeqCst))
        .open(dir.join("rushdb.redb"))
        .expect("reopen redb");
    drop(db);
    assert!(
        !repaired.load(Ordering::SeqCst),
        "a clean shutdown must not leave redb needing repair"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// Whether opening the store file again needs a repair.
fn needs_repair(path: &std::path::Path) -> bool {
    let repaired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&repaired);
    let db = redb::Database::builder()
        .set_repair_callback(move |_| flag.store(true, Ordering::SeqCst))
        .open(path)
        .expect("reopen redb");
    drop(db);
    repaired.load(Ordering::SeqCst)
}

#[test]
fn a_shutdown_stops_a_long_read_and_closes_the_store() {
    // SHUT-1: a request still running at shutdown held the node past the
    // drain, so redb was never closed and the next open repaired the file.
    // The shutdown cancels it; the node is released and redb closes.
    let dir = rushdb_storage::TempPath::dir("shutdown-long-read");
    let cfg = ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        port: 0,
        pgwire_port: 0,
        ..Default::default()
    };
    let (mut router, port) = new_router(cfg).unwrap();
    let stop = router.shutdown_handle();
    let node = router.node();
    let server = std::thread::spawn(move || router.serve_forever());
    let docs: Vec<String> = (0..600).map(|i| format!(r#"{{"n":{i}}}"#)).collect();
    post(
        port,
        &format!(
            r#"{{"protocolVersion":1,"context":{{}},"operation":{{"kind":"ingest","ingest":{{"kind":"documents","label":"Doc","documents":[{}],"options":{{}}}}}}}}"#,
            docs.join(",")
        ),
    );
    // 600³ rows to combine: far longer than the test waits.
    let reader = std::thread::spawn(move || {
        post(
            port,
            r#"{"protocolVersion":1,"context":{},"operation":{"kind":"query","query":{"kind":"statement","language":"sql","statement":"SELECT max(a.n + b.n + c.n) AS m FROM doc a, doc b, doc c"}}}"#,
        );
    });
    std::thread::sleep(Duration::from_millis(1_000));
    let stopped = std::time::Instant::now();
    stop();
    let _ = server.join().unwrap();
    assert_eq!(
        rushdb_server::http::node_released(&node, Duration::from_secs(20)),
        0,
        "the running read must let go of the node"
    );
    assert!(
        stopped.elapsed() < Duration::from_secs(15),
        "{:?}",
        stopped.elapsed()
    );
    reader.join().unwrap();
    assert!(!needs_repair(&dir.join("rushdb.redb")));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_shutdown_cancels_requests_whatever_their_transport() {
    // pgwire, gRPC and Bolt run requests without a client-disconnect signal:
    // the store's shutdown is what stops them.
    let (router, _) = new_router(ServerConfig {
        port: 0,
        pgwire_port: 0,
        ..Default::default()
    })
    .unwrap();
    let stop = router.shutdown_handle();
    let node = router.node().upgrade().unwrap();
    let docs: Vec<serde_json::Value> = (0..600).map(|n| serde_json::json!({ "n": n })).collect();
    let ingest: rushdb_protocol::RequestEnvelope = serde_json::from_value(serde_json::json!({
        "protocolVersion": 1,
        "operation": {"kind": "ingest", "ingest": {"kind": "documents", "label": "Doc", "documents": docs}}
    }))
    .unwrap();
    rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(ingest)
        .unwrap();
    let running = Arc::clone(&node);
    let reader = std::thread::spawn(move || {
        let read: rushdb_protocol::RequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "operation": {"kind": "query", "query": {"kind": "statement", "language": "sql",
                "statement": "SELECT max(a.n + b.n + c.n) AS m FROM doc a, doc b, doc c"}}
        }))
        .unwrap();
        rushdb_server::protocol::ProtocolExecutor::new(&running, None)
            .execute_cancellable(read, &|| false)
    });
    std::thread::sleep(Duration::from_millis(500));
    let stopped = std::time::Instant::now();
    stop();
    let outcome = reader.join().unwrap();
    assert!(
        stopped.elapsed() < Duration::from_secs(5),
        "{:?}",
        stopped.elapsed()
    );
    let error = outcome.expect_err("a read running at shutdown is cancelled");
    assert_eq!(
        error.code,
        rushdb_protocol::ErrorCode::Cancelled,
        "{error:?}"
    );
}
