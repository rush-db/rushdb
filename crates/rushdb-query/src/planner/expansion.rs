//! Which `$relation` conjuncts the planner answers by expanding backwards
//! from the records they match, and how large each expansion is expected
//! to be, so it is ordered among the leaves by expected set size.

use rushdb_lmpg::reader::TryRead;

use super::{collect_leaves, leaf_estimate};
use crate::{Expr, QueryError, RelatedExpr};

/// Which traversal conjuncts the planner answers by expanding backwards
/// from their targets, and which it leaves to a per-candidate check.
pub(crate) struct Expansions<'a> {
    /// `(expected records reached, block)`.
    pub(crate) expand: Vec<(u64, &'a RelatedExpr)>,
    pub(crate) residual: Vec<&'a RelatedExpr>,
}

/// A traversal expands from its targets when they are at most half as many
/// as the records the root would check otherwise (its most selective other
/// condition, or its labels): both plans visit about one neighbourhood per
/// record they start from, and the backward plan also checks each target.
pub(crate) fn expansions<'a, R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    labels: &[String],
    eligible: &[(u64, &Expr)],
    traversals: Vec<&'a RelatedExpr>,
) -> Result<Expansions<'a>, QueryError> {
    let mut out = Expansions {
        expand: Vec::new(),
        residual: Vec::new(),
    };
    if traversals.is_empty() {
        return Ok(out);
    }
    let records = records_of(snap, tenant, labels)?;
    let candidates = match eligible.iter().map(|(estimate, _)| *estimate).min() {
        Some(estimate) if estimate != u64::MAX => estimate,
        _ => records,
    };
    let usage = rushdb_lmpg::derived::tenant_usage(snap, tenant)?;
    for rel in traversals {
        let targets = target_estimate(snap, tenant, rel)?;
        if targets.saturating_mul(2) <= candidates {
            let reach = reach_estimate(targets, rel, &usage).min(records);
            out.expand.push((reach, rel));
        } else {
            out.residual.push(rel);
        }
    }
    Ok(out)
}

/// Records an expansion from `targets` records is expected to reach: the
/// targets times the tenant's average degree (per direction walked) to the
/// power of each hop count up to `max`.
fn reach_estimate(
    targets: u64,
    rel: &RelatedExpr,
    usage: &rushdb_lmpg::derived::TenantUsage,
) -> u64 {
    let directions = if rel.direction == crate::Direction::Both {
        2
    } else {
        1
    };
    // Rounded up, at least one: an estimate, so overshooting is the safe
    // side (the expansion is ordered later, never skipped).
    let degree = usage
        .relationships
        .saturating_mul(directions)
        .div_ceil(usage.records.max(1))
        .max(1);
    let (_, max) = crate::expand::hop_bounds(rel);
    let mut layer = targets;
    let mut reached = 0u64;
    for _ in 0..max {
        layer = layer.saturating_mul(degree);
        reached = reached.saturating_add(layer);
        if reached == u64::MAX {
            break;
        }
    }
    reached
}

/// Upper bound of the records a traversal block's labels and filter match:
/// the filter's most selective exact leaf, else its labels' record count.
pub(crate) fn target_estimate<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    rel: &RelatedExpr,
) -> Result<u64, QueryError> {
    let mut best = records_of(snap, tenant, &rel.target_labels)?;
    if let Some(filter) = &rel.filter {
        let mut leaves = Vec::new();
        collect_leaves(filter, &mut leaves);
        for leaf in leaves {
            if let Some(estimate) = leaf_estimate(snap, tenant, leaf, &rel.target_labels)? {
                best = best.min(estimate);
            }
        }
    }
    Ok(best)
}

/// Records carrying any of `labels` (every record when empty), from the
/// maintained counters.
pub(crate) fn records_of<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    labels: &[String],
) -> Result<u64, QueryError> {
    if labels.is_empty() {
        return Ok(rushdb_lmpg::derived::tenant_usage(snap, tenant)?.records);
    }
    let mut records = 0u64;
    for label in labels {
        records = records.saturating_add(rushdb_lmpg::derived::label_count(snap, tenant, label)?);
    }
    Ok(records)
}
