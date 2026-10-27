//! Memory a catalog read holds while it works (value dictionaries and the
//! values matched from them), capped like a query's working set by
//! `query-memory-max`, tightened by the request (READ-1, QRY-3). Charges
//! also poll cancellation and the request deadline, so a long dictionary
//! read stops when the request does.

use std::time::Instant;

use super::RequestScope;

/// How many charges pass between cancellation and deadline checks.
const CHECK_EVERY: u32 = 1024;

/// The bytes one distinct value holds in a working map: its encoded form
/// twice (key and stored value) plus the slot, as the facet scan counts it.
pub(crate) fn value_bytes(encoded_len: usize) -> u64 {
    u64::try_from(encoded_len)
        .unwrap_or(u64::MAX)
        .saturating_mul(2)
        .saturating_add(64)
}

/// A request's working-set meter for catalog reads.
pub(crate) struct WorkingSet<'a> {
    limit: Option<u64>,
    charged: u64,
    is_cancelled: &'a dyn Fn() -> bool,
    deadline: Option<Instant>,
    limits: Option<rushdb_lmpg::QueryLimits>,
    charges: u32,
}

impl RequestScope<'_> {
    /// A meter under this request's bounds: its working-set cap and time,
    /// and `deadline` (the execution context's), stopping when
    /// `is_cancelled` says so.
    pub(crate) fn working_set<'a>(
        &self,
        is_cancelled: &'a dyn Fn() -> bool,
        deadline: Option<Instant>,
    ) -> WorkingSet<'a> {
        let limits = self.lmpg().limits();
        let mut meter = WorkingSet::new(limits.query_memory_max_bytes(), is_cancelled, deadline);
        meter.limits = Some(limits);
        meter
    }
}

impl<'a> WorkingSet<'a> {
    /// A meter capped at `limit` bytes (`None`: uncapped) that stops when
    /// `is_cancelled` says so or `deadline` passes.
    pub(crate) fn new(
        limit: Option<u64>,
        is_cancelled: &'a dyn Fn() -> bool,
        deadline: Option<Instant>,
    ) -> Self {
        Self {
            limit,
            charged: 0,
            is_cancelled,
            deadline,
            limits: None,
            charges: 0,
        }
    }

    /// Fails when the request was cancelled or ran past its deadline.
    pub(crate) fn check(&self) -> Result<(), String> {
        if (self.is_cancelled)() {
            return Err("query cancelled".into());
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
            || self.limits.is_some_and(|limits| limits.expired())
        {
            return Err("execution deadline exceeded".into());
        }
        Ok(())
    }

    /// Charges `bytes` now held; fails past the limit, and every so often
    /// checks cancellation and the deadline.
    pub(crate) fn charge(&mut self, bytes: u64) -> Result<(), String> {
        self.charged = self.charged.saturating_add(bytes);
        if let Some(limit) = self.limit {
            if self.charged > limit {
                return Err(format!("query working set exceeded {limit} bytes"));
            }
        }
        self.charges = self.charges.wrapping_add(1);
        if self.charges.is_multiple_of(CHECK_EVERY) {
            self.check()?;
        }
        Ok(())
    }

    /// Returns `bytes` no longer held.
    pub(crate) fn release(&mut self, bytes: u64) {
        self.charged = self.charged.saturating_sub(bytes);
    }

    /// Bytes currently charged.
    pub(crate) fn charged(&self) -> u64 {
        self.charged
    }
}

#[cfg(test)]
mod tests {
    use super::{value_bytes, WorkingSet};

    #[test]
    fn charges_up_to_the_limit_and_releases() {
        let mut meter = WorkingSet::new(Some(200), &|| false, None);
        meter.charge(value_bytes(10)).unwrap();
        meter.charge(value_bytes(10)).unwrap();
        let error = meter.charge(value_bytes(10)).unwrap_err();
        assert!(error.contains("query working set exceeded 200 bytes"));
        meter.release(value_bytes(10) * 2);
        meter.charge(value_bytes(10)).unwrap();
    }

    #[test]
    fn stops_when_cancelled_or_past_the_deadline() {
        assert!(WorkingSet::new(None, &|| true, None).check().is_err());
        let past = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(1))
            .unwrap();
        let meter = WorkingSet::new(None, &|| false, Some(past));
        assert!(meter.check().unwrap_err().contains("deadline exceeded"));
        let mut cancelled = WorkingSet::new(None, &|| true, None);
        let failed = (0..2048).any(|_| cancelled.charge(1).is_err());
        assert!(failed);
    }
}
