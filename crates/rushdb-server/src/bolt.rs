//! Bolt (Neo4j wire) transport adapter (server side).
//!
//! Binds a `BoltServer` when `bolt-port` is configured AND `cypher` is
//! enabled — Bolt sends every statement as openCypher. Mirrors the gRPC
//! adapter: `disabled()` when not bound.

use std::sync::Arc;

use rushdb_bolt::BoltServer;
use rushdb_protocol::EnvelopeHandler;

use crate::{Node, ServerConfig};

/// Whether this configuration serves Bolt: a port and the `cypher` language.
pub(crate) fn bolt_enabled(cfg: &ServerConfig) -> bool {
    cfg.bolt_port != 0 && cfg.languages.cypher
}

/// Bind the Bolt transport for a booted node, if [`bolt_enabled`]. Returns the server and its bound port
/// (0 when disabled).
///
/// # Errors
///
/// Returns the bind failure message when the Bolt listener cannot bind the
/// configured address.
pub fn bind_bolt(cfg: &ServerConfig, node: Arc<Node>) -> Result<(BoltServer, u16), String> {
    if !bolt_enabled(cfg) {
        return Ok((BoltServer::disabled(), 0));
    }
    let handler: Arc<dyn EnvelopeHandler> = Arc::new(crate::NodeEnvelopeHandler::new(node));
    BoltServer::bind(handler, cfg.token.clone(), &cfg.listen_addr, cfg.bolt_port)
}
