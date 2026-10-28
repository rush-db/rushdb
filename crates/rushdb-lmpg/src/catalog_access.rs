//! What a catalog read may name and count under the tenant's data-access
//! policy. Every catalog surface (`describe`, name suggestions, the
//! schema summary, value overlap, value search and SQL's tables) asks this
//! one type, so a new surface cannot apply the policy differently.
//!
//! The record-level checks are [`AuthorizedView`](crate::AuthorizedView)'s;
//! on top of them a catalog read withholds groups counted over fewer records
//! than the policy's `minCount`, and the extremes (min, max, samples) that
//! often are one record's value.

use std::sync::Arc;

use rushdb_types::security::{ActivePolicy, AuthorizationMode};

use crate::authorized_view;

/// The tenant's enforced policy, as catalog reads apply it. The default
/// (no enforced policy) shows everything.
#[derive(Clone, Debug, Default)]
pub struct CatalogAccess {
    policy: Option<Arc<ActivePolicy>>,
}

impl CatalogAccess {
    /// Applies `policy` when it is enforced; a `BoundaryOnly` policy (or
    /// none) restricts nothing.
    #[must_use]
    pub fn new(policy: Option<Arc<ActivePolicy>>) -> Self {
        Self {
            policy: policy.filter(|active| active.mode == AuthorizationMode::PolicyEnforced),
        }
    }

    /// Whether an enforced policy restricts what is shown.
    #[must_use]
    pub fn enforced(&self) -> bool {
        self.policy.is_some()
    }

    /// The enforced policy, if any.
    #[must_use]
    pub fn policy(&self) -> Option<&ActivePolicy> {
        self.policy.as_deref()
    }

    /// The smallest group a catalog read may report; 0 withholds nothing.
    #[must_use]
    pub fn min_count(&self) -> u64 {
        authorized_view::min_count(self.policy())
    }

    /// Whether `label` may be named.
    #[must_use]
    pub fn label(&self, label: &str) -> bool {
        authorized_view::label_allowed(self.policy(), label)
    }

    /// Whether `property` of `label` may be named.
    #[must_use]
    pub fn property(&self, label: &str, property: &str) -> bool {
        authorized_view::property_allowed(self.policy(), label, property)
    }

    /// Whether relationships of `rel_type` may be named.
    #[must_use]
    pub fn rel_type(&self, rel_type: &str) -> bool {
        authorized_view::rel_type_allowed(self.policy(), rel_type)
    }

    /// Whether a group counted over `records` records may be reported.
    #[must_use]
    pub fn counted(&self, records: u64) -> bool {
        records >= self.min_count()
    }

    /// Whether `label`, holding `records` records, may be listed.
    #[must_use]
    pub fn label_listed(&self, label: &str, records: u64) -> bool {
        self.label(label) && self.counted(records)
    }

    /// Whether `property` of `label`, held by `records` records, may be
    /// listed.
    #[must_use]
    pub fn column_listed(&self, label: &str, property: &str, records: u64) -> bool {
        self.property(label, property) && self.counted(records)
    }

    /// Whether `count` relationships of `rel_type` from `source` to
    /// `target` may be listed: the type and both labels must be allowed.
    #[must_use]
    pub fn shape_listed(&self, rel_type: &str, source: &str, target: &str, count: u64) -> bool {
        self.rel_type(rel_type) && self.label(source) && self.label(target) && self.counted(count)
    }

    /// Whether minimum, maximum and sample values may be shown: not under a
    /// small-count threshold, where they are often one record's value.
    #[must_use]
    pub fn shows_extremes(&self) -> bool {
        self.min_count() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::CatalogAccess;
    use rushdb_types::security::{ActivePolicy, AuthorizationMode, DataAccessPolicy};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn access(mode: AuthorizationMode, min_count: u64) -> CatalogAccess {
        CatalogAccess::new(Some(Arc::new(ActivePolicy {
            generation: 1,
            policy: DataAccessPolicy {
                allowed_labels: vec!["Patient".into(), "Visit".into()],
                denied_rel_types: vec!["BILLED".into()],
                visible_properties: BTreeMap::from([("Patient".into(), vec!["age".into()])]),
                max_results: 100,
            },
            mode,
            fingerprint: 0,
            min_count,
        })))
    }

    #[test]
    fn an_enforced_policy_hides_names_and_small_groups() {
        let access = access(AuthorizationMode::PolicyEnforced, 5);
        assert!(access.enforced());
        assert!(access.label_listed("patient", 5));
        assert!(!access.label_listed("Patient", 4));
        assert!(!access.label_listed("Invoice", 100));
        assert!(access.column_listed("Patient", "AGE", 5));
        assert!(!access.column_listed("Patient", "name", 100));
        assert!(access.column_listed("Visit", "reason", 5));
        assert!(access.shape_listed("HAD", "Patient", "Visit", 9));
        assert!(!access.shape_listed("BILLED", "Patient", "Visit", 9));
        assert!(!access.shape_listed("HAD", "Patient", "Invoice", 9));
        assert!(!access.shows_extremes());
    }

    #[test]
    fn no_policy_or_boundary_only_shows_everything() {
        for access in [
            CatalogAccess::default(),
            access(AuthorizationMode::BoundaryOnly, 5),
        ] {
            assert!(!access.enforced());
            assert_eq!(access.min_count(), 0);
            assert!(access.label_listed("Invoice", 0));
            assert!(access.column_listed("Patient", "name", 1));
            assert!(access.shape_listed("BILLED", "Patient", "Invoice", 1));
            assert!(access.shows_extremes());
        }
    }
}
