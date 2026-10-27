//! Bounded in-memory window of per-commit write sets used for optimistic
//! conflict validation.
//!
//! A reader validated at commit time must know which keys every commit
//! published after its snapshot wrote. Those readers are live statements and
//! transactions: they are bounded by `max_txn_ttl` and never survive a
//! restart. The write sets therefore live in memory only. They are not
//! WAL ops and not redb rows. Persisting them as JSON number arrays cost
//! ~13.7 KB per record on disk and roughly half of every WAL record
//! (C4, cloud-capacity F1).
//!
//! The window keeps the newest commits under a byte cap and evicts oldest
//! first. A validation that needs an evicted commit fails with the retryable
//! [`crate::StorageError::ConflictHistoryUnavailable`], never with a missed
//! conflict.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::Op;

/// Default byte cap for the retained write sets.
const DEFAULT_CONFLICT_WINDOW_BYTES: usize = 64 * 1024 * 1024;

/// Accounting overhead charged per retained key, on top of its length.
const PER_KEY_OVERHEAD: usize = 24;

/// Write sets of recently published commits, oldest first.
pub(crate) struct ConflictWindow {
    entries: VecDeque<(u64, Arc<[Vec<u8>]>, usize)>,
    bytes: usize,
    cap: usize,
    /// The window's bytes charged to the store's managed budget. When the
    /// budget cannot admit more, the oldest write sets go first.
    charge: Option<crate::resources::ByteReservation>,
}

impl std::fmt::Debug for ConflictWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConflictWindow")
            .field("entries", &self.entries.len())
            .field("bytes", &self.bytes)
            .field("cap", &self.cap)
            .finish_non_exhaustive()
    }
}

impl ConflictWindow {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            cap,
            charge: None,
        }
    }

    /// A window of the default cap whose bytes are charged to `budget`.
    pub(crate) fn charged_to(budget: &crate::resources::ByteBudget) -> Self {
        Self::charged(DEFAULT_CONFLICT_WINDOW_BYTES, budget)
    }

    pub(crate) fn charged(cap: usize, budget: &crate::resources::ByteBudget) -> Self {
        Self {
            charge: budget.try_reserve(0).ok(),
            ..Self::new(cap)
        }
    }

    /// Charge the window's current bytes; `false` when the budget refuses.
    fn charge_bytes(&mut self) -> bool {
        let bytes = self.bytes;
        self.charge
            .as_mut()
            .is_none_or(|charge| charge.resize(bytes).is_ok())
    }

    fn evict_oldest(&mut self) {
        if let Some((_, _, evicted)) = self.entries.pop_front() {
            self.bytes -= evicted;
        }
    }

    /// Record the application-visible write set of commit `ci`.
    ///
    /// Commits are recorded in publication (ascending index) order.
    /// Coordination rows such as the outcome, journal and watermark keys are
    /// excluded, because every commit rewrites them and they are never read
    /// dependencies.
    pub(crate) fn record(&mut self, ci: u64, ops: &[Op]) {
        let keys: Arc<[Vec<u8>]> = ops
            .iter()
            .map(op_key)
            .filter(|key| !crate::is_coordination_write(key))
            .cloned()
            .collect();
        let size = keys.iter().map(|key| key.len() + PER_KEY_OVERHEAD).sum();
        self.entries.push_back((ci, keys, size));
        self.bytes += size;
        // Keep the newest commit even when it alone exceeds the cap (the
        // next validation most likely needs exactly that one), but never
        // hold bytes the managed budget refused: an evicted write set only
        // makes a validation that needs it fail retryably.
        while self.bytes > self.cap && self.entries.len() > 1 {
            self.evict_oldest();
        }
        while !self.charge_bytes() {
            self.evict_oldest();
        }
    }

    /// The write set of commit `ci`, or `None` when it was evicted or never
    /// published by this process.
    pub(crate) fn keys(&self, ci: u64) -> Option<Arc<[Vec<u8>]>> {
        let first = self.entries.front()?.0;
        let index = usize::try_from(ci.checked_sub(first)?).ok()?;
        let (found, keys, _) = self.entries.get(index)?;
        (*found == ci).then(|| Arc::clone(keys))
    }

    /// Bytes currently charged to the window.
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    /// Drop the entry for `ci` (test hook for the eviction path).
    #[cfg(test)]
    pub(crate) fn forget(&mut self, ci: u64) {
        if let Some(position) = self.entries.iter().position(|(found, _, _)| *found == ci) {
            if let Some((_, _, size)) = self.entries.remove(position) {
                self.bytes -= size;
                self.charge_bytes();
            }
        }
    }
}

fn op_key(op: &Op) -> &Vec<u8> {
    match op {
        Op::Put(key, _) | Op::Set(key, _) | Op::Del(key) | Op::Purge(key) => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(key: &[u8]) -> Op {
        Op::Put(key.to_vec(), b"v".to_vec())
    }

    #[test]
    fn records_application_keys_and_skips_coordination_rows() {
        let mut window = ConflictWindow::new(1024);
        window.record(
            1,
            &[
                put(b"T/a"),
                Op::Del(b"T/b".to_vec()),
                Op::Set(crate::LAST_APPLIED_KEY.to_vec(), Vec::new()),
            ],
        );
        let keys = window.keys(1).unwrap();
        assert_eq!(&keys[..], &[b"T/a".to_vec(), b"T/b".to_vec()]);
        assert!(window.keys(2).is_none());
        assert!(window.keys(0).is_none());
    }

    #[test]
    fn evicts_oldest_first_under_the_cap_but_keeps_the_newest() {
        let mut window = ConflictWindow::new(2 * (3 + PER_KEY_OVERHEAD));
        for ci in 1..=3 {
            window.record(ci, &[put(b"T/x")]);
        }
        assert!(window.keys(1).is_none(), "oldest evicted");
        assert!(window.keys(2).is_some() && window.keys(3).is_some());
        assert_eq!(window.bytes(), 2 * (3 + PER_KEY_OVERHEAD));

        let mut tiny = ConflictWindow::new(1);
        tiny.record(7, &[put(b"T/huge")]);
        assert!(
            tiny.keys(7).is_some(),
            "a single oversized commit is still kept"
        );
    }

    #[test]
    fn the_window_is_charged_to_the_budget_and_shrinks_to_fit_it() {
        let per = 3 + PER_KEY_OVERHEAD;
        let budget = crate::resources::ByteBudget::new("test", 3 * per).unwrap();
        let mut window = ConflictWindow::charged(1024, &budget);
        window.record(1, &[put(b"T/x")]);
        window.record(2, &[put(b"T/y")]);
        assert_eq!(budget.snapshot().used, 2 * per, "charged to the budget");
        // Another holder takes the rest: the window gives way, oldest first.
        let other = budget.try_reserve(per).unwrap();
        window.record(3, &[put(b"T/z")]);
        assert!(window.keys(1).is_none(), "the budget evicts the oldest");
        assert!(window.keys(2).is_some() && window.keys(3).is_some());
        assert_eq!(window.bytes(), 2 * per);
        assert_eq!(budget.snapshot().used, 3 * per);
        drop(other);
        drop(window);
        assert_eq!(
            budget.snapshot().used,
            0,
            "dropping the window releases its charge"
        );
    }
}
