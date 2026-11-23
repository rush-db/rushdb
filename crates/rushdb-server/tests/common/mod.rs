//! A server over HTTP for integration tests (tier 2): the real router with
//! its background workers (index builder and applier, GC, receipts), on a
//! free port, ready before the test sends anything. A test of a feature a
//! client reaches through background work drives it here, in the client's
//! own order of requests.
//!
//! `mod common;` in a test file; each file uses part of it.
#![allow(dead_code, reason = "each test file uses part of the helper")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use rushdb_server::{new_router, ServerConfig};
use serde_json::{json, Value};

/// The token a test server accepts.
pub const TOKEN: &str = "test-token";

/// A running server.
pub struct TestServer {
    /// Its HTTP port.
    pub port: u16,
}

/// An HTTP response: status and body (JSON when it parses, else a string).
pub struct Reply {
    /// The status code.
    pub status: u16,
    /// The body.
    pub body: Value,
}

impl TestServer {
    /// A single-tenant, in-memory server with the defaults.
    pub fn start() -> Self {
        Self::start_with(|_| {})
    }

    /// A server with the defaults, then `configure` (multitenant, a data
    /// directory, resources...).
    pub fn start_with(configure: impl FnOnce(&mut ServerConfig)) -> Self {
        let mut cfg = ServerConfig {
            port: 0,
            pgwire_port: 0,
            grpc_port: 0,
            bolt_port: 0,
            token: TOKEN.into(),
            max_request_bytes: 64 * 1024 * 1024,
            ..ServerConfig::default()
        };
        configure(&mut cfg);
        let (mut router, port) = new_router(cfg).expect("bootstrap");
        std::thread::spawn(move || router.serve_forever());
        let server = Self { port };
        let deadline = Instant::now() + Duration::from_secs(30);
        while server.get("/ready").status != 200 {
            assert!(
                Instant::now() < deadline,
                "the test server never became ready"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        server
    }

    /// `GET path`.
    pub fn get(&self, path: &str) -> Reply {
        self.request("GET", path, None, &[])
    }

    /// `POST path` with a JSON body and extra headers.
    pub fn post(&self, path: &str, body: &Value, headers: &[(&str, &str)]) -> Reply {
        self.request("POST", path, Some(body), headers)
    }

    /// Execute one protocol operation; panics unless it answers 200, and
    /// returns the envelope's `response`.
    pub fn execute(&self, operation: &Value) -> Value {
        let reply = self.execute_raw(operation);
        assert_eq!(reply.status, 200, "{operation}\n{}", reply.body);
        reply.body["response"].clone()
    }

    /// Execute one protocol operation; the reply as it is.
    pub fn execute_raw(&self, operation: &Value) -> Reply {
        self.post(
            "/protocol/v1/execute",
            &json!({"protocolVersion": 1, "context": {}, "operation": operation}),
            &[],
        )
    }

    /// Wait until every index is ready, then repeat `query` until it
    /// reports nothing pending (an `eventual` index); returns its response.
    pub fn settled(&self, query: &Value) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let indexes =
                self.execute(&json!({"kind": "describe", "describe": {"kind": "indexes"}}));
            let ready = indexes["indexes"]
                .as_array()
                .is_some_and(|all| all.iter().all(|index| index["state"] == "ready"));
            if ready {
                let response = self.execute(query);
                if response["pending"].as_u64().unwrap_or(0) == 0 {
                    return response;
                }
            }
            assert!(Instant::now() < deadline, "never settled: {indexes}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        headers: &[(&str, &str)],
    ) -> Reply {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let payload = body.map_or_else(String::new, Value::to_string);
        let mut extra = String::new();
        for (name, value) in headers {
            for piece in [name, ": ", value, "\r\n"] {
                extra.push_str(piece);
            }
        }
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        stream.write_all(request.as_bytes()).unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw.as_str(), ""));
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        let body = serde_json::from_str(body).unwrap_or_else(|_| Value::String(body.to_owned()));
        Reply { status, body }
    }
}
