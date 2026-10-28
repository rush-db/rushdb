//! Budgeted grouping, aggregate evaluation, and result projection.

use super::{
    aggregate_budget, alias_field_values, attach_collects_with_budget, binding_roots_with_cancel,
    canonical_scalar_cmp, count_related, infer_scalar, json_value_cmp, normalize, reconstruct,
    related_specs_from_where, resolve_field, resolve_field_numeric, AggregateRequest, Arc,
    GroupBucket, Json, Lmpg, Map, OutputExpr, Pagination, QueryError, QuerySnapshot, Scalar,
    SearchRequest, MISSING_SENTINEL,
};

/// Branch-grain aggregation over a `BindingRelation` of distinct roots:
/// one root contributes exactly one binding row, independent branches are
/// filtered existentially rather than multiplied into flat rows.
///
/// # Errors
///
/// Propagates storage/engine read errors and predicate evaluation failures as
/// [`QueryError`].
pub fn aggregate(lmpg: &Lmpg, req: &AggregateRequest) -> Result<Vec<GroupBucket>, QueryError> {
    aggregate_with_cancel(lmpg, req, &|| false).map(|(buckets, _)| buckets)
}

/// Maximum number of distinct buckets a single aggregation may create before
/// it is refused with [`QueryError::ResourceLimit`].
pub const MAX_AGGREGATE_BUCKETS: usize = 1_000;

/// Aggregate execution with cooperative cancellation at root and bucket
/// boundaries. Returns the completed buckets plus the total number of matched
/// roots (the `total` reported by the data plane, matching a search result's
/// total).
///
/// # Errors
///
/// Returns [`QueryError::Cancelled`] when the probe requests a stop, and
/// [`QueryError::ResourceLimit`] when more than [`MAX_AGGREGATE_BUCKETS`]
/// distinct groups would be created. Storage/engine and predicate errors are
/// propagated from the underlying reads.
///
/// # Panics
///
/// Panics only if a bucket just inserted by `ensure_bucket` cannot be
/// retrieved, which is unreachable.
pub fn aggregate_with_cancel(
    lmpg: &Lmpg,
    req: &AggregateRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<GroupBucket>, u64), QueryError> {
    let snap = Arc::new(lmpg.store.read_view()?);
    aggregate_with_cancel_in_view(lmpg, req, is_cancelled, snap)
}

/// Aggregate execution over a caller-owned pinned read view. Embedded
/// engines use this to keep every phase of a request on one snapshot and to
/// apply the same deadline/budget context as the transport path.
///
/// # Errors
/// Propagates storage, predicate, cancellation and aggregate resource errors.
pub fn aggregate_in_view(
    lmpg: &Lmpg,
    req: &AggregateRequest,
    snap: QuerySnapshot,
) -> Result<(Vec<GroupBucket>, u64), QueryError> {
    aggregate_with_cancel_in_view(lmpg, req, &|| false, snap)
}

/// Execute an aggregate on the supplied view using the configured query budget.
///
/// # Errors
/// Returns storage, predicate, cancellation, or resource errors.
pub fn aggregate_with_cancel_in_view(
    lmpg: &Lmpg,
    req: &AggregateRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
) -> Result<(Vec<GroupBucket>, u64), QueryError> {
    aggregate_with_cancel_in_view_with_budget(lmpg, req, is_cancelled, snap, None)
}

/// Execute an aggregate with one caller budget across working state and groups.
/// An explicit zero budget rejects allocations; an absent budget uses storage configuration.
///
/// # Errors
/// Returns storage, predicate, cancellation, or resource errors.
pub fn aggregate_with_cancel_in_view_with_budget(
    lmpg: &Lmpg,
    req: &AggregateRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
    work_budget: Option<usize>,
) -> Result<(Vec<GroupBucket>, u64), QueryError> {
    let is_cancelled: &dyn Fn() -> bool = &crate::cancel_probe(lmpg, is_cancelled);
    if !snap.belongs_to(&lmpg.store) {
        return Err(rushdb_storage::StorageError::InvalidRead(
            "aggregate view belongs to another Store".into(),
        )
        .into());
    }
    if is_cancelled() {
        return Err(QueryError::Cancelled);
    }
    let configured = lmpg
        .limits()
        .query_memory_max_bytes()
        .map(|bytes| usize::try_from(bytes).unwrap_or(usize::MAX));
    let budget = match (work_budget, configured) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    aggregate_budget::check(budget, std::mem::size_of::<GroupBucket>())?;
    let snap = match budget {
        Some(bytes) => snap.with_materialization_limit(bytes / 16),
        None => snap,
    };
    aggregate_budgeted(lmpg, req, is_cancelled, snap, budget)
        .map_err(aggregate_budget::resource_error)
}

#[expect(
    clippy::too_many_lines,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value,
    reason = "aggregate accumulation keeps one live budget and pinned snapshot"
)]
fn aggregate_budgeted(
    lmpg: &Lmpg,
    req: &AggregateRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
    budget: Option<usize>,
) -> Result<(Vec<GroupBucket>, u64), QueryError> {
    let page_req = SearchRequest {
        labels: req.labels.clone(),
        where_: req.where_.clone(),
        related: req.related.clone(),
        collects: Vec::new(),
        pagination: Pagination::default(),
        order_by: Vec::new(),
        force_scan: false,
    };
    // Count-only fast path: a single global `$count: "*"` with no grouping or
    // other metrics needs only the match count. A single unscoped label is
    // answered straight from the maintained label catalog (O(labels)); other
    // shapes fall through to the ordinal matcher below, which never
    // reconstructs records to produce a count.
    if is_count_only(req) {
        if let Some(total) = catalog_count_total(lmpg, &page_req, &snap)? {
            let mut buckets = count_only_bucket(total);
            apply_aggregate_pagination(&mut buckets, req);
            return Ok((buckets, total));
        }
    }
    let candidates: Vec<u64> =
        binding_roots_with_cancel(lmpg, &page_req, is_cancelled, &snap, budget)?;
    let candidate_bytes = candidates
        .capacity()
        .saturating_mul(std::mem::size_of::<u64>());
    aggregate_budget::check(budget, candidate_bytes)?;
    let total_matched = candidates.len() as u64;
    if is_count_only(req) {
        let mut buckets = count_only_bucket(total_matched);
        apply_aggregate_pagination(&mut buckets, req);
        return Ok((buckets, total_matched));
    }
    // Aggregate alias targets come from the `where` traversal blocks.
    let alias_specs = related_specs_from_where(req.where_.as_ref());
    let pure_projection = !req.plain_fields.is_empty()
        && req.group_by.is_empty()
        && req.sums.is_empty()
        && req.counts.is_empty()
        && req.mins.is_empty()
        && req.maxs.is_empty()
        && req.outputs.is_empty()
        && req.expr_sums.is_empty()
        && req.expr_avgs.is_empty()
        && req.avg_terms.is_empty()
        && req.time_bucket.is_none();

    // A `select` without groupBy is per-root output-shaping (each root its own
    // row) when it projects root fields or collects. Metrics alone roll up
    // globally; a bare rollup (e.g. timeBucket with no select/groupBy) is a
    // single bucket.
    let per_root =
        req.group_by.is_empty() && (!req.plain_fields.is_empty() || !req.collects.is_empty());
    // Each matched root is its own row, so rows are bounded by the paging
    // and the memory budget, not by the group cap; without an `orderBy` only
    // the requested page is reconstructed.
    let per_row = pure_projection || per_root;
    let candidates = if per_row && req.order_by.is_empty() {
        let page_end = req.skip.saturating_add(req.limit.unwrap_or(u64::MAX));
        candidates
            .into_iter()
            .take(usize::try_from(page_end).unwrap_or(usize::MAX))
            .collect()
    } else {
        candidates
    };

    let mut buckets: Vec<GroupBucket> = Vec::new();
    // Family definitions are frozen within one pinned snapshot: a session
    // caches them per candidate (bounded, semantically invisible — P20).
    let mut retained_bytes = candidate_bytes;
    for ord in candidates {
        let snap = budget.map_or_else(
            || Arc::clone(&snap),
            |limit| snap.with_materialization_limit(limit.saturating_sub(retained_bytes) / 16),
        );
        let reader = aggregate_budget::RecordReader::new(
            &snap,
            budget.map(|limit| limit.saturating_sub(retained_bytes)),
        );
        let mut session = reconstruct::ReconstructSession::new(&reader, lmpg.tenant);
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        let Some(json) = session.reconstruct_record(ord)? else {
            continue;
        };
        let row_bytes = aggregate_budget::value_bytes(&json);
        let metric_count = req
            .counts
            .len()
            .saturating_add(req.sums.len())
            .saturating_add(req.mins.len())
            .saturating_add(req.maxs.len())
            .saturating_add(req.expr_avgs.len())
            .saturating_add(req.expr_sums.len())
            .saturating_add(req.avg_terms.len())
            .saturating_add(req.group_by.len())
            .saturating_add(req.plain_fields.len());
        // Reserve space for row clones and per-row metric/key additions before retaining them.
        let row_allowance = row_bytes.saturating_mul(metric_count.saturating_add(4));
        aggregate_budget::check(budget, retained_bytes.saturating_add(row_allowance))?;
        let alias_budget = budget.map(|limit| {
            limit.saturating_sub(retained_bytes.saturating_add(row_allowance))
                / metric_count.saturating_add(1)
        });
        let obj = json.as_object().cloned().unwrap_or_default();

        let mut bucket_keys: Map<String, Json> = Map::new();
        if pure_projection || per_root {
            // A `select` without groupBy is row projection / per-root
            // aggregation, not a global rollup: the hidden root ordinal keeps
            // each root a distinct row; aggregate_to_rows removes it.
            bucket_keys.insert("__rushdb_projection_root".into(), Json::Number(ord.into()));
        }
        if pure_projection {
            for (output, field) in &req.plain_fields {
                if let Some(value) = resolve_field(&obj, field) {
                    bucket_keys.insert(output.clone(), value);
                }
            }
        } else if per_root {
            // Per-root rows with metrics still carry plain field projections.
            for (output, field) in &req.plain_fields {
                if let Some(value) = resolve_field(&obj, field) {
                    bucket_keys.insert(output.clone(), value);
                }
            }
        }
        for g in &req.group_by {
            // a group-by entry that is a timeBucket alias is resolved by the
            // timeBucket step below; don't leak a separate missing key
            if req.time_bucket_alias.as_deref() == Some(g.as_str()) {
                continue;
            }
            // The key keeps the caller's spelling; lookups by output name
            // compare normalized names.
            let v = resolve_field(&obj, g).unwrap_or(Json::String(MISSING_SENTINEL.into()));
            bucket_keys.insert(g.clone(), v);
        }
        if let Some(tb) = &req.time_bucket {
            let keyed = resolve_field(&obj, &tb.field)
                .and_then(|v| v.as_f64())
                .map_or_else(
                    || Json::String(MISSING_SENTINEL.into()),
                    |ms| Json::Number((tb.floor_millis(ms) as u64).into()),
                );
            // output naming honors the user's `as` alias; the default key is
            // the documented `__bucket_<field>/<unit>` form
            let tb_key = req.time_bucket_alias.clone().unwrap_or_else(|| {
                format!("__bucket_{}/{}", normalize(&tb.field), tb.unit.unit_str())
            });
            bucket_keys.insert(tb_key, keyed);
        }

        if !per_row
            && buckets.len() >= MAX_AGGREGATE_BUCKETS
            && !buckets.iter().any(|b| b.keys == bucket_keys)
        {
            return Err(QueryError::ResourceLimit(format!("aggregate exceeds {MAX_AGGREGATE_BUCKETS} distinct groups; narrow the groupBy or add filters")));
        }
        // Per-row keys carry the root ordinal, so they never repeat.
        let previous_bucket_bytes = if per_row {
            0
        } else {
            buckets
                .iter()
                .find(|b| b.keys == bucket_keys)
                .map_or(0, aggregate_budget::bucket_bytes)
        };
        let bucket = if per_row {
            new_bucket(&mut buckets, bucket_keys)
        } else {
            ensure_bucket(&mut buckets, bucket_keys)
        };

        // $count '*' = distinct root identity; each row already unique.
        bucket.count_roots += 1;

        for (out, field) in &req.counts {
            if req.hidden_outputs.contains(out) {
                continue;
            }
            let target_key = out.clone();
            if field.as_str() == "*" {
                continue;
            }
            // P20 fix: a `$count` over a related-record alias (`$count: "$battle"`)
            // counts the distinct related records, not a root-record field. The
            // alias comes from the `where`/`related` traversal specs.
            if let Some(related) = count_related(
                lmpg,
                &snap,
                lmpg.tenant,
                ord,
                &alias_specs,
                field,
                alias_budget,
            )? {
                aggregate_budget::check(alias_budget, related.len().saturating_mul(64))?;
                for _other in related {
                    bump_count(bucket, &target_key);
                }
                continue;
            }
            if resolve_field(&obj, field).is_some() {
                bump_count(bucket, &target_key);
            }
        }
        for (out, field) in &req.sums {
            // alias metrics: `$sum: "$employee.salary"` aggregates over the
            // root's related records via the `where` traversal alias.
            if let Some(values) = alias_field_values(
                lmpg,
                &snap,
                lmpg.tenant,
                ord,
                &alias_specs,
                field,
                alias_budget,
            )? {
                let key = out.clone();
                let cur = bucket
                    .sums
                    .get(&key)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let sum = values
                    .iter()
                    .try_fold(cur, |sum, value| -> Result<f64, QueryError> {
                        if !value.is_number() {
                            return Ok(sum);
                        }
                        let value = exact_f64(value).ok_or_else(|| {
                            QueryError::InvalidPredicate(
                                "aggregate integer is not exactly representable as f64".into(),
                            )
                        })?;
                        Ok(sum + value)
                    })?;
                bucket.sums.insert(key, serde_json::json!(sum));
            } else if let Some(Json::Number(n)) = resolve_field_numeric(&obj, field) {
                let key = out.clone();
                let cur = bucket
                    .sums
                    .get(&key)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let next = cur
                    + exact_f64(&Json::Number(n.clone())).ok_or_else(|| {
                        QueryError::InvalidPredicate(
                            "aggregate integer is not exactly representable as f64".into(),
                        )
                    })?;
                bucket.sums.insert(key, serde_json::json!(next));
            }
        }
        for (out, expr) in &req.expr_sums {
            if let Some(v) = eval_row_expr(expr, &obj) {
                let key = out.clone();
                let cur = bucket
                    .sums
                    .get(&key)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                bucket.sums.insert(key, serde_json::json!(cur + v));
            }
        }
        for (out, expr) in &req.expr_avgs {
            if let Some(v) = eval_row_expr(expr, &obj) {
                let key = out.clone();
                let cur = bucket
                    .sums
                    .get(&key)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                bucket.sums.insert(key, serde_json::json!(cur + v));
                bump_avg_count(bucket, format!("__avg_n_{out}"), 1);
            }
        }
        for (out, field) in &req.avg_terms {
            if let Some(values) = alias_field_values(
                lmpg,
                &snap,
                lmpg.tenant,
                ord,
                &alias_specs,
                field,
                alias_budget,
            )? {
                let key = out.clone();
                let sum = values
                    .iter()
                    .try_fold(0.0, |sum, value| -> Result<f64, QueryError> {
                        if !value.is_number() {
                            return Ok(sum);
                        }
                        let value = exact_f64(value).ok_or_else(|| {
                            QueryError::InvalidPredicate(
                                "aggregate integer is not exactly representable as f64".into(),
                            )
                        })?;
                        Ok(sum + value)
                    })?;
                let cur = bucket
                    .sums
                    .get(&key)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                bucket.sums.insert(key, serde_json::json!(cur + sum));
                let added = values.iter().filter(|v| v.is_number()).count() as u64;
                bump_avg_count(bucket, format!("__avg_n_{out}"), added);
            } else if let Some(Json::Number(n)) = resolve_field_numeric(&obj, field) {
                let key = out.clone();
                let cur = bucket
                    .sums
                    .get(&key)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let value = exact_f64(&Json::Number(n.clone())).ok_or_else(|| {
                    QueryError::InvalidPredicate(
                        "aggregate integer is not exactly representable as f64".into(),
                    )
                })?;
                bucket.sums.insert(key, serde_json::json!(cur + value));
                bump_avg_count(bucket, format!("__avg_n_{out}"), 1);
            }
        }
        for (out, field) in &req.mins {
            if let Some(values) = alias_field_values(
                lmpg,
                &snap,
                lmpg.tenant,
                ord,
                &alias_specs,
                field,
                alias_budget,
            )? {
                for v in values {
                    minmax(bucket, out.clone(), &v, true);
                }
            } else if let Some(v) = resolve_field(&obj, field) {
                minmax(bucket, out.clone(), &v, true);
            }
        }
        for (out, field) in &req.maxs {
            if let Some(values) = alias_field_values(
                lmpg,
                &snap,
                lmpg.tenant,
                ord,
                &alias_specs,
                field,
                alias_budget,
            )? {
                for v in values {
                    minmax(bucket, out.clone(), &v, false);
                }
            } else if let Some(v) = resolve_field(&obj, field) {
                minmax(bucket, out.clone(), &v, false);
            }
        }
        retained_bytes = retained_bytes
            .saturating_sub(previous_bucket_bytes)
            .saturating_add(aggregate_budget::bucket_bytes(bucket));
        aggregate_budget::check(budget, retained_bytes)?;
    }

    // Derived outputs ($ref chains): source sums materialize first, then
    // refs resolve in declaration order; cycles surface as unresolved refs.
    if !req.outputs.is_empty() || !req.avg_terms.is_empty() || !req.expr_avgs.is_empty() {
        let output_bytes = req
            .outputs
            .iter()
            .fold(0usize, |sum, (name, _)| {
                sum.saturating_add(name.len()).saturating_add(256)
            })
            .saturating_mul(buckets.len());
        aggregate_budget::check(
            budget,
            retained_bytes
                .saturating_mul(3)
                .saturating_add(output_bytes),
        )?;
        for b in &mut buckets {
            if is_cancelled() {
                return Err(QueryError::Cancelled);
            }
            let mut resolved: std::collections::BTreeMap<String, f64> = b
                .sums
                .clone()
                .into_iter()
                .filter_map(|(k, v)| v.as_f64().map(|x| (k, x)))
                .collect();
            resolved.insert("count_roots".into(), b.count_roots as f64);
            // Count outputs are also valid `$ref` targets in derived math
            // (`$count: "*"` → the root count; other counts → occurrence
            // count, which equals the distinct related-record count because
            // the traversal already deduplicates).
            for (out, field) in &req.counts {
                if req.hidden_outputs.contains(out) {
                    continue;
                }
                let value = if field.as_str() == "*" {
                    b.count_roots as f64
                } else {
                    b.counts
                        .get(out)
                        .and_then(serde_json::Value::as_u64)
                        .map_or(0.0, |n| n as f64)
                };
                resolved.insert(out.clone(), value);
            }
            // avg aliases: value = sum / non-null count, precision applied
            for name in req
                .avg_terms
                .iter()
                .map(|(n, _)| n)
                .chain(req.expr_avgs.iter().map(|(n, _)| n))
            {
                let sum = resolved.get(name).copied().unwrap_or(0.0);
                let n = b
                    .counts
                    .get(&format!("__avg_n_{name}"))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let avg = if n > 0 { sum / n as f64 } else { 0.0 };
                let avg = match req.avg_precision.get(name) {
                    // reference `toInteger(avg(...))`: truncate toward zero
                    Some(&0) => avg.trunc(),
                    Some(&p) => {
                        // Defensive: the parser clamps to `MAX_AVG_PRECISION`,
                        // but a directly built request must not overflow.
                        let p = p.min(crate::MAX_AVG_PRECISION);
                        let m = 10f64.powi(i32::try_from(p).unwrap_or(0));
                        (avg * m).round() / m
                    }
                    None => avg,
                };
                resolved.insert(name.clone(), avg);
                b.sums.insert(name.clone(), serde_json::json!(avg));
            }
            for (name, expr) in &req.outputs {
                eval_output(&normalize(name), expr, &mut resolved)?;
            }
            for (k, v) in &resolved {
                b.derived_outputs.insert(k.clone(), serde_json::json!(v));
                // mirror derived outputs into sums so orderBy sees them
                b.sums
                    .entry(k.clone())
                    .or_insert_with(|| serde_json::json!(v));
            }
        }
    }

    // AggregateThenPaginate: complete buckets first, then deterministic
    // ordering by output name and a single explicit slice domain.
    for (name, desc) in req.order_by.iter().rev() {
        buckets.sort_by(|a, b| {
            let va = bucket_metric(a, name, req);
            let vb = bucket_metric(b, name, req);
            let ord = compare_json_metric(va.as_ref(), vb.as_ref());
            if *desc {
                ord.reverse()
            } else {
                ord
            }
        });
    }
    apply_aggregate_pagination(&mut buckets, req);
    Ok((buckets, total_matched))
}

/// True when an aggregate is exactly a global `$count: "*"` (no grouping, no
/// other metrics, no per-root projection/collect). Such a request needs only
/// the match count, never per-record reconstruction.
fn is_count_only(req: &AggregateRequest) -> bool {
    req.group_by.is_empty()
        && req.plain_fields.is_empty()
        && req.collects.is_empty()
        && req.sums.is_empty()
        && req.mins.is_empty()
        && req.maxs.is_empty()
        && req.outputs.is_empty()
        && req.expr_sums.is_empty()
        && req.expr_avgs.is_empty()
        && req.avg_terms.is_empty()
        && req.time_bucket.is_none()
        && !req.counts.is_empty()
        && req.counts.iter().all(|(_, field)| field == "*")
}

/// One row's worth of `count_roots` (zero matches yields no bucket, matching
/// the per-candidate loop's behavior).
fn count_only_bucket(total: u64) -> Vec<GroupBucket> {
    if total == 0 {
        Vec::new()
    } else {
        vec![GroupBucket {
            count_roots: total,
            ..GroupBucket::default()
        }]
    }
}

fn apply_aggregate_pagination(buckets: &mut Vec<GroupBucket>, req: &AggregateRequest) {
    if req.skip > 0 || req.limit.is_some() {
        let start = usize::try_from(req.skip)
            .unwrap_or(usize::MAX)
            .min(buckets.len());
        *buckets = buckets[start..]
            .iter()
            .take(usize::try_from(req.limit.unwrap_or(u64::MAX)).unwrap_or(usize::MAX))
            .cloned()
            .collect();
    }
}

/// O(labels) count for a single unscoped label from the maintained catalog.
/// Returns `None` when the shape needs the ordinal matcher (predicates,
/// traversal, multiple labels, or a label absent from the catalog).
fn catalog_count_total(
    lmpg: &Lmpg,
    req: &SearchRequest,
    snap: &QuerySnapshot,
) -> Result<Option<u64>, QueryError> {
    if req.where_.is_some() || !req.related.is_empty() || !req.order_by.is_empty() {
        return Ok(None);
    }
    if req.labels.len() != 1 {
        return Ok(None);
    }
    // The on-disk label counter is exact and snapshot-consistent (DC1/DC12).
    Ok(Some(rushdb_lmpg::derived::label_count(
        snap,
        lmpg.tenant,
        &req.labels[0],
    )?))
}

/// Shape aggregate buckets into flat result rows: group-by dimensions (incl.
/// timeBucket aliases), count/sum outputs, and derived outputs keyed by their
/// output name.
///
/// # Errors
///
/// Propagates storage/engine read errors when opening the snapshot or
/// attaching per-root `$collect` shapes.
pub fn aggregate_to_rows(
    lmpg: &Lmpg,
    buckets: &[GroupBucket],
    req: &AggregateRequest,
) -> Result<Vec<Json>, QueryError> {
    let snap: QuerySnapshot = Arc::new(lmpg.store.read_view()?);
    aggregate_to_rows_in_view(lmpg, &snap, buckets, req)
}

/// Shape aggregate buckets using a caller-owned snapshot.
///
/// Keeping the view supplied by the caller prevents `$collect` reconstruction
/// from observing a later commit than the aggregate scan itself.
///
/// # Errors
///
/// Propagates storage/engine read errors while attaching per-root collects.
pub fn aggregate_to_rows_in_view(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    buckets: &[GroupBucket],
    req: &AggregateRequest,
) -> Result<Vec<Json>, QueryError> {
    aggregate_to_rows_in_view_with_budget(lmpg, snap, buckets, req, None)
}

/// Shape aggregate output with bounded working and retained rows.
///
/// # Errors
/// Returns storage or resource errors while reconstructing collects.
pub fn aggregate_to_rows_in_view_with_budget(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    buckets: &[GroupBucket],
    req: &AggregateRequest,
    budget: Option<usize>,
) -> Result<Vec<Json>, QueryError> {
    let configured = lmpg
        .limits()
        .query_memory_max_bytes()
        .map(|bytes| usize::try_from(bytes).unwrap_or(usize::MAX));
    let budget = match (budget, configured) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let snap = budget.map_or_else(
        || Arc::clone(snap),
        |bytes| snap.with_materialization_limit(bytes / 16),
    );
    let mut retained = buckets.iter().fold(0usize, |sum, bucket| {
        sum.saturating_add(aggregate_budget::bucket_bytes(bucket))
    });
    aggregate_budget::check(budget, retained)?;
    let mut rows = Vec::new();
    for b in buckets {
        aggregate_budget::check(
            budget,
            retained.saturating_add(aggregate_budget::bucket_bytes(b)),
        )?;
        let root_ordinal = b
            .keys
            .get("__rushdb_projection_root")
            .and_then(serde_json::Value::as_u64);
        let mut row = Map::new();
        for (k, v) in &b.keys {
            if k != "__rushdb_projection_root" {
                row.insert(k.clone(), v.clone());
            }
        }
        for (out, field) in &req.counts {
            if field == "*" {
                row.insert(out.clone(), Json::Number((b.count_roots).into()));
            } else {
                let n = map_metric(&b.counts, out).and_then(serde_json::Value::as_u64);
                row.insert(out.clone(), Json::Number(n.unwrap_or(0).into()));
            }
        }
        for (out, _field) in &req.sums {
            if req.hidden_outputs.contains(out) {
                continue;
            }
            if let Some(v) = b.sums.get(out) {
                row.insert(out.clone(), v.clone());
            }
        }
        for out in req
            .avg_terms
            .iter()
            .map(|(out, _)| out)
            .chain(req.expr_avgs.iter().map(|(out, _)| out))
        {
            if !req.hidden_outputs.contains(out) {
                if let Some(v) = b.sums.get(out) {
                    row.insert(out.clone(), v.clone());
                }
            }
        }
        for (out, _) in &req.mins {
            if !req.hidden_outputs.contains(out) {
                if let Some(v) = b.mins.get(out) {
                    row.insert(out.clone(), v.clone());
                }
            }
        }
        for (out, _) in &req.maxs {
            if !req.hidden_outputs.contains(out) {
                if let Some(v) = b.maxs.get(out) {
                    row.insert(out.clone(), v.clone());
                }
            }
        }
        for (out, _) in &req.outputs {
            if let Some(v) = b.derived_outputs.get(&normalize(out)) {
                row.insert(out.clone(), v.clone());
            }
        }
        // Attach per-root `$collect` shapes (docs' "aggregation with select"):
        // the row keeps its aggregate metrics and gains the collected arrays.
        let mut row_json = Json::Object(row);
        if !req.collects.is_empty() {
            if let Some(ord) = root_ordinal {
                row_json = attach_collects_with_budget(
                    lmpg,
                    &snap,
                    ord,
                    row_json,
                    &req.collects,
                    budget.map(|limit| limit.saturating_sub(retained)),
                    &|| false,
                )
                .map_err(aggregate_budget::resource_error)?;
            }
        }
        retained = retained.saturating_add(aggregate_budget::value_bytes(&row_json));
        aggregate_budget::check(budget, retained)?;
        rows.push(row_json);
    }
    Ok(rows)
}

fn ensure_bucket(buckets: &mut Vec<GroupBucket>, keys: Map<String, Json>) -> &mut GroupBucket {
    if let Some(pos) = buckets.iter().position(|b| b.keys == keys) {
        return &mut buckets[pos];
    }
    new_bucket(buckets, keys)
}

fn new_bucket(buckets: &mut Vec<GroupBucket>, keys: Map<String, Json>) -> &mut GroupBucket {
    buckets.push(GroupBucket {
        keys,
        count_roots: 0,
        sums: Map::new(),
        counts: Map::new(),
        mins: Map::new(),
        maxs: Map::new(),
        derived_outputs: std::collections::BTreeMap::new(),
    });
    let last = buckets.len() - 1;
    &mut buckets[last]
}

/// Increment an `$count` occurrence counter. Stored as a numeric counter rather
/// than one array entry per counted value, so a high-cardinality count does not
/// retain O(values) memory. A `$count` over a traversal alias counts distinct
/// related records because the traversal already deduplicates its result.
fn bump_count(bucket: &mut GroupBucket, key: &str) {
    let entry = bucket
        .counts
        .entry(key.to_string())
        .or_insert_with(|| Json::Number(0.into()));
    let next = entry.as_u64().unwrap_or(0).saturating_add(1);
    *entry = Json::Number(next.into());
}

/// Increment an `$avg` non-null count. Stored as a numeric counter rather than
/// one array entry per value, so a high-cardinality average does not retain
/// O(values) memory.
fn bump_avg_count(bucket: &mut GroupBucket, key: String, delta: u64) {
    let entry = bucket
        .counts
        .entry(key)
        .or_insert_with(|| Json::Number(0.into()));
    if let Json::Number(n) = entry {
        *n = (n.as_u64().unwrap_or(0) + delta).into();
    }
}

pub(super) fn minmax(bucket: &mut GroupBucket, key: String, v: &Json, is_min: bool) {
    let entry = if is_min {
        &mut bucket.mins
    } else {
        &mut bucket.maxs
    };
    let entry = entry
        .entry(key)
        .or_insert_with(|| aggregate_numeric_value(v));
    let Some((_, next)) = infer_scalar(v) else {
        return;
    };
    let Some((_, current)) = infer_scalar(entry) else {
        return;
    };
    if !matches!(next, Scalar::Int64(_) | Scalar::Float64(_))
        || !matches!(current, Scalar::Int64(_) | Scalar::Float64(_))
    {
        return;
    }
    let Some(ordering) = canonical_scalar_cmp(&next, &current) else {
        return;
    };
    let replace = if is_min {
        ordering == std::cmp::Ordering::Less
    } else {
        ordering == std::cmp::Ordering::Greater
    };
    if replace {
        *entry = aggregate_numeric_value(v);
    }
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the paired integer round-trip check preserves exact large integers"
)]
fn aggregate_numeric_value(value: &Json) -> Json {
    let Some((_, scalar)) = infer_scalar(value) else {
        return value.clone();
    };
    match scalar {
        Scalar::Int64(integer) => {
            let float = integer as f64;
            if (float as i128) == i128::from(integer) {
                serde_json::Number::from_f64(float).map_or_else(|| value.clone(), Json::Number)
            } else {
                value.clone()
            }
        }
        _ => value.clone(),
    }
}

fn map_metric<'a>(map: &'a Map<String, Json>, name: &str) -> Option<&'a Json> {
    let normalized = normalize(name);
    map.iter()
        .find(|(key, _)| normalize(key) == normalized)
        .map(|(_, value)| value)
}

fn bucket_metric(b: &GroupBucket, name: &str, req: &AggregateRequest) -> Option<Json> {
    if normalize(name) == "count_roots" {
        return Some(Json::Number(b.count_roots.into()));
    }
    if let Some((output, field)) = req
        .counts
        .iter()
        .find(|(output, _)| normalize(output) == normalize(name))
    {
        if field == "*" {
            return Some(Json::Number(b.count_roots.into()));
        }
        return map_metric(&b.counts, output)
            .map(|values| Json::Number(values.as_u64().unwrap_or(0).into()));
    }
    map_metric(&b.sums, name)
        .or_else(|| map_metric(&b.mins, name))
        .or_else(|| map_metric(&b.maxs, name))
        .or_else(|| map_metric(&b.keys, name))
        .cloned()
}

fn compare_json_metric(a: Option<&Json>, b: Option<&Json>) -> std::cmp::Ordering {
    match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, _) => std::cmp::Ordering::Less,
        (_, None) => std::cmp::Ordering::Greater,
        (Some(x), Some(y)) => json_value_cmp(x, y),
    }
}

/// Evaluate an `OutputExpr` against a single reconstructed record object.
pub(super) fn eval_row_expr(expr: &OutputExpr, obj: &Map<String, Json>) -> Option<f64> {
    match expr {
        OutputExpr::Literal(l) => Some(*l),
        OutputExpr::Field(field) => resolve_field(obj, field).and_then(|value| exact_f64(&value)),
        OutputExpr::Ref(name) => obj.get(&normalize(name)).and_then(exact_f64),
        OutputExpr::Add(a, b) => Some(eval_row_expr(a, obj)? + eval_row_expr(b, obj)?),
        OutputExpr::Subtract(a, b) => Some(eval_row_expr(a, obj)? - eval_row_expr(b, obj)?),
        OutputExpr::Multiply(a, b) => Some(eval_row_expr(a, obj)? * eval_row_expr(b, obj)?),
        OutputExpr::Divide {
            dividend, divisor, ..
        } => {
            let d = eval_row_expr(dividend, obj)?;
            let q = eval_row_expr(divisor, obj)?;
            if q == 0.0 {
                None
            } else {
                Some(d / q)
            }
        }
    }
}

/// Convert a JSON number to the aggregate expression representation without
/// silently rounding an integer that cannot be represented by `f64`.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the integer round-trip check rejects lossy widening"
)]
pub(super) fn exact_f64(value: &Json) -> Option<f64> {
    let number = value.as_number()?;
    if let Some(integer) = number.as_i64() {
        let float = integer as f64;
        return ((float as i128) == i128::from(integer)).then_some(float);
    }
    if let Some(integer) = number.as_u64() {
        let float = integer as f64;
        return ((float as i128) == i128::from(integer)).then_some(float);
    }
    number.as_f64()
}

fn eval_output(
    name: &str,
    expr: &OutputExpr,
    results: &mut std::collections::BTreeMap<String, f64>,
) -> Result<f64, QueryError> {
    let v = match expr {
        OutputExpr::Literal(l) => *l,
        OutputExpr::Field(_) => {
            // Field refs are evaluated per-row in the aggregate loop; at the
            // bucket stage they carry no value.
            return Ok(0.0);
        }
        OutputExpr::Ref(other) => results
            .get(other)
            .copied()
            .ok_or_else(|| QueryError::InvalidPredicate(format!("unresolved ref `{other}`")))?,
        OutputExpr::Add(a, b) => eval_output(name, a, results)? + eval_output(name, b, results)?,
        OutputExpr::Subtract(a, b) => {
            eval_output(name, a, results)? - eval_output(name, b, results)?
        }
        OutputExpr::Multiply(a, b) => {
            eval_output(name, a, results)? * eval_output(name, b, results)?
        }
        OutputExpr::Divide {
            dividend, divisor, ..
        } => {
            let d = eval_output(name, dividend, results)?;
            let q = eval_output(name, divisor, results)?;
            if q == 0.0 {
                return Err(QueryError::InvalidPredicate("divide by zero".into()));
            }
            d / q
        }
    };
    results.insert(name.to_string(), v);
    Ok(v)
}
