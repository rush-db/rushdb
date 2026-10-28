//! Query planner and executor over the LMPG state.
//!
//! Turns a [`SearchRequest`] into an access path (`index_candidates`), executes
//! it against a pinned snapshot, and falls back to a full scan whenever a
//! predicate cannot be served exactly by the maintained indexes. Every index
//! path is required to return exactly what the forced-scan path returns; the
//! differential tests in `tests/planner_differential.rs` enforce that.
use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::reconstruct;
use std::sync::Arc;
/// Caller-owned snapshot used for query execution and result shaping.
pub type QuerySnapshot = Arc<rushdb_storage::ReadView>;
use equality::{any_element, is_datetime, value_equals};
use rushdb_lmpg::{Lmpg, LmpgError};
use rushdb_types::model::{LogicalType, TypeState};
use rushdb_types::value::{
    infer_scalar, scalar_cmp as canonical_scalar_cmp, scalar_equal, Scalar, ID_FIELD,
};
use serde_json::{Map, Value as Json};
use thiserror::Error;

mod aggregate_budget;
mod aggregation;
mod cycle;
mod equality;
pub use aggregation::MAX_AGGREGATE_BUCKETS;
pub use aggregation::{
    aggregate, aggregate_in_view, aggregate_to_rows, aggregate_to_rows_in_view,
    aggregate_to_rows_in_view_with_budget, aggregate_with_cancel, aggregate_with_cancel_in_view,
    aggregate_with_cancel_in_view_with_budget,
};
#[cfg(test)]
use aggregation::{eval_row_expr, exact_f64, minmax};
mod page;
pub use page::effective_budget;
mod planned;
mod planner;
mod point_fields;
pub use point_fields::PointFields;
mod property_predicate;
pub use hex::hex_decode_16;
pub use index_access::explain_index;
pub use planned::{planned_candidates, PlannedCandidates};
use planner::index_candidates_with_budget;
pub use planner::{
    explain_cardinality, explain_range_selectivity, explain_walk, unindexed_walks_total,
    UNINDEXED_WALK_THRESHOLD,
};
pub use property_predicate::{matches_properties, parse_property_predicate};
pub mod datetime;
pub mod document;
mod hex;
mod index_access;
mod scan;
mod stats_cache;
pub use document::{ParsedQuery, QueryDocumentError};

/// A single-field predicate inside a `where` expression.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FieldPred {
    /// `$eq` equality; `$eq:null` is an explicit alias for the is-null state predicate.
    Eq(String, Json),
    /// `$ne` inequality; the field must be present for the predicate to match.
    Ne(String, Json),
    /// `$gt` numeric/datetime strictly-greater comparison.
    Gt(String, f64),
    /// `$gte` numeric/datetime greater-than-or-equal comparison.
    Gte(String, f64),
    /// `$lt` numeric/datetime strictly-less comparison.
    Lt(String, f64),
    /// `$lte` numeric/datetime less-than-or-equal comparison.
    Lte(String, f64),
    /// `$contains` case-insensitive substring match on a string.
    Contains(String, String),
    /// `$startsWith` case-insensitive prefix match on a string.
    StartsWith(String, String),
    /// `$exists` presence test; the flag selects present (`true`) or absent.
    Exists(String, bool),
    /// `$isNull`/`$eq:null` null-state test; the flag selects null (`true`) or non-null.
    IsNull(String, bool),
    /// `$in` membership against a set of literal values.
    In(String, Vec<Json>),
    /// `$nin` non-membership; the field must be present and match none of the values.
    Nin(String, Vec<Json>),
    /// `$endsWith` case-insensitive suffix match on a string.
    EndsWith(String, String),
    /// `$id` — match the record identity itself.
    Id(String),
    /// Datetime component predicate: `{ created: { $year: 2023 } }`.
    /// `which` = 0..6 (year/month/day/hour/minute/second/millisecond).
    DatetimeComponent {
        /// Normalized property name the component is read from.
        field: String,
        /// Component selector: 0=year, 1=month, 2=day, 3=hour, 4=minute, 5=second, 6=millisecond.
        which: u8,
        /// Expected component value.
        value: i64,
    },
    /// `$type` operator: "string" | "number" | "boolean" | "array" | "object"
    Type(String, String),
}

/// Boolean expression tree evaluated against a reconstructed record.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    /// Logical AND of every child.
    And(Vec<Expr>),
    /// Logical OR of every child.
    Or(Vec<Expr>),
    /// Logical XOR (`$xor`)
    Xor(Box<Expr>, Box<Expr>),
    /// Logical NOR (`$nor`)
    Nor(Vec<Expr>),
    /// Logical negation of the child.
    Not(Box<Expr>),
    /// A single field predicate.
    Field(FieldPred),
    /// Related-record traversal predicate (label-key blocks, `$relation`,
    /// `hops`, `$cycle`). Evaluated existentially over reachable targets.
    Related(RelatedExpr),
}

/// Traversal predicate mirroring the documented relationship syntax.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RelatedExpr {
    /// Labels an endpoint record must carry (empty = any label).
    pub target_labels: Vec<String>,
    /// Optional `$alias` binding for the endpoint, used by `$count`/`$sum`/`$collect`.
    pub alias: Option<String>,
    /// Relationship type filter; `None` matches every relation type.
    pub rel_type: Option<String>,
    /// Edge direction to traverse.
    pub direction: Direction,
    /// Variable-length traversal bounds `(min, max)`; absent = one hop.
    pub hops: Option<(u64, u64)>,
    /// `$cycle`: true when the traversal must return to the start record.
    pub cycle: bool,
    /// Predicates evaluated on the endpoint record (may nest further
    /// `RelatedExpr` children for multi-level blocks).
    pub filter: Option<Box<Expr>>,
}

/// A root-bound traversal branch declared under `related` or a `where` alias.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RelatedSpec {
    /// Alias the branch is bound to (without leading `$`).
    pub alias: Option<String>,
    /// Relationship type filter for the final step; `None` matches any type.
    pub rel_type: Option<String>,
    /// Edge direction for the final step.
    pub direction: Direction,
    /// Labels the final-step endpoint must carry (empty = any).
    pub target_labels: Vec<String>,
    /// The full traversal path from the root record to this spec's level
    /// (ancestors first; the last step is the spec's own level). Each step
    /// carries that level's label/rel-type/direction and endpoint predicates,
    /// so alias `$count`/`$sum`/`$collect` match the reference: multi-hop
    /// aliases traverse every level and apply each level's `where` filter.
    pub steps: Vec<TraversalStep>,
}

/// One level of an alias traversal path (see [`RelatedSpec::steps`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TraversalStep {
    /// Labels the endpoint at this level must carry (empty = any label).
    pub target_labels: Vec<String>,
    /// Relationship type filter at this level; `None` matches any type.
    pub rel_type: Option<String>,
    /// Edge direction at this level.
    pub direction: Direction,
    /// Endpoint predicates evaluated on the record at this level (nested label
    /// blocks are separate steps, not part of this filter).
    pub filter: Option<Expr>,
}

/// `$collect` at the root binding grain (document 19): one declared step
/// per level, batched typed-adjacency collection, optional uniqueness and
/// local slice. Output attaches to each returned record under `alias`.
///
/// Two forms: **alias-based** (`from` in the docs) resolves `rel_type`/
/// `direction`/`target_labels` from the matching `where` traversal alias, and
/// projects whole records unless `fields` is set; **label-based** (`label` in
/// the docs) traverses inline (direction defaults to outgoing), supports a
/// `where` filter, an output-key→field projection map, and **nested** collects
/// evaluated recursively on each collected record (hierarchical output).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CollectSpec {
    /// Output key the collected array is attached under.
    pub alias: String,
    /// Relationship type filter for direct-adjacency collects.
    pub rel_type: Option<String>,
    /// Edge direction for direct-adjacency collects.
    pub direction: Direction,
    /// Labels a collected record must carry (empty = any label).
    pub target_labels: Vec<String>,
    /// Alias-based (`from`) collects carry the full traversal path to the
    /// alias, including each level's `where` predicates. Empty for
    /// label-based collects (direct adjacency).
    pub steps: Vec<TraversalStep>,
    /// `(output_key, source_field)` projection; empty = whole record.
    pub fields: Vec<(String, String)>,
    /// Nested (hierarchical) collects evaluated on each collected record.
    pub nested: Vec<CollectSpec>,
    /// `where` filter evaluated on each collected record (label-based).
    pub filter: Option<Expr>,
    /// ORDER BY (field, desc) applied to the collected array.
    pub order_by: Vec<(String, bool)>,
    /// When true, deduplicate collected records by identity.
    pub unique: bool,
    /// Number of collected records to skip after ordering.
    pub skip: u64,
    /// Maximum number of collected records to return.
    pub limit: Option<u64>,
}

/// Direction of an edge traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    /// Follow outgoing edges (root to target).
    Outgoing,
    /// Follow incoming edges (target to root).
    Incoming,
    /// Follow edges in either direction.
    Both,
}

/// Calendar/clock unit for a fixed-width time bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TimeUnit {
    /// One-second buckets.
    Seconds,
    /// One-minute buckets.
    Minutes,
    /// One-hour buckets.
    Hours,
    /// One-day buckets.
    Days,
    /// One-week buckets.
    Weeks,
    /// One-month buckets (calendar-aligned).
    Months,
    /// Three-month buckets (calendar-aligned).
    Quarters,
    /// One-year buckets (calendar-aligned).
    Years,
}

impl TimeUnit {
    fn millis(self) -> i64 {
        match self {
            TimeUnit::Seconds => 1_000,
            TimeUnit::Minutes => 60_000,
            TimeUnit::Hours => 3_600_000,
            TimeUnit::Days => 86_400_000,
            TimeUnit::Weeks => 7 * 86_400_000,
            TimeUnit::Months => 30 * 86_400_000,
            TimeUnit::Quarters => 90 * 86_400_000,
            TimeUnit::Years => 365 * 86_400_000,
        }
    }

    /// Parse a time unit from its documented name (singular or plural).
    ///
    /// # Errors
    ///
    /// Returns [`crate::QueryDocumentError::Parse`] when `name` is not a
    /// recognized time unit.
    pub fn from_name(name: &str) -> Result<TimeUnit, crate::QueryDocumentError> {
        Ok(match name {
            "second" | "seconds" => TimeUnit::Seconds,
            "minute" | "minutes" => TimeUnit::Minutes,
            "hour" | "hours" => TimeUnit::Hours,
            "day" | "days" => TimeUnit::Days,
            "week" | "weeks" => TimeUnit::Weeks,
            "month" | "months" => TimeUnit::Months,
            "quarter" => TimeUnit::Quarters,
            "year" | "years" => TimeUnit::Years,
            other => {
                return Err(crate::QueryDocumentError::Parse(format!(
                    "unknown time unit `{other}`"
                )));
            }
        })
    }

    pub(crate) fn unit_str(self) -> &'static str {
        match self {
            TimeUnit::Seconds => "seconds",
            TimeUnit::Minutes => "minutes",
            TimeUnit::Hours => "hours",
            TimeUnit::Days => "days",
            TimeUnit::Weeks => "weeks",
            TimeUnit::Months => "months",
            TimeUnit::Quarters => "quarters",
            TimeUnit::Years => "years",
        }
    }
}

/// Fixed-size calendar bucket over an epoch-millisecond field. Non-datetime
/// or missing input collapses to the documented MISSING state key.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TimeBucketSpec {
    /// Property whose epoch-millisecond value is bucketed.
    pub field: String,
    /// Calendar/clock unit of the bucket.
    pub unit: TimeUnit,
    /// Number of `unit`s per bucket (always at least 1).
    pub size: u64,
}

/// Maximum accepted `$precision` for `$avg`. `f64` carries ~15 significant
/// decimal digits; larger values would only amplify rounding noise (and
/// `10f64.powi` overflows past ~308). Parsed values are clamped to this.
pub const MAX_AVG_PRECISION: u32 = 15;

impl TimeBucketSpec {
    /// A bucket of exactly one `unit`.
    pub fn fixed(field: impl Into<String>, unit: TimeUnit) -> Self {
        TimeBucketSpec {
            field: field.into(),
            unit,
            size: 1,
        }
    }

    /// A bucket spanning `size` `unit`s.
    ///
    /// # Panics
    ///
    /// Panics when `size` is zero.
    pub fn sized(field: impl Into<String>, unit: TimeUnit, size: u64) -> Self {
        assert!(size > 0, "timeBucket size must be positive");
        TimeBucketSpec {
            field: field.into(),
            unit,
            size,
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "epoch-ms is a bounded timestamp; the saturating float-to-int cast matches the reference calendar flooring"
    )]
    fn floor_millis(&self, epoch_ms: f64) -> i64 {
        let ms = epoch_ms as i64;
        // calendar-aware flooring for Months/Years (fixed-width would not
        // align to calendar boundaries)

        match self.unit {
            TimeUnit::Months => rushdb_types::datetime::start_of_month(ms),
            TimeUnit::Quarters => rushdb_types::datetime::start_of_quarter(ms),
            TimeUnit::Years => rushdb_types::datetime::start_of_year(ms),
            _ => {
                // Defensive even though the parser rejects `size == 0`: a
                // directly constructed spec must not divide by zero or wrap.
                let size = i64::try_from(self.size).unwrap_or(i64::MAX).max(1);
                let width = self.unit.millis().saturating_mul(size).max(1);
                ms.div_euclid(width).saturating_mul(width)
            }
        }
    }
}

/// Offset/limit pagination applied after matching and ordering.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Pagination {
    /// Number of matching roots to skip.
    pub skip: u64,
    /// Maximum number of matching roots to return; `None` = unbounded.
    pub limit: Option<u64>,
}

/// A records-perspective search over one committed snapshot.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchRequest {
    /// Labels a root record must carry; empty matches every label.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Boolean predicate the root record must satisfy.
    #[serde(default, rename = "where")]
    pub where_: Option<Expr>,
    /// Independent traversal branches; every spec filters existentially so
    /// independent one-to-many branches never multiply the root grain.
    #[serde(default)]
    pub related: Vec<RelatedSpec>,
    /// Per-root `$collect` output shapes attached to each returned record.
    #[serde(default)]
    pub collects: Vec<CollectSpec>,
    /// Offset/limit applied after matching and ordering.
    #[serde(default)]
    pub pagination: Pagination,
    /// ORDER BY (field, desc) applied after matching, before pagination.
    #[serde(default)]
    pub order_by: Vec<(String, bool)>,
    /// Test/oracle switch: when true, the index access path is bypassed and
    /// the executor performs a full scan, letting differential tests compare
    /// the two paths on identical input.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force_scan: bool,
}

/// Errors returned by query planning and execution.
#[derive(Debug, Error)]
pub enum QueryError {
    /// A storage read or snapshot error.
    #[error("storage error: {0}")]
    Storage(#[from] rushdb_storage::StorageError),
    /// An engine (LMPG) error.
    #[error("engine error: {0}")]
    Engine(#[from] LmpgError),
    /// A predicate could not be evaluated (e.g. divide by zero, unresolved ref).
    #[error("invalid predicate on `{0}`")]
    InvalidPredicate(String),
    /// The cooperative cancellation probe requested a stop.
    #[error("query cancelled")]
    Cancelled,
    /// A declared resource budget (e.g. distinct aggregate buckets) was exceeded.
    #[error("resource exhausted: {0}")]
    ResourceLimit(String),
    /// A filter without a usable index would check more records than
    /// `unindexed-walk-max-owners` allows.
    #[error("unindexed walk refused: {0}")]
    UnindexedWalk(String),
}

/// One page of reconstructed records plus the pre-pagination match total.
#[derive(Debug, Clone)]
pub struct Page {
    /// Number of distinct roots matched before pagination.
    pub total_matched: u64,
    /// Reconstructed records in this page, in query order.
    pub records: Vec<Json>,
}

/// Records-perspective execution over one committed snapshot. Missing
/// property = no occurrence; explicit null = live payload-free occurrence.
///
/// # Errors
///
/// Propagates storage/engine read errors and predicate evaluation failures as
/// [`QueryError`].
pub fn find_records(lmpg: &Lmpg, req: &SearchRequest) -> Result<Page, QueryError> {
    find_records_with_cancel(lmpg, req, &|| false)
}

/// Records-perspective execution with a cooperative cancellation probe.
/// The probe is checked inside scan and reconstruction loops so a cancelled
/// stream stops consuming CPU instead of merely discarding its response.
///
/// # Errors
///
/// Returns [`QueryError::Cancelled`] when the probe requests a stop, and
/// propagates storage/engine read errors and predicate evaluation failures.
pub fn find_records_with_cancel(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Page, QueryError> {
    let snap = Arc::new(lmpg.store.read_view()?);
    find_records_in_view(lmpg, req, is_cancelled, snap)
}

/// Every ordinal matching `req`, in match order (`order_by` applied).
///
/// Pagination is ignored: callers apply `skip`/`limit` themselves. The index
/// bounded-page path is not used, so a delete can snapshot the full match set
/// once instead of re-scanning after every page. Page JSON is not built. The
/// working set is capped by `query-memory-max` ([`working_set_budget`]).
///
/// # Errors
///
/// Returns [`QueryError::Cancelled`] when the probe requests a stop,
/// [`QueryError::ResourceLimit`] when the working set exceeds
/// `query-memory-max`, and propagates storage/engine read errors and
/// predicate evaluation failures.
#[expect(
    clippy::needless_pass_by_value,
    reason = "callers hand over a freshly pinned view, matching find_records_in_view"
)]
pub fn matching_ordinals(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
) -> Result<Vec<u64>, QueryError> {
    let is_cancelled: &dyn Fn() -> bool = &cancel_probe(lmpg, is_cancelled);
    if !snap.belongs_to(&lmpg.store) {
        return Err(rushdb_storage::StorageError::InvalidRead(
            "query view belongs to another Store".into(),
        )
        .into());
    }
    if is_cancelled() {
        return Err(QueryError::Cancelled);
    }
    let mut full = req.clone();
    full.pagination = Pagination::default();
    Ok(run_matches(lmpg, &full, is_cancelled, &snap, usize::MAX, None)?.retained)
}

/// The matches a search keeps, and how many there are in all.
struct Matches {
    /// The first `keep` matches in query order.
    retained: Vec<u64>,
    /// Every match, retained or not.
    total: u64,
}

/// One pass over the candidates in ascending ordinal order.
///
/// Keeps the first `keep` matches in query order and counts the rest
/// without keeping them. Without `order_by` the pass stops once `keep`
/// matches are held and the total is already known: from the caller (a
/// cursor continuation on its pinned view), from counters (no predicate),
/// or from the index plan's owner set when the index answered the whole
/// predicate. With `order_by` it keeps the best `keep` rows seen so far.
/// The working set (the candidate owner set, retained ordinals, ordering
/// rows and the record being evaluated) is charged against
/// `query-memory-max`.
fn run_matches(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &QuerySnapshot,
    keep: usize,
    known_total: Option<u64>,
) -> Result<Matches, QueryError> {
    check_walk_budget(lmpg, snap, req)?;
    let mut working = WorkingSet {
        limit: working_set_budget(lmpg).map(|bytes| u64::try_from(bytes).unwrap_or(u64::MAX)),
        charged: 0,
    };
    let CandidateSource {
        mut candidates,
        exact,
        known,
        predicate,
    } = candidate_source(lmpg, req, is_cancelled, snap, known_total, &mut working)?;
    let ctx = EvalCtx {
        lmpg,
        snap: snap.clone(),
        is_cancelled: Some(is_cancelled),
        cycles: crate::cycle::CycleGraphs::default(),
        declared_types: std::cell::RefCell::default(),
    };
    let mut session = reconstruct::ReconstructSession::new(snap, lmpg.tenant);
    let order_fields = if req.order_by.is_empty() {
        None
    } else {
        PointFields::resolve(
            snap,
            lmpg.tenant,
            req.order_by.iter().map(|(f, _)| f.as_str()),
        )?
    };
    let predicate_fields = match &predicate {
        Some(expr) => PointFields::for_predicate(snap, lmpg.tenant, expr)?,
        None => None,
    };
    let mut retained: Vec<u64> = Vec::new();
    // Ordered scans hold `(ordinal, row, row bytes)`, sorted and cut back to
    // `keep` whenever they double, so memory follows `keep`, not the matches.
    let mut ordered: Vec<OrderRow> = Vec::new();
    let mut ordered_bytes: Vec<u64> = Vec::new();
    let mut total: u64 = 0;
    while let Some(ord) = candidates.next(is_cancelled)? {
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        if !exact
            && !is_match(
                &ctx,
                req,
                predicate.as_ref(),
                predicate_fields.as_ref(),
                &mut session,
                &mut working,
                ord,
            )?
        {
            continue;
        }
        total += 1;
        if req.order_by.is_empty() {
            if retained.len() < keep {
                working.charge(8, true)?;
                retained.push(ord);
            } else if known.is_some() {
                break;
            }
            continue;
        }
        let row = read_row(order_fields.as_ref(), &mut session, snap, lmpg.tenant, ord)?;
        let bytes =
            serde_json::to_vec(&row).map_or(0, |v| u64::try_from(v.len()).unwrap_or(u64::MAX));
        working.charge(bytes, true)?;
        ordered.push((ord, row));
        ordered_bytes.push(bytes);
        if ordered.len() >= keep.saturating_mul(2).max(64) {
            working.charged = working.charged.saturating_sub(cut_ordered(
                &mut ordered,
                &mut ordered_bytes,
                req,
                keep,
            ));
        }
    }
    if !req.order_by.is_empty() {
        cut_ordered(&mut ordered, &mut ordered_bytes, req, keep);
        retained = ordered.into_iter().map(|(ord, _)| ord).collect();
    }
    Ok(Matches {
        retained,
        total: known.unwrap_or(total),
    })
}

/// The bytes a search holds at once, against `query-memory-max`.
struct WorkingSet {
    limit: Option<u64>,
    charged: u64,
}

impl WorkingSet {
    /// Admit `bytes`; `retained` bytes stay charged, others (a record that
    /// only feeds the predicate) only need to fit in the remaining room.
    fn charge(&mut self, bytes: u64, retained: bool) -> Result<(), QueryError> {
        if let Some(limit) = self.limit {
            if self.charged.saturating_add(bytes) > limit {
                return Err(QueryError::ResourceLimit(format!(
                    "query working set exceeded {limit} bytes"
                )));
            }
            if retained {
                self.charged += bytes;
            }
        }
        Ok(())
    }
}

/// Whether candidate `ord` is a live match: the record exists, `predicate`
/// holds on it, and every relationship branch binds.
fn is_match(
    ctx: &EvalCtx<'_>,
    req: &SearchRequest,
    predicate: Option<&Expr>,
    fields: Option<&PointFields>,
    session: &mut reconstruct::ReconstructSession<'_, QuerySnapshot>,
    working: &mut WorkingSet,
    ord: u64,
) -> Result<bool, QueryError> {
    let (snap, tenant) = (&ctx.snap, ctx.lmpg.tenant);
    if access::record_by_ord(snap, tenant, ord)?.is_none() {
        return Ok(false);
    }
    if let Some(expr) = predicate {
        let obj = read_row(fields, session, snap, tenant, ord)?;
        let bytes =
            serde_json::to_vec(&obj).map_or(0, |v| u64::try_from(v.len()).unwrap_or(u64::MAX));
        working.charge(bytes, false)?;
        if !eval_expr_ctx(ctx, expr, &obj)? {
            return Ok(false);
        }
    }
    for spec in &req.related {
        if !related_exists(snap, tenant, ord, spec)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The properties `fields` names, by point reads, or the rebuilt record.
fn read_row(
    fields: Option<&PointFields>,
    session: &mut reconstruct::ReconstructSession<'_, QuerySnapshot>,
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ord: u64,
) -> Result<Map<String, Json>, QueryError> {
    match fields {
        Some(fields) => fields.row(snap, tenant, ord),
        None => Ok(session
            .reconstruct_record(ord)?
            .and_then(|record| match record {
                Json::Object(map) => Some(map),
                _ => None,
            })
            .unwrap_or_default()),
    }
}

/// Where a search's candidates come from and what is left to check.
struct CandidateSource<'v> {
    candidates: scan::Candidates<'v>,
    /// Every candidate is a live match: neither the predicate nor the
    /// record's existence is checked per candidate.
    exact: bool,
    /// The total, when known without counting.
    known: Option<u64>,
    /// What is left to check per candidate: the conjuncts the owner set does
    /// not answer, or the whole `where` on a walk.
    predicate: Option<Expr>,
}

/// The index plan's owner set (charged to `working`), or a walk of the
/// tenant's or labels' records.
fn candidate_source<'v>(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &'v QuerySnapshot,
    known_total: Option<u64>,
    working: &mut WorkingSet,
) -> Result<CandidateSource<'v>, QueryError> {
    if let Some(plan) =
        index_candidates_with_budget(lmpg, snap, req, working_set_budget(lmpg), is_cancelled)?
    {
        working.charge(
            u64::try_from(plan.ords.serialized_size()).unwrap_or(u64::MAX),
            true,
        )?;
        let known = known_total
            .or(plan.known_total)
            .or_else(|| plan.exact.then(|| plan.ords.len()));
        return Ok(CandidateSource {
            candidates: scan::Candidates::from_set(plan.ords),
            exact: plan.exact,
            known,
            predicate: plan.residual,
        });
    }
    let exact = req.where_.is_none() && req.related.is_empty();
    let known = match known_total {
        Some(total) => Some(total),
        None if exact => Some(walk_total(lmpg, snap, &req.labels)?),
        None => None,
    };
    Ok(CandidateSource {
        candidates: scan::Candidates::walk(snap, lmpg.tenant, &req.labels)?,
        exact,
        known,
        predicate: req.where_.clone(),
    })
}

/// One ordering row: a match's ordinal and the properties it is ordered by.
type OrderRow = (u64, Map<String, Json>);

/// Sort the ordering buffer and keep its best `keep` rows; returns the bytes
/// of the rows dropped.
fn cut_ordered(
    ordered: &mut Vec<OrderRow>,
    bytes: &mut Vec<u64>,
    req: &SearchRequest,
    keep: usize,
) -> u64 {
    let mut rows: Vec<(OrderRow, u64)> = ordered.drain(..).zip(bytes.drain(..)).collect();
    // Candidates arrive in ascending ordinal order and the sort is stable,
    // so ties keep that order across cuts, exactly as one sort of every
    // match would.
    sort_ordered_rows(&mut rows, req);
    let freed = rows.iter().skip(keep).map(|(_, b)| *b).sum();
    rows.truncate(keep);
    for (row, b) in rows {
        ordered.push(row);
        bytes.push(b);
    }
    freed
}

/// Live records of the tenant (`labels` empty) or of `labels`, from counters.
fn walk_total(lmpg: &Lmpg, snap: &QuerySnapshot, labels: &[String]) -> Result<u64, QueryError> {
    if labels.is_empty() {
        return Ok(rushdb_lmpg::derived::tenant_usage(snap.as_ref(), lmpg.tenant)?.records);
    }
    let distinct: std::collections::BTreeSet<&String> = labels.iter().collect();
    let mut total = 0u64;
    for label in distinct {
        total = total.saturating_add(rushdb_lmpg::derived::label_count(
            snap.as_ref(),
            lmpg.tenant,
            label,
        )?);
    }
    Ok(total)
}

/// Execute against a caller-owned statement view, allowing response metadata
/// and follow-up reconstruction to share the same committed version. The
/// view must belong to `lmpg.store`; the caller owns authorization scope.
///
/// # Errors
///
/// Returns a storage error when `snap` belongs to a different store, and
/// [`QueryError::Cancelled`] when the probe requests a stop. Storage/engine
/// read errors and predicate evaluation failures are propagated.
pub fn find_records_in_view(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
) -> Result<Page, QueryError> {
    find_records_in_view_with_budget(lmpg, req, is_cancelled, snap, None)
}

/// Refuse a filter that has no usable index when the records it would check
/// exceed `unindexed-walk-max-owners` or the request's own limit (SCALE-3,
/// QRY-3; none by default, like PostgreSQL). The population is exact
/// and read from counters before any work: the walked property's support,
/// or, when no condition can be walked, every record of the labels.
fn check_walk_budget(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    req: &SearchRequest,
) -> Result<(), QueryError> {
    let (Some(limit), Some(expr)) = (lmpg.limits().unindexed_walk_max_owners(), &req.where_) else {
        return Ok(());
    };
    if explain_index(snap.as_ref(), lmpg.tenant, &req.labels, expr)?.is_some() {
        return Ok(());
    }
    let walk = explain_walk(snap.as_ref(), lmpg.tenant, &req.labels, expr)?;
    let (property, owners, labels) = if let Some(walk) = walk {
        walk
    } else {
        // No condition can be walked: every record of the labels is checked.
        let owners = if req.labels.is_empty() {
            rushdb_lmpg::derived::tenant_usage(snap.as_ref(), lmpg.tenant)?.records
        } else {
            let mut owners = 0u64;
            for label in &req.labels {
                owners = owners.saturating_add(rushdb_lmpg::derived::label_count(
                    snap.as_ref(),
                    lmpg.tenant,
                    label,
                )?);
            }
            owners
        };
        (
            "(no indexable condition)".to_owned(),
            owners,
            req.labels.clone(),
        )
    };
    if owners <= limit {
        return Ok(());
    }
    Err(QueryError::UnindexedWalk(format!(
        "the filter on `{property}` would check {owners} records of [{}], over \
         the unindexed-walk limit ({limit}); declare an ordered index on the property",
        labels.join(", ")
    )))
}

/// The query working-set cap: candidate and match ordinals, ordering rows
/// and the record being evaluated. It is `query-memory-max` (`0` = off),
/// tightened by the request,
/// never the request's response byte budget, which caps only the returned
/// page (READ-1: charging the working set against the 16 MiB response
/// budget refused every read of a label over ~2M records).
fn working_set_budget(lmpg: &Lmpg) -> Option<usize> {
    lmpg.limits()
        .query_memory_max_bytes()
        .map(|bytes| usize::try_from(bytes).unwrap_or(usize::MAX))
}

/// The caller's cancellation or the request's deadline, so a
/// time cap stops every read that runs on the request's handle.
pub(crate) fn cancel_probe<'a>(
    lmpg: &Lmpg,
    is_cancelled: &'a dyn Fn() -> bool,
) -> impl Fn() -> bool + 'a {
    let limits = lmpg.limits();
    move || is_cancelled() || limits.expired()
}

/// Execute a search with an optional caller-owned response byte budget for
/// the returned page. The working set (candidates, matches, ordering rows) is
/// capped by `query-memory-max` instead ([`working_set_budget`]).
///
/// # Errors
/// Returns [`QueryError`] when the view is invalid, cancellation is observed,
/// storage is corrupt, or the working-byte budget is exceeded.
#[allow(clippy::needless_pass_by_value)]
pub fn find_records_in_view_with_budget(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
    work_budget: Option<usize>,
) -> Result<Page, QueryError> {
    find_page(lmpg, req, is_cancelled, &snap, work_budget, None)
}

/// Streaming execution: identical to [`find_records_in_view`]; kept as the
/// name the differential tests compare against the forced-scan path.
///
/// # Errors
///
/// Returns [`QueryError::Cancelled`] when the probe requests a stop, and
/// propagates storage/engine read errors and predicate evaluation failures.
pub fn find_records_streaming(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
) -> Result<Page, QueryError> {
    find_records_streaming_with_total(lmpg, req, is_cancelled, snap, None)
}

/// Page with a caller-supplied total (E6 cursor continuations). The total
/// is only valid when the caller reuses the exact pinned snapshot and match
/// set the total was computed from — the native cursor guarantees both, so
/// continuations stop after their page instead of counting again. `None`
/// computes it (ordinary search path).
///
/// # Errors
///
/// Returns [`QueryError::Cancelled`] when the probe requests a stop, and
/// propagates storage/engine read errors and predicate evaluation failures.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the snapshot is accepted by value as part of the public API contract so callers can hand over a freshly pinned view"
)]
pub fn find_records_streaming_with_total(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
    known_total: Option<u64>,
) -> Result<Page, QueryError> {
    find_page(lmpg, req, is_cancelled, &snap, None, known_total)
}

/// The one records-page executor: match ([`run_matches`]), cut the page,
/// reconstruct it against the response budget.
fn find_page(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &QuerySnapshot,
    work_budget: Option<usize>,
    known_total: Option<u64>,
) -> Result<Page, QueryError> {
    let is_cancelled: &dyn Fn() -> bool = &cancel_probe(lmpg, is_cancelled);
    if !snap.belongs_to(&lmpg.store) {
        return Err(rushdb_storage::StorageError::InvalidRead(
            "query view belongs to another Store".into(),
        )
        .into());
    }
    if is_cancelled() {
        return Err(QueryError::Cancelled);
    }
    let skip = usize::try_from(req.pagination.skip).unwrap_or(usize::MAX);
    let limit = usize::try_from(req.pagination.limit.unwrap_or(u64::MAX)).unwrap_or(usize::MAX);
    let matches = run_matches(
        lmpg,
        req,
        is_cancelled,
        snap,
        skip.saturating_add(limit),
        known_total,
    )?;
    let page_roots: Vec<u64> = matches
        .retained
        .into_iter()
        .skip(skip)
        .take(limit)
        .collect();
    let budget = effective_budget(
        work_budget,
        lmpg.limits().query_memory_max_bytes().unwrap_or(0),
    );
    let records =
        page::reconstruct_page_with_budget(lmpg, req, is_cancelled, snap, page_roots, budget)?;
    Ok(Page {
        total_matched: matches.total,
        records,
    })
}

/// Sort ordering rows by the declared query order. `Vec::sort_by` is
/// stable, so ties preserve candidate order and pagination remains repeatable.
fn sort_ordered_rows<T>(sortable: &mut [(OrderRow, T)], req: &SearchRequest) {
    for (field, desc) in req.order_by.iter().rev() {
        let field = normalize(field);
        sortable.sort_by(|((_, a), _), ((_, b), _)| {
            let va = a
                .iter()
                .find(|(k, _)| normalize(k) == field)
                .map(|(_, v)| v);
            let vb = b
                .iter()
                .find(|(k, _)| normalize(k) == field)
                .map(|(_, v)| v);
            let ord = match (va, vb) {
                (Some(x), Some(y)) => json_value_cmp(x, y),
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };
            if *desc {
                ord.reverse()
            } else {
                ord
            }
        });
    }
}

/// `(ordinal, raw record, projected record)` for one matching collected child.
type RelatedItem = (u64, Map<String, Json>, Map<String, Json>);

fn collect_neighbors(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    root: u64,
    spec: &CollectSpec,
    budget: Option<usize>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u64>, QueryError> {
    Ok(if spec.steps.is_empty() {
        let edges: Vec<(String, u64, u64)> = match spec.direction {
            Direction::Outgoing => {
                access::adj_out(snap, lmpg.tenant, root, spec.rel_type.as_deref())?
            }
            Direction::Incoming => {
                access::adj_in(snap, lmpg.tenant, root, spec.rel_type.as_deref())?
            }
            Direction::Both => {
                let mut all = access::adj_out(snap, lmpg.tenant, root, spec.rel_type.as_deref())?;
                all.extend(access::adj_in(
                    snap,
                    lmpg.tenant,
                    root,
                    spec.rel_type.as_deref(),
                )?);
                all
            }
        };
        edges.into_iter().map(|(_, other, _)| other).collect()
    } else {
        traverse_alias(
            lmpg,
            snap,
            lmpg.tenant,
            root,
            &spec.steps,
            budget,
            is_cancelled,
        )?
    })
}

fn order_collected_items(items: &mut Vec<RelatedItem>, spec: &CollectSpec) {
    // uniqueness by identity.
    if spec.unique {
        items.sort_by_key(|(o, _, _)| *o);
        items.dedup_by(|a, b| a.1.get(ID_FIELD) == b.1.get(ID_FIELD));
    } else {
        items.sort_by_key(|(o, _, _)| *o);
    }
    // order by collected output aliases (fall back to the raw field).
    if !spec.order_by.is_empty() {
        items.sort_by(|a, b| {
            for (field, desc) in &spec.order_by {
                let av = raw_field(&a.2, field).or_else(|| raw_field(&a.1, field));
                let bv = raw_field(&b.2, field).or_else(|| raw_field(&b.1, field));
                let ord = scalar_cmp(av, bv).unwrap_or(std::cmp::Ordering::Equal);
                if ord != std::cmp::Ordering::Equal {
                    return if *desc { ord.reverse() } else { ord };
                }
            }
            std::cmp::Ordering::Equal
        });
    }
}

fn attach_collects_with_budget(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    root: u64,
    mut record: Json,
    collects: &[CollectSpec],
    budget: Option<usize>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Json, QueryError> {
    if collects.is_empty() {
        return Ok(record);
    }
    let mut retained = aggregate_budget::value_bytes(&record);
    aggregate_budget::check(budget, retained)?;
    let ctx = EvalCtx {
        lmpg,
        snap: snap.clone(),
        is_cancelled: None,
        cycles: crate::cycle::CycleGraphs::default(),
        declared_types: std::cell::RefCell::default(),
    };
    if let Some(obj) = record.as_object_mut() {
        for spec in collects {
            // Alias-based (`from`) collects traverse the full path (multi-hop)
            // applying every level's `where` predicates; label-based collects
            // use direct adjacency.
            if is_cancelled() {
                return Err(QueryError::Cancelled);
            }
            let neighbors = collect_neighbors(
                lmpg,
                snap,
                root,
                spec,
                budget.map(|limit| limit.saturating_sub(retained)),
                is_cancelled,
            )?;
            // (ordinal, raw record, projected record) for every matching child.
            let base_bytes = retained;
            retained = retained.saturating_add(neighbors.capacity().saturating_mul(8));
            aggregate_budget::check(budget, retained)?;
            let mut items: Vec<RelatedItem> = Vec::new();
            for other in neighbors {
                if is_cancelled() {
                    return Err(QueryError::Cancelled);
                }
                if !spec.target_labels.is_empty()
                    && !access::record_by_ord(snap, lmpg.tenant, other)?
                        .is_some_and(|h| spec.target_labels.contains(&h.label))
                {
                    continue;
                }
                let reader = aggregate_budget::RecordReader::new(
                    snap,
                    budget.map(|limit| limit.saturating_sub(retained)),
                );
                let Some(Json::Object(child)) =
                    reconstruct::reconstruct_record(&reader, lmpg.tenant, other)?
                else {
                    continue;
                };
                // label-based `where` filter on the collected level.
                if let Some(filter) = &spec.filter {
                    if !eval_expr_ctx(&ctx, filter, &child)? {
                        continue;
                    }
                }
                retained = retained
                    .saturating_add(aggregate_budget::object_bytes(&child).saturating_mul(3));
                aggregate_budget::check(budget, retained)?;
                let projected = match project_fields_map(Json::Object(child.clone()), &spec.fields)
                {
                    Json::Object(p) => p,
                    _ => child.clone(),
                };
                items.push((other, child, projected));
            }
            order_collected_items(&mut items, spec);
            // slice then recurse nested collects on the projected record.
            let start = usize::try_from(spec.skip)
                .unwrap_or(usize::MAX)
                .min(items.len());
            let end = spec.limit.map_or(items.len(), |l| {
                start
                    .saturating_add(usize::try_from(l).unwrap_or(usize::MAX))
                    .min(items.len())
            });
            let mut projected = Vec::new();
            for (other, raw, child) in items.into_iter().take(end).skip(start) {
                retained =
                    retained.saturating_sub(aggregate_budget::object_bytes(&raw).saturating_mul(3));
                let child = attach_collects_with_budget(
                    lmpg,
                    snap,
                    other,
                    Json::Object(child),
                    &spec.nested,
                    budget.map(|limit| limit.saturating_sub(retained)),
                    is_cancelled,
                )?;
                retained = retained.saturating_add(aggregate_budget::value_bytes(&child));
                aggregate_budget::check(budget, retained)?;
                projected.push(child);
            }
            let collected = Json::Array(projected);
            retained = base_bytes
                .saturating_add(spec.alias.len())
                .saturating_add(aggregate_budget::value_bytes(&collected));
            aggregate_budget::check(budget, retained)?;
            obj.insert(spec.alias.clone(), collected);
        }
    }
    Ok(record)
}

/// Project a collected record into a `(output_key, source_field)` map; an
/// empty map keeps the whole record (alias-based default).
fn project_fields_map(value: Json, fields: &[(String, String)]) -> Json {
    if fields.is_empty() {
        return value;
    }
    let mut out = Map::new();
    if let Json::Object(obj) = value {
        for (out_key, source) in fields {
            if let Some(v) = resolve_field(&obj, source) {
                out.insert(out_key.clone(), v);
            }
        }
    }
    Json::Object(out)
}

/// Scalar ordering for collected-record sorting: numeric for numbers,
/// lexical for strings (ISO datetimes order correctly), else equal.
fn scalar_cmp(a: Option<&Json>, b: Option<&Json>) -> Option<std::cmp::Ordering> {
    let (Some(a), Some(b)) = (a, b) else {
        return None;
    };
    match (a, b) {
        (Json::Number(_), Json::Number(_)) => {
            let left = infer_scalar(a).map(|(_, scalar)| scalar);
            let right = infer_scalar(b).map(|(_, scalar)| scalar);
            left.as_ref()
                .zip(right.as_ref())
                .and_then(|(left, right)| canonical_scalar_cmp(left, right))
        }
        (Json::String(x), Json::String(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

fn related_exists(
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ord: u64,
    spec: &RelatedSpec,
) -> Result<bool, QueryError> {
    let edges: Vec<(String, u64, u64)> = match spec.direction {
        Direction::Outgoing => access::adj_out(snap, tenant, ord, spec.rel_type.as_deref())?,
        Direction::Incoming => access::adj_in(snap, tenant, ord, spec.rel_type.as_deref())?,
        Direction::Both => {
            let mut all = access::adj_out(snap, tenant, ord, spec.rel_type.as_deref())?;
            all.extend(access::adj_in(snap, tenant, ord, spec.rel_type.as_deref())?);
            all
        }
    };
    if spec.target_labels.is_empty() {
        return Ok(!edges.is_empty());
    }
    for (_, other_ord, _) in edges {
        if access::record_by_ord(snap, tenant, other_ord)?
            .is_some_and(|h| spec.target_labels.contains(&h.label))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Distinct related ordinals for a `$count` target that names a traversal
/// alias from `where`/`related` (e.g. `$count: "$battle"`). Returns `None`
/// when the target is not an alias, so callers fall back to field counting.
/// The alias's full traversal path is followed (multi-hop) and every level's
/// `where` predicates are applied, matching the reference binding semantics.
fn count_related(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ord: u64,
    specs: &[RelatedSpec],
    alias: &str,
    budget: Option<usize>,
) -> Result<Option<Vec<u64>>, QueryError> {
    let Some(spec) = specs.iter().find(|s| {
        s.alias
            .as_deref()
            .is_some_and(|a| a.trim_start_matches('$') == alias)
    }) else {
        return Ok(None);
    };
    Ok(Some(traverse_alias(
        lmpg,
        snap,
        tenant,
        ord,
        &spec.steps,
        budget,
        &|| false,
    )?))
}

/// Follow a traversal path from `ord` (root) through each step, applying the
/// step's target-label filter, rel type/direction, and endpoint predicates.
/// Returns the distinct ordinals bound at the final level.
fn traverse_alias(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ord: u64,
    steps: &[TraversalStep],
    budget: Option<usize>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u64>, QueryError> {
    if steps.is_empty() {
        return Ok(Vec::new());
    }
    let ctx = EvalCtx {
        lmpg,
        snap: snap.clone(),
        is_cancelled: Some(is_cancelled),
        cycles: crate::cycle::CycleGraphs::default(),
        declared_types: std::cell::RefCell::default(),
    };
    let mut cur = vec![ord];
    for step in steps {
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        let mut next: Vec<u64> = Vec::new();
        for o in &cur {
            if is_cancelled() {
                return Err(QueryError::Cancelled);
            }
            let edges: Vec<(String, u64, u64)> = match step.direction {
                Direction::Outgoing => access::adj_out(snap, tenant, *o, step.rel_type.as_deref())?,
                Direction::Incoming => access::adj_in(snap, tenant, *o, step.rel_type.as_deref())?,
                Direction::Both => {
                    let mut all = access::adj_out(snap, tenant, *o, step.rel_type.as_deref())?;
                    all.extend(access::adj_in(snap, tenant, *o, step.rel_type.as_deref())?);
                    all
                }
            };
            for (_, other, _) in edges {
                if !step.target_labels.is_empty()
                    && !access::record_by_ord(snap, tenant, other)?
                        .is_some_and(|h| step.target_labels.contains(&h.label))
                {
                    continue;
                }
                if let Some(pred) = &step.filter {
                    let reader = aggregate_budget::RecordReader::new(
                        snap,
                        budget.map(|limit| {
                            limit.saturating_sub(
                                cur.len().saturating_add(next.len()).saturating_mul(32),
                            )
                        }),
                    );
                    let props = reconstruct::reconstruct_record(&reader, tenant, other)?;
                    let obj = props
                        .as_ref()
                        .and_then(|p| p.as_object().cloned())
                        .unwrap_or_default();
                    if !eval_expr_ctx(&ctx, pred, &obj)? {
                        continue;
                    }
                }
                aggregate_budget::check(
                    budget,
                    cur.len()
                        .saturating_add(next.len())
                        .saturating_add(1)
                        .saturating_mul(32),
                )?;
                next.push(other);
            }
        }
        next.sort_unstable();
        next.dedup();
        cur = next;
    }
    Ok(cur)
}

/// Collect the `where`-block related aliases (as [`RelatedSpec`]s) so aggregate
/// `$count`/`$sum` targets can reference them. Aggregates keep `related` empty;
/// the aliases live inside the `Expr::Related` nodes of `where_`.
fn related_specs_from_where(expr: Option<&Expr>) -> Vec<RelatedSpec> {
    /// Split a level's filter into its endpoint predicates and the nested
    /// label-block children (each child becomes a further traversal step).
    fn split_level(filter: Option<&Expr>, children: &mut Vec<Expr>) -> Option<Expr> {
        let f = filter?;
        match f {
            Expr::And(items) => {
                let mut preds: Vec<Expr> = Vec::new();
                for item in items {
                    match item {
                        Expr::Related(_) => children.push(item.clone()),
                        other => preds.push(other.clone()),
                    }
                }
                match preds.len() {
                    0 => None,
                    1 => Some(preds.remove(0)),
                    _ => Some(Expr::And(preds)),
                }
            }
            Expr::Related(_) => {
                children.push(f.clone());
                None
            }
            other => Some(other.clone()),
        }
    }

    fn walk(expr: &Expr, path: &mut Vec<TraversalStep>, out: &mut Vec<RelatedSpec>) {
        match expr {
            Expr::And(items) | Expr::Or(items) | Expr::Nor(items) => {
                for e in items {
                    walk(e, path, out);
                }
            }
            Expr::Xor(a, b) => {
                walk(a, path, out);
                walk(b, path, out);
            }
            Expr::Not(b) => {
                walk(b, path, out);
            }
            Expr::Related(rel) => {
                let mut children: Vec<Expr> = Vec::new();
                let filter = split_level(rel.filter.as_deref(), &mut children);
                let step = TraversalStep {
                    target_labels: rel.target_labels.clone(),
                    rel_type: rel.rel_type.clone(),
                    direction: rel.direction,
                    filter,
                };
                path.push(step);
                if let Some(alias) = &rel.alias {
                    out.push(RelatedSpec {
                        alias: Some(alias.trim_start_matches('$').to_string()),
                        rel_type: rel.rel_type.clone(),
                        direction: rel.direction,
                        target_labels: rel.target_labels.clone(),
                        steps: path.clone(),
                    });
                }
                for child in &children {
                    walk(child, path, out);
                }
                path.pop();
            }
            Expr::Field(_) => {}
        }
    }
    let mut specs = Vec::new();
    if let Some(e) = expr {
        let mut path: Vec<TraversalStep> = Vec::new();
        walk(e, &mut path, &mut specs);
    }
    specs
}

/// Resolve `alias.field` values across a root's related records (the docs'
/// aggregate-over-alias semantics: `$sum: "$employee.salary"`). Returns `None`
/// when `field` is not an alias reference.
fn alias_field_values(
    lmpg: &Lmpg,
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ord: u64,
    specs: &[RelatedSpec],
    field: &str,
    budget: Option<usize>,
) -> Result<Option<Vec<Json>>, QueryError> {
    let Some((alias, prop)) = field.split_once('.') else {
        return Ok(None);
    };
    let Some(related) = count_related(lmpg, snap, tenant, ord, specs, alias, budget)? else {
        return Ok(None);
    };
    let mut retained = related.capacity().saturating_mul(8);
    aggregate_budget::check(budget, retained)?;
    let mut values = Vec::new();
    for other in related {
        let reader = aggregate_budget::RecordReader::new(
            snap,
            budget.map(|limit| limit.saturating_sub(retained)),
        );
        if let Some(Json::Object(child)) = reconstruct::reconstruct_record(&reader, tenant, other)?
        {
            if let Some(v) = resolve_field(&child, prop) {
                retained = retained.saturating_add(aggregate_budget::value_bytes(&v));
                aggregate_budget::check(budget, retained)?;
                values.push(v);
            }
        }
    }
    Ok(Some(values))
}

/// Group-key sentinel emitted when a `groupBy`/timeBucket source field is
/// missing (distinct from an explicit JSON null).
pub const MISSING_SENTINEL: &str = "__missing__";

/// One aggregation bucket carrying its typed group tuple, per-metric maps
/// and materialized derived outputs.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GroupBucket {
    /// One entry per `group_by` field; missing renders as the documented
    /// MISSING sentinel so explicit Null (`Json::Null`) stays distinct.
    pub keys: Map<String, Json>,
    /// Number of root records bound to this bucket.
    pub count_roots: u64,
    /// `$sum` outputs keyed by normalized output name.
    pub sums: Map<String, Json>,
    /// `$count` outputs: a JSON array holding each distinct counted value.
    pub counts: Map<String, Json>,
    /// `$min` outputs keyed by normalized output name.
    pub mins: Map<String, Json>,
    /// `$max` outputs keyed by normalized output name.
    pub maxs: Map<String, Json>,
    /// Materialized derived outputs keyed by normalized name.
    pub derived_outputs: std::collections::BTreeMap<String, Json>,
}

/// Derived-output expression graph ($ref chains in document 19):
/// arithmetic over other named outputs, evaluated after all source metrics.
#[derive(Debug, Clone)]
pub enum OutputExpr {
    /// Reference to another named output.
    Ref(String),
    /// Raw property on the root/related record, evaluated per row.
    Field(String),
    /// Addition of two derived expressions.
    Add(Box<OutputExpr>, Box<OutputExpr>),
    /// Subtraction of the second expression from the first.
    Subtract(Box<OutputExpr>, Box<OutputExpr>),
    /// Multiplication of two derived expressions.
    Multiply(Box<OutputExpr>, Box<OutputExpr>),
    /// Division of `dividend` by `divisor`, rounded to `precision` decimals.
    Divide {
        /// Numerator expression.
        dividend: Box<OutputExpr>,
        /// Denominator expression.
        divisor: Box<OutputExpr>,
        /// Decimal places to round the quotient to.
        precision: u32,
    },
    /// A numeric literal.
    Literal(f64),
}

/// A branch-grain aggregation request over distinct roots.
#[derive(Debug, Clone, Default)]
pub struct AggregateRequest {
    /// Labels a root record must carry; empty matches every label.
    pub labels: Vec<String>,
    /// Boolean predicate the root record must satisfy.
    pub where_: Option<Expr>,
    /// Independent traversal branches filtering the roots existentially.
    pub related: Vec<RelatedSpec>,
    /// Group key field names (`$record.field`-style without prefix).
    pub group_by: Vec<String>,
    /// Sum metrics keyed by output name -> source field.
    pub sums: Vec<(String, String)>,
    /// Distinct-root or distinct-entity counts keyed by output name;
    /// `"*"` counts root identity per document 19 semantics.
    pub counts: Vec<(String, String)>,
    /// Min/max aggregates keyed by output name -> source field.
    pub mins: Vec<(String, String)>,
    /// `$max` aggregates keyed by output name -> source field.
    pub maxs: Vec<(String, String)>,
    /// Calendar dimension added to group keys when present.
    pub time_bucket: Option<TimeBucketSpec>,
    /// Derived outputs evaluated after all source metrics (doc 19).
    pub outputs: Vec<(String, OutputExpr)>,
    /// Per-root math expressions summed into an alias ($sum over math).
    pub expr_sums: Vec<(String, OutputExpr)>,
    /// Per-root math expressions averaged into an alias ($avg over math).
    pub expr_avgs: Vec<(String, OutputExpr)>,
    /// Plain-field averages ($avg over a field ref): alias -> field.
    pub avg_terms: Vec<(String, String)>,
    /// Rounding precision per avg alias.
    pub avg_precision: std::collections::BTreeMap<String, u32>,
    /// Alias under which the time-bucket dimension is emitted.
    pub time_bucket_alias: Option<String>,
    /// Plain field projections in listing mode.
    pub plain_fields: Vec<(String, String)>,
    /// Per-root `$collect` output shapes attached to each aggregate row
    /// (the aggregate path mirrors the listing collect behavior).
    pub collects: Vec<CollectSpec>,
    /// `AggregateThenPaginate`: sort completed buckets by output name.
    pub order_by: Vec<(String, bool /* desc */)>,
    /// Number of completed buckets to skip after ordering.
    pub skip: u64,
    /// Maximum number of completed buckets to return; `None` = unbounded.
    pub limit: Option<u64>,
    /// Compiler-generated source metrics used only by derived outputs.
    pub hidden_outputs: std::collections::BTreeSet<String>,
}

/// Traversal-capable evaluation context. Records-perspective execution
/// routes through this when the `where` tree contains `Expr::Related`.
pub struct EvalCtx<'a> {
    /// Engine handle used for tenant scope and store access.
    pub lmpg: &'a Lmpg,
    /// Pinned snapshot all traversal reads resolve against.
    pub snap: QuerySnapshot,
    /// Cooperative cancellation probe, when the caller supplied one.
    pub is_cancelled: Option<&'a dyn Fn() -> bool>,
    /// `$cycle` graphs built for this query.
    pub(crate) cycles: cycle::CycleGraphs,
    /// Declared types resolved for this query, by normalized name: a range
    /// predicate asks once per record (READ-1: two catalog reads per record
    /// and predicate).
    pub(crate) declared_types:
        std::cell::RefCell<std::collections::HashMap<String, Option<LogicalType>>>,
}

impl<'a> EvalCtx<'a> {
    /// Open a context pinned to the store's current committed view.
    ///
    /// # Errors
    ///
    /// Propagates a [`rushdb_storage::StorageError`] if the read view cannot
    /// be opened.
    pub fn new(lmpg: &'a Lmpg) -> Result<Self, QueryError> {
        Ok(EvalCtx {
            lmpg,
            snap: Arc::new(lmpg.store.read_view()?),
            is_cancelled: None,
            cycles: cycle::CycleGraphs::default(),
            declared_types: std::cell::RefCell::default(),
        })
    }

    /// Poll the cancellation probe between traversal steps. A no-op when
    /// the caller supplied no probe.
    ///
    /// # Errors
    /// Returns [`QueryError::Cancelled`] when the probe requests a stop.
    pub fn check_cancelled(&self) -> Result<(), QueryError> {
        if self.is_cancelled.is_some_and(|probe| probe()) {
            Err(QueryError::Cancelled)
        } else {
            Ok(())
        }
    }
}

fn eval_expr_ctx(
    ctx: &EvalCtx<'_>,
    expr: &Expr,
    obj: &Map<String, Json>,
) -> Result<bool, QueryError> {
    match expr {
        Expr::And(items) => {
            for item in items {
                if !eval_expr_ctx(ctx, item, obj)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Expr::Or(items) | Expr::Nor(items) => {
            let inverted = matches!(expr, Expr::Nor(_));
            for item in items {
                if eval_expr_ctx(ctx, item, obj)? {
                    return Ok(!inverted);
                }
            }
            Ok(inverted)
        }
        Expr::Xor(a, b) => Ok(eval_expr_ctx(ctx, a, obj)? != eval_expr_ctx(ctx, b, obj)?),
        Expr::Not(inner) => Ok(!eval_expr_ctx(ctx, inner, obj)?),
        Expr::Field(pred) => eval_pred(&|name| type_of_field(ctx, name), pred, obj),
        Expr::Related(rel) => eval_related(ctx, obj, rel),
    }
}

/// Traversal evaluation with hops + cycle support.
///
/// - normal (`hops`): existential reachability of an endpoint matching the
///   filter at any depth within bounds; edges may be reused across
///   different paths but a path never revisits a node.
/// - `$cycle`: trail semantics — each relationship edge is used at most once
///   per path, and the path must return to the start within bounds
///   (answered by [`cycle::CycleGraphs`]).
fn eval_related(
    ctx: &EvalCtx<'_>,
    root: &Map<String, Json>,
    rel: &RelatedExpr,
) -> Result<bool, QueryError> {
    let root_id = root.get(ID_FIELD).and_then(|v| v.as_str()).unwrap_or("");
    let start_ord = root_ord_of(ctx, root_id)?;
    if start_ord == u64::MAX {
        return Ok(false);
    }
    let (min_hops, max_hops) = rel.hops.unwrap_or((1, 1));

    let neighbors = |node: u64| -> Result<Vec<(String, u64, u64)>, QueryError> {
        Ok(match rel.direction {
            Direction::Outgoing => {
                access::adj_out(&ctx.snap, ctx.lmpg.tenant, node, rel.rel_type.as_deref())?
            }
            Direction::Incoming => {
                access::adj_in(&ctx.snap, ctx.lmpg.tenant, node, rel.rel_type.as_deref())?
            }
            Direction::Both => {
                let mut all =
                    access::adj_out(&ctx.snap, ctx.lmpg.tenant, node, rel.rel_type.as_deref())?;
                all.extend(access::adj_in(
                    &ctx.snap,
                    ctx.lmpg.tenant,
                    node,
                    rel.rel_type.as_deref(),
                )?);
                all
            }
        })
    };

    // endpoint label check for normal traversal
    let endpoint_ok = |other: u64| -> Result<bool, QueryError> {
        if rel.target_labels.is_empty() {
            Ok(true)
        } else {
            Ok(access::record_by_ord(&ctx.snap, ctx.lmpg.tenant, other)?
                .is_some_and(|h| rel.target_labels.contains(&h.label)))
        }
    };

    if rel.cycle {
        let cancel = || ctx.check_cancelled();
        return ctx
            .cycles
            .on_cycle(&ctx.snap, ctx.lmpg.tenant, rel, start_ord, &cancel);
    }

    // normal existential traversal: BFS bounded by max_hops
    let mut frontier: Vec<u64> = vec![start_ord];
    let mut visited: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    visited.insert(start_ord);
    let mut depth = 0u64;

    while !frontier.is_empty() && depth < max_hops {
        ctx.check_cancelled()?;
        depth += 1;
        let mut next: Vec<u64> = Vec::new();
        for node in &frontier {
            for (_ty, other, _ro) in neighbors(*node)? {
                if !endpoint_ok(other)? {
                    continue;
                }
                if depth >= min_hops {
                    if let Some(filter) = &rel.filter {
                        if let Some(target_json) =
                            reconstruct::reconstruct_record(&ctx.snap, ctx.lmpg.tenant, other)?
                        {
                            let target_obj = target_json.as_object().cloned().unwrap_or_default();
                            if eval_expr_ctx(ctx, filter, &target_obj)? {
                                return Ok(true);
                            }
                        }
                    } else {
                        return Ok(true);
                    }
                }
                if depth < max_hops && visited.insert(other) {
                    next.push(other);
                }
            }
        }
        frontier = next;
    }
    Ok(false)
}

fn root_ord_of(ctx: &EvalCtx<'_>, id_hex: &str) -> Result<u64, QueryError> {
    let bytes = hex_decode_16(id_hex).unwrap_or([0u8; 16]);
    let id = rushdb_types::ids::RecordId(bytes);
    Ok(access::record_by_id(&ctx.snap, ctx.lmpg.tenant, &id)?.map_or(u64::MAX, |h| h.ordinal))
}

/// The declared (catalog) type of a property name, when the caller has one.
pub(crate) type DeclaredType<'a> =
    dyn Fn(&str) -> Result<Option<rushdb_types::model::LogicalType>, QueryError> + 'a;

#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive match over every field predicate keeps the operator semantics readable in a single place"
)]
pub(crate) fn eval_pred(
    declared_type: &DeclaredType<'_>,
    pred: &FieldPred,
    obj: &Map<String, Json>,
) -> Result<bool, QueryError> {
    Ok(match pred {
        FieldPred::Exists(name, want) => {
            let present = raw_field(obj, name).is_some();
            present == *want
        }
        FieldPred::IsNull(name, want) => {
            let v = raw_field(obj, name);
            let is_null = matches!(v, Some(Json::Null));
            is_null == *want
        }
        FieldPred::Eq(name, val) => match (raw_field(obj, name), val) {
            (Some(_), Json::Null) => matches!(raw_field(obj, name), Some(Json::Null)),
            (Some(actual), expected) => {
                let datetime = is_datetime(declared_type, name)?;
                any_element(actual, |el| value_equals(el, expected, datetime))
            }
            (None, _) => false,
        },
        FieldPred::Ne(name, val) => {
            // $ne requires the field to be present (RushDB: value <> X).
            match raw_field(obj, name) {
                None => false,
                Some(actual) => {
                    let datetime = is_datetime(declared_type, name)?;
                    !any_element(actual, |el| value_equals(el, val, datetime))
                }
            }
        }
        FieldPred::In(name, vals) => match raw_field(obj, name) {
            None => false,
            Some(actual) => {
                let datetime = is_datetime(declared_type, name)?;
                any_element(actual, |el| {
                    vals.iter().any(|v| value_equals(el, v, datetime))
                })
            }
        },
        FieldPred::Contains(name, sub) => raw_field(obj, name).is_some_and(|j| {
            let sub = sub.to_lowercase();
            any_element(j, |el| {
                el.as_str()
                    .is_some_and(|st| st.to_lowercase().contains(sub.as_str()))
            })
        }),
        FieldPred::StartsWith(name, prefix) => raw_field(obj, name).is_some_and(|j| {
            let prefix = prefix.to_lowercase();
            any_element(j, |el| {
                el.as_str()
                    .is_some_and(|st| st.to_lowercase().starts_with(prefix.as_str()))
            })
        }),
        FieldPred::Gt(name, bound) => numeric_cmp_any(raw_field(obj, name), *bound, |o| {
            o == std::cmp::Ordering::Greater
        }),
        FieldPred::Gte(name, bound) => numeric_cmp_any(raw_field(obj, name), *bound, |o| {
            o != std::cmp::Ordering::Less
        }),
        FieldPred::Lt(name, bound) => numeric_cmp_any(raw_field(obj, name), *bound, |o| {
            o == std::cmp::Ordering::Less
        }),
        FieldPred::Lte(name, bound) => numeric_cmp_any(raw_field(obj, name), *bound, |o| {
            o != std::cmp::Ordering::Greater
        }),
        FieldPred::Nin(name, vals) => {
            // $nin requires presence; present value must match none.
            match raw_field(obj, name) {
                None => false,
                Some(actual) => {
                    let datetime = is_datetime(declared_type, name)?;
                    !any_element(actual, |el| {
                        vals.iter().any(|v| value_equals(el, v, datetime))
                    })
                }
            }
        }
        FieldPred::EndsWith(name, suffix) => raw_field(obj, name).is_some_and(|j| {
            let suffix = suffix.to_lowercase();
            any_element(j, |el| {
                el.as_str()
                    .is_some_and(|st| st.to_lowercase().ends_with(suffix.as_str()))
            })
        }),
        FieldPred::Id(id) => obj
            .get(ID_FIELD)
            .and_then(|v| v.as_str())
            .is_some_and(|s| canonical_id(s) == canonical_id(id)),
        FieldPred::DatetimeComponent {
            field,
            which,
            value,
        } => raw_field(obj, field)
            .and_then(serde_json::Value::as_i64)
            .is_some_and(|ms| {
                let actual = crate::datetime::component_of(ms, *which);
                actual == *value
            }),
        FieldPred::Type(name, expected) => {
            let want = canonical_type_alias(&expected.to_lowercase());
            let (base_want, want_array) = match want.strip_suffix("[]") {
                Some(base) if !base.is_empty() => (Some(canonical_type_alias(base)), true),
                _ => (None, want == "array"),
            };
            let value = raw_field(obj, name);
            // A record without the property is only `null`: the catalog type
            // of the property name must not make it match anything else.
            if value.is_none() {
                return Ok(want == "null");
            }
            let is_array = matches!(value, Some(Json::Array(_)));

            // "array" matches any array-shaped value regardless of cardinality.
            if want == "array" {
                return Ok(is_array);
            }

            // declared-type match: a field whose definition resolves to the
            // expected logical type (datetime/string/number/boolean) matches
            // regardless of the reconstructed JSON shape
            if let Some(ty) = declared_type(name)? {
                let name_of = |t: &rushdb_types::model::LogicalType| -> &'static str {
                    match t {
                        rushdb_types::model::LogicalType::Boolean => "boolean",
                        rushdb_types::model::LogicalType::Int64
                        | rushdb_types::model::LogicalType::Float64 => "number",
                        rushdb_types::model::LogicalType::Utf8 => "string",
                        rushdb_types::model::LogicalType::DateTime => "datetime",
                    }
                };
                if want_array {
                    // `$type: "string[]"`: value must be an array whose element
                    // type is `string` (baseline reports string[] for arrays).
                    if is_array && name_of(&ty) == base_want.as_deref().unwrap_or("") {
                        return Ok(true);
                    }
                } else if name_of(&ty) == want {
                    // `$type: "string"` matches both scalar and string[] values
                    // (RushDB baseline stores array primitives as plain string).
                    return Ok(true);
                }
            }

            let shape_of = |v: &Json| -> &'static str {
                if v.is_string() {
                    "string"
                } else if v.is_number() {
                    "number"
                } else if v.is_boolean() {
                    "boolean"
                } else if v.is_array() {
                    "array"
                } else if v.is_object() {
                    "object"
                } else {
                    "null"
                }
            };

            if want_array {
                // element-shape fallback: any non-null element carries the
                // requested base type
                let base = base_want.as_deref().unwrap_or("");
                return Ok(
                    matches!(value, Some(Json::Array(items)) if items.iter().any(|el| shape_of(el) == base)),
                );
            }

            let actual = value.map_or("null", shape_of);
            actual == want
        }
    })
}

fn numeric_cmp(
    value: Option<&Json>,
    bound: f64,
    accept: impl Fn(std::cmp::Ordering) -> bool,
) -> bool {
    value
        .and_then(infer_scalar)
        .and_then(|(_, scalar)| match scalar {
            Scalar::Int64(_) | Scalar::Float64(_) => Some(scalar),
            _ => None,
        })
        .and_then(|value| canonical_scalar_cmp(&value, &Scalar::Float64(bound)))
        .is_some_and(accept)
}

fn numeric_cmp_any(
    value: Option<&Json>,
    bound: f64,
    accept: impl Fn(std::cmp::Ordering) -> bool,
) -> bool {
    match value {
        Some(v) if v.is_array() => v
            .as_array()
            .is_some_and(|a| a.iter().any(|el| numeric_cmp(Some(el), bound, &accept))),
        other => numeric_cmp(other, bound, accept),
    }
}

/// Canonical client-facing type alias: `bool` is accepted as an alias for
/// `boolean` so legacy queries keep working after the type was renamed for
/// the client surface (NestJS/UI operate with `boolean`).
fn canonical_type_alias(t: &str) -> String {
    if t == "bool" {
        "boolean".to_string()
    } else {
        t.to_string()
    }
}

fn type_of_field(
    ctx: &EvalCtx<'_>,
    name: &str,
) -> Result<Option<rushdb_types::model::LogicalType>, QueryError> {
    let norm = normalize(name);
    if let Some(known) = ctx.declared_types.borrow().get(&norm) {
        return Ok(*known);
    }
    let declared = declared_type_of(ctx, &norm)?;
    ctx.declared_types.borrow_mut().insert(norm, declared);
    Ok(declared)
}

fn declared_type_of(
    ctx: &EvalCtx<'_>,
    norm: &str,
) -> Result<Option<rushdb_types::model::LogicalType>, QueryError> {
    let sig = rushdb_lmpg::reader::keys_ext::family_sig_key(
        ctx.lmpg.tenant,
        "default",
        rushdb_types::ids::OwnerKind::Record,
        norm,
    );
    let Some(raw) = ctx.snap.get(&sig)? else {
        return Ok(None);
    };
    let fid = u64::from_be_bytes(
        raw.as_slice()
            .try_into()
            .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
    );
    for def in access::definitions_of_family(&ctx.snap, ctx.lmpg.tenant, fid)? {
        if let rushdb_types::model::TypeState::Known(ty) = def.type_state {
            return Ok(Some(ty));
        }
    }
    Ok(None)
}

/// Total order for ORDER BY. Handles NaN deterministically (NaN sorts below
/// every number — `partial_cmp(NaN, _) == None` breaks `sort_by`'s total-order
/// requirement), and gives mixed non-numeric types a stable string fallback so
/// the comparator stays transitive.
fn json_value_cmp(a: &Json, b: &Json) -> std::cmp::Ordering {
    let numeric = |v: &Json| {
        infer_scalar(v).and_then(|(_, scalar)| match scalar {
            Scalar::Int64(_) | Scalar::Float64(_) => Some(scalar),
            _ => None,
        })
    };
    match (numeric(a), numeric(b)) {
        (Some(x), Some(y)) => canonical_scalar_cmp(&x, &y).unwrap_or(std::cmp::Ordering::Equal),
        // one side is not a real number (NaN or non-numeric): it sorts first
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, None) => match (a.as_str(), b.as_str()) {
            (Some(x), Some(y)) => x.cmp(y),
            _ => format!("{a}").cmp(&format!("{b}")),
        },
    }
}

fn raw_field<'a>(obj: &'a Map<String, Json>, name: &str) -> Option<&'a Json> {
    let norm = normalize(name);
    obj.iter()
        .find(|(k, _)| normalize(k) == norm)
        .map(|(_, v)| v)
}

fn canonical_id(id: &str) -> String {
    id.bytes()
        .filter(|byte| *byte != b'-')
        .map(|byte| (byte as char).to_ascii_lowercase())
        .collect()
}

/// Distinct root ordinals feeding every perspective and aggregate.
fn binding_roots_with_cancel(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &QuerySnapshot,
    budget: Option<usize>,
) -> Result<Vec<u64>, QueryError> {
    // Ordinal-level matching: never round-trip through reconstructed JSON
    // (`_id` -> `record_by_id`) just to recover an ordinal the index already
    // holds. Reconstruction happens only when a residual predicate or a
    // traversal must be evaluated against record properties.
    let plan = index_candidates_with_budget(lmpg, snap, req, budget, is_cancelled)?;
    let index_exact = plan.as_ref().is_some_and(|plan| plan.exact);
    let candidates: Vec<u64> = match plan {
        Some(plan) => plan.ords.into_iter().collect(),
        None => scan::Candidates::walk(snap, lmpg.tenant, &req.labels)?.collect(is_cancelled)?,
    };
    let candidate_bytes = candidates.capacity().saturating_mul(16);
    aggregate_budget::check(budget, candidate_bytes)?;
    if index_exact && req.order_by.is_empty() {
        return Ok(candidates);
    }
    let ctx = EvalCtx {
        lmpg,
        snap: snap.clone(),
        is_cancelled: Some(is_cancelled),
        cycles: crate::cycle::CycleGraphs::default(),
        declared_types: std::cell::RefCell::default(),
    };
    let mut matched: Vec<u64> = Vec::new();
    for ord in candidates {
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        if access::record_by_ord(snap, lmpg.tenant, ord)?.is_none() {
            continue;
        }
        if let Some(expr) = &req.where_ {
            let reader = aggregate_budget::RecordReader::new(
                snap,
                budget.map(|limit| limit.saturating_sub(candidate_bytes)),
            );
            let mut session = reconstruct::ReconstructSession::new(&reader, lmpg.tenant);
            let props = session.reconstruct_record(ord)?;
            let obj = props
                .as_ref()
                .and_then(|p| p.as_object().cloned())
                .unwrap_or_default();
            if !eval_expr_ctx(&ctx, expr, &obj)? {
                continue;
            }
        }
        let mut all_branches_bound = true;
        for spec in &req.related {
            if !related_exists(snap, lmpg.tenant, ord, spec)? {
                all_branches_bound = false;
                break;
            }
        }
        if all_branches_bound {
            matched.push(ord);
        }
    }
    Ok(matched)
}

/// Normalize a property/label name to the engine's canonical form.
#[must_use]
pub fn normalize(name: &str) -> String {
    rushdb_types::value::normalize_name(name)
}

fn resolve_field(obj: &Map<String, Json>, field: &str) -> Option<Json> {
    let norm = normalize(field.strip_prefix("$record.").unwrap_or(field));
    obj.iter()
        .find(|(k, _)| normalize(k) == norm)
        .map(|(_, v)| v.clone())
}

fn resolve_field_numeric(obj: &Map<String, Json>, field: &str) -> Option<Json> {
    resolve_field(obj, field).filter(serde_json::Value::is_number)
}

#[cfg(test)]
mod index_planner_tests;
