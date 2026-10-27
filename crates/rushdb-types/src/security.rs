//! Canonical tenant policy payload shared by admission, replay and readers.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How strongly a tenant's data-access policy is enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorizationMode {
    /// Enforce tenant isolation only; no per-query policy filtering.
    BoundaryOnly,
    /// Apply the tenant's [`DataAccessPolicy`] to every query.
    PolicyEnforced,
}

/// Tenant-scoped data-access restrictions applied to query results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataAccessPolicy {
    /// Label allow-list, compared as label names are (case-insensitive,
    /// see [`crate::value::normalize_name`]). `["*"]`
    /// ([`ALL_LABELS`]) allows every label, including labels created later;
    /// an empty list allows none, so an enforced policy without labels locks
    /// the tenant's data (reads and writes).
    pub allowed_labels: Vec<String>,
    /// Relationship types that readers must not traverse.
    pub denied_rel_types: Vec<String>,
    /// Per label, the only properties a reader sees (an allow-list); a label
    /// without an entry shows all of its properties.
    pub visible_properties: BTreeMap<String, Vec<String>>,
    /// Maximum number of rows a query may return.
    pub max_results: u64,
}

/// The `allowedLabels` entry that allows every label.
pub const ALL_LABELS: &str = "*";

impl DataAccessPolicy {
    /// Whether the allow-list holds [`ALL_LABELS`].
    #[must_use]
    pub fn allows_every_label(&self) -> bool {
        self.allowed_labels
            .iter()
            .any(|allowed| allowed == ALL_LABELS)
    }

    /// Whether `label` is allowed: [`ALL_LABELS`] is listed, or a name equal
    /// to `label` once normalized ([`crate::value::normalize_name`]). Every
    /// read and write check of the allow-list goes through here.
    #[must_use]
    pub fn allows_label(&self, label: &str) -> bool {
        let label = crate::value::normalize_name(label);
        self.allowed_labels
            .iter()
            .any(|allowed| allowed == ALL_LABELS || crate::value::normalize_name(allowed) == label)
    }
}

/// A policy together with the commit that published it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivePolicy {
    /// Applying commit index: monotonic, but not necessarily consecutive.
    pub generation: u64,
    /// The active policy payload.
    pub policy: DataAccessPolicy,
    /// Enforcement strength for this policy.
    pub mode: AuthorizationMode,
    /// Stable digest of the policy payload and mode.
    pub fingerprint: u64,
    /// Smallest group a catalog read may report under this policy (POL-2
    /// small-count suppression): labels, properties, values, relationship
    /// shapes and join candidates counted over fewer records are withheld,
    /// as are sample and min/max values. 0 (the default) withholds nothing.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min_count: u64,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// Stable digest of a policy's semantic content and enforcement mode.
///
/// # Panics
///
/// Panics if the policy cannot be serialized to JSON, which cannot happen for
/// the plain data types held by [`DataAccessPolicy`].
#[must_use]
pub fn policy_fingerprint(policy: &DataAccessPolicy, mode: AuthorizationMode) -> u64 {
    let json = serde_json::json!({
        "labels": policy.allowed_labels, "denied": policy.denied_rel_types,
        "visible": policy.visible_properties, "max": policy.max_results,
        "mode": format!("{mode:?}"),
    });
    crate::hash::stable_hash64(&serde_json::to_vec(&json).expect("policy JSON is serializable"))
}

/// [`policy_fingerprint`] covering a small-count threshold too; equal to it
/// when `min_count` is 0, so a policy without one keeps its digest.
///
/// # Panics
///
/// Panics if the policy cannot be serialized to JSON, which cannot happen for
/// the plain data types held by [`DataAccessPolicy`].
#[must_use]
pub fn policy_fingerprint_with_min_count(
    policy: &DataAccessPolicy,
    mode: AuthorizationMode,
    min_count: u64,
) -> u64 {
    if min_count == 0 {
        return policy_fingerprint(policy, mode);
    }
    let json = serde_json::json!({
        "labels": policy.allowed_labels, "denied": policy.denied_rel_types,
        "visible": policy.visible_properties, "max": policy.max_results,
        "mode": format!("{mode:?}"), "minCount": min_count,
    });
    crate::hash::stable_hash64(&serde_json::to_vec(&json).expect("policy JSON is serializable"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_threshold_changes_the_fingerprint_only_when_set() {
        let policy = DataAccessPolicy {
            allowed_labels: vec!["A".into()],
            ..DataAccessPolicy::default()
        };
        let mode = AuthorizationMode::PolicyEnforced;
        assert_eq!(
            policy_fingerprint_with_min_count(&policy, mode, 0),
            policy_fingerprint(&policy, mode)
        );
        assert_ne!(
            policy_fingerprint_with_min_count(&policy, mode, 5),
            policy_fingerprint(&policy, mode)
        );
        // A stored policy without a threshold reads back as 0 and writes no field.
        let active = ActivePolicy {
            generation: 1,
            policy,
            mode,
            fingerprint: 7,
            min_count: 0,
        };
        let json = serde_json::to_value(&active).unwrap();
        assert!(json.get("min_count").is_none());
        let back: ActivePolicy = serde_json::from_value(json).unwrap();
        assert_eq!(back.min_count, 0);
    }

    #[test]
    fn the_allow_list_is_empty_for_none_star_for_all_and_ignores_case() {
        let policy = |labels: &[&str]| DataAccessPolicy {
            allowed_labels: labels.iter().map(ToString::to_string).collect(),
            ..DataAccessPolicy::default()
        };
        assert!(!policy(&[]).allows_label("Note"));
        assert!(!policy(&[]).allows_every_label());
        assert!(policy(&["*"]).allows_label("Note"));
        assert!(policy(&["Note", "*"]).allows_every_label());
        assert!(policy(&["note"]).allows_label("Note"));
        assert!(policy(&["first-name"]).allows_label("First_Name"));
        assert!(!policy(&["Note"]).allows_label("Notes"));
        assert!(!policy(&["Note"]).allows_every_label());
    }
}
