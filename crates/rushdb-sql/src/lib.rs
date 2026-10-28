//! Relational SQL frontend backed by Apache `DataFusion`.
//!
//! Each execution exposes a tenant-scoped, immutable snapshot of LMPG records
//! as Arrow tables. Record labels are tables in the default schema and
//! relationship types are tables in the `rushdb_edges` schema. `DataFusion` owns
//! SQL parsing, planning, optimization, and execution; this crate only adapts
//! `RushDB`'s storage representation to Arrow.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use arrow_json::writer::{JsonArray, WriterBuilder};
use datafusion::common::ScalarValue;
use datafusion::execution::context::{SessionConfig, SessionContext};
use datafusion::execution::memory_pool::MemoryPool as _;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::sql::parser::{DFParser, Statement as DFStatement};
use datafusion::sql::sqlparser::ast::{ObjectName, Statement, TableFactor};
use datafusion::sql::sqlparser::dialect::PostgreSqlDialect;
use futures::StreamExt;
use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::{CatalogAccess, Lmpg};
use rushdb_storage::{resources::ByteReservation, ReadView, StorageError};
use rushdb_textsearch::TextIndexes;
use rushdb_types::keys::{key, P_REL_BY_ORD};
use rushdb_types::value::{ID_FIELD, LABEL_FIELD};
use serde_json::Value as Json;

/// A single query result row: column name to JSON value, in the
/// statement's column order.
pub type Row = serde_json::Map<String, Json>;

mod checked_sum;
mod dml;
mod edge_provider;
mod lazy_catalog;
mod scan_provider;
mod text_search;

fn ensure_sync_context() -> Result<(), SqlError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(SqlError::BlockingInRuntime);
    }
    Ok(())
}

/// Work counters emitted by the bounded `DataFusion` label scan. They are
/// request-local so a transport can attribute candidate and reconstruction
/// work without introducing a global metric label for every tenant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SqlWorkCounters {
    /// Label-membership candidates visited.
    pub candidate_rows: u64,
    /// Canonical records reconstructed from selected properties.
    pub reconstructed_rows: u64,
    /// Candidates rejected by a pushed predicate.
    pub filtered_rows: u64,
    /// Membership posting bytes read from the bounded cursor.
    pub posting_bytes: u64,
}

thread_local! {
    static SQL_WORK: RefCell<SqlWorkCounters> = RefCell::new(SqlWorkCounters::default());
}

/// Reset request-local SQL scan counters before execution.
pub fn reset_work_counters() {
    SQL_WORK.with(|work| *work.borrow_mut() = SqlWorkCounters::default());
}

/// Take and reset request-local SQL scan counters after execution.
#[must_use]
pub fn take_work_counters() -> SqlWorkCounters {
    SQL_WORK.with(|work| std::mem::take(&mut *work.borrow_mut()))
}

pub(crate) fn record_work(update: impl FnOnce(&mut SqlWorkCounters)) {
    SQL_WORK.with(|work| update(&mut work.borrow_mut()));
}

#[cfg(test)]
mod pinned_tests;

/// Rows a SQL result, an eager mutation table or a relationship table may
/// hold, so an accidental unbounded query cannot build an unbounded
/// in-memory Arrow result. Label reads stream and never count against it
pub const MAX_SCAN_ROWS: usize = 100_000;
/// Small execution-pool allowance for `DataFusion`'s planner/operator state.
/// Retained result and Arrow-batch bytes are still checked against the exact
/// caller budget below; without this allowance even a one-byte result budget
/// fails while registering the schema before result admission runs.
const DATAFUSION_PHASE_OVERHEAD: usize = 16 * 1024 * 1024;

/// Wraps a [`datafusion::execution::memory_pool::GreedyMemoryPool`] (`DataFusion`'s
/// own enforcement, unchanged — `try_grow` still rejects over the real
/// `max_result_bytes` limit) while tracking the high-water mark of
/// `reserved()` across the query's execution. `MemoryPool` itself exposes
/// only the current `reserved()`, not a peak, so this is the minimal
/// wrapper needed to make `DataFusion`'s *real* memory use observable
/// (a measured number once a query has run, instead of a flat guess).
#[derive(Debug)]
struct PeakTrackingMemoryPool {
    // `TrackConsumersPool` wrapping `GreedyMemoryPool` is exactly what
    // `RuntimeEnvBuilder::with_memory_limit` built before this change (same
    // top-5 "largest consumers" detail in a resource-exhausted error
    // message); wrapping it here (instead of the other way around) keeps
    // that error-message behavior while still letting this struct's own
    // `Arc` handle be queried for `peak()` after the `RuntimeEnv` has taken
    // ownership of it as a trait object.
    inner: datafusion::execution::memory_pool::TrackConsumersPool<
        datafusion::execution::memory_pool::GreedyMemoryPool,
    >,
    peak: std::sync::atomic::AtomicUsize,
}

impl PeakTrackingMemoryPool {
    fn new(limit: usize) -> Self {
        Self {
            inner: datafusion::execution::memory_pool::TrackConsumersPool::new(
                datafusion::execution::memory_pool::GreedyMemoryPool::new(limit),
                std::num::NonZeroUsize::new(5).expect("5 is nonzero"),
            ),
            peak: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn peak(&self) -> usize {
        self.peak.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn observe(&self) {
        self.peak
            .fetch_max(self.inner.reserved(), std::sync::atomic::Ordering::Relaxed);
    }
}

impl std::fmt::Display for PeakTrackingMemoryPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl datafusion::execution::memory_pool::MemoryPool for PeakTrackingMemoryPool {
    fn name(&self) -> &'static str {
        "rushdb-peak-tracking"
    }

    fn register(&self, consumer: &datafusion::execution::memory_pool::MemoryConsumer) {
        self.inner.register(consumer);
    }

    fn unregister(&self, consumer: &datafusion::execution::memory_pool::MemoryConsumer) {
        self.inner.unregister(consumer);
    }

    fn grow(
        &self,
        reservation: &datafusion::execution::memory_pool::MemoryReservation,
        additional: usize,
    ) {
        self.inner.grow(reservation, additional);
        self.observe();
    }

    fn shrink(
        &self,
        reservation: &datafusion::execution::memory_pool::MemoryReservation,
        shrink: usize,
    ) {
        self.inner.shrink(reservation, shrink);
    }

    fn try_grow(
        &self,
        reservation: &datafusion::execution::memory_pool::MemoryReservation,
        additional: usize,
    ) -> datafusion::error::Result<()> {
        self.inner.try_grow(reservation, additional)?;
        self.observe();
        Ok(())
    }

    fn reserved(&self) -> usize {
        self.inner.reserved()
    }

    fn memory_limit(&self) -> datafusion::execution::memory_pool::MemoryLimit {
        self.inner.memory_limit()
    }
}

/// Errors produced while parsing, planning, or executing SQL over LMPG.
#[derive(Debug, thiserror::Error)]
pub enum SqlError {
    /// A synchronous entry point was called from a Tokio runtime thread.
    #[error("synchronous SQL cannot run inside Tokio; use the async SQL API")]
    BlockingInRuntime,
    /// Reading the pinned storage snapshot failed.
    #[error("SQL storage read failed: {0}")]
    Storage(#[from] StorageError),
    /// The SQL text could not be parsed or bound.
    #[error("SQL parse failed: {0}")]
    Parse(String),
    /// A write statement was submitted to the read-only query path.
    #[error("SQL statement is not read-only: {0}")]
    ReadOnly(String),
    /// The statement is outside the supported atomic DML subset.
    #[error("SQL mutation is unsupported: {0}")]
    UnsupportedMutation(String),
    /// A row's stable `__id` is missing, malformed, or changed unexpectedly.
    #[error("SQL row identity is invalid: {0}")]
    InvalidIdentity(String),
    /// Another commit wrote the tenant after the statement's snapshot; the
    /// statement can be retried.
    #[error("SQL write conflict: {0}")]
    WriteConflict(String),
    /// Publishing the computed delta to LMPG failed.
    #[error("SQL mutation failed to commit atomically: {0}")]
    Commit(String),
    /// The snapshot exceeded the [`MAX_SCAN_ROWS`] safety limit.
    #[error("SQL snapshot exceeds the {MAX_SCAN_ROWS} row safety limit: {0}")]
    ResourceLimit(usize),
    /// A filter without a usable index would check more records of a label
    /// than the unindexed-walk limit allows (SQL-2, QRY-3; like records
    /// reads).
    #[error("unindexed walk refused: {0}")]
    UnindexedWalk(String),
    /// The statement needed more working memory than `query-memory-max`,
    /// tightened by the request, allows.
    #[error("query working set exceeded: the SQL statement needed more than {0} bytes")]
    MemoryLimit(usize),
    /// The caller cancelled or the deadline elapsed during execution. Polled
    /// between result batches, and by label scans between their batches, so
    /// a long scan stops promptly.
    #[error("SQL execution cancelled")]
    Cancelled,
    /// The `DataFusion` engine reported a planning or execution failure.
    #[error("SQL engine failed: {0}")]
    Engine(String),
    /// A `text_search(...)` call was invalid or its search failed (unknown
    /// index, bad arguments, unparsable query).
    #[error("SQL text search failed: {0}")]
    TextSearch(String),
}

/// Tenant-visible relational catalog returned by [`virtual_catalog`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSnapshot {
    /// Record labels exposed as tables in the default schema, lowercased.
    pub tables: Vec<String>,
    /// Relationship types exposed in the `rushdb_edges` schema, lowercased.
    pub relationship_tables: Vec<String>,
}

/// Logical SQL type used in column and parameter descriptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlType {
    /// Boolean value.
    Boolean,
    /// 16-bit signed integer.
    Int16,
    /// 32-bit signed integer.
    Int32,
    /// 64-bit signed integer.
    Int64,
    /// 16-bit unsigned integer.
    UInt16,
    /// 32-bit unsigned integer.
    UInt32,
    /// 64-bit unsigned integer.
    UInt64,
    /// 32-bit floating-point value.
    Float32,
    /// 64-bit floating-point value.
    Float64,
    /// UTF-8 text.
    Utf8,
    /// Variable-length binary data.
    Binary,
    /// Calendar date.
    Date,
    /// Time of day or duration.
    Time,
    /// Date and time with optional time zone.
    Timestamp,
    /// Nested list, struct, map, or union value.
    Json,
    /// Type could not be mapped to a supported logical SQL type.
    Unknown,
}

/// One result column described by [`describe_sql`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlColumn {
    /// Column name.
    pub name: String,
    /// Logical SQL type of the column.
    pub data_type: SqlType,
    /// Whether the column may contain null values.
    pub nullable: bool,
}

/// Parsed description of a statement's bound parameters and result columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlDescription {
    /// Ordered by positional placeholder number, then named placeholder.
    pub parameters: Vec<(String, SqlType)>,
    /// Result columns, empty when the statement returns no rows.
    pub columns: Vec<SqlColumn>,
}

/// Result of executing one atomic DML statement.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlMutationResult {
    /// Rows produced by the `DataFusion` DML plan, typically the affected count.
    pub rows: Vec<Row>,
    /// Number of rows affected by the mutation.
    pub affected: u64,
    /// LMPG commit index published for the delta, or the snapshot version when
    /// the statement changed nothing.
    pub commit_index: u64,
}

/// Return the tenant-visible relational catalog without executing a query.
///
/// # Errors
///
/// Returns [`SqlError::Storage`] if the pinned catalog cannot be read.
pub fn virtual_catalog(lmpg: &Lmpg) -> Result<CatalogSnapshot, SqlError> {
    let snapshot = lmpg.store.read_view()?;
    let tables = access::labels(&snapshot, lmpg.tenant)?
        .into_iter()
        .map(|label| label.to_ascii_lowercase())
        .collect();
    let relationship_tables = relationship_types(lmpg, &snapshot)?
        .into_iter()
        .map(|rel_type| rel_type.to_ascii_lowercase())
        .collect();
    Ok(CatalogSnapshot {
        tables,
        relationship_tables,
    })
}

/// Execute a read-only SQL query and return its result rows.
///
/// # Errors
///
/// Returns [`SqlError::Parse`] for invalid SQL, [`SqlError::ReadOnly`] for a
/// write statement, [`SqlError::Storage`] for a storage read failure, or
/// [`SqlError::Engine`] for a `DataFusion` planning or execution failure.
pub fn execute_sql(lmpg: &Lmpg, sql: &str) -> Result<Vec<Row>, SqlError> {
    execute_sql_with_parameters(lmpg, sql, None)
}

/// Execute a read-only SQL query with optional bound parameters.
///
/// # Errors
///
/// Returns the same errors as [`execute_sql`], plus [`SqlError::Parse`] when
/// `parameters` is neither a JSON array nor object, or a bound value cannot be
/// converted to a supported scalar.
pub fn execute_sql_with_parameters(
    lmpg: &Lmpg,
    sql: &str,
    parameters: Option<&Json>,
) -> Result<Vec<Row>, SqlError> {
    execute_sql_with_parameters_budget(lmpg, sql, parameters, None)
}

/// Execute SQL with an optional cumulative response working-byte budget.
/// The budget is enforced while decoding `DataFusion` batches, before rows are
/// retained in the result vector.
///
/// # Errors
/// Returns the same errors as [`execute_sql_with_parameters`], including a
/// resource-limit error when the result exceeds `max_result_bytes`.
pub fn execute_sql_with_parameters_budget(
    lmpg: &Lmpg,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
) -> Result<Vec<Row>, SqlError> {
    ensure_sync_context()?;
    futures::executor::block_on(execute_sql_async_with_budget(
        lmpg,
        sql,
        parameters,
        max_result_bytes,
    ))
}

/// Execute SQL against a caller-owned pinned read view.
///
/// This is the engine transport path: the adapter must not open a second
/// snapshot while the surrounding request is using its execution context.
///
/// # Errors
/// Returns the same errors as [`execute_sql_with_parameters_budget`].
pub fn execute_sql_with_parameters_budget_in_view(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
) -> Result<Vec<Row>, SqlError> {
    execute_sql_with_parameters_budget_in_view_cancellable(
        lmpg,
        snapshot,
        sql,
        parameters,
        max_result_bytes,
        &|| false,
        None,
    )
}

/// Cancellation-aware form of [`execute_sql_with_parameters_budget_in_view`].
/// `is_cancelled` is polled between result batches so a long analytical scan
/// stops promptly instead of only at the engine/transport boundary. Returns
/// [`SqlError::Cancelled`] when the predicate reports cancellation.
///
/// # Errors
///
/// Returns the same errors as [`execute_sql_with_parameters_budget_in_view`],
/// plus [`SqlError::Cancelled`].
pub fn execute_sql_with_parameters_budget_in_view_cancellable(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
    is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    text: Option<&Arc<TextIndexes>>,
) -> Result<Vec<Row>, SqlError> {
    ensure_sync_context()?;
    futures::executor::block_on(execute_sql_async_with_budget_in_view_cancellable(
        lmpg,
        snapshot,
        sql,
        parameters,
        max_result_bytes,
        is_cancelled,
        text,
    ))
}

/// Async form of [`execute_sql_with_parameters`].
///
/// # Errors
///
/// Returns the same errors as [`execute_sql_with_parameters`].
pub async fn execute_sql_async(
    lmpg: &Lmpg,
    sql: &str,
    parameters: Option<&Json>,
) -> Result<Vec<Row>, SqlError> {
    execute_sql_async_with_budget(lmpg, sql, parameters, None).await
}

/// Async form of [`execute_sql_with_parameters_budget`].
///
/// # Errors
/// Returns the same errors as [`execute_sql_with_parameters_budget`].
pub async fn execute_sql_async_with_budget(
    lmpg: &Lmpg,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
) -> Result<Vec<Row>, SqlError> {
    validate_read_only(sql)?;
    let snapshot = Arc::new(lmpg.store.read_view()?);
    execute_sql_async_with_budget_in_view(lmpg, snapshot, sql, parameters, max_result_bytes).await
}

/// Async form of [`execute_sql_with_parameters_budget_in_view`].
///
/// # Errors
/// Returns the same errors as [`execute_sql_with_parameters_budget_in_view`].
pub async fn execute_sql_async_with_budget_in_view(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
) -> Result<Vec<Row>, SqlError> {
    execute_sql_async_with_budget_in_view_cancellable(
        lmpg,
        snapshot,
        sql,
        parameters,
        max_result_bytes,
        &|| false,
        None,
    )
    .await
}

/// Cancellation-aware async form of
/// [`execute_sql_async_with_budget_in_view`].
///
/// # Errors
///
/// Returns the same errors as [`execute_sql_async_with_budget_in_view`], plus
/// [`SqlError::Cancelled`].
pub async fn execute_sql_async_with_budget_in_view_cancellable(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
    is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    text: Option<&Arc<TextIndexes>>,
) -> Result<Vec<Row>, SqlError> {
    execute_read(
        lmpg,
        snapshot,
        sql,
        parameters,
        max_result_bytes,
        is_cancelled,
        text,
        &CatalogAccess::default(),
    )
    .await
}

/// Execute read-only SQL under a tenant's data-access policy: the
/// statement sees tables for the policy's allowed labels only, columns for
/// their visible properties only, and edge tables for allowed relationship
/// types holding only relationships between allowed labels. A hidden table
/// or column fails like one that does not exist. A policy that is not
/// enforced reads like [`execute_sql_with_parameters_budget_in_view_cancellable`].
///
/// # Errors
///
/// Returns the same errors as
/// [`execute_sql_with_parameters_budget_in_view_cancellable`].
#[expect(
    clippy::too_many_arguments,
    reason = "the cancellable read entry point plus the policy"
)]
pub fn execute_sql_under_policy_in_view(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
    is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    text: Option<&Arc<TextIndexes>>,
    policy: Arc<rushdb_types::security::ActivePolicy>,
) -> Result<Vec<Row>, SqlError> {
    ensure_sync_context()?;
    futures::executor::block_on(execute_read(
        lmpg,
        snapshot,
        sql,
        parameters,
        max_result_bytes,
        is_cancelled,
        text,
        &CatalogAccess::new(Some(policy)),
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "the cancellable read entry point plus what the catalog shows"
)]
async fn execute_read(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
    is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    text: Option<&Arc<TextIndexes>>,
    visible: &CatalogAccess,
) -> Result<Vec<Row>, SqlError> {
    validate_read_only(sql)?;
    if is_cancelled() {
        return Err(SqlError::Cancelled);
    }

    let mut context = context_in_view_visible(lmpg, &snapshot, max_result_bytes, text, visible)?;

    let mut dataframe = context
        .context
        .sql(sql)
        .await
        .map_err(|error| with_table_hint(planning_error(error), &context.context))?;
    if let Some(parameters) = parameters {
        dataframe = match parameters {
            Json::Array(values) => dataframe
                .with_param_values(
                    values
                        .iter()
                        .map(json_to_scalar)
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .map_err(|error| SqlError::Engine(error.to_string()))?,
            Json::Object(values) => dataframe
                .with_param_values(
                    values
                        .iter()
                        .map(|(name, value)| Ok((name.clone(), json_to_scalar(value)?)))
                        .collect::<Result<Vec<_>, SqlError>>()?,
                )
                .map_err(|error| SqlError::Engine(error.to_string()))?,
            _ => {
                return Err(SqlError::Parse(
                    "SQL parameters must be a JSON array or object".into(),
                ))
            }
        };
    }
    let mut stream = dataframe
        .execute_stream()
        .await
        .map_err(|error| map_stream_error(&error, max_result_bytes, memory_cap(lmpg)))?;
    let mut rows = Vec::new();
    let mut retained_bytes = 0usize;
    while let Some(batch) = stream.next().await {
        if is_cancelled() {
            return Err(SqlError::Cancelled);
        }
        let batch =
            batch.map_err(|error| map_stream_error(&error, max_result_bytes, memory_cap(lmpg)))?;
        append_batch_rows_with_budget(&batch, &mut rows, &mut retained_bytes, max_result_bytes)?;
    }
    // Now that every batch is drained, DataFusion's own
    // memory pool has reached its final peak for this query — resize the
    // phase reservation to that real number instead of the flat estimate.
    context.record_real_memory_peak();
    Ok(rows)
}

/// A statement that failed to plan: a failed `text_search` call keeps its
/// own class, anything else is the engine's.
#[expect(
    clippy::needless_pass_by_value,
    reason = "used directly as a `map_err` adapter, which supplies the owned error value"
)]
fn planning_error(error: datafusion::error::DataFusionError) -> SqlError {
    if let datafusion::error::DataFusionError::External(inner) = error.find_root() {
        if let Some(failed) = inner.downcast_ref::<text_search::TextSearchError>() {
            return SqlError::TextSearch(failed.to_string());
        }
    }
    scan_error(&error).unwrap_or_else(|| SqlError::Engine(error.to_string()))
}

/// A "table not found" planning failure, with the closest table names of
/// the schema it was looked up in. `DataFusion` already suggests
/// close column names; it does not for tables.
fn with_table_hint(error: SqlError, context: &SessionContext) -> SqlError {
    let SqlError::Engine(message) = &error else {
        return error;
    };
    let Some(name) = message
        .split_once("table '")
        .and_then(|(_, rest)| rest.split_once("' not found"))
        .map(|(name, _)| name)
    else {
        return error;
    };
    let mut parts = name.rsplitn(2, '.');
    let wanted = parts.next().unwrap_or(name);
    let schema_name = parts
        .next()
        .and_then(|qualifier| qualifier.rsplit('.').next())
        .unwrap_or("public");
    let Some(schema) = context
        .catalog("datafusion")
        .and_then(|catalog| catalog.schema(schema_name))
    else {
        return error;
    };
    let tables = schema.table_names();
    let closest = rushdb_types::names::closest(wanted, tables.iter().map(String::as_str), 3);
    if closest.is_empty() {
        return error;
    }
    let quoted: Vec<String> = closest.iter().map(|table| format!("'{table}'")).collect();
    SqlError::Engine(format!("{message}. Did you mean {}?", quoted.join(" or ")))
}

/// A label scan refused or stopped like a records read (SQL-2, QRY-3)
/// keeps its class.
fn scan_error(error: &datafusion::error::DataFusionError) -> Option<SqlError> {
    let datafusion::error::DataFusionError::External(inner) = error.find_root() else {
        return None;
    };
    match inner.downcast_ref::<rushdb_query::QueryError>()? {
        rushdb_query::QueryError::UnindexedWalk(message) => {
            Some(SqlError::UnindexedWalk(message.clone()))
        }
        rushdb_query::QueryError::Cancelled => Some(SqlError::Cancelled),
        _ => None,
    }
}

fn map_stream_error(
    error: &datafusion::error::DataFusionError,
    max_result_bytes: Option<usize>,
    memory_cap: Option<usize>,
) -> SqlError {
    if let Some(stopped) = scan_error(error) {
        return stopped;
    }
    let message = error.to_string();
    // The pool's refusal, however an operator wrapped it (an external sort
    // reports "Not enough memory to continue external sort").
    if matches!(
        error.find_root(),
        datafusion::error::DataFusionError::ResourcesExhausted(_)
    ) {
        // The pool is the tighter of the result budget and the memory cap.
        match (max_result_bytes, memory_cap) {
            (budget, Some(cap))
                if budget.is_none_or(|budget| {
                    cap < budget.saturating_add(DATAFUSION_PHASE_OVERHEAD)
                }) =>
            {
                return SqlError::MemoryLimit(cap);
            }
            (Some(budget), _) => return SqlError::ResourceLimit(budget),
            _ => {}
        }
    }
    SqlError::Engine(message)
}

/// The working-memory cap of a statement run on `lmpg`: `query-memory-max`,
/// tightened by the request.
fn memory_cap(lmpg: &Lmpg) -> Option<usize> {
    lmpg.limits()
        .query_memory_max_bytes()
        .map(|bytes| usize::try_from(bytes).unwrap_or(usize::MAX))
}

/// Execute one relational DML statement against an isolated `DataFusion`
/// `MemTable` catalog and atomically publish the resulting row delta to LMPG.
/// `DataFusion` owns SQL parsing, binding, expressions, filters, and affected-row
/// semantics. `RushDB` only validates stable record identity and commits the
/// already-computed delta with optimistic snapshot protection.
///
/// # Errors
///
/// Returns [`SqlError::UnsupportedMutation`] when `sql` is not a single
/// supported `INSERT`, `UPDATE`, or `DELETE` against a record table,
/// [`SqlError::InvalidIdentity`] when the resulting delta breaks stable row
/// identity, and other [`SqlError`] variants for parse, storage, engine, or
/// commit failures.
pub fn execute_sql_mutation(
    lmpg: &Lmpg,
    sql: &str,
    parameters: Option<&Json>,
) -> Result<SqlMutationResult, SqlError> {
    ensure_sync_context()?;
    futures::executor::block_on(execute_sql_mutation_async(lmpg, sql, parameters))
}

/// Async form of [`execute_sql_mutation`].
///
/// # Errors
///
/// Returns the same errors as [`execute_sql_mutation`].
///
/// # Panics
///
/// Panics if the validated DML target is absent from the mutable catalog; this
/// cannot occur because the target is checked immediately before the lookup.
pub async fn execute_sql_mutation_async(
    lmpg: &Lmpg,
    sql: &str,
    parameters: Option<&Json>,
) -> Result<SqlMutationResult, SqlError> {
    let snapshot = Arc::new(lmpg.store.read_view()?);
    execute_sql_mutation_async_in_view(lmpg, snapshot, sql, parameters, None, None).await
}

/// Execute a SQL mutation against a caller-owned pinned view and cumulative
/// result budget. The commit remains optimistic against that pinned version.
///
/// # Errors
/// Returns the same parse, storage, resource, identity, and commit errors as
/// [`execute_sql_mutation`].
pub fn execute_sql_mutation_in_view_with_budget(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
    text: Option<&Arc<TextIndexes>>,
) -> Result<SqlMutationResult, SqlError> {
    ensure_sync_context()?;
    futures::executor::block_on(execute_sql_mutation_async_in_view(
        lmpg,
        snapshot,
        sql,
        parameters,
        max_result_bytes,
        text,
    ))
}

/// Async form of [`execute_sql_mutation_in_view_with_budget`].
///
/// # Errors
/// Returns the same errors as [`execute_sql_mutation_in_view_with_budget`].
pub async fn execute_sql_mutation_async_in_view(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
    text: Option<&Arc<TextIndexes>>,
) -> Result<SqlMutationResult, SqlError> {
    dml::execute(lmpg, snapshot, sql, parameters, max_result_bytes, text).await
}

/// Describe a read-only statement's parameter types and result columns.
///
/// # Errors
///
/// Returns the same errors as [`execute_sql`].
pub fn describe_sql(lmpg: &Lmpg, sql: &str) -> Result<SqlDescription, SqlError> {
    ensure_sync_context()?;
    futures::executor::block_on(describe_sql_async(lmpg, sql, None))
}

/// Describe parameter types for the supported atomic DML subset. Pgwire uses
/// this during Parse so standard PostgreSQL clients can encode bound values
/// before the statement is executed. It does not make DML a read query and it
/// intentionally exposes no result columns.
///
/// # Errors
///
/// Returns [`SqlError::UnsupportedMutation`] when `sql` is not a supported DML
/// statement, and other [`SqlError`] variants for storage or engine failures.
pub fn describe_sql_mutation(lmpg: &Lmpg, sql: &str) -> Result<SqlDescription, SqlError> {
    ensure_sync_context()?;
    futures::executor::block_on(describe_sql_mutation_async(lmpg, sql, None))
}

/// Async form of [`describe_sql_mutation`].
///
/// # Errors
///
/// Returns the same errors as [`describe_sql_mutation`].
pub async fn describe_sql_mutation_async(
    lmpg: &Lmpg,
    sql: &str,
    text: Option<&Arc<TextIndexes>>,
) -> Result<SqlDescription, SqlError> {
    let plan = dml::plan(sql)?;
    let snapshot = Arc::new(lmpg.store.read_view()?);
    dml::target_label(lmpg, &snapshot, &plan.target)?;
    let context = context_in_view_with_budget(lmpg, &snapshot, None, text)?;
    let dataframe = context
        .context
        .sql(&plan.query)
        .await
        .map_err(|error| with_table_hint(planning_error(error), &context.context))?;
    let mut parameters = dataframe
        .logical_plan()
        .get_parameter_types()
        .map_err(|error| SqlError::Engine(error.to_string()))?
        .into_iter()
        .map(|(name, data_type)| {
            (
                name,
                data_type
                    .as_ref()
                    .map_or(SqlType::Unknown, sql_type_from_arrow),
            )
        })
        .collect::<Vec<_>>();
    parameters.sort_by(|(left, _), (right, _)| parameter_order(left).cmp(&parameter_order(right)));
    // The rewritten read carries only values (`VALUES ($1, $2)`), so a bare
    // parameter has no type of its own. Resolve it from the target column, as
    // PostgreSQL does for INSERT/UPDATE.
    if !plan.parameter_columns.is_empty() {
        if let Ok(provider) = context.context.table_provider(&plan.target).await {
            let schema = provider.schema();
            for (index, target) in &plan.parameter_columns {
                let field = match target {
                    dml::ParameterTarget::Column(name) => schema.field_with_name(name).ok(),
                    dml::ParameterTarget::Position(position) => schema
                        .fields()
                        .get(*position)
                        .map(std::convert::AsRef::as_ref),
                };
                let Some(field) = field else { continue };
                let Some((_, data_type)) = parameters.iter_mut().find(|(name, _)| {
                    name.strip_prefix('$')
                        .and_then(|value| value.parse::<u64>().ok())
                        == Some(*index)
                }) else {
                    continue;
                };
                *data_type = sql_type_from_arrow(field.data_type());
            }
        }
    }
    Ok(SqlDescription {
        parameters,
        columns: Vec::new(),
    })
}

/// Async form of [`describe_sql`], describing a statement without executing it.
///
/// # Errors
///
/// Returns the same errors as [`describe_sql`].
pub async fn describe_sql_async(
    lmpg: &Lmpg,
    sql: &str,
    text: Option<&Arc<TextIndexes>>,
) -> Result<SqlDescription, SqlError> {
    validate_read_only(sql)?;
    let context = context_for_lmpg(lmpg, text)?;
    let dataframe = context
        .sql(sql)
        .await
        .map_err(|error| with_table_hint(planning_error(error), &context))?;

    let mut parameters = dataframe
        .logical_plan()
        .get_parameter_types()
        .map_err(|error| SqlError::Engine(error.to_string()))?
        .into_iter()
        .map(|(name, data_type)| {
            let data_type = data_type
                .as_ref()
                .map_or(SqlType::Unknown, sql_type_from_arrow);
            (name, data_type)
        })
        .collect::<Vec<_>>();
    parameters.sort_by(|(left, _), (right, _)| parameter_order(left).cmp(&parameter_order(right)));

    let columns = dataframe
        .schema()
        .iter()
        .map(|(_, field)| SqlColumn {
            name: field.name().clone(),
            data_type: sql_type_from_arrow(field.data_type()),
            nullable: field.is_nullable(),
        })
        .collect();
    Ok(SqlDescription {
        parameters,
        columns,
    })
}

fn context_for_lmpg(
    lmpg: &Lmpg,
    text: Option<&Arc<TextIndexes>>,
) -> Result<SessionContext, SqlError> {
    let snapshot = Arc::new(lmpg.store.read_view()?);
    Ok(context_in_view(lmpg, &snapshot, text)?.context)
}

struct DataFusionContext {
    context: SessionContext,
    // Keeps the shared store-level phase reservation alive for the complete
    // DataFusion query, including planning and stream execution. Resized to
    // the real measured peak (`record_real_memory_peak`) once execution
    // finishes, so a dropped-but-not-yet-resized reservation still charges
    // the conservative flat estimate rather than nothing.
    phase_reservation: Option<ByteReservation>,
    // `DataFusion`'s real memory-pool peak for this query, when a budget
    // was configured (`None` otherwise — an unbounded context has no
    // `GreedyMemoryPool` to track).
    peak_pool: Option<Arc<PeakTrackingMemoryPool>>,
}

impl DataFusionContext {
    /// Resize the phase reservation to `DataFusion`'s real measured peak for
    /// this query (follow-up), replacing the conservative
    /// `DATAFUSION_PHASE_OVERHEAD` flat estimate the reservation started
    /// with. Call once execution has fully drained the result stream — the
    /// peak only reaches its final value once every operator has run.
    /// Best-effort: a resize failure only means the budget's `peak` gauge
    /// under-reports a query that already completed successfully, so it is
    /// silently ignored rather than propagated.
    fn record_real_memory_peak(&mut self) {
        let (Some(reservation), Some(pool)) = (self.phase_reservation.as_mut(), &self.peak_pool)
        else {
            return;
        };
        let real = pool.peak().max(DATAFUSION_PHASE_OVERHEAD);
        let _ = reservation.resize(real);
    }
}

fn context_in_view(
    lmpg: &Lmpg,
    snapshot: &Arc<ReadView>,
    text: Option<&Arc<TextIndexes>>,
) -> Result<DataFusionContext, SqlError> {
    context_in_view_with_budget(lmpg, snapshot, None, text)
}

fn context_in_view_with_budget(
    lmpg: &Lmpg,
    snapshot: &Arc<ReadView>,
    max_result_bytes: Option<usize>,
    text: Option<&Arc<TextIndexes>>,
) -> Result<DataFusionContext, SqlError> {
    context_in_view_visible(
        lmpg,
        snapshot,
        max_result_bytes,
        text,
        &CatalogAccess::default(),
    )
}

/// A statement context whose catalog shows only what `visible` allows
///. Text search is not offered under an enforced policy: its
/// indexes are not policy-scoped.
fn context_in_view_visible(
    lmpg: &Lmpg,
    snapshot: &Arc<ReadView>,
    max_result_bytes: Option<usize>,
    text: Option<&Arc<TextIndexes>>,
    visible: &CatalogAccess,
) -> Result<DataFusionContext, SqlError> {
    let config = SessionConfig::new()
        .with_information_schema(true)
        .with_target_partitions(1);
    let (context, phase_reservation, peak_pool) =
        session_context(&lmpg.store, config, max_result_bytes, memory_cap(lmpg))?;
    // Lazy catalog: labels and edge types are materialized only when the
    // statement actually resolves them, instead of reconstructing the whole
    // tenant on every query.
    register_lazy_catalog_visible(&context, lmpg, snapshot, visible)?;
    if !visible.enforced() {
        text_search::install(&context, lmpg, snapshot, text);
    }
    Ok(DataFusionContext {
        context,
        phase_reservation,
        peak_pool,
    })
}

#[cfg(test)]
fn register_lazy_catalog(
    context: &SessionContext,
    lmpg: &Lmpg,
    snapshot: &Arc<ReadView>,
) -> Result<(), SqlError> {
    register_lazy_catalog_visible(context, lmpg, snapshot, &CatalogAccess::default())
}

fn register_lazy_catalog_visible(
    context: &SessionContext,
    lmpg: &Lmpg,
    snapshot: &Arc<ReadView>,
    visible: &CatalogAccess,
) -> Result<(), SqlError> {
    let tenant = lmpg.tenant;
    let catalog = context
        .catalog("datafusion")
        .ok_or_else(|| SqlError::Engine("DataFusion default catalog is unavailable".into()))?;
    let labels = lazy_catalog::LazyLabelSchema::new(lmpg, Arc::clone(snapshot), visible.clone())?;
    catalog
        .register_schema("public", Arc::new(labels))
        .map_err(|error| SqlError::Engine(error.to_string()))?;
    let edges =
        lazy_catalog::LazyRelationshipSchema::new(tenant, Arc::clone(snapshot), visible.clone());
    catalog
        .register_schema("rushdb_edges", Arc::new(edges))
        .map_err(|error| SqlError::Engine(error.to_string()))?;
    Ok(())
}

/// `(execution context, phase reservation, real-peak-tracking pool)` — the
/// latter two are `None` together (unbounded context, nothing to track) or
/// `Some` together (budgeted context).
type SessionContextParts = (
    SessionContext,
    Option<ByteReservation>,
    Option<Arc<PeakTrackingMemoryPool>>,
);

fn session_context(
    store: &rushdb_storage::Store,
    config: SessionConfig,
    max_result_bytes: Option<usize>,
    memory_cap: Option<usize>,
) -> Result<SessionContextParts, SqlError> {
    // A named child of the shared phase budget, so SQL's own
    // `peak`/`used` is individually observable.
    let phase_reservation = if let Some(limit) = max_result_bytes {
        store
            .sql_phase_budget()
            .map(|budget| {
                budget
                    .try_reserve(DATAFUSION_PHASE_OVERHEAD)
                    .map_err(|_| SqlError::ResourceLimit(limit))
            })
            .transpose()?
    } else {
        None
    };
    // The real admission gate: the result budget plus the phase overhead,
    // and never more than the statement's memory cap.
    let pool_limit = match (max_result_bytes, memory_cap) {
        (Some(limit), cap) => {
            let limit = limit.saturating_add(DATAFUSION_PHASE_OVERHEAD);
            Some(cap.map_or(limit, |cap| cap.min(limit)))
        }
        (None, cap) => cap,
    };
    let (context, peak_pool) = if let Some(limit) = pool_limit {
        // The pool additionally tracks its own high-water mark so the
        // reservation above can later be resized to the real measured peak
        // instead of staying at the flat overhead estimate.
        let pool = Arc::new(PeakTrackingMemoryPool::new(limit));
        let runtime = RuntimeEnvBuilder::new()
            .with_memory_pool(
                Arc::clone(&pool) as Arc<dyn datafusion::execution::memory_pool::MemoryPool>
            )
            .build()
            .map_err(|error| SqlError::Engine(error.to_string()))?;
        (
            SessionContext::new_with_config_rt(config, Arc::new(runtime)),
            Some(pool),
        )
    } else {
        (SessionContext::new_with_config(config), None)
    };
    // Q-10: register the overflow-checked integer SUM (built-in SUM(Int64)
    // wraps). Installed on every execution context so no query path misses it.
    checked_sum::install(&context);
    Ok((context, phase_reservation, peak_pool))
}

fn parameter_order(name: &str) -> (u8, u64, &str) {
    match name.strip_prefix('$').and_then(|value| value.parse().ok()) {
        Some(index) => (0, index, ""),
        None => (1, 0, name),
    }
}

fn bind_parameters(
    dataframe: datafusion::dataframe::DataFrame,
    parameters: &Json,
) -> Result<datafusion::dataframe::DataFrame, SqlError> {
    match parameters {
        Json::Array(values) => dataframe
            .with_param_values(
                values
                    .iter()
                    .map(json_to_scalar)
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .map_err(|error| SqlError::Engine(error.to_string())),
        Json::Object(values) => dataframe
            .with_param_values(
                values
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), json_to_scalar(value)?)))
                    .collect::<Result<Vec<_>, SqlError>>()?,
            )
            .map_err(|error| SqlError::Engine(error.to_string())),
        _ => Err(SqlError::Parse(
            "SQL parameters must be a JSON array or object".into(),
        )),
    }
}

fn sql_type_from_arrow(data_type: &DataType) -> SqlType {
    match data_type {
        DataType::Boolean => SqlType::Boolean,
        DataType::Int8 | DataType::Int16 => SqlType::Int16,
        DataType::Int32 => SqlType::Int32,
        DataType::Int64 => SqlType::Int64,
        DataType::UInt8 | DataType::UInt16 => SqlType::UInt16,
        DataType::UInt32 => SqlType::UInt32,
        DataType::UInt64 => SqlType::UInt64,
        DataType::Float16 | DataType::Float32 => SqlType::Float32,
        DataType::Float64
        | DataType::Decimal32(_, _)
        | DataType::Decimal64(_, _)
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _) => SqlType::Float64,
        DataType::Utf8 | DataType::Utf8View | DataType::LargeUtf8 => SqlType::Utf8,
        DataType::Binary
        | DataType::BinaryView
        | DataType::LargeBinary
        | DataType::FixedSizeBinary(_) => SqlType::Binary,
        DataType::Date32 | DataType::Date64 => SqlType::Date,
        DataType::Time32(_) | DataType::Time64(_) | DataType::Duration(_) => SqlType::Time,
        DataType::Timestamp(_, _) => SqlType::Timestamp,
        DataType::List(_)
        | DataType::ListView(_)
        | DataType::LargeList(_)
        | DataType::LargeListView(_)
        | DataType::FixedSizeList(_, _)
        | DataType::Struct(_)
        | DataType::Map(_, _)
        | DataType::Union(_, _) => SqlType::Json,
        _ => SqlType::Unknown,
    }
}

fn json_to_scalar(value: &Json) -> Result<ScalarValue, SqlError> {
    match value {
        Json::Null => Ok(ScalarValue::Null),
        Json::Bool(value) => Ok(ScalarValue::Boolean(Some(*value))),
        Json::Number(value) if value.is_i64() => Ok(ScalarValue::Int64(value.as_i64())),
        Json::Number(value) if value.is_u64() => Ok(ScalarValue::UInt64(value.as_u64())),
        Json::Number(value) => Ok(ScalarValue::Float64(value.as_f64())),
        Json::String(value) => Ok(ScalarValue::Utf8(Some(value.clone()))),
        Json::Array(values) => {
            let scalars = values
                .iter()
                .map(json_to_scalar)
                .collect::<Result<Vec<_>, _>>()?;
            let data_type = scalars
                .iter()
                .find(|value| !matches!(value, ScalarValue::Null))
                .map_or(DataType::Null, ScalarValue::data_type);
            if scalars
                .iter()
                .filter(|value| !matches!(value, ScalarValue::Null))
                .any(|value| value.data_type() != data_type)
            {
                return Err(SqlError::Parse(
                    "SQL array parameters must contain one logical type".into(),
                ));
            }
            Ok(ScalarValue::List(ScalarValue::new_list_nullable(
                &scalars, &data_type,
            )))
        }
        Json::Object(_) => Err(SqlError::Parse(
            "SQL object-valued parameters are not supported; bind scalar or array values".into(),
        )),
    }
}

fn validate_read_only(sql: &str) -> Result<(), SqlError> {
    let dialect = PostgreSqlDialect {};
    let statements = DFParser::parse_sql_with_dialect(sql, &dialect)
        .map_err(|error| SqlError::Parse(error.to_string()))?;
    if statements.is_empty() {
        return Err(SqlError::Parse("empty statement".into()));
    }

    for statement in statements {
        if !df_statement_is_read_only(&statement) {
            return Err(SqlError::ReadOnly(statement.to_string()));
        }
    }
    Ok(())
}

fn table_factor_name(table: &TableFactor) -> Result<String, SqlError> {
    match table {
        TableFactor::Table { name, args, .. } if args.is_none() => unqualified_table_name(name),
        _ => Err(SqlError::UnsupportedMutation(
            "DML target must be an unqualified record table".into(),
        )),
    }
}

fn unqualified_table_name(name: &ObjectName) -> Result<String, SqlError> {
    let [part] = name.0.as_slice() else {
        return Err(SqlError::UnsupportedMutation(
            "DML target must be an unqualified record table; relationship tables are read-only"
                .into(),
        ));
    };
    let ident = part.as_ident().ok_or_else(|| {
        SqlError::UnsupportedMutation("computed DML table names are unsupported".into())
    })?;
    Ok(ident.value.to_ascii_lowercase())
}

fn df_statement_is_read_only(statement: &DFStatement) -> bool {
    match statement {
        DFStatement::Statement(statement) => sql_statement_is_read_only(statement),
        DFStatement::Explain(explain) => df_statement_is_read_only(&explain.statement),
        DFStatement::CreateExternalTable(_) | DFStatement::CopyTo(_) | DFStatement::Reset(_) => {
            false
        }
    }
}

fn sql_statement_is_read_only(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::Query(_) | Statement::Explain { .. } | Statement::ExplainTable { .. }
    )
}

/// Distinct relationship types for a tenant, in stable order.
///
/// # Errors
/// Returns a storage or scan-limit error.
pub(crate) fn relationship_types_for_tenant(
    snapshot: &ReadView,
    tenant: rushdb_types::ids::TenantId,
) -> Result<BTreeSet<String>, SqlError> {
    // The maintained relationship catalog holds one row per shape, so the
    // types are read without walking the relationships.
    Ok(
        rushdb_lmpg::reader::try_access::relationship_catalog(snapshot, tenant)?
            .into_iter()
            .filter(|(_, _, _, count)| *count > 0)
            .map(|(rel_type, _, _, _)| rel_type)
            .collect(),
    )
}

pub(crate) fn externalize_record(value: Json) -> Json {
    let Json::Object(mut object) = value else {
        return value;
    };
    let id = object.remove(ID_FIELD).unwrap_or(Json::Null);
    object.remove(LABEL_FIELD);
    object.insert(ID_FIELD.into(), dashed_id(id));
    Json::Object(object)
}

/// A stored 32-hex record or relationship id in the wire form every
/// protocol surface uses: a dashed UUID.
pub(crate) fn dashed_id(id: Json) -> Json {
    match id.as_str().and_then(|raw| uuid::Uuid::parse_str(raw).ok()) {
        Some(uuid) => Json::String(uuid.hyphenated().to_string()),
        None => id,
    }
}

fn canonical_uuid(value: &str) -> Result<String, SqlError> {
    uuid::Uuid::parse_str(value)
        .map(|uuid| uuid.to_string())
        .map_err(|_| SqlError::InvalidIdentity(format!("`{value}` is not a UUID")))
}

#[cfg(test)]
fn batches_to_rows(batches: &[RecordBatch]) -> Result<Vec<Row>, SqlError> {
    batches_to_rows_with_budget(batches, None)
}

#[cfg(test)]
fn batches_to_rows_with_budget(
    batches: &[RecordBatch],
    max_result_bytes: Option<usize>,
) -> Result<Vec<Row>, SqlError> {
    if batches.is_empty() {
        return Ok(Vec::new());
    }
    // Decode one bounded DataFusion batch at a time. Keeping one JSON buffer
    // for the complete result made response memory scale with the full query
    // rather than the configured batch size.
    let mut rows = Vec::new();
    let mut retained_bytes = 0usize;
    for batch in batches {
        append_batch_rows_with_budget(batch, &mut rows, &mut retained_bytes, max_result_bytes)?;
    }
    Ok(rows)
}

fn append_batch_rows_with_budget(
    batch: &RecordBatch,
    rows: &mut Vec<Row>,
    retained_bytes: &mut usize,
    max_result_bytes: Option<usize>,
) -> Result<(), SqlError> {
    if rows.len().saturating_add(batch.num_rows()) > MAX_SCAN_ROWS {
        return Err(SqlError::ResourceLimit(
            rows.len().saturating_add(batch.num_rows()),
        ));
    }
    if let Some(limit) = max_result_bytes {
        let transient = batch.get_array_memory_size();
        if transient > limit.saturating_sub(*retained_bytes) {
            return Err(SqlError::ResourceLimit(
                retained_bytes.saturating_add(transient),
            ));
        }
    }
    let mut buffer = Vec::new();
    {
        let mut writer = WriterBuilder::new()
            .with_explicit_nulls(true)
            .build::<_, JsonArray>(&mut buffer);
        writer
            .write(batch)
            .map_err(|error| SqlError::Engine(error.to_string()))?;
        writer
            .finish()
            .map_err(|error| SqlError::Engine(error.to_string()))?;
    }
    if let Some(limit) = max_result_bytes {
        *retained_bytes = retained_bytes.saturating_add(buffer.len());
        if *retained_bytes > limit {
            return Err(SqlError::ResourceLimit(*retained_bytes));
        }
    }
    let values: Vec<Json> =
        serde_json::from_slice(&buffer).map_err(|error| SqlError::Engine(error.to_string()))?;
    for value in values {
        match value {
            Json::Object(object) => rows.push(object.into_iter().collect()),
            other => {
                return Err(SqlError::Engine(format!(
                    "DataFusion returned a non-object row: {other}"
                )))
            }
        }
    }
    Ok(())
}

fn relationship_types(lmpg: &Lmpg, snapshot: &ReadView) -> Result<BTreeSet<String>, SqlError> {
    let prefix = key(lmpg.tenant, &[P_REL_BY_ORD]);
    snapshot
        .range_prefix(&prefix)?
        .into_iter()
        .map(|(storage_key, _)| {
            let ordinal = ordinal_suffix(&storage_key, &prefix)?;
            Ok(access::relationship_by_ord(snapshot, lmpg.tenant, ordinal)?
                .ok_or(StorageError::CorruptLog)?
                .rel_type)
        })
        .collect()
}

fn ordinal_suffix(key: &[u8], prefix: &[u8]) -> Result<u64, StorageError> {
    if key.len() != prefix.len() + 9 || !key.starts_with(prefix) || key[prefix.len()] != b'/' {
        return Err(StorageError::CorruptLog);
    }
    Ok(u64::from_be_bytes(
        key[prefix.len() + 1..]
            .try_into()
            .map_err(|_| StorageError::CorruptLog)?,
    ))
}

/// Stable tenant-local fingerprint useful for prepared-query invalidation.
///
/// # Errors
///
/// Returns the same errors as [`virtual_catalog`].
pub fn catalog_fingerprint(lmpg: &Lmpg) -> Result<String, SqlError> {
    let catalog = virtual_catalog(lmpg)?;
    let mut hasher = blake3::Hasher::new();
    for table in catalog.tables {
        hasher.update(b"record\0");
        hasher.update(table.as_bytes());
        hasher.update(b"\0");
    }
    for table in catalog.relationship_tables {
        hasher.update(b"relationship\0");
        hasher.update(table.as_bytes());
        hasher.update(b"\0");
    }
    Ok(hasher.finalize().to_hex().to_string())
}
