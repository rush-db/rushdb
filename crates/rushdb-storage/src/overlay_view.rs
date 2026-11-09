//! Owned, read-buffer-first view over a pinned snapshot.
//!
//! An [`OverlayView`] layers uncommitted writes (`None` = tombstone) on top
//! of a [`ReadView`]. It is the single primitive behind transaction
//! read-your-writes (a statement sees earlier statements' staged writes) and
//! the sequencer's expansion layer (a command sees every assigned but not yet
//! published commit). Reads consult the staged writes first, then the pinned
//! base snapshot, so the overlay never blocks a reader and never mutates
//! committed state.
//!
//! Staged writes come in two tiers: shared, read-only [`CommitLayer`]s (one
//! per in-flight commit, oldest first) and the view's own mutable `buffer`
//! above them. A layer shares its commit's op vector with the publisher queue,
//! so handing the in-flight state to the next command copies no key or value.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;
use std::sync::Arc;

use crate::{Op, ReadCancellation, ReadView, ScanBounds, ScanProjection, StorageError};

fn op_entry(op: &Op) -> (&[u8], Option<&[u8]>) {
    match op {
        Op::Put(key, value) | Op::Set(key, value) => (key, Some(value)),
        Op::Del(key) | Op::Purge(key) => (key, None),
    }
}

/// The writes of one assigned-but-unpublished commit as a read layer: the
/// commit's ops, shared with the publisher queue, indexed by key (the last
/// write of each key wins).
pub(crate) struct CommitLayer {
    ci: u64,
    ops: Arc<Vec<Op>>,
    /// Indices into `ops`, ordered by key, one per distinct key.
    order: Vec<usize>,
}

impl CommitLayer {
    pub(crate) fn new(ci: u64, ops: Arc<Vec<Op>>) -> Self {
        let mut order: Vec<usize> = (0..ops.len()).collect();
        // Key ascending, then op index descending: the first index of each
        // key run is that key's last write, which `dedup_by` keeps.
        order.sort_unstable_by(|a, b| {
            op_entry(&ops[*a])
                .0
                .cmp(op_entry(&ops[*b]).0)
                .then(b.cmp(a))
        });
        order.dedup_by(|later, kept| op_entry(&ops[*later]).0 == op_entry(&ops[*kept]).0);
        Self { ci, ops, order }
    }

    pub(crate) fn ci(&self) -> u64 {
        self.ci
    }

    /// The op vector this layer shares.
    #[cfg(test)]
    pub(crate) fn ops(&self) -> &Arc<Vec<Op>> {
        &self.ops
    }

    /// Distinct keys written.
    pub(crate) fn len(&self) -> usize {
        self.order.len()
    }

    fn entry(&self, at: usize) -> (&[u8], Option<&[u8]>) {
        op_entry(&self.ops[self.order[at]])
    }

    /// `Some(None)` for a tombstone, `None` when the layer does not write
    /// `key`.
    #[expect(
        clippy::option_option,
        reason = "not written by this layer and written as a tombstone are different answers"
    )]
    pub(crate) fn get(&self, key: &[u8]) -> Option<Option<&[u8]>> {
        self.order
            .binary_search_by(|at| op_entry(&self.ops[*at]).0.cmp(key))
            .ok()
            .map(|at| op_entry(&self.ops[self.order[at]]).1)
    }

    /// Written keys in key order.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &[u8]> {
        (0..self.order.len()).map(|at| self.entry(at).0)
    }

    /// Entries with keys inside `(start, end)`, in key order.
    fn range<'a>(
        &'a self,
        start: Bound<&[u8]>,
        end: Bound<&[u8]>,
    ) -> impl Iterator<Item = (&'a [u8], Option<&'a [u8]>)> + 'a {
        let key = |at: &usize| op_entry(&self.ops[*at]).0;
        let from = match start {
            Bound::Included(start) => self.order.partition_point(|at| key(at) < start),
            Bound::Excluded(start) => self.order.partition_point(|at| key(at) <= start),
            Bound::Unbounded => 0,
        };
        let to = match end {
            Bound::Included(end) => self.order.partition_point(|at| key(at) <= end),
            Bound::Excluded(end) => self.order.partition_point(|at| key(at) < end),
            Bound::Unbounded => self.order.len(),
        };
        (from..to.max(from)).map(|at| self.entry(at))
    }
}

/// A pinned base snapshot plus uncommitted key overlays.
///
/// `Clone` is cheap for the base and the commit layers (`Arc`) and copies
/// the own buffer; callers that only read should share one instance behind
/// an `Arc`.
#[derive(Clone)]
pub struct OverlayView {
    base: Arc<ReadView>,
    /// In-flight commits' writes, oldest first, below `buffer`.
    layers: Vec<Arc<CommitLayer>>,
    buffer: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    // Derived rows implied by the staged writes, computed lazily on the first
    // read of a derived key and reset by every write, so a transaction's own
    // writes are visible to derived reads (T-5b for aggregates, DC12).
    derived: std::sync::OnceLock<Result<Arc<BTreeMap<Vec<u8>, Option<Vec<u8>>>>, StorageError>>,
}

impl std::fmt::Debug for OverlayView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayView")
            .field("commit_index", &self.base.commit_index())
            .field("layers", &self.layers.len())
            .field("overlay_keys", &self.buffer.len())
            .finish_non_exhaustive()
    }
}

impl OverlayView {
    /// Wrap a pinned snapshot with an empty overlay.
    #[must_use]
    pub fn new(base: Arc<ReadView>) -> Self {
        Self::with_buffer(base, BTreeMap::new())
    }

    /// Wrap a pinned snapshot with a pre-existing overlay buffer.
    #[must_use]
    pub fn with_buffer(base: Arc<ReadView>, buffer: BTreeMap<Vec<u8>, Option<Vec<u8>>>) -> Self {
        Self {
            base,
            layers: Vec::new(),
            buffer,
            derived: std::sync::OnceLock::new(),
        }
    }

    /// Wrap a pinned snapshot with in-flight commit layers (oldest first).
    pub(crate) fn with_layers(base: Arc<ReadView>, layers: Vec<Arc<CommitLayer>>) -> Self {
        Self {
            layers,
            ..Self::new(base)
        }
    }

    fn has_staged(&self) -> bool {
        !self.buffer.is_empty() || self.layers.iter().any(|layer| layer.len() > 0)
    }

    /// The staged state of `key`: the own buffer, then the layers newest
    /// first. `Some(None)` is a tombstone.
    #[expect(
        clippy::option_option,
        reason = "not staged and staged as a tombstone are different answers"
    )]
    fn staged(&self, key: &[u8]) -> Option<Option<&[u8]>> {
        if let Some(value) = self.buffer.get(key) {
            return Some(value.as_deref());
        }
        self.layers.iter().rev().find_map(|layer| layer.get(key))
    }

    /// Every staged write in `(start, end)`, newest winning, borrowed.
    fn staged_range(
        &self,
        start: Bound<&[u8]>,
        end: Bound<&[u8]>,
    ) -> BTreeMap<&[u8], Option<&[u8]>> {
        let mut merged = BTreeMap::new();
        for layer in &self.layers {
            merged.extend(layer.range(start, end));
        }
        merged.extend(
            self.buffer
                .range::<[u8], _>((start, end))
                .map(|(key, value)| (key.as_slice(), value.as_deref())),
        );
        merged
    }

    /// Every staged write in `(start, end)`, newest winning, as owned rows.
    pub(crate) fn staged_rows(
        &self,
        start: Bound<&[u8]>,
        end: Bound<&[u8]>,
    ) -> BTreeMap<Vec<u8>, Option<Vec<u8>>> {
        self.staged_range(start, end)
            .into_iter()
            .map(|(key, value)| (key.to_vec(), value.map(<[u8]>::to_vec)))
            .collect()
    }

    /// Derived rows of the staged writes against the base (lazy, cached until
    /// the next write).
    fn derived_rows(&self) -> Result<Arc<BTreeMap<Vec<u8>, Option<Vec<u8>>>>, StorageError> {
        self.derived
            .get_or_init(|| {
                let mut rows = BTreeMap::new();
                if let Some(deriver) = self.base.deriver() {
                    if self.has_staged() {
                        let ops: Vec<Op> = self
                            .staged_range(Bound::Unbounded, Bound::Unbounded)
                            .into_iter()
                            .map(|(key, value)| match value {
                                Some(value) => Op::Put(key.to_vec(), value.to_vec()),
                                None => Op::Del(key.to_vec()),
                            })
                            .collect();
                        for op in deriver.derive(0, &ops, &*self.base)? {
                            match op {
                                Op::Put(key, value) | Op::Set(key, value) => {
                                    rows.insert(key, Some(value));
                                }
                                Op::Del(key) | Op::Purge(key) => {
                                    rows.insert(key, None);
                                }
                            }
                        }
                    }
                }
                Ok(Arc::new(rows))
            })
            .clone()
    }

    fn reaches_derived(&self, prefix: &[u8]) -> bool {
        self.has_staged()
            && self
                .base
                .deriver()
                .is_some_and(|deriver| deriver.prefix_reaches_derived(prefix))
    }

    /// The pinned base snapshot underneath this overlay.
    #[must_use]
    pub fn base(&self) -> &Arc<ReadView> {
        &self.base
    }

    /// Commit index this view reads its committed base at.
    #[must_use]
    pub fn commit_index(&self) -> u64 {
        self.base.commit_index()
    }

    /// Cancellation handle shared with the base snapshot.
    #[must_use]
    pub fn cancellation(&self) -> ReadCancellation {
        self.base.cancellation()
    }

    /// Stage a write in the overlay.
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.derived = std::sync::OnceLock::new();
        self.buffer.insert(key, Some(value));
    }

    /// Stage a tombstone in the overlay.
    pub fn delete(&mut self, key: Vec<u8>) {
        self.derived = std::sync::OnceLock::new();
        self.buffer.insert(key, None);
    }

    /// Fold a batch of LMPG ops into the overlay. History is irrelevant to
    /// reads, so `Put`/`Set` become live values and `Del`/`Purge` tombstones.
    pub fn apply_ops(&mut self, ops: &[Op]) {
        for op in ops {
            match op {
                Op::Put(k, v) | Op::Set(k, v) => {
                    self.buffer.insert(k.clone(), Some(v.clone()));
                }
                Op::Del(k) | Op::Purge(k) => {
                    self.buffer.insert(k.clone(), None);
                }
            }
        }
        self.derived = std::sync::OnceLock::new();
    }

    /// Read one key: overlay first, then the pinned base.
    ///
    /// # Errors
    /// Returns the underlying storage error from the base read.
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.with_value(key, |value| value.map(<[u8]>::to_vec))
    }

    /// Read one key without copying it ([`ReadView::with_value`]): `f` sees
    /// the staged value (or `None` for a staged delete), a derived row of the
    /// staged writes, or else the pinned base's value in place.
    ///
    /// # Errors
    /// Returns the underlying storage error from the base read.
    pub fn with_value<R>(
        &self,
        key: &[u8],
        f: impl FnOnce(Option<&[u8]>) -> R,
    ) -> Result<R, StorageError> {
        if let Some(value) = self.staged(key) {
            return Ok(f(value));
        }
        if self.reaches_derived(key)
            && self
                .base
                .deriver()
                .is_some_and(|deriver| deriver.is_derived_key(key))
        {
            if let Some(value) = self.derived_rows()?.get(key) {
                return Ok(f(value.as_deref()));
            }
        }
        self.base.with_value(key, f)
    }

    /// Materialize every key/value with `key` starting with `prefix`, merging
    /// the overlay over the base (overlay wins; tombstones hide base rows).
    ///
    /// # Errors
    /// Returns the underlying storage error from the base scan.
    pub fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let merged = self.base.range_prefix(prefix)?.into_iter().collect();
        let upper = crate::prefix_range(prefix).1;
        let end = if upper.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(upper.as_slice())
        };
        let derived = if self.reaches_derived(prefix) {
            Some(self.derived_rows()?)
        } else {
            None
        };
        Ok(self.merge_staged(merged, prefix, end, derived.as_deref()))
    }

    /// `merged` (base rows) overlaid with the staged writes and then the
    /// derived rows in `[start, end)`; tombstones remove rows.
    fn merge_staged(
        &self,
        mut merged: BTreeMap<Vec<u8>, Vec<u8>>,
        start: &[u8],
        end: Bound<&[u8]>,
        derived: Option<&BTreeMap<Vec<u8>, Option<Vec<u8>>>>,
    ) -> Vec<(Vec<u8>, Vec<u8>)> {
        let start = Bound::Included(start);
        let derived_rows = derived.into_iter().flat_map(|rows| {
            rows.range::<[u8], _>((start, end))
                .map(|(key, value)| (key.as_slice(), value.as_deref()))
        });
        for (key, value) in self
            .staged_range(start, end)
            .into_iter()
            .chain(derived_rows)
        {
            match value {
                Some(value) => {
                    merged.insert(key.to_vec(), value.to_vec());
                }
                None => {
                    merged.remove(key);
                }
            }
        }
        merged.into_iter().collect()
    }

    /// Materialize the keys with `start <= key < end`, merging the overlay over
    /// the base (overlay wins; tombstones hidden). Values are never read, so a
    /// keys-only scan does not pay for large payloads.
    ///
    /// # Errors
    /// Returns the underlying storage error from the base scan.
    pub fn range_keys(&self, start: &[u8], end: &[u8]) -> Result<Vec<Vec<u8>>, StorageError> {
        let bounds = ScanBounds {
            start: Bound::Included(start.to_vec()),
            end: if end.is_empty() {
                Bound::Unbounded
            } else {
                Bound::Excluded(end.to_vec())
            },
        };
        let mut keys: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut cursor = self.base.scan(bounds, ScanProjection::Keys)?;
        loop {
            let batch = cursor.next_batch(1024, 8 * 1024 * 1024)?;
            for row in batch.rows {
                keys.insert(row.key);
            }
            if batch.done {
                break;
            }
        }
        let end_bound = if end.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(end)
        };
        for (key, value) in self.staged_range(Bound::Included(start), end_bound) {
            match value {
                Some(_) => {
                    keys.insert(key.to_vec());
                }
                None => {
                    keys.remove(key);
                }
            }
        }
        Ok(keys.into_iter().collect())
    }

    /// Materialize every key/value with `start <= key < end`, merging the
    /// overlay over the base. An empty `end` is unbounded above.
    ///
    /// # Errors
    /// Returns the underlying storage error from the base scan.
    pub fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let merged = self.base.range(start, end)?.into_iter().collect();
        let end_bound = if end.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(end)
        };
        let common = start
            .iter()
            .zip(end.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let derived = if self.reaches_derived(&start[..common]) {
            Some(self.derived_rows()?)
        } else {
            None
        };
        Ok(self.merge_staged(merged, start, end_bound, derived.as_deref()))
    }
}
