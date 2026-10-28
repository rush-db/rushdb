//! JSON/HTTP transport adapter for the canonical database protocol.
//!
//! This module only decodes and encodes `rushdb-protocol` envelopes around
//! the transport-independent executor.

use crate::node::Node;
use crate::ApiError;

/// Execute a protocol request with deadline/disconnect cancellation: the query
/// executor observes `is_cancelled` so long-running work aborts with a
/// structured `Cancelled` error instead of running to completion.
pub(crate) fn execute_cancellable(
    node: &Node,
    scope: Option<String>,
    body: &[u8],
    is_cancelled: &dyn Fn() -> bool,
    deadline_ms: Option<u64>,
) -> Result<serde_json::Value, ApiError> {
    let mut request: rushdb_protocol::RequestEnvelope =
        serde_json::from_slice(body).map_err(|error| {
            ApiError::Protocol(
                400,
                rushdb_protocol::ProtocolError::new(
                    rushdb_protocol::ErrorCode::InvalidRequest,
                    format!("invalid protocol request: {error}"),
                ),
            )
        })?;
    // The server-owned transport deadline bounds every engine phase; a
    // client's own `deadlineMs` may only shorten it.
    request.context.deadline_ms = crate::protocol::effective_deadline_ms(
        request.context.deadline_ms,
        deadline_ms.unwrap_or(0),
    );
    let op_kind = crate::protocol::operation_kind(&request.operation);
    let response = crate::protocol::ProtocolExecutor::new(node, scope)
        .execute_cancellable(request, is_cancelled)
        .map_err(|error| {
            // Shared error taxonomy: the same ErrorCode maps to the same
            // HTTP status on every adapter. Ordering matters — add below.
            let status = http_status(error.code);
            ApiError::Protocol(status, error)
        })?;
    // Pure data-queried metric: rows returned by this execution.
    let row_count = match &response.response {
        rushdb_protocol::Response::Records { records, .. } => records.len() as u64,
        rushdb_protocol::Response::Rows { rows, .. }
        | rushdb_protocol::Response::TextHits { rows, .. } => rows.len() as u64,
        _ => 0,
    };
    node.observe_rows(op_kind, row_count);
    serde_json::to_value(response).map_err(|error| ApiError::Engine(error.to_string()))
}

/// Shared HTTP error taxonomy: the same `ErrorCode` maps to the same status on
/// every adapter. Add below — do not reorder.
pub(crate) fn http_status(code: rushdb_protocol::ErrorCode) -> u16 {
    match code {
        rushdb_protocol::ErrorCode::UnsupportedVersion => 426,
        rushdb_protocol::ErrorCode::ProtocolViolation
        | rushdb_protocol::ErrorCode::InvalidRequest => 400,
        rushdb_protocol::ErrorCode::Unauthorized => 401,
        rushdb_protocol::ErrorCode::MissingScope | rushdb_protocol::ErrorCode::ScopeMismatch => 403,
        rushdb_protocol::ErrorCode::NotFound => 404,
        rushdb_protocol::ErrorCode::Conflict | rushdb_protocol::ErrorCode::WriteConflict => 409,
        rushdb_protocol::ErrorCode::Cancelled => 499,
        rushdb_protocol::ErrorCode::ResourceExhausted => 429,
        rushdb_protocol::ErrorCode::Unavailable => 503,
        rushdb_protocol::ErrorCode::ExecutionFailed
        | rushdb_protocol::ErrorCode::LanguageDisabled
        | rushdb_protocol::ErrorCode::Unsupported => 422,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_error_taxonomy_is_complete_and_stable() {
        let expected = [
            (rushdb_protocol::ErrorCode::UnsupportedVersion, 426),
            (rushdb_protocol::ErrorCode::ProtocolViolation, 400),
            (rushdb_protocol::ErrorCode::InvalidRequest, 400),
            (rushdb_protocol::ErrorCode::Unauthorized, 401),
            (rushdb_protocol::ErrorCode::MissingScope, 403),
            (rushdb_protocol::ErrorCode::ScopeMismatch, 403),
            (rushdb_protocol::ErrorCode::NotFound, 404),
            (rushdb_protocol::ErrorCode::Conflict, 409),
            (rushdb_protocol::ErrorCode::Cancelled, 499),
            (rushdb_protocol::ErrorCode::ResourceExhausted, 429),
            (rushdb_protocol::ErrorCode::Unavailable, 503),
            (rushdb_protocol::ErrorCode::ExecutionFailed, 422),
            (rushdb_protocol::ErrorCode::LanguageDisabled, 422),
            (rushdb_protocol::ErrorCode::Unsupported, 422),
        ];
        for (code, status) in expected {
            assert_eq!(http_status(code), status, "{code:?}");
        }
    }
}
