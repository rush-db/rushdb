//! Tenant directory resolution and registration.
//!
//! A scope string is hashed to a [`TenantScope`]; the directory maps it to a
//! 4-byte [`TenantId`] ordinal. Registration is an ordinary commit expanded
//! by LMPG, so it is logged, replicated and replayed deterministically.

use std::sync::Arc;

use rushdb_storage::{CommitOutcome, StorageError, Store};
use rushdb_types::command::Mutation;
use rushdb_types::ids::{TenantId, TenantScope};
use uuid::Uuid;

use crate::Lmpg;

/// The active ordinal for `scope`, registering it first if the directory has
/// never seen it (or only as a purged tenant, which gets a fresh ordinal).
///
/// # Errors
/// Returns [`StorageError`] if the directory cannot be read or the
/// registration commit fails.
pub fn resolve_or_register(
    store: &Arc<Store>,
    scope: &TenantScope,
) -> Result<TenantId, StorageError> {
    if let Some(tenant) = store.resolve_tenant(scope)? {
        return Ok(tenant);
    }
    // Registration expands through LMPG; make sure its expander is installed
    // even when no tenant facade exists yet on this store.
    drop(Lmpg::new(Arc::clone(store), TenantId::SYSTEM));
    let commit_key = Uuid::now_v7();
    let scope = *scope;
    match store.commit(commit_key, |ci, _txn| {
        let mut batch = crate::empty_batch(TenantId::SYSTEM, ci);
        batch.mutations = vec![Mutation::RegisterTenant { scope }];
        Ok((batch, ()))
    })? {
        CommitOutcome::Committed { .. } => {}
        CommitOutcome::Conflicted(reason) => return Err(StorageError::Builder(reason)),
    }
    store
        .resolve_tenant(&scope)?
        .ok_or(StorageError::CorruptLog)
}

/// [`resolve_or_register`] for a scope string.
///
/// # Errors
/// See [`resolve_or_register`].
pub fn resolve_or_register_scope(
    store: &Arc<Store>,
    scope: &str,
) -> Result<TenantId, StorageError> {
    resolve_or_register(store, &TenantScope::of(scope))
}
