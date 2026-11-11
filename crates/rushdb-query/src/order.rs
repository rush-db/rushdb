//! Order-aware planning: whether a page is cheaper read from a source that
//! already yields records in the requested order ("order-first": walk it,
//! check the filter, stop at the page) than by reading every candidate and
//! keeping the best `skip + limit` ("predicate-first"). The costs are
//! estimated reads from the exact counters the predicate planner uses
//! (label and property support counts, the plan's owner set), so the
//! choice is arithmetic, not a guess.
//!
//! Order-first needs the number of matches up front, so that it can stop
//! at the page: from counters (no filter), the index plan's exact owner set,
//! or a cursor continuation. Sources, each yielding the exact sequence a
//! full sort gives (ties by ascending ordinal):
//!
//! - storage order (`orderBy` empty) walked from its far end, for a page
//!   past the middle of the matches;
//! - record id order (`__id`), either way, from the identity rows;
//! - one property: the records missing it (they sort first ascending, last
//!   descending), then its holders, whose values are read and sorted.
//!   Holders are counted per definition and label, so a property few
//!   records hold is ordered without visiting the rest.
//!
//! A walk that passes more rows than predicate-first would read gives up,
//! and the page is read predicate-first: a wrong estimate costs at most
//! twice the plain read.

use std::collections::BTreeSet;

use rushdb_lmpg::reader::TryRead;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::value::{is_reserved_name, ID_FIELD};

use crate::planner::{family_id, pids_of};
use crate::{normalize, QueryError};

/// How an ordered page is read when order-first wins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OrderPath {
    /// Storage order walked from the last record back: the page is the
    /// mirror's rows, reversed.
    StorageMirror,
    /// Record id order. `walk_descending` is the walk's direction; with
    /// `mirror` the page is read from the far end and reversed.
    Id { walk_descending: bool, mirror: bool },
    /// One property: the records of `scope` missing it, and its holders
    /// (definitions `pids`, held under the labels `holding`): read and
    /// sorted, or walked through `index` in value order.
    Property {
        descending: bool,
        pids: Vec<u64>,
        holding: BTreeSet<String>,
        scope: Vec<String>,
        /// Records of `scope` missing the property (from counters): the
        /// walk for them stops after this many.
        missing: u64,
        index: Option<IndexOrder>,
    },
}

/// A property's values in order from its ordered indexes, one per label
/// holding it. Only for a property stored as one type whose index order is
/// the sort's order: integers, datetimes (numbers to the sort) or strings.
/// Its nulls are not indexed; they sort before every number and after
/// every string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IndexOrder {
    pub(crate) ids: Vec<u32>,
    /// Numbers (nulls sort before the values) or strings (after).
    pub(crate) numeric: bool,
}

/// An order-first plan and the two costs it was chosen on.
#[derive(Clone, Debug)]
pub(crate) struct OrderPlan {
    pub(crate) path: OrderPath,
    /// Estimated rows the order-first walk reads.
    pub(crate) order_first_reads: u64,
    /// Rows predicate-first reads: every candidate.
    pub(crate) predicate_first_reads: u64,
}

impl OrderPlan {
    /// Whether the page is read from the far end and reversed.
    pub(crate) fn mirror(&self) -> bool {
        matches!(
            self.path,
            OrderPath::StorageMirror | OrderPath::Id { mirror: true, .. }
        )
    }

    /// The name `describe.explain` reports.
    pub(crate) fn source(&self) -> &'static str {
        match self.path {
            OrderPath::StorageMirror => "storage",
            OrderPath::Id { .. } => "id",
            OrderPath::Property { index: None, .. } => "property",
            OrderPath::Property { index: Some(_), .. } => "index",
        }
    }
}

/// What the planner knows about a page before choosing its order path.
pub(crate) struct PageShape<'a> {
    pub(crate) labels: &'a [String],
    pub(crate) order_by: &'a [(String, bool)],
    pub(crate) skip: usize,
    pub(crate) limit: usize,
    /// Every match, when known up front.
    pub(crate) matches: Option<u64>,
    /// The candidates predicate-first reads (the owner set, or the labels'
    /// records).
    pub(crate) candidates: u64,
}

/// The order-first plan for `page`, when it reads fewer rows than
/// predicate-first; `None` otherwise (and always when the number of matches
/// is not known, a page wants every match, or it orders by several keys).
///
/// # Errors
/// Returns storage read errors from the counters and the property catalog.
pub(crate) fn plan_order<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    page: &PageShape<'_>,
) -> Result<Option<OrderPlan>, QueryError> {
    let Some(matches) = page.matches.filter(|matches| *matches > 0) else {
        return Ok(None);
    };
    let keep = u128::try_from(page.skip.saturating_add(page.limit)).unwrap_or(u128::MAX);
    let skip = u128::try_from(page.skip).unwrap_or(u128::MAX);
    let (matches, candidates) = (u128::from(matches), u128::from(page.candidates.max(1)));
    // Rows left from the far end; reading them is cheaper than reading
    // `keep` from the near end.
    let from_far_end = matches.saturating_sub(skip);
    let mirror = page.skip > 0 && from_far_end < keep;
    let walked = keep.min(from_far_end);
    let plan = |path, reads: u128, predicate_first: u128| {
        (reads < predicate_first).then(|| OrderPlan {
            path,
            order_first_reads: saturate(reads),
            predicate_first_reads: saturate(predicate_first),
        })
    };
    match page.order_by {
        [] => {
            // Storage order is what predicate-first walks already, from the
            // near end: `keep` matches' worth of candidates.
            let near = keep.saturating_mul(candidates) / matches;
            let far = walked.saturating_mul(candidates) / matches;
            Ok(if mirror {
                plan(OrderPath::StorageMirror, far, near)
            } else {
                None
            })
        }
        [(field, descending)] if field == ID_FIELD => {
            // The identity rows hold every record of the tenant; `matches`
            // of them are wanted.
            let records = u128::from(rushdb_lmpg::derived::tenant_usage(snap, tenant)?.records);
            let reads = walked.saturating_mul(records) / matches;
            Ok(plan(
                OrderPath::Id {
                    walk_descending: *descending != mirror,
                    mirror,
                },
                reads,
                candidates,
            ))
        }
        [(field, descending)] => property_plan(
            snap,
            tenant,
            page,
            field,
            *descending,
            keep,
            matches,
            candidates,
        ),
        _ => Ok(None),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the page's numbers, already widened by the caller"
)]
fn property_plan<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    page: &PageShape<'_>,
    field: &str,
    descending: bool,
    keep: u128,
    matches: u128,
    candidates: u128,
) -> Result<Option<OrderPlan>, QueryError> {
    let norm = normalize(field);
    if norm.is_empty() || is_reserved_name(&norm) {
        return Ok(None);
    }
    let kind = OwnerKind::Record;
    let pids = match family_id(snap, tenant, &norm)? {
        Some(fid) => pids_of(snap, tenant, fid)?,
        None => Vec::new(),
    };
    for &pid in &pids {
        // An array value sorts as its JSON text: only the whole record has it.
        if rushdb_lmpg::derived::array_markers(snap, tenant, kind, pid)?.0 {
            return Ok(None);
        }
    }
    let scope: Vec<String> = if page.labels.is_empty() {
        rushdb_lmpg::reader::try_access::labels(snap, tenant)?
    } else {
        page.labels
            .iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .cloned()
            .collect()
    };
    let mut holding = BTreeSet::new();
    let mut holders = 0u64;
    let mut records = 0u64;
    for label in &scope {
        records = records.saturating_add(rushdb_lmpg::derived::label_count(snap, tenant, label)?);
        for &pid in &pids {
            let held = rushdb_lmpg::derived::prop_support_label(snap, tenant, kind, pid, label)?;
            if held > 0 {
                holding.insert(label.clone());
                holders = holders.saturating_add(held);
            }
        }
    }
    let (records, holders) = (u128::from(records.max(1)), u128::from(holders));
    let missing = records.saturating_sub(holders);
    // Matches are spread over the labels' records like the filter's
    // selectivity: `matches / records` of each part matches.
    let matching = |part: u128| part.saturating_mul(matches) / records;
    // Rows of the labels' walk read to find `wanted` missing matches; a
    // holder passed on the way costs a presence lookup.
    let missing_walk = |wanted: u128| {
        let found = matching(missing);
        if missing == 0 {
            0
        } else if found == 0 || wanted >= found {
            records
        } else {
            (wanted.saturating_mul(records) / found).min(records)
        }
    };
    let index = index_order(snap, tenant, &norm, &pids, &holding)?;
    // Holders read to place `wanted` matches: every one when they are
    // sorted, about as many as needed when an index walks them in order.
    let holder_reads = |wanted: u128| match index {
        Some(_) => (wanted.saturating_mul(records) / matches).min(holders),
        None => holders,
    };
    let reads = if descending {
        holder_reads(keep).saturating_add(if matching(holders) >= keep {
            0
        } else {
            missing_walk(keep - matching(holders))
        })
    } else if matching(missing) >= keep {
        missing_walk(keep)
    } else {
        missing_walk(u128::MAX).saturating_add(holder_reads(keep - matching(missing)))
    };
    let path = OrderPath::Property {
        descending,
        pids,
        holding,
        scope,
        missing: saturate(missing),
        index,
    };
    Ok((reads < candidates).then(|| OrderPlan {
        path,
        order_first_reads: saturate(reads),
        predicate_first_reads: saturate(candidates),
    }))
}

/// The ordered indexes that walk the property's values in sort order: one
/// `Ready` index per label in `holding`, and exactly one typed definition
/// (integer, datetime or string) holding values there. Floats are left to
/// the sort: the index keeps `-0.0` and `0.0` apart, the sort ties them.
fn index_order<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    property: &str,
    pids: &[u64],
    holding: &BTreeSet<String>,
) -> Result<Option<IndexOrder>, QueryError> {
    use rushdb_types::model::{LogicalType, TypeState};
    if holding.is_empty() {
        return Ok(None);
    }
    let mut typed = None;
    for &pid in pids {
        let Some(def) = rushdb_lmpg::reader::try_access::definition(snap, tenant, pid)? else {
            continue;
        };
        let TypeState::Known(ty) = def.type_state else {
            continue;
        };
        let held = holding.iter().try_fold(0u64, |held, label| {
            rushdb_lmpg::derived::prop_support_label(snap, tenant, OwnerKind::Record, pid, label)
                .map(|n| held.saturating_add(n))
        })?;
        if held == 0 {
            continue;
        }
        if typed.replace(ty).is_some() {
            return Ok(None);
        }
    }
    let numeric = match typed {
        Some(LogicalType::Int64 | LogicalType::DateTime) => true,
        Some(LogicalType::Utf8) => false,
        _ => return Ok(None),
    };
    let mut ids = Vec::new();
    for label in holding {
        match rushdb_lmpg::index::ready_ordered(snap, tenant, label, property)? {
            Some(def) => ids.push(def.id),
            None => return Ok(None),
        }
    }
    Ok(Some(IndexOrder { ids, numeric }))
}

fn saturate(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
