//! The aggregate's pass over its candidates, in fixed-size parts.
//!
//! The candidates are split into parts whose bounds depend on nothing but
//! the data: the index plan's owner set in parts of [`PART_ORDINALS`]
//! owners, a label walk in fixed ranges of ordinals that each part reads
//! itself (no list of every member is held). Each part checks its candidates against the
//! residual filter and folds the matches into a partial set of groups; parts
//! run on the threads [`rushdb_storage::parallel`] lends and their partials
//! merge in part order. The answer is therefore the same whether one thread
//! ran every part or many did. A part stops when another one failed or the
//! request's deadline passed; the caller's own cancellation is polled
//! between rounds of parts.
//!
//! Every candidate is a live record: a label's members and the index's
//! owner rows are written and deleted in the commit that writes and deletes
//! the record, so no record header is read per candidate.

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use rushdb_lmpg::reconstruct::ReconstructSession;

use crate::aggregate_rows::{row_bytes, Cell, Partial, RowCtx, RowPlan};
use crate::planner::index_candidates_with_budget;
use crate::{
    is_match, normalize, resolve_field, scan, walk_total, Budget, EvalCtx, Expr, Json, Lmpg,
    PointFields, QueryError, QuerySnapshot, SearchRequest, WorkingSet,
};

/// Candidate ordinals per part.
pub(crate) const PART_ORDINALS: usize = 8192;

/// What a pass produces of each match.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Only how many there are.
    Count,
    /// Their ordinals, in order (per-root rows).
    Roots,
    /// Their groups.
    Groups,
}

/// What the pass found.
pub(crate) struct Pass {
    /// Matched roots.
    pub(crate) total: u64,
    /// The matches' ordinals ([`Mode::Roots`]).
    pub(crate) roots: Vec<u64>,
    /// The matches' groups ([`Mode::Groups`]).
    pub(crate) groups: Partial,
    /// Bytes the candidate ordinals held.
    pub(crate) candidate_bytes: usize,
}

/// The reads one aggregate makes, shared by its parts.
pub(crate) struct Scan<'a> {
    pub(crate) lmpg: &'a Lmpg,
    pub(crate) search: &'a SearchRequest,
    pub(crate) snap: &'a QuerySnapshot,
    pub(crate) budget: Option<usize>,
    pub(crate) plan: &'a RowPlan<'a>,
}

/// One part's outcome: its result, or why it stopped.
enum PartError {
    /// Another part failed first.
    Stopped,
    Failed(QueryError),
}

impl From<QueryError> for PartError {
    fn from(error: QueryError) -> Self {
        Self::Failed(error)
    }
}

/// One part's result.
#[derive(Default)]
struct PartOut {
    matched: u64,
    roots: Vec<u64>,
    groups: Partial,
}

/// State the parts share.
struct Shared<'a> {
    check: bool,
    predicate: Option<&'a Expr>,
    predicate_fields: Option<&'a PointFields>,
    rows: &'a Rows<'a>,
    mode: Mode,
    /// The candidate ordinals' bytes, held throughout.
    base: usize,
    /// Set by the first part that fails.
    stop: AtomicBool,
    /// Bytes every part's partial holds together. Partials only grow, so
    /// whether the total ever passes the budget does not depend on timing.
    retained: AtomicUsize,
}

/// How a row's properties are read.
pub(crate) struct Rows<'a> {
    pub(crate) points: Option<&'a PointFields>,
    pub(crate) selected: &'a HashSet<String>,
}

impl Rows<'_> {
    /// Record `ord`'s row into `out`; `false` when its record is gone.
    pub(crate) fn read(
        &self,
        plan: &RowPlan<'_>,
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        session: &mut ReconstructSession<'_, QuerySnapshot>,
        ord: u64,
        out: &mut Vec<Option<Cell>>,
    ) -> Result<bool, QueryError> {
        if let Some(points) = self.points {
            points.cells(snap, tenant, ord, out)?;
            return Ok(true);
        }
        let Some(Json::Object(record)) = session.reconstruct_record_selected(ord, self.selected)?
        else {
            return Ok(false);
        };
        out.clear();
        out.extend(
            plan.names
                .iter()
                .map(|name| resolve_field(&record, name).map(Cell::from_json)),
        );
        Ok(true)
    }

    /// The names a rebuilt record is narrowed to: each read property by
    /// either spelling.
    pub(crate) fn selection(plan: &RowPlan<'_>) -> HashSet<String> {
        plan.names
            .iter()
            .flat_map(|name| [name.clone(), normalize(name)])
            .collect()
    }
}

/// Run the aggregate's pass.
///
/// # Errors
/// Returns the first failing part's error in part order: storage,
/// predicate, budget, group-cap or cancellation errors.
pub(crate) fn run(
    scan: &Scan<'_>,
    rows: &Rows<'_>,
    mode: Mode,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Pass, QueryError> {
    let (lmpg, snap, search) = (scan.lmpg, scan.snap, scan.search);
    let (source, check, predicate) =
        match index_candidates_with_budget(lmpg, snap, search, scan.budget, is_cancelled)? {
            Some(plan) => (
                Source::Owners(plan.ords.into_iter().collect()),
                !plan.exact,
                plan.residual,
            ),
            None => (
                Source::walk(lmpg, snap, &search.labels, is_cancelled)?,
                search.where_.is_some() || !search.related.is_empty(),
                search.where_.clone(),
            ),
        };
    let candidate_bytes = match &source {
        Source::Owners(ords) => ords.capacity().saturating_mul(std::mem::size_of::<u64>()),
        Source::Walk { .. } => 0,
    };
    Budget::WorkingSet.check(scan.budget, candidate_bytes)?;
    let predicate_fields = match (&predicate, check) {
        (Some(expr), true) => PointFields::for_predicate(snap, lmpg.tenant, &search.labels, expr)?,
        _ => None,
    };
    let shared = Shared {
        check,
        predicate: predicate.as_ref(),
        predicate_fields: predicate_fields.as_ref(),
        rows,
        mode,
        base: candidate_bytes,
        stop: AtomicBool::new(false),
        retained: AtomicUsize::new(0),
    };
    let parts = source.parts();
    let mut pass = Pass {
        total: 0,
        roots: Vec::new(),
        groups: Partial::default(),
        candidate_bytes,
    };
    // Rounds of as many parts as there are threads to lend, so the caller's
    // cancellation is polled between them.
    let round = rushdb_storage::parallel::budget().saturating_add(1);
    let mut start = 0;
    while start < parts {
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        let end = start.saturating_add(round).min(parts);
        let outs = rushdb_storage::parallel::run_parts(end - start, |i| {
            let out = source
                .part(scan, start + i)
                .map_err(PartError::Failed)
                .and_then(|ords| run_part(scan, &shared, &ords, source.held(&ords)));
            if matches!(out, Err(PartError::Failed(_))) {
                shared.stop.store(true, Ordering::SeqCst);
            }
            Ok::<_, std::convert::Infallible>(out)
        })
        .unwrap_or_else(|never| match never {});
        // The first failure in part order wins; a part that only saw the
        // stop (or the deadline) yields to a real failure after it.
        let mut stopped = false;
        for out in outs {
            match out {
                Ok(out) if !stopped => {
                    pass.total += out.matched;
                    pass.roots.extend(out.roots);
                    pass.groups.merge(out.groups)?;
                }
                Ok(_) | Err(PartError::Stopped | PartError::Failed(QueryError::Cancelled)) => {
                    stopped = true;
                }
                Err(PartError::Failed(error)) => return Err(error),
            }
        }
        if stopped {
            return Err(QueryError::Cancelled);
        }
        start = end;
    }
    Ok(pass)
}

/// Where the parts' candidates come from.
enum Source {
    /// The index plan's owners, [`PART_ORDINALS`] per part.
    Owners(Vec<u64>),
    /// The labels' members (every record without labels), read by each part
    /// for its own range of `width` ordinals below `end`. The width is
    /// [`PART_ORDINALS`] scaled by how sparse the members are among the
    /// tenant's ordinals, so a part holds about [`PART_ORDINALS`] members.
    Walk { width: u64, end: u64 },
}

impl Source {
    fn walk(
        lmpg: &Lmpg,
        snap: &QuerySnapshot,
        labels: &[String],
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryError> {
        let Some(end) = scan::ordinal_end(snap, lmpg.tenant)? else {
            // No allocator watermark: collect the members in one scan.
            return Ok(Self::Owners(
                scan::Candidates::walk(snap, lmpg.tenant, labels)?.collect(is_cancelled)?,
            ));
        };
        let members = walk_total(lmpg, snap, labels)?.max(1);
        let part = u64::try_from(PART_ORDINALS).unwrap_or(u64::MAX);
        let width = part.saturating_mul(end.div_ceil(members).max(1));
        Ok(Self::Walk { width, end })
    }

    fn parts(&self) -> usize {
        match self {
            Self::Owners(ords) => ords.len().div_ceil(PART_ORDINALS),
            Self::Walk { width, end } => {
                usize::try_from(end.div_ceil(*width)).unwrap_or(usize::MAX)
            }
        }
    }

    /// The candidates of part `part`, ascending.
    fn part(&self, scan: &Scan<'_>, part: usize) -> Result<Cow<'_, [u64]>, QueryError> {
        match self {
            Self::Owners(ords) => {
                let start = part.saturating_mul(PART_ORDINALS);
                let end = start.saturating_add(PART_ORDINALS).min(ords.len());
                Ok(Cow::Borrowed(&ords[start..end]))
            }
            Self::Walk { width, end } => {
                let start = u64::try_from(part)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(*width);
                let stop = start.saturating_add(*width).min(*end);
                Ok(Cow::Owned(scan::walk_range(
                    scan.snap,
                    scan.lmpg.tenant,
                    &scan.search.labels,
                    start..stop,
                )?))
            }
        }
    }

    /// Bytes a part's own candidates hold beside the shared ones.
    fn held(&self, ords: &[u64]) -> usize {
        match self {
            Self::Owners(_) => 0,
            Self::Walk { .. } => ords.len().saturating_mul(std::mem::size_of::<u64>()),
        }
    }
}

/// Check and fold one part's candidates; `own_candidates` is what the
/// part's candidate list holds.
fn run_part(
    scan: &Scan<'_>,
    shared: &Shared<'_>,
    ords: &[u64],
    own_candidates: usize,
) -> Result<PartOut, PartError> {
    let (lmpg, snap, budget, plan) = (scan.lmpg, scan.snap, scan.budget, scan.plan);
    let limits = lmpg.limits();
    let stopped = || shared.stop.load(Ordering::Relaxed) || limits.expired();
    let ctx = EvalCtx::at(lmpg, Arc::clone(snap), Some(&stopped));
    let mut session = ReconstructSession::new(snap, lmpg.tenant);
    let room = budget.map(|limit| limit.saturating_sub(shared.base));
    let mut working = WorkingSet {
        limit: room.map(|bytes| u64::try_from(bytes).unwrap_or(u64::MAX)),
        charged: 0,
    };
    let mut out = PartOut::default();
    let mut row: Vec<Option<Cell>> = Vec::with_capacity(plan.names.len());
    // This part's own retained bytes.
    let mut own = own_candidates;
    Budget::WorkingSet.check(budget, shared.base.saturating_add(own))?;
    for (i, &ord) in ords.iter().enumerate() {
        if i % 256 == 0 && stopped() {
            return Err(if shared.stop.load(Ordering::Relaxed) {
                PartError::Stopped
            } else {
                PartError::Failed(QueryError::Cancelled)
            });
        }
        if shared.check
            && !is_match(
                &ctx,
                scan.search,
                shared.predicate,
                shared.predicate_fields,
                &mut session,
                &mut working,
                ord,
            )?
        {
            continue;
        }
        out.matched += 1;
        let added = match shared.mode {
            Mode::Count => continue,
            Mode::Roots => {
                out.roots.push(ord);
                std::mem::size_of::<u64>()
            }
            Mode::Groups => {
                if !shared
                    .rows
                    .read(plan, snap, lmpg.tenant, &mut session, ord, &mut row)?
                {
                    continue;
                }
                // The row is transient: it must fit beside what this part
                // retains, but it is not retained itself.
                let row_bytes = row_bytes(&row);
                let held = shared.base.saturating_add(own).saturating_add(row_bytes);
                Budget::WorkingSet.check(budget, held)?;
                let alias_budget = budget.map(|limit| {
                    limit.saturating_sub(held) / plan.metric_count().saturating_add(1)
                });
                let row_ctx = RowCtx {
                    lmpg,
                    snap,
                    ord,
                    alias_budget,
                };
                out.groups.accumulate(plan, &row, &row_ctx)?
            }
        };
        if added > 0 {
            own = own.saturating_add(added);
            let total = shared
                .retained
                .fetch_add(added, Ordering::SeqCst)
                .saturating_add(added);
            Budget::WorkingSet.check(budget, shared.base.saturating_add(total))?;
        }
    }
    Ok(out)
}
