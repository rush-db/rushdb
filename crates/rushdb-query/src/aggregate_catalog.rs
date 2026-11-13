//! Grouped counts answered from the derived catalog instead of the records:
//! the value dictionary holds the exact owner count per value of a property
//! under a label, maintained in the records' own commits.

use crate::{
    normalize, AggregateRequest, GroupBucket, Json, Lmpg, Map, QueryError, QuerySnapshot,
    MISSING_SENTINEL,
};

use super::aggregation::MAX_AGGREGATE_BUCKETS;

/// `groupBy` on one property with only `$count: "*"` metrics, over one
/// label with no filter or traversal: one row per distinct value, counted
/// from the value dictionary (the exact owner count per value, maintained
/// in the records' own commits), plus a row for explicit nulls and one for
/// records without the property, as the record loop would key them. No
/// record is read. `None` when the shape, or how the property is stored
/// under the label (several typed definitions, arrays, values too long for
/// the dictionary), needs the record loop.
///
/// # Errors
/// Returns storage errors, a corrupt dictionary row, the group cap, or
/// [`QueryError::Cancelled`] (polled per dictionary entry).
pub(crate) fn dictionary_grouped_count(
    lmpg: &Lmpg,
    req: &AggregateRequest,
    snap: &QuerySnapshot,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<GroupBucket>>, QueryError> {
    use rushdb_lmpg::derived;
    use rushdb_types::ids::OwnerKind;
    use rushdb_types::model::TypeState;
    let only_root_counts =
        !req.counts.is_empty() && req.counts.iter().all(|(_, field)| field == "*");
    if req.group_by.len() != 1
        || req.labels.len() != 1
        || !only_root_counts
        || req.where_.is_some()
        || !req.related.is_empty()
        || req.time_bucket.is_some()
        || !req.plain_fields.is_empty()
        || !req.collects.is_empty()
        || !req.sums.is_empty()
        || !req.mins.is_empty()
        || !req.maxs.is_empty()
        || !req.outputs.is_empty()
        || !req.expr_sums.is_empty()
        || !req.expr_avgs.is_empty()
        || !req.avg_terms.is_empty()
        || rushdb_lmpg::heal::untrusted(snap.as_ref(), lmpg.tenant, &req.labels)
    {
        return Ok(None);
    }
    let group = &req.group_by[0];
    let norm = normalize(group.strip_prefix("$record.").unwrap_or(group));
    if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
        return Ok(None);
    }
    let tenant = lmpg.tenant;
    let label = &req.labels[0];
    let labels = std::slice::from_ref(label);
    let total = derived::label_count(snap, tenant, label)?;
    let bucket = |value: Json, count: u64| GroupBucket {
        keys: Map::from_iter([(group.clone(), value)]),
        count_roots: count,
        ..GroupBucket::default()
    };
    let missing = || Json::String(MISSING_SENTINEL.into());
    let held = match crate::planner::family_id(snap, tenant, &norm)? {
        Some(fid) => definitions_held_under(snap, tenant, fid, label)?,
        None => Vec::new(),
    };
    let [(state, pid, support)] = held[..] else {
        if held.is_empty() {
            // No record of the label carries the property.
            return Ok(Some(if total == 0 {
                Vec::new()
            } else {
                vec![bucket(missing(), total)]
            }));
        }
        // Type drift: which value a record shows is decided per record.
        return Ok(None);
    };
    let (_, array_labels) = derived::array_markers(snap, tenant, OwnerKind::Record, pid)?;
    if array_labels.iter().any(|l| l == label)
        || derived::long_values(snap, tenant, OwnerKind::Record, pid, labels)? > 0
    {
        return Ok(None);
    }
    let (mut buckets, counted) = match state {
        TypeState::Known(ty) => {
            dictionary_buckets(snap, tenant, pid, ty, labels, &bucket, is_cancelled)?
        }
        TypeState::Unresolved => (Vec::new(), 0),
    };
    let nulls = support.saturating_sub(counted);
    if nulls > 0 {
        buckets.push(bucket(Json::Null, nulls));
    }
    let absent = total.saturating_sub(support);
    if absent > 0 {
        buckets.push(bucket(missing(), absent));
    }
    if buckets.len() > MAX_AGGREGATE_BUCKETS {
        return Err(too_many_groups());
    }
    Ok(Some(buckets))
}

/// The typed definitions of family `fid` that records of `label` hold:
/// `(type state, pid, owners under the label)`.
fn definitions_held_under(
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    fid: u64,
    label: &str,
) -> Result<Vec<(rushdb_types::model::TypeState, u64, u64)>, QueryError> {
    let mut held = Vec::new();
    for state in crate::planner::all_type_states() {
        let key = rushdb_lmpg::reader::keys_ext::definition_key(tenant, fid, state);
        let Some(raw) = snap.get(&key)? else {
            continue;
        };
        let pid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
        );
        let support = rushdb_lmpg::derived::prop_support_label(
            snap,
            tenant,
            rushdb_types::ids::OwnerKind::Record,
            pid,
            label,
        )?;
        if support > 0 {
            held.push((state, pid, support));
        }
    }
    Ok(held)
}

pub(crate) fn too_many_groups() -> QueryError {
    QueryError::ResourceLimit(format!(
        "aggregate exceeds {MAX_AGGREGATE_BUCKETS} distinct groups; narrow the groupBy or add filters"
    ))
}

/// One bucket per dictionary entry of `(pid, ty)` under `labels`, and how
/// many owners they account for together.
fn dictionary_buckets(
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    pid: u64,
    ty: rushdb_types::model::LogicalType,
    labels: &[String],
    bucket: &dyn Fn(Json, u64) -> GroupBucket,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<GroupBucket>, u64), QueryError> {
    let mut buckets = Vec::new();
    let mut counted = 0u64;
    let mut failure: Option<QueryError> = None;
    rushdb_lmpg::derived::for_each_dictionary_entry(
        snap,
        tenant,
        rushdb_types::ids::OwnerKind::Record,
        pid,
        labels,
        &mut |entry, count| {
            if is_cancelled() {
                failure = Some(QueryError::Cancelled);
                return false;
            }
            let Some(payload) = entry.get(1..) else {
                failure = Some(rushdb_storage::StorageError::CorruptLog.into());
                return false;
            };
            let Ok(scalar) = rushdb_types::encoding::decode_order_preserving(ty, payload) else {
                failure = Some(rushdb_storage::StorageError::CorruptLog.into());
                return false;
            };
            if buckets.len() >= MAX_AGGREGATE_BUCKETS {
                failure = Some(too_many_groups());
                return false;
            }
            counted = counted.saturating_add(count);
            // Rendered as reconstruction renders a stored scalar.
            buckets.push(bucket(rushdb_types::value::scalar_to_json(&scalar), count));
            true
        },
    )?;
    if let Some(error) = failure {
        return Err(error);
    }
    Ok((buckets, counted))
}
