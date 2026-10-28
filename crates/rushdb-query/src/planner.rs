//! Access-path selection and bounded candidate admission.

use super::{
    access, canonical_id, hex_decode_16, normalize, scalar_equal, Expr, FieldPred, Json, Lmpg,
    LogicalType, QueryError, QuerySnapshot, Scalar, SearchRequest, TypeState,
};
use rushdb_lmpg::reader::TryRead;

/// Index-only candidate plan for a `where` conjunction. Eligible positive
/// leaves (`field == scalar`, `field IN`, `$exists: true`, null, and numeric
/// range) resolve to maintained posting/presence owner sets; residual leaves
/// (`$ne`, contains, `$type`, logical, traversal) are evaluated later on the
/// reduced candidate set. Eligible leaves are materialized in ascending
/// estimated cardinality (most selective first) with an early-out once the
/// intersection is empty. `exact` is true when every leaf was answered AND
/// there are no traversal predicates, so the caller may skip per-candidate
/// evaluation and the exact-count rescan.
pub(super) struct IndexPlan {
    /// Candidate owners, ascending.
    pub(super) ords: OwnerSet,
    pub(super) exact: bool,
    /// Maintained owner count for the query, when known (single unscoped
    /// `$exists`): lets the executor report an exact total without
    /// materializing or counting every candidate.
    pub(super) known_total: Option<u64>,
    /// The conjuncts the owner set does not answer exactly (non-indexable
    /// leaves, and null-only leaves, a superset prefilter), for the executor
    /// to evaluate on each candidate instead of the whole `where`. `None`
    /// when there are none.
    pub(super) residual: Option<Expr>,
}

/// Cap on the smallest eligible leaf the planner walks occurrence by
/// occurrence; beyond it a full scan is cheaper. A leaf a declared ordered
/// index answers is never capped: its entries stream into a compressed
/// owner set (a few bits per owner when dense), far cheaper than scanning
/// and evaluating every record (READ-1: `city = Oslo` at ~2M owners fell
/// back to a 10M-record scan). Unknown estimates sort last.
const PLANNER_MATERIALIZE_CAP: u64 = 2_000_000;

/// An unindexed predicate leaf that walks more owners than this is counted
/// in [`unindexed_walks_total`]: the signal that a declared ordered index
/// would pay off (DC2 keeps no automatic value postings).
pub const UNINDEXED_WALK_THRESHOLD: u64 = 10_000;

static UNINDEXED_WALKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Unindexed leaf walks over more than [`UNINDEXED_WALK_THRESHOLD`] owners
/// in this process (`rushdb_unindexed_walks_total`).
#[must_use]
pub fn unindexed_walks_total() -> u64 {
    UNINDEXED_WALKS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Largest page the bounded presence merge will materialize for a known-total
/// count plan; beyond it the normal path (full materialization) is used.
const PLANNER_BOUNDED_PAGE_CAP: usize = 100_000;

#[expect(
    clippy::too_many_lines,
    reason = "the planner is a single cohesive access-path selection routine; splitting it would obscure the fall-through ordering"
)]
pub(super) fn index_candidates_with_budget(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    req: &SearchRequest,
    budget: Option<usize>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<IndexPlan>, QueryError> {
    let owner_limit = budget.map_or(usize::MAX, |bytes| bytes / OWNER_SET_BYTES_PER_OWNER);
    if req.force_scan {
        return Ok(None);
    }
    let Some(expr) = &req.where_ else {
        return Ok(None);
    };
    let mut leaves: Vec<&Expr> = Vec::new();
    collect_leaves(expr, &mut leaves);
    if leaves.is_empty() {
        return Ok(None);
    }
    let mut eligible: Vec<(u64, &Expr)> = Vec::new();
    let mut residual: Vec<Expr> = Vec::new();
    let mut has_residual = false;
    let mut exactable = true;
    for leaf in leaves {
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        if let Some(estimate) = leaf_estimate(snap, lmpg.tenant, leaf, &req.labels)? {
            if leaf_is_null_only(leaf) {
                residual.push(leaf.clone());
            }
            eligible.push((estimate, leaf));
        } else {
            residual.push(leaf.clone());
            has_residual = true;
            if leaf_is_null_only(leaf) {
                exactable = false;
            }
        }
    }
    if eligible.is_empty() {
        return Ok(None);
    }
    // Single unscoped `$exists: true` with a maintained owner count: answer the
    // total from the counter and materialize only the requested page.
    if !has_residual
        && exactable
        && req.related.is_empty()
        && req.order_by.is_empty()
        && eligible.len() == 1
    {
        if let Expr::Field(FieldPred::Exists(name, true)) = eligible[0].1 {
            if let Some(limit) = req.pagination.limit {
                let page = req
                    .pagination
                    .skip
                    .saturating_add(limit)
                    .min(u64::try_from(PLANNER_BOUNDED_PAGE_CAP).unwrap_or(u64::MAX));
                let page = usize::try_from(page).unwrap_or(PLANNER_BOUNDED_PAGE_CAP);
                if let Some(total) = total_support(snap, lmpg.tenant, name, &req.labels)? {
                    let ords =
                        owners_with_presence_bounded(snap, lmpg.tenant, name, &req.labels, page)?;
                    return Ok(Some(IndexPlan {
                        ords: ords.into_iter().collect(),
                        exact: true,
                        known_total: Some(total),
                        residual: None,
                    }));
                }
            }
        }
    }
    eligible.sort_by_key(|(estimate, _)| *estimate);
    if eligible[0].0 != u64::MAX
        && eligible[0].0 > PLANNER_MATERIALIZE_CAP
        && !crate::index_access::answers(snap, lmpg.tenant, eligible[0].1, &req.labels)?
    {
        // The most selective eligible leaf is still too large to materialize.
        // (Unknown estimates are `u64::MAX` and are materialized normally.)
        return Ok(None);
    }
    // Every leaf but `$id` resolves under the requested labels (index
    // entries, presence and value rows are kept per label), so one such leaf
    // in the conjunction scopes the whole intersection.
    let label_scoped = req.labels.is_empty()
        || eligible
            .iter()
            .any(|(_, leaf)| !is_id_leaf(leaf) && !matches!(leaf, Expr::Or(_)));
    let mut acc: Option<OwnerSet> = None;
    for (_, leaf) in eligible {
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        if acc.as_ref().is_some_and(OwnerSet::is_empty) {
            break;
        }
        // Later leaves are checked on the owners earlier leaves left
        // (`LeafScope::candidates`), not walked over their population.
        let scope = LeafScope {
            labels: &req.labels,
            owner_limit,
            candidates: acc.as_ref(),
        };
        let Some(owners) = leaf_owners(snap, lmpg.tenant, leaf, &scope)? else {
            // Classified eligible but not materializable: fall back safely.
            return Ok(None);
        };
        acc = Some(match acc {
            None => owners,
            Some(prev) => prev & owners,
        });
    }
    let mut out = acc.unwrap_or_default();
    if !label_scoped {
        // Only `$id` leaves: probe labels for that (small) id set.
        let mut outside = Vec::new();
        for ord in &out {
            if is_cancelled() {
                return Err(QueryError::Cancelled);
            }
            let mut allowed = false;
            for label in &req.labels {
                let key = rushdb_types::keys::label_member_key(lmpg.tenant, label, ord);
                if snap.get(&key)?.is_some() {
                    allowed = true;
                    break;
                }
            }
            if !allowed {
                outside.push(ord);
            }
        }
        for ord in outside {
            out.remove(ord);
        }
    }
    let exact = !has_residual && exactable && req.related.is_empty();
    Ok(Some(IndexPlan {
        ords: out,
        exact,
        known_total: None,
        residual: match residual.len() {
            0 => None,
            1 => residual.pop(),
            _ => Some(Expr::And(residual)),
        },
    }))
}

/// Flatten a top-level `And` into conjuncts; any other node is a single
/// conjunct (eligible or residual), so partial-index planning can still use
/// the other conjuncts.
pub(super) fn collect_leaves<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    match expr {
        Expr::And(items) => {
            for item in items {
                collect_leaves(item, out);
            }
        }
        _ => out.push(expr),
    }
}

/// Null-only leaves are a superset prefilter (a record may hold both a null
/// and a non-null occurrence of one property), so they never take the exact
/// shortcut.
fn leaf_is_null_only(expr: &Expr) -> bool {
    matches!(expr, Expr::Field(FieldPred::Eq(_, value)) if value.is_null())
        || matches!(expr, Expr::Field(FieldPred::IsNull(_, true)))
}

/// Cardinality estimate for an indexable leaf, restricted to `labels` when
/// given. `None` = residual (not served by presence/dictionary lookups).
/// `u64::MAX` = eligible but unknown (sorts last).
fn leaf_estimate<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    expr: &Expr,
    labels: &[String],
) -> Result<Option<u64>, QueryError> {
    // A prefix leaf is answered only by a declared ordered index.
    if matches!(expr, Expr::Field(FieldPred::StartsWith(..))) {
        return Ok(crate::index_access::answers(snap, tenant, expr, labels)?.then_some(u64::MAX));
    }
    Ok(match expr {
        Expr::Field(FieldPred::Eq(_, value)) if value.is_null() => Some(u64::MAX),
        Expr::Field(FieldPred::Eq(name, value)) => {
            estimate_values(snap, tenant, name, std::slice::from_ref(value), labels)?
        }
        Expr::Field(FieldPred::In(name, values)) if values.iter().all(|v| !v.is_null()) => {
            estimate_values(snap, tenant, name, values, labels)?
        }
        // `$id` resolves through the record-id index: one lookup, one ordinal.
        Expr::Field(FieldPred::Id(_)) => Some(1),
        // `$id: {$in: [...]}` lowers to an `Or` of id leaves; the union is
        // exactly the id set, so it is indexable (no residual scan).
        Expr::Or(items) if !items.is_empty() && items.iter().all(is_id_leaf) => {
            Some(items.len() as u64)
        }
        Expr::Field(FieldPred::IsNull(_, true) | FieldPred::Exists(_, true)) => Some(u64::MAX),
        // A bounded estimate from the on-demand value distribution
        // (never surfaced as EXPLAIN's exact field, see `is_range_leaf`).
        Expr::Field(FieldPred::Gt(name, bound)) => {
            range_leaf_estimate(snap, tenant, name, RangeOp::Gt, *bound)?
        }
        Expr::Field(FieldPred::Gte(name, bound)) => {
            range_leaf_estimate(snap, tenant, name, RangeOp::Gte, *bound)?
        }
        Expr::Field(FieldPred::Lt(name, bound)) => {
            range_leaf_estimate(snap, tenant, name, RangeOp::Lt, *bound)?
        }
        Expr::Field(FieldPred::Lte(name, bound)) => {
            range_leaf_estimate(snap, tenant, name, RangeOp::Lte, *bound)?
        }
        _ => None,
    })
}

/// Whether `expr` is a range leaf (`Gt`/`Gte`/`Lt`/`Lte`). Used to exclude
/// range leaves from [`explain_cardinality`] *structurally* — not by
/// coincidentally filtering on the `u64::MAX` sentinel value — so a future
/// improvement to the histogram estimate can never silently leak an
/// approximation into EXPLAIN's exact `estimatedCardinality` field.
fn is_range_leaf(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Field(
            FieldPred::Gt(..) | FieldPred::Gte(..) | FieldPred::Lt(..) | FieldPred::Lte(..)
        )
    )
}

/// Resolves a range leaf's selectivity against the value distribution of the
/// property, computed on demand from the value dictionary:
/// `(estimated owners, owner-occurrences the distribution covers)` for the
/// first numeric typed definition that holds values, or `None` when none
/// does.
fn range_leaf_histogram_estimate<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    op: RangeOp,
    bound: f64,
) -> Result<Option<(u64, u64)>, QueryError> {
    let norm = normalize(name);
    if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let Some(fid) = family_id(snap, tenant, &norm)? else {
        return Ok(None);
    };
    let above = matches!(op, RangeOp::Gt | RangeOp::Gte);
    for ty in [
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::DateTime,
    ] {
        let Some(pid) = property_id(snap, tenant, fid, ty)? else {
            continue;
        };
        let Some(hist) = crate::stats_cache::distribution(
            snap,
            tenant,
            rushdb_types::ids::OwnerKind::Record,
            None,
            pid,
        )?
        else {
            continue;
        };
        let scalar = match ty {
            LogicalType::Float64 => Scalar::Float64(bound),
            LogicalType::Int64 => Scalar::Int64(round_bound(bound, above)),
            LogicalType::DateTime => Scalar::DateTime(round_bound(bound, above)),
            LogicalType::Boolean | LogicalType::Utf8 => continue,
        };
        let Ok(enc) = rushdb_types::encoding::encode_order_preserving(ty, &scalar) else {
            continue;
        };
        // Few distinct values is where bucket interpolation is weakest and an
        // exact dictionary sum is cheapest (a partial walk saw too few to know).
        if !hist.partial && hist.distinct <= LOW_CARDINALITY_EXACT_THRESHOLD {
            let exact = exact_low_cardinality_range_count(snap, tenant, pid, ty, op, bound)?;
            return Ok(Some((exact, hist.total())));
        }
        return Ok(Some((hist.estimate(&enc, above), hist.total())));
    }
    Ok(None)
}

/// Distinct-value ceiling gating [`exact_low_cardinality_range_count`].
const LOW_CARDINALITY_EXACT_THRESHOLD: u64 = 256;

/// Exact owner-occurrence count for a range predicate: the sum of the value
/// dictionary's refcounts over matching values.
fn exact_low_cardinality_range_count<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    pid: u64,
    ty: LogicalType,
    op: RangeOp,
    bound: f64,
) -> Result<u64, QueryError> {
    let tag = rushdb_types::encoding::logical_tag(ty);
    let mut total = 0u64;
    for (entry, count) in rushdb_lmpg::derived::dictionary(
        snap,
        tenant,
        rushdb_types::ids::OwnerKind::Record,
        pid,
        &[],
    )? {
        let Some((&entry_tag, payload)) = entry.split_first() else {
            continue;
        };
        if entry_tag != tag {
            continue;
        }
        let Ok(scalar) = rushdb_types::encoding::decode_order_preserving(ty, payload) else {
            continue;
        };
        if range_scalar_matches(&scalar, op, bound) {
            total = total.saturating_add(count);
        }
    }
    Ok(total)
}

/// Bounded estimate of owners satisfying a range predicate, or `u64::MAX`
/// ("eligible but unknown") when the property holds no numeric values.
fn range_leaf_estimate<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    op: RangeOp,
    bound: f64,
) -> Result<Option<u64>, QueryError> {
    Ok(Some(
        range_leaf_histogram_estimate(snap, tenant, name, op, bound)?
            .map_or(u64::MAX, |(estimate, _total)| estimate),
    ))
}

/// Selectivity fraction (`estimated owners / exact owner-occurrences`,
/// clamped to `[0.0, 1.0]`) for the first range leaf (`Gt`/`Gte`/`Lt`/`Lte`)
/// in `where_`, from the value-distribution histogram — `None` when the predicate has no
/// range leaf or no usable histogram exists. This is `ExplainPlan`'s own
/// estimate channel: deliberately never merged with
/// [`explain_cardinality`]'s exact field.
///
/// # Errors
///
/// Returns the underlying storage error from a definition/family lookup.
#[expect(
    clippy::cast_precision_loss,
    reason = "estimate/total feed a selectivity fraction, not an exact count; this is \
              approximate by design"
)]
pub fn explain_range_selectivity<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    where_: &Expr,
) -> Result<Option<f64>, QueryError> {
    let mut leaves = Vec::new();
    collect_leaves(where_, &mut leaves);
    for leaf in leaves {
        let Expr::Field(pred) = leaf else {
            continue;
        };
        let (name, op, bound) = match pred {
            FieldPred::Gt(name, bound) => (name, RangeOp::Gt, *bound),
            FieldPred::Gte(name, bound) => (name, RangeOp::Gte, *bound),
            FieldPred::Lt(name, bound) => (name, RangeOp::Lt, *bound),
            FieldPred::Lte(name, bound) => (name, RangeOp::Lte, *bound),
            _ => continue,
        };
        if let Some((estimate, total)) =
            range_leaf_histogram_estimate(snap, tenant, name, op, bound)?
        {
            if total > 0 {
                return Ok(Some((estimate as f64 / total as f64).clamp(0.0, 1.0)));
            }
        }
    }
    Ok(None)
}

/// A set of owner ordinals. Ordinals are allocated densely per tenant, so a
/// compressed bitmap holds a million owners in ~128 KiB and intersects them
/// word by word.
pub(crate) type OwnerSet = roaring::RoaringTreemap;

/// Upper bound of an [`OwnerSet`]'s bytes per owner: array containers store
/// two bytes per owner, bitmap containers less; the slack covers container
/// headers.
pub(crate) const OWNER_SET_BYTES_PER_OWNER: usize = 4;

pub(super) fn extend_owners(
    owners: &mut OwnerSet,
    additions: impl IntoIterator<Item = u64>,
    limit: usize,
) -> Result<(), QueryError> {
    for owner in additions {
        if owners.len() >= u64::try_from(limit).unwrap_or(u64::MAX) && !owners.contains(owner) {
            return Err(QueryError::ResourceLimit(
                "aggregate candidate budget exceeded".into(),
            ));
        }
        owners.insert(owner);
    }
    Ok(())
}

/// Materialize an eligible leaf's owner set exactly: from a declared ordered
/// index when one covers every requested label; otherwise presence
/// rows narrow to the owners carrying the property (under `labels` when
/// given), and their occurrence values decide the predicate. There are no
/// automatic value postings.
fn leaf_owners<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    expr: &Expr,
    scope: &LeafScope<'_>,
) -> Result<Option<OwnerSet>, QueryError> {
    let (labels, owner_limit) = (scope.labels, scope.owner_limit);
    if let Some(owners) = crate::index_access::leaf_owners(snap, tenant, expr, labels, owner_limit)?
    {
        return Ok(Some(owners));
    }
    match expr {
        Expr::Field(FieldPred::Eq(name, value)) if value.is_null() => {
            owners_with_presence(snap, tenant, name, true, labels, owner_limit)
        }
        Expr::Field(FieldPred::Eq(name, value)) => {
            owners_for_values(snap, tenant, name, std::slice::from_ref(value), scope)
        }
        Expr::Field(FieldPred::In(name, values)) if values.iter().all(|v| !v.is_null()) => {
            owners_for_values(snap, tenant, name, values, scope)
        }
        Expr::Field(FieldPred::Id(id)) => Ok(Some(owners_for_id(snap, tenant, id)?)),
        Expr::Or(items) if !items.is_empty() && items.iter().all(is_id_leaf) => {
            let mut out = OwnerSet::new();
            for item in items {
                if let Expr::Field(FieldPred::Id(id)) = item {
                    extend_owners(&mut out, owners_for_id(snap, tenant, id)?, owner_limit)?;
                }
            }
            Ok(Some(out))
        }
        Expr::Field(FieldPred::Exists(name, true)) => {
            owners_with_presence(snap, tenant, name, false, labels, owner_limit)
        }
        Expr::Field(FieldPred::IsNull(name, true)) => {
            owners_with_presence(snap, tenant, name, true, labels, owner_limit)
        }
        Expr::Field(FieldPred::Gt(name, bound)) => {
            owners_with_range(snap, tenant, name, RangeOp::Gt, *bound, scope)
        }
        Expr::Field(FieldPred::Gte(name, bound)) => {
            owners_with_range(snap, tenant, name, RangeOp::Gte, *bound, scope)
        }
        Expr::Field(FieldPred::Lt(name, bound)) => {
            owners_with_range(snap, tenant, name, RangeOp::Lt, *bound, scope)
        }
        Expr::Field(FieldPred::Lte(name, bound)) => {
            owners_with_range(snap, tenant, name, RangeOp::Lte, *bound, scope)
        }
        _ => Ok(None),
    }
}

/// Leaf resolving through the record-id index (`$id`).
fn is_id_leaf(expr: &Expr) -> bool {
    matches!(expr, Expr::Field(FieldPred::Id(_)))
}

/// Resolve `$id` string values through the maintained id index. Hyphens are
/// ignored and case-folded, matching the evaluator's `canonical_id`.
fn owners_for_id<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    id: &str,
) -> Result<OwnerSet, QueryError> {
    let canonical = canonical_id(id);
    let Some(bytes) = hex_decode_16(&canonical) else {
        return Ok(OwnerSet::new());
    };
    let rid = rushdb_types::ids::RecordId(bytes);
    Ok(access::record_by_id(snap, tenant, &rid)?
        .map(|header| header.ordinal)
        .into_iter()
        .collect())
}

/// Exact owner-occurrence count of `values` from the value dictionary,
/// restricted to `labels` when given. `None` = not indexable.
fn estimate_values<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    values: &[Json],
    labels: &[String],
) -> Result<Option<u64>, QueryError> {
    let norm = normalize(name);
    if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let Some(fid) = family_id(snap, tenant, &norm)? else {
        return Ok(Some(0));
    };
    let Some((pids, encodings)) = value_probes(snap, tenant, fid, values)? else {
        return Ok(None);
    };
    let mut total = 0u64;
    for (ty, enc) in &encodings {
        let mut entry = vec![rushdb_types::encoding::logical_tag(*ty)];
        entry.extend_from_slice(enc);
        if entry.len() > rushdb_types::keys::DICT_MAX_VALUE_BYTES {
            // Long values are not in the dictionary: unknown, sorts last.
            return Ok(Some(u64::MAX));
        }
        for &pid in &pids {
            for label in labels_for(snap, tenant, pid, labels)? {
                total = total.saturating_add(rushdb_lmpg::derived::value_count(
                    snap,
                    tenant,
                    rushdb_types::ids::OwnerKind::Record,
                    &label,
                    pid,
                    &entry,
                )?);
            }
        }
    }
    Ok(Some(total))
}

/// The labels to search for a pid: the requested ones, or every label whose
/// records carry it.
fn labels_for<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    pid: u64,
    labels: &[String],
) -> Result<Vec<String>, QueryError> {
    if labels.is_empty() {
        Ok(rushdb_lmpg::derived::labels_of_pid(
            snap,
            tenant,
            rushdb_types::ids::OwnerKind::Record,
            pid,
        )?)
    } else {
        Ok(labels.to_vec())
    }
}

/// What a leaf is resolved against: the requested labels, the owner budget,
/// and the owners earlier leaves already narrowed to (when any).
pub(super) struct LeafScope<'a> {
    pub(super) labels: &'a [String],
    pub(super) owner_limit: usize,
    pub(super) candidates: Option<&'a OwnerSet>,
}

/// Owners under `labels` (all when empty) carrying a non-null value of `pid`
/// for which `keep` holds. When earlier leaves already narrowed the owners
/// to fewer candidates than `pid`'s population, only those candidates are
/// checked; otherwise the population is walked. Returns `false` (scan
/// instead) when the population to walk exceeds the materialization cap.
fn owners_matching<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    pid: u64,
    scope: &LeafScope<'_>,
    owners: &mut OwnerSet,
    keep: &dyn Fn(&access::Occurrence) -> bool,
) -> Result<bool, QueryError> {
    let kind = rushdb_types::ids::OwnerKind::Record;
    let labels = labels_for(snap, tenant, pid, scope.labels)?;
    let mut population = 0u64;
    for label in &labels {
        population = population.saturating_add(rushdb_lmpg::derived::prop_support_label(
            snap, tenant, kind, pid, label,
        )?);
    }
    // Labels whose owners hold `pid` as an array (maintained per label): only
    // those need every element; elsewhere the value is the single row at
    // element 0, read with one point get instead of a range scan per owner.
    let (_, array_labels) = rushdb_lmpg::derived::array_markers(snap, tenant, kind, pid)?;
    let matches = |owner: u64, scalar: bool| -> Result<bool, QueryError> {
        Ok(if scalar {
            access::occurrence(snap, tenant, kind, owner, pid, 0)?.is_some_and(|occ| keep(&occ))
        } else {
            access::occurrences_of_pid(snap, tenant, kind, owner, pid)?
                .iter()
                .any(keep)
        })
    };
    if let Some(candidates) = scope.candidates.filter(|c| c.len() < population) {
        let scalar = array_labels.is_empty();
        for owner in candidates {
            if !owners.contains(owner) && matches(owner, scalar)? {
                extend_owners(owners, std::iter::once(owner), scope.owner_limit)?;
            }
        }
        return Ok(true);
    }
    if population > PLANNER_MATERIALIZE_CAP {
        return Ok(false);
    }
    if population > UNINDEXED_WALK_THRESHOLD {
        UNINDEXED_WALKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    for label in &labels {
        let scalar = !array_labels.contains(label);
        for owner in rushdb_lmpg::derived::presence_owners(
            snap,
            tenant,
            kind,
            label,
            pid,
            rushdb_types::keys::NULLNESS_VALUE,
        )? {
            if !owners.contains(owner) && matches(owner, scalar)? {
                extend_owners(owners, std::iter::once(owner), scope.owner_limit)?;
            }
        }
    }
    Ok(true)
}

/// Owner ordinals whose value satisfies a range predicate. String and boolean
/// properties never satisfy `numeric_cmp_any`, so they contribute nothing.
fn owners_with_range<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    op: RangeOp,
    bound: f64,
    scope: &LeafScope<'_>,
) -> Result<Option<OwnerSet>, QueryError> {
    let norm = normalize(name);
    if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let Some(fid) = family_id(snap, tenant, &norm)? else {
        return Ok(Some(OwnerSet::new()));
    };
    let mut owners = OwnerSet::new();
    for ty in [
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::DateTime,
    ] {
        let Some(pid) = property_id(snap, tenant, fid, ty)? else {
            continue;
        };
        let keep = |occ: &access::Occurrence| {
            matches!(occ.value, rushdb_types::occurrence::OccurrenceValue::Stored { ty: stored } if stored == ty)
                && rushdb_types::encoding::decode_order_preserving(ty, &occ.payload)
                    .is_ok_and(|scalar| range_scalar_matches(&scalar, op, bound))
        };
        if !owners_matching(snap, tenant, pid, scope, &mut owners, &keep)? {
            return Ok(None);
        }
    }
    Ok(Some(owners))
}

#[derive(Clone, Copy)]
pub(super) enum RangeOp {
    Gt,
    Gte,
    Lt,
    Lte,
}

/// Round a float range bound to an integer threshold without leaving the i64
/// domain (`as` saturates on overflow).
#[expect(
    clippy::cast_possible_truncation,
    reason = "floor/ceil already made the value integral; `as` saturates at the i64 edges"
)]
fn round_bound(bound: f64, lower: bool) -> i64 {
    if lower {
        bound.floor() as i64
    } else {
        bound.ceil() as i64
    }
}

pub(super) fn range_scalar_matches(scalar: &Scalar, op: RangeOp, bound: f64) -> bool {
    // DateTime and integer postings share the exact integer domain. Comparing
    // them through `f64` would make values above 2^53 collide; the canonical
    // scalar comparator preserves the integer/float ordering instead.
    let scalar = match scalar {
        Scalar::DateTime(value) => Scalar::Int64(*value),
        other => other.clone(),
    };
    let Some(ordering) = rushdb_types::value::scalar_cmp(&scalar, &Scalar::Float64(bound)) else {
        return false;
    };
    match op {
        RangeOp::Gt => ordering == std::cmp::Ordering::Greater,
        RangeOp::Gte => matches!(
            ordering,
            std::cmp::Ordering::Greater | std::cmp::Ordering::Equal
        ),
        RangeOp::Lt => ordering == std::cmp::Ordering::Less,
        RangeOp::Lte => matches!(
            ordering,
            std::cmp::Ordering::Less | std::cmp::Ordering::Equal
        ),
    }
}

pub(super) fn family_id<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    normalized: &str,
) -> Result<Option<u64>, QueryError> {
    use rushdb_lmpg::reader::keys_ext as kx;
    let sig = kx::family_sig_key(
        tenant,
        "default",
        rushdb_types::ids::OwnerKind::Record,
        normalized,
    );
    let Some(raw) = snap.get(&sig)? else {
        return Ok(None);
    };
    Ok(Some(u64::from_be_bytes(
        raw.as_slice()
            .try_into()
            .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
    )))
}

fn property_id<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    fid: u64,
    ty: LogicalType,
) -> Result<Option<u64>, QueryError> {
    use rushdb_lmpg::reader::keys_ext as kx;
    let Some(raw) = snap.get(&kx::definition_key(tenant, fid, TypeState::Known(ty)))? else {
        return Ok(None);
    };
    Ok(Some(u64::from_be_bytes(
        raw.as_slice()
            .try_into()
            .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
    )))
}

/// Every logical type state a property family may have definitions for.
fn all_type_states() -> Vec<TypeState> {
    let mut states = vec![TypeState::Unresolved];
    states.extend(
        [
            LogicalType::Boolean,
            LogicalType::Int64,
            LogicalType::Float64,
            LogicalType::Utf8,
            LogicalType::DateTime,
        ]
        .into_iter()
        .map(TypeState::Known),
    );
    states
}

/// Typed definitions `(pid)` of a property family, every type state.
pub(super) fn pids_of<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    fid: u64,
) -> Result<Vec<u64>, QueryError> {
    use rushdb_lmpg::reader::keys_ext as kx;
    let mut pids = Vec::new();
    for state in all_type_states() {
        let Some(raw) = snap.get(&kx::definition_key(tenant, fid, state))? else {
            continue;
        };
        pids.push(u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
        ));
    }
    Ok(pids)
}

/// Presence row prefixes `(label, pid, nullness)` of a property name under
/// `labels` (all labels carrying it when empty); `null_only` restricts to
/// owners whose values are all null.
fn presence_prefixes<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    null_only: bool,
    labels: &[String],
) -> Result<Option<Vec<Vec<u8>>>, QueryError> {
    let norm = normalize(name);
    if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let Some(fid) = family_id(snap, tenant, &norm)? else {
        return Ok(Some(Vec::new()));
    };
    let mut prefixes = Vec::new();
    for pid in pids_of(snap, tenant, fid)? {
        for label in labels_for(snap, tenant, pid, labels)? {
            let nullness: &[u8] = if null_only {
                &[rushdb_types::keys::NULLNESS_NULL]
            } else {
                &[
                    rushdb_types::keys::NULLNESS_VALUE,
                    rushdb_types::keys::NULLNESS_NULL,
                ]
            };
            for &nullness in nullness {
                prefixes.push(rushdb_types::keys::presence_nullness_prefix(
                    tenant,
                    rushdb_types::ids::OwnerKind::Record,
                    &label,
                    pid,
                    nullness,
                ));
            }
        }
    }
    Ok(Some(prefixes))
}

/// Owner ordinals carrying `name` (any value, or only nulls with
/// `null_only`) under `labels`, from presence rows. `None` when not
/// indexable.
pub(super) fn owners_with_presence<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    null_only: bool,
    labels: &[String],
    owner_limit: usize,
) -> Result<Option<OwnerSet>, QueryError> {
    let Some(prefixes) = presence_prefixes(snap, tenant, name, null_only, labels)? else {
        return Ok(None);
    };
    let mut owners = OwnerSet::new();
    for prefix in prefixes {
        let (start, end) = rushdb_storage::prefix_range(&prefix);
        let mut failed = None;
        snap.for_each_key(&start, &end, &mut |key| {
            let result = rushdb_types::keys::read_ord_tail(key, prefix.len())
                .ok_or_else(|| rushdb_storage::StorageError::CorruptLog.into())
                .and_then(|owner| extend_owners(&mut owners, std::iter::once(owner), owner_limit));
            match result {
                Ok(()) => true,
                Err(error) => {
                    failed = Some(error);
                    false
                }
            }
        })?;
        if let Some(error) = failed {
            return Err(error);
        }
    }
    Ok(Some(owners))
}

/// Owner ordinals whose value equals any of `values` (exact), under
/// `labels`. `None` when the shape is not indexable or the population is too
/// large to inspect (scan instead).
fn owners_for_values<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    values: &[Json],
    scope: &LeafScope<'_>,
) -> Result<Option<OwnerSet>, QueryError> {
    let norm = normalize(name);
    // System fields and `_id` are not properties; leave them to the evaluator.
    if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let Some(fid) = family_id(snap, tenant, &norm)? else {
        return Ok(Some(OwnerSet::new()));
    };
    let Some((pids, encodings)) = value_probes(snap, tenant, fid, values)? else {
        return Ok(None);
    };
    let keep = |occ: &access::Occurrence| {
        encodings.iter().any(|(ty, enc)| {
            matches!(occ.value, rushdb_types::occurrence::OccurrenceValue::Stored { ty: stored } if stored == *ty)
                && occ.payload == *enc
        })
    };
    let mut owners = OwnerSet::new();
    for pid in pids {
        if !owners_matching(snap, tenant, pid, scope, &mut owners, &keep)? {
            return Ok(None);
        }
    }
    Ok(Some(owners))
}

/// Typed definitions of family `fid` that may hold any of `values`, and every
/// `(type, encoding)` a matching occurrence may carry. Encodings are probed
/// under every definition, not only their own: a write can widen an Int64
/// value into an existing Float64 definition while keeping its Int64
/// encoding. `None` when a value is not equality-indexable.
type ValueProbes = (std::collections::BTreeSet<u64>, Vec<(LogicalType, Vec<u8>)>);

fn value_probes<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    fid: u64,
    values: &[Json],
) -> Result<Option<ValueProbes>, QueryError> {
    let mut pids = std::collections::BTreeSet::new();
    let mut encodings = Vec::new();
    for value in values {
        let Some(variants) = scalar_variants(value) else {
            return Ok(None);
        };
        for (ty, scalar) in variants {
            let Ok(enc) = rushdb_types::encoding::encode_order_preserving(ty, &scalar) else {
                return Ok(None);
            };
            encodings.push((ty, enc));
            if let Some(pid) = property_id(snap, tenant, fid, ty)? {
                pids.insert(pid);
            }
        }
    }
    Ok(Some((pids, encodings)))
}

/// Exact owner support of a property (summed over its typed definitions;
/// owners with several typed values count once per definition), restricted
/// to `labels` when given. `None` when not a property name.
pub(super) fn total_support<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    labels: &[String],
) -> Result<Option<u64>, QueryError> {
    let norm = normalize(name);
    if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let Some(fid) = family_id(snap, tenant, &norm)? else {
        return Ok(Some(0));
    };
    let kind = rushdb_types::ids::OwnerKind::Record;
    let mut total = 0u64;
    for pid in pids_of(snap, tenant, fid)? {
        if labels.is_empty() {
            total = total.saturating_add(rushdb_lmpg::derived::prop_support_total(
                snap, tenant, kind, pid,
            )?);
        } else {
            for label in labels {
                total = total.saturating_add(rushdb_lmpg::derived::prop_support_label(
                    snap, tenant, kind, pid, label,
                )?);
            }
        }
    }
    Ok(Some(total))
}

/// The `k` smallest owners carrying `name` under `labels`, merged lazily
/// from presence-row cursors (owner-ordered within each prefix), so a
/// known-total count plan never materializes the whole support set.
pub(super) fn owners_with_presence_bounded(
    snap: &rushdb_storage::ReadView,
    tenant: rushdb_types::ids::TenantId,
    name: &str,
    labels: &[String],
    k: usize,
) -> Result<Vec<u64>, QueryError> {
    if k == 0 {
        return Ok(Vec::new());
    }
    let Some(prefixes) = presence_prefixes(snap, tenant, name, false, labels)? else {
        return Ok(Vec::new());
    };
    let mut cursors: Vec<rushdb_storage::ScanCursor<'_>> = Vec::new();
    for prefix in &prefixes {
        cursors.push(snap.scan(
            rushdb_storage::ScanBounds::prefix(prefix),
            rushdb_storage::ScanProjection::Keys,
        )?);
    }
    let mut pending: Vec<Option<u64>> = Vec::with_capacity(cursors.len());
    for cursor in &mut cursors {
        pending.push(next_presence_owner(cursor)?);
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(k.min(4096));
    while out.len() < k {
        let Some(i) = pending
            .iter()
            .enumerate()
            .filter_map(|(i, owner)| owner.map(|owner| (owner, i)))
            .min()
            .map(|(_, i)| i)
        else {
            break;
        };
        let owner = pending[i]
            .take()
            .ok_or(rushdb_storage::StorageError::CorruptLog)?;
        pending[i] = next_presence_owner(&mut cursors[i])?;
        if seen.insert(owner) {
            out.push(owner);
        }
    }
    Ok(out)
}

fn next_presence_owner(
    cursor: &mut rushdb_storage::ScanCursor<'_>,
) -> Result<Option<u64>, QueryError> {
    let batch = cursor.next_batch(1, 1024 * 1024)?;
    batch
        .rows
        .into_iter()
        .next()
        .map(|row| {
            rushdb_types::keys::parse_presence_key(&row.key)
                .map(|(_, _, _, _, owner)| owner)
                .ok_or_else(|| rushdb_storage::StorageError::CorruptLog.into())
        })
        .transpose()
}

/// Canonical `(type, scalar)` variants a JSON value can equal. Integral JSON
/// values use a Float64 index variant only when the conversion round-trips
/// exactly under the canonical comparator; otherwise a rounded float posting
/// could become a false candidate. `None` = not indexable (null, array,
/// object).
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the f64 range guards against i64 bounds before narrowing an integral JSON number back to i64"
)]
pub(super) fn scalar_variants(value: &Json) -> Option<Vec<(LogicalType, Scalar)>> {
    match value {
        // ISO-8601 text also names a datetime instant.
        Json::String(s) => {
            let mut out = vec![(LogicalType::Utf8, Scalar::Utf8(s.clone()))];
            if let Some(ms) = crate::datetime::parse_iso_8601(s) {
                out.push((LogicalType::DateTime, Scalar::DateTime(ms)));
            }
            Some(out)
        }
        Json::Bool(b) => Some(vec![(LogicalType::Boolean, Scalar::Boolean(*b))]),
        Json::Number(n) => {
            let mut out = Vec::new();
            // An integer also probes datetimes, which compare as their epoch
            // milliseconds.
            let push_integer = |out: &mut Vec<(LogicalType, Scalar)>, i: i64| {
                out.push((LogicalType::Int64, Scalar::Int64(i)));
                out.push((LogicalType::DateTime, Scalar::DateTime(i)));
            };
            // Prefer the exact i64 representation when the JSON parser kept
            // one, so integers beyond f64's exact range are not rounded.
            if let Some(i) = n.as_i64() {
                push_integer(&mut out, i);
            }
            if let Some(f) = n.as_f64() {
                let matches_integer = n
                    .as_i64()
                    .is_some_and(|i| scalar_equal(&Scalar::Int64(i), &Scalar::Float64(f)));
                let is_fractional = n.as_i64().is_none() && !n.is_u64();
                if f.is_finite() && (matches_integer || is_fractional) {
                    out.push((LogicalType::Float64, Scalar::Float64(f)));
                }
                // Numeric equality treats both signed zeros and integer zero
                // as equal. Probe every corresponding posting so the index
                // path has the same candidate set as the evaluator.
                if f == 0.0 {
                    if !out
                        .iter()
                        .any(|(_, scalar)| matches!(scalar, Scalar::Int64(0)))
                    {
                        push_integer(&mut out, 0);
                    }
                    out.retain(
                        |(_, scalar)| !matches!(scalar, Scalar::Float64(value) if *value == 0.0),
                    );
                    out.push((LogicalType::Float64, Scalar::Float64(-0.0)));
                    out.push((LogicalType::Float64, Scalar::Float64(0.0)));
                }
                if !n.is_u64() && n.as_i64().is_none() && f.fract() == 0.0 && f >= i64::MIN as f64 {
                    // `i64::MAX as f64` rounds up to 2^63.  Require the
                    // narrowed value to round-trip exactly so that an
                    // out-of-domain float cannot alias i64::MAX.
                    let i = f as i64;
                    if (i as f64).to_bits() == f.to_bits() && f < 2_f64.powi(63) {
                        push_integer(&mut out, i);
                    }
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        }
        _ => None,
    }
}

/// Exact most-selective index-leaf cardinality for a predicate, for `EXPLAIN`
///. Uses the maintained derived posting cardinality when the view
/// carries it, so `EXPLAIN` need not materialize candidates. Returns `None`
/// when no leaf is index-eligible (a full scan).
///
/// # Errors
/// Propagates storage/engine read errors from the estimate lookup.
pub fn explain_cardinality<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    labels: &[String],
    where_: &Expr,
) -> Result<Option<u64>, QueryError> {
    let mut leaves = Vec::new();
    collect_leaves(where_, &mut leaves);
    let mut best: Option<u64> = None;
    for leaf in leaves {
        // Range leaves now get a bounded histogram *estimate* from
        // `leaf_estimate` for the planner's own cost ordering, but this
        // field is contractually exact — excluded structurally (by leaf
        // shape), not by filtering the `u64::MAX` sentinel, so that estimate
        // can never leak in here. See `is_range_leaf`.
        if is_range_leaf(leaf) {
            continue;
        }
        if let Some(estimate) = leaf_estimate(snap, tenant, leaf, labels)? {
            // `u64::MAX` marks an unknown/unbounded leaf; only a known exact
            // cardinality is useful for EXPLAIN.
            if estimate != u64::MAX {
                best = Some(best.map_or(estimate, |current| current.min(estimate)));
            }
        }
    }
    Ok(best)
}

/// The unindexed walk a query would start with, for `EXPLAIN`: the
/// property of the most selective eligible leaf when no declared ordered
/// index serves it, and the owners carrying that property (under `labels`
/// when given), which the planner visits one point read at a time.
///
/// # Errors
/// Propagates storage reads from the estimate and support lookups.
pub fn explain_walk<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    labels: &[String],
    where_: &Expr,
) -> Result<Option<(String, u64, Vec<String>)>, QueryError> {
    let mut leaves = Vec::new();
    collect_leaves(where_, &mut leaves);
    let mut eligible = Vec::new();
    for leaf in leaves {
        if let Some(estimate) = leaf_estimate(snap, tenant, leaf, labels)? {
            eligible.push((estimate, leaf));
        }
    }
    eligible.sort_by_key(|(estimate, _)| *estimate);
    let Some((_, leaf)) = eligible.first() else {
        return Ok(None);
    };
    if crate::index_access::answers(snap, tenant, leaf, labels)? {
        return Ok(None);
    }
    let Expr::Field(
        FieldPred::Eq(name, _)
        | FieldPred::In(name, _)
        | FieldPred::Gt(name, _)
        | FieldPred::Gte(name, _)
        | FieldPred::Lt(name, _)
        | FieldPred::Lte(name, _),
    ) = leaf
    else {
        return Ok(None);
    };
    let norm = normalize(name);
    let Some(fid) = family_id(snap, tenant, &norm)? else {
        return Ok(None);
    };
    let kind = rushdb_types::ids::OwnerKind::Record;
    let mut owners = 0u64;
    let mut walked_labels = std::collections::BTreeSet::new();
    for ty in [
        LogicalType::Utf8,
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::Boolean,
        LogicalType::DateTime,
    ] {
        let Some(pid) = property_id(snap, tenant, fid, ty)? else {
            continue;
        };
        for label in labels_for(snap, tenant, pid, labels)? {
            owners = owners.saturating_add(rushdb_lmpg::derived::prop_support_label(
                snap, tenant, kind, pid, &label,
            )?);
            walked_labels.insert(label);
        }
    }
    Ok(Some((norm, owners, walked_labels.into_iter().collect())))
}
