//! Owner sets of declared-index leaves, cached across commits (READ-2).
//!
//! A page over an indexed filter used to read every index entry of each
//! leaf on every request: millions of entries for a broad value on a large
//! tenant. The set a leaf resolves to depends only on the entries of the
//! indexes it reads, so it is kept under the leaf's key together with the
//! version of those indexes: the commit index of the last commit that wrote
//! one of their entries. The publisher records that version before a commit
//! becomes visible. A set stays valid across every commit that does not
//! write those indexes (other tenants, other properties), and a reader uses
//! it when no newer write to them has been published and its snapshot
//! includes the version. Charged to the logical cache budget; a set that
//! does not fit is simply not kept, and stale sets age out by LRU.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rushdb_types::ids::TenantId;

use crate::cache::LogicalCache;
use crate::resources::ByteBudget;
use crate::StorageError;

/// A cached owner set: opaque to storage (the query layer's set type).
pub type OwnerSet = Arc<dyn Any + Send + Sync>;

/// One declared index of one tenant.
pub type IndexRef = (TenantId, u32);

/// Bytes the cache may hold, a child of the logical cache budget.
const OWNER_SET_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// The store's owner-set cache.
pub struct OwnerSets {
    cache: Mutex<Option<LogicalCache<Vec<u8>, (u64, OwnerSet)>>>,
    /// Commit index of the last published write to each index's entries.
    versions: Mutex<HashMap<IndexRef, u64>>,
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
            versions: Mutex::new(HashMap::new()),
        })
    }

    /// Record that commit `ci` wrote entries of `indexes`. Called by the
    /// publisher before `ci` becomes visible to readers.
    pub(crate) fn note_writes(&self, ci: u64, indexes: impl IntoIterator<Item = IndexRef>) {
        let mut versions = self
            .versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for index in indexes {
            versions.insert(index, ci);
        }
    }

    /// The version of `indexes`: the last commit that wrote any of their
    /// entries (0 when none has since the store opened).
    #[must_use]
    pub fn version(&self, indexes: &[IndexRef]) -> u64 {
        let versions = self
            .versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        indexes
            .iter()
            .filter_map(|index| versions.get(index).copied())
            .max()
            .unwrap_or(0)
    }

    /// The set cached for `key` over `indexes`, if it holds for a reader at
    /// snapshot `ci`: no write to `indexes` has been published since the set
    /// was computed, and the snapshot includes the last one before it.
    #[must_use]
    pub fn get(&self, key: &[u8], indexes: &[IndexRef], ci: u64) -> Option<OwnerSet> {
        let current = self.version(indexes);
        let mut guard = self.cache.lock().ok()?;
        let (version, set) = guard.as_mut()?.get(&key.to_vec())?;
        (*version == current && ci >= *version).then(|| Arc::clone(set))
    }

    /// Keep `set` (about `bytes` resident) for `key`, computed by a reader at
    /// snapshot `ci` when the indexes' version was `version`. A set computed
    /// at a snapshot that does not include `version` is not kept: it may
    /// predate that write.
    pub fn insert(&self, key: Vec<u8>, version: u64, ci: u64, set: OwnerSet, bytes: usize) {
        if ci < version {
            return;
        }
        if let Ok(mut guard) = self.cache.lock() {
            if let Some(cache) = guard.as_mut() {
                cache.insert(key, (version, set), bytes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sets() -> OwnerSets {
        let budget = ByteBudget::new("test", 1 << 20).unwrap();
        OwnerSets::new(Some(&budget)).unwrap()
    }

    fn set(n: u64) -> OwnerSet {
        Arc::new(n)
    }

    fn value(hit: Option<OwnerSet>) -> Option<u64> {
        hit.and_then(|set| set.downcast_ref::<u64>().copied())
    }

    #[test]
    fn a_set_survives_writes_to_other_indexes_and_other_tenants() {
        let sets = sets();
        let mine = [(TenantId(1), 7)];
        sets.note_writes(5, mine);
        sets.insert(b"leaf".to_vec(), sets.version(&mine), 9, set(1), 8);
        // Commits 10..=12 write another index of the tenant and another tenant.
        sets.note_writes(10, [(TenantId(1), 8)]);
        sets.note_writes(12, [(TenantId(2), 7)]);
        assert_eq!(value(sets.get(b"leaf", &mine, 12)), Some(1));
        // A reader still at an older snapshot that includes version 5 too.
        assert_eq!(value(sets.get(b"leaf", &mine, 6)), Some(1));
        // One before the version does not see the set's state.
        assert_eq!(value(sets.get(b"leaf", &mine, 4)), None);
    }

    #[test]
    fn a_write_to_the_index_invalidates_its_sets() {
        let sets = sets();
        let mine = [(TenantId(1), 7)];
        sets.insert(b"leaf".to_vec(), sets.version(&mine), 3, set(1), 8);
        assert_eq!(value(sets.get(b"leaf", &mine, 3)), Some(1));
        sets.note_writes(4, mine);
        assert_eq!(value(sets.get(b"leaf", &mine, 4)), None);
        assert_eq!(
            value(sets.get(b"leaf", &mine, 3)),
            None,
            "even for older readers"
        );
    }

    #[test]
    fn a_set_computed_before_the_current_version_is_not_kept() {
        let sets = sets();
        let mine = [(TenantId(1), 7)];
        sets.note_writes(8, mine);
        // A reader at snapshot 5 computed its set after commit 8 published.
        sets.insert(b"leaf".to_vec(), sets.version(&mine), 5, set(1), 8);
        assert_eq!(value(sets.get(b"leaf", &mine, 9)), None);
    }
}
