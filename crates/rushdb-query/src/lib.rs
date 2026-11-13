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

mod adjacency;
mod aggregate_budget;
mod aggregate_rows;
mod aggregate_scan;
mod budget;
pub use budget::Budget;
use budget::{Cap, WorkingSet};
mod aggregate_catalog;
mod aggregation;
mod cycle;
mod equality;
mod expand;
mod field_types;
#[cfg(test)]
use aggregation::exact_f64;
pub use aggregation::MAX_AGGREGATE_BUCKETS;
pub use aggregation::{
    aggregate, aggregate_in_view, aggregate_to_rows, aggregate_to_rows_in_view,
    aggregate_to_rows_in_view_with_budget, aggregate_with_cancel, aggregate_with_cancel_in_view,
};
use field_types::type_held;
mod matches;
mod order;
mod page;
mod planned;
mod planner;
mod point_fields;
mod projection;
pub use matches::{explain_order, OrderExplain};
use matches::{filtered_row, is_match, run_matches, walk_total, OrderRow};
pub use point_fields::PointFields;
mod property_predicate;
pub use hex::hex_decode_16;
pub use index_access::explain_index;
pub use planned::{planned_candidates, PlannedCandidates};
use planner::index_candidates_with_budget;
pub use planner::{
    explain_cardinality, explain_expansion, explain_range_selectivity, explain_walk,
    unindexed_walks_total, UNINDEXED_WALK_THRESHOLD,
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
    /// Predicates evaluated on the record at this level, its nested label
    /// blocks included (a record is bound only when every block below it
    /// matches too).
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// A `select` of plain field references: each page root becomes a row of
    /// `(output name, field)` pairs instead of the whole record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection: Vec<(String, String)>,
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
    Ok(run_matches(lmpg, &full, is_cancelled, &snap, 0, usize::MAX, None)?.retained)
}

/// The first `keep` ordinals matching `req`, in match order: the same prefix
/// [`matching_ordinals`] returns, without resolving the rest. A search with
/// no predicate and no `order_by` stops after `keep` matches, so its cost
/// does not grow with the label's size.
///
/// # Errors
///
/// As [`matching_ordinals`].
#[expect(
    clippy::needless_pass_by_value,
    reason = "callers hand over a freshly pinned view, matching find_records_in_view"
)]
pub fn matching_ordinals_first(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: QuerySnapshot,
    keep: usize,
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
    Ok(run_matches(lmpg, &full, is_cancelled, &snap, 0, keep, None)?.retained)
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
    if explain_index(snap.as_ref(), lmpg.tenant, &req.labels, expr)?.is_some()
        || explain_expansion(snap.as_ref(), lmpg.tenant, &req.labels, expr)?.is_some()
    {
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
#[cfg(any(test, feature = "test-support"))]
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
    let matches = run_matches(lmpg, req, is_cancelled, snap, skip, limit, known_total)?;
    let page_roots: Vec<u64> = matches
        .retained
        .into_iter()
        .skip(skip.saturating_sub(matches.offset))
        .take(limit)
        .collect();
    let budget = Cap::for_response(work_budget, lmpg.limits().query_memory_max_bytes());
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
    budget: Option<Cap>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Json, QueryError> {
    if collects.is_empty() {
        return Ok(record);
    }
    let mut retained = aggregate_budget::value_bytes(&record);
    Cap::check_opt(budget, retained)?;
    let ctx = EvalCtx::at(lmpg, snap.clone(), None);
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
                budget.map(|cap| cap.after(retained).remaining()),
                is_cancelled,
            )?;
            // (ordinal, raw record, projected record) for every matching child.
            let base_bytes = retained;
            retained = retained.saturating_add(neighbors.capacity().saturating_mul(8));
            Cap::check_opt(budget, retained)?;
            // A collect that projects `fields` decodes those, its filter's and
            // its ordering's properties only; one without returns whole children.
            let selected = point_fields::collect_selection(spec);
            let mut items: Vec<RelatedItem> = Vec::new();
            for other in neighbors {
                if is_cancelled() {
                    return Err(QueryError::Cancelled);
                }
                if !has_target_label(snap, lmpg.tenant, other, &spec.target_labels)? {
                    continue;
                }
                let Some(child) = aggregate_budget::collected_child(
                    snap,
                    lmpg.tenant,
                    other,
                    selected.as_ref(),
                    budget.map(|cap| cap.after(retained).remaining()),
                )?
                else {
                    continue;
                };
                // label-based `where` filter on the collected level.
                if let Some(filter) = &spec.filter {
                    if !eval_expr_ctx(&ctx, filter, &child, other)? {
                        continue;
                    }
                }
                retained = retained
                    .saturating_add(aggregate_budget::object_bytes(&child).saturating_mul(3));
                Cap::check_opt(budget, retained)?;
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
                    budget.map(|cap| cap.after(retained)),
                    is_cancelled,
                )?;
                retained = retained.saturating_add(aggregate_budget::value_bytes(&child));
                Cap::check_opt(budget, retained)?;
                projected.push(child);
            }
            let collected = Json::Array(projected);
            retained = base_bytes
                .saturating_add(spec.alias.len())
                .saturating_add(aggregate_budget::value_bytes(&collected));
            Cap::check_opt(budget, retained)?;
            obj.insert(spec.alias.clone(), collected);
        }
    }
    Ok(record)
}

/// Project a collected record into a `(output_key, source_field)` map; an
/// empty map keeps the whole record (alias-based default).
/// Whether a collected child carries one of the collect's target labels; an
/// empty target list accepts every child.
fn has_target_label(
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    ordinal: u64,
    target_labels: &[String],
) -> Result<bool, QueryError> {
    if target_labels.is_empty() {
        return Ok(true);
    }
    Ok(access::record_by_ord(snap, tenant, ordinal)?
        .is_some_and(|header| target_labels.contains(&header.label)))
}

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
    let ctx = EvalCtx::at(lmpg, snap.clone(), Some(is_cancelled));
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
                    let obj = filtered_row(&reader, tenant, pred, other)?;
                    if !eval_expr_ctx(&ctx, pred, &obj, other)? {
                        continue;
                    }
                }
                Budget::WorkingSet.check(
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

/// The traversal aliases of a `where` clause as alias specs, each with its
/// full path ([`document::alias_paths`]).
fn related_specs_from_where(expr: Option<&Expr>) -> Vec<RelatedSpec> {
    expr.map(document::alias_paths)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(alias, steps)| {
            let last = steps.last()?.clone();
            Some(RelatedSpec {
                alias: Some(alias),
                rel_type: last.rel_type,
                direction: last.direction,
                target_labels: last.target_labels,
                steps,
            })
        })
        .collect()
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
    Budget::WorkingSet.check(budget, retained)?;
    let mut values = Vec::new();
    // Only the named property is decoded, by either spelling.
    let selected: std::collections::HashSet<String> =
        [prop.to_owned(), normalize(prop)].into_iter().collect();
    for other in related {
        let reader = aggregate_budget::RecordReader::new(
            snap,
            budget.map(|limit| limit.saturating_sub(retained)),
        );
        let mut session = reconstruct::ReconstructSession::new(&reader, tenant);
        if let Some(Json::Object(child)) = session.reconstruct_record_selected(other, &selected)? {
            if let Some(v) = resolve_field(&child, prop) {
                retained = retained.saturating_add(aggregate_budget::value_bytes(&v));
                Budget::WorkingSet.check(budget, retained)?;
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
    /// The root a per-root row (a `select` without `groupBy`) belongs to,
    /// for attaching its `$collect`s; `None` for a group.
    #[serde(skip)]
    pub root: Option<u64>,
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
    /// The typed definitions `(pid, type)` of each name stored under more
    /// than one type (empty otherwise), by normalized name: a record of such
    /// a name is matched by the definition it holds, not the first one.
    pub(crate) record_types:
        std::cell::RefCell<std::collections::HashMap<String, Vec<(u64, LogicalType)>>>,
    /// Adjacency opened for this query, by relationship type and direction:
    /// a per-candidate traversal reuses it, and its storage reads are
    /// rented toward a resident graph once, when the query ends.
    adjacencies: std::cell::RefCell<AdjacencyByShape>,
}

type AdjacencyByShape =
    std::collections::HashMap<(Option<String>, Direction), std::rc::Rc<adjacency::TypeAdjacency>>;

impl<'a> EvalCtx<'a> {
    /// A context reading `snap`, polling `is_cancelled` between steps.
    pub(crate) fn at(
        lmpg: &'a Lmpg,
        snap: QuerySnapshot,
        is_cancelled: Option<&'a dyn Fn() -> bool>,
    ) -> Self {
        EvalCtx {
            lmpg,
            snap,
            is_cancelled,
            cycles: cycle::CycleGraphs::default(),
            declared_types: std::cell::RefCell::default(),
            record_types: std::cell::RefCell::default(),
            adjacencies: std::cell::RefCell::default(),
        }
    }

    /// The adjacency of `rel_type` along `direction` for this query.
    fn adjacency(
        &self,
        rel_type: Option<&str>,
        direction: Direction,
    ) -> Result<std::rc::Rc<adjacency::TypeAdjacency>, QueryError> {
        let key = (rel_type.map(str::to_owned), direction);
        if let Some(open) = self.adjacencies.borrow().get(&key) {
            return Ok(std::rc::Rc::clone(open));
        }
        let cancel = || self.check_cancelled();
        let open = std::rc::Rc::new(adjacency::TypeAdjacency::open(
            &self.snap,
            self.lmpg.tenant,
            rel_type,
            direction,
            &cancel,
        )?);
        self.adjacencies
            .borrow_mut()
            .insert(key, std::rc::Rc::clone(&open));
        Ok(open)
    }

    /// Open a context pinned to the store's current committed view.
    ///
    /// # Errors
    ///
    /// Propagates a [`rushdb_storage::StorageError`] if the read view cannot
    /// be opened.
    pub fn new(lmpg: &'a Lmpg) -> Result<Self, QueryError> {
        Ok(EvalCtx::at(lmpg, Arc::new(lmpg.store.read_view()?), None))
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

/// Evaluate `expr` on record `ord`, whose properties the predicate reads
/// are in `obj`.
fn eval_expr_ctx(
    ctx: &EvalCtx<'_>,
    expr: &Expr,
    obj: &Map<String, Json>,
    ord: u64,
) -> Result<bool, QueryError> {
    match expr {
        Expr::And(items) => {
            for item in items {
                if !eval_expr_ctx(ctx, item, obj, ord)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Expr::Or(items) | Expr::Nor(items) => {
            let inverted = matches!(expr, Expr::Nor(_));
            for item in items {
                if eval_expr_ctx(ctx, item, obj, ord)? {
                    return Ok(!inverted);
                }
            }
            Ok(inverted)
        }
        Expr::Xor(a, b) => Ok(eval_expr_ctx(ctx, a, obj, ord)? != eval_expr_ctx(ctx, b, obj, ord)?),
        Expr::Not(inner) => Ok(!eval_expr_ctx(ctx, inner, obj, ord)?),
        Expr::Field(pred) => eval_pred(&|name| type_held(ctx, name, ord), pred, obj),
        Expr::Related(rel) => expand::related_holds(ctx, ord, rel),
    }
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

/// Normalize a property/label name to the engine's canonical form.
#[must_use]
pub fn normalize(name: &str) -> String {
    rushdb_types::value::normalize_name(name)
}

fn resolve_field(obj: &Map<String, Json>, field: &str) -> Option<Json> {
    let field = field.strip_prefix("$record.").unwrap_or(field);
    // A row keyed by the request's own spelling (point-read rows, or a
    // record whose stored name is the request's) answers without
    // normalizing every key.
    if let Some(value) = obj.get(field) {
        return Some(value.clone());
    }
    let norm = normalize(field);
    obj.iter()
        .find(|(k, _)| normalize(k) == norm)
        .map(|(_, v)| v.clone())
}

#[cfg(test)]
mod index_planner_tests;
#[cfg(test)]
mod order_tests;
