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
    let (stream, raw) = head_only_raw(port, token, content_length);
    let status = raw.lines().next().unwrap_or("").to_string();
    (stream, status)
}

/// As [`head_only`], returning the whole answer the server sends.
fn head_only_raw(port: u16, token: &str, content_length: usize) -> (TcpStream, String) {
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
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    (stream, String::from_utf8_lossy(&buf[..n]).to_string())
}

/// The JSON body of a raw HTTP answer.
fn body_json(raw: &str) -> serde_json::Value {
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("");
    serde_json::from_str(body).unwrap_or_else(|_| panic!("not a JSON body: {raw:?}"))
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
fn oversized_declared_body_is_refused_up_front_with_the_protocol_envelope() {
    let port = start(ServerConfig {
        port: 0,
        pgwire_port: 0,
        max_request_bytes: 1024 * 1024,
        ..Default::default()
    });
    let (_stream, raw) = head_only_raw(port, "local-token", 2 * 1024 * 1024);
    let status = raw.lines().next().unwrap_or("");
    assert!(status.contains("413"), "got {status:?}");
    // The envelope a JSON client can read, not a body-limit layer's
    // `length limit exceeded` text.
    assert!(
        raw.to_ascii_lowercase()
            .contains("content-type: application/json"),
        "{raw:?}"
    );
    let body = body_json(&raw);
    assert_eq!(body["protocolVersion"], 1, "{body}");
    assert_eq!(body["error"]["code"], "payload_too_large", "{body}");
    assert_eq!(body["error"]["retryable"], false, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("1048576-byte limit"),
        "{body}"
    );
}

#[test]
fn an_unauthenticated_oversized_body_is_refused_as_unauthorized() {
    // Authentication comes before the size check (C7): an unauthenticated
    // caller learns nothing about the limit.
    let port = start(ServerConfig {
        port: 0,
        pgwire_port: 0,
        max_request_bytes: 1024 * 1024,
        ..Default::default()
    });
    let (_stream, status) = head_only(port, "wrong-token", 2 * 1024 * 1024);
    assert!(status.contains("401"), "got {status:?}");
}

#[test]
fn an_undeclared_body_over_the_limit_is_refused_with_the_envelope() {
    // Chunked bodies declare no length: the limit holds while streaming.
    let port = start(ServerConfig {
        port: 0,
        pgwire_port: 0,
        max_request_bytes: 64 * 1024,
        ..Default::default()
    });
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "POST /protocol/v1/execute HTTP/1.1\r\nhost: x\r\nauthorization: Bearer local-token\r\n\
         content-type: application/json\r\ntransfer-encoding: chunked\r\n\r\n"
    )
    .unwrap();
    let chunk = "x".repeat(16 * 1024);
    for _ in 0..8 {
        if write!(stream, "{:x}\r\n{chunk}\r\n", chunk.len()).is_err() {
            break;
        }
    }
    let _ = write!(stream, "0\r\n\r\n");
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let raw = String::from_utf8_lossy(&raw).to_string();
    assert!(raw.lines().next().unwrap_or("").contains("413"), "{raw:?}");
    assert_eq!(
        body_json(&raw)["error"]["code"],
        "payload_too_large",
        "{raw:?}"
    );
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
