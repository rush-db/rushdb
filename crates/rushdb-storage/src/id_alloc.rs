//! Store-wide allocation of monotone ids (record and relationship ordinals,
//! property families and definitions).
//!
//! An id is reserved when a statement is planned, like a PostgreSQL sequence:
//! two concurrent transactions never receive the same id, and an id is never
//! handed out twice, even when the transaction that reserved it aborts (that
//! leaves a gap). The durable counter row is a watermark that commits advance
//! to `max(current, id + 1)` ([`crate::Txn::set_watermark`]); it is neither a
//! read dependency nor a write conflict of a transaction, so transactions that
//! create disjoint records do not conflict on it.
//!
//! The reservations live in memory. After a restart the next id comes from
//! the durable watermark: every id of a committed write is below it, and an id
//! reserved by a transaction that never committed is free again, since that
//! transaction did not survive the restart.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::StorageError;

/// Ids reserved so far, per counter key: the next id each counter hands out
/// when its durable watermark is not higher.
#[derive(Debug, Default)]
pub struct IdAllocator {
    next: Mutex<HashMap<Vec<u8>, u64>>,
}

impl IdAllocator {
    /// Reserve the next id of the counter at `key`, given the counter's
    /// watermark as the caller's view sees it.
    ///
    /// # Errors
    /// Returns [`StorageError::Builder`] if the reservation table's lock is
    /// poisoned, or [`StorageError::ResourceExhausted`] if the counter would
    /// pass `u64::MAX`.
    pub fn reserve(&self, key: &[u8], watermark: u64) -> Result<u64, StorageError> {
        let mut next = self
            .next
            .lock()
            .map_err(|_| StorageError::Builder("id allocator lock poisoned".into()))?;
        let slot = next.entry(key.to_vec()).or_insert(0);
        let id = (*slot).max(watermark);
        *slot = id
            .checked_add(1)
            .ok_or_else(|| StorageError::ResourceExhausted("id counter exhausted".into()))?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::IdAllocator;

    #[test]
    fn reservations_are_unique_and_follow_the_watermark() {
        let ids = IdAllocator::default();
        // Two planners that read the same stale watermark get distinct ids.
        assert_eq!(ids.reserve(b"c", 5).unwrap(), 5);
        assert_eq!(ids.reserve(b"c", 5).unwrap(), 6);
        // A watermark advanced by a commit moves the next id past it.
        assert_eq!(ids.reserve(b"c", 10).unwrap(), 10);
        assert_eq!(ids.reserve(b"c", 0).unwrap(), 11);
        // Counters are independent.
        assert_eq!(ids.reserve(b"d", 0).unwrap(), 0);
    }
}
