//! `query.path`: the shortest path between two records
//! (`rushdb_query::shortest_path`), under the enforced policy: a denied
//! relationship type is never followed, and a record whose label the policy
//! hides is never passed through, so the path's length and its existence
//! reveal nothing the policy hides.

use rushdb_lmpg::reader::try_access as access;
use rushdb_types::ids::RecordId;

use super::{bytes_from_hex, RequestScope};

/// Why a path query did not run.
#[derive(Debug)]
pub(crate) enum PathError {
    /// The search's typed failure (its visit budget, cancellation).
    Query(rushdb_query::QueryError),
    /// Anything else, as the engine phrased it.
    Other(String),
}

impl From<String> for PathError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

impl From<rushdb_query::QueryError> for PathError {
    fn from(error: rushdb_query::QueryError) -> Self {
        Self::Query(error)
    }
}

impl RequestScope<'_> {
    /// The records on a shortest path from `query.from` to `query.to`, in
    /// order, or none when no path is within `maxHops`.
    pub(crate) fn shortest_path_with_execution_context(
        &self,
        context: &rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::PathQuery,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<serde_json::Value>, PathError> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let (view, tenant) = (context.read_view(), self.tenant());
        let active = rushdb_authz::load_active_policy_in_view(view, tenant)
            .map_err(|error| error.to_string())?;
        let authz = rushdb_lmpg::AuthorizedView::new(view.as_ref(), tenant, active.as_ref());
        match &query.relation_type {
            Some(rel_type) if !authz.rel_type_allowed(rel_type) => {
                return Err(format!("relationship type `{rel_type}` denied by policy").into());
            }
            // An untyped path could cross a type the policy denies.
            None if active
                .as_ref()
                .is_some_and(|active| !active.policy.denied_rel_types.is_empty()) =>
            {
                return Err("a path without a relationship type is denied by policy \
                            (it denies some relationship types)"
                    .to_string()
                    .into());
            }
            _ => {}
        }
        let endpoint = |id: &str| -> Result<u64, PathError> {
            let header = access::record_by_id(view, tenant, &RecordId(bytes_from_hex(id)?))
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "record not found".to_string())?;
            if !authz.label_allowed(&header.label) {
                return Err("record denied by policy".to_string().into());
            }
            Ok(header.ordinal)
        };
        let (from, to) = (endpoint(&query.from)?, endpoint(&query.to)?);
        let direction = match query.direction {
            rushdb_protocol::PathDirection::Out => rushdb_query::Direction::Outgoing,
            rushdb_protocol::PathDirection::In => rushdb_query::Direction::Incoming,
            rushdb_protocol::PathDirection::Both => rushdb_query::Direction::Both,
        };
        let path = rushdb_query::shortest_path(
            view,
            tenant,
            &rushdb_query::PathRequest {
                from,
                to,
                rel_type: query.relation_type.as_deref(),
                direction,
                max_hops: query.max_hops,
                labels: &query.labels,
            },
            &|label| authz.label_allowed(label),
            is_cancelled,
        )?;
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let Some(path) = path else {
            return Ok(Vec::new());
        };
        let proptypes = self.proptypes_cached(view)?;
        path.into_iter()
            .map(|ord| {
                let record = authz
                    .reconstruct_record(ord)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "record denied by policy".to_string())?;
                Ok(Self::canonical_record_with(&record, &proptypes))
            })
            .collect()
    }
}
