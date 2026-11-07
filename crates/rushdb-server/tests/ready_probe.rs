//! `/ready` says no traffic while a storage compaction holds the database:
//! every request waits on it, and `/health` stays a liveness probe (a node
//! that answered 200 to both while no request could complete hid the
//! condition from its orchestrator).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use rushdb_server::http::new_router;
use rushdb_server::ServerConfig;

fn get(port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n"
    )
    .unwrap();
    let mut raw = String::new();
    let _ = stream.read_to_string(&mut raw);
    raw
}

#[test]
fn readiness_reports_a_compaction_that_holds_the_database() {
    let (mut router, port) = new_router(ServerConfig {
        port: 0,
        pgwire_port: 0,
        ..Default::default()
    })
    .unwrap();
    let store = router.store().expect("the node is alive");
    std::thread::spawn(move || router.serve_forever());
    std::thread::sleep(Duration::from_millis(50));

    assert!(get(port, "/ready").starts_with("HTTP/1.1 200"));
    store.mark_compacting(true);
    let raw = get(port, "/ready");
    assert!(raw.starts_with("HTTP/1.1 503"), "{raw}");
    assert!(
        raw.contains("storage compaction holds the database"),
        "{raw}"
    );
    // Liveness is unchanged: the process is up.
    assert!(get(port, "/health").starts_with("HTTP/1.1 200"));
    store.mark_compacting(false);
    assert!(get(port, "/ready").starts_with("HTTP/1.1 200"));
}
