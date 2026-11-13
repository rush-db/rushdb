//! Derived-state health (SELF-HEAL): the labels whose derived rows the
//! engine found disagreeing with their base rows while reading them.
//!
//! Derived rows are exact by construction (one deriver, applied with the base
//! rows of every commit), so a disagreement means a deriver bug. A read that
//! notices one reports it here instead of skipping it: the label becomes
//! *suspect*, the planner stops trusting its derived rows (it answers from
//! base rows, slower but exact), and the server repairs it with a logged
//! re-derive command and logs each finding. The registry is in memory: after
//! a restart a remaining disagreement is found again by the next read.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Mutex;

use rushdb_types::ids::TenantId;

/// Findings kept for the server to log; older ones are counted, not kept.
const PENDING_FINDINGS: usize = 256;

/// One disagreement between derived and base rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The tenant whose derived rows disagree.
    pub tenant: TenantId,
    /// The label whose derived rows disagree.
    pub label: String,
    /// What disagreed (a stable metric label, e.g. `presence_count`).
    pub kind: &'static str,
    /// The specifics, for the log line.
    pub detail: String,
}

#[derive(Default)]
struct State {
    suspects: BTreeSet<(TenantId, String)>,
    pending: VecDeque<Finding>,
    reported: BTreeMap<&'static str, u64>,
}

/// The store's registry of suspect labels.
#[derive(Default)]
pub struct DerivedHealth {
    state: Mutex<State>,
}

impl std::fmt::Debug for DerivedHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedHealth").finish_non_exhaustive()
    }
}

impl DerivedHealth {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Report a disagreement: the label becomes suspect.
    pub fn report(&self, finding: Finding) {
        let mut state = self.state();
        *state.reported.entry(finding.kind).or_default() += 1;
        state
            .suspects
            .insert((finding.tenant, finding.label.clone()));
        if state.pending.len() >= PENDING_FINDINGS {
            state.pending.pop_front();
        }
        state.pending.push_back(finding);
    }

    /// Whether `label`'s derived rows are untrusted.
    #[must_use]
    pub fn is_suspect(&self, tenant: TenantId, label: &str) -> bool {
        self.state().suspects.contains(&(tenant, label.to_owned()))
    }

    /// Whether any label of `tenant` is suspect.
    #[must_use]
    pub fn tenant_suspect(&self, tenant: TenantId) -> bool {
        self.state()
            .suspects
            .range((tenant, String::new())..)
            .next()
            .is_some_and(|(t, _)| *t == tenant)
    }

    /// Every suspect `(tenant, label)`.
    #[must_use]
    pub fn suspects(&self) -> Vec<(TenantId, String)> {
        self.state().suspects.iter().cloned().collect()
    }

    /// `label` was repaired: its derived rows are trusted again.
    pub fn clear(&self, tenant: TenantId, label: &str) {
        self.state().suspects.remove(&(tenant, label.to_owned()));
    }

    /// The findings not yet taken, oldest first.
    #[must_use]
    pub fn take_findings(&self) -> Vec<Finding> {
        self.state().pending.drain(..).collect()
    }

    /// Findings reported since start, by kind.
    #[must_use]
    pub fn reported(&self) -> BTreeMap<&'static str, u64> {
        self.state().reported.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_makes_its_label_suspect_until_cleared() {
        let health = DerivedHealth::default();
        let (a, b) = (TenantId::from_ordinal(1), TenantId::from_ordinal(2));
        assert!(!health.tenant_suspect(a));
        health.report(Finding {
            tenant: a,
            label: "Order".into(),
            kind: "presence_count",
            detail: String::new(),
        });
        assert!(health.is_suspect(a, "Order"));
        assert!(health.tenant_suspect(a));
        assert!(!health.tenant_suspect(b));
        assert!(!health.is_suspect(a, "Customer"));
        assert_eq!(health.take_findings().len(), 1);
        assert!(health.take_findings().is_empty());
        assert_eq!(health.reported()["presence_count"], 1);
        health.clear(a, "Order");
        assert!(!health.tenant_suspect(a));
    }
}
