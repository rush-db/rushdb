//! Declared ordered indexes in the planner (SD-4.4): when every
//! requested label has a `Ready` ordered index on a leaf's property, the leaf
//! is answered from `INDEX_ENTRY` rows instead of a presence-narrowed walk.
//! Each candidate entry is re-checked against the predicate, so the owner
//! set is exact.

use rushdb_lmpg::index;
use rushdb_lmpg::reader::TryRead;
use rushdb_types::encoding::{decode_order_preserving, encode_order_preserving, logical_tag};
use rushdb_types::ids::TenantId;
use rushdb_types::index::IndexDef;
use rushdb_types::model::LogicalType;
use rushdb_types::value::{normalize_name, Scalar};

use crate::planner::{extend_owners, range_scalar_matches, scalar_variants, OwnerSet, RangeOp};
use crate::{Expr, FieldPred, QueryError};

type Json = serde_json::Value;

/// The `Ready` ordered indexes covering property `name` for every label in
/// `labels` (one per label), or `None` when some label has none or no label
/// is given (an index belongs to a label).
pub(crate) fn covering<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    name: &str,
    labels: &[String],
) -> Result<Option<Vec<IndexDef>>, QueryError> {
    let norm = normalize_name(name);
    if labels.is_empty() || norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(labels.len());
    for label in labels {
        match index::ready_ordered(snap, tenant, label, &norm)? {
            Some(def) => out.push(def),
            None => return Ok(None),
        }
    }
    Ok(Some(out))
}

/// `[start, end)` covering every entry that begins with `prefix`.
fn prefix_bounds(prefix: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let end = rushdb_storage::prefix_range(&prefix).1;
    (prefix, end)
}

fn scan<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    defs: &[IndexDef],
    (start, end): (&[u8], &[u8]),
    keep: &dyn Fn(&[u8]) -> bool,
    owners: &mut OwnerSet,
    owner_limit: usize,
) -> Result<(), QueryError> {
    // Streamed straight into the owner set: an entry range of millions of
    // owners never exists as a row list.
    let mut over_budget = None;
    for def in defs {
        index::for_each_entry(snap, tenant, def.id, start, end, &mut |entry, owner| {
            if !keep(entry) {
                return true;
            }
            match extend_owners(owners, std::iter::once(owner), owner_limit) {
                Ok(()) => true,
                Err(error) => {
                    over_budget = Some(error);
                    false
                }
            }
        })?;
        if let Some(error) = over_budget {
            return Err(error);
        }
    }
    Ok(())
}

fn entry_of(ty: LogicalType, scalar: &Scalar) -> Option<Vec<u8>> {
    let mut entry = vec![logical_tag(ty)];
    entry.extend(encode_order_preserving(ty, scalar).ok()?);
    Some(entry)
}

/// Owners whose value equals any of `values` (every type variant a value
/// may be stored under, as the presence path probes).
fn equal<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    defs: &[IndexDef],
    values: &[Json],
    owner_limit: usize,
) -> Result<Option<OwnerSet>, QueryError> {
    let mut owners = OwnerSet::new();
    for value in values {
        let Some(variants) = scalar_variants(value) else {
            return Ok(None);
        };
        for (ty, scalar) in variants {
            let Some(entry) = entry_of(ty, &scalar) else {
                return Ok(None);
            };
            let (start, end) = prefix_bounds(entry.clone());
            scan(
                snap,
                tenant,
                defs,
                (&start, &end),
                &|found| found == entry.as_slice(),
                &mut owners,
                owner_limit,
            )?;
        }
    }
    Ok(Some(owners))
}

/// Owners holding a numeric value satisfying `op bound`.
fn range<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    defs: &[IndexDef],
    op: RangeOp,
    bound: f64,
    owner_limit: usize,
) -> Result<Option<OwnerSet>, QueryError> {
    let mut owners = OwnerSet::new();
    let above = matches!(op, RangeOp::Gt | RangeOp::Gte);
    for ty in [
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::DateTime,
    ] {
        let tag = logical_tag(ty);
        // Bound the scan at the bound's encoding, rounded outward for the
        // integer domains; the exact comparison happens on each entry.
        let edge = match ty {
            LogicalType::Float64 => Scalar::Float64(bound),
            LogicalType::Int64 => Scalar::Int64(outward(bound, above)),
            _ => Scalar::DateTime(outward(bound, above)),
        };
        let Some(edge) = entry_of(ty, &edge) else {
            continue;
        };
        let (start, end) = if above {
            (edge, vec![tag + 1])
        } else {
            (vec![tag], rushdb_storage::prefix_range(&edge).1)
        };
        scan(
            snap,
            tenant,
            defs,
            (&start, &end),
            &|entry| {
                entry.split_first().is_some_and(|(&found, payload)| {
                    found == tag
                        && decode_order_preserving(ty, payload)
                            .is_ok_and(|scalar| range_scalar_matches(&scalar, op, bound))
                })
            },
            &mut owners,
            owner_limit,
        )?;
    }
    Ok(Some(owners))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "floor/ceil made the bound integral; `as` saturates at the i64 edges"
)]
fn outward(bound: f64, above: bool) -> i64 {
    if above {
        bound.floor() as i64
    } else {
        bound.ceil() as i64
    }
}

/// Owners holding a string starting with `prefix`.
fn starts_with<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    defs: &[IndexDef],
    prefix: &str,
    owner_limit: usize,
) -> Result<Option<OwnerSet>, QueryError> {
    let mut entry = vec![logical_tag(LogicalType::Utf8)];
    entry.extend_from_slice(prefix.as_bytes());
    let (start, end) = prefix_bounds(entry);
    let mut owners = OwnerSet::new();
    scan(
        snap,
        tenant,
        defs,
        (&start, &end),
        &|entry| {
            decode_order_preserving(LogicalType::Utf8, &entry[1..]).is_ok_and(
                |scalar| matches!(scalar, Scalar::Utf8(text) if text.starts_with(prefix)),
            )
        },
        &mut owners,
        owner_limit,
    )?;
    Ok(Some(owners))
}

/// The leaf's property name when an ordered index can answer it.
fn indexed_property(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Field(
            FieldPred::Eq(name, _)
            | FieldPred::In(name, _)
            | FieldPred::Gt(name, _)
            | FieldPred::Gte(name, _)
            | FieldPred::Lt(name, _)
            | FieldPred::Lte(name, _)
            | FieldPred::StartsWith(name, _),
        ) => Some(name),
        _ => None,
    }
}

/// Exact owners of `expr` from declared indexes, or `None` when no `Ready`
/// ordered index covers it for every label in `labels`.
pub(crate) fn leaf_owners<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    expr: &Expr,
    labels: &[String],
    owner_limit: usize,
) -> Result<Option<OwnerSet>, QueryError> {
    let Some(name) = indexed_property(expr) else {
        return Ok(None);
    };
    let Some(defs) = covering(snap, tenant, name, labels)? else {
        return Ok(None);
    };
    match expr {
        Expr::Field(FieldPred::Eq(_, value)) if !value.is_null() => equal(
            snap,
            tenant,
            &defs,
            std::slice::from_ref(value),
            owner_limit,
        ),
        Expr::Field(FieldPred::In(_, values)) if values.iter().all(|v| !v.is_null()) => {
            equal(snap, tenant, &defs, values, owner_limit)
        }
        Expr::Field(FieldPred::Gt(_, bound)) => {
            range(snap, tenant, &defs, RangeOp::Gt, *bound, owner_limit)
        }
        Expr::Field(FieldPred::Gte(_, bound)) => {
            range(snap, tenant, &defs, RangeOp::Gte, *bound, owner_limit)
        }
        Expr::Field(FieldPred::Lt(_, bound)) => {
            range(snap, tenant, &defs, RangeOp::Lt, *bound, owner_limit)
        }
        Expr::Field(FieldPred::Lte(_, bound)) => {
            range(snap, tenant, &defs, RangeOp::Lte, *bound, owner_limit)
        }
        Expr::Field(FieldPred::StartsWith(_, prefix)) => {
            starts_with(snap, tenant, &defs, prefix, owner_limit)
        }
        _ => Ok(None),
    }
}

/// Whether a declared index answers `expr` (a `startsWith` leaf is only
/// eligible through one).
pub(crate) fn answers<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    expr: &Expr,
    labels: &[String],
) -> Result<bool, QueryError> {
    let Some(name) = indexed_property(expr) else {
        return Ok(false);
    };
    Ok(covering(snap, tenant, name, labels)?.is_some())
}

/// Name of the first declared index the predicate's leaves can use, for
/// `EXPLAIN`'s access path.
///
/// # Errors
/// Returns the underlying storage error from an index-catalog read.
pub fn explain_index<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    labels: &[String],
    where_: &Expr,
) -> Result<Option<String>, QueryError> {
    let mut leaves = Vec::new();
    crate::planner::collect_leaves(where_, &mut leaves);
    for leaf in leaves {
        if let Some(name) = indexed_property(leaf) {
            if let Some(defs) = covering(snap, tenant, name, labels)? {
                return Ok(Some(
                    defs.iter()
                        .map(|def| def.name.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                ));
            }
        }
    }
    Ok(None)
}
