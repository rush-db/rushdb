//! An authentication or permission refusal on `/protocol/v1/execute` is a
//! protocol error like every other failure: HTTP 401 with
//! `{"protocolVersion":1,"error":{"code":"unauthorized","message",…}}`, the
//! message saying what was missing and never echoing the credential. The
//! operator endpoints (`/metrics`, `/internal/*`) keep their plain
//! `{"error":"unauthorized"}`.

use std::io::{Read, Write};
use std::sync::Arc;

use rushdb_server::{Node, ServerConfig};
use serde_json::Value;

const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

fn serve() -> u16 {
    let mut cfg = ServerConfig {
        port: 0,
        token: "boot-token-0123456789abcdef".into(),
        capability_secret: Some(SECRET.to_vec()),
        ..ServerConfig::default()
    };
    let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
    let (mut router, port) = rushdb_server::http::bind_router(cfg, node).unwrap();
    std::thread::spawn(move || {
        let _ = router.serve_forever();
    });
    port
}

fn request(port: u16, path: &str, token: Option<&str>, body: &str) -> (u16, Value) {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let auth = token.map_or(String::new(), |token| {
        format!("Authorization: Bearer {token}\r\n")
    });
    let method = if body.is_empty() { "GET" } else { "POST" };
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: x\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response[9..12].parse().unwrap();
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default();
    let json_start = body.find('{').unwrap_or(0);
    let json_end = body.rfind('}').map_or(body.len(), |end| end + 1);
    (
        status,
        serde_json::from_str(&body[json_start..json_end]).unwrap(),
    )
}

const WRITE: &str = r#"{"protocolVersion":1,"operation":{"kind":"mutate","mutation":{"kind":"putRecord","label":"DOC","properties":{"n":1}}}}"#;
const READ: &str =
    r#"{"protocolVersion":1,"operation":{"kind":"describe","describe":{"kind":"buildInfo"}}}"#;

/// The envelope's error, after checking the shape every protocol error has.
fn refusal(status: u16, body: &Value) -> (String, String) {
    assert_eq!(status, 401, "{body}");
    assert_eq!(body["protocolVersion"], 1, "{body}");
    assert_eq!(body["error"]["code"], "unauthorized", "{body}");
    assert_eq!(body["error"]["retryable"], false, "{body}");
    let message = body["error"]["message"]
        .as_str()
        .expect("message")
        .to_string();
    (message, body.to_string())
}

#[test]
fn a_bad_or_missing_credential_gets_the_protocol_envelope() {
    let port = serve();
    let bad = "not-the-token-but-a-guess-0123456789";
    let (status, body) = request(port, "/protocol/v1/execute", Some(bad), READ);
    let (message, raw) = refusal(status, &body);
    assert_eq!(message, "invalid token");
    assert!(!raw.contains(bad), "the credential is echoed: {raw}");

    let (status, body) = request(port, "/protocol/v1/execute", None, READ);
    let (message, _) = refusal(status, &body);
    assert!(message.starts_with("missing credential"), "{message}");
}

#[test]
fn a_read_only_capability_writing_gets_the_protocol_envelope() {
    let port = serve();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let reader = rushdb_server::capability::issue(SECRET, "svc", "read", 3600, now);
    let (status, body) = request(port, "/protocol/v1/execute", Some(&reader), READ);
    assert_eq!(status, 200, "{body}");
    let (status, body) = request(port, "/protocol/v1/execute", Some(&reader), WRITE);
    let (message, raw) = refusal(status, &body);
    assert!(message.contains("`write`"), "{message}");
    assert!(!raw.contains(&reader), "the credential is echoed: {raw}");
}

#[test]
fn operator_endpoints_keep_their_plain_refusal() {
    let port = serve();
    let (status, body) = request(port, "/metrics", Some("wrong"), "");
    assert_eq!(
        (status, body),
        (401, serde_json::json!({ "error": "unauthorized" }))
    );
}
