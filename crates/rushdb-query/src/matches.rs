//! The records-page pipeline: plan a page (the predicate planner, then the
//! order planner on its numbers), pick its candidates, and keep the
//! matches the page needs, in query order.

use rushdb_lmpg::reconstruct;
use rushdb_lmpg::Lmpg;
use serde_json::{Map, Value as Json};

use crate::budget::WorkingSet;
use crate::planner::index_candidates_with_budget;
use crate::point_fields::{self, PointFields};
use crate::{
    check_walk_budget, eval_expr_ctx, normalize, order, planner, related_exists, scan,
    sort_ordered_rows, working_set_budget, EvalCtx, Expr, QueryError, QuerySnapshot, SearchRequest,
};

/// The matches a search keeps, and how many there are in all.
pub(crate) struct Matches {
    /// Matches in query order, from position `offset` of the full order:
    /// the first `skip + limit`, or (read from the far end) the page itself.
    pub(crate) retained: Vec<u64>,
    /// Position of `retained[0]` among every match in query order.
    pub(crate) offset: usize,
    /// Every match, retained or not.
    pub(crate) total: u64,
}

/// The matches of `req` in query order, enough for the page
/// `skip..skip + limit` (`limit` `usize::MAX`: every match), and how many
/// there are in all.
///
/// The page is read predicate-first (every candidate of the filter, the
/// best `skip + limit` kept) unless the order planner ([`order::plan_order`])
/// finds a source already in the requested order that reaches the page in
/// fewer reads. Such a walk gives up when it passes more rows than
/// predicate-first reads, and the page is read predicate-first, so a wrong
/// estimate costs at most twice the plain read.
///
/// The working set (the candidate owner set, retained ordinals, ordering
/// rows and the record being evaluated) is charged against
/// `query-memory-max`.
pub(crate) fn run_matches(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &QuerySnapshot,
    skip: usize,
    limit: usize,
    known_total: Option<u64>,
) -> Result<Matches, QueryError> {
    check_walk_budget(lmpg, snap, req)?;
    let page = (skip, limit);
    if let Some(matches) = run_pass(lmpg, req, is_cancelled, snap, page, known_total, true)? {
        return Ok(matches);
    }
    Ok(
        run_pass(lmpg, req, is_cancelled, snap, page, known_total, false)?
            .expect("a predicate-first pass always completes"),
    )
}

/// One [`run_matches`] pass; `None` when its order-first walk (allowed by
/// `order_first`) gave up.
pub(crate) fn run_pass(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &QuerySnapshot,
    (skip, limit): (usize, usize),
    known_total: Option<u64>,
    order_first: bool,
) -> Result<Option<Matches>, QueryError> {
    let mut working = WorkingSet {
        limit: working_set_budget(lmpg).map(|bytes| u64::try_from(bytes).unwrap_or(u64::MAX)),
        charged: 0,
    };
    let CandidateSource {
        phases,
        exact,
        known,
        predicate,
        member_of,
        plan,
    } = candidate_source(
        lmpg,
        req,
        is_cancelled,
        snap,
        (skip, limit),
        known_total,
        order_first,
        &mut working,
    )?;
    let mirror = plan.as_ref().is_some_and(order::OrderPlan::mirror);
    let budget = plan.as_ref().map(|plan| plan.predicate_first_reads);
    let ctx = EvalCtx::at(lmpg, snap.clone(), Some(is_cancelled));
    let mut session = reconstruct::ReconstructSession::new(snap, lmpg.tenant);
    let (order_fields, predicate_fields) =
        point_reads(snap, lmpg.tenant, req, &phases, predicate.as_ref())?;
    let mut kept = Kept::new(mirror, known, skip, limit);
    let mut total: u64 = 0;
    let mut examined: u64 = 0;
    for Phase {
        mut candidates,
        sorted,
    } in phases
    {
        if known.is_some() && kept.full() {
            break;
        }
        let mut ordered = Sorted::new(kept.keep - kept.count.min(kept.keep));
        while let Some(ord) = candidates.next(is_cancelled)? {
            if is_cancelled() {
                return Err(QueryError::Cancelled);
            }
            examined += 1;
            if budget.is_some_and(|budget| examined + candidates.passed() > budget) {
                return Ok(None);
            }
            if member_of.as_ref().is_some_and(|set| !set.contains(ord)) {
                continue;
            }
            if !exact
                && !is_match(
                    &ctx,
                    req,
                    predicate.as_ref(),
                    predicate_fields.as_ref(),
                    &mut session,
                    &mut working,
                    ord,
                )?
            {
                continue;
            }
            total += 1;
            if sorted {
                let row = read_row(order_fields.as_ref(), &mut session, snap, lmpg.tenant, ord)?;
                ordered.push(ord, row, req, &mut working)?;
            } else if !kept.push(ord, &mut working)? && known.is_some() {
                break;
            }
        }
        if candidates.gave_up() {
            return Ok(None);
        }
        examined += candidates.passed();
        if sorted {
            for ord in ordered.finish(req) {
                kept.count += 1;
                kept.retained.push_back(ord);
            }
        }
    }
    let (retained, offset) = if mirror {
        (kept.retained.into_iter().rev().collect(), skip)
    } else {
        (kept.retained.into(), 0)
    };
    Ok(Some(Matches {
        retained,
        offset,
        total: known.unwrap_or(total),
    }))
}

/// The point reads of a pass: the order fields (when a phase is sorted) and
/// the fields the residual predicate reads; `None` where the whole record
/// is needed.
fn point_reads(
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    req: &SearchRequest,
    phases: &[Phase<'_>],
    predicate: Option<&Expr>,
) -> Result<(Option<PointFields>, Option<PointFields>), QueryError> {
    let order_fields = if phases.iter().any(|phase| phase.sorted) {
        PointFields::resolve_in(
            snap,
            tenant,
            &req.labels,
            req.order_by.iter().map(|(f, _)| f.as_str()),
        )?
    } else {
        None
    };
    let predicate_fields = match predicate {
        Some(expr) => PointFields::for_predicate(snap, tenant, &req.labels, expr)?,
        None => None,
    };
    Ok((order_fields, predicate_fields))
}

/// Matches kept as they come, in query (or far-end) order.
struct Kept {
    retained: std::collections::VecDeque<u64>,
    count: usize,
    /// Matches wanted in walk order.
    keep: usize,
    /// From the far end, only the page's rows (this many) are held.
    page: Option<usize>,
}

impl Kept {
    /// Matches to keep in walk order: from the far end (`mirror`), the rows
    /// up to the page `skip..skip + limit`, holding only the page's; else
    /// the first `skip + limit`.
    fn new(mirror: bool, known: Option<u64>, skip: usize, limit: usize) -> Self {
        let keep = if mirror {
            usize::try_from(known.unwrap_or(0))
                .unwrap_or(usize::MAX)
                .saturating_sub(skip)
        } else {
            skip.saturating_add(limit)
        };
        Self {
            retained: std::collections::VecDeque::new(),
            count: 0,
            keep,
            page: mirror.then_some(limit),
        }
    }

    fn full(&self) -> bool {
        self.count >= self.keep
    }

    /// Keep `ord` if more are wanted; `false` when the page was full.
    fn push(&mut self, ord: u64, working: &mut WorkingSet) -> Result<bool, QueryError> {
        if self.full() {
            return Ok(false);
        }
        self.count += 1;
        self.retained.push_back(ord);
        if self.page.is_some_and(|page| self.retained.len() > page) {
            self.retained.pop_front();
        } else {
            working.charge(8, true)?;
        }
        Ok(true)
    }
}

/// A sorted phase's best rows `(ordinal, order fields)`, sorted and cut back
/// to what the page still needs whenever they double, so memory follows the
/// page, not the matches.
struct Sorted {
    rows: Vec<OrderRow>,
    bytes: Vec<u64>,
    wanted: usize,
}

impl Sorted {
    fn new(wanted: usize) -> Self {
        Self {
            rows: Vec::new(),
            bytes: Vec::new(),
            wanted,
        }
    }

    fn push(
        &mut self,
        ord: u64,
        row: Map<String, Json>,
        req: &SearchRequest,
        working: &mut WorkingSet,
    ) -> Result<(), QueryError> {
        let bytes =
            serde_json::to_vec(&row).map_or(0, |v| u64::try_from(v.len()).unwrap_or(u64::MAX));
        working.charge(bytes, true)?;
        self.rows.push((ord, row));
        self.bytes.push(bytes);
        if self.rows.len() >= self.wanted.saturating_mul(2).max(64) {
            working.charged = working.charged.saturating_sub(cut_ordered(
                &mut self.rows,
                &mut self.bytes,
                req,
                self.wanted,
            ));
        }
        Ok(())
    }

    /// The best `wanted` ordinals, in query order.
    fn finish(mut self, req: &SearchRequest) -> impl Iterator<Item = u64> {
        cut_ordered(&mut self.rows, &mut self.bytes, req, self.wanted);
        self.rows.into_iter().map(|(ord, _)| ord)
    }
}

/// Whether candidate `ord` matches: `predicate` holds on it and every
/// relationship branch binds. Candidates are live records (a label's
/// members, the tenant's record headers, or the index plan's owners, all
/// written and deleted in the commit that writes and deletes the record),
/// so the record header is not read again here.
pub(crate) fn is_match(
    ctx: &EvalCtx<'_>,
    req: &SearchRequest,
    predicate: Option<&Expr>,
    fields: Option<&PointFields>,
    session: &mut reconstruct::ReconstructSession<'_, QuerySnapshot>,
    working: &mut WorkingSet,
    ord: u64,
) -> Result<bool, QueryError> {
    let (snap, tenant) = (&ctx.snap, ctx.lmpg.tenant);
    if let Some(expr) = predicate {
        let obj = read_row(fields, session, snap, tenant, ord)?;
        let bytes =
            serde_json::to_vec(&obj).map_or(0, |v| u64::try_from(v.len()).unwrap_or(u64::MAX));
        working.charge(bytes, false)?;
        if !eval_expr_ctx(ctx, expr, &obj, ord)? {
            return Ok(false);
        }
    }
    for spec in &req.related {
        if !related_exists(snap, tenant, ord, spec)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The properties `predicate` reads of record `ord`, decoded through
/// `reader` (which may meter the bytes): the predicate's own fields by
/// name, or the whole record when it needs it (`$id`, a traversal, an
/// array-typed property). A record the predicate cannot see is empty.
pub(crate) fn filtered_row<R: rushdb_lmpg::reader::TryRead>(
    reader: &R,
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    predicate: &Expr,
    ord: u64,
) -> Result<Map<String, Json>, QueryError> {
    let mut session = reconstruct::ReconstructSession::new(reader, tenant);
    let mut names = std::collections::BTreeSet::new();
    let record = if point_fields::predicate_fields(predicate, &mut names) {
        let selected: std::collections::HashSet<String> = names
            .iter()
            .flat_map(|name| [name.clone(), normalize(name)])
            .collect();
        session.reconstruct_record_selected(ord, &selected)?
    } else {
        session.reconstruct_record(ord)?
    };
    let _ = snap;
    Ok(match record {
        Some(Json::Object(map)) => map,
        _ => Map::new(),
    })
}

/// The properties `fields` names, by point reads, or the rebuilt record.
pub(crate) fn read_row(
    fields: Option<&PointFields>,
    session: &mut reconstruct::ReconstructSession<'_, QuerySnapshot>,
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ord: u64,
) -> Result<Map<String, Json>, QueryError> {
    match fields {
        Some(fields) => fields.row(snap, tenant, ord),
        None => Ok(session
            .reconstruct_record(ord)?
            .and_then(|record| match record {
                Json::Object(map) => Some(map),
                _ => None,
            })
            .unwrap_or_default()),
    }
}

/// A page's plan: the predicate planner's owner set, what is known before
/// reading, and the order-first plan if one reads the page in fewer rows.
pub(crate) struct PagePlan {
    index_plan: Option<planner::IndexPlan>,
    exact: bool,
    known: Option<u64>,
    predicate: Option<Expr>,
    order: Option<order::OrderPlan>,
}

/// Plan the page `skip..skip + limit` of `req`: the predicate planner first,
/// then, when `order_first` is allowed, the order planner on its numbers.
pub(crate) fn plan_page(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &QuerySnapshot,
    (skip, limit): (usize, usize),
    known_total: Option<u64>,
    order_first: bool,
) -> Result<PagePlan, QueryError> {
    let index_plan =
        index_candidates_with_budget(lmpg, snap, req, working_set_budget(lmpg), is_cancelled)?;
    let (exact, known, predicate, candidates) = if let Some(plan) = &index_plan {
        let known = known_total
            .or(plan.known_total)
            .or_else(|| plan.exact.then(|| plan.ords.len()));
        (plan.exact, known, plan.residual.clone(), plan.ords.len())
    } else {
        let exact = req.where_.is_none() && req.related.is_empty();
        let records = walk_total(lmpg, snap, &req.labels)?;
        let known = known_total.or_else(|| exact.then_some(records));
        (exact, known, req.where_.clone(), records)
    };
    // An owner set cut to the page (the bounded `$exists` plan) holds only
    // the first matches: an order-first walk would check membership in it.
    let whole_set = index_plan
        .as_ref()
        .is_none_or(|plan| plan.known_total.is_none());
    let order = if order_first && whole_set {
        order::plan_order(
            snap.as_ref(),
            lmpg.tenant,
            &order::PageShape {
                labels: &req.labels,
                order_by: &req.order_by,
                skip,
                limit,
                matches: known,
                candidates,
            },
        )?
    } else {
        None
    };
    Ok(PagePlan {
        index_plan,
        exact,
        known,
        predicate,
        order,
    })
}

/// How a page ordered by a single key is read when a source already in
/// that order wins over reading every candidate (`describe.explain`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderExplain {
    /// `storage` (record creation order), `id` or `property`.
    pub source: &'static str,
    /// The page is read from the far end of the order and reversed.
    pub reverse_walk: bool,
    /// Estimated rows the ordered walk reads.
    pub estimated_reads: u64,
    /// Rows reading every candidate would take.
    pub predicate_first_reads: u64,
}

/// The order-first plan of the page `skip..skip + limit` of `req`, if one
/// wins; it does not read the page.
///
/// # Errors
/// Returns storage read errors from the planners' counters.
pub fn explain_order(
    lmpg: &Lmpg,
    req: &SearchRequest,
    snap: &QuerySnapshot,
    skip: usize,
    limit: usize,
) -> Result<Option<OrderExplain>, QueryError> {
    let plan = plan_page(lmpg, req, &|| false, snap, (skip, limit), None, true)?;
    Ok(plan.order.map(|order| OrderExplain {
        source: order.source(),
        reverse_walk: order.mirror(),
        estimated_reads: order.order_first_reads,
        predicate_first_reads: order.predicate_first_reads,
    }))
}

/// One stretch of a search's candidates.
pub(crate) struct Phase<'v> {
    candidates: scan::Candidates<'v>,
    /// The candidates are not in the requested order: their best rows are
    /// kept by reading and sorting their order fields. Otherwise they are
    /// (or no order is requested) and are kept as they come.
    sorted: bool,
}

/// Where a search's candidates come from and what is left to check.
pub(crate) struct CandidateSource<'v> {
    /// The candidates, one stretch after another.
    phases: Vec<Phase<'v>>,
    /// Every candidate is a live match: neither the predicate nor the
    /// record's existence is checked per candidate.
    exact: bool,
    /// The total, when known without counting.
    known: Option<u64>,
    /// What is left to check per candidate: the conjuncts the owner set does
    /// not answer, or the whole `where` on a walk.
    predicate: Option<Expr>,
    /// The index plan's owner set, checked per candidate when the candidates
    /// come from an order-first walk instead of the set.
    member_of: Option<planner::OwnerSet>,
    /// The order-first plan the phases follow, if one won.
    plan: Option<order::OrderPlan>,
}

/// The candidates of a search: the index plan's owner set (charged to
/// `working`) or a walk of the tenant's or labels' records, unless
/// `order_first` is allowed and an order-first plan reads the page in fewer
/// rows ([`order::plan_order`]).
#[expect(
    clippy::too_many_arguments,
    reason = "the pass's inputs, each used once to choose its source"
)]
pub(crate) fn candidate_source<'v>(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &'v QuerySnapshot,
    (skip, limit): (usize, usize),
    known_total: Option<u64>,
    order_first: bool,
    working: &mut WorkingSet,
) -> Result<CandidateSource<'v>, QueryError> {
    let PagePlan {
        index_plan,
        exact,
        known,
        predicate,
        order: plan,
    } = plan_page(
        lmpg,
        req,
        is_cancelled,
        snap,
        (skip, limit),
        known_total,
        order_first,
    )?;
    if let Some(plan) = &index_plan {
        working.charge(
            u64::try_from(plan.ords.serialized_size()).unwrap_or(u64::MAX),
            true,
        )?;
    }
    let in_order = |candidates| Phase {
        candidates,
        sorted: false,
    };
    let Some(plan) = plan else {
        let candidates = match index_plan {
            Some(plan) => scan::Candidates::from_set(plan.ords),
            None => scan::Candidates::walk(snap, lmpg.tenant, &req.labels)?,
        };
        return Ok(CandidateSource {
            phases: vec![Phase {
                candidates,
                sorted: !req.order_by.is_empty(),
            }],
            exact,
            known,
            predicate,
            member_of: None,
            plan: None,
        });
    };
    let (phases, member_of) = match &plan.path {
        order::OrderPath::StorageMirror => {
            let candidates = match index_plan {
                Some(plan) => scan::Candidates::from_set_descending(plan.ords),
                None => scan::Candidates::walk_ordered(snap, lmpg.tenant, &req.labels, true)?,
            };
            (vec![in_order(candidates)], None)
        }
        order::OrderPath::Id {
            walk_descending, ..
        } => (
            vec![in_order(scan::Candidates::by_id(
                snap,
                lmpg.tenant,
                &req.labels,
                *walk_descending,
            )?)],
            index_plan.map(|plan| plan.ords),
        ),
        order::OrderPath::Property {
            descending,
            pids,
            holding,
            scope,
            missing,
            index,
        } => {
            let phases = property_phases(
                snap,
                lmpg.tenant,
                *descending,
                (pids, holding, scope),
                *missing,
                index.as_ref(),
            )?;
            (phases, index_plan.map(|plan| plan.ords))
        }
    };
    let mut phases = phases;
    for phase in &mut phases {
        // Past the rows predicate-first reads, the walk is not worth it.
        phase.candidates.give_up_after(plan.predicate_first_reads);
    }
    Ok(CandidateSource {
        phases,
        exact,
        known,
        predicate,
        member_of,
        plan: Some(plan),
    })
}

/// One ordering row: a match's ordinal and the properties it is ordered by.
pub(crate) type OrderRow = (u64, Map<String, Json>);

/// Sort the ordering buffer and keep its best `keep` rows; returns the bytes
/// of the rows dropped.
pub(crate) fn cut_ordered(
    ordered: &mut Vec<OrderRow>,
    bytes: &mut Vec<u64>,
    req: &SearchRequest,
    keep: usize,
) -> u64 {
    let mut rows: Vec<(OrderRow, u64)> = ordered.drain(..).zip(bytes.drain(..)).collect();
    // Candidates arrive in ascending ordinal order and the sort is stable,
    // so ties keep that order across cuts, exactly as one sort of every
    // match would.
    sort_ordered_rows(&mut rows, req);
    let freed = rows.iter().skip(keep).map(|(_, b)| *b).sum();
    rows.truncate(keep);
    for (row, b) in rows {
        ordered.push(row);
        bytes.push(b);
    }
    freed
}

/// Live records of the tenant (`labels` empty) or of `labels`, from counters.
pub(crate) fn walk_total(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    labels: &[String],
) -> Result<u64, QueryError> {
    if labels.is_empty() {
        return Ok(rushdb_lmpg::derived::tenant_usage(snap.as_ref(), lmpg.tenant)?.records);
    }
    label_total(snap.as_ref(), lmpg.tenant, labels)
}

/// Live records of `labels`, from their counters.
pub(crate) fn label_total<R: rushdb_lmpg::reader::TryRead + ?Sized>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    labels: &[String],
) -> Result<u64, QueryError> {
    let distinct: std::collections::BTreeSet<&String> = labels.iter().collect();
    let mut total = 0u64;
    for label in distinct {
        total = total.saturating_add(rushdb_lmpg::derived::label_count(snap, tenant, label)?);
    }
    Ok(total)
}

/// The phases of a property order: the records missing the property (first
/// ascending, last descending) and its holders, read and sorted, or (with
/// an index order) its nulls and its indexed values, each already in order.
fn property_phases<'v>(
    snap: &'v QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    descending: bool,
    (pids, holding, scope): (&[u64], &std::collections::BTreeSet<String>, &[String]),
    missing: u64,
    index: Option<&order::IndexOrder>,
) -> Result<Vec<Phase<'v>>, QueryError> {
    let in_order = |candidates| Phase {
        candidates,
        sorted: false,
    };
    let missing = in_order(scan::Candidates::missing(
        snap,
        tenant,
        scope,
        holding.clone(),
        pids.to_vec(),
        missing,
    )?);
    let presence = |nullness: &[u8]| {
        let mut prefixes = Vec::new();
        for label in holding {
            for &pid in pids {
                for &nullness in nullness {
                    prefixes.push(rushdb_types::keys::presence_nullness_prefix(
                        tenant,
                        rushdb_types::ids::OwnerKind::Record,
                        label,
                        pid,
                        nullness,
                    ));
                }
            }
        }
        scan::Candidates::presence(snap, &prefixes)
    };
    // Missing values sort first ascending and last descending.
    let phases = match index {
        None => {
            let holders = Phase {
                candidates: presence(&[
                    rushdb_types::keys::NULLNESS_VALUE,
                    rushdb_types::keys::NULLNESS_NULL,
                ])?,
                sorted: true,
            };
            if descending {
                vec![holders, missing]
            } else {
                vec![missing, holders]
            }
        }
        Some(index) => {
            // Nulls sort before numbers and after strings; the
            // index walks the values themselves in order.
            let nulls = in_order(presence(&[rushdb_types::keys::NULLNESS_NULL])?);
            let values = in_order(scan::Candidates::index_order(
                snap, tenant, &index.ids, descending,
            )?);
            let mut phases = if index.numeric {
                vec![missing, nulls, values]
            } else {
                vec![missing, values, nulls]
            };
            if descending {
                phases.reverse();
            }
            phases
        }
    };
    Ok(phases)
}
