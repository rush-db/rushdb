//! The one credential rule every transport applies: the boot token is the
//! operator; a signed capability (`--capability-secret`) is accepted for
//! exactly the tenant it is bound to, and grants the permissions in its
//! scope. HTTP reads the credential from a header, pgwire from the
//! password, gRPC from bearer metadata and Bolt from `HELLO`; all of them
//! end here.

use rushdb_protocol::{ErrorCode, Principal, ProtocolError};

/// What a credential is checked against.
#[derive(Clone)]
pub(crate) struct Credentials {
    token: String,
    capability_secret: Option<Vec<u8>>,
    multitenant: bool,
}

impl Credentials {
    /// The server's credentials, from its configuration.
    pub(crate) fn from_config(cfg: &crate::ServerConfig) -> Self {
        Self {
            token: cfg.token.clone(),
            capability_secret: cfg.capability_secret.clone(),
            multitenant: cfg.multitenant,
        }
    }

    /// The principal `credential` grants for `scope` (the requested tenant).
    ///
    /// # Errors
    /// Returns `unauthorized` when the credential is neither the boot token
    /// nor a capability verified and bound to `scope`.
    pub(crate) fn authenticate(
        &self,
        credential: &str,
        scope: Option<&str>,
    ) -> Result<Principal, ProtocolError> {
        if rushdb_protocol::constant_time_eq(credential.as_bytes(), self.token.as_bytes()) {
            return Ok(Principal::Operator);
        }
        let refused = || ProtocolError::new(ErrorCode::Unauthorized, "invalid token");
        let secret = self.capability_secret.as_ref().ok_or_else(refused)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let capability =
            crate::capability::verify(secret, credential, now).map_err(|_| refused())?;
        if !crate::capability::tenant_matches(self.multitenant, &capability, scope) {
            return Err(refused());
        }
        Ok(Principal::Capability {
            scope: capability.scope,
        })
    }
}

/// The refusal a principal that lacks `permission` gets, on every transport.
pub(crate) fn forbidden(permission: &str) -> ProtocolError {
    ProtocolError::new(
        ErrorCode::Unauthorized,
        format!("{FORBIDDEN_PREFIX}`{permission}`"),
    )
}

/// How a permission refusal starts: transports with a distinct status for
/// "authenticated but not allowed" (gRPC `PERMISSION_DENIED`, Bolt
/// `Forbidden`) tell it from a failed login by this prefix.
pub(crate) const FORBIDDEN_PREFIX: &str = "the session's credential does not grant ";
