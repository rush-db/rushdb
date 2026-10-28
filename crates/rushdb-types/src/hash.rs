/// Hash domain separator for the explicit-null value marker.
pub const NULL_HASH_DOMAIN: &[u8] = b"rushdb:explicit-null:v1";

fn prefix64(bytes: &[u8; 32]) -> u64 {
    let mut prefix = [0u8; 8];
    prefix.copy_from_slice(&bytes[..8]);
    u64::from_be_bytes(prefix)
}

/// Stable 64-bit digest of arbitrary bytes, independent of process state.
#[must_use]
pub fn stable_hash64(bytes: &[u8]) -> u64 {
    prefix64(blake3::hash(bytes).as_bytes())
}

/// Domain-separated 64-bit hash of a type tag and payload.
#[must_use]
pub fn hash_domain(domain: &[u8], ty: u8, payload: &[u8]) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&[ty]);
    hasher.update(payload);
    prefix64(hasher.finalize().as_bytes())
}

/// Hash of the payload-free explicit-null marker.
#[must_use]
pub fn null_value_hash() -> u64 {
    stable_hash64(NULL_HASH_DOMAIN)
}

/// Value hash for a canonical payload under the given type tag.
#[must_use]
pub fn value_hash(ty: u8, canonical_payload: &[u8]) -> u64 {
    hash_domain(b"rushdb:value:v1", ty, canonical_payload)
}
