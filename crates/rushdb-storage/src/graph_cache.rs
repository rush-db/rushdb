//! Resident adjacency of one relationship type, cached per snapshot.
//!
//! A traversal reads each record's adjacency rows with one range scan per
//! record and direction. When traversals at one snapshot keep reading the
//! same relationships, holding them in memory is cheaper: the graph a type
//! resolves to is a function of the snapshot's commit index, so it is kept
//! under `(key, commit index)` and shared by every reader of that snapshot,
//! like the owner-set cache. A commit moves readers to a new index; older
//! graphs age out by LRU.
//!
//! **When to build** is a rent-or-buy decision. Building reads every
//! adjacency row of the tenant once (the rows are keyed by record, not by
//! type). Traversals report the rows they read through storage for a key at
//! a commit index ([`GraphCache::rent`]); once that reaches the cost of one
//! build, the next traversal builds and keeps the graph. A read-mostly
//! tenant pays at most twice the build; a tenant whose commits keep moving
//! the snapshot never builds, because its rent restarts at every commit.
//!
//! Charged to its own budget (`graph-cache-bytes`) under the managed limit;
//! a graph that does not fit is simply not kept, and traversals read
//! storage as before.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::cache::LogicalCache;
use crate::resources::ByteBudget;
use crate::StorageError;

/// A cached graph: opaque to storage (the query layer's adjacency type).
pub type Graph = Arc<dyn Any + Send + Sync>;

/// The store's per-snapshot adjacency cache.
pub struct GraphCache {
    cache: Mutex<Option<LogicalCache<(Vec<u8>, u64), Graph>>>,
    /// Rows read through storage per key, at the commit index they were
    /// read at.
    rent: Mutex<HashMap<Vec<u8>, (u64, u64)>>,
}

impl GraphCache {
    /// Build under `budget` (`graph-cache-bytes`); without one there is no
    /// cache: every lookup misses and nothing is ever built.
    ///
    /// # Errors
    /// Returns [`StorageError::InvalidConfiguration`] if the cache cannot
    /// reserve its budget.
    pub(crate) fn new(budget: Option<ByteBudget>) -> Result<Self, StorageError> {
        let cache = budget
            .map(LogicalCache::new)
            .transpose()
            .map_err(|e| StorageError::InvalidConfiguration(e.to_string()))?;
        Ok(Self {
            cache: Mutex::new(cache),
            rent: Mutex::new(HashMap::new()),
        })
    }

    /// Whether graphs can be kept at all.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.cache.lock().is_ok_and(|cache| cache.is_some())
    }

    /// The graph cached for `key` at commit index `ci`.
    #[must_use]
    pub fn get(&self, key: &[u8], ci: u64) -> Option<Graph> {
        let mut guard = self.cache.lock().ok()?;
        guard.as_mut()?.get(&(key.to_vec(), ci)).cloned()
    }

    /// Keep `graph` (about `bytes` resident) for `key` at `ci`. Returns
    /// whether it is now resident.
    pub fn insert(&self, key: Vec<u8>, ci: u64, graph: Graph, bytes: usize) -> bool {
        let Ok(mut guard) = self.cache.lock() else {
            return false;
        };
        let Some(cache) = guard.as_mut() else {
            return false;
        };
        // Graphs of older snapshots serve only readers still pinned there;
        // the newest one is what later traversals use.
        cache.retain(|(_, at), _| *at >= ci);
        if let Ok(mut rent) = self.rent.lock() {
            rent.remove(&key);
        }
        cache.insert((key, ci), graph, bytes)
    }

    /// Record `rows` adjacency rows read through storage for `key` at `ci`,
    /// and whether the rows read at `ci` now pay for one build of
    /// `build_rows` rows.
    pub fn rent(&self, key: &[u8], ci: u64, rows: u64, build_rows: u64) -> bool {
        if !self.enabled() {
            return false;
        }
        let Ok(mut rent) = self.rent.lock() else {
            return false;
        };
        // Rent paid at an older snapshot buys nothing at this one.
        rent.retain(|_, (at, _)| *at >= ci);
        let spent = rent.entry(key.to_vec()).or_insert((ci, 0));
        if spent.0 != ci {
            *spent = (ci, 0);
        }
        spent.1 = spent.1.saturating_add(rows);
        spent.1 >= build_rows
    }
}
