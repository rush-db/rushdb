//! A protocol request whose HTTP client goes away is cancelled: the worker
//! running it stops at its next cancellation check instead of finishing work
//! nobody will read.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use rushdb_server::{new_router, ServerConfig};
use serde_json::{json, Value};

const RING: usize = 2_000;

fn start() -> u16 {
    let (mut router, port) = new_router(ServerConfig {
        port: 0,
        pgwire_port: 0,
        ..Default::default()
    })
    .unwrap();
    std::thread::spawn(move || router.serve_forever());
    port
}

fn send(port: u16, method: &str, path: &str, body: &str) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nhost: x\r\nauthorization: Bearer local-token\r\n\
         content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    stream
}

fn execute(port: u16, operation: &Value) -> Value {
    let body = json!({ "protocolVersion": 1, "context": {}, "operation": operation });
    let mut stream = send(port, "POST", "/protocol/v1/execute", &body.to_string());
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{raw}");
    serde_json::from_str(body).unwrap()
}

/// Requests the scheduler is running right now.
fn scheduler_inflight(port: u16) -> u64 {
    let mut stream = send(port, "GET", "/metrics", "");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    raw.lines()
        .find_map(|line| line.strip_prefix("rushdb_scheduler_inflight "))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or_else(|| panic!("no scheduler gauge in {raw}"))
}

fn wait_for(what: &str, within: Duration, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(started.elapsed() < within, "{what} within {within:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_query_stops_when_its_client_disconnects() {
    let port = start();
    execute(
        port,
        &json!({ "kind": "ingest", "ingest": {
            "kind": "documents",
            "documents": { "NODE": (0..RING).map(|i| json!({ "n": i })).collect::<Vec<_>>() },
            "options": {}
        }}),
    );
    let listed = execute(
        port,
        &json!({ "kind": "query", "query": { "kind": "records", "query": {
            "labels": ["NODE"], "limit": RING
        }}}),
    );
    let mut ids: Vec<(u64, String)> = listed["response"]["records"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .map(|record| {
            (
                record["properties"]["n"]
                    .as_u64()
                    .unwrap_or_else(|| panic!("{record}")),
                record["id"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{record}"))
                    .to_string(),
            )
        })
        .collect();
    ids.sort();
    let pairs: Vec<Value> = (0..RING)
        .map(|i| json!({ "sourceId": ids[i].1, "targetId": ids[(i + 1) % RING].1 }))
        .collect();
    execute(
        port,
        &json!({ "kind": "mutate", "mutation": { "kind": "linkRecordsPairs", "command": {
            "relationType": "NEXT", "pairs": pairs
        }}}),
    );

    // Every record walks the whole ring looking for a name nobody has:
    // RING² visits, far longer than this test waits.
    let slow = json!({ "protocolVersion": 1, "context": {}, "operation": {
        "kind": "query", "query": { "kind": "records", "query": {
            "labels": ["NODE"],
            "predicate": { "NODE": {
                "$relation": { "type": "NEXT", "direction": "out", "hops": { "min": 1, "max": 100_000 } },
                "name": "nowhere"
            }}
        }}
    }});
    let client = send(port, "POST", "/protocol/v1/execute", &slow.to_string());
    wait_for("the query to start", Duration::from_secs(10), || {
        scheduler_inflight(port) == 1
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(scheduler_inflight(port), 1, "the query is still running");

    drop(client);
    wait_for(
        "the worker to stop after the disconnect",
        Duration::from_secs(3),
        || scheduler_inflight(port) == 0,
    );
}
