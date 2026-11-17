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

use crate::aggregate_rows::{row_bytes, Cell, CodedPlan, Partial, RowCtx, RowPlan};
use crate::columns::{self, Column};
use crate::matches::{related_bind, row_matches};
use crate::planner::index_candidates_with_budget;
use crate::{
    is_match, normalize, resolve_field, scan, walk_total, Budget, EvalCtx, Expr, Json, Lmpg,
    PointFields, QueryError, QuerySnapshot, SearchRequest, WorkingSet,
};
use serde_json::Map;

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
    /// Candidates checked against the residual predicate.
    pub(crate) checked: u64,
    /// Whether the pass read resident columns (its predicate's or its
    /// rows'): its memory differs from storage's pass, so a caller whose
    /// budget it fails reruns it storage-only.
    pub(crate) used_columns: bool,
    /// The matches' ordinals ([`Mode::Roots`]).
    pub(crate) roots: Vec<u64>,
    /// The matches' groups ([`Mode::Groups`]).
    pub(crate) groups: Partial,
    /// Bytes the candidate ordinals held.
    pub(crate) candidate_bytes: usize,
}

/// Resident columns serving some of a row's properties, with each part's
/// position in them.
pub(crate) struct ColumnRow<'a> {
    pub(crate) names: &'a [String],
    pub(crate) columns: &'a [Arc<Column>],
}

impl ColumnRow<'_> {
    /// The cells of `ord` into `out`, one per column; `false` when a column
    /// does not hold `ord` (the caller reads storage instead).
    fn cells(&self, ord: u64, cursor: &mut Vec<usize>, out: &mut Vec<Option<Cell>>) -> bool {
        cursor.resize(self.columns.len(), 0);
        out.clear();
        for (column, at) in self.columns.iter().zip(cursor.iter_mut()) {
            let Some(found) = column.seek(ord, *at) else {
                return false;
            };
            *at = found;
            out.push(column.cell(found));
        }
        true
    }

    /// The row a predicate reads of `ord`, as point reads would build it.
    fn row(
        &self,
        ord: u64,
        cursor: &mut Vec<usize>,
        cells: &mut Vec<Option<Cell>>,
    ) -> Option<Map<String, Json>> {
        if !self.cells(ord, cursor, cells) {
            return None;
        }
        let mut row = Map::new();
        for (name, cell) in self.names.iter().zip(cells.iter()) {
            if let Some(cell) = cell {
                row.insert(name.clone(), cell.to_json());
            }
        }
        Some(row)
    }
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
    checked: u64,
    roots: Vec<u64>,
    groups: Partial,
}

/// State the parts share.
struct Shared<'a> {
    check: bool,
    predicate: Option<&'a Expr>,
    predicate_fields: Option<&'a PointFields>,
    /// The predicate's properties from resident columns, when all are.
    predicate_columns: Option<ColumnRow<'a>>,
    rows: &'a Rows<'a>,
    mode: Mode,
    /// Groups folded from the row's resident columns by code, when the plan
    /// allows it ([`RowPlan::coded`]) and every column is resident.
    coded: Option<CodedPlan>,
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
    /// Every read property from resident columns, when all are.
    pub(crate) columns: Option<ColumnRow<'a>>,
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
        (cursor, out): (&mut Vec<usize>, &mut Vec<Option<Cell>>),
    ) -> Result<bool, QueryError> {
        if self
            .columns
            .as_ref()
            .is_some_and(|columns| columns.cells(ord, cursor, out))
        {
            return Ok(true);
        }
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
    storage_only: bool,
) -> Result<Pass, QueryError> {
    if storage_only {
        let storage_rows = Rows {
            points: rows.points,
            selected: rows.selected,
            columns: None,
        };
        let used_columns = std::cell::Cell::new(false);
        return run_with(
            scan,
            &storage_rows,
            mode,
            is_cancelled,
            (true, &used_columns),
        );
    }
    // Columns never refuse what storage answers: a pass that read columns
    // (the predicate's or the rows') and ran out of working set holds its
    // memory differently from storage's pass, so it is run again exactly the
    // way storage runs it, and refused only if that is.
    let used_columns = std::cell::Cell::new(false);
    match run_with(scan, rows, mode, is_cancelled, (false, &used_columns)) {
        Err(QueryError::ResourceLimit(reason))
            if used_columns.get() && reason.starts_with("query working set") =>
        {
            run(scan, rows, mode, is_cancelled, true)
        }
        other => other,
    }
}

/// The pass. Unless `storage_only`, the predicate is answered from columns
/// when they can (its candidates, or each row's check); `used_columns` is
/// set when the pass reads any column (the predicate's or the rows').
fn run_with(
    scan: &Scan<'_>,
    rows: &Rows<'_>,
    mode: Mode,
    is_cancelled: &dyn Fn() -> bool,
    (storage_only, used_columns): (bool, &std::cell::Cell<bool>),
) -> Result<Pass, QueryError> {
    let (lmpg, snap, search) = (scan.lmpg, scan.snap, scan.search);
    let (source, check, predicate, chosen) = choose_source(scan, is_cancelled, storage_only)?;
    let candidate_bytes = match &source {
        Source::Owners(ords) => ords.capacity().saturating_mul(std::mem::size_of::<u64>()),
        Source::Walk { .. } => 0,
    };
    Budget::WorkingSet.check(scan.budget, candidate_bytes)?;
    let predicate_fields = match (&predicate, check) {
        (Some(expr), true) => PointFields::for_predicate(snap, lmpg.tenant, &search.labels, expr)?,
        _ => None,
    };
    // The predicate's properties from resident columns.
    let served = columns::Served::lookup(
        snap,
        lmpg.tenant,
        (&search.labels, !search.related.is_empty()),
        predicate_fields.as_ref(),
    );
    let shared = Shared {
        check,
        predicate: predicate.as_ref(),
        predicate_fields: predicate_fields.as_ref(),
        predicate_columns: served
            .as_ref()
            .filter(|_| !storage_only)
            .and_then(columns::Served::row),
        rows,
        mode,
        coded: coded_plan(scan.plan, rows, mode),
        base: candidate_bytes,
        stop: AtomicBool::new(false),
        retained: AtomicUsize::new(0),
    };
    used_columns
        .set(chosen.from_columns || shared.predicate_columns.is_some() || rows.columns.is_some());
    let members = shared.members();
    let parts = source.parts();
    let mut pass = Pass {
        total: 0,
        checked: 0,
        used_columns: used_columns.get(),
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
                .part(scan, start + i, members)
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
                    pass.checked += out.checked;
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
    // Point reads of the predicate's properties, or the planner's
    // candidates for it, pay toward its columns.
    if let Some(served) = served.as_ref().or(chosen.served.as_ref()) {
        served.pay(snap, lmpg.tenant, pass.checked.max(chosen.planned), || {
            crate::walk_total(lmpg, snap, &search.labels)
        })?;
    }
    Ok(pass)
}

/// Groups are folded from the rows' resident columns by code when the plan
/// allows it ([`RowPlan::coded`]) and every column holds the same members.
fn coded_plan(plan: &RowPlan<'_>, rows: &Rows<'_>, mode: Mode) -> Option<CodedPlan> {
    let row = rows.columns.as_ref()?;
    let same = row
        .columns
        .windows(2)
        .all(|pair| pair[0].same_members(&pair[1]));
    (mode == Mode::Groups && same)
        .then(|| plan.coded())
        .flatten()
}

impl Shared<'_> {
    /// The label's members at this view, from a resident row column, when
    /// rows fold by code: a walk takes its candidates from them instead of
    /// reading the membership rows.
    fn members(&self) -> Option<&[u64]> {
        self.coded.as_ref()?;
        self.rows
            .columns
            .as_ref()
            .and_then(|row| row.columns.first())
            .map(|column| column.members())
    }
}

/// One part's state for folding rows by code: the position in the
/// columns, the cache from key codes to groups, and the row buffer.
#[derive(Default)]
struct Coded {
    from: usize,
    groups: columns::CodeMap<usize>,
    row: Vec<Option<Cell>>,
}

/// The context record `ord`'s row folds in: what its alias reads may hold
/// beside `held`, shared out over the metrics.
fn row_context<'a>(scan: &Scan<'a>, ord: u64, held: usize) -> RowCtx<'a> {
    RowCtx {
        lmpg: scan.lmpg,
        snap: scan.snap,
        ord,
        alias_budget: scan
            .budget
            .map(|limit| limit.saturating_sub(held) / scan.plan.metric_count().saturating_add(1)),
    }
}

/// Fold candidate `ord` from the row's resident columns by code; `None`
/// when the pass does not fold by code or `ord` is not a column member
/// (the caller builds the row). The transient row is charged as the built
/// row would be, beside `held` (what the pass and this part retain).
fn fold_coded(
    shared: &Shared<'_>,
    (scan, ord): (&Scan<'_>, u64),
    coded: &mut Coded,
    (held, groups): (usize, &mut Partial),
) -> Result<Option<usize>, QueryError> {
    let (Some(plan_coded), Some(row)) = (&shared.coded, &shared.rows.columns) else {
        return Ok(None);
    };
    let Some(at) = row
        .columns
        .first()
        .and_then(|first| first.seek(ord, coded.from))
    else {
        return Ok(None);
    };
    coded.from = at;
    let row_bytes = row.columns.iter().fold(0usize, |sum, column| {
        sum.saturating_add(std::mem::size_of::<Option<Cell>>())
            .saturating_add(column.heap_bytes(at))
    });
    let held = held.saturating_add(row_bytes);
    Budget::WorkingSet.check(scan.budget, held)?;
    let row_ctx = row_context(scan, ord, held);
    groups
        .accumulate_coded(
            (scan.plan, plan_coded),
            (row.columns, at),
            &mut coded.groups,
            &mut coded.row,
            &row_ctx,
        )
        .map(Some)
}

/// How the predicate was answered, for the rent it pays.
struct Chosen {
    /// The predicate's columns, when it reads only stored properties.
    served: Option<columns::Served>,
    /// Candidates the planner built for it (0: none).
    planned: u64,
    /// Whether the candidates came from the predicate's columns.
    from_columns: bool,
}

/// The pass's candidates: a predicate over the one label's stored
/// properties is answered from their resident columns when it can be (C2),
/// else by the planner or the walk.
fn choose_source(
    scan: &Scan<'_>,
    is_cancelled: &dyn Fn() -> bool,
    storage_only: bool,
) -> Result<(Source, bool, Option<Expr>, Chosen), QueryError> {
    let (lmpg, snap, search) = (scan.lmpg, scan.snap, scan.search);
    let served = match &search.where_ {
        Some(expr) if search.related.is_empty() && !reads_beyond_row(expr) => {
            let fields = PointFields::for_predicate(snap, lmpg.tenant, &search.labels, expr)?;
            columns::Served::lookup(snap, lmpg.tenant, (&search.labels, false), fields.as_ref())
        }
        _ => None,
    };
    let candidates = match (&served, &search.where_) {
        (Some(served), Some(expr)) if !storage_only => {
            let ctx = EvalCtx::at(lmpg, Arc::clone(snap), Some(is_cancelled));
            served.candidates(&ctx, expr, is_cancelled)?
        }
        _ => None,
    }
    // Columns never refuse what storage answers: candidates over the
    // working-set budget leave the predicate to the planner or the walk.
    .filter(|ords: &Vec<u64>| {
        let bytes = ords.capacity().saturating_mul(std::mem::size_of::<u64>());
        Budget::WorkingSet.check(scan.budget, bytes).is_ok()
    });
    let from_columns = candidates.is_some();
    let mut planned = 0;
    let (source, check, predicate) = match candidates {
        Some(ords) => (Source::Owners(ords), false, None),
        None => {
            match index_candidates_with_budget(lmpg, snap, search, scan.budget, is_cancelled)? {
                Some(plan) => {
                    planned = plan.ords.len();
                    (
                        Source::Owners(plan.ords.into_iter().collect()),
                        !plan.exact,
                        plan.residual,
                    )
                }
                None => (
                    Source::walk(lmpg, snap, &search.labels, is_cancelled)?,
                    search.where_.is_some() || !search.related.is_empty(),
                    search.where_.clone(),
                ),
            }
        }
    };
    Ok((
        source,
        check,
        predicate,
        Chosen {
            served,
            planned,
            from_columns,
        },
    ))
}

/// Whether `expr` reads more than the record's own stored properties
/// (relationships or the record id): such a predicate is not answered from
/// columns.
fn reads_beyond_row(expr: &Expr) -> bool {
    match expr {
        Expr::And(items) | Expr::Or(items) | Expr::Nor(items) => items.iter().any(reads_beyond_row),
        Expr::Xor(a, b) => reads_beyond_row(a) || reads_beyond_row(b),
        Expr::Not(inner) => reads_beyond_row(inner),
        Expr::Related(_) | Expr::Field(crate::FieldPred::Id(_)) => true,
        Expr::Field(_) => false,
    }
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

    /// The candidates of part `part`, ascending. A walk takes them from
    /// `members` (the label's members, from a resident column) when given,
    /// else from the membership rows.
    fn part<'s>(
        &'s self,
        scan: &Scan<'_>,
        part: usize,
        members: Option<&'s [u64]>,
    ) -> Result<Cow<'s, [u64]>, QueryError> {
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
                if let Some(members) = members {
                    let from = members.partition_point(|&ord| ord < start);
                    let to = members.partition_point(|&ord| ord < stop);
                    return Ok(Cow::Borrowed(&members[from..to]));
                }
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
    // Positions in the resident columns: candidates ascend within a part.
    let (mut row_cursor, mut predicate_cursor) = (Vec::new(), Vec::new());
    let mut coded = Coded::default();
    let mut predicate_cells: Vec<Option<Cell>> = Vec::new();
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
        if shared.check {
            out.checked += 1;
            let from_columns = match (&shared.predicate_columns, shared.predicate) {
                (Some(columns), Some(expr)) => columns
                    .row(ord, &mut predicate_cursor, &mut predicate_cells)
                    .map(|obj| row_matches(&ctx, expr, &obj, &mut working, ord)),
                _ => None,
            };
            let matched = match from_columns {
                Some(matched) => matched? && related_bind(&ctx, scan.search, ord)?,
                None => is_match(
                    &ctx,
                    scan.search,
                    shared.predicate,
                    shared.predicate_fields,
                    &mut session,
                    &mut working,
                    ord,
                )?,
            };
            if !matched {
                continue;
            }
        }
        out.matched += 1;
        let added = match shared.mode {
            Mode::Count => continue,
            Mode::Roots => {
                out.roots.push(ord);
                std::mem::size_of::<u64>()
            }
            Mode::Groups => {
                if let Some(added) = fold_coded(
                    shared,
                    (scan, ord),
                    &mut coded,
                    (shared.base.saturating_add(own), &mut out.groups),
                )? {
                    added
                } else {
                    if !shared.rows.read(
                        plan,
                        snap,
                        lmpg.tenant,
                        &mut session,
                        ord,
                        (&mut row_cursor, &mut row),
                    )? {
                        continue;
                    }
                    // The row is transient: it must fit beside what this part
                    // retains, but it is not retained itself.
                    let row_bytes = row_bytes(&row);
                    let held = shared.base.saturating_add(own).saturating_add(row_bytes);
                    Budget::WorkingSet.check(budget, held)?;
                    out.groups
                        .accumulate(plan, &row, &row_context(scan, ord, held))?
                }
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
