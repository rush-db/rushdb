//! Tenant directory: scope digest ↔ 4-byte tenant ordinal.
//!
//! The rows live in the system namespace and are written by an ordinary
//! commit (`Mutation::RegisterTenant`, expanded by LMPG), so registration is
//! logged, replicated and replayed like any other write. Ordinals are never
//! reused: a purged tenant keeps its rows with state `Purged`, and a later
//! registration of the same scope gets a fresh ordinal, so leftovers of the
//! old ordinal can never become visible. This module owns the encoding and
//! the resident resolution layer.

use rushdb_types::ids::{TenantId, TenantScope};
use rushdb_types::keys;

use crate::{StorageError, Store};

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

/// Scope → active ordinal (`None`: unregistered or retired), a resident
/// layer valid until a commit writes the tenant directory: a registration
/// or a retirement anywhere, on this node or applied from the leader.
#[derive(Default)]
pub(crate) struct TenantDirectory;

impl crate::resident::Resident for TenantDirectory {
    type Key = TenantScope;
    type Value = Option<TenantId>;
    const NAME: &'static str = "tenant-directory";
    const BUDGET: crate::resident::Budget = crate::resident::Budget::Logical;

    fn deps(
        &self,
        _scope: &TenantScope,
        _view: &crate::ReadView,
    ) -> Result<Vec<crate::resident::Dep>, StorageError> {
        Ok(vec![crate::resident::Dep::TenantDir])
    }

    fn build(
        &self,
        scope: &TenantScope,
        view: &crate::ReadView,
        _cx: &crate::resident::BuildCx,
    ) -> Result<Option<Option<TenantId>>, crate::resident::BuildError> {
        Ok(Some(active_ordinal(
            view.get(&keys::tenant_by_scope_key(scope))?,
        )?))
    }

    fn bytes(_: &Option<TenantId>) -> usize {
        std::mem::size_of::<(TenantScope, Option<TenantId>)>()
    }
}

/// The active ordinal a scope row names, if any.
fn active_ordinal(raw: Option<Vec<u8>>) -> Result<Option<TenantId>, StorageError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let (tenant, state) = decode_by_scope(&raw)?;
    Ok((state == TenantState::Active).then_some(tenant))
}

impl Store {
    /// The active ordinal registered for `scope`, if any. Reads through a
    /// bounded cache; a miss reads the committed directory row.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the directory row cannot be read or is
    /// malformed.
    pub fn resolve_tenant(&self, scope: &TenantScope) -> Result<Option<TenantId>, StorageError> {
        let layer = self.residents().layer::<TenantDirectory>();
        if let Some(tenant) = layer.get_latest(scope) {
            return Ok(*tenant);
        }
        let view = self.read_view()?;
        let tenant = active_ordinal(view.get(&keys::tenant_by_scope_key(scope))?)?;
        layer.offer(*scope, &view, std::sync::Arc::new(tenant));
        Ok(tenant)
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
}
