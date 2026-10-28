//! Apply-time derivation.
//!
//! Derived rows (presence, value dictionary, counters, declared-index
//! entries) are computed by one deterministic [`Deriver`] from a commit's
//! *base* ops plus the state before that commit, and written in the same
//! backend batch at the same commit index. They are never logged in the WAL
//! and never enter a transaction's write set, so they cannot cause write
//! conflicts. Publication, command replay, and WAL replay (crash recovery,
//! PITR, replicas) all call [`Store::derive_ops`], so every node derives the
//! same rows from the same base log.

use std::collections::BTreeMap;
use std::ops::Bound;
use std::sync::Arc;

use crate::backend::StorageReadView;
use crate::read_view::{ReadLimits, ScanProjection};
use crate::{Op, StorageError, Store};

/// Read access to the state *before* a commit, for a [`Deriver`].
pub trait DeriveRead {
    /// Current value of `key`, or `None` when absent.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the underlying read fails.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;

    /// Every live `(key, value)` whose key starts with `prefix`, in key order.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the underlying read fails.
    fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError>;

    /// Every live `(key, value)` with `start <= key < end`, in key order. An
    /// empty `end` is unbounded above.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the underlying read fails.
    fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError>;
}

/// Rows of `map` in `[start, end)` (empty `end` = unbounded).
fn map_range<'a, V: 'a>(
    map: &'a BTreeMap<Vec<u8>, V>,
    start: &[u8],
    end: &[u8],
) -> impl Iterator<Item = (&'a Vec<u8>, &'a V)> {
    let upper = if end.is_empty() {
        Bound::Unbounded
    } else {
        Bound::Excluded(end.to_vec())
    };
    map.range::<Vec<u8>, _>((Bound::Included(start.to_vec()), upper))
}

/// Deterministic derivation of derived rows from a commit's base ops.
///
/// Implementations must be pure functions of `(ci, base_ops, before)`: no
/// clock, randomness, or global state, because replay and replicas call the
/// same function and must reach byte-identical state.
pub trait Deriver: Send + Sync {
    /// Materialized derived ops (final values, not deltas) for one commit.
    ///
    /// # Errors
    /// Returns [`StorageError`] when the base ops or the prior state are
    /// inconsistent; the commit (or replay) then fails.
    fn derive(
        &self,
        ci: u64,
        base_ops: &[Op],
        before: &dyn DeriveRead,
    ) -> Result<Vec<Op>, StorageError>;

    /// Whether `key` belongs to a derived family (so an overlay must derive
    /// its staged writes before answering a read of it).
    fn is_derived_key(&self, key: &[u8]) -> bool;

    /// Whether a scan of `prefix` can reach derived keys.
    fn prefix_reaches_derived(&self, prefix: &[u8]) -> bool;
}

impl DeriveRead for crate::ReadView {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        crate::ReadView::get(self, key)
    }

    fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        crate::ReadView::range_prefix(self, prefix)
    }

    fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        crate::ReadView::range(self, start, end)
    }
}

/// A read base plus a buffer of writes not yet visible in it (`None` =
/// tombstone): the state before a commit inside a publish group.
pub(crate) struct Layered<'a> {
    pub(crate) base: &'a dyn DeriveRead,
    pub(crate) buffer: &'a BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl DeriveRead for Layered<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        match self.buffer.get(key) {
            Some(value) => Ok(value.clone()),
            None => self.base.get(key),
        }
    }

    fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let mut merged: BTreeMap<Vec<u8>, Vec<u8>> =
            self.base.range_prefix(prefix)?.into_iter().collect();
        let upper = crate::prefix_range(prefix).1;
        let end = if upper.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(upper)
        };
        for (key, value) in self
            .buffer
            .range::<Vec<u8>, _>((Bound::Included(prefix.to_vec()), end))
        {
            match value {
                Some(value) => {
                    merged.insert(key.clone(), value.clone());
                }
                None => {
                    merged.remove(key);
                }
            }
        }
        Ok(merged.into_iter().collect())
    }

    fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let mut merged: BTreeMap<Vec<u8>, Vec<u8>> =
            self.base.range(start, end)?.into_iter().collect();
        for (key, value) in map_range(self.buffer, start, end) {
            match value {
                Some(value) => {
                    merged.insert(key.clone(), value.clone());
                }
                None => {
                    merged.remove(key);
                }
            }
        }
        Ok(merged.into_iter().collect())
    }
}

/// Fold ops into a layered buffer (history is irrelevant to derivation).
pub(crate) fn fold(buffer: &mut BTreeMap<Vec<u8>, Option<Vec<u8>>>, ops: &[Op]) {
    for op in ops {
        match op {
            Op::Put(key, value) | Op::Set(key, value) => {
                buffer.insert(key.clone(), Some(value.clone()));
            }
            Op::Del(key) | Op::Purge(key) => {
                buffer.insert(key.clone(), None);
            }
        }
    }
}

/// [`DeriveRead`] over one backend's current state.
pub(crate) struct BackendRead(pub(crate) Box<dyn StorageReadView>);

impl DeriveRead for BackendRead {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.0.read_value(
            key,
            u64::MAX,
            true,
            ReadLimits::default(),
            ScanProjection::KeyValue,
        )
    }

    fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let end = crate::prefix_range(prefix).1;
        self.0
            .current_range(
                prefix,
                &end,
                ReadLimits::default(),
                ScanProjection::KeyValue,
            )?
            .collect()
    }

    fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        self.0
            .current_range(start, end, ReadLimits::default(), ScanProjection::KeyValue)?
            .collect()
    }
}

/// [`DeriveRead`] over an in-memory key map (command-replay shadow state).
pub(crate) struct MapRead<'a>(pub(crate) &'a BTreeMap<Vec<u8>, Vec<u8>>);

impl DeriveRead for MapRead<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.0.get(key).cloned())
    }

    fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        Ok(self
            .0
            .range::<[u8], _>((Bound::Included(prefix), Bound::Unbounded))
            .take_while(|(key, _)| key.starts_with(prefix))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect())
    }

    fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        Ok(map_range(self.0, start, end)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect())
    }
}

/// [`DeriveRead`] over nothing (an empty prior state).
pub(crate) struct Empty;

impl DeriveRead for Empty {
    fn get(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(None)
    }

    fn range_prefix(&self, _prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        Ok(Vec::new())
    }

    fn range(&self, _start: &[u8], _end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        Ok(Vec::new())
    }
}

impl Store {
    /// Install the apply-time deriver. Install it before any commit or replay
    /// that must produce derived rows (see [`Store::open_with_deriver`]).
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn set_deriver(&self, deriver: Arc<dyn Deriver>) {
        // WAL records replayed at open without a deriver applied base rows
        // only; their derived rows are missing. Serving reads from that state
        // would be silently wrong, so fence the store (reopen with the
        // deriver installed, e.g. `rushdb_lmpg::open_store`).
        if self
            .underived_replay
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            self.health.fence();
        }
        *self.health.cleanup(self.deriver.write()) = Some(deriver);
    }

    /// **Fault injection for tests only.** Run `f` (typically one commit of
    /// malformed base rows) with apply-time derivation suspended, so a test
    /// can plant corruption the deriver would refuse and prove that readers
    /// fail on it. The deriver is restored afterwards; derived rows for the
    /// injected commit are intentionally absent.
    #[doc(hidden)]
    pub fn inject_without_derivation<T>(&self, f: impl FnOnce() -> T) -> T {
        let saved = self.health.cleanup(self.deriver.write()).take();
        let out = f();
        *self.health.cleanup(self.deriver.write()) = saved;
        out
    }

    pub(crate) fn deriver(&self) -> Option<Arc<dyn Deriver>> {
        self.health.cleanup(self.deriver.read()).clone()
    }

    /// Base ops followed by their derived ops, or `None` when no deriver is
    /// installed (the base ops are applied unchanged).
    pub(crate) fn derive_ops(
        deriver: Option<&Arc<dyn Deriver>>,
        ci: u64,
        base_ops: &[Op],
        before: &dyn DeriveRead,
    ) -> Result<Option<Vec<Op>>, StorageError> {
        let Some(deriver) = deriver else {
            return Ok(None);
        };
        let extra = deriver.derive(ci, base_ops, before)?;
        if extra.is_empty() {
            return Ok(None);
        }
        let mut all = Vec::with_capacity(base_ops.len() + extra.len());
        all.extend_from_slice(base_ops);
        all.extend(extra);
        Ok(Some(all))
    }
}
