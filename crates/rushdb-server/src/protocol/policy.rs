//! Protocol handler for a tenant's data-access policy (operator only).

use super::{execution_failed, ErrorCode, ProtocolError, ProtocolExecutor, Response};

impl ProtocolExecutor<'_> {
    /// Read or replace the tenant's data-access policy.
    pub(super) fn policy(
        &self,
        operation: rushdb_protocol::PolicyOperation,
        execution_context: &rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        match operation {
            rushdb_protocol::PolicyOperation::Get => {
                let active = rushdb_authz::load_active_policy_in_view(
                    execution_context.read_view().as_ref(),
                    execution_context.tenant(),
                )
                .map_err(|error| execution_failed(error.to_string()))?;
                Ok(match active {
                    Some(active) => Response::Policy {
                        policy: Some(policy_to_wire(&active.policy, active.min_count)),
                        enforced: active.mode == rushdb_authz::AuthorizationMode::PolicyEnforced,
                        generation: Some(active.generation),
                    },
                    None => Response::Policy {
                        policy: None,
                        enforced: false,
                        generation: None,
                    },
                })
            }
            rushdb_protocol::PolicyOperation::Put { policy, enforced } => {
                if self.node.maintenance() {
                    return Err(ProtocolError::new(
                        ErrorCode::Unavailable,
                        "maintenance mode: writes are rejected until maintenance is cleared",
                    ));
                }
                let (parsed, min_count) = policy_from_wire(policy)?;
                let mode = if enforced {
                    rushdb_authz::AuthorizationMode::PolicyEnforced
                } else {
                    rushdb_authz::AuthorizationMode::BoundaryOnly
                };
                let stored = self
                    .node
                    .security_put_policy_with_min_count(&parsed, mode, min_count)
                    .map_err(execution_failed)?;
                Ok(Response::Policy {
                    policy: Some(policy_to_wire(&parsed, min_count)),
                    enforced,
                    generation: stored.get("generation").and_then(serde_json::Value::as_u64),
                })
            }
        }
    }
}

/// The wire (camelCase) form of a policy.
fn policy_to_wire(policy: &rushdb_authz::DataAccessPolicy, min_count: u64) -> serde_json::Value {
    serde_json::json!({
        "allowedLabels": policy.allowed_labels,
        "deniedRelTypes": policy.denied_rel_types,
        "visibleProperties": policy.visible_properties,
        "maxResults": policy.max_results,
        "minCount": min_count,
    })
}

/// Parse a wire policy; an unknown or mistyped field is `invalid_request`.
fn policy_from_wire(
    policy: serde_json::Value,
) -> Result<(rushdb_authz::DataAccessPolicy, u64), ProtocolError> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct WirePolicy {
        #[serde(default)]
        allowed_labels: Vec<String>,
        #[serde(default)]
        denied_rel_types: Vec<String>,
        #[serde(default)]
        visible_properties: std::collections::BTreeMap<String, Vec<String>>,
        #[serde(default)]
        max_results: u64,
        #[serde(default)]
        min_count: u64,
    }
    let wire: WirePolicy = serde_json::from_value(policy).map_err(|error| {
        ProtocolError::new(
            ErrorCode::InvalidRequest,
            format!("invalid policy: {error}"),
        )
    })?;
    Ok((
        rushdb_authz::DataAccessPolicy {
            allowed_labels: wire.allowed_labels,
            denied_rel_types: wire.denied_rel_types,
            visible_properties: wire.visible_properties,
            max_results: wire.max_results,
        },
        wire.min_count,
    ))
}
