//! Graph-aware data access policies layered above the hard tenant boundary
//! (plan document 23). Policies are compiled once into a `SecurityScope` with
//! a stable fingerprint; enforcement is default-deny, evaluated by callers
//! over `MatchScope` candidates and mutation images, and can only narrow the
//! domain — never widen it.

use rushdb_lmpg::Lmpg;
use serde_json::Value as Json;
use thiserror::Error;

use rushdb_types::security::{policy_fingerprint, policy_fingerprint_with_min_count};
pub use rushdb_types::security::{ActivePolicy, AuthorizationMode, DataAccessPolicy};

type PolicyReader<'a> = dyn Fn(&[u8]) -> Result<Option<Vec<u8>>, rushdb_storage::StorageError> + 'a;

trait PolicyAccess {
    fn label_allowed(&self, mode: AuthorizationMode, label: &str) -> bool;
    fn rel_allowed(&self, rel_type: &str) -> bool;
}

impl PolicyAccess for DataAccessPolicy {
    fn label_allowed(&self, mode: AuthorizationMode, label: &str) -> bool {
        match mode {
            AuthorizationMode::BoundaryOnly => true,
            AuthorizationMode::PolicyEnforced => self.allows_label(label),
        }
    }

    fn rel_allowed(&self, rel_type: &str) -> bool {
        !self.denied_rel_types.iter().any(|t| t == rel_type)
    }
}

/// Compiled security scope bound to one policy generation.
#[derive(Clone)]
pub struct SecurityScope<'a> {
    /// Policy supplying the label grants, relationship denials, and masks.
    pub policy: &'a DataAccessPolicy,
    /// Mode controlling how strictly the policy is applied.
    pub mode: AuthorizationMode,
    fingerprint: u64,
}

impl<'a> SecurityScope<'a> {
    /// Compiles `policy` under `mode` and records its stable fingerprint.
    #[must_use]
    pub fn compile(policy: &'a DataAccessPolicy, mode: AuthorizationMode) -> Self {
        SecurityScope {
            policy,
            mode,
            fingerprint: policy_fingerprint(policy, mode),
        }
    }

    /// Returns the stable fingerprint identifying this policy generation.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

/// Errors produced while compiling or enforcing data access policies.
#[derive(Debug, Error)]
pub enum AuthzError {
    /// The storage layer failed while reading policy state.
    #[error("authorization storage error: {0}")]
    Storage(#[from] rushdb_storage::StorageError),
    /// Stored policy metadata is malformed or internally inconsistent.
    #[error("invalid authorization policy: {0}")]
    InvalidPolicy(String),
    /// The authorized query failed while executing against the engine.
    #[error("authorized query failed: {0}")]
    Query(#[from] rushdb_query::QueryError),
    /// The requested label lies outside the authorized domain.
    #[error("label `{0}` is outside the authorized domain")]
    LabelDenied(String),
    /// The requested relationship type is explicitly denied.
    #[error("relationship type `{0}` is denied")]
    RelDenied(String),
    /// No candidate matched the intersection of request and grants.
    #[error("policy denied all candidates for this request")]
    NoCandidates,
}

/// Mutation-side check over the new image (old-image revalidation happens
/// at commit inside the engine's own preconditions).
///
/// # Errors
///
/// Returns [`AuthzError::LabelDenied`] when the label is not granted in
/// [`AuthorizationMode::PolicyEnforced`] mode.
pub fn authorize_mutation(scope: &SecurityScope<'_>, label: &str) -> Result<(), AuthzError> {
    if matches!(scope.mode, AuthorizationMode::PolicyEnforced)
        && !scope.policy.label_allowed(scope.mode, label)
    {
        return Err(AuthzError::LabelDenied(label.to_string()));
    }
    Ok(())
}

/// Rejects traversal through a relationship type the policy denies.
///
/// # Errors
///
/// Returns [`AuthzError::RelDenied`] when the relationship type is denied.
pub fn authorize_traversal_type(
    scope: &SecurityScope<'_>,
    rel_type: &str,
) -> Result<(), AuthzError> {
    if scope.policy.rel_allowed(rel_type) {
        Ok(())
    } else {
        Err(AuthzError::RelDenied(rel_type.to_string()))
    }
}

fn mask_record_json(lmpg: &Lmpg, record: &mut Json, scope: &SecurityScope<'_>) {
    let Some(obj) = record.as_object_mut() else {
        return;
    };
    let raw_label = obj
        .get("__label")
        .and_then(|l| l.as_str())
        .unwrap_or("")
        .to_string();
    let label_norm = normalize(&raw_label);
    if matches!(scope.mode, AuthorizationMode::PolicyEnforced)
        && !scope.policy.allows_label(&raw_label)
    {
        obj.clear();
        return;
    }
    // enforce relationship-type grants through adjacency? traversal types
    // are checked eagerly by callers via authorize_traversal_type.
    let _ = lmpg;
    // masks are keyed by normalized label name (case-insensitive grants)
    let masked_fields = scope
        .policy
        .visible_properties
        .iter()
        .find(|(k, _)| normalize(k) == label_norm)
        .map(|(_, v)| v.clone());
    if let Some(masked) = masked_fields {
        obj.retain(|k, _| masked.iter().any(|m| normalize(k) == normalize(m)));
    }
}

fn normalize(name: &str) -> String {
    rushdb_types::value::normalize_name(name)
}

/// Authorized query execution (Records perspective): scans, masks output,
/// enforces result caps. Tenant scoping underneath is structural.
///
/// # Errors
///
/// Returns [`AuthzError::RelDenied`] or [`AuthzError::NoCandidates`] when the
/// request falls outside the authorized domain, and propagates
/// [`AuthzError::Query`] from the underlying query engine.
pub fn find_records_authorized(
    lmpg: &Lmpg,
    req: &rushdb_query::SearchRequest,
    scope: &SecurityScope<'_>,
) -> Result<rushdb_query::Page, AuthzError> {
    use rushdb_query::Page;

    // default-deny: explicit labels required in enforced mode
    let allowed_labels = match scope.mode {
        AuthorizationMode::BoundaryOnly => req.labels.clone(),
        AuthorizationMode::PolicyEnforced => {
            let requested: Vec<String> = if req.labels.is_empty() {
                // "all labels" request intersects the granted set: the labels
                // present that it allows, as stored (under `*`, every label:
                // left unnarrowed below).
                rushdb_lmpg::reader::try_access::labels(lmpg.store.as_ref(), lmpg.tenant)?
                    .into_iter()
                    .filter(|label| scope.policy.allows_label(label))
                    .collect()
            } else {
                req.labels
                    .iter()
                    .filter(|l| scope.policy.label_allowed(scope.mode, l))
                    .cloned()
                    .collect()
            };
            requested
        }
    };

    if let AuthorizationMode::PolicyEnforced = scope.mode {
        if let Some(spec) = req.related.first() {
            if let Some(rt) = &spec.rel_type {
                if !scope.policy.rel_allowed(rt) {
                    return Err(AuthzError::RelDenied(rt.clone()));
                }
            }
        }
    }

    // Under `*` an "all labels" request stays one.
    let every = matches!(scope.mode, AuthorizationMode::PolicyEnforced)
        && req.labels.is_empty()
        && scope.policy.allows_every_label();
    let allowed_labels = if every { Vec::new() } else { allowed_labels };
    let effective = rushdb_query::SearchRequest {
        labels: allowed_labels.clone(),
        related: sanitize_related(req.related.as_slice(), scope),
        ..req.clone()
    };

    if matches!(scope.mode, AuthorizationMode::PolicyEnforced)
        && allowed_labels.is_empty()
        && !every
    {
        return Err(AuthzError::NoCandidates);
    }

    let page = rushdb_query::find_records(lmpg, &effective)?;

    let mut records = page.records;
    for r in &mut records {
        mask_record_json(lmpg, r, scope);
    }
    records.retain(|r| !r.as_object().is_some_and(serde_json::Map::is_empty));
    if scope.policy.max_results > 0 {
        records.truncate(usize::try_from(scope.policy.max_results).unwrap_or(usize::MAX));
    }
    let total = records.len() as u64;
    Ok(Page {
        total_matched: total,
        records,
    })
}

fn sanitize_related(
    specs: &[rushdb_query::RelatedSpec],
    scope: &SecurityScope<'_>,
) -> Vec<rushdb_query::RelatedSpec> {
    specs
        .iter()
        .filter(|s| s.rel_type.iter().all(|rt| scope.policy.rel_allowed(rt)))
        .cloned()
        .collect()
}

/// Exposes Direction / `CollectSpec` passthrough to keep engine facade tidy.
pub type QueryDirection = DirectionAlias;

type DirectionAlias = rushdb_query::Direction;

// ---- replicated policy persistence + engine authorizer (document 23) ----

use std::sync::Arc;

/// Read one complete policy generation from one retained native view.
///
/// # Errors
///
/// Propagates storage failures and returns [`AuthzError::InvalidPolicy`] when
/// the stored generation/fingerprint pair does not validate.
pub fn load_active_policy_in_view(
    view: &rushdb_storage::ReadView,
    tenant: rushdb_types::ids::TenantId,
) -> Result<Option<ActivePolicy>, AuthzError> {
    load_policy_from(view, tenant, view.commit_index())
}

fn load_policy_from<R: rushdb_lmpg::reader::TryRead>(
    view: &R,
    tenant: rushdb_types::ids::TenantId,
    cap: u64,
) -> Result<Option<ActivePolicy>, AuthzError> {
    load_policy_from_reader(&|key: &[u8]| view.get(key), tenant, cap)
}

/// Policy loader over an injected key-reader. Used by the coordinator's
/// commit-time revalidation so it never opens a nested store view (which
/// would deadlock under the store's publish lock).
///
/// # Errors
///
/// Propagates reader failures and returns [`AuthzError::InvalidPolicy`] when
/// stored metadata is malformed or uses the removed global policy layout.
pub fn load_policy_from_reader(
    read: &PolicyReader<'_>,
    tenant: rushdb_types::ids::TenantId,
    cap: u64,
) -> Result<Option<ActivePolicy>, AuthzError> {
    use rushdb_types::keys::{
        security_generation_key, security_policy_key, SECURITY_GENERATION_KEY,
        SECURITY_POLICY_ACTIVE_KEY,
    };
    let raw_generation = read(&security_generation_key(tenant))?;
    let raw_policy = read(&security_policy_key(tenant))?;
    match (raw_generation, raw_policy) {
        (None, None) => {
            // Global policy metadata has no tenant ownership in the current
            // format. Reject it rather than silently widening access.
            if read(SECURITY_GENERATION_KEY.as_bytes())?.is_some()
                || read(SECURITY_POLICY_ACTIVE_KEY.as_bytes())?.is_some()
            {
                return Err(AuthzError::InvalidPolicy(
                    "unsupported global authorization policy metadata".into(),
                ));
            }
            Ok(None)
        }
        (Some(generation), Some(raw)) => {
            let generation = u64::from_be_bytes(
                generation
                    .as_slice()
                    .try_into()
                    .map_err(|_| AuthzError::InvalidPolicy("invalid generation width".into()))?,
            );
            let active: ActivePolicy = serde_json::from_slice(&raw)
                .map_err(|e| AuthzError::InvalidPolicy(e.to_string()))?;
            if generation == 0
                || generation > cap
                || active.generation != generation
                || active.fingerprint
                    != policy_fingerprint_with_min_count(
                        &active.policy,
                        active.mode,
                        active.min_count,
                    )
            {
                return Err(AuthzError::InvalidPolicy(
                    "generation/fingerprint mismatch".into(),
                ));
            }
            Ok(Some(active))
        }
        _ => Err(AuthzError::InvalidPolicy(
            "incomplete policy/generation pair".into(),
        )),
    }
}

/// Loads the active policy for `lmpg`'s tenant from live committed state.
///
/// # Errors
///
/// Propagates storage failures and policy validation errors.
pub fn load_active_policy(lmpg: &Lmpg) -> Result<Option<ActivePolicy>, AuthzError> {
    load_active_policy_in_view(&lmpg.store.read_view()?, lmpg.tenant)
}

/// Returns the committed policy generation for `lmpg`'s tenant, or `0` when
/// no policy is active.
///
/// # Errors
///
/// Propagates storage failures and policy validation errors.
pub fn current_generation(lmpg: &Lmpg) -> Result<u64, AuthzError> {
    Ok(load_active_policy(lmpg)?.map_or(0, |active| active.generation))
}

/// Explicit administrative policy replacement. New tenant metadata overrides
/// legacy global metadata for this tenant only; no legacy bytes are changed.
/// The applying CI is assigned under admission, not before entering it.
///
/// # Errors
///
/// Returns the storage error or commit-conflict reason as a string when the
/// administrative commit is rejected.
pub fn store_policy(
    lmpg: &Lmpg,
    policy: &DataAccessPolicy,
    mode: AuthorizationMode,
) -> Result<ActivePolicy, String> {
    store_policy_with_min_count(lmpg, policy, mode, 0)
}

/// [`store_policy`] with a small-count threshold for catalog reads:
/// groups counted over fewer than `min_count` records are withheld. 0 is no
/// threshold and publishes exactly what [`store_policy`] does.
///
/// # Errors
///
/// Returns the storage error or commit-conflict reason as a string when the
/// administrative commit is rejected.
pub fn store_policy_with_min_count(
    lmpg: &Lmpg,
    policy: &DataAccessPolicy,
    mode: AuthorizationMode,
    min_count: u64,
) -> Result<ActivePolicy, String> {
    let tenant = lmpg.tenant;
    let policy = policy.clone();
    let outcome = lmpg
        .store
        .commit_for_tenant(&tenant, uuid::Uuid::now_v7(), move |ci, _txn| {
            let active = ActivePolicy {
                generation: ci,
                fingerprint: policy_fingerprint_with_min_count(&policy, mode, min_count),
                policy: policy.clone(),
                mode,
                min_count,
            };
            let mutation = if min_count == 0 {
                rushdb_types::command::Mutation::SetSecurityPolicy { policy, mode }
            } else {
                rushdb_types::command::Mutation::SetSecurityPolicyThreshold {
                    policy,
                    mode,
                    min_count,
                }
            };
            let batch = rushdb_types::command::CommitBatch {
                command_version: rushdb_types::command::COMMAND_VERSION,
                tenant,
                transaction_id: String::new(),
                commit_key: uuid::Uuid::nil(),
                start_index: 0,
                isolation: rushdb_types::command::IsolationMode::Snapshot,
                preconditions: vec![],
                mutations: vec![mutation],
            };
            Ok((batch, active))
        })
        .map_err(|e| e.to_string())?;
    match outcome {
        rushdb_storage::CommitOutcome::Committed { payload, .. } => Ok(payload),
        rushdb_storage::CommitOutcome::Conflicted(reason) => Err(reason),
    }
}

/// Engine authorizer reading live committed policy state (default-deny in
/// `PolicyEnforced` mode). Implemented against the replicated keyspace.
pub struct ActiveAuthorizer {
    store: Arc<rushdb_storage::Store>,
    tenant: rushdb_types::ids::TenantId,
}

impl ActiveAuthorizer {
    /// Creates an authorizer that reads policy for `tenant` from `store`.
    pub fn new(store: Arc<rushdb_storage::Store>, tenant: rushdb_types::ids::TenantId) -> Self {
        ActiveAuthorizer { store, tenant }
    }

    fn active(&self) -> Result<Option<ActivePolicy>, String> {
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        load_active_policy_in_view(&view, self.tenant).map_err(|e| e.to_string())
    }
}

impl rushdb_lmpg::Authorizer for ActiveAuthorizer {
    fn authorize_label(&self, label: &str) -> Result<(), String> {
        let Some(ap) = self.active()? else {
            return Ok(());
        };
        if ap.mode != AuthorizationMode::PolicyEnforced {
            return Ok(());
        }
        if !ap.policy.allows_label(label) {
            return Err(format!("label `{label}` not authorized"));
        }
        Ok(())
    }

    fn authorize_rel_type(&self, rel_type: &str) -> Result<(), String> {
        let Some(ap) = self.active()? else {
            return Ok(());
        };
        if ap.policy.denied_rel_types.iter().any(|t| t == rel_type) {
            return Err(format!("relationship type `{rel_type}` denied"));
        }
        Ok(())
    }

    fn current_generation(&self) -> Result<u64, String> {
        Ok(self.active()?.map_or(0, |active| active.generation))
    }

    fn authorize_label_in_txn(
        &self,
        label: &str,
        txn: &rushdb_storage::Txn<'_>,
        cap: u64,
    ) -> Result<(), String> {
        let active = load_policy_from(txn, self.tenant, cap).map_err(|e| e.to_string())?;
        if active.as_ref().is_some_and(|ap| {
            ap.mode == AuthorizationMode::PolicyEnforced && !ap.policy.allows_label(label)
        }) {
            return Err(format!("label `{label}` not authorized"));
        }
        Ok(())
    }

    fn authorize_rel_type_in_txn(
        &self,
        rel_type: &str,
        txn: &rushdb_storage::Txn<'_>,
        cap: u64,
    ) -> Result<(), String> {
        let active = load_policy_from(txn, self.tenant, cap).map_err(|e| e.to_string())?;
        if active.as_ref().is_some_and(|ap| {
            ap.policy
                .denied_rel_types
                .iter()
                .any(|denied| denied == rel_type)
        }) {
            return Err(format!("relationship type `{rel_type}` denied"));
        }
        Ok(())
    }

    fn current_generation_in_txn(
        &self,
        txn: &rushdb_storage::Txn<'_>,
        cap: u64,
    ) -> Result<u64, String> {
        Ok(load_policy_from(txn, self.tenant, cap)
            .map_err(|e| e.to_string())?
            .map_or(0, |ap| ap.generation))
    }
}
