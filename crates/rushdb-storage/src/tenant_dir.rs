//! Tenant directory: scope digest ↔ 4-byte tenant ordinal.
//!
//! The rows live in the system namespace and are written by an ordinary
//! commit (`Mutation::RegisterTenant`, expanded by LMPG), so registration is
//! logged, replicated and replayed like any other write. Ordinals are never
//! reused: a purged tenant keeps its rows with state `Purged`, and a later
//! registration of the same scope gets a fresh ordinal, so leftovers of the
//! old ordinal can never become visible. This module owns the encoding and a
//! bounded resolution cache.

use std::collections::HashMap;

use rushdb_types::ids::{TenantId, TenantScope};
use rushdb_types::keys;

use crate::{StorageError, Store};

/// Maximum cached scope → ordinal entries; the cache is cleared when full
/// (entries are re-read from disk on the next miss).
const CACHE_ENTRIES: usize = 65_536;

/// Lifecycle state of a directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantState {
    /// Serving reads and writes.
    Active,
    /// Purged; the ordinal is retired forever.
    Purged,
}

impl TenantState {
    fn to_byte(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Purged => 1,
        }
    }

    fn from_byte(byte: u8) -> Result<Self, StorageError> {
        match byte {
            0 => Ok(Self::Active),
            1 => Ok(Self::Purged),
            _ => Err(StorageError::CorruptLog),
        }
    }
}

/// Encode a scope → ordinal row value.
#[must_use]
pub fn encode_by_scope(tenant: TenantId, state: TenantState) -> Vec<u8> {
    let mut out = tenant.to_be_bytes().to_vec();
    out.push(state.to_byte());
    out
}

/// Decode a scope → ordinal row value.
///
/// # Errors
/// Returns [`StorageError::CorruptLog`] for a malformed value.
pub fn decode_by_scope(raw: &[u8]) -> Result<(TenantId, TenantState), StorageError> {
    let (ordinal, state) = raw.split_at_checked(4).ok_or(StorageError::CorruptLog)?;
    let ordinal: [u8; 4] = ordinal.try_into().map_err(|_| StorageError::CorruptLog)?;
    let [state] = state else {
        return Err(StorageError::CorruptLog);
    };
    Ok((
        TenantId(u32::from_be_bytes(ordinal)),
        TenantState::from_byte(*state)?,
    ))
}

/// Encode an ordinal → scope row value.
#[must_use]
pub fn encode_by_ord(scope: &TenantScope, state: TenantState) -> Vec<u8> {
    let mut out = scope.0.to_vec();
    out.push(state.to_byte());
    out
}

/// Decode an ordinal → scope row value.
///
/// # Errors
/// Returns [`StorageError::CorruptLog`] for a malformed value.
pub fn decode_by_ord(raw: &[u8]) -> Result<(TenantScope, TenantState), StorageError> {
    let (digest, state) = raw.split_at_checked(32).ok_or(StorageError::CorruptLog)?;
    let digest: [u8; 32] = digest.try_into().map_err(|_| StorageError::CorruptLog)?;
    let [state] = state else {
        return Err(StorageError::CorruptLog);
    };
    Ok((TenantScope(digest), TenantState::from_byte(*state)?))
}

/// Decode the next-ordinal row (absent → [`TenantId::FIRST_ASSIGNED`]).
///
/// # Errors
/// Returns [`StorageError::CorruptLog`] for a malformed value.
pub fn decode_seq(raw: Option<&[u8]>) -> Result<u32, StorageError> {
    raw.map_or(Ok(TenantId::FIRST_ASSIGNED), |raw| {
        raw.try_into()
            .map(u32::from_be_bytes)
            .map_err(|_| StorageError::CorruptLog)
    })
}

/// Bounded scope → ordinal cache in front of the directory rows.
#[derive(Default)]
pub(crate) struct TenantCache {
    entries: HashMap<TenantScope, TenantId>,
}

impl Store {
    /// The active ordinal registered for `scope`, if any. Reads through a
    /// bounded cache; a miss reads the committed directory row.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the directory row cannot be read or is
    /// malformed.
    pub fn resolve_tenant(&self, scope: &TenantScope) -> Result<Option<TenantId>, StorageError> {
        if let Some(tenant) = self
            .health
            .guard(self.tenant_cache.lock())?
            .entries
            .get(scope)
        {
            return Ok(Some(*tenant));
        }
        let Some(raw) = self.try_get(&keys::tenant_by_scope_key(scope))? else {
            return Ok(None);
        };
        let (tenant, state) = decode_by_scope(&raw)?;
        if state != TenantState::Active {
            return Ok(None);
        }
        let mut cache = self.health.guard(self.tenant_cache.lock())?;
        if cache.entries.len() >= CACHE_ENTRIES {
            cache.entries.clear();
        }
        cache.entries.insert(*scope, tenant);
        Ok(Some(tenant))
    }

    /// The scope digest registered for an ordinal, with its state.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the directory row cannot be read or is
    /// malformed.
    pub fn tenant_scope(
        &self,
        tenant: TenantId,
    ) -> Result<Option<(TenantScope, TenantState)>, StorageError> {
        self.try_get(&keys::tenant_by_ord_key(tenant))?
            .map(|raw| decode_by_ord(&raw))
            .transpose()
    }

    /// Every active registered tenant, in ordinal order (O(tenants)).
    ///
    /// # Errors
    /// Returns [`StorageError`] if the directory cannot be read or a row is
    /// malformed.
    pub fn active_tenants(&self) -> Result<Vec<(TenantId, TenantScope)>, StorageError> {
        let prefix = keys::tenant_by_ord_prefix();
        let mut out = Vec::new();
        for (key, raw) in self.try_range_prefix(&prefix)? {
            let ordinal: [u8; 4] = key
                .get(prefix.len()..)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(StorageError::CorruptLog)?;
            let (scope, state) = decode_by_ord(&raw)?;
            if state == TenantState::Active {
                out.push((TenantId(u32::from_be_bytes(ordinal)), scope));
            }
        }
        Ok(out)
    }

    /// Drop a scope from the resolution cache (after it is retired).
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn forget_tenant(&self, scope: &TenantScope) {
        self.health
            .cleanup(self.tenant_cache.lock())
            .entries
            .remove(scope);
    }
}
