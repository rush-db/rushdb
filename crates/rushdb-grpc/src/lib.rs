//! gRPC transport for the canonical rush-rushdb protocol.
//!
//! A single unary `Execute` RPC carries the canonical `RequestEnvelope` /
//! `ResponseEnvelope` as a JSON payload inside a `bytes` protobuf field. Every
//! operation (query/mutate/describe/index/ingest/transaction/purge/backup) goes
//! through the same protocol surface as HTTP — no new execution logic.
//!
//! Auth: `authorization: Bearer <token>` metadata (the boot `--token` secret).
//! Tenant scope: `x-rushdb-tenant` metadata (multitenant mode).
//!
//! The crate is transport-only: it knows the canonical envelope types and a
//! handler trait, never the engine internals (no dependency on the server
//! crate, so the server can bind it without a dependency cycle).

use std::{
    convert::Infallible,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use tonic::{codec::ProstCodec, server::Grpc, Request, Response, Status};

/// Upper bound on concurrently executing requests (and HTTP/2 streams) per
/// gRPC connection (C7, cloud-capacity F8).
const MAX_CONCURRENT_REQUESTS_PER_CONNECTION: usize = 64;
/// HTTP/2 stream cap per gRPC connection; equal to the request cap above.
const MAX_CONCURRENT_STREAMS: u32 = 64;

pub use rushdb_protocol::EnvelopeHandler;
use rushdb_protocol::RequestEnvelope;

/// Protobuf wire envelope: field 1 = the canonical JSON envelope bytes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BytesEnvelope {
    /// Canonical JSON-encoded request or response envelope.
    pub json: Vec<u8>,
}

impl prost::Message for BytesEnvelope {
    fn encode_raw(&self, buf: &mut impl bytes::BufMut) {
        prost::encoding::bytes::encode(1, &self.json, buf);
    }

    fn merge_field(
        &mut self,
        tag: u32,
        wire_type: prost::encoding::WireType,
        buf: &mut impl bytes::Buf,
        ctx: prost::encoding::DecodeContext,
    ) -> Result<(), prost::DecodeError> {
        if tag == 1 {
            prost::encoding::bytes::merge(wire_type, &mut self.json, buf, ctx)
        } else {
            prost::encoding::skip_field(wire_type, tag, buf, ctx)
        }
    }

    fn encoded_len(&self) -> usize {
        prost::encoding::bytes::encoded_len(1, &self.json)
    }

    fn clear(&mut self) {
        self.json.clear();
    }
}

/// The gRPC `Database.Execute` unary handler.
#[derive(Clone)]
pub struct DatabaseService {
    handler: Arc<dyn EnvelopeHandler>,
    token: String,
}

impl DatabaseService {
    /// Creates a service that authenticates `token` and dispatches request
    /// envelopes to `handler`.
    #[must_use]
    pub fn new(handler: Arc<dyn EnvelopeHandler>, token: String) -> Self {
        DatabaseService { handler, token }
    }

    fn execute(&self, request: Request<BytesEnvelope>) -> Result<Response<BytesEnvelope>, Status> {
        let metadata = request.metadata();
        let token = metadata
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| Status::unauthenticated("missing bearer token"))?;
        if !rushdb_protocol::constant_time_eq(token.as_bytes(), self.token.as_bytes()) {
            return Err(Status::unauthenticated("invalid token"));
        }
        let scope = metadata
            .get("x-rushdb-tenant")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);

        let payload = request.into_inner().json;
        let envelope: RequestEnvelope = serde_json::from_slice(&payload)
            .map_err(|e| Status::invalid_argument(format!("invalid envelope: {e}")))?;

        let result = self.handler.execute(envelope, scope);
        let response = result.map_err(|error| grpc_status(&error))?;
        let encoded = serde_json::to_vec(&response)
            .map_err(|e| Status::internal(format!("serialize response: {e}")))?;
        Ok(Response::new(BytesEnvelope { json: encoded }))
    }
}

/// tower service adapter: `Grpc::unary` accepts any
/// `Service<Request<BytesEnvelope>>` (blanket `UnaryService` impl).
impl tower::Service<Request<BytesEnvelope>> for DatabaseService {
    type Response = Response<BytesEnvelope>;
    type Error = Status;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<BytesEnvelope>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || this.execute(request))
                .await
                .map_err(|error| Status::internal(format!("gRPC execution task failed: {error}")))?
        })
    }
}

/// The gRPC routing service bound by `tonic::transport::Server`.
pub struct DatabaseGrpc<S> {
    inner: S,
    max_message_size: usize,
}

impl<S> Clone for DatabaseGrpc<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        DatabaseGrpc {
            inner: self.inner.clone(),
            max_message_size: self.max_message_size,
        }
    }
}

impl<S> tonic::server::NamedService for DatabaseGrpc<S> {
    const NAME: &'static str = "rushdb.v1.Database";
}

impl<S> tower::Service<http::Request<tonic::body::BoxBody>> for DatabaseGrpc<S>
where
    S: tonic::server::UnaryService<BytesEnvelope, Response = BytesEnvelope>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = http::Response<tonic::body::BoxBody>;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<tonic::body::BoxBody>) -> Self::Future {
        let path = request.uri().path().to_string();
        let inner = self.inner.clone();
        let max_message_size = self.max_message_size;
        Box::pin(async move {
            if path == "/rushdb.v1.Database/Execute" {
                // Match the HTTP transport's body budget (configurable,
                // default 64MB): tonic's default per-message limit is 4MB,
                // which would reject large batch imports that are fine over HTTP.
                let mut grpc = Grpc::new(ProstCodec::default())
                    .max_decoding_message_size(max_message_size)
                    .max_encoding_message_size(max_message_size);
                Ok(grpc.unary(inner, request).await)
            } else {
                let mut not_found = http::Response::new(tonic::body::boxed(
                    http_body_util::Empty::<bytes::Bytes>::new(),
                ));
                *not_found.status_mut() = http::StatusCode::NOT_FOUND;
                Ok(not_found)
            }
        })
    }
}

/// gRPC server mirroring the pgwire adapter: a non-binding disabled variant and
/// a pre-bound listener served from a dedicated runtime thread.
pub struct GrpcServer {
    handler: Option<Arc<dyn EnvelopeHandler>>,
    token: Option<String>,
    listener: Option<std::net::TcpListener>,
    max_message_size: usize,
}

impl GrpcServer {
    /// A server that does not bind (no grpc-port configured).
    #[must_use]
    pub fn disabled() -> Self {
        GrpcServer {
            handler: None,
            token: None,
            listener: None,
            max_message_size: 64 * 1024 * 1024,
        }
    }

    /// Pre-binds the listener and returns the server plus the bound port.
    /// `max_message_size` matches the HTTP transport's body budget so large
    /// batch imports work over gRPC too (tonic's default per-message limit is
    /// 4MB).
    ///
    /// # Errors
    ///
    /// Returns an error string if the listen address cannot be bound, its
    /// local address cannot be read, or the listener cannot be set
    /// non-blocking.
    pub fn bind(
        handler: Arc<dyn EnvelopeHandler>,
        token: String,
        listen_addr: &str,
        port: u16,
        max_message_size: usize,
    ) -> Result<(Self, u16), String> {
        let l = std::net::TcpListener::bind((listen_addr, port)).map_err(|e| e.to_string())?;
        let p = l.local_addr().map_err(|e| e.to_string())?.port();
        l.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok((
            GrpcServer {
                handler: Some(handler),
                token: Some(token),
                listener: Some(l),
                max_message_size,
            },
            p,
        ))
    }

    /// Serves the pre-bound listener until shutdown on the caller-owned
    /// transport thread.
    ///
    /// # Panics
    ///
    /// Panics if a listener is present without a handler or token (only
    /// possible when not constructed via [`GrpcServer::bind`]), or if the
    /// tokio runtime, listener conversion, or server fails to start.
    /// # Errors
    /// Returns the listener failure or a bounded-drain timeout.
    pub fn serve_forever(&mut self) -> Result<(), String> {
        let Some(l) = self.listener.take() else {
            // grpc-port not configured: nothing to serve.
            return Ok(());
        };
        let handler = self.handler.take().unwrap();
        let token = self.token.take().unwrap();
        let max_message_size = self.max_message_size;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("grpc runtime");
        let outcome = rt.block_on(async move {
            let service = DatabaseService::new(handler, token);
            let shutdown_handler = Arc::clone(&service.handler);
            let handler_for_deadline = Arc::clone(&service.handler);
            let router = DatabaseGrpc {
                inner: service,
                max_message_size,
            };
            let stream = tokio_stream::wrappers::TcpListenerStream::new(
                tokio::net::TcpListener::from_std(l).expect("grpc listener"),
            );
            // The per-message budget is enforced by the codec above
            // (`max_decoding/encoding_message_size`); HTTP/2 frame size
            // stays at the default (tonic enforces h2's 16MB ceiling).
            // C7: bound concurrent requests per connection and HTTP/2 streams
            // so one client cannot fan out unbounded in-flight work.
            let server = tonic::transport::Server::builder()
                .concurrency_limit_per_connection(MAX_CONCURRENT_REQUESTS_PER_CONNECTION)
                .max_concurrent_streams(MAX_CONCURRENT_STREAMS)
                .add_service(router)
                .serve_with_incoming_shutdown(stream, async move {
                    while !shutdown_handler.shutdown_requested() {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                });
            tokio::pin!(server);
            tokio::select! {
                result = &mut server => result.map_err(|error| error.to_string()),
                () = async {
                    while !handler_for_deadline.shutdown_requested() {
                        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                } => Err("gRPC shutdown drain deadline elapsed".into()),
            }
        });
        rt.shutdown_timeout(std::time::Duration::from_secs(1));
        outcome
    }
}
/// Shared gRPC error taxonomy: the same `ErrorCode` maps to the same gRPC status
/// on every adapter. Add below — do not reorder.
fn grpc_status(error: &rushdb_protocol::ProtocolError) -> tonic::Status {
    let message = error.message.clone();
    match error.code {
        rushdb_protocol::ErrorCode::UnsupportedVersion
        | rushdb_protocol::ErrorCode::ProtocolViolation
        | rushdb_protocol::ErrorCode::InvalidRequest
        | rushdb_protocol::ErrorCode::ExecutionFailed
        | rushdb_protocol::ErrorCode::LanguageDisabled
        | rushdb_protocol::ErrorCode::Unsupported => tonic::Status::invalid_argument(message),
        rushdb_protocol::ErrorCode::Unauthorized => tonic::Status::unauthenticated(message),
        rushdb_protocol::ErrorCode::MissingScope | rushdb_protocol::ErrorCode::ScopeMismatch => {
            tonic::Status::unauthenticated(message)
        }
        rushdb_protocol::ErrorCode::NotFound => tonic::Status::not_found(message),
        rushdb_protocol::ErrorCode::Conflict => tonic::Status::already_exists(message),
        rushdb_protocol::ErrorCode::WriteConflict => tonic::Status::aborted(message),
        rushdb_protocol::ErrorCode::Cancelled => tonic::Status::cancelled(message),
        rushdb_protocol::ErrorCode::ResourceExhausted => tonic::Status::resource_exhausted(message),
        rushdb_protocol::ErrorCode::Unavailable => tonic::Status::unavailable(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_bounds_an_active_uncooperative_grpc_handler() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Mutex,
        };
        use std::time::Duration;

        struct Handler {
            stopping: AtomicBool,
            entered: mpsc::Sender<()>,
            release: Mutex<mpsc::Receiver<()>>,
        }
        impl EnvelopeHandler for Handler {
            fn shutdown_requested(&self) -> bool {
                self.stopping.load(Ordering::Acquire)
            }
            fn execute(
                &self,
                _: RequestEnvelope,
                _: Option<String>,
            ) -> Result<rushdb_protocol::ResponseEnvelope, rushdb_protocol::ProtocolError>
            {
                self.entered.send(()).unwrap();
                let _ = self
                    .release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(15));
                Err(rushdb_protocol::ProtocolError::new(
                    rushdb_protocol::ErrorCode::Cancelled,
                    "released",
                ))
            }
        }
        let (entered, started) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let handler = Arc::new(Handler {
            stopping: AtomicBool::new(false),
            entered,
            release: Mutex::new(released),
        });
        let (mut server, port) =
            GrpcServer::bind(handler.clone(), "token".into(), "127.0.0.1", 0, 1024 * 1024).unwrap();
        let (finished, outcome) = mpsc::channel();
        let worker = std::thread::spawn(move || finished.send(server.serve_forever()).unwrap());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let client = std::thread::spawn(move || {
            runtime.block_on(async move {
                let channel =
                    tonic::transport::Endpoint::from_shared(format!("http://127.0.0.1:{port}"))
                        .unwrap()
                        .connect()
                        .await
                        .unwrap();
                let mut grpc = tonic::client::Grpc::new(channel);
                grpc.ready().await.unwrap();
                let mut request = Request::new(BytesEnvelope {
                    json: serde_json::to_vec(&RequestEnvelope {
                        protocol_version: rushdb_protocol::PROTOCOL_VERSION,
                        request_id: None,
                        context: rushdb_protocol::RequestContext::default(),
                        operation: rushdb_protocol::Operation::Describe {
                            describe: rushdb_protocol::Describe::BuildInfo,
                        },
                    })
                    .unwrap(),
                });
                request
                    .metadata_mut()
                    .insert("authorization", "Bearer token".parse().unwrap());
                let response = grpc.unary(
                    request,
                    http::uri::PathAndQuery::from_static("/rushdb.v1.Database/Execute"),
                    ProstCodec::<BytesEnvelope, BytesEnvelope>::default(),
                );
                tokio::time::timeout(Duration::from_secs(10), response).await
            })
        });
        started.recv_timeout(Duration::from_secs(3)).unwrap();
        handler.stopping.store(true, Ordering::Release);
        let result = outcome.recv_timeout(Duration::from_secs(8));
        // Release the deliberately uncooperative callback even if the timeout
        // assertion fails; Rust cannot forcibly cancel a blocking provider.
        release.send(()).unwrap();
        assert_eq!(
            result.unwrap().unwrap_err(),
            "gRPC shutdown drain deadline elapsed"
        );
        worker.join().unwrap();
        assert!(client.join().unwrap().unwrap().is_err());
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    }

    #[test]
    fn blocking_handler_does_not_stall_a_current_thread_runtime() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Mutex,
        };
        use std::time::Duration;
        struct BlockingHandler {
            release: Mutex<mpsc::Receiver<()>>,
            timed_out: Arc<AtomicBool>,
        }
        impl EnvelopeHandler for BlockingHandler {
            fn execute(
                &self,
                _: RequestEnvelope,
                _: Option<String>,
            ) -> Result<rushdb_protocol::ResponseEnvelope, rushdb_protocol::ProtocolError>
            {
                if self
                    .release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .is_err()
                {
                    self.timed_out.store(true, Ordering::Release);
                }
                Err(rushdb_protocol::ProtocolError::new(
                    rushdb_protocol::ErrorCode::Cancelled,
                    "finished",
                ))
            }
        }
        let (release, receiver) = mpsc::channel();
        let timed_out = Arc::new(AtomicBool::new(false));
        let mut service = DatabaseService::new(
            Arc::new(BlockingHandler {
                release: Mutex::new(receiver),
                timed_out: Arc::clone(&timed_out),
            }),
            "token".into(),
        );
        let envelope = RequestEnvelope {
            protocol_version: rushdb_protocol::PROTOCOL_VERSION,
            request_id: None,
            context: rushdb_protocol::RequestContext::default(),
            operation: rushdb_protocol::Operation::Describe {
                describe: rushdb_protocol::Describe::BuildInfo,
            },
        };
        let mut request = Request::new(BytesEnvelope {
            json: serde_json::to_vec(&envelope).unwrap(),
        });
        request
            .metadata_mut()
            .insert("authorization", "Bearer token".parse().unwrap());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let future = tower::Service::call(&mut service, request);
            let release_task = tokio::spawn(async move {
                tokio::task::yield_now().await;
                release.send(()).unwrap();
            });
            assert!(future.await.is_err());
            release_task.await.unwrap();
        });
        assert!(
            !timed_out.load(Ordering::Acquire),
            "the handler blocked its async runtime"
        );
    }

    #[test]
    fn grpc_error_taxonomy_is_complete_and_stable() {
        use tonic::Code;
        let cases: &[(rushdb_protocol::ErrorCode, Code)] = &[
            (
                rushdb_protocol::ErrorCode::UnsupportedVersion,
                Code::InvalidArgument,
            ),
            (
                rushdb_protocol::ErrorCode::ProtocolViolation,
                Code::InvalidArgument,
            ),
            (
                rushdb_protocol::ErrorCode::InvalidRequest,
                Code::InvalidArgument,
            ),
            (
                rushdb_protocol::ErrorCode::Unauthorized,
                Code::Unauthenticated,
            ),
            (
                rushdb_protocol::ErrorCode::MissingScope,
                Code::Unauthenticated,
            ),
            (
                rushdb_protocol::ErrorCode::ScopeMismatch,
                Code::Unauthenticated,
            ),
            (rushdb_protocol::ErrorCode::NotFound, Code::NotFound),
            (rushdb_protocol::ErrorCode::Conflict, Code::AlreadyExists),
            (rushdb_protocol::ErrorCode::WriteConflict, Code::Aborted),
            (rushdb_protocol::ErrorCode::Cancelled, Code::Cancelled),
            (
                rushdb_protocol::ErrorCode::ResourceExhausted,
                Code::ResourceExhausted,
            ),
            (rushdb_protocol::ErrorCode::Unavailable, Code::Unavailable),
            (
                rushdb_protocol::ErrorCode::ExecutionFailed,
                Code::InvalidArgument,
            ),
            (
                rushdb_protocol::ErrorCode::LanguageDisabled,
                Code::InvalidArgument,
            ),
            (
                rushdb_protocol::ErrorCode::Unsupported,
                Code::InvalidArgument,
            ),
        ];
        for (code, grpc_code) in cases {
            let status = grpc_status(&rushdb_protocol::ProtocolError::new(*code, "x"));
            assert_eq!(status.code(), *grpc_code, "{code:?}");
        }
    }
}
