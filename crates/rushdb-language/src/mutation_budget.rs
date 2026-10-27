//! Cumulative admission for a mutation's copy-on-write layer. Deleted or
//! replaced values are not refunded: originals, current versions, and the
//! eventual delta may coexist until publication.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use grafeo_common::types::Value;

use crate::LanguageError;

pub(crate) struct MutationBudget {
    used: AtomicUsize,
    exceeded: AtomicBool,
    limit: usize,
}

impl MutationBudget {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            used: AtomicUsize::new(0),
            exceeded: AtomicBool::new(false),
            limit,
        }
    }

    pub(crate) fn admit(&self, bytes: usize) -> bool {
        if self.exceeded.load(Ordering::Acquire) {
            return false;
        }
        // Cover originals, mutable state, delta, and conversion scratch.
        let accepted = self
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes.checked_mul(4)?)
                    .filter(|next| *next <= self.limit)
            })
            .is_ok();
        if !accepted {
            self.exceeded.store(true, Ordering::Release);
        }
        accepted
    }

    pub(crate) fn check(&self) -> Result<(), LanguageError> {
        if self.exceeded.load(Ordering::Acquire) {
            Err(LanguageError::MutationBytesExceeded(self.limit))
        } else {
            Ok(())
        }
    }
}

pub(crate) fn property_bytes(key: &str, value: &Value) -> usize {
    128usize
        .saturating_add(key.len())
        .saturating_add(value.estimated_size_bytes())
}
