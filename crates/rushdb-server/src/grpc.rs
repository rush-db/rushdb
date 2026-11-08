//! gRPC transport adapter (server side).
//!
//! Implements `rushdb_grpc::EnvelopeHandler` on top of `ProtocolExecutor`,
//! applying the tenant scope thread-local exactly like the HTTP adapter, and
//! binds a `GrpcServer` when `grpc-port` is configured (mirroring the pgwire
//! adapter's disabled/bind/thread pattern).

use std::sync::Arc;

use rushdb_grpc::{EnvelopeHandler, GrpcServer};
use rushdb_protocol::{Principal, ProtocolError, RequestEnvelope, ResponseEnvelope};

use crate::protocol::scheduler_error_to_protocol_error;
use crate::{Node, ServerConfig};

/// Executes canonical envelopes on a node under the request tenant scope,
/// and authenticates the transports' credentials with the server's one rule.
#[derive(Clone)]
pub struct NodeEnvelopeHandler {
    node: Arc<Node>,
    credentials: crate::auth::Credentials,
}

impl NodeEnvelopeHandler {
    /// Wrap a booted node as an envelope handler for the gRPC/Bolt transports,
    /// checking credentials against `cfg` (boot token, capability secret,
    /// tenancy mode).
    pub fn new(node: Arc<Node>, cfg: &ServerConfig) -> Self {
        NodeEnvelopeHandler {
            node,
            credentials: crate::auth::Credentials::from_config(cfg),
        }
    }
}

/// A certificate chain and a private key, as PEM bytes.
pub(crate) type PemPair = (Vec<u8>, Vec<u8>);

/// The PEM files behind `--tls-cert`/`--tls-key`, read for a transport that
/// takes them as bytes; `None` when TLS is not configured.
pub(crate) fn tls_pem(cfg: &ServerConfig, transport: &str) -> Result<Option<PemPair>, String> {
    let (Some(cert), Some(key)) = (&cfg.tls_cert, &cfg.tls_key) else {
        return Ok(None);
    };
    let cert_pem = std::fs::read(cert).map_err(|error| {
        format!(
            "{transport} TLS: cannot read certificate {}: {error}",
            cert.display()
        )
    })?;
    let key_pem = std::fs::read(key).map_err(|error| {
        format!(
            "{transport} TLS: cannot read private key {}: {error}",
            key.display()
        )
    })?;
    Ok(Some((cert_pem, key_pem)))
}

impl EnvelopeHandler for NodeEnvelopeHandler {
    fn authenticate(
        &self,
        credential: &str,
        scope: Option<&str>,
    ) -> Result<Principal, ProtocolError> {
        self.credentials.authenticate(credential, scope)
    }

    fn authenticate_session(
        &self,
        credential: &str,
    ) -> Result<(Principal, Option<String>), ProtocolError> {
        self.credentials.authenticate_session(credential)
    }

    fn execute(
        &self,
        envelope: RequestEnvelope,
        scope: Option<String>,
    ) -> Result<ResponseEnvelope, ProtocolError> {
        // A single-tenant node has one keyspace: a transport scope (gRPC
        // `x-rushdb-tenant`, Bolt HELLO `db`) is refused like the HTTP header,
        // never served as a separate keyspace.
        if scope.is_some() && !self.node.multitenant() {
            return Err(ProtocolError::new(
                rushdb_protocol::ErrorCode::ScopeMismatch,
                "tenant scope must be omitted in single-tenant mode",
            ));
        }
        let scope_for_job = scope.clone();
        let tenant = self.node.schedule_key(scope.as_deref());
        let started = std::time::Instant::now();
        let node = Arc::clone(&self.node);
        let result = self
            .node
            .schedule_for_tenant(tenant, move || {
                let result =
                    crate::protocol::ProtocolExecutor::new(&node, scope_for_job).execute(envelope);
                result
            })
            .map_err(scheduler_error_to_protocol_error)?;
        let seconds = started.elapsed().as_secs_f64();
        // gRPC and Bolt share this handler: observe latency + rows returned.
        self.node.observe("transport.execute", seconds);
        if let Ok(resp) = &result {
            let rows = match &resp.response {
                rushdb_protocol::Response::Records { records, .. } => records.len() as u64,
                rushdb_protocol::Response::Rows { rows, .. }
                | rushdb_protocol::Response::TextHits { rows, .. } => rows.len() as u64,
                _ => 0,
            };
            self.node.observe_rows("transport.execute", rows);
        }
        result
    }

    fn shutdown_requested(&self) -> bool {
        self.node.store().maintenance_aborted()
    }
}

/// Bind the gRPC transport for a booted node, if `grpc-port` is configured.
/// Returns the server and its bound port (0 when disabled).
///
/// # Errors
///
/// Returns the bind failure message when the gRPC listener cannot bind the
/// configured address.
pub fn bind_grpc(cfg: &ServerConfig, node: Arc<Node>) -> Result<(GrpcServer, u16), String> {
    if cfg.grpc_port == 0 {
        return Ok((GrpcServer::disabled(), 0));
    }
    crate::install_crypto_provider();
    let handler: Arc<dyn EnvelopeHandler> = Arc::new(NodeEnvelopeHandler::new(node, cfg));
    let tls =
        tls_pem(cfg, "gRPC")?.map(|(cert_pem, key_pem)| rushdb_grpc::GrpcTls { cert_pem, key_pem });
    GrpcServer::bind(
        handler,
        &cfg.listen_addr,
        cfg.grpc_port,
        cfg.max_request_bytes,
        tls,
    )
}
