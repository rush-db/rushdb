use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// Resolved tenant ordinal: the 4-byte prefix of every tenant-owned key
///. Assigned once by the tenant directory and never reused, so a purged
/// tenant's leftovers can never become visible to a later tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TenantId(pub u32);

impl TenantId {
    /// Reserved for system rows; never assigned to a tenant.
    pub const SYSTEM: Self = TenantId(0);
    /// The fixed tenant of single-tenant deployments.
    pub const SINGLE: Self = TenantId(1);
    /// First ordinal the tenant directory assigns.
    pub const FIRST_ASSIGNED: u32 = 2;
    /// A scope the directory has never registered. Never assigned, so it owns
    /// no keys: every read through it is empty.
    pub const UNREGISTERED: Self = TenantId(u32::MAX);

    /// The fixed identity used by single-tenant deployments.
    #[must_use]
    pub fn single() -> Self {
        Self::SINGLE
    }

    /// A tenant id from an ordinal the caller already resolved (tests,
    /// embedders, and the directory itself).
    ///
    /// # Panics
    /// Panics if `ordinal` does not fit the 4-byte key prefix.
    #[must_use]
    pub const fn from_ordinal(ordinal: u64) -> Self {
        assert!(ordinal <= u32::MAX as u64, "tenant ordinal exceeds u32");
        #[expect(clippy::cast_possible_truncation, reason = "range checked above")]
        TenantId(ordinal as u32)
    }

    /// Big-endian key-prefix bytes.
    #[must_use]
    pub const fn to_be_bytes(self) -> [u8; 4] {
        self.0.to_be_bytes()
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unresolved tenant identity: the SHA-256 digest of the caller's scope
/// string. The tenant directory maps it to a [`TenantId`]; only the digest is
/// stored, never the scope string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TenantScope(pub [u8; 32]);

impl TenantScope {
    /// Digest of a scope value.
    #[must_use]
    pub fn of(scope: &str) -> Self {
        TenantScope(Sha256::digest(scope.as_bytes()).into())
    }

    /// The scope of single-tenant deployments.
    #[must_use]
    pub fn single() -> Self {
        Self::of("rushdb:single-tenant")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_scope_values_are_stable_and_distinct() {
        assert_ne!(TenantScope::of("scope-a"), TenantScope::of("scope-b"));
        assert_eq!(TenantScope::of("not-a-uuid"), TenantScope::of("not-a-uuid"));
    }
}

macro_rules! dense_id {
    ($name:ident) => {
        #[doc = concat!("Dense monotonic ", stringify!($name), " identifier.")]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
            Default,
        )]
        pub struct $name(#[doc = "Wrapped ordinal value."] pub u64);

        impl $name {
            /// Return the identifier that follows this one.
            #[must_use]
            pub fn next(self) -> Self {
                $name(self.0 + 1)
            }
        }
    };
}

dense_id!(RecordOrdinal);
dense_id!(RelationshipOrdinal);
dense_id!(OccurrenceOrdinal);
dense_id!(FamilyId);
dense_id!(PropertyId);
dense_id!(ValueSeq);
dense_id!(SpaceId);

/// Lexical token identifier.
pub type TokenId = u32;

/// Stable 128-bit public identity of a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecordId(pub [u8; 16]);

impl RecordId {
    /// Generate a new time-ordered (`UUIDv7`) record id.
    #[must_use]
    pub fn generate() -> Self {
        RecordId(*Uuid::now_v7().as_bytes())
    }

    /// The all-zero record id, used as a sentinel.
    #[must_use]
    pub fn nil() -> Self {
        RecordId([0u8; 16])
    }

    /// View this id as a [`Uuid`].
    #[must_use]
    pub fn to_uuid(&self) -> Uuid {
        Uuid::from_bytes(self.0)
    }
}

impl From<Uuid> for RecordId {
    fn from(u: Uuid) -> Self {
        RecordId(*u.as_bytes())
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uuid().simple())
    }
}

/// Stable 128-bit public identity of a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelationshipId(pub [u8; 16]);

impl RelationshipId {
    /// Generate a new time-ordered (`UUIDv7`) relationship id.
    #[must_use]
    pub fn generate() -> Self {
        RelationshipId(*Uuid::now_v7().as_bytes())
    }

    /// View this id as a [`Uuid`].
    #[must_use]
    pub fn to_uuid(&self) -> Uuid {
        Uuid::from_bytes(self.0)
    }
}

impl From<Uuid> for RelationshipId {
    fn from(u: Uuid) -> Self {
        RelationshipId(*u.as_bytes())
    }
}

impl fmt::Display for RelationshipId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uuid().simple())
    }
}

/// Whether an owned row belongs to a record or a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OwnerKind {
    /// A record owner.
    Record,
    /// A relationship owner.
    Relationship,
}

impl OwnerKind {
    /// One-byte tag used in keys (`b'r'` or `b'e'`).
    #[must_use]
    pub fn as_byte(self) -> u8 {
        match self {
            OwnerKind::Record => b'r',
            OwnerKind::Relationship => b'e',
        }
    }

    /// Decode an owner kind from its one-byte key tag.
    #[must_use]
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            b'r' => Some(OwnerKind::Record),
            b'e' => Some(OwnerKind::Relationship),
            _ => None,
        }
    }
}
