//! Budgeted grouping, aggregate evaluation, and result projection.

use super::{
    aggregate_budget, attach_collects_with_budget, json_value_cmp, normalize, reconstruct,
    related_specs_from_where, working_set_budget, AggregateRequest, Arc, Budget, Cap, GroupBucket,
    Json, Lmpg, Map, OutputExpr, PointFields, QueryError, QuerySnapshot, SearchRequest,
};
use crate::aggregate_catalog::dictionary_grouped_count;
use crate::aggregate_rows::{row_bytes, Partial, RowCtx, RowPlan};
use crate::aggregate_scan::{self, Mode, Rows, Scan};
use crate::page::serialized_bytes;

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

/// Execute an aggregate on the supplied view.
///
/// The working set (candidate ordinals, the row being aggregated, the
/// buckets) is capped by `query-memory-max`, tightened by the request, the
/// same cap a records search uses. It is never the request's response byte
/// budget: that caps only the rendered page
/// ([`aggregate_to_rows_in_view_with_budget`]). Charging the working set
/// against the 16 MiB response budget refused every grouped aggregate over
/// ~1M records (the candidate ordinals alone filled it).
///
/// # Errors
/// Returns storage, predicate, cancellation, or resource errors.
pub fn aggregate_with_cancel_in_view(
    lmpg: &Lmpg,
    req: &AggregateRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
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
    let budget = working_set_budget(lmpg);
    Budget::WorkingSet.check(budget, std::mem::size_of::<GroupBucket>())?;
    aggregate_budgeted(lmpg, req, is_cancelled, snap, budget)
        .map_err(aggregate_budget::resource_error)
}

#[expect(
    clippy::too_many_lines,
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
        ..SearchRequest::default()
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
    // Grouped counts over one property: the value dictionary holds the
    // exact owner count per value, so no record is read.
    if let Some(buckets) = dictionary_grouped_count(lmpg, req, &snap, is_cancelled)? {
        Budget::WorkingSet.check(
            budget,
            buckets.iter().fold(0usize, |sum, b| {
                sum.saturating_add(aggregate_budget::bucket_bytes(b))
            }),
        )?;
        let total = buckets.iter().map(|b| b.count_roots).sum();
        return Ok((order_and_page(buckets, req), total));
    }
    // Only the properties the request names are read: by point reads when
    // every one is a scalar (READ-1's `PointFields`), else by folding the
    // record's occurrences into just those names. Family definitions are
    // frozen within the pinned snapshot.
    let plan = RowPlan::new(req, related_specs_from_where(req.where_.as_ref()));
    let points = PointFields::resolve_in(
        &snap,
        lmpg.tenant,
        &req.labels,
        plan.names.iter().map(String::as_str),
    )?;
    let selected = Rows::selection(&plan);
    let rows = Rows {
        points: points.as_ref(),
        selected: &selected,
    };
    let scan = Scan {
        lmpg,
        search: &page_req,
        snap: &snap,
        budget,
        plan: &plan,
    };
    let mode = if is_count_only(req) {
        Mode::Count
    } else if plan.per_row() {
        Mode::Roots
    } else {
        Mode::Groups
    };
    let pass = aggregate_scan::run(&scan, &rows, mode, is_cancelled)?;
    let total_matched = pass.total;
    let candidate_bytes = pass.candidate_bytes;
    let mut buckets = match mode {
        Mode::Count => {
            let mut buckets = count_only_bucket(total_matched);
            apply_aggregate_pagination(&mut buckets, req);
            return Ok((buckets, total_matched));
        }
        Mode::Roots => per_root_rows(&scan, &rows, &pass, req, is_cancelled)?,
        Mode::Groups => pass.groups.finish(&plan),
    };
    let retained_bytes = buckets.iter().fold(candidate_bytes, |sum, b| {
        sum.saturating_add(aggregate_budget::bucket_bytes(b))
    });
    Budget::WorkingSet.check(budget, retained_bytes)?;

    // Derived outputs ($ref chains): source sums materialize first, then
    // refs resolve in declaration order; cycles surface as unresolved refs.
    // The candidate ordinals are no longer needed; what stays is the buckets
    // plus one derived value per output and bucket.
    if !req.outputs.is_empty() || !req.avg_terms.is_empty() || !req.expr_avgs.is_empty() {
        let output_bytes = req
            .outputs
            .iter()
            .map(|(name, _)| name)
            .chain(req.avg_terms.iter().map(|(name, _)| name))
            .chain(req.expr_avgs.iter().map(|(name, _)| name))
            .fold(0usize, |sum, name| {
                sum.saturating_add(name.len()).saturating_add(256)
            })
            .saturating_mul(buckets.len());
        Budget::WorkingSet.check(
            budget,
            retained_bytes
                .saturating_sub(candidate_bytes)
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

    Ok((order_and_page(buckets, req), total_matched))
}

/// Per-root rows (a `select` of root fields or collects without
/// `groupBy`): each matched root is its own row, so rows are bounded by the
/// paging and the memory budget, not by the group cap; without an `orderBy`
/// only the requested page is read.
fn per_root_rows(
    scan: &Scan<'_>,
    rows: &Rows<'_>,
    pass: &aggregate_scan::Pass,
    req: &AggregateRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<GroupBucket>, QueryError> {
    let roots = if req.order_by.is_empty() {
        let page_end = req.skip.saturating_add(req.limit.unwrap_or(u64::MAX));
        &pass.roots[..usize::try_from(page_end)
            .unwrap_or(usize::MAX)
            .min(pass.roots.len())]
    } else {
        &pass.roots[..]
    };
    let (snap, tenant, budget) = (scan.snap, scan.lmpg.tenant, scan.budget);
    let mut session = reconstruct::ReconstructSession::new(snap, tenant);
    let mut partial = Partial::default();
    let mut row = Vec::new();
    let mut retained = pass
        .candidate_bytes
        .saturating_add(pass.roots.len().saturating_mul(8));
    for &ord in roots {
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        if !rows.read(scan.plan, snap, tenant, &mut session, ord, &mut row)? {
            continue;
        }
        let held = retained.saturating_add(row_bytes(&row));
        Budget::WorkingSet.check(budget, held)?;
        let ctx = RowCtx {
            lmpg: scan.lmpg,
            snap,
            ord,
            alias_budget: budget.map(|limit| {
                limit.saturating_sub(held) / scan.plan.metric_count().saturating_add(1)
            }),
        };
        retained = retained.saturating_add(partial.accumulate(scan.plan, &row, &ctx)?);
        Budget::WorkingSet.check(budget, retained)?;
    }
    Ok(partial.finish(scan.plan))
}

/// Aggregate, then paginate: complete buckets first, then deterministic
/// ordering by output name and a single explicit slice domain.
fn order_and_page(mut buckets: Vec<GroupBucket>, req: &AggregateRequest) -> Vec<GroupBucket> {
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
    if buckets.is_empty() && is_global_rollup(req) {
        // As in SQL, a rollup over no matches is one row: counts are 0.
        buckets.push(GroupBucket::default());
    }
    apply_aggregate_pagination(&mut buckets, req);
    buckets
}

/// A metric with no value over a global rollup's matches is `null`, as in
/// SQL.
fn fill_absent_metrics(row: &mut Map<String, Json>, req: &AggregateRequest) {
    if !is_global_rollup(req) {
        return;
    }
    let metrics = req
        .sums
        .iter()
        .chain(&req.avg_terms)
        .map(|(out, _)| out)
        .chain(req.expr_sums.iter().map(|(out, _)| out))
        .chain(req.expr_avgs.iter().map(|(out, _)| out))
        .chain(req.mins.iter().map(|(out, _)| out))
        .chain(req.maxs.iter().map(|(out, _)| out))
        .chain(req.outputs.iter().map(|(out, _)| out));
    for out in metrics {
        if !req.hidden_outputs.contains(out) {
            row.entry(out.clone()).or_insert(Json::Null);
        }
    }
}

/// True when metrics roll up into one global row: some metric, and no
/// grouping, time buckets, per-root fields or collects. Such a rollup always
/// has its row.
fn is_global_rollup(req: &AggregateRequest) -> bool {
    let metrics = !req.counts.is_empty()
        || !req.sums.is_empty()
        || !req.mins.is_empty()
        || !req.maxs.is_empty()
        || !req.outputs.is_empty()
        || !req.expr_sums.is_empty()
        || !req.expr_avgs.is_empty()
        || !req.avg_terms.is_empty();
    metrics
        && req.group_by.is_empty()
        && req.time_bucket.is_none()
        && req.plain_fields.is_empty()
        && req.collects.is_empty()
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

/// One row's worth of `count_roots` (zero matches is a row of 0, as the
/// general rollup gives).
fn count_only_bucket(total: u64) -> Vec<GroupBucket> {
    vec![GroupBucket {
        count_roots: total,
        ..GroupBucket::default()
    }]
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

/// Shape aggregate output against the response budget.
///
/// The buckets were already admitted under `query-memory-max` while they
/// accumulated. What rendering adds is the response: each row is charged by
/// its real serialized bytes, `$collect`s included, as it is rendered,
/// against `budget` (the response size limit), or `query-memory-max` when
/// the caller gave none.
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
    let cap = Cap::for_response(budget, lmpg.limits().query_memory_max_bytes());
    let snap = cap.map_or_else(
        || Arc::clone(snap),
        |cap| snap.with_materialization_limit(cap.limit() / 16),
    );
    let mut charged = 0usize;
    let mut rows = Vec::with_capacity(buckets.len());
    for b in buckets {
        let mut row = b.keys.clone();
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
            .expr_sums
            .iter()
            .map(|(out, _)| out)
            .chain(req.avg_terms.iter().map(|(out, _)| out))
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
        fill_absent_metrics(&mut row, req);
        // Attach per-root `$collect` shapes (docs' "aggregation with select"):
        // the row keeps its aggregate metrics and gains the collected arrays.
        let mut row_json = Json::Object(row);
        if let Some(ord) = b.root.filter(|_| !req.collects.is_empty()) {
            row_json = attach_collects_with_budget(
                lmpg,
                &snap,
                ord,
                row_json,
                &req.collects,
                cap.map(|cap| cap.after(charged)),
                &|| false,
            )
            .map_err(aggregate_budget::resource_error)?;
        }
        if cap.is_some() {
            charged = charged.saturating_add(serialized_bytes(&row_json));
            Cap::check_opt(cap, charged)?;
        }
        rows.push(row_json);
    }
    Ok(rows)
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

/// Convert a JSON number to the aggregate expression representation without
/// silently rounding an integer that cannot be represented by `f64`.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the integer round-trip check rejects lossy widening"
)]
pub(crate) fn exact_f64(value: &Json) -> Option<f64> {
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
