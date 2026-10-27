//! Bolt (Neo4j binary wire protocol) transport for the canonical protocol.
//!
//! A hand-rolled Bolt v5 server that maps Bolt messages onto the canonical
//! envelopes and executes them through `rushdb_protocol::EnvelopeHandler`
//! (the same surface gRPC and HTTP use). Cypher statements run through the
//! existing language layer — the engine already executes openCypher — so this
//! crate is a protocol translator, not a query engine.
//!
//! Supported: HELLO (basic auth), RUN/PULL (read + write Cypher), BEGIN /
//! COMMIT / ROLLBACK (cross-service transactions), RESET / DISCARD, ROUTE,
//! GOODBYE.

pub mod value;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Number, Value};

use rushdb_protocol::{
    EnvelopeHandler, ErrorCode, LanguageSet, Operation, ProtocolError, Query, RequestContext,
    RequestEnvelope, StatementLanguage, TransactionMutation, TransactionOperation,
};

/// Magic bytes that begin every Bolt handshake.
const MAGIC: [u8; 4] = [0x60, 0x60, 0xB0, 0x17];
/// The Bolt version we negotiate (Bolt 5.0).
const VERSION_5_0: [u8; 4] = [0x00, 0x00, 0x00, 0x05];

// Bolt struct tags.
const HELLO: u8 = 0x01;
const GOODBYE: u8 = 0x02;
const RESET: u8 = 0x0F;
const RUN: u8 = 0x10;
const BEGIN: u8 = 0x11;
const COMMIT: u8 = 0x12;
const ROLLBACK: u8 = 0x13;
const DISCARD: u8 = 0x2F;
const PULL: u8 = 0x3F;
const ROUTE: u8 = 0x42;
const SUCCESS: u8 = 0x70;
const RECORD: u8 = 0x71;
const IGNORED: u8 = 0x7E;
const FAILURE: u8 = 0x7F;

const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_PENDING_ROWS: usize = 100_000;
const MAX_CONNECTIONS: usize = 1024;
const CONNECTION_IO_TIMEOUT: Duration = Duration::from_secs(60);
const AUTH_IDLE_TIMEOUT: Duration = Duration::from_secs(10);
static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

struct ConnectionPermit;

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
    }
}

fn acquire_connection() -> Result<ConnectionPermit, String> {
    ACTIVE_CONNECTIONS
        .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |active| {
            (active < MAX_CONNECTIONS).then_some(active + 1)
        })
        .map(|_| ConnectionPermit)
        .map_err(|_| "bolt: connection limit reached".into())
}

/// Classifies a Cypher statement as a write exactly like Grafeo does
/// (`query_looks_like_mutation`), so read and write intents route to the
/// `Query::Statement` / `Mutation::Statement` envelopes respectively.
#[must_use]
pub fn looks_like_mutation(statement: &str) -> bool {
    // Invalid/unsupported syntax is sent through the mutation envelope so
    // the canonical frontend returns its structured parser error rather than
    // accidentally treating an unknown write as a read.
    rushdb_language::classify_cypher_mutation(statement).unwrap_or(true)
}

// ---- framing --------------------------------------------------------------

/// Read one full (de-chunked) Bolt message from the stream.
fn read_message(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut payload = Vec::new();
    loop {
        let mut len = [0u8; 2];
        stream
            .read_exact(&mut len)
            .map_err(|e| format!("bolt: read chunk header: {e}"))?;
        let chunk_len = u16::from_be_bytes(len) as usize;
        if chunk_len == 0 {
            // Chunk terminator: message complete.
            if payload.is_empty() {
                return Err("bolt: empty message".into());
            }
            return Ok(payload);
        }
        if payload.len() + chunk_len > MAX_MESSAGE_BYTES {
            return Err("bolt: message too large".into());
        }
        let start = payload.len();
        payload.resize(start + chunk_len, 0);
        stream
            .read_exact(&mut payload[start..])
            .map_err(|e| format!("bolt: read chunk: {e}"))?;
    }
}

/// Write one Bolt message with chunk framing.
fn write_message(stream: &mut TcpStream, payload: &[u8]) -> Result<(), String> {
    let mut offset = 0;
    while offset < payload.len() {
        let chunk = payload.len() - offset;
        let chunk = chunk.min(usize::from(u16::MAX));
        let len = u16::try_from(chunk)
            .expect("chunk is clamped to u16::MAX")
            .to_be_bytes();
        stream.write_all(&len).map_err(|e| e.to_string())?;
        stream
            .write_all(&payload[offset..offset + chunk])
            .map_err(|e| e.to_string())?;
        offset += chunk;
    }
    stream.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())
}

// ---- responses ------------------------------------------------------------

fn success(metadata: Map<String, Value>) -> Vec<u8> {
    let mut out = Vec::new();
    value::encode_struct(SUCCESS, vec![Value::Object(metadata)], &mut out);
    out
}

fn ignored() -> Vec<u8> {
    let mut out = Vec::new();
    value::encode_struct(IGNORED, Vec::new(), &mut out);
    out
}

/// Whether an encoded response is a FAILURE: a one-field struct is the
/// tiny-struct marker followed by its tag.
fn is_failure(message: &[u8]) -> bool {
    message.get(1) == Some(&FAILURE)
}

fn failure(code: &str, message: &str) -> Vec<u8> {
    let mut meta = Map::new();
    meta.insert("code".into(), Value::String(code.to_string()));
    meta.insert("message".into(), Value::String(message.to_string()));
    let mut out = Vec::new();
    value::encode_struct(FAILURE, vec![Value::Object(meta)], &mut out);
    out
}

fn record(values: Vec<Value>) -> Vec<u8> {
    let mut out = Vec::new();
    value::encode_struct(RECORD, vec![Value::Array(values)], &mut out);
    out
}

/// Neo4j-style error code for a protocol error.
fn neo_code(error: &ProtocolError) -> &'static str {
    // Shared error taxonomy: the same ErrorCode maps to the same Neo4j-style
    // status code on every adapter. Add below — do not reorder.
    match error.code {
        ErrorCode::UnsupportedVersion => "Neo.ClientError.Request.Invalid",
        ErrorCode::ProtocolViolation
        | ErrorCode::InvalidRequest
        | ErrorCode::LanguageDisabled
        | ErrorCode::Unsupported => "Neo.ClientError.Statement.SyntaxError",
        ErrorCode::Unauthorized | ErrorCode::MissingScope | ErrorCode::ScopeMismatch => {
            "Neo.ClientError.Security.Unauthorized"
        }
        ErrorCode::NotFound => "Neo.ClientError.Statement.EntityNotFound",
        ErrorCode::Conflict => "Neo.ClientError.Constraint.UniqueConstraintViolation",
        ErrorCode::WriteConflict => "Neo.TransientError.Transaction.DeadlockDetected",
        ErrorCode::Cancelled => "Neo.ClientError.Transaction.TransactionTerminated",
        ErrorCode::ResourceExhausted | ErrorCode::Unavailable => {
            "Neo.TransientError.General.DatabaseUnavailable"
        }
        ErrorCode::ExecutionFailed => "Neo.DatabaseError.Execution.Failed",
    }
}

// ---- per-connection state machine ------------------------------------------

struct Connection {
    stream: TcpStream,
    handler: Arc<dyn EnvelopeHandler>,
    token: String,
    /// HELLO authentication must succeed before any operation is dispatched.
    authenticated: bool,
    /// Multitenant scope selected by the HELLO `db` field.
    scope: Option<String>,
    /// Pending RUN result (buffered rows + column order).
    pending: Option<PendingResult>,
    /// Cross-service transaction opened by BEGIN.
    transaction_id: Option<String>,
    /// Set by a FAILURE: until RESET, every message but GOODBYE is IGNORED.
    failed: bool,
    /// Server identity used in routing metadata.
    address: String,
}

struct PendingResult {
    fields: Vec<String>,
    rows: Vec<Map<String, Value>>,
}

enum MessageResponse {
    Buffered(Vec<Vec<u8>>),
    StreamPull(Vec<Value>),
}

fn pull_window(fields: &[Value]) -> Result<usize, String> {
    if fields.len() > 1 {
        return Err("bolt: PULL/DISCARD accepts at most one metadata map".to_string());
    }
    let Some(first) = fields.first() else {
        return Ok(usize::MAX);
    };
    let Some(meta) = first.as_object() else {
        return Err("bolt: PULL/DISCARD metadata must be a map".to_string());
    };
    let Some(raw) = meta.get("n") else {
        return Ok(usize::MAX);
    };
    let n = raw
        .as_i64()
        .ok_or_else(|| "bolt: PULL/DISCARD `n` must be an integer".to_string())?;
    // Bolt uses -1 to request all remaining rows. Other negative windows are
    // malformed and must fail closed.
    if n == -1 {
        return Ok(usize::MAX);
    }
    let n = usize::try_from(n)
        .map_err(|_| "bolt: PULL/DISCARD `n` must be non-negative or -1".to_string())?;
    Ok(n)
}

fn discard_pending(pending: &mut PendingResult, window: usize) -> bool {
    let count = window.min(pending.rows.len());
    pending.rows.drain(..count);
    !pending.rows.is_empty()
}

impl Connection {
    /// Every envelope carries the HELLO-selected scope in its context too: the
    /// executor requires the envelope scope to match the transport scope.
    fn execute(&self, mut envelope: RequestEnvelope) -> Result<Value, ProtocolError> {
        envelope.context.scope.clone_from(&self.scope);
        let response = self.handler.execute(envelope, self.scope.clone())?;
        serde_json::to_value(response.response)
            .map_err(|e| ProtocolError::new(ErrorCode::ExecutionFailed, e.to_string()))
    }

    fn handle_decoded(&mut self, decoded: &Value) -> Result<MessageResponse, String> {
        let tag = value::struct_tag(decoded).ok_or("bolt: message is not a struct")?;
        let fields = value::struct_fields(decoded);
        if tag != HELLO && tag != GOODBYE && !self.authenticated {
            return Ok(MessageResponse::Buffered(vec![failure(
                "Neo.ClientError.Security.Unauthorized",
                "HELLO authentication is required before this operation.",
            )]));
        }
        if self.failed && tag != RESET && tag != GOODBYE {
            return Ok(MessageResponse::Buffered(vec![ignored()]));
        }
        match tag {
            HELLO => Ok(MessageResponse::Buffered(self.handle_hello(&fields))),
            RUN => self.handle_run(&fields).map(MessageResponse::Buffered),
            PULL => Ok(MessageResponse::StreamPull(fields)),
            BEGIN => self.handle_begin(&fields).map(MessageResponse::Buffered),
            COMMIT => Ok(MessageResponse::Buffered(self.handle_commit())),
            ROLLBACK => Ok(MessageResponse::Buffered(self.handle_rollback())),
            RESET => {
                // RESET clears a failure and ends any open transaction, so a
                // later RUN auto-commits instead of staging into an abandoned one.
                self.failed = false;
                self.pending = None;
                if let Some(tx_id) = self.transaction_id.take() {
                    if let Err(error) = self.rollback(tx_id) {
                        return Ok(MessageResponse::Buffered(vec![failure(
                            neo_code(&error),
                            &error.message,
                        )]));
                    }
                }
                Ok(MessageResponse::Buffered(vec![success(Map::new())]))
            }
            DISCARD => Ok(MessageResponse::Buffered(self.handle_discard(&fields))),
            ROUTE => Ok(MessageResponse::Buffered(self.handle_route(&fields))),
            GOODBYE => Ok(MessageResponse::Buffered(Vec::new())),
            other => Err(format!("bolt: unsupported message tag 0x{other:02x}")),
        }
    }

    fn handle_hello(&mut self, fields: &[Value]) -> Vec<Vec<u8>> {
        let meta = fields
            .first()
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let scheme = meta
            .get("scheme")
            .and_then(Value::as_str)
            .unwrap_or("basic");
        let credentials = meta
            .get("credentials")
            .and_then(Value::as_str)
            .unwrap_or("");
        let authorized = match scheme {
            "none" => self.token.is_empty(),
            _ => rushdb_protocol::constant_time_eq(credentials.as_bytes(), self.token.as_bytes()),
        };
        if !authorized {
            self.authenticated = false;
            return vec![failure(
                "Neo.ClientError.Security.Unauthorized",
                "The client is unauthorized due to authentication failure.",
            )];
        }
        self.authenticated = true;
        // The `db` field selects the tenant scope (multitenant mode); a
        // single-tenant server refuses statements sent under one.
        if let Some(db) = meta.get("db").and_then(Value::as_str) {
            if !db.is_empty() && !db.eq_ignore_ascii_case("neo4j") {
                self.scope = Some(db.to_string());
            }
        }
        let mut resp = Map::new();
        resp.insert("server".into(), Value::String("RushDB/1.0".into()));
        resp.insert("connection_id".into(), Value::String("bolt-1".into()));
        let hints = Map::new();
        resp.insert("hints".into(), Value::Object(hints));
        vec![success(resp)]
    }

    fn run_envelope(
        &self,
        statement: &str,
        parameters: Option<Value>,
    ) -> Result<Value, ProtocolError> {
        let language = StatementLanguage::Cypher;
        let mutation = looks_like_mutation(statement);
        // Inside an open transaction a write is staged; its exact delta commits
        // or rolls back with the transaction. A read runs against committed
        // data, as reads in a transaction do on every other transport.
        if let (Some(tx_id), true) = (&self.transaction_id, mutation) {
            let envelope = RequestEnvelope {
                protocol_version: 1,
                request_id: None,
                context: RequestContext::default(),
                operation: Operation::Transaction {
                    transaction: TransactionOperation::Stage {
                        transaction_id: tx_id.clone(),
                        mutation: TransactionMutation::Statement {
                            language,
                            statement: statement.to_string(),
                            parameters,
                        },
                    },
                },
            };
            return self.execute(envelope);
        }
        if mutation {
            let envelope = RequestEnvelope {
                protocol_version: 1,
                request_id: None,
                context: RequestContext::default(),
                operation: Operation::Mutate {
                    mutation: rushdb_protocol::Mutation::Statement {
                        language,
                        statement: statement.to_string(),
                        parameters,
                    },
                },
            };
            self.execute(envelope)
        } else {
            let envelope = RequestEnvelope {
                protocol_version: 1,
                request_id: None,
                context: RequestContext::default(),
                operation: Operation::Query {
                    query: Query::Statement {
                        language,
                        statement: statement.to_string(),
                        parameters,
                    },
                },
            };
            self.execute(envelope)
        }
    }

    fn handle_run(&mut self, fields: &[Value]) -> Result<Vec<Vec<u8>>, String> {
        let statement = fields
            .first()
            .and_then(Value::as_str)
            .ok_or("bolt: RUN needs a statement string")?
            .to_string();
        let parameters = fields.get(1).cloned();
        let result = self.run_envelope(&statement, parameters);
        match result {
            Ok(response) => {
                let rows = response
                    .get("rows")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if rows.len() > MAX_PENDING_ROWS {
                    return Ok(vec![failure(
                        "Neo.TransientError.General.DatabaseUnavailable",
                        "RUN result exceeds the bounded Bolt pending-result limit; use a smaller query page.",
                    )]);
                }
                let mut row_maps = Vec::with_capacity(rows.len());
                let mut fields_order: Vec<String> = Vec::new();
                for row in rows {
                    let obj = row.as_object().cloned().unwrap_or_default();
                    if fields_order.is_empty() {
                        fields_order = obj.keys().cloned().collect();
                    }
                    row_maps.push(obj);
                }
                let mut meta = Map::new();
                meta.insert(
                    "fields".into(),
                    Value::Array(
                        fields_order
                            .iter()
                            .map(|f| Value::String(f.clone()))
                            .collect(),
                    ),
                );
                let type_name = if looks_like_mutation(&statement) {
                    "w"
                } else {
                    "r"
                };
                meta.insert("type".into(), Value::String(type_name.into()));
                self.pending = Some(PendingResult {
                    fields: fields_order,
                    rows: row_maps,
                });
                Ok(vec![success(meta)])
            }
            Err(error) => {
                let code = neo_code(&error);
                Ok(vec![failure(code, &error.message)])
            }
        }
    }

    /// Stream a PULL response directly to the peer in bounded batches.
    ///
    /// The Bolt `n=-1` operation can legitimately return every pending row.
    /// Keeping the rows in `pending` is already bounded by `MAX_PENDING_ROWS`,
    /// but constructing a second vector containing every encoded response would
    /// unnecessarily double the peak memory. Each batch is encoded and written
    /// before the next one is drained.
    fn stream_pull(&mut self, fields: &[Value]) -> Result<(), String> {
        let Some(mut pending) = self.pending.take() else {
            self.failed = true;
            write_message(
                &mut self.stream,
                &failure(
                    "Neo.ClientError.Statement.ExecutionFailed",
                    "PULL with no pending RUN",
                ),
            )?;
            return Ok(());
        };
        let window = match pull_window(fields) {
            Ok(window) => window,
            Err(error) => {
                self.pending = Some(pending);
                self.failed = true;
                write_message(
                    &mut self.stream,
                    &failure("Neo.ClientError.Request.Invalid", &error),
                )?;
                return Ok(());
            }
        };
        let count = window.min(pending.rows.len());
        let fields_order = pending.fields.clone();
        // Borrow the requested prefix while encoding each record, then remove
        // it once the writes succeed. This preserves the protocol window
        // without allocating a second Vec containing the whole response.
        for row in pending.rows.iter().take(count) {
            let values = fields_order
                .iter()
                .map(|field| row.get(field).cloned().unwrap_or(Value::Null))
                .collect();
            write_message(&mut self.stream, &record(values))?;
        }
        pending.rows.drain(..count);
        let has_more = !pending.rows.is_empty();
        if has_more {
            self.pending = Some(pending);
        }
        let mut meta = Map::new();
        meta.insert("type".into(), Value::String("r".into()));
        if let Some(scope) = &self.scope {
            meta.insert("db".into(), Value::String(scope.clone()));
        }
        meta.insert("has_more".into(), Value::Bool(has_more));
        write_message(&mut self.stream, &success(meta))
    }

    fn handle_discard(&mut self, fields: &[Value]) -> Vec<Vec<u8>> {
        let Some(mut pending) = self.pending.take() else {
            return vec![failure(
                "Neo.ClientError.Statement.ExecutionFailed",
                "DISCARD with no pending RUN",
            )];
        };
        let window = match pull_window(fields) {
            Ok(window) => window,
            Err(error) => {
                self.pending = Some(pending);
                return vec![failure("Neo.ClientError.Request.Invalid", &error)];
            }
        };
        let has_more = discard_pending(&mut pending, window);
        if has_more {
            self.pending = Some(pending);
        }
        let mut meta = Map::new();
        meta.insert("type".into(), Value::String("r".into()));
        meta.insert("has_more".into(), Value::Bool(has_more));
        vec![success(meta)]
    }

    fn handle_begin(&mut self, _fields: &[Value]) -> Result<Vec<Vec<u8>>, String> {
        let envelope = RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Begin {
                    ttl_ms: None,
                    autocommit: false,
                },
            },
        };
        match self.execute(envelope) {
            Ok(response) => {
                let tx_id = response
                    .get("transactionId")
                    .or_else(|| response.get("transaction_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let tx_id = tx_id.ok_or("bolt: BEGIN response missing transaction id")?;
                self.transaction_id = Some(tx_id);
                Ok(vec![success(Map::new())])
            }
            Err(error) => Ok(vec![failure(neo_code(&error), &error.message)]),
        }
    }

    fn handle_commit(&mut self) -> Vec<Vec<u8>> {
        let Some(tx_id) = self.transaction_id.take() else {
            return vec![failure(
                "Neo.ClientError.Transaction.TransactionNotFound",
                "COMMIT with no open transaction",
            )];
        };
        let envelope = RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Commit {
                    transaction_id: tx_id,
                },
            },
        };
        match self.execute(envelope) {
            Ok(_) => vec![success(Map::new())],
            Err(error) => vec![failure(neo_code(&error), &error.message)],
        }
    }

    fn handle_rollback(&mut self) -> Vec<Vec<u8>> {
        let Some(tx_id) = self.transaction_id.take() else {
            return vec![failure(
                "Neo.ClientError.Transaction.TransactionNotFound",
                "ROLLBACK with no open transaction",
            )];
        };
        match self.rollback(tx_id) {
            Ok(_) => vec![success(Map::new())],
            Err(error) => vec![failure(neo_code(&error), &error.message)],
        }
    }

    fn rollback(&self, transaction_id: String) -> Result<Value, ProtocolError> {
        self.execute(RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Rollback { transaction_id },
            },
        })
    }

    fn handle_route(&mut self, _fields: &[Value]) -> Vec<Vec<u8>> {
        // Single node: a default routing table pointing at ourselves.
        let mut servers = Map::new();
        servers.insert(
            "addresses".into(),
            Value::Array(vec![Value::String(self.address.clone())]),
        );
        servers.insert("role".into(), Value::String("WRITE".into()));
        let mut routing = Map::new();
        routing.insert("ttl".into(), Number::from(3000).into());
        routing.insert("servers".into(), Value::Array(vec![Value::Object(servers)]));
        let mut rt = Map::new();
        rt.insert("routing".into(), Value::Object(routing));
        let mut meta = Map::new();
        meta.insert("rt".into(), Value::Object(rt));
        vec![success(meta)]
    }
}

// ---- server ---------------------------------------------------------------

/// Bolt transport server mirroring the gRPC/pgwire adapters.
pub struct BoltServer {
    handler: Option<Arc<dyn EnvelopeHandler>>,
    token: Option<String>,
    listener: Option<TcpListener>,
}

impl BoltServer {
    /// A server that does not bind (no bolt-port configured / cypher disabled).
    #[must_use]
    pub fn disabled() -> Self {
        BoltServer {
            handler: None,
            token: None,
            listener: None,
        }
    }

    /// Whether this server was created disabled (nothing will be served).
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.listener.is_none()
    }

    /// Pre-bind the listener and return the bound port.
    ///
    /// # Errors
    ///
    /// Returns an error if the address cannot be bound or its local address
    /// cannot be read.
    pub fn bind(
        handler: Arc<dyn EnvelopeHandler>,
        token: String,
        listen_addr: &str,
        port: u16,
    ) -> Result<(Self, u16), String> {
        let listener = TcpListener::bind((listen_addr, port)).map_err(|e| e.to_string())?;
        let bound = listener.local_addr().map_err(|e| e.to_string())?.port();
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("set bolt listener nonblocking: {e}"))?;
        Ok((
            BoltServer {
                handler: Some(handler),
                token: Some(token),
                listener: Some(listener),
            },
            bound,
        ))
    }

    /// Serve on the caller-owned thread, retaining and draining connection workers.
    ///
    /// Does nothing when the server was created with [`BoltServer::disabled`].
    ///
    /// # Errors
    /// Returns an error when a connection panics or exceeds the drain deadline.
    pub fn serve_forever(&mut self) -> Result<(), String> {
        let Some(listener) = self.listener.take() else {
            return Ok(());
        };
        let (Some(handler), Some(token)) = (self.handler.take(), self.token.take()) else {
            return Ok(());
        };
        let local_addr = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();
        let mut connections: Vec<(TcpStream, std::thread::JoinHandle<()>)> = Vec::new();
        while !handler.shutdown_requested() {
            // Reap completed sessions so an idle long-lived listener does not
            // retain one handle per historical connection.
            let mut index = 0;
            while index < connections.len() {
                if connections[index].1.is_finished() {
                    let (_, worker) = connections.swap_remove(index);
                    let _ = worker.join();
                } else {
                    index += 1;
                }
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    if connections.len() >= 1024 {
                        // Bound the thread/socket ownership ledger. The client
                        // may reconnect after existing sessions finish.
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        continue;
                    }
                    let Ok(control) = stream.try_clone() else {
                        continue;
                    };
                    let _ = stream.set_nodelay(true);
                    let handler = Arc::clone(&handler);
                    let token = token.clone();
                    let address = local_addr.clone();
                    connections.push((
                        control,
                        std::thread::spawn(move || {
                            let _ = handle_connection(stream, handler, token, address);
                        }),
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        drop(listener);
        // Close blocked handshake/read/write calls before joining. Rust cannot
        // forcibly terminate a provider executing inside a connection thread.
        for (socket, _) in &connections {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut failed = false;
        for (_, worker) in connections {
            while !worker.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if worker.is_finished() {
                failed |= worker.join().is_err();
            } else {
                // The process lifecycle owner must treat an uncooperative
                // provider as a failed shutdown, never a clean close.
                failed = true;
            }
        }
        if failed {
            Err("Bolt connection shutdown incomplete".into())
        } else {
            Ok(())
        }
    }
}

/// Serve a single Bolt connection (handshake + message loop).
///
/// # Errors
///
/// Returns an error if the handshake or a response cannot be written to the
/// socket.
pub fn handle_connection(
    stream: TcpStream,
    handler: Arc<dyn EnvelopeHandler>,
    token: String,
    address: String,
) -> Result<(), String> {
    handle_connection_with_timeouts(
        stream,
        handler,
        token,
        address,
        AUTH_IDLE_TIMEOUT,
        CONNECTION_IO_TIMEOUT,
    )
}

fn handle_connection_with_timeouts(
    mut stream: TcpStream,
    handler: Arc<dyn EnvelopeHandler>,
    token: String,
    address: String,
    auth_idle_timeout: Duration,
    connection_io_timeout: Duration,
) -> Result<(), String> {
    let _permit = acquire_connection()?;
    stream
        .set_read_timeout(Some(auth_idle_timeout))
        .map_err(|error| format!("bolt: set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(connection_io_timeout))
        .map_err(|error| format!("bolt: set write timeout: {error}"))?;
    if !handshake(&mut stream)? {
        return Ok(());
    }
    let mut connection = Connection {
        stream,
        handler,
        token,
        authenticated: false,
        scope: None,
        pending: None,
        transaction_id: None,
        failed: false,
        address,
    };
    loop {
        // Stop accepting additional work once the owning service begins its
        // bounded drain. The current request, if any, has already been
        // written before the loop reaches this point.
        if connection.handler.shutdown_requested() {
            return Ok(());
        }
        let Ok(message) = read_message(&mut connection.stream) else {
            // connection closed / protocol error
            return Ok(());
        };
        let Ok(decoded) = value::decode(&message) else {
            connection.failed = true;
            write_message(
                &mut connection.stream,
                &failure("Neo.DatabaseError.General.UnknownError", "protocol error"),
            )?;
            continue;
        };
        let response = match connection.handle_decoded(&decoded) {
            Ok(response) => response,
            Err(_) => MessageResponse::Buffered(vec![failure(
                "Neo.DatabaseError.General.UnknownError",
                "protocol error",
            )]),
        };
        if connection.authenticated {
            connection
                .stream
                .set_read_timeout(Some(connection_io_timeout))
                .map_err(|error| format!("bolt: set read timeout: {error}"))?;
        }
        match response {
            MessageResponse::StreamPull(fields) => connection.stream_pull(&fields)?,
            MessageResponse::Buffered(responses) => {
                if responses.is_empty() {
                    // GOODBYE.
                    return Ok(());
                }
                for response in responses {
                    connection.failed |= is_failure(&response);
                    write_message(&mut connection.stream, &response)?;
                }
            }
        }
    }
}

/// Bolt handshake: verify magic, negotiate v5, and respond.
fn handshake(stream: &mut TcpStream) -> Result<bool, String> {
    let mut header = [0u8; 20];
    stream
        .read_exact(&mut header)
        .map_err(|e| format!("bolt: handshake read: {e}"))?;
    if header[0..4] != MAGIC {
        return Err("bolt: invalid handshake magic".into());
    }
    let mut accepted = false;
    for proposal in header[4..].as_chunks::<4>().0 {
        // Bolt 5.x proposals encode major version 5 in the final byte.
        if proposal[3] == 5 {
            accepted = true;
            break;
        }
    }
    if accepted {
        stream.write_all(&VERSION_5_0).map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        stream.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;
        Ok(false)
    }
}

/// Default language enablement for the Bolt transport: cypher must be enabled.
#[must_use]
pub fn requires_languages() -> LanguageSet {
    LanguageSet {
        cypher: true,
        ..Default::default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;

    #[test]
    fn mutation_classifier_ignores_strings_and_comments() {
        assert!(looks_like_mutation("CREATE (n {name: 'read'})"));
        assert!(!looks_like_mutation(
            "MATCH (n) WHERE n.name = 'CREATE' RETURN n"
        ));
        assert!(!looks_like_mutation(
            "// CREATE is documentation\nMATCH (n) RETURN n"
        ));
        assert!(looks_like_mutation("MATCH (n) SET n.seen = true RETURN n"));
    }

    #[test]
    fn discard_obeys_bounded_window_and_reports_remaining_rows() {
        let mut pending = PendingResult {
            fields: vec!["x".into()],
            rows: (0..3).map(|_| Map::new()).collect(),
        };
        assert!(discard_pending(&mut pending, 2));
        assert_eq!(pending.rows.len(), 1);
        assert!(!discard_pending(&mut pending, 10));
        assert!(pending.rows.is_empty());
    }

    #[test]
    fn pull_window_accepts_bolt_unbounded_window_and_rejects_other_negatives() {
        let malformed = Value::String("ten".into());
        let unbounded = Value::Number((-1).into());
        let negative = Value::Number((-2).into());
        assert!(pull_window(&[]).is_ok());
        assert!(pull_window(&[Value::String("metadata".into())]).is_err());
        assert!(pull_window(&[Value::Object(Map::new()), Value::Object(Map::new())]).is_err());
        assert!(pull_window(&[Value::Object(
            [("n".into(), malformed)].into_iter().collect(),
        )])
        .is_err());
        assert_eq!(
            pull_window(&[Value::Object(
                [("n".into(), unbounded)].into_iter().collect(),
            )])
            .unwrap(),
            usize::MAX
        );
        assert!(pull_window(&[Value::Object(
            [("n".into(), negative)].into_iter().collect(),
        )])
        .is_err());
        assert_eq!(
            pull_window(&[Value::Object(
                [("n".into(), Value::Number(50_000.into()))]
                    .into_iter()
                    .collect(),
            )])
            .unwrap(),
            50_000
        );
    }

    #[test]
    fn discard_accepts_finite_windows_larger_than_internal_pull_batch() {
        let mut pending = PendingResult {
            fields: vec!["x".into()],
            rows: (0..10_001).map(|_| Map::new()).collect(),
        };
        let window = pull_window(&[Value::Object(
            [("n".into(), Value::Number(10_001.into()))]
                .into_iter()
                .collect(),
        )])
        .expect("finite Bolt window");
        assert!(!discard_pending(&mut pending, window));
        assert!(pending.rows.is_empty());
    }

    #[test]
    fn discard_unbounded_window_consumes_more_than_internal_pull_batch() {
        let mut pending = PendingResult {
            fields: vec!["x".into()],
            rows: (0..10_001).map(|_| Map::new()).collect(),
        };
        assert!(!discard_pending(&mut pending, usize::MAX));
        assert!(pending.rows.is_empty());
    }

    #[test]
    fn pull_streams_finite_window_above_batch_without_materializing_responses() {
        pull_stream_boundary(10_001, 10_002, true);
        pull_stream_boundary(usize::MAX, 10_001, false);
    }

    fn pull_stream_boundary(window: usize, total_rows: usize, expect_remaining: bool) {
        struct NoopHandler;
        impl EnvelopeHandler for NoopHandler {
            fn execute(
                &self,
                _envelope: RequestEnvelope,
                _scope: Option<String>,
            ) -> Result<rushdb_protocol::ResponseEnvelope, ProtocolError> {
                Err(ProtocolError::new(ErrorCode::ExecutionFailed, "unused"))
            }
        }

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let address = listener.local_addr().expect("local address");
        let client = TcpStream::connect(address).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        let mut connection = Connection {
            stream: server,
            handler: Arc::new(NoopHandler),
            token: String::new(),
            authenticated: true,
            scope: None,
            pending: Some(PendingResult {
                fields: vec!["x".into()],
                rows: (0..total_rows)
                    .map(|value| {
                        [("x".into(), Value::Number(value.into()))]
                            .into_iter()
                            .collect()
                    })
                    .collect(),
            }),
            transaction_id: None,
            failed: false,
            address: address.to_string(),
        };
        let raw_window = if window == usize::MAX {
            Value::Number((-1).into())
        } else {
            Value::Number(window.into())
        };
        let metadata = [("n".into(), raw_window)].into_iter().collect();
        connection
            .stream_pull(&[Value::Object(metadata)])
            .expect("stream pull");

        let mut client = client;
        for expected in 0..total_rows.min(window) {
            let message = read_message(&mut client).expect("record");
            let decoded = value::decode(&message).expect("decode record");
            assert_eq!(value::struct_tag(&decoded), Some(RECORD));
            let fields = value::struct_fields(&decoded);
            assert_eq!(
                fields[0].as_array().and_then(|values| values.first()),
                Some(&Value::Number(expected.into()))
            );
        }
        let success_message = read_message(&mut client).expect("success");
        let success = value::decode(&success_message).expect("decode success");
        assert_eq!(value::struct_tag(&success), Some(SUCCESS));
        let success_fields = value::struct_fields(&success);
        let metadata = success_fields[0].as_object().expect("metadata");
        assert_eq!(metadata["has_more"], Value::Bool(expect_remaining));
        assert_eq!(
            connection.pending.as_ref().map(|p| p.rows.len()),
            expect_remaining.then_some(total_rows - window.min(total_rows))
        );
    }

    #[test]
    fn bolt_error_taxonomy_is_complete_and_stable() {
        let cases: &[(ErrorCode, &str)] = &[
            (
                ErrorCode::UnsupportedVersion,
                "Neo.ClientError.Request.Invalid",
            ),
            (
                ErrorCode::ProtocolViolation,
                "Neo.ClientError.Statement.SyntaxError",
            ),
            (
                ErrorCode::InvalidRequest,
                "Neo.ClientError.Statement.SyntaxError",
            ),
            (
                ErrorCode::Unauthorized,
                "Neo.ClientError.Security.Unauthorized",
            ),
            (
                ErrorCode::MissingScope,
                "Neo.ClientError.Security.Unauthorized",
            ),
            (
                ErrorCode::ScopeMismatch,
                "Neo.ClientError.Security.Unauthorized",
            ),
            (
                ErrorCode::NotFound,
                "Neo.ClientError.Statement.EntityNotFound",
            ),
            (
                ErrorCode::Conflict,
                "Neo.ClientError.Constraint.UniqueConstraintViolation",
            ),
            (
                ErrorCode::WriteConflict,
                "Neo.TransientError.Transaction.DeadlockDetected",
            ),
            (
                ErrorCode::Cancelled,
                "Neo.ClientError.Transaction.TransactionTerminated",
            ),
            (
                ErrorCode::ResourceExhausted,
                "Neo.TransientError.General.DatabaseUnavailable",
            ),
            (
                ErrorCode::Unavailable,
                "Neo.TransientError.General.DatabaseUnavailable",
            ),
            (
                ErrorCode::ExecutionFailed,
                "Neo.DatabaseError.Execution.Failed",
            ),
            (
                ErrorCode::LanguageDisabled,
                "Neo.ClientError.Statement.SyntaxError",
            ),
            (
                ErrorCode::Unsupported,
                "Neo.ClientError.Statement.SyntaxError",
            ),
        ];
        for (code, expected) in cases {
            let err = ProtocolError::new(*code, "x");
            assert_eq!(neo_code(&err), *expected, "{code:?}");
        }
    }

    #[test]
    fn unauthenticated_idle_connection_is_closed_by_deadline() {
        struct NoopHandler;
        impl EnvelopeHandler for NoopHandler {
            fn execute(
                &self,
                _envelope: RequestEnvelope,
                _scope: Option<String>,
            ) -> Result<rushdb_protocol::ResponseEnvelope, ProtocolError> {
                Err(ProtocolError::new(ErrorCode::ExecutionFailed, "unused"))
            }
        }

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let address = listener.local_addr().expect("local address");
        let client = TcpStream::connect(address).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        let task = std::thread::spawn(move || {
            handle_connection_with_timeouts(
                server,
                Arc::new(NoopHandler),
                "token".into(),
                address.to_string(),
                Duration::from_millis(20),
                Duration::from_millis(20),
            )
            .expect("idle timeout closes connection cleanly");
        });

        let mut client = client;
        let mut handshake = Vec::from(MAGIC);
        handshake.extend_from_slice(&[0, 0, 0, 5]);
        handshake.extend_from_slice(&[0, 0, 0, 4]);
        handshake.extend_from_slice(&[0, 0, 0, 3]);
        handshake.extend_from_slice(&[0, 0, 0, 2]);
        client.write_all(&handshake).expect("handshake");
        let mut version = [0; 4];
        client.read_exact(&mut version).expect("version");
        assert_eq!(version, VERSION_5_0);
        task.join().expect("server thread");
    }

    #[test]
    fn authenticated_idle_connection_uses_the_normal_io_deadline() {
        struct NoopHandler;
        impl EnvelopeHandler for NoopHandler {
            fn execute(
                &self,
                _envelope: RequestEnvelope,
                _scope: Option<String>,
            ) -> Result<rushdb_protocol::ResponseEnvelope, ProtocolError> {
                Err(ProtocolError::new(ErrorCode::ExecutionFailed, "unused"))
            }
        }

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let address = listener.local_addr().expect("local address");
        let client = TcpStream::connect(address).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        let task = std::thread::spawn(move || {
            handle_connection_with_timeouts(
                server,
                Arc::new(NoopHandler),
                "token".into(),
                address.to_string(),
                Duration::from_secs(1),
                Duration::from_millis(20),
            )
            .expect("idle timeout closes connection cleanly");
        });

        let mut client = client;
        let mut handshake = Vec::from(MAGIC);
        handshake.extend_from_slice(&[0, 0, 0, 5]);
        handshake.extend_from_slice(&[0, 0, 0, 4]);
        handshake.extend_from_slice(&[0, 0, 0, 3]);
        handshake.extend_from_slice(&[0, 0, 0, 2]);
        client.write_all(&handshake).expect("handshake");
        let mut version = [0; 4];
        client.read_exact(&mut version).expect("version");

        let mut hello = Map::new();
        hello.insert("scheme".into(), Value::String("basic".into()));
        hello.insert("credentials".into(), Value::String("token".into()));
        let mut payload = Vec::new();
        value::encode_struct(HELLO, vec![Value::Object(hello)], &mut payload);
        write_message(&mut client, &payload).expect("hello");
        let response = read_message(&mut client).expect("hello response");
        assert_eq!(
            value::struct_tag(&value::decode(&response).expect("decode")),
            Some(SUCCESS)
        );
        task.join().expect("server thread");
    }

    #[test]
    fn active_session_closes_before_accepting_more_work_during_shutdown() {
        struct ShuttingDownHandler;
        impl EnvelopeHandler for ShuttingDownHandler {
            fn execute(
                &self,
                _envelope: RequestEnvelope,
                _scope: Option<String>,
            ) -> Result<rushdb_protocol::ResponseEnvelope, ProtocolError> {
                Err(ProtocolError::new(ErrorCode::ExecutionFailed, "unused"))
            }

            fn shutdown_requested(&self) -> bool {
                true
            }
        }

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let address = listener.local_addr().expect("local address");
        let client = TcpStream::connect(address).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        let task = std::thread::spawn(move || {
            handle_connection_with_timeouts(
                server,
                Arc::new(ShuttingDownHandler),
                "token".into(),
                address.to_string(),
                Duration::from_secs(1),
                Duration::from_millis(20),
            )
            .expect("shutdown closes active session cleanly");
        });

        let mut client = client;
        let mut handshake = Vec::from(MAGIC);
        handshake.extend_from_slice(&[0, 0, 0, 5]);
        handshake.extend_from_slice(&[0, 0, 0, 4]);
        handshake.extend_from_slice(&[0, 0, 0, 3]);
        handshake.extend_from_slice(&[0, 0, 0, 2]);
        client.write_all(&handshake).expect("handshake");
        let mut version = [0; 4];
        client.read_exact(&mut version).expect("version");
        assert_eq!(version, VERSION_5_0);
        let mut buf = [0u8; 1];
        assert_eq!(client.read(&mut buf).expect("read close"), 0);
        task.join().expect("server thread");
    }

    #[test]
    fn listener_shutdown_closes_idle_connections_and_joins_workers() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Handler(Arc<AtomicBool>);
        impl EnvelopeHandler for Handler {
            fn execute(
                &self,
                _: RequestEnvelope,
                _: Option<String>,
            ) -> Result<rushdb_protocol::ResponseEnvelope, ProtocolError> {
                Err(ProtocolError::new(ErrorCode::ExecutionFailed, "unused"))
            }
            fn shutdown_requested(&self) -> bool {
                self.0.load(Ordering::Acquire)
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (mut server, port) = BoltServer::bind(
            Arc::new(Handler(Arc::clone(&stop))),
            "token".into(),
            "127.0.0.1",
            0,
        )
        .unwrap();
        let (done, finished) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            done.send(server.serve_forever()).unwrap();
        });
        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut handshake = Vec::from(MAGIC);
        handshake.extend_from_slice(&[0, 0, 0, 5]);
        handshake.extend_from_slice(&[0; 12]);
        client.write_all(&handshake).unwrap();
        let mut version = [0; 4];
        client.read_exact(&mut version).unwrap();
        // Receiving the handshake proves the worker owns this socket before
        // the stop signal; the client stays idle and never closes voluntarily.
        stop.store(true, Ordering::Release);
        finished
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        worker.join().unwrap();
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
        assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
    }

    #[test]
    fn response_writer_times_out_when_peer_stops_reading() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let address = listener.local_addr().expect("local address");
        let _client = TcpStream::connect(address).expect("connect");
        let (mut server, _) = listener.accept().expect("accept");
        server
            .set_write_timeout(Some(Duration::from_millis(20)))
            .expect("write timeout");

        // The client is intentionally kept unread. Once the kernel send
        // buffer fills, the framing writer must return the socket timeout
        // instead of retaining the connection forever during shutdown.
        let payload = vec![0u8; MAX_MESSAGE_BYTES];
        let error = write_message(&mut server, &payload).expect_err("slow reader timeout");
        assert!(
            error.contains("timed out")
                || error.contains("would block")
                || error.contains("temporarily unavailable"),
            "unexpected slow-reader error: {error}"
        );
    }
}
