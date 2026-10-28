//! Planner statistics on demand: a value distribution is read
//! from the store's evictable [`rushdb_storage::PlannerStats`] cache, or
//! loaded by a budgeted walk of the value dictionary on a miss. Estimates
//! only; exact answers read counters at the snapshot.

use std::sync::Arc;

use rushdb_lmpg::derived::{value_distribution, WalkBudget};
use rushdb_lmpg::reader::TryRead;
use rushdb_storage::planner_stats::PlannerStatsEntry;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::stats::Histogram;

use crate::QueryError;

/// The value distribution of `(kind, label, pid)` (`label` `None` = every
/// label), cached per store when `snap` is a committed snapshot.
pub(crate) fn distribution<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    kind: OwnerKind,
    label: Option<&str>,
    pid: u64,
) -> Result<Option<Arc<Histogram>>, QueryError> {
    let load = || -> Result<Option<Arc<Histogram>>, QueryError> {
        Ok(
            value_distribution(snap, tenant, kind, label, pid, WalkBudget::default())?
                .map(Arc::new),
        )
    };
    let Some((cache, ci)) = snap.planner_stats() else {
        return load();
    };
    let key = (tenant, kind, label.map(str::to_owned), pid);
    if let Some(entry) = cache.get(&key, ci) {
        return Ok(entry.histogram);
    }
    let histogram = load()?;
    cache.insert(
        key,
        PlannerStatsEntry {
            histogram: histogram.clone(),
            computed_at_ci: ci,
        },
    );
    Ok(histogram)
}
