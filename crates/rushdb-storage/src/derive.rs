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
pub trait DeriveRead: Sync {
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

/// A read base plus the writes of earlier commits of the same publish group
/// not yet visible in it (`None` = tombstone): the state before a commit
/// inside the group. The buffer borrows the group's ops instead of copying
/// them.
pub(crate) struct Layered<'a, 'b> {
    pub(crate) base: &'a dyn DeriveRead,
    pub(crate) buffer: &'a BTreeMap<&'b [u8], Option<&'b [u8]>>,
}

impl Layered<'_, '_> {
    fn overlay(
        &self,
        mut merged: BTreeMap<Vec<u8>, Vec<u8>>,
        start: &[u8],
        end: Bound<&[u8]>,
    ) -> Vec<(Vec<u8>, Vec<u8>)> {
        for (key, value) in self.buffer.range::<[u8], _>((Bound::Included(start), end)) {
            match value {
                Some(value) => {
                    merged.insert(key.to_vec(), value.to_vec());
                }
                None => {
                    merged.remove(*key);
                }
            }
        }
        merged.into_iter().collect()
    }
}

impl DeriveRead for Layered<'_, '_> {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        match self.buffer.get(key) {
            Some(value) => Ok(value.map(<[u8]>::to_vec)),
            None => self.base.get(key),
        }
    }

    fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let merged = self.base.range_prefix(prefix)?.into_iter().collect();
        let upper = crate::prefix_range(prefix).1;
        let end = if upper.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(upper.as_slice())
        };
        Ok(self.overlay(merged, prefix, end))
    }

    fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let merged = self.base.range(start, end)?.into_iter().collect();
        let end = if end.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(end)
        };
        Ok(self.overlay(merged, start, end))
    }
}

/// Fold ops into a borrowed layered buffer (history is irrelevant to
/// derivation).
pub(crate) fn fold<'b>(
    buffer: &mut BTreeMap<&'b [u8], Option<&'b [u8]>>,
    ops: impl IntoIterator<Item = &'b Op>,
) {
    for op in ops {
        match op {
            Op::Put(key, value) | Op::Set(key, value) => {
                buffer.insert(key, Some(value));
            }
            Op::Del(key) | Op::Purge(key) => {
                buffer.insert(key, None);
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

    /// The ops derived from `base_ops` (applied after them, at the same
    /// commit index), or none when no deriver is installed. The base ops are
    /// never copied: callers apply the two slices together
    /// ([`crate::backend::CommitOps`]).
    pub(crate) fn derive_ops(
        deriver: Option<&Arc<dyn Deriver>>,
        ci: u64,
        base_ops: &[Op],
        before: &dyn DeriveRead,
    ) -> Result<Vec<Op>, StorageError> {
        match deriver {
            Some(deriver) => deriver.derive(ci, base_ops, before),
            None => Ok(Vec::new()),
        }
    }
}
