//! Planner answers for `EXPLAIN`: the exact cardinality of the most
//! selective index leaf, and the unindexed walk a query would start with.

use rushdb_lmpg::reader::TryRead;
use rushdb_types::model::LogicalType;

use super::{
    collect_leaves, family_id, is_range_leaf, labels_for, leaf_estimate, normalize, property_id,
};
use crate::{Expr, FieldPred, QueryError};

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
