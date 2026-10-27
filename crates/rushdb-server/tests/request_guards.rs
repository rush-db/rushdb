//! C7 (cloud-capacity F8): request bodies are not read before the caller is
//! authenticated, an oversized declared body is refused up front, and the
//! node-wide in-flight body budget sheds load instead of buffering it.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use rushdb_server::http::new_router;
use rushdb_server::ServerConfig;

fn start(cfg: ServerConfig) -> u16 {
    let (mut router, port) = new_router(cfg).unwrap();
    std::thread::spawn(move || router.serve_forever());
    port
}

/// Send only the request head (declaring `content_length`), never the body,
/// and return the status line the server answers with.
fn head_only(port: u16, token: &str, content_length: usize) -> (TcpStream, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "POST /protocol/v1/execute HTTP/1.1\r\nhost: x\r\nauthorization: Bearer {token}\r\n\
         content-type: application/json\r\ncontent-length: {content_length}\r\n\r\n"
    )
    .unwrap();
    let mut buf = [0u8; 256];
    let n = stream.read(&mut buf).unwrap_or(0);
    let status = String::from_utf8_lossy(&buf[..n])
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    (stream, status)
}

#[test]
fn unauthenticated_request_is_refused_before_its_body_is_read() {
    let port = start(ServerConfig {
        port: 0,
        pgwire_port: 0,
        ..Default::default()
    });
    // The 1 MiB body is never sent: a 401 can only arrive if the server
    // rejected the request from its headers alone.
    let (_stream, status) = head_only(port, "wrong-token", 1024 * 1024);
    assert!(status.contains("401"), "got {status:?}");
}

#[test]
fn oversized_declared_body_is_refused_up_front() {
    let port = start(ServerConfig {
        port: 0,
        pgwire_port: 0,
        max_request_bytes: 1024 * 1024,
        ..Default::default()
    });
    let (_stream, status) = head_only(port, "local-token", 2 * 1024 * 1024);
    assert!(status.contains("413"), "got {status:?}");
}

#[test]
fn inflight_body_budget_sheds_requests_before_buffering_them() {
    let mut cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        max_request_bytes: 1024 * 1024,
        ..Default::default()
    };
    cfg.resources.http_inflight_body_bytes =
        rushdb_server::resource_config::ByteSize(2 * 1024 * 1024);
    let port = start(cfg);
    let mut holders = Vec::new();
    // Two authenticated requests declare 1 MiB each and stall before sending
    // their bodies: they hold the whole 2 MiB budget.
    for _ in 0..2 {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "POST /protocol/v1/execute HTTP/1.1\r\nhost: x\r\nauthorization: Bearer local-token\r\n\
             content-type: application/json\r\ncontent-length: {}\r\n\r\n",
            1024 * 1024
        )
        .unwrap();
        holders.push(stream);
    }
    std::thread::sleep(Duration::from_millis(300));
    let (_stream, status) = head_only(port, "local-token", 1024 * 1024);
    assert!(
        status.contains("429"),
        "third request must be shed as resource_exhausted, got {status:?}"
    );
    drop(holders);
}
