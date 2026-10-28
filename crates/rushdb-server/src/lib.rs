//! Pgwire and HTTP surfaces for the `RushDB` engine (`rushdb serve`) plus the
//! one-command distribution entry (`rushdb up`) — deployment slice of plan
//! documents 04/10/25. The server hosts SQL over PostgreSQL wire, the
//! versioned HTTP database protocol, and operational endpoints.

use thiserror::Error;

pub mod allocator;
pub mod archive;
pub mod bolt;
pub mod boot;
mod bulk_share;
pub mod capability;
pub mod config;
pub mod grpc;
pub mod http;
pub mod lifecycle;
mod node;
pub mod pgwire;
pub mod protocol;
mod protocol_http;
pub mod replication;
pub mod resource_config;
pub mod retention;

pub use bolt::bind_bolt;
pub use grpc::{bind_grpc, NodeEnvelopeHandler};
pub use http::{new_router, HttpRouter};
pub use node::scope::RequestScope;
pub use node::Node;
pub use pgwire::{new_gateway, PgWireServer};
pub use rushdb_bolt::BoltServer;
pub use rushdb_grpc::GrpcServer;

/// Effective boot configuration for a `rushdb serve`/`rushdb up` process.
#[derive(Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent operator switches (multitenancy, maintenance, TLS listen, index-warmup readiness) are clearer as named fields than as a bitflag set"
)]
pub struct ServerConfig {
    /// HTTP protocol adapter port.
    pub port: u16,
    /// Bind address for the HTTP protocol adapter (default `127.0.0.1`).
    /// Set to `0.0.0.0` to expose the engine beyond loopback (pair with TLS).
    pub listen_addr: String,
    /// Path to a PEM certificate chain for HTTPS. Both `tls_cert` and
    /// `tls_key` must be set to enable TLS on the HTTP listener.
    pub tls_cert: Option<std::path::PathBuf>,
    /// Path to a PEM private key (PKCS#8) for HTTPS.
    pub tls_key: Option<std::path::PathBuf>,
    /// PostgreSQL wire-protocol port.
    pub pgwire_port: u16,
    /// gRPC transport port (`0` = disabled).
    pub grpc_port: u16,
    /// Bolt (Neo4j wire) transport port (`0` = disabled; requires a graph
    /// language).
    pub bolt_port: u16,
    /// Boot bearer token granting full access on every transport.
    pub token: String,
    /// Directory holding the durable store and `config.yml`; `None` is an
    /// in-memory node.
    pub data_dir: Option<std::path::PathBuf>,
    /// Enables scope-keyed multi-tenancy when set. Each request must present a
    /// scope (`X-RushDB-Tenant` on HTTP, `database` on pgwire); the tenant
    /// identity is the full SHA-256 of that scope, so the keyspace prefix is
    /// the isolation boundary and no discriminator property is ever stored on
    /// records. `false` is true single-tenant mode: no scope, one keyspace.
    pub multitenant: bool,
    /// Optional HMAC secret for signed capability tokens. When set, a valid
    /// capability presented via `X-RushDB-Token` is accepted (scope-gated);
    /// the boot `token` always remains a full-access fallback.
    pub capability_secret: Option<Vec<u8>>,
    /// Optional raft replication: when `Some`, the node boots an in-process
    /// `OpenRaft` cluster of `size` nodes and routes every local commit through
    /// the elected leader. The leader replicates to the quorum and each node's
    /// store applies the batch deterministically before the commit returns.
    /// Note: the transport adapter is in-process (engine tests / single-host
    /// HA preview); external TCP transports can be plugged into
    /// `RaftNetworkFactory` without changing the store contract.
    pub raft: Option<RaftConfig>,
    /// Storage resource budgets and maintenance policy.
    pub resources: resource_config::ResourceConfig,
    /// Transaction coordinator limits.
    pub transactions: resource_config::TransactionConfig,
    /// Startup query-language enablement. `searchquery` is always on; the
    /// statement languages (`sql`, `gql`, `cypher`, `gremlin`, `sql-pgq`,
    /// `mongodb`) are opt-in via `config.yml` / `--languages`.
    pub languages: rushdb_protocol::LanguageSet,
    /// Maximum protocol payload in bytes: HTTP request body limit, gRPC
    /// per-message limit, and the response output budget. Configurable via
    /// `config.yml` `server.body-limit` (default 16MiB).
    pub max_request_bytes: usize,
    /// Maintenance mode: `/health` reports `maintenance`, `/ready` returns 503,
    /// and write operations are rejected with `Unavailable` until cleared.
    pub maintenance: bool,
    /// Explicit acknowledgement that pgwire/gRPC/Bolt (cleartext transports)
    /// may bind a non-loopback `--listen` address without TLS. Loopback binds
    /// never require this.
    pub allow_insecure_listen: bool,
    /// When true, `/ready` reports 503 until every configured vector index has
    /// caught up with its source records (no unindexed or stale bindings).
    /// Default false: readiness reflects store writability only. Enabling it
    /// lets orchestration avoid routing reads to a node whose ANN indexes are
    /// still warming. Single-tenant only; multitenant readiness never gates on
    /// a cross-tenant scan.
    pub require_index_warmup: bool,
    /// Request execution deadline (ms). Queries that exceed it are cancelled
    /// with `Cancelled`. 0 = no deadline.
    pub request_timeout_ms: u64,
    /// Per-tenant concurrent request cap in multitenant mode (0 = unlimited).
    /// A tenant that exceeds the cap is refused with `Unavailable` instead of
    /// starving its neighbors through the shared pools.
    pub max_concurrent_requests_per_tenant: usize,
    /// Long names of the CLI flags (or their environment variables) the
    /// operator set explicitly for this boot. The data directory's
    /// `config.yml` is the single source of truth: a stored value that
    /// disagrees with an explicitly supplied one is a startup error naming
    /// both, never a silent override.
    pub explicit_flags: std::collections::BTreeSet<String>,
}

/// In-process raft replication settings.
#[derive(Debug, Clone, PartialEq)]
pub struct RaftConfig {
    /// Number of state machines in the cluster (including this node).
    pub size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 3000,
            listen_addr: "127.0.0.1".into(),
            tls_cert: None,
            tls_key: None,
            pgwire_port: 5432,
            grpc_port: 0,
            bolt_port: 0,
            token: DEFAULT_TOKEN.into(),
            data_dir: None,
            multitenant: false,
            capability_secret: None,
            raft: None,
            resources: resource_config::ResourceConfig::default(),
            transactions: resource_config::TransactionConfig::default(),
            languages: rushdb_protocol::LanguageSet::default(),
            max_request_bytes: 16 * 1024 * 1024,
            maintenance: false,
            allow_insecure_listen: false,
            require_index_warmup: false,
            request_timeout_ms: 0,
            max_concurrent_requests_per_tenant: 0,
            explicit_flags: std::collections::BTreeSet::new(),
        }
    }
}

/// The token a server uses when none is configured: for local development
/// only, so a non-loopback listener refuses to start with it.
pub const DEFAULT_TOKEN: &str = "local-token";

impl ServerConfig {
    /// Reject boot configurations that would silently fail open or downgrade
    /// transport security. Called before any storage or listener is opened.
    ///
    /// # Errors
    /// Returns a message when the bearer token is empty, the capability secret
    /// is present but shorter than 16 bytes, the TLS certificate/key pair is
    /// half-configured, a non-loopback listener would use the default token,
    /// or a non-loopback HTTP listener has neither TLS nor an explicit
    /// insecure-listen acknowledgement.
    pub fn validate_boot_security(&self) -> Result<(), String> {
        if self.token.is_empty() {
            return Err(
                "server token must not be empty: an empty token authenticates every request".into(),
            );
        }
        if let Some(secret) = &self.capability_secret {
            if secret.len() < 16 {
                return Err(format!(
                    "capability secret must be at least 16 bytes (got {}); a short HMAC key is guessable",
                    secret.len()
                ));
            }
        }
        match (&self.tls_cert, &self.tls_key) {
            (Some(_), None) | (None, Some(_)) => {
                return Err(
                    "TLS requires both a certificate and a private key; refusing a half-configured TLS pair"
                        .into(),
                );
            }
            _ => {}
        }
        if self.token == DEFAULT_TOKEN && !crate::pgwire::is_loopback(&self.listen_addr) {
            return Err(format!(
                "refusing to listen on non-loopback `{}` with the default token `{DEFAULT_TOKEN}`; \
                 set --token (RUSHDB_TOKEN)",
                self.listen_addr
            ));
        }
        let tls_enabled = self.tls_cert.is_some() && self.tls_key.is_some();
        if !crate::pgwire::is_loopback(&self.listen_addr)
            && !tls_enabled
            && !self.allow_insecure_listen
        {
            return Err(format!(
                "refusing to bind HTTP (cleartext) to non-loopback listen address `{}` without TLS or --allow-insecure-listen",
                self.listen_addr
            ));
        }
        Ok(())
    }
}

/// Error surfaced by the HTTP adapter when a protocol or engine call fails.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The protocol layer rejected the request with an HTTP status and cause.
    #[error("protocol error: {1}")]
    Protocol(u16, rushdb_protocol::ProtocolError),
    /// The storage engine failed outside the protocol's error taxonomy.
    #[error("engine failure: {0}")]
    Engine(String),
}
