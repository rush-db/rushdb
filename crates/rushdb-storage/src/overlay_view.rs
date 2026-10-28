//! Owned, read-buffer-first view over a pinned snapshot.
//!
//! An [`OverlayView`] layers an uncommitted write buffer (`None` = tombstone)
//! on top of a [`ReadView`]. It is the single primitive behind transaction
//! read-your-writes (a statement sees earlier statements' staged writes) and,
//! later, the sequencer's speculative expansion layer. Reads consult the buffer
//! first, then the pinned base snapshot, so the overlay never blocks a reader
//! and never mutates committed state.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;
use std::sync::Arc;

use crate::{Op, ReadCancellation, ReadView, ScanBounds, ScanProjection, StorageError};

/// A pinned base snapshot plus an uncommitted key overlay.
///
/// `Clone` is cheap for the base (`Arc`) and copies the overlay map; callers
/// that only read should share one instance behind an `Arc`.
#[derive(Clone)]
pub struct OverlayView {
    base: Arc<ReadView>,
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
            .field("overlay_keys", &self.buffer.len())
            .finish_non_exhaustive()
    }
}

impl OverlayView {
    /// Wrap a pinned snapshot with an empty overlay.
    #[must_use]
    pub fn new(base: Arc<ReadView>) -> Self {
        Self {
            base,
            buffer: BTreeMap::new(),
            derived: std::sync::OnceLock::new(),
        }
    }

    /// Wrap a pinned snapshot with a pre-existing overlay buffer.
    #[must_use]
    pub fn with_buffer(base: Arc<ReadView>, buffer: BTreeMap<Vec<u8>, Option<Vec<u8>>>) -> Self {
        Self {
            base,
            buffer,
            derived: std::sync::OnceLock::new(),
        }
    }

    /// Derived rows of the staged writes against the base (lazy, cached until
    /// the next write).
    fn derived_rows(&self) -> Result<Arc<BTreeMap<Vec<u8>, Option<Vec<u8>>>>, StorageError> {
        self.derived
            .get_or_init(|| {
                let mut rows = BTreeMap::new();
                if let Some(deriver) = self.base.deriver() {
                    if !self.buffer.is_empty() {
                        let ops: Vec<Op> = self
                            .buffer
                            .iter()
                            .map(|(key, value)| match value {
                                Some(value) => Op::Put(key.clone(), value.clone()),
                                None => Op::Del(key.clone()),
                            })
                            .collect();
                        crate::derive::fold(&mut rows, &deriver.derive(0, &ops, &*self.base)?);
                    }
                }
                Ok(Arc::new(rows))
            })
            .clone()
    }

    fn reaches_derived(&self, prefix: &[u8]) -> bool {
        !self.buffer.is_empty()
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

    /// The current overlay buffer (read-buffer-first).
    #[must_use]
    pub fn buffer(&self) -> &BTreeMap<Vec<u8>, Option<Vec<u8>>> {
        &self.buffer
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
        if let Some(value) = self.buffer.get(key) {
            return Ok(value.clone());
        }
        if self.reaches_derived(key)
            && self
                .base
                .deriver()
                .is_some_and(|deriver| deriver.is_derived_key(key))
        {
            if let Some(value) = self.derived_rows()?.get(key) {
                return Ok(value.clone());
            }
        }
        self.base.get(key)
    }

    /// Materialize every key/value with `key` starting with `prefix`, merging
    /// the overlay over the base (overlay wins; tombstones hide base rows).
    ///
    /// # Errors
    /// Returns the underlying storage error from the base scan.
    pub fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let mut merged: BTreeMap<Vec<u8>, Vec<u8>> =
            self.base.range_prefix(prefix)?.into_iter().collect();
        let derived = if self.reaches_derived(prefix) {
            Some(self.derived_rows()?)
        } else {
            None
        };
        for (key, value) in self
            .buffer
            .range::<[u8], _>((Bound::Included(prefix), Bound::Unbounded))
            .chain(derived.iter().flat_map(|rows| {
                rows.range::<[u8], _>((Bound::Included(prefix), Bound::Unbounded))
            }))
        {
            if !key.starts_with(prefix) {
                continue;
            }
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
        for (key, value) in self
            .buffer
            .range::<[u8], _>((Bound::Included(start), end_bound))
        {
            match value {
                Some(_) => {
                    keys.insert(key.clone());
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
        let mut merged: BTreeMap<Vec<u8>, Vec<u8>> =
            self.base.range(start, end)?.into_iter().collect();
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
        for (key, value) in self
            .buffer
            .range::<[u8], _>((Bound::Included(start), end_bound))
            .chain(
                derived
                    .iter()
                    .flat_map(|rows| rows.range::<[u8], _>((Bound::Included(start), end_bound))),
            )
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
}
