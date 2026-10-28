//! Per-tenant usage figures for a list of tenants, an operator read.
//!
//! The engine has no workspace, plan or billing concept: the caller names the
//! tenant scopes it wants grouped and gets one row per scope plus their sum.
//! Every figure is a counter maintained by the commit that changes it, so the
//! read costs one `get` per metric and embedding space for each tenant and
//! never scans tenant data.

use rushdb_lmpg::derived::{tenant_usage, TenantUsage};
use rushdb_types::ids::TenantScope;
use serde_json::{json, Value};

use super::Node;

/// Why a usage read failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageError {
    /// The request does not fit the node's tenancy mode or limits.
    Invalid(String),
    /// The store read failed.
    Store(String),
}

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) | Self::Store(message) => f.write_str(message),
        }
    }
}

/// Most tenant scopes one usage read accepts.
pub(crate) const MAX_USAGE_TENANTS: usize = 10_000;

impl Node {
    /// Usage of each tenant scope in `scopes`, and their total, read from one
    /// snapshot. A scope the directory does not know (never written, or
    /// purged) reports `exists: false` and zeros; it is never registered by
    /// this read. In single-tenant mode `scopes` must be empty and the one
    /// tenant is reported with a `null` scope.
    ///
    /// # Errors
    /// Returns a message for a request that does not fit the node's tenancy
    /// mode or exceeds [`MAX_USAGE_TENANTS`], or when the store read fails.
    pub fn tenant_usage(&self, scopes: &[String]) -> Result<Value, UsageError> {
        if scopes.len() > MAX_USAGE_TENANTS {
            return Err(UsageError::Invalid(format!(
                "at most {MAX_USAGE_TENANTS} tenants per usage read"
            )));
        }
        if !self.multitenant() && !scopes.is_empty() {
            return Err(UsageError::Invalid(
                "tenants are not accepted in single-tenant mode".into(),
            ));
        }
        if self.multitenant() && scopes.is_empty() {
            return Err(UsageError::Invalid(
                "tenants must list at least one tenant scope".into(),
            ));
        }
        let view = self
            .store
            .read_view()
            .map_err(|e| UsageError::Store(e.to_string()))?;
        let mut rows = Vec::with_capacity(scopes.len().max(1));
        let mut total = TenantUsage::default();
        let mut total_vectors = 0u64;
        let targets: Vec<Option<&str>> = if self.multitenant() {
            scopes.iter().map(|scope| Some(scope.as_str())).collect()
        } else {
            vec![None]
        };
        for scope in targets {
            let tenant = match scope {
                None => Some(self.base_tenant),
                Some(scope) => self
                    .store
                    .resolve_tenant(&TenantScope::of(scope))
                    .map_err(|e| UsageError::Store(e.to_string()))?,
            };
            let usage = match tenant {
                Some(tenant) => {
                    tenant_usage(&view, tenant).map_err(|e| UsageError::Store(e.to_string()))?
                }
                None => TenantUsage::default(),
            };
            let vectors: u64 = usage.vectors.iter().map(|space| space.vectors).sum();
            total.records += usage.records;
            total.relationships += usage.relationships;
            total.labels += usage.labels;
            total.properties += usage.properties;
            total.logical_bytes += usage.logical_bytes;
            total_vectors += vectors;
            let mut row = usage_json(&usage, vectors);
            row["tenant"] = json!(scope);
            row["exists"] = json!(tenant.is_some());
            row["vectorIndexes"] = usage
                .vectors
                .iter()
                .map(|space| json!({ "index": space.name, "vectors": space.vectors }))
                .collect();
            rows.push(row);
        }
        Ok(json!({
            "commitIndex": view.commit_index(),
            "tenants": rows,
            "total": usage_json(&total, total_vectors),
        }))
    }
}

fn usage_json(usage: &TenantUsage, vectors: u64) -> Value {
    json!({
        "records": usage.records,
        "relationships": usage.relationships,
        "labels": usage.labels,
        "properties": usage.properties,
        "logicalBytes": usage.logical_bytes,
        "vectors": vectors,
    })
}
