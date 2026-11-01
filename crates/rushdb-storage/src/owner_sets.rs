//! Owner sets of declared-index leaves, cached per snapshot (READ-2).
//!
//! A page over an indexed filter used to read every index entry of each
//! leaf on every request: millions of entries for a broad value on a large
//! tenant. The set a leaf resolves to is a function of the snapshot's commit
//! index, so it is kept here under `(key, commit index)` and reused by every
//! reader of that snapshot; a commit moves readers to a new index and the
//! older entries age out by LRU. Charged to the logical cache budget; a set
//! that does not fit is simply not kept.

use std::any::Any;
use std::sync::{Arc, Mutex};

use crate::cache::LogicalCache;
use crate::resources::ByteBudget;
use crate::StorageError;

/// A cached owner set: opaque to storage (the query layer's set type).
pub type OwnerSet = Arc<dyn Any + Send + Sync>;

/// Bytes the cache may hold, a child of the logical cache budget.
const OWNER_SET_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// The store's per-snapshot owner-set cache.
pub struct OwnerSets {
    cache: Mutex<Option<LogicalCache<(Vec<u8>, u64), OwnerSet>>>,
}

impl OwnerSets {
    /// Build under `parent` (the logical cache budget); without one there is
    /// no cache and every lookup misses.
    ///
    /// # Errors
    /// Returns [`StorageError::InvalidConfiguration`] if the budget cannot be
    /// created.
    pub(crate) fn new(parent: Option<&ByteBudget>) -> Result<Self, StorageError> {
        let invalid =
            |e: crate::resources::ResourceError| StorageError::InvalidConfiguration(e.to_string());
        let cache = match parent {
            Some(parent) => {
                let bytes = OWNER_SET_CACHE_BYTES.min(parent.snapshot().limit);
                let budget = parent.child("owner-sets", bytes).map_err(invalid)?;
                Some(LogicalCache::new(budget).map_err(invalid)?)
            }
            None => None,
        };
        Ok(Self {
            cache: Mutex::new(cache),
        })
    }

    /// The set cached for `key` at commit index `ci`.
    #[must_use]
    pub fn get(&self, key: &[u8], ci: u64) -> Option<OwnerSet> {
        let mut guard = self.cache.lock().ok()?;
        guard.as_mut()?.get(&(key.to_vec(), ci)).cloned()
    }

    /// Keep `set` (about `bytes` resident) for `key` at `ci`.
    pub fn insert(&self, key: Vec<u8>, ci: u64, set: OwnerSet, bytes: usize) {
        if let Ok(mut guard) = self.cache.lock() {
            if let Some(cache) = guard.as_mut() {
                // Sets of older snapshots are only read by readers still
                // pinned there; the newest one is what later reads use.
                cache.retain(|(_, at), _| *at >= ci);
                cache.insert((key, ci), set, bytes);
            }
        }
    }
}
