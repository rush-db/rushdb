//! `$relation` traversal (with `hops`): does a record reach a target record
//! along relationships of one type and direction, over a trail of
//! `min..=max` hops?
//!
//! **Semantics.** A trail uses no relationship twice (as in Cypher
//! variable-length patterns). Every record on the trail after the start,
//! the target included, carries one of the block's labels (when it names
//! any); the target also satisfies the block's filter. The start record
//! itself is a target only through a closed trail (a cycle of distinct
//! relationships).
//!
//! **Two plans** answer the same question:
//!
//! * [`reaching_owners`] resolves the targets once (through the planner, so
//!   an indexed filter is an index lookup) and expands *backwards* from them
//!   breadth-first: the cost follows the targets' neighbourhood, not the
//!   label. The planner picks it when the targets are fewer than the
//!   records the root would otherwise check.
//! * [`trail_exists`] checks one start record, expanding forward; used when
//!   the root's other conditions already leave few candidates, or when the
//!   block sits under `$or`/`$not`.
//!
//! A shortest path is always a trail, so breadth-first distance answers
//! every record whose nearest target is `min..=max` hops away. Only a
//! record whose nearest target is closer than `min` hops, or a target
//! checked against itself, needs more: the shortest closed trail through
//! it, then (only if that is still too short) a depth-first search over
//! trails within [`TRAIL_SEARCH_BUDGET`] steps.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use rushdb_lmpg::reader::try_access as access;
use rushdb_storage::ReadView;
use rushdb_types::ids::TenantId;

use crate::adjacency::{Adjacency, TypeAdjacency};
use crate::planner::{extend_owners, OwnerSet};
use crate::{
    eval_expr_ctx, filtered_row, run_matches, EvalCtx, Lmpg, QueryError, QuerySnapshot,
    RelatedExpr, SearchRequest,
};

/// Trail expansions one check may spend when breadth-first distances cannot
/// answer it (a target closer than `min` hops).
pub(crate) const TRAIL_SEARCH_BUDGET: usize = 100_000;

/// Records one breadth-first search may reach: past it the check fails
/// rather than hold (and walk) an unbounded part of the graph.
pub(crate) const TRAIL_VISIT_BUDGET: usize = 1_000_000;

fn visits_exceeded() -> QueryError {
    QueryError::ResourceLimit(
        "$relation hops search reaches more records than its budget; narrow hops or add a filter"
            .into(),
    )
}

/// Whether a record carries one of `labels` (any record when empty), read
/// from its header once per query.
pub(crate) struct LabelGate<'a> {
    snap: &'a ReadView,
    tenant: TenantId,
    labels: &'a [String],
    seen: RefCell<HashMap<u64, bool>>,
}

impl<'a> LabelGate<'a> {
    pub(crate) fn new(snap: &'a ReadView, tenant: TenantId, labels: &'a [String]) -> Self {
        Self {
            snap,
            tenant,
            labels,
            seen: RefCell::default(),
        }
    }

    pub(crate) fn admits(&self, ord: u64) -> Result<bool, QueryError> {
        if self.labels.is_empty() {
            return Ok(true);
        }
        if let Some(&known) = self.seen.borrow().get(&ord) {
            return Ok(known);
        }
        let admits = access::record_by_ord(self.snap, self.tenant, ord)?
            .is_some_and(|header| self.labels.contains(&header.label));
        self.seen.borrow_mut().insert(ord, admits);
        Ok(admits)
    }
}

/// The question one trail check answers.
pub(crate) struct TrailCheck<'a> {
    pub(crate) adjacency: &'a dyn Adjacency,
    /// Hop bounds, `1 <= min <= max`.
    pub(crate) min: u64,
    pub(crate) max: u64,
    /// May the trail pass through (or end at) this record?
    pub(crate) passable: &'a dyn Fn(u64) -> Result<bool, QueryError>,
    /// Is this record a target? Only asked of passable records.
    pub(crate) target: &'a dyn Fn(u64) -> Result<bool, QueryError>,
    pub(crate) cancel: &'a dyn Fn() -> Result<(), QueryError>,
    /// Records a breadth-first search may reach ([`TRAIL_VISIT_BUDGET`]).
    pub(crate) visits: usize,
}

/// Whether a trail of `min..=max` hops leads forward from `start` to a
/// target (`start` itself only over a closed trail).
pub(crate) fn trail_exists(check: &TrailCheck<'_>, start: u64) -> Result<bool, QueryError> {
    let start_is_target = (check.passable)(start)? && (check.target)(start)?;
    let undirected = check.adjacency.undirected();
    // node -> (hops from `start`, first record after `start`, relationship
    // the breadth-first tree entered it by)
    let mut seen: HashMap<u64, (u64, u64, Option<u64>)> = HashMap::new();
    seen.insert(start, (0, start, None));
    let mut queue = VecDeque::from([start]);
    let mut too_short = false;
    let mut edges = Vec::new();
    while let Some(node) = queue.pop_front() {
        (check.cancel)()?;
        let (depth, branch, entered_by) = seen[&node];
        if depth >= check.max {
            continue;
        }
        edges.clear();
        check.adjacency.neighbours(node, true, &mut edges)?;
        for &(other, rel) in &edges {
            if other == start {
                // Back at the start: a closed trail unless it reuses the
                // relationship the tree came by (undirected).
                if start_is_target && (!undirected || entered_by != Some(rel)) {
                    if depth + 1 >= check.min {
                        return Ok(true);
                    }
                    too_short = true;
                }
                continue;
            }
            match seen.get(&other) {
                None => {
                    if !(check.passable)(other)? {
                        continue;
                    }
                    let branch = if node == start { other } else { branch };
                    if seen.len() >= check.visits {
                        return Err(visits_exceeded());
                    }
                    seen.insert(other, (depth + 1, branch, Some(rel)));
                    if (check.target)(other)? {
                        if depth + 1 >= check.min {
                            return Ok(true);
                        }
                        too_short = true;
                    }
                    queue.push_back(other);
                }
                // Two branches from the start meet: a simple cycle through
                // it, of `depth + other_depth + 1` hops.
                Some(&(other_depth, other_branch, other_entered)) => {
                    if start_is_target
                        && undirected
                        && other_branch != branch
                        && entered_by != Some(rel)
                        && other_entered != Some(rel)
                        && depth + other_depth < check.max
                    {
                        if depth + other_depth + 1 >= check.min {
                            return Ok(true);
                        }
                        too_short = true;
                    }
                }
            }
        }
    }
    if too_short {
        return longer_trail(check, start, start_is_target);
    }
    Ok(false)
}

/// One depth-first frame: a record, its `(neighbour, relationship)` pairs
/// and the next pair to try.
type TrailFrame = (u64, Vec<(u64, u64)>, usize);

/// A trail of `min..=max` hops to a target when every one breadth-first
/// search finds is shorter than `min`: depth-first over trails, within the
/// step budget.
fn longer_trail(
    check: &TrailCheck<'_>,
    start: u64,
    start_is_target: bool,
) -> Result<bool, QueryError> {
    let mut used: Vec<u64> = Vec::new();
    let mut steps = 0usize;
    let mut first = Vec::new();
    check.adjacency.neighbours(start, true, &mut first)?;
    let mut work: Vec<TrailFrame> = vec![(start, first, 0)];
    while let Some((_, edges, at)) = work.last_mut() {
        let Some(&(other, rel)) = edges.get(*at) else {
            work.pop();
            used.pop();
            continue;
        };
        *at += 1;
        if used.contains(&rel) {
            continue;
        }
        steps += 1;
        if steps > TRAIL_SEARCH_BUDGET {
            return Err(QueryError::ResourceLimit(
                "$relation hops search exceeds its work budget; narrow hops or add a filter".into(),
            ));
        }
        if steps.is_multiple_of(1_024) {
            (check.cancel)()?;
        }
        let depth = work.len() as u64;
        let at_target = if other == start {
            start_is_target
        } else {
            (check.passable)(other)? && (check.target)(other)?
        };
        if at_target && depth >= check.min {
            return Ok(true);
        }
        if depth < check.max && (check.passable)(other)? {
            let mut next = Vec::new();
            check.adjacency.neighbours(other, true, &mut next)?;
            used.push(rel);
            work.push((other, next, 0));
        }
    }
    Ok(false)
}

/// Every record of `root_labels` (any record when empty) that reaches one
/// of `targets` over a trail of `min..=max` hops, expanding backwards from
/// the targets. `targets` must already satisfy the block's labels and
/// filter.
pub(crate) fn reaching_owners(
    check: &TrailCheck<'_>,
    targets: &OwnerSet,
    root: &dyn Fn(u64) -> Result<bool, QueryError>,
    owner_limit: usize,
) -> Result<OwnerSet, QueryError> {
    let mut out = OwnerSet::new();
    let mut hops: HashMap<u64, u64> = targets.iter().map(|ord| (ord, 0)).collect();
    // Records nearer than `min` hops to a target, and the targets
    // themselves: breadth-first distance does not decide them.
    let mut undecided: Vec<u64> = targets.iter().collect();
    let mut frontier: Vec<u64> = targets.iter().collect();
    let mut edges = Vec::new();
    for depth in 1..=check.max {
        let mut next = Vec::new();
        for &node in &frontier {
            (check.cancel)()?;
            edges.clear();
            check.adjacency.neighbours(node, false, &mut edges)?;
            for &(other, _) in &edges {
                if hops.contains_key(&other) {
                    continue;
                }
                if hops.len() >= check.visits {
                    return Err(visits_exceeded());
                }
                hops.insert(other, depth);
                if depth >= check.min {
                    if root(other)? {
                        extend_owners(&mut out, [other], owner_limit)?;
                    }
                } else {
                    undecided.push(other);
                }
                if depth < check.max && (check.passable)(other)? {
                    next.push(other);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    for ord in undecided {
        if !out.contains(ord) && root(ord)? && trail_exists(check, ord)? {
            extend_owners(&mut out, [ord], owner_limit)?;
        }
    }
    Ok(out)
}

/// Hop bounds of a `$relation` block (one hop when it names none).
pub(crate) fn hop_bounds(rel: &RelatedExpr) -> (u64, u64) {
    let (min, max) = rel.hops.unwrap_or((1, 1));
    (min.max(1), max.max(min.max(1)))
}

/// Whether record `ord` satisfies the traversal block `rel`: `$cycle` asks
/// for a closed trail through it; otherwise a trail must lead from it to a
/// target record (see [`expand`]).
pub(crate) fn related_holds(
    ctx: &EvalCtx<'_>,
    ord: u64,
    rel: &RelatedExpr,
) -> Result<bool, QueryError> {
    let cancel = || ctx.check_cancelled();
    if rel.cycle {
        return ctx
            .cycles
            .on_cycle(&ctx.snap, ctx.lmpg.tenant, rel, ord, &cancel);
    }
    let (snap, tenant) = (ctx.snap.as_ref(), ctx.lmpg.tenant);
    let adjacency = ctx.adjacency(rel.rel_type.as_deref(), rel.direction)?;
    let labels = LabelGate::new(snap, tenant, &rel.target_labels);
    let passable = |other: u64| -> Result<bool, QueryError> {
        if rel.target_labels.is_empty() {
            return Ok(access::record_by_ord(snap, tenant, other)?.is_some());
        }
        labels.admits(other)
    };
    let target = |other: u64| -> Result<bool, QueryError> {
        let Some(filter) = &rel.filter else {
            return Ok(true);
        };
        let row = filtered_row(snap, tenant, filter, other)?;
        eval_expr_ctx(ctx, filter, &row, other)
    };
    let (min, max) = hop_bounds(rel);
    trail_exists(
        &TrailCheck {
            adjacency: adjacency.as_ref(),
            min,
            max,
            passable: &passable,
            target: &target,
            cancel: &cancel,
            visits: TRAIL_VISIT_BUDGET,
        },
        ord,
    )
}

/// The records satisfying a traversal block's own labels and filter: the
/// targets [`reaching_owners`] expands backwards from.
fn related_targets(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    rel: &RelatedExpr,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<OwnerSet, QueryError> {
    let req = SearchRequest {
        labels: rel.target_labels.clone(),
        where_: rel.filter.as_deref().cloned(),
        ..SearchRequest::default()
    };
    Ok(
        run_matches(lmpg, &req, is_cancelled, snap, 0, usize::MAX, None)?
            .retained
            .into_iter()
            .collect(),
    )
}

/// Records of `labels` that reach a target of `rel` (see
/// [`reaching_owners`]); the targets resolve through the planner.
pub(crate) fn expand_relation(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    rel: &RelatedExpr,
    labels: &[String],
    owner_limit: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<OwnerSet, QueryError> {
    let targets = related_targets(lmpg, snap, rel, is_cancelled)?;
    let (view, tenant) = (snap.as_ref(), lmpg.tenant);
    let passable_labels = LabelGate::new(view, tenant, &rel.target_labels);
    let root_labels = LabelGate::new(view, tenant, labels);
    let passable = |ord: u64| -> Result<bool, QueryError> {
        if rel.target_labels.is_empty() {
            return Ok(access::record_by_ord(view, tenant, ord)?.is_some());
        }
        passable_labels.admits(ord)
    };
    let target = |ord: u64| Ok(targets.contains(ord));
    let root = |ord: u64| root_labels.admits(ord);
    let cancel = || {
        if is_cancelled() {
            Err(QueryError::Cancelled)
        } else {
            Ok(())
        }
    };
    let adjacency = TypeAdjacency::open(
        snap,
        tenant,
        rel.rel_type.as_deref(),
        rel.direction,
        &cancel,
    )?;
    let (min, max) = hop_bounds(rel);
    reaching_owners(
        &TrailCheck {
            adjacency: &adjacency,
            min,
            max,
            passable: &passable,
            target: &target,
            cancel: &cancel,
            visits: TRAIL_VISIT_BUDGET,
        },
        &targets,
        &root,
        owner_limit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chain `0 -> 1 -> … -> len - 1`.
    struct Chain(u64);

    impl Adjacency for Chain {
        fn neighbours(
            &self,
            node: u64,
            forward: bool,
            out: &mut Vec<(u64, u64)>,
        ) -> Result<(), QueryError> {
            match (forward, node) {
                (true, n) if n + 1 < self.0 => out.push((n + 1, n)),
                (false, n) if n > 0 => out.push((n - 1, n - 1)),
                _ => {}
            }
            Ok(())
        }

        fn undirected(&self) -> bool {
            false
        }
    }

    #[test]
    fn a_hops_search_stops_at_its_visit_budget() {
        let chain = Chain(50);
        let yes = |_: u64| Ok(true);
        let last = |ord: u64| Ok(ord == 49);
        let cancel = || Ok(());
        let check = |visits| TrailCheck {
            adjacency: &chain,
            min: 1,
            max: 100,
            passable: &yes,
            target: &last,
            cancel: &cancel,
            visits,
        };
        assert!(trail_exists(&check(TRAIL_VISIT_BUDGET), 0).unwrap());
        assert!(matches!(
            trail_exists(&check(10), 0),
            Err(QueryError::ResourceLimit(_))
        ));
        let targets: OwnerSet = [49].into_iter().collect();
        assert!(
            reaching_owners(&check(TRAIL_VISIT_BUDGET), &targets, &yes, usize::MAX)
                .unwrap()
                .contains(0)
        );
        assert!(matches!(
            reaching_owners(&check(10), &targets, &yes, usize::MAX),
            Err(QueryError::ResourceLimit(_))
        ));
    }
}
