//! Per-tenant logical byte accounting.
//!
//! Every backend write already reads the value it replaces (for MVCC history),
//! so the change in a tenant's stored `key + value` bytes is known at apply
//! time at no extra read. A backend folds each op of a batch into
//! [`UsageBytes`] and writes one [`usage_bytes_key`] row per touched tenant in
//! the same backend transaction, so the figure is exact at every commit index
//! and replay (WAL, replicas) reproduces it.
//!
//! The figure is *logical*: it sums the row bytes the engine stores, not the
//! file pages that hold them (redb pages run partly full, so the file is
//! larger). The usage row itself is excluded, which keeps it a fixed point.

use std::collections::BTreeMap;

use rushdb_types::ids::TenantId;
use rushdb_types::keys::{parse_tenant, usage_bytes_key, F_USAGE_BYTES, TENANT_PREFIX_LEN};

/// Whether `key` is a tenant's usage-bytes row.
#[must_use]
pub fn is_usage_bytes_key(key: &[u8]) -> bool {
    key.len() == TENANT_PREFIX_LEN + 1
        && parse_tenant(key).is_some()
        && key[TENANT_PREFIX_LEN] == F_USAGE_BYTES
}

/// Byte deltas of one batch, per tenant.
#[derive(Default)]
pub(crate) struct UsageBytes {
    deltas: BTreeMap<TenantId, i64>,
}

impl UsageBytes {
    /// Record that `key` went from a value of `old` bytes to one of `new`
    /// bytes (`None` = absent).
    pub(crate) fn record(&mut self, key: &[u8], old: Option<usize>, new: Option<usize>) {
        let Some(tenant) = parse_tenant(key) else {
            return;
        };
        if is_usage_bytes_key(key) {
            return;
        }
        let size = |value: Option<usize>| {
            value.map_or(0, |len| i64::try_from(key.len() + len).unwrap_or(i64::MAX))
        };
        let delta = size(new) - size(old);
        if delta != 0 {
            *self.deltas.entry(tenant).or_default() += delta;
        }
    }

    /// The usage rows to write: for each tenant, its new total from `current`
    /// (the row's value before the batch). A total of zero deletes the row.
    ///
    /// # Errors
    /// Returns a message when a stored total is malformed or the delta would
    /// take it below zero, which means the accounting has diverged.
    pub(crate) fn rows(
        self,
        mut current: impl FnMut(&[u8]) -> Result<Option<Vec<u8>>, String>,
    ) -> Result<Vec<(Vec<u8>, Option<Vec<u8>>)>, String> {
        let mut out = Vec::with_capacity(self.deltas.len());
        for (tenant, delta) in self.deltas {
            if delta == 0 {
                continue;
            }
            let key = usage_bytes_key(tenant);
            let total = rushdb_types::keys::decode_counter(current(&key)?.as_deref())
                .ok_or_else(|| format!("tenant {tenant}: malformed usage-bytes row"))?;
            let next = total
                .checked_add_signed(delta)
                .ok_or_else(|| format!("tenant {tenant}: usage bytes would underflow"))?;
            out.push((key, (next > 0).then(|| next.to_be_bytes().to_vec())));
        }
        Ok(out)
    }
}
