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
