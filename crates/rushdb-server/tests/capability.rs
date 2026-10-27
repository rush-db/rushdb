//! Signed capability tokens (plan document 14/25): HMAC-SHA256 signatures,
//! expiry, and scope gating enforced over HTTP. The boot token remains a
//! full-access fallback; a capability is only as strong as its signature.

use rushdb_server::capability::{allows_scope, issue, issue_for_tenant, verify, CapError};

const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

#[test]
fn issue_and_verify_roundtrip() {
    let now = 1_700_000_000u64;
    let token = issue(SECRET, "svc-a", "read", 3600, now);
    let cap = verify(SECRET, &token, now + 100).unwrap();
    assert_eq!(cap.sub, "svc-a");
    assert_eq!(cap.scope, "read");
    assert_eq!(cap.tenant, None);
    assert_eq!(cap.iat, now);
    assert_eq!(cap.exp, now + 3600);
}

#[test]
fn tenant_capability_roundtrip_binds_opaque_scope() {
    let now = 1_700_000_000u64;
    let token = issue_for_tenant(SECRET, "svc-a", "read write", Some("tenant-a"), 60, now);
    let cap = verify(SECRET, &token, now + 1).unwrap();
    assert_eq!(cap.tenant.as_deref(), Some("tenant-a"));
}

#[test]
fn tampered_token_fails_signature() {
    let now = 1_700_000_000u64;
    let token = issue(SECRET, "svc-a", "read", 3600, now);
    // flip one char in the claims segment
    let (claims, sig) = token.split_once('.').unwrap();
    let tampered = format!(
        "{}.",
        claims.replace(
            claims.get(3..4).unwrap(),
            if claims.as_bytes()[3] == b'C' {
                "D"
            } else {
                "C"
            }
        )
    );
    let _ = sig;
    let full = format!("{tampered}{}", token.split_once('.').unwrap().1);
    assert_eq!(verify(SECRET, &full, now), Err(CapError::BadSignature));
    // wrong secret
    assert_eq!(
        verify(b"different-secret", &token, now),
        Err(CapError::BadSignature)
    );
}

#[test]
fn expired_token_rejected() {
    let now = 1_700_000_000u64;
    let token = issue(SECRET, "svc-a", "read", 60, now);
    assert!(verify(SECRET, &token, now + 30).is_ok());
    assert_eq!(verify(SECRET, &token, now + 61), Err(CapError::Expired));
}

#[test]
fn scope_check() {
    assert!(allows_scope("read", "read"));
    assert!(!allows_scope("read", "write"));
    assert!(allows_scope("read write", "write"));
    assert!(allows_scope("*", "anything"));
    assert!(!allows_scope("", "read"));
}

#[test]
fn malformed_rejected() {
    assert_eq!(
        verify(SECRET, "not-a-capability", 0),
        Err(CapError::Malformed)
    );
    // Structurally two segments but invalid base64url is malformed.
    assert_eq!(verify(SECRET, "abc.xyz", 0), Err(CapError::Malformed));
}
