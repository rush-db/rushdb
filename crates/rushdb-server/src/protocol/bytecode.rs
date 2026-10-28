//! Gremlin bytecode operations become Gremlin statements before dispatch:
//! one translation (`rushdb_language::gremlin_bytecode`), then the same
//! authorization, language gating, execution and atomicity as Gremlin text.

use rushdb_protocol::{
    ErrorCode, Mutation, Operation, ProtocolError, Query, RequestEnvelope, StatementLanguage,
};

fn text(bytecode: &serde_json::Value) -> Result<String, ProtocolError> {
    rushdb_language::gremlin_bytecode::bytecode_to_gremlin(bytecode)
        .map_err(|error| ProtocolError::new(ErrorCode::Unsupported, error.to_string()))
}

/// `request` with a bytecode query or mutation rewritten as the equivalent
/// Gremlin statement; any other request unchanged.
pub(super) fn as_statement(mut request: RequestEnvelope) -> Result<RequestEnvelope, ProtocolError> {
    request.operation = match request.operation {
        Operation::Query {
            query:
                Query::GremlinBytecode {
                    bytecode,
                    parameters,
                },
        } => Operation::Query {
            query: Query::Statement {
                language: StatementLanguage::Gremlin,
                statement: text(&bytecode)?,
                parameters,
            },
        },
        Operation::Mutate {
            mutation:
                Mutation::GremlinBytecode {
                    bytecode,
                    parameters,
                },
        } => Operation::Mutate {
            mutation: Mutation::Statement {
                language: StatementLanguage::Gremlin,
                statement: text(&bytecode)?,
                parameters,
            },
        },
        other => other,
    };
    Ok(request)
}

/// A bytecode operation reached dispatch without [`as_statement`].
pub(super) fn not_rewritten() -> ProtocolError {
    ProtocolError::new(
        ErrorCode::ExecutionFailed,
        "gremlin bytecode reached dispatch untranslated",
    )
}
