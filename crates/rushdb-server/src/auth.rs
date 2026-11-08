//! The one credential rule every transport applies: the boot token is the
//! operator; a signed capability (`--capability-secret`) is accepted for
//! exactly the tenant it is bound to, and grants the permissions in its
//! scope. HTTP reads the credential from a header, pgwire from the
//! password, gRPC from bearer metadata and Bolt from `HELLO` (and each
//! `BEGIN`/`RUN` that names a tenant); all of them
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
        if self.is_boot_token(credential) {
            return Ok(Principal::Operator);
        }
        let capability = self.verify_capability(credential)?;
        if !crate::capability::tenant_matches(self.multitenant, &capability, scope) {
            return Err(refused());
        }
        Ok(Principal::Capability {
            scope: capability.scope,
        })
    }

    /// The principal `credential` grants to a session that names no tenant
    /// at login, and the tenant it is bound to: the boot token is the
    /// operator of no tenant in particular; a capability is bound to its
    /// own tenant on a multitenant server and to none on a single-tenant one.
    ///
    /// # Errors
    /// Returns `unauthorized` when the credential is neither the boot token
    /// nor a verified capability bound the way the server's mode requires.
    pub(crate) fn authenticate_session(
        &self,
        credential: &str,
    ) -> Result<(Principal, Option<String>), ProtocolError> {
        if self.is_boot_token(credential) {
            return Ok((Principal::Operator, None));
        }
        let capability = self.verify_capability(credential)?;
        let tenant = if self.multitenant {
            capability.tenant.clone()
        } else {
            None
        };
        if !crate::capability::tenant_matches(self.multitenant, &capability, tenant.as_deref()) {
            return Err(refused());
        }
        Ok((
            Principal::Capability {
                scope: capability.scope,
            },
            tenant,
        ))
    }

    fn is_boot_token(&self, credential: &str) -> bool {
        rushdb_protocol::constant_time_eq(credential.as_bytes(), self.token.as_bytes())
    }

    /// The capability `credential` is, verified against the secret and now.
    fn verify_capability(
        &self,
        credential: &str,
    ) -> Result<crate::capability::Capability, ProtocolError> {
        let secret = self.capability_secret.as_ref().ok_or_else(refused)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        crate::capability::verify(secret, credential, now).map_err(|_| refused())
    }
}

fn refused() -> ProtocolError {
    ProtocolError::new(ErrorCode::Unauthorized, "invalid token")
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
