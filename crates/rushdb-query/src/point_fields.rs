//! Record properties read by point reads instead of rebuilding the record
//!: the ordering properties of an ordered page, the properties a
//! residual predicate names, and the columns a SQL label scan reads.

use crate::aggregate_rows::Cell;
use crate::planner::{family_id, pids_of};
use crate::{access, normalize, reconstruct, Expr, FieldPred, Json, QueryError, QuerySnapshot};
use rushdb_lmpg::reader::TryRead;
use rushdb_types::value::ID_FIELD;
use serde_json::Map;

/// Record properties read with one point read per typed definition instead
/// of rebuilding the record (READ-1: sorting 667K matches, or checking an
/// unindexed `age < 25` on them, rebuilt every one). Covers scalar record
/// properties and the record id (`__id`, read from the record header);
/// [`Self::resolve`] answers `None` for another system field or a property
/// stored as an array anywhere, and the caller rebuilds the record.
///
/// A record holds one value per property: a write deletes the value of
/// every typed definition of the property before it stores the new one
/// (also when one batch writes the property twice). So the first definition
/// holding a value is the only one, and definitions are probed in the order
/// most records hold them: by descending owner support (under the read's
/// labels). A CSV column inferred as both Int64 and Float64 is read with one
/// point read per row for most rows, not two.
pub struct PointFields(Vec<(String, Source)>);

/// Where one point field is read from.
enum Source {
    /// The record id, from the record header.
    Id,
    /// The typed definitions of a scalar property, in probe order.
    Pids(Vec<u64>),
}

impl PointFields {
    /// Point reads for `names`, each row keyed by the name as given; `None`
    /// when one of them needs the whole record. Definitions are probed by
    /// their tenant-wide support.
    ///
    /// # Errors
    /// Returns storage read errors from the property catalog.
    pub fn resolve<'n>(
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        names: impl IntoIterator<Item = &'n str>,
    ) -> Result<Option<Self>, QueryError> {
        Self::resolve_in(snap, tenant, &[], names)
    }

    /// [`Self::resolve`] for records of `labels` (every label when empty):
    /// definitions are probed by their support under those labels.
    ///
    /// # Errors
    /// Returns storage read errors from the property catalog.
    pub fn resolve_in<'n>(
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        labels: &[String],
        names: impl IntoIterator<Item = &'n str>,
    ) -> Result<Option<Self>, QueryError> {
        let mut fields = Vec::new();
        for name in names {
            let source = if normalize(name) == ID_FIELD {
                Source::Id
            } else {
                let Some(pids) = scalar_pids(snap, tenant, labels, name)? else {
                    return Ok(None);
                };
                Source::Pids(pids)
            };
            fields.push((name.to_owned(), source));
        }
        Ok(Some(Self(fields)))
    }

    /// The properties `predicate` reads, or `None` when it needs the whole
    /// record.
    pub(super) fn for_predicate(
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        labels: &[String],
        predicate: &Expr,
    ) -> Result<Option<Self>, QueryError> {
        let mut names = std::collections::BTreeSet::new();
        if !predicate_fields(predicate, &mut names) {
            return Ok(None);
        }
        Self::resolve_in(snap, tenant, labels, names.iter().map(String::as_str))
    }

    /// The fields of record `ord`: each one's value, as the rebuilt record
    /// would carry it.
    ///
    /// # Errors
    /// Returns storage read errors and corrupt-occurrence errors.
    pub fn row(
        &self,
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        ord: u64,
    ) -> Result<Map<String, Json>, QueryError> {
        let mut row = Map::new();
        for (field, source) in &self.0 {
            match source {
                Source::Id => {
                    if let Some(id) = record_id(snap, tenant, ord)? {
                        row.insert(field.clone(), Json::String(id));
                    }
                }
                Source::Pids(pids) => {
                    if let Some(occ) = held(snap, tenant, ord, pids)? {
                        row.insert(
                            field.clone(),
                            reconstruct::decode_value(occ.value, &occ.payload)?,
                        );
                    }
                }
            }
        }
        Ok(row)
    }

    /// The fields of record `ord` as typed cells, one per field in the order
    /// the fields were resolved (`None`: the record does not hold it).
    pub(crate) fn cells(
        &self,
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        ord: u64,
        out: &mut Vec<Option<Cell>>,
    ) -> Result<(), QueryError> {
        out.clear();
        for (_, source) in &self.0 {
            out.push(match source {
                Source::Id => record_id(snap, tenant, ord)?.map(Cell::Str),
                Source::Pids(pids) => match held(snap, tenant, ord, pids)? {
                    Some(occ) => Some(Cell::decode(occ.value, &occ.payload)?),
                    None => None,
                },
            });
        }
        Ok(())
    }
}

/// The id of record `ord`, as the rebuilt record carries it.
fn record_id(
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ord: u64,
) -> Result<Option<String>, QueryError> {
    Ok(access::record_by_ord(snap, tenant, ord)?
        .map(|header| reconstruct::id_string(&header.record_id)))
}

/// The occurrence record `ord` holds under one of `pids` (probed in order).
fn held(
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ord: u64,
    pids: &[u64],
) -> Result<Option<access::Occurrence>, QueryError> {
    for &pid in pids {
        if let Some(occ) = access::occurrence(
            snap,
            tenant,
            rushdb_types::ids::OwnerKind::Record,
            ord,
            pid,
            0,
        )? {
            return Ok(Some(occ));
        }
    }
    Ok(None)
}

/// Collect the property names `expr` reads into `out`; `false` when it needs
/// the whole record (`$id`). A traversal reads none of the record's
/// properties: it starts from the record's ordinal.
pub(super) fn predicate_fields(expr: &Expr, out: &mut std::collections::BTreeSet<String>) -> bool {
    match expr {
        Expr::And(items) | Expr::Or(items) | Expr::Nor(items) => {
            items.iter().all(|item| predicate_fields(item, out))
        }
        Expr::Xor(a, b) => predicate_fields(a, out) && predicate_fields(b, out),
        Expr::Not(inner) => predicate_fields(inner, out),
        Expr::Related(_) => true,
        Expr::Field(FieldPred::Id(_)) => false,
        Expr::Field(
            FieldPred::Eq(name, _)
            | FieldPred::Ne(name, _)
            | FieldPred::Gt(name, _)
            | FieldPred::Gte(name, _)
            | FieldPred::Lt(name, _)
            | FieldPred::Lte(name, _)
            | FieldPred::Contains(name, _)
            | FieldPred::StartsWith(name, _)
            | FieldPred::EndsWith(name, _)
            | FieldPred::Exists(name, _)
            | FieldPred::IsNull(name, _)
            | FieldPred::In(name, _)
            | FieldPred::Nin(name, _)
            | FieldPred::Type(name, _)
            | FieldPred::DatetimeComponent { field: name, .. },
        ) => {
            out.insert(name.clone());
            true
        }
    }
}

/// Typed definitions holding record property `name` as a scalar, in probe
/// order: most owners (under `labels`, every label when empty) first, ties
/// by id. `None` when the name is a system field or any label stores the
/// property as an array: those need the whole record.
fn scalar_pids<R: TryRead>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    labels: &[String],
    name: &str,
) -> Result<Option<Vec<u64>>, QueryError> {
    let norm = normalize(name);
    if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let Some(fid) = family_id(snap, tenant, &norm)? else {
        return Ok(Some(Vec::new()));
    };
    let kind = rushdb_types::ids::OwnerKind::Record;
    let mut by_support = Vec::new();
    for pid in pids_of(snap, tenant, fid)? {
        let (arrays, _) = rushdb_lmpg::derived::array_markers(snap, tenant, kind, pid)?;
        if arrays {
            return Ok(None);
        }
        let support = if labels.is_empty() {
            rushdb_lmpg::derived::prop_support_total(snap, tenant, kind, pid)?
        } else {
            let mut support = 0u64;
            let distinct: std::collections::BTreeSet<&String> = labels.iter().collect();
            for label in distinct {
                support = support.saturating_add(rushdb_lmpg::derived::prop_support_label(
                    snap, tenant, kind, pid, label,
                )?);
            }
            support
        };
        by_support.push((std::cmp::Reverse(support), pid));
    }
    by_support.sort_unstable();
    Ok(Some(by_support.into_iter().map(|(_, pid)| pid).collect()))
}

/// The properties a collect with a `fields` projection reads of each child:
/// the projected sources, the ordering fields and the filter's fields, by
/// either spelling. `None` (whole child) without `fields`, or when the
/// filter needs the whole record (`$id`, a traversal).
pub(super) fn collect_selection(
    spec: &crate::CollectSpec,
) -> Option<std::collections::HashSet<String>> {
    if spec.fields.is_empty() {
        return None;
    }
    let mut names = std::collections::BTreeSet::new();
    if let Some(filter) = &spec.filter {
        if !predicate_fields(filter, &mut names) {
            return None;
        }
    }
    for (_, source) in &spec.fields {
        names.insert(source.strip_prefix("$record.").unwrap_or(source).to_owned());
    }
    for (field, _) in &spec.order_by {
        names.insert(field.clone());
    }
    Some(
        names
            .into_iter()
            .flat_map(|name| [normalize(&name), name])
            .collect(),
    )
}
