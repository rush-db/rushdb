//! Planner statistics on demand: a value distribution is read from a
//! resident layer (`rushdb_storage::resident`), or loaded by a budgeted walk
//! of the value dictionary on a miss and offered to it. An entry is dropped
//! as soon as a commit writes the property's values; there is no tolerated
//! staleness. Estimates only; exact answers read counters at the snapshot.

use std::sync::Arc;

use rushdb_lmpg::derived::{value_distribution, WalkBudget};
use rushdb_lmpg::reader::TryRead;
use rushdb_storage::resident::{Budget, BuildCx, BuildError, Dep, Resident};
use rushdb_storage::{ReadView, StorageError};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::stats::Histogram;

use crate::QueryError;

/// `(tenant, owner kind, label — `None` for every label —, pid)`.
pub(crate) type StatsKey = (TenantId, OwnerKind, Option<String>, u64);

/// One distribution (`None`: no range-comparable values).
#[derive(Debug)]
pub(crate) struct Distribution(Option<Arc<Histogram>>);

impl PartialEq for Distribution {
    /// A walk stopped by its time budget covers a timing-dependent prefix,
    /// so two partial walks of the same rows may differ; only complete ones
    /// are compared (an estimate, never a result).
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) if a.partial || b.partial => true,
            (a, b) => a == b,
        }
    }
}

/// Value distributions, one per property (and label).
#[derive(Default)]
pub(crate) struct PlannerStats;

impl Resident for PlannerStats {
    type Key = StatsKey;
    type Value = Distribution;
    const NAME: &'static str = "planner-stats";
    const BUDGET: Budget = Budget::PlannerStats;

    fn deps(&self, key: &StatsKey, _view: &ReadView) -> Result<Vec<Dep>, StorageError> {
        // The value dictionary, and the support counters that list the
        // labels carrying the pid.
        Ok(vec![Dep::ValueDict(key.0, key.3), Dep::Pid(key.0, key.3)])
    }

    fn build(
        &self,
        (tenant, kind, label, pid): &StatsKey,
        view: &ReadView,
        _cx: &BuildCx,
    ) -> Result<Option<Distribution>, BuildError> {
        let histogram = value_distribution(
            view,
            *tenant,
            *kind,
            label.as_deref(),
            *pid,
            WalkBudget::default(),
        )?;
        Ok(Some(Distribution(histogram.map(Arc::new))))
    }

    fn bytes(value: &Distribution) -> usize {
        std::mem::size_of::<StatsKey>() + 64 + value.0.as_ref().map_or(0, |h| h.resident_bytes())
    }
}

/// The value distribution of `(kind, label, pid)` (`label` `None` = every
/// label), resident when `snap` is a committed snapshot.
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
    let Some(view) = snap.committed() else {
        return load();
    };
    let layer = view.residents().layer::<PlannerStats>();
    let key = (tenant, kind, label.map(str::to_owned), pid);
    if let Some(entry) = layer.get(&key, view) {
        return Ok(entry.0.clone());
    }
    let histogram = load()?;
    layer.offer(key, view, Arc::new(Distribution(histogram.clone())));
    Ok(histogram)
}
