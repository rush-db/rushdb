//! Byte-accounted logical caches.
//!
//! A bounded, LRU-evicting key/value cache whose resident bytes are charged to
//! the shared [`ByteBudget`] tree. Unlike an unbounded `HashMap`, growth is
//! visible to the resource governor and a cache can never exceed its budget:
//! `insert` evicts least-recently-used entries until the new entry fits, and
//! skips caching a single entry larger than the whole budget.
//!
//! This is the shared mechanism behind the decoded-record, adjacency, and
//! property-dictionary caches; callers supply each entry's byte weight so the
//! accounting matches what they actually retain.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use crate::resources::{ByteBudget, ByteReservation, ResourceError};

/// Point-in-time cache occupancy and hit-rate counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheStats {
    /// Number of live entries.
    pub entries: u64,
    /// Resident bytes charged to the budget.
    pub bytes: u64,
    /// Successful lookups.
    pub hits: u64,
    /// Lookups that missed.
    pub misses: u64,
    /// Entries evicted to stay within budget.
    pub evictions: u64,
}

struct Entry<V> {
    value: V,
    bytes: usize,
    seq: u64,
}

/// A bounded logical cache charged to a [`ByteBudget`].
pub struct LogicalCache<K, V> {
    budget: ByteBudget,
    reserved: ByteReservation,
    entries: HashMap<K, Entry<V>>,
    /// LRU order: monotonically increasing sequence -> key. The smallest
    /// sequence is the least-recently-used entry.
    order: BTreeMap<u64, K>,
    seq: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl<K, V> LogicalCache<K, V>
where
    K: Eq + Hash + Clone,
{
    /// Build a cache charged to `budget`. A zero-limit budget makes every
    /// `insert` a no-op (the cache is effectively disabled).
    ///
    /// # Errors
    /// Returns [`ResourceError`] if the initial zero-byte reservation cannot be
    /// created (only when the budget is already over its limit).
    pub fn new(budget: ByteBudget) -> Result<Self, ResourceError> {
        let reserved = budget.try_reserve(0)?;
        Ok(Self {
            budget,
            reserved,
            entries: HashMap::new(),
            order: BTreeMap::new(),
            seq: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        })
    }

    fn next_seq(&mut self) -> u64 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// Configured byte limit for this cache.
    #[must_use]
    pub fn limit(&self) -> usize {
        self.budget.snapshot().limit
    }

    /// Look up `key`, marking it most-recently-used on a hit.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let seq = self.next_seq();
        let Some(entry) = self.entries.get_mut(key) else {
            self.misses = self.misses.saturating_add(1);
            return None;
        };
        self.hits = self.hits.saturating_add(1);
        self.order.remove(&entry.seq);
        entry.seq = seq;
        self.order.insert(seq, key.clone());
        self.entries.get(key).map(|entry| &entry.value)
    }

    /// Insert or replace an entry whose retained size is `bytes`. Evicts LRU
    /// entries until it fits; a value larger than the whole budget is not
    /// cached. Returns whether the entry is now resident.
    pub fn insert(&mut self, key: K, value: V, bytes: usize) -> bool {
        // Never evict live entries to make room for one that cannot fit.
        if bytes > self.limit() {
            return false;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.order.remove(&old.seq);
            let _ = self
                .reserved
                .resize(self.reserved.bytes().saturating_sub(old.bytes));
        }
        while self.reserved.bytes().saturating_add(bytes) > self.limit() {
            if !self.evict_one() {
                return false;
            }
        }
        if self
            .reserved
            .resize(self.reserved.bytes().saturating_add(bytes))
            .is_err()
        {
            return false;
        }
        let seq = self.next_seq();
        self.order.insert(seq, key.clone());
        self.entries.insert(key, Entry { value, bytes, seq });
        true
    }

    /// Remove the least-recently-used entry. Returns false when empty.
    fn evict_one(&mut self) -> bool {
        let Some((&seq, key)) = self.order.iter().next() else {
            return false;
        };
        let key = key.clone();
        self.order.remove(&seq);
        if let Some(entry) = self.entries.remove(&key) {
            let _ = self
                .reserved
                .resize(self.reserved.bytes().saturating_sub(entry.bytes));
            self.evictions = self.evictions.saturating_add(1);
            return true;
        }
        false
    }

    /// Drop every entry `keep` rejects, releasing its bytes (counted as
    /// evictions). Returns how many entries were dropped.
    pub fn retain(&mut self, mut keep: impl FnMut(&K, &V) -> bool) -> usize {
        let doomed: Vec<K> = self
            .entries
            .iter()
            .filter(|(key, entry)| !keep(key, &entry.value))
            .map(|(key, _)| key.clone())
            .collect();
        for key in &doomed {
            if let Some(entry) = self.entries.remove(key) {
                self.order.remove(&entry.seq);
                let _ = self
                    .reserved
                    .resize(self.reserved.bytes().saturating_sub(entry.bytes));
                self.evictions = self.evictions.saturating_add(1);
            }
        }
        doomed.len()
    }

    /// Drop every entry, releasing all charged bytes.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        let _ = self.reserved.resize(0);
    }

    /// Occupancy and counters.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len() as u64,
            bytes: self.reserved.bytes() as u64,
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(limit: usize) -> LogicalCache<u64, String> {
        LogicalCache::new(ByteBudget::new("test-cache", limit).unwrap()).unwrap()
    }

    #[test]
    fn evicts_least_recently_used_when_over_budget() {
        let mut c = cache(100);
        assert!(c.insert(1, "a".into(), 40));
        assert!(c.insert(2, "b".into(), 40));
        // Touch 1 so 2 is the LRU.
        assert_eq!(c.get(&1).map(String::as_str), Some("a"));
        assert!(c.insert(3, "c".into(), 40)); // evicts 2
        assert!(c.get(&2).is_none(), "LRU evicted");
        assert_eq!(c.get(&1).map(String::as_str), Some("a"));
        assert_eq!(c.get(&3).map(String::as_str), Some("c"));
        let s = c.stats();
        assert_eq!(s.entries, 2);
        assert_eq!(s.bytes, 80);
        assert_eq!(s.evictions, 1);
        assert_eq!(s.hits, 3); // 1,1,3
        assert_eq!(s.misses, 1);
    }

    #[test]
    fn oversized_entry_is_not_cached_and_releases_nothing() {
        let mut c = cache(100);
        assert!(c.insert(1, "a".into(), 40));
        assert!(!c.insert(2, "huge".into(), 500), "oversized rejected");
        assert!(c.get(&2).is_none());
        assert_eq!(c.stats().bytes, 40, "failed insert charges nothing");
        assert_eq!(c.get(&1).map(String::as_str), Some("a"));
    }

    #[test]
    fn replacing_releases_the_old_charge() {
        let mut c = cache(100);
        c.insert(1, "a".into(), 60);
        c.insert(1, "bb".into(), 20);
        let s = c.stats();
        assert_eq!(s.entries, 1);
        assert_eq!(s.bytes, 20);
        assert_eq!(c.get(&1).map(String::as_str), Some("bb"));
    }

    #[test]
    fn budget_too_small_to_hold_any_entry_caches_nothing() {
        let mut c = cache(10);
        assert!(!c.insert(1, "a".into(), 20));
        assert!(c.get(&1).is_none());
        assert_eq!(c.stats().bytes, 0);
    }

    #[test]
    fn clear_releases_all_bytes() {
        let mut c = cache(100);
        c.insert(1, "a".into(), 40);
        c.insert(2, "b".into(), 40);
        c.clear();
        assert_eq!(c.stats().entries, 0);
        assert_eq!(c.stats().bytes, 0);
    }
}
