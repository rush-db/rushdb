//! Shared error classification for the canonical protocol executor: engine,
//! storage, and language failures all map to one stable [`ErrorCode`]
//! taxonomy, regardless of which transport (Bolt, gRPC, HTTP, pgwire) a
//! request arrived through.

use rushdb_protocol::{ErrorCode, ProtocolError};

/// Map a [`rushdb_engine::SchedulerError`] (RM-3's single admission owner) to
/// a transport-facing [`ProtocolError`]. Shared by every transport
/// (`grpc.rs`, `pgwire.rs`, `http.rs`) so admission failures surface
/// identically regardless of which one a request arrived through.
#[expect(
    clippy::needless_pass_by_value,
    reason = "used directly as a `map_err` adapter, which supplies the owned error value"
)]
pub(crate) fn scheduler_error_to_protocol_error(
    error: rushdb_engine::SchedulerError,
) -> ProtocolError {
    let message = match error {
        rushdb_engine::SchedulerError::QueueFull => "tenant concurrent request limit exceeded",
        rushdb_engine::SchedulerError::Unavailable => "server is shutting down",
        rushdb_engine::SchedulerError::Panicked => "internal error executing request",
    };
    ProtocolError::new(ErrorCode::Unavailable, message)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "used directly as a `map_err` adapter, which supplies the owned error value"
)]
pub(crate) fn execution_failed(error: impl ToString) -> ProtocolError {
    let message = error.to_string();
    let code = if message.contains("cancelled") || message.contains("deadline exceeded") {
        ErrorCode::Cancelled
    } else if message.contains("unindexed walk refused") {
        // A fixed property of the query and the data: retrying
        // cannot help; declaring the named index does.
        ErrorCode::InvalidRequest
    } else if message.contains("resource exhausted:")
        || message.contains("storage resource exhausted:")
        || message.contains("execution budget exceeded")
        || message.contains("query working set exceeded")
        || message.contains("row safety limit")
        || message.contains("graph mutation staging exceeds")
    {
        // Budget and admission failures are not generic execution faults.
        // Keep the wire taxonomy stable even when an adapter has not yet
        // exposed its concrete error type.
        ErrorCode::ResourceExhausted
    } else if message.contains("storage unhealthy after lock poisoning")
        || message.contains("tenant_capacity")
    {
        // Residency: every resident tenant is active; retry later.
        ErrorCode::Unavailable
    } else if message.contains("denied by policy") || message.contains("` not authorized") {
        // AuthorizedView's label/property gate; same taxonomy as the
        // pre-execution authorize_search_in_view/authorize_aggregate_in_view.
        // Writes to a label outside `allowedLabels` say "label `X` not
        // authorized".
        ErrorCode::Unauthorized
    } else if message.contains("not found") || message.contains("unknown property") {
        ErrorCode::NotFound
    } else if message.contains("serializable_conflict") || message.contains("transaction_conflict")
    {
        ErrorCode::WriteConflict
    } else if message.contains("conflict")
        || message.contains("already exists")
        || message.contains("label changed")
        || message.contains("reference is stale")
    {
        ErrorCode::Conflict
    } else if message.contains("unsupported") {
        ErrorCode::Unsupported
    } else if is_invalid_request(&message) {
        ErrorCode::InvalidRequest
    } else {
        ErrorCode::ExecutionFailed
    };
    // A fixed payload/operation ceiling requires changing the transaction.
    // Shared-pool and active-transaction pressure may clear after other work
    // finishes, so retain the protocol's retryable default for those cases.
    let fixed_staging_limit = [
        "resource exhausted: transaction staging byte limit",
        "resource exhausted: transaction operation limit",
        "resource exhausted: transaction JSON depth/size limit",
        "resource exhausted: staging size overflow",
        "resource exhausted: staging capacity overflow",
        // Large transactions: a commit past the configured
        // transaction ceiling (`StorageError::ResourceExhausted`, Display
        // = "storage resource exhausted: {0}") — the ETL/bulk-load boundary.
        "storage resource exhausted: expanded mutation batch exceeds",
    ]
    .iter()
    .any(|reason| message.starts_with(reason));
    // SEC-5: do not forward an unclassified execution fault's raw text (paths,
    // key bytes, invariants); log it and return a generic message. Client-actionable
    // codes (invalid request, not found, conflict, limits, cancellation) keep theirs.
    let client_message = if code == ErrorCode::ExecutionFailed {
        tracing::warn!(detail = %message, "suppressing internal execution error detail");
        "internal execution error".to_string()
    } else {
        message
    };
    let error = ProtocolError::new(code, client_message);
    if fixed_staging_limit {
        error.with_retryable(false)
    } else {
        error
    }
}

/// Request validation the engine reports as text: a malformed `where`, a
/// missing required field, an out-of-range option. It describes the caller's
/// input, so the caller sees it. A stored-state fault (`corrupt …`) stays
/// internal however it is phrased.
fn is_invalid_request(message: &str) -> bool {
    !message.contains("corrupt")
        && (message.starts_with("invalid ")
            || message.contains("invalid property name")
            || message.contains("invalid merge key")
            || message.contains(" requires ")
            || message.contains(" require an explicit ")
            || message.contains(" must be "))
}

/// Native query failures carry their class in the error type: an
/// unevaluable predicate is the client's, a query budget or group cap is a
/// fixed property of the query and the data.
#[expect(
    clippy::needless_pass_by_value,
    reason = "used directly as a `map_err` adapter, which supplies the owned error value"
)]
pub(crate) fn query_failed(error: rushdb_query::QueryError) -> ProtocolError {
    use rushdb_query::QueryError;
    let message = error.to_string();
    match error {
        QueryError::InvalidPredicate(_) => ProtocolError::new(ErrorCode::InvalidRequest, message),
        QueryError::Cancelled => ProtocolError::new(ErrorCode::Cancelled, message),
        QueryError::ResourceLimit(_) => {
            ProtocolError::new(ErrorCode::ResourceExhausted, message).with_retryable(false)
        }
        QueryError::UnindexedWalk(_) => {
            ProtocolError::new(ErrorCode::InvalidRequest, message).with_retryable(false)
        }
        QueryError::Storage(_) | QueryError::Engine(_) => execution_failed(message),
    }
}

/// SQL failures carry their class in the error type: statement errors are
/// the client's, the row safety limit is a fixed property of the query.
#[expect(
    clippy::needless_pass_by_value,
    reason = "used directly as a `map_err` adapter, which supplies the owned error value"
)]
pub(crate) fn sql_failed(error: rushdb_sql::SqlError) -> ProtocolError {
    use rushdb_sql::SqlError;
    let message = error.to_string();
    match error {
        SqlError::Parse(_)
        | SqlError::InvalidIdentity(_)
        | SqlError::TextSearch(_)
        | SqlError::UnindexedWalk(_) => ProtocolError::new(ErrorCode::InvalidRequest, message),
        SqlError::ReadOnly(_) => ProtocolError::new(
            ErrorCode::InvalidRequest,
            format!("{message}; send writes as mutate.statement"),
        ),
        SqlError::UnsupportedMutation(_) => ProtocolError::new(ErrorCode::Unsupported, message),
        SqlError::ResourceLimit(_) | SqlError::MemoryLimit(_) => {
            ProtocolError::new(ErrorCode::ResourceExhausted, message).with_retryable(false)
        }
        SqlError::Cancelled => ProtocolError::new(ErrorCode::Cancelled, message),
        SqlError::WriteConflict(_) => ProtocolError::new(ErrorCode::WriteConflict, message),
        SqlError::BlockingInRuntime
        | SqlError::Storage(_)
        | SqlError::Commit(_)
        | SqlError::Engine(_) => execution_failed(message),
    }
}

/// Decode a required numeric field from an engine response. Protocol response
/// shaping must not turn a malformed or schema-shifted engine payload into a
/// successful zero-count mutation.
pub(crate) fn response_u64(value: &serde_json::Value, field: &str) -> Result<u64, ProtocolError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| execution_failed(format!("engine response missing valid `{field}`")))
}

pub(crate) fn response_array(
    value: &serde_json::Value,
    field: &str,
) -> Result<Vec<serde_json::Value>, ProtocolError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .cloned()
        .ok_or_else(|| execution_failed(format!("engine response missing array `{field}`")))
}

/// Community-language parsers distinguish syntax they do not implement from a
/// storage or execution failure. Preserve that distinction at Protocol v1 so a
/// client can rely on the advertised profile instead of heuristically parsing
/// an opaque server error.
#[expect(
    clippy::needless_pass_by_value,
    reason = "used directly as a `map_err` adapter, which supplies the owned error value"
)]
pub(crate) fn language_execution_failed(error: rushdb_language::LanguageError) -> ProtocolError {
    use rushdb_language::LanguageError;
    let message = error.to_string();
    match error {
        LanguageError::MutationBytesExceeded(_) | LanguageError::GraphTooLarge(_) => {
            ProtocolError::new(ErrorCode::ResourceExhausted, message).with_retryable(false)
        }
        LanguageError::UnsupportedMutation(_) => {
            ProtocolError::new(ErrorCode::Unsupported, message)
        }
        LanguageError::Cancelled => ProtocolError::new(ErrorCode::Cancelled, message),
        LanguageError::WriteConflict(_) => ProtocolError::new(ErrorCode::WriteConflict, message),
        // The community engine reports syntax errors only as text.
        LanguageError::Query { .. } if message.contains("syntax error") => {
            ProtocolError::new(ErrorCode::Unsupported, message)
        }
        // Its internal errors (`GRAFEO-X…`) are ours, except a feature it
        // was built without, which is an unsupported construct.
        LanguageError::Query { .. } if message.contains("GRAFEO-X") => {
            if message.contains(" require the ") {
                ProtocolError::new(ErrorCode::Unsupported, message)
            } else {
                execution_failed(message)
            }
        }
        // Every other statement error (semantic, type, parameter, a write on
        // the read path, a failed text search) is the client's, with its
        // explanation.
        LanguageError::Query { .. }
        | LanguageError::InvalidParameters
        | LanguageError::StatementTooLong { .. }
        | LanguageError::TextSearch(_) => ProtocolError::new(ErrorCode::InvalidRequest, message),
        LanguageError::Storage(_) | LanguageError::Initialization(_) | LanguageError::Commit(_) => {
            execution_failed(message)
        }
    }
}

#[test]
fn unhealthy_storage_and_graph_admission_preserve_protocol_outcomes() {
    let unhealthy = execution_failed(rushdb_storage::StorageError::Unhealthy);
    assert_eq!(unhealthy.code, ErrorCode::Unavailable);
    let exhausted =
        language_execution_failed(rushdb_language::LanguageError::MutationBytesExceeded(1024));
    assert_eq!(exhausted.code, ErrorCode::ResourceExhausted);
    assert!(!exhausted.retryable);
}

#[test]
fn staging_limit_is_resource_exhausted_not_generic_execution_failure() {
    for reason in [
        "resource exhausted: transaction staging byte limit",
        "resource exhausted: transaction operation limit",
        "resource exhausted: transaction JSON depth/size limit",
    ] {
        let error = execution_failed(reason);
        assert_eq!(error.code, ErrorCode::ResourceExhausted);
        assert!(
            !error.retryable,
            "a fixed per-transaction limit is not fixed by blind retry"
        );
    }
}

#[test]
fn malformed_engine_count_response_fails_closed() {
    let value = serde_json::json!({"created": "one"});
    let error = response_u64(&value, "created").expect_err("invalid count must be rejected");
    assert_eq!(error.code, ErrorCode::ExecutionFailed);
}

#[test]
fn malformed_engine_array_response_fails_closed() {
    let value = serde_json::json!({"data": null});
    let error = response_array(&value, "data").expect_err("invalid array must be rejected");
    assert_eq!(error.code, ErrorCode::ExecutionFailed);
}

#[test]
fn temporary_staging_pressure_remains_retryable() {
    for reason in [
        "resource exhausted: active transaction limit",
        "resource exhausted: resource budget `transaction-staging` exhausted",
    ] {
        let error = execution_failed(reason);
        assert_eq!(error.code, ErrorCode::ResourceExhausted);
        assert!(error.retryable);
    }
}
