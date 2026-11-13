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
mod distinct_topk;
mod dml;
mod edge_provider;
mod join_order;
mod late_materialization;
mod lazy_catalog;
mod lookup_join;
mod pg_catalog;
mod scan_provider;
mod session;
mod text_search;

pub use session::bind_session_functions;

fn ensure_sync_context() -> Result<(), SqlError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(SqlError::BlockingInRuntime);
    }
    Ok(())
}

thread_local! {
    /// The calling thread's Tokio runtime for the synchronous entry points,
    /// built on first use and kept for the thread's next statement.
    static RUNTIME: RefCell<Option<tokio::runtime::Runtime>> = const { RefCell::new(None) };
}

/// Run a statement to completion on the calling thread. `DataFusion`
/// spawns Tokio tasks for some operators (one per `UNION` input), which
/// panic outside a Tokio runtime, so the statement runs on a
/// current-thread runtime rather than a plain executor. A panic inside
/// the engine fails this statement and drops the runtime it ran on.
fn block_on<T>(
    statement: impl std::future::Future<Output = Result<T, SqlError>>,
) -> Result<T, SqlError> {
    ensure_sync_context()?;
    let runtime = match RUNTIME.with(RefCell::take) {
        Some(runtime) => runtime,
        None => tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|error| SqlError::Engine(format!("SQL runtime failed to start: {error}")))?,
    };
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.block_on(statement)));
    match outcome {
        Ok(result) => {
            RUNTIME.with(|slot| slot.replace(Some(runtime)));
            result
        }
        Err(panic) => {
            let detail = panic
                .downcast_ref::<&str>()
                .map(|detail| (*detail).to_owned())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            Err(SqlError::Engine(format!("SQL engine panicked: {detail}")))
        }
    }
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
/// The least a statement holds of the shared SQL budget from admission on:
/// planning and operator state before the first reservation. Admission
/// refuses a statement with [`SqlError::Busy`] when even this does not fit.
const DATAFUSION_PHASE_OVERHEAD: usize = 16 * 1024 * 1024;
/// A statement's working set when `query-memory-max` is unset (0): SQL never
/// runs unbounded.
const DEFAULT_WORKING_SET: usize = 256 * 1024 * 1024;

/// One statement's `DataFusion` memory pool. Every operator that buffers
/// (sorts, aggregates, joins, `RushDB`'s own top-k and lookup joins) reserves
/// here, and the pool enforces two bounds as it grows:
///
/// - the statement's cap (`query-memory-max`), through the inner
///   [`GreedyMemoryPool`](datafusion::execution::memory_pool::GreedyMemoryPool)
///   (refused as [`SqlError::MemoryLimit`]);
/// - the store's shared SQL budget, charged live by the statement's
///   reservation, so concurrent statements together stay within it
///   (refused as [`SqlError::Busy`], retryable).
struct StatementPool {
    // `TrackConsumersPool` names the largest consumers in a refusal.
    inner: datafusion::execution::memory_pool::TrackConsumersPool<
        datafusion::execution::memory_pool::GreedyMemoryPool,
    >,
    /// The statement's charge on the shared SQL budget, kept at
    /// `max(reserved, DATAFUSION_PHASE_OVERHEAD)`.
    shared: Option<std::sync::Mutex<ByteReservation>>,
    /// The size the shared budget refused, when it did.
    shared_refused: std::sync::atomic::AtomicUsize,
}

impl StatementPool {
    fn new(limit: usize, shared: Option<ByteReservation>) -> Self {
        Self {
            inner: datafusion::execution::memory_pool::TrackConsumersPool::new(
                datafusion::execution::memory_pool::GreedyMemoryPool::new(limit),
                std::num::NonZeroUsize::new(5).expect("5 is nonzero"),
            ),
            shared: shared.map(std::sync::Mutex::new),
            shared_refused: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Move the shared charge to what the pool holds now.
    fn charge_shared(&self) -> Result<(), usize> {
        let Some(shared) = &self.shared else {
            return Ok(());
        };
        let wanted = self.inner.reserved().max(DATAFUSION_PHASE_OVERHEAD);
        let mut shared = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if shared.bytes() == wanted {
            return Ok(());
        }
        shared.resize(wanted).map_err(|_| wanted)
    }

    /// The size the shared budget refused this statement, if it did.
    fn refused(&self) -> Option<usize> {
        match self
            .shared_refused
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            0 => None,
            bytes => Some(bytes),
        }
    }
}

impl std::fmt::Debug for StatementPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatementPool")
            .field("inner", &self.inner)
            .field("shared", &self.shared.is_some())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for StatementPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl datafusion::execution::memory_pool::MemoryPool for StatementPool {
    fn name(&self) -> &'static str {
        "rushdb-statement"
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
        // The infallible path reports memory an operator already holds: the
        // shared budget cannot refuse it, and any later `try_grow` of this
        // statement is checked against the total including it.
        self.inner.grow(reservation, additional);
        // Over the shared budget the charge stays where it was; the
        // statement's next `try_grow` charges the whole total or is refused.
        let _deferred = self.charge_shared();
    }

    fn shrink(
        &self,
        reservation: &datafusion::execution::memory_pool::MemoryReservation,
        shrink: usize,
    ) {
        self.inner.shrink(reservation, shrink);
        // Releasing only fails while an infallible grow is still uncharged;
        // the charge then stays where it was, as in `grow`.
        let _released = self.charge_shared();
    }

    fn try_grow(
        &self,
        reservation: &datafusion::execution::memory_pool::MemoryReservation,
        additional: usize,
    ) -> datafusion::error::Result<()> {
        self.inner.try_grow(reservation, additional)?;
        if let Err(wanted) = self.charge_shared() {
            self.inner.shrink(reservation, additional);
            self.shared_refused
                .store(wanted, std::sync::atomic::Ordering::Relaxed);
            return Err(datafusion::error::DataFusionError::ResourcesExhausted(
                format!(
                    "the shared SQL memory budget cannot hold {wanted} bytes for this statement"
                ),
            ));
        }
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
    /// The result (or an eager mutation or relationship table) would hold
    /// more than [`MAX_SCAN_ROWS`] rows.
    #[error(
        "SQL result exceeds the {MAX_SCAN_ROWS} row safety limit ({0} rows); add a LIMIT or narrow the query"
    )]
    RowLimit(usize),
    /// The rows returned would exceed the response size limit (the
    /// caller's result budget, in bytes).
    #[error(
        "response would exceed the response size limit ({0} bytes); raise --body-limit or add a LIMIT"
    )]
    ResultTooLarge(usize),
    /// The SQL working memory shared by concurrent statements is in use;
    /// the statement can be retried.
    #[error(
        "SQL working memory shared by concurrent statements is in use ({0} bytes requested); retry"
    )]
    Busy(usize),
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
    /// The statement names a table, column or function the catalog does
    /// not have.
    #[error("SQL planning failed: {0}")]
    Undefined(String),
    /// The statement does not plan for another reason of its own: an
    /// ambiguous name, operand types that do not combine.
    #[error("SQL planning failed: {0}")]
    Plan(String),
    /// The statement, or a feature it uses, is valid SQL this engine does
    /// not run (`PREPARE`, `LISTEN`, cursors, table DDL, an unsupported
    /// type).
    #[error("SQL statement is not supported: {0}")]
    Unsupported(String),
    /// A value the statement computes is invalid for its type: a text that
    /// does not cast to a number, a division by zero.
    #[error("SQL value is invalid: {0}")]
    InvalidValue(String),
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

impl SqlType {
    /// The object id of the PostgreSQL type a value of this type is sent as.
    #[must_use]
    pub fn pg_oid(&self) -> u32 {
        match self {
            SqlType::Boolean => 16,
            SqlType::Int16 | SqlType::UInt16 => 21,
            SqlType::Int32 | SqlType::UInt32 => 23,
            SqlType::Int64 | SqlType::UInt64 => 20,
            SqlType::Float32 => 700,
            SqlType::Float64 => 701,
            SqlType::Utf8 => 25,
            SqlType::Binary => 17,
            SqlType::Date => 1082,
            SqlType::Time => 1083,
            SqlType::Timestamp => 1114,
            SqlType::Json => 3802,
            SqlType::Unknown => 705,
        }
    }

    /// That type as `information_schema.columns.data_type` spells it.
    #[must_use]
    pub fn pg_data_type(&self) -> &'static str {
        match self {
            SqlType::Boolean => "boolean",
            SqlType::Int16 | SqlType::UInt16 => "smallint",
            SqlType::Int32 | SqlType::UInt32 => "integer",
            SqlType::Int64 | SqlType::UInt64 => "bigint",
            SqlType::Float32 => "real",
            SqlType::Float64 => "double precision",
            SqlType::Utf8 => "text",
            SqlType::Binary => "bytea",
            SqlType::Date => "date",
            SqlType::Time => "time without time zone",
            SqlType::Timestamp => "timestamp without time zone",
            SqlType::Json => "jsonb",
            SqlType::Unknown => "unknown",
        }
    }

    /// That PostgreSQL type's name.
    #[must_use]
    pub fn pg_name(&self) -> &'static str {
        match self {
            SqlType::Boolean => "bool",
            SqlType::Int16 | SqlType::UInt16 => "int2",
            SqlType::Int32 | SqlType::UInt32 => "int4",
            SqlType::Int64 | SqlType::UInt64 => "int8",
            SqlType::Float32 => "float4",
            SqlType::Float64 => "float8",
            SqlType::Utf8 => "text",
            SqlType::Binary => "bytea",
            SqlType::Date => "date",
            SqlType::Time => "time",
            SqlType::Timestamp => "timestamp",
            SqlType::Json => "jsonb",
            SqlType::Unknown => "unknown",
        }
    }
}

/// The user a session runs as and every catalog object's owner.
pub const SESSION_OWNER: &str = "rushdb";

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
    block_on(execute_sql_async_with_budget(
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
    block_on(execute_sql_async_with_budget_in_view_cancellable(
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
    block_on(execute_read(
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

    let context = context_in_view_visible(lmpg, &snapshot, text, visible)?;
    let compatible = pg_catalog::compatible(sql);
    let sql = compatible.as_deref().unwrap_or(sql);

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
        .map_err(|error| context.stream_error(&error))?;
    let mut rows = Vec::new();
    let mut retained_bytes = 0usize;
    while let Some(batch) = stream.next().await {
        if is_cancelled() {
            return Err(SqlError::Cancelled);
        }
        let batch = batch.map_err(|error| context.stream_error(&error))?;
        append_batch_rows_with_budget(&batch, &mut rows, &mut retained_bytes, max_result_bytes)?;
    }
    Ok(rows)
}

/// A statement that failed to plan: a failed `text_search` call keeps its
/// own class, a statement error is the caller's, anything else is the
/// engine's.
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
    scan_error(&error)
        .or_else(|| statement_error(&error))
        .unwrap_or_else(|| SqlError::Engine(error.to_string()))
}

/// The class of a failure that describes the statement rather than the
/// engine (SQL-1): an unknown name, a plan that does not type-check, an
/// unsupported feature, a value that does not cast. `None` for the rest.
fn statement_error(error: &datafusion::error::DataFusionError) -> Option<SqlError> {
    use arrow::error::ArrowError;
    use datafusion::common::SchemaError;
    use datafusion::error::DataFusionError;
    Some(match error.find_root() {
        DataFusionError::SchemaError(schema, _) => match schema.as_ref() {
            SchemaError::FieldNotFound { .. } => SqlError::Undefined(schema.to_string()),
            _ => SqlError::Plan(schema.to_string()),
        },
        DataFusionError::Plan(message)
            if message.starts_with("Invalid function ")
                || (message.starts_with("table '") && message.contains("' not found")) =>
        {
            SqlError::Undefined(message.clone())
        }
        DataFusionError::Plan(message) => SqlError::Plan(message.clone()),
        DataFusionError::SQL(parse, _) => SqlError::Parse(parse.to_string()),
        DataFusionError::NotImplemented(message) => SqlError::Unsupported(message.clone()),
        DataFusionError::ArrowError(arrow, _) => match arrow.as_ref() {
            // Text compared with a number is cast to the number's type: the
            // values of a property stored as both text and number (it reads
            // as text), or a date literal against a datetime column (it
            // reads as epoch milliseconds).
            ArrowError::CastError(message) if message.starts_with("Cannot cast string") => {
                SqlError::InvalidValue(format!(
                    "{message}; text compared with a number is cast to a number. A column \
                     whose values have more than one type reads as text: compare it with \
                     text, or skip the values that are not numbers with \
                     TRY_CAST(column AS BIGINT). A datetime column reads as epoch \
                     milliseconds: compare to_timestamp_millis(column) with a TIMESTAMP"
                ))
            }
            ArrowError::CastError(message)
            | ArrowError::ParseError(message)
            | ArrowError::ArithmeticOverflow(message) => SqlError::InvalidValue(message.clone()),
            ArrowError::DivideByZero => SqlError::InvalidValue(arrow.to_string()),
            _ => return None,
        },
        _ => return None,
    })
}

/// A "table not found" planning failure, with the closest table names of
/// the schema it was looked up in. `DataFusion` already suggests
/// close column names; it does not for tables.
fn with_table_hint(error: SqlError, context: &SessionContext) -> SqlError {
    let SqlError::Undefined(message) = &error else {
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
    SqlError::Undefined(format!("{message}. Did you mean {}?", quoted.join(" or ")))
}

/// A label scan refused or stopped like a records read (SQL-2, QRY-3)
/// keeps its class.
fn scan_error(error: &datafusion::error::DataFusionError) -> Option<SqlError> {
    let datafusion::error::DataFusionError::External(inner) = error.find_root() else {
        return None;
    };
    // RushDB's own operators (lookup joins, edge fetches) stop as SQL.
    if matches!(inner.downcast_ref::<SqlError>(), Some(SqlError::Cancelled)) {
        return Some(SqlError::Cancelled);
    }
    match inner.downcast_ref::<rushdb_query::QueryError>()? {
        rushdb_query::QueryError::UnindexedWalk(message) => {
            Some(SqlError::UnindexedWalk(message.clone()))
        }
        rushdb_query::QueryError::Cancelled => Some(SqlError::Cancelled),
        _ => None,
    }
}

fn map_stream_error(error: &datafusion::error::DataFusionError, cap: usize) -> SqlError {
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
        // The pool is the statement's working set: `query-memory-max`.
        return SqlError::MemoryLimit(cap);
    }
    statement_error(error).unwrap_or(SqlError::Engine(message))
}

/// The working-set cap of a statement run on `lmpg`: `query-memory-max`
/// tightened by the request, or [`DEFAULT_WORKING_SET`] when unset.
fn working_set_cap(lmpg: &Lmpg) -> usize {
    lmpg.limits()
        .query_memory_max_bytes()
        .map_or(DEFAULT_WORKING_SET, |bytes| {
            usize::try_from(bytes).unwrap_or(usize::MAX)
        })
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
    block_on(execute_sql_mutation_async(lmpg, sql, parameters))
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
    block_on(execute_sql_mutation_async_in_view(
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
    block_on(describe_sql_async(
        lmpg,
        sql,
        None,
        &CatalogAccess::default(),
    ))
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
    block_on(describe_sql_mutation_async(lmpg, sql, None))
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
    let context = context_in_view(lmpg, &snapshot, text)?;
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

/// Async form of [`describe_sql`], describing a statement without executing
/// it, over the catalog `visible` shows (under an enforced policy, a hidden
/// table or column fails to describe exactly as it fails to run).
///
/// # Errors
///
/// Returns the same errors as [`describe_sql`].
pub async fn describe_sql_async(
    lmpg: &Lmpg,
    sql: &str,
    text: Option<&Arc<TextIndexes>>,
    visible: &CatalogAccess,
) -> Result<SqlDescription, SqlError> {
    validate_read_only(sql)?;
    let snapshot = Arc::new(lmpg.store.read_view()?);
    let context = context_in_view_visible(lmpg, &snapshot, text, visible)?.context;
    let compatible = pg_catalog::compatible(sql);
    let sql = compatible.as_deref().unwrap_or(sql);
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

struct DataFusionContext {
    context: SessionContext,
    /// The statement's pool: its working set and its charge on the shared
    /// SQL budget, released when the context drops.
    pool: Arc<StatementPool>,
    /// The pool's cap, named by a [`SqlError::MemoryLimit`].
    cap: usize,
}

impl DataFusionContext {
    /// A failed stream as the [`SqlError`] that names its cause.
    fn stream_error(&self, error: &datafusion::error::DataFusionError) -> SqlError {
        match self.pool.refused() {
            Some(bytes) => SqlError::Busy(bytes),
            None => map_stream_error(error, self.cap),
        }
    }
}

fn context_in_view(
    lmpg: &Lmpg,
    snapshot: &Arc<ReadView>,
    text: Option<&Arc<TextIndexes>>,
) -> Result<DataFusionContext, SqlError> {
    context_in_view_visible(lmpg, snapshot, text, &CatalogAccess::default())
}

/// A statement context whose catalog shows only what `visible` allows
///. Text search is not offered under an enforced policy: its
/// indexes are not policy-scoped.
fn context_in_view_visible(
    lmpg: &Lmpg,
    snapshot: &Arc<ReadView>,
    text: Option<&Arc<TextIndexes>>,
    visible: &CatalogAccess,
) -> Result<DataFusionContext, SqlError> {
    let config = SessionConfig::new()
        // `information_schema` is RushDB's own (`pg_catalog.rs`), with
        // PostgreSQL's columns.
        .with_information_schema(false)
        .with_target_partitions(1);
    let cap = working_set_cap(lmpg);
    let (context, pool) = session_context(&lmpg.store, config, cap)?;
    // Lazy catalog: labels and edge types are materialized only when the
    // statement actually resolves them, instead of reconstructing the whole
    // tenant on every query.
    register_lazy_catalog_visible(&context, lmpg, snapshot, visible)?;
    if !visible.enforced() {
        text_search::install(&context, lmpg, snapshot, text);
    }
    Ok(DataFusionContext { context, pool, cap })
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
    let labels: Arc<dyn datafusion::catalog::SchemaProvider> = Arc::new(
        lazy_catalog::LazyLabelSchema::new(lmpg, Arc::clone(snapshot), visible.clone())?,
    );
    catalog
        .register_schema("public", Arc::clone(&labels))
        .map_err(|error| SqlError::Engine(error.to_string()))?;
    let edges: Arc<dyn datafusion::catalog::SchemaProvider> =
        Arc::new(lazy_catalog::LazyRelationshipSchema::new(
            tenant,
            Arc::clone(snapshot),
            lmpg.limits(),
            visible.clone(),
        ));
    catalog
        .register_schema("rushdb_edges", Arc::clone(&edges))
        .map_err(|error| SqlError::Engine(error.to_string()))?;
    // PostgreSQL clients list the same tables through `pg_catalog`.
    pg_catalog::install(context, labels, edges)
        .map_err(|error| SqlError::Engine(error.to_string()))?;
    Ok(())
}

fn session_context(
    store: &rushdb_storage::Store,
    config: SessionConfig,
    cap: usize,
) -> Result<(SessionContext, Arc<StatementPool>), SqlError> {
    // Admission: the statement's first charge on the shared SQL budget.
    let shared = store
        .sql_phase_budget()
        .map(|budget| {
            budget
                .try_reserve(DATAFUSION_PHASE_OVERHEAD)
                .map_err(|_| SqlError::Busy(DATAFUSION_PHASE_OVERHEAD))
        })
        .transpose()?;
    let pool = Arc::new(StatementPool::new(cap, shared));
    let runtime = RuntimeEnvBuilder::new()
        .with_memory_pool(
            Arc::clone(&pool) as Arc<dyn datafusion::execution::memory_pool::MemoryPool>
        )
        .build()
        .map_err(|error| SqlError::Engine(error.to_string()))?;
    let context = SessionContext::new_with_config_rt(config, Arc::new(runtime));
    // Joins on a RushDB table's lookup key fetch its rows by key instead of
    // scanning it into a hash table, and inner-join chains are ordered from
    // the smallest table outward (both from the store's exact counts).
    let context = SessionContext::new_with_state(
        datafusion::execution::SessionStateBuilder::new_from_existing(context.state())
            .with_physical_optimizer_rule(Arc::new(lookup_join::LookupJoins))
            .with_physical_optimizer_rule(Arc::new(distinct_topk::DistinctTopK))
            .build(),
    );
    context.add_optimizer_rule(Arc::new(join_order::JoinOrder));
    // A top-k reads the columns it orders and deduplicates by; the rest of a
    // label's columns are fetched by id for the rows it returns.
    context.add_optimizer_rule(Arc::new(late_materialization::LateMaterialization));
    context.add_optimizer_rule(Arc::new(late_materialization::DistinctKeys));
    // Q-10: register the overflow-checked integer SUM (built-in SUM(Int64)
    // wraps). Installed on every execution context so no query path misses it.
    checked_sum::install(&context);
    Ok((context, pool))
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

pub(crate) fn sql_type_from_arrow(data_type: &DataType) -> SqlType {
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
            let text = statement.to_string();
            return Err(match unsupported_statement(&text) {
                Some(reason) => SqlError::Unsupported(reason),
                None => SqlError::ReadOnly(text),
            });
        }
    }
    Ok(())
}

/// Why a statement that is neither a read nor a write the mutation path
/// runs (DML and index DDL) is not run at all, by its leading keywords.
/// `None` for such a write.
fn unsupported_statement(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split_whitespace()
        .take(3)
        .map(str::to_ascii_uppercase)
        .collect();
    let word = |index: usize| words.get(index).map_or("", String::as_str);
    let reason = match (word(0), word(1)) {
        ("INSERT" | "UPDATE" | "DELETE", _) | ("DROP" | "CREATE", "INDEX") => return None,
        ("CREATE", "UNIQUE") if word(2) == "INDEX" => return None,
        ("SET" | "RESET", _) => "session settings are not SQL statements here; over the \
             PostgreSQL wire, SET accepts the settings listed in the manual"
            .to_owned(),
        ("PREPARE" | "EXECUTE" | "DEALLOCATE", _) => "prepared statements are made with the \
             extended query protocol (Parse/Bind) or bound parameters, not with SQL PREPARE"
            .to_owned(),
        ("LISTEN" | "UNLISTEN" | "NOTIFY", _) => "change notifications are not offered".to_owned(),
        ("DECLARE" | "FETCH" | "CLOSE" | "MOVE", _) => {
            "cursors are not offered; page with ORDER BY … LIMIT … OFFSET".to_owned()
        }
        ("COPY", _) => "COPY is not offered; read with SELECT".to_owned(),
        ("CREATE" | "ALTER" | "DROP" | "TRUNCATE", _) => {
            return Some(format!(
                "{} {}: tables are record labels, made by writing records; the only DDL is \
                 CREATE INDEX and DROP INDEX",
                word(0),
                word(1)
            ))
        }
        _ => "not offered".to_owned(),
    };
    Some(format!("{}: {reason}", word(0)))
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
        return Err(SqlError::RowLimit(
            rows.len().saturating_add(batch.num_rows()),
        ));
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
            return Err(SqlError::ResultTooLarge(limit));
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
