//! Signed capability tokens (plan document 14/25 security transport): a
//! capability is `base64url(claims).sig` where `sig` is HMAC-SHA256 over the
//! claims with a server-held secret. Claims carry subject, scope (space
//! separated permissions), an optional opaque tenant binding, issuance and
//! expiry. Verification checks the signature and expiry; transports enforce
//! permission and exact tenant binding on every connection/request.
//! An opaque token is never authority on its own — the signature binds it to
//! the issuing secret, and the scope gates every route.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Verified claims carried by a signed capability token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    /// Subject the capability was issued to.
    pub sub: String,
    /// Space-separated permissions granted to the subject.
    pub scope: String,
    /// Opaque scope value this capability is allowed to select.
    /// `None` is valid only for a single-tenant server.
    pub tenant: Option<String>,
    /// Issued-at time as a Unix timestamp in seconds.
    pub iat: u64,
    /// Expiry time as a Unix timestamp in seconds.
    pub exp: u64,
}

/// Failure modes when issuing or verifying a capability token.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CapError {
    /// The token is not `base64url(claims).signature` or decodes to invalid JSON.
    #[error("malformed capability")]
    Malformed,
    /// The HMAC signature did not match the claims under the server secret.
    #[error("signature verification failed")]
    BadSignature,
    /// The token's `exp` timestamp is earlier than the verification time.
    #[error("capability expired")]
    Expired,
    /// A requested permission is absent from the capability's scope.
    #[error("permission `{0}` not in scope")]
    ScopeDenied(String),
}

/// Issue a signed capability with the given TTL (seconds).
#[must_use]
pub fn issue(secret: &[u8], sub: &str, scope: &str, ttl_secs: u64, now: u64) -> String {
    issue_for_tenant(secret, sub, scope, None, ttl_secs, now)
}

/// Issue a capability bound to one opaque scope value.
#[must_use]
pub fn issue_for_tenant(
    secret: &[u8],
    sub: &str,
    scope: &str,
    tenant: Option<&str>,
    ttl_secs: u64,
    now: u64,
) -> String {
    let mut claims = serde_json::Map::new();
    claims.insert("sub".into(), serde_json::Value::String(sub.to_string()));
    claims.insert("scope".into(), serde_json::Value::String(scope.to_string()));
    if let Some(tenant) = tenant {
        claims.insert(
            "tenant".into(),
            serde_json::Value::String(tenant.to_string()),
        );
    }
    claims.insert("iat".into(), serde_json::json!(now));
    claims.insert("exp".into(), serde_json::json!(now + ttl_secs));
    let claims = serde_json::Value::Object(claims).to_string();
    let b64 = URL_SAFE_NO_PAD.encode(claims.as_bytes());
    let sig = sign(secret, &b64);
    format!("{b64}.{}", URL_SAFE_NO_PAD.encode(sig))
}

/// Verify signature + expiry, returning the parsed claims.
///
/// # Errors
///
/// Returns [`CapError::Malformed`] when the token cannot be split, decoded, or
/// parsed; [`CapError::BadSignature`] when the HMAC does not verify; and
/// [`CapError::Expired`] when `exp` is earlier than `now`.
///
/// # Panics
///
/// Never panics in practice: HMAC-SHA256 accepts keys of any length, so the
/// internal `new_from_slice` call cannot fail.
pub fn verify(secret: &[u8], token: &str, now: u64) -> Result<Capability, CapError> {
    let Some((b64, sigb64)) = token.split_once('.') else {
        return Err(CapError::Malformed);
    };
    let sig = URL_SAFE_NO_PAD
        .decode(sigb64)
        .map_err(|_| CapError::Malformed)?;
    let mut mac = HmacSha256::new_from_slice(secret).expect("any key length");
    mac.update(b64.as_bytes());
    mac.verify_slice(&sig).map_err(|_| CapError::BadSignature)?;
    let claims = String::from_utf8(
        URL_SAFE_NO_PAD
            .decode(b64)
            .map_err(|_| CapError::Malformed)?,
    )
    .map_err(|_| CapError::Malformed)?;
    let v: serde_json::Value = serde_json::from_str(&claims).map_err(|_| CapError::Malformed)?;
    let cap = Capability {
        sub: v
            .get("sub")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        scope: v
            .get("scope")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        tenant: v.get("tenant").and_then(|x| x.as_str()).map(str::to_string),
        iat: v
            .get("iat")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        exp: v
            .get("exp")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
    };
    if cap.exp < now {
        return Err(CapError::Expired);
    }
    Ok(cap)
}

/// Check that `permission` is granted by the scope string.
#[must_use]
pub fn allows_scope(scope: &str, permission: &str) -> bool {
    scope
        .split_whitespace()
        .any(|p| p == "*" || p == permission)
}

fn sign(secret: &[u8], data: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(secret).expect("any key length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod rotation_tests {
    use super::*;

    #[test]
    fn secret_rotation_invalidates_outstanding_tokens() {
        let old_secret: &[u8] = b"0123456789abcdef0123456789abcdef";
        let new_secret: &[u8] = b"fedcba9876543210fedcba9876543210";
        let now = 1_700_000_000u64;
        let token = issue_for_tenant(old_secret, "svc", "read", Some("tenant-a"), 3600, now);
        // Under the old secret the token verifies.
        let ok = verify(old_secret, &token, now).unwrap();
        assert_eq!(ok.scope, "read");
        // After rotation the token no longer verifies (HMAC key change).
        assert!(verify(new_secret, &token, now).is_err());
    }
}
