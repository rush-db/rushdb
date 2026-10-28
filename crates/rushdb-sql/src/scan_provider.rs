//! P09 lazy scan provider: a label is exposed to `DataFusion` as a streaming
//! `TableProvider` instead of an eagerly-materialized `MemTable`. Records are
//! reconstructed from the pinned `ReadView` on demand in bounded batches, so
//! catalog construction does NOT load the whole tenant and a selective query
//! materializes only what it scans.
//!
//! SQL-2: the table's schema comes from the property catalog, never from the
//! rows, and a filtered scan reads its candidates the way records reads do:
//! from the index plan when an index answers a pushed condition, from a
//! streamed walk of the label otherwise, refused like a records read when a
//! filter without a usable index would check more records than
//! `unindexed-walk-max-owners` allows. `DataFusion` re-applies every filter.
//!
//! Implementation: a `PartitionStream` feeds `StreamingTableExec`; the
//! `TableProvider::scan` returns that plan.

use std::collections::{HashMap, HashSet};
use std::ops::Bound;
use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::{Schema, SchemaRef};
use arrow_json::reader::infer_json_schema_from_iterator;
use datafusion::catalog::Session;
use datafusion::common::stats::Precision;
use datafusion::common::{Result as DFResult, ScalarValue, Statistics};
use datafusion::datasource::TableProvider;
use datafusion::execution::TaskContext;
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::streaming::{PartitionStream, StreamingTableExec};
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, SendableRecordBatchStream,
};
use datafusion_expr::TableType;
use datafusion_expr::{Expr, Operator, TableProviderFilterPushDown};
use rushdb_lmpg::reader::try_access;
use rushdb_lmpg::{reconstruct, CatalogAccess, Lmpg};
use rushdb_query::{FieldPred, PlannedCandidates};
use rushdb_storage::{ReadView, ScanBounds, ScanProjection};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::model::{LogicalType, TypeState};
use rushdb_types::value::ID_FIELD;
use serde_json::{Map, Value as Json};

use crate::SqlError;

/// Rows reconstructed per output batch (bounded working memory).
const BATCH_ROWS: usize = 256;
/// Rows per batch when no column is read: only ordinals are held.
const COUNT_BATCH_ROWS: usize = 4096;
/// Upper bound for the transient NDJSON buffer used to convert one bounded
/// Arrow batch. This is separate from the row ceiling so one oversized JSON
/// property cannot turn a single batch into an unbounded allocation.
const MAX_BATCH_JSON_BYTES: usize = 8 * 1024 * 1024;

/// A lazy table whose rows are the LMPG records of one label.
/// Column-vs-literal comparison pushed into the LMPG scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CmpOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

/// A conjunct of `column OP literal` filters pushed into the label scan.
type Pushdown = Vec<(String, CmpOp, datafusion::common::ScalarValue)>;

/// Translate `DataFusion` filter expressions into a scan pushdown: every
/// `column OP literal` conjunct of their `AND` trees. Other conjuncts are
/// left out; the candidates are planned from these, so they are a superset
/// of the filtered rows either way.
fn translate_filters(filters: &[Expr]) -> Pushdown {
    let mut out = Pushdown::new();
    for filter in filters {
        walk_filter(filter, &mut out);
    }
    out
}

/// Collect the `column OP literal` conjuncts of an `AND` tree; `true` when
/// the tree is nothing else.
fn walk_filter(e: &Expr, out: &mut Pushdown) -> bool {
    let Expr::BinaryExpr(b) = e else {
        return false;
    };
    if matches!(b.op, Operator::And) {
        let left = walk_filter(&b.left, out);
        let right = walk_filter(&b.right, out);
        return left && right;
    }
    let (Expr::Column(c), Expr::Literal(lit, _)) = (b.left.as_ref(), b.right.as_ref()) else {
        return false;
    };
    let op = match b.op {
        Operator::Eq => CmpOp::Eq,
        Operator::NotEq => CmpOp::Ne,
        Operator::Gt => CmpOp::Gt,
        Operator::GtEq => CmpOp::Gte,
        Operator::Lt => CmpOp::Lt,
        Operator::LtEq => CmpOp::Lte,
        _ => return false,
    };
    out.push((c.name.clone(), op, lit.clone()));
    true
}

/// The conjuncts of `filter` when the scan answers it in full: every one is
/// a comparison whose native predicate keeps exactly the rows SQL keeps
/// (`=` on a column of one kind of value, `>=`/`<=` on a numeric one), so
/// an exact index plan, or the strict check of [`eval_pushdown`], answers
/// it. `DataFusion` then drops the filter (`Exact`).
fn exact_conjuncts(filter: &Expr, columns: &HashMap<String, ColumnFacts>) -> Option<Pushdown> {
    let mut conjuncts = Pushdown::new();
    let exact = walk_filter(filter, &mut conjuncts)
        && conjuncts.iter().all(|(column, op, lit)| {
            column != ID_FIELD
                && matches!(op, CmpOp::Eq | CmpOp::Gte | CmpOp::Lte)
                && native_condition(column, *op, lit, columns).is_some()
        });
    exact.then_some(conjuncts)
}

/// Integers beyond this magnitude are not exact as an `f64` range bound.
const EXACT_F64_INTEGER: i64 = 1 << 53;

/// `i` as an `f64` range bound, when the conversion is exact.
#[expect(
    clippy::cast_precision_loss,
    reason = "only magnitudes up to 2^53, which an f64 holds exactly"
)]
fn exact_f64(i: i64) -> Option<f64> {
    (i.abs() <= EXACT_F64_INTEGER).then_some(i as f64)
}

/// A pushed conjunct as a native predicate whose index candidates hold every
/// row the conjunct keeps, or `None` when no such predicate is safe: the
/// column holds arrays, or a type the literal's comparison could coerce
/// (`'5'` against integers widened to text). Strict bounds widen to
/// inclusive ones; the rows are re-checked exactly.
fn native_condition(
    column: &str,
    op: CmpOp,
    lit: &ScalarValue,
    columns: &HashMap<String, ColumnFacts>,
) -> Option<rushdb_query::Expr> {
    if column == ID_FIELD {
        return match (op, lit) {
            (CmpOp::Eq, ScalarValue::Utf8(Some(id))) => {
                Some(rushdb_query::Expr::Field(FieldPred::Id(id.clone())))
            }
            _ => None,
        };
    }
    let facts = columns.get(column).filter(|facts| !facts.array)?;
    let only = |allowed: &[LogicalType]| {
        facts.types.iter().all(|state| match state {
            TypeState::Unresolved => true,
            TypeState::Known(ty) => allowed.contains(ty),
        })
    };
    let name = rushdb_query::normalize(column);
    let field = |pred: FieldPred| Some(rushdb_query::Expr::Field(pred));
    match lit {
        ScalarValue::Utf8(Some(text)) if op == CmpOp::Eq && only(&[LogicalType::Utf8]) => {
            field(FieldPred::Eq(name, Json::String(text.clone())))
        }
        ScalarValue::Boolean(Some(flag)) if op == CmpOp::Eq && only(&[LogicalType::Boolean]) => {
            field(FieldPred::Eq(name, Json::Bool(*flag)))
        }
        ScalarValue::Int64(Some(_)) | ScalarValue::Float64(Some(_))
            if only(&[LogicalType::Int64, LogicalType::Float64]) =>
        {
            let (number, bound) = match lit {
                ScalarValue::Int64(Some(i)) => (Json::Number((*i).into()), exact_f64(*i)),
                ScalarValue::Float64(Some(f)) => (
                    serde_json::Number::from_f64(*f).map(Json::Number)?,
                    Some(*f),
                ),
                _ => return None,
            };
            match op {
                CmpOp::Eq => field(FieldPred::Eq(name, number)),
                CmpOp::Gt | CmpOp::Gte => field(FieldPred::Gte(name, bound?)),
                CmpOp::Lt | CmpOp::Lte => field(FieldPred::Lte(name, bound?)),
                CmpOp::Ne => None,
            }
        }
        _ => None,
    }
}

/// Evaluate a pushdown against an externalized record (JSON object with the
/// scan's column values), strictly: a missing column, a null, or a value of
/// another kind than the literal never satisfies a comparison, as in SQL.
/// Only [`exact_conjuncts`] are checked here, whose columns hold one kind
/// of value.
fn eval_pushdown(row: &serde_json::Value, pushdown: &Pushdown) -> bool {
    let Some(obj) = row.as_object() else {
        return false;
    };
    pushdown.iter().all(|(column, op, lit)| {
        let Some(value) = obj.get(column) else {
            return false;
        };
        let cmp = match (value, lit) {
            (serde_json::Value::Number(_), datafusion::common::ScalarValue::Int64(Some(l))) => {
                let bound = serde_json::Value::Number((*l).into());
                rushdb_types::value::json_scalar_cmp(value, &bound)
                    .map(|ordering| compare_ordering(ordering, *op))
            }
            (serde_json::Value::Number(_), datafusion::common::ScalarValue::Float64(Some(l))) => {
                serde_json::Number::from_f64(*l)
                    .map(serde_json::Value::Number)
                    .and_then(|bound| {
                        rushdb_types::value::json_scalar_cmp(value, &bound)
                            .map(|ordering| compare_ordering(ordering, *op))
                    })
            }
            (serde_json::Value::String(s), datafusion::common::ScalarValue::Utf8(Some(l))) => {
                Some(match op {
                    CmpOp::Eq => s == l,
                    CmpOp::Ne => s != l,
                    CmpOp::Gt => s > l,
                    CmpOp::Gte => s >= l,
                    CmpOp::Lt => s < l,
                    CmpOp::Lte => s <= l,
                })
            }
            (serde_json::Value::Bool(b), datafusion::common::ScalarValue::Boolean(Some(l))) => {
                Some(match op {
                    CmpOp::Eq => *b == *l,
                    CmpOp::Ne => *b != *l,
                    _ => false,
                })
            }
            _ => None,
        };
        cmp.unwrap_or(false)
    })
}

fn compare_ordering(ordering: std::cmp::Ordering, op: CmpOp) -> bool {
    match op {
        CmpOp::Eq => ordering == std::cmp::Ordering::Equal,
        CmpOp::Ne => ordering != std::cmp::Ordering::Equal,
        CmpOp::Gt => ordering == std::cmp::Ordering::Greater,
        CmpOp::Gte => matches!(
            ordering,
            std::cmp::Ordering::Greater | std::cmp::Ordering::Equal
        ),
        CmpOp::Lt => ordering == std::cmp::Ordering::Less,
        CmpOp::Lte => matches!(
            ordering,
            std::cmp::Ordering::Less | std::cmp::Ordering::Equal
        ),
    }
}

pub struct LabelTable {
    lmpg: Arc<Lmpg>,
    label: String,
    snapshot: Arc<ReadView>,
    schema: SchemaRef,
    /// The catalog's facts per column, for planning pushed conditions.
    columns: Arc<HashMap<String, ColumnFacts>>,
    /// Exact live row count for the label at this pinned view (from the derived
    /// label catalog). Surfaced to `DataFusion` as `Precision::Exact` so join and
    /// scan costing does not fall back to guesses.
    row_count: usize,
}

impl std::fmt::Debug for LabelTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LabelTable")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// What the property catalog says about one column of a label.
#[derive(Debug, Default)]
struct ColumnFacts {
    /// The type states of the typed definitions the label's records carry.
    types: Vec<TypeState>,
    /// Some record of the label holds an array under the property.
    array: bool,
}

impl LabelTable {
    /// Build a table from catalog reads only: the label's columns from its
    /// property support counters and definitions, its row count from the
    /// label counter. No record is read, whatever the label's size.
    #[cfg(test)]
    pub fn try_new(
        lmpg: Arc<Lmpg>,
        label: String,
        snapshot: Arc<ReadView>,
    ) -> Result<Self, SqlError> {
        Self::try_new_visible(lmpg, label, snapshot, &CatalogAccess::default())
    }

    /// Like [`Self::try_new`], with only the columns `visible` allows
    ///: a hidden property is not a column, so no statement can read,
    /// filter on or name it.
    pub(crate) fn try_new_visible(
        lmpg: Arc<Lmpg>,
        label: String,
        snapshot: Arc<ReadView>,
        visible: &CatalogAccess,
    ) -> Result<Self, SqlError> {
        let tenant = lmpg.tenant;
        let (schema, columns) = label_columns(&snapshot, tenant, &label, &|property| {
            visible.property(&label, property)
        })?;
        let row_count = usize::try_from(rushdb_lmpg::derived::label_count(
            &snapshot, tenant, &label,
        )?)
        .unwrap_or(usize::MAX);
        Ok(Self {
            lmpg,
            label,
            snapshot,
            schema: Arc::new(schema),
            columns: Arc::new(columns),
            row_count,
        })
    }
}

/// The label's schema from the property catalog. Every typed
/// definition its records carry contributes one sample value of its type
/// (in an array when the label holds arrays under it), and the samples are
/// inferred by the same Arrow JSON rules as row data, so mixed types widen
/// exactly as they would over the rows. Columns follow property creation
/// order, then `__id`.
fn label_columns(
    snapshot: &ReadView,
    tenant: TenantId,
    label: &str,
    visible: &dyn Fn(&str) -> bool,
) -> Result<(Schema, HashMap<String, ColumnFacts>), SqlError> {
    let mut support =
        rushdb_lmpg::derived::prop_support_of_label(snapshot, tenant, OwnerKind::Record, label)?;
    support.retain(|(_, owners)| *owners > 0);
    support.sort_unstable();
    let mut first = Map::new();
    let mut extra: Vec<Json> = Vec::new();
    let mut columns: HashMap<String, ColumnFacts> = HashMap::new();
    for (pid, _) in support {
        let definition = try_access::definition(snapshot, tenant, pid)?
            .ok_or(rushdb_storage::StorageError::CorruptLog)?;
        let family = try_access::family(snapshot, tenant, definition.family_id)?
            .ok_or(rushdb_storage::StorageError::CorruptLog)?;
        if !visible(&family.display_name) {
            continue;
        }
        let array = rushdb_lmpg::derived::array_markers(snapshot, tenant, OwnerKind::Record, pid)?
            .1
            .iter()
            .any(|marked| marked == label);
        let sample = match definition.type_state {
            TypeState::Unresolved => Json::Null,
            TypeState::Known(LogicalType::Boolean) => Json::Bool(true),
            TypeState::Known(LogicalType::Int64 | LogicalType::DateTime) => Json::from(0),
            TypeState::Known(LogicalType::Float64) => Json::from(0.5),
            TypeState::Known(LogicalType::Utf8) => Json::String(String::new()),
        };
        let value = if array {
            Json::Array(vec![sample])
        } else {
            sample
        };
        let facts = columns.entry(family.display_name.clone()).or_default();
        facts.types.push(definition.type_state);
        facts.array |= array;
        if first.contains_key(&family.display_name) {
            let mut row = Map::new();
            row.insert(family.display_name, value);
            extra.push(Json::Object(row));
        } else {
            first.insert(family.display_name, value);
        }
    }
    first.insert(ID_FIELD.into(), Json::String(String::new()));
    let schema =
        infer_json_schema_from_iterator(std::iter::once(Json::Object(first)).chain(extra).map(Ok))
            .map_err(|e| SqlError::Engine(e.to_string()))?;
    Ok((schema, columns))
}

#[async_trait::async_trait]
impl TableProvider for LabelTable {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    /// Exact row count for the label, from the maintained derived label
    /// catalog. `DataFusion` uses this for scan/join costing without materializing
    /// rows; it is never an approximation.
    fn statistics(&self) -> Option<Statistics> {
        Some(Statistics {
            num_rows: Precision::Exact(self.row_count),
            ..Default::default()
        })
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> DFResult<Vec<TableProviderFilterPushDown>> {
        // Every filter reaches the scan: it plans candidates from the
        // conjuncts it can state, and a filter it cannot state still tells it
        // the scan is filtered (the unindexed-walk guard). Only filters it
        // answers in full are `Exact`; DataFusion re-applies the rest.
        Ok(filters
            .iter()
            .map(|filter| {
                if exact_conjuncts(filter, &self.columns).is_some() {
                    TableProviderFilterPushDown::Exact
                } else {
                    TableProviderFilterPushDown::Inexact
                }
            })
            .collect())
    }

    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> DFResult<Arc<dyn ExecutionPlan>> {
        if filters.is_empty() && projection.is_some_and(Vec::is_empty) {
            return counted_rows(limit.map_or(self.row_count, |l| l.min(self.row_count)));
        }
        let pushdown = translate_filters(filters);
        // The native predicates the index plan is asked for; `None` when the
        // scan is unfiltered and walks the label.
        let conditions = (!filters.is_empty()).then(|| {
            pushdown
                .iter()
                .filter_map(|(column, op, lit)| native_condition(column, *op, lit, &self.columns))
                .collect::<Vec<_>>()
        });
        // The conjuncts of the `Exact` filters, which DataFusion no longer
        // applies: checked per row unless the index plan answers them.
        let strict: Pushdown = filters
            .iter()
            .filter_map(|filter| exact_conjuncts(filter, &self.columns))
            .flatten()
            .collect();
        // Reconstruct only columns needed by the projection (and the strict
        // check). The partition still exposes the full schema; omitted fields
        // are null and StreamingTableExec applies the final projection.
        let full_schema = Arc::clone(&self.schema);
        let projected = selected_properties(&self.schema, projection, None);
        let partition = Arc::new(LabelPartition {
            lmpg: Arc::clone(&self.lmpg),
            label: self.label.clone(),
            snapshot: Arc::clone(&self.snapshot),
            schema: full_schema.clone(),
            strict,
            conditions,
            projected: Arc::new(projected),
        });
        let plan = StreamingTableExec::try_new(
            full_schema,
            vec![partition],
            projection,
            Vec::<datafusion::physical_expr::LexOrdering>::new(),
            false,
            limit,
        )?;
        Ok(Arc::new(plan))
    }
}

/// One scan partition: incrementally reconstructs label records into bounded
/// Arrow batches.
struct LabelPartition {
    lmpg: Arc<Lmpg>,
    label: String,
    snapshot: Arc<ReadView>,
    schema: SchemaRef,
    strict: Pushdown,
    conditions: Option<Vec<rushdb_query::Expr>>,
    projected: Arc<HashSet<String>>,
}

impl std::fmt::Debug for LabelPartition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LabelPartition").finish()
    }
}

impl PartitionStream for LabelPartition {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<TaskContext>) -> SendableRecordBatchStream {
        let tenant = self.lmpg.tenant;
        let prefix = rushdb_types::keys::label_member_prefix(tenant, &self.label);
        let bounds = ScanBounds::prefix(&prefix);
        let state = LabelScanState {
            lmpg: Arc::clone(&self.lmpg),
            label: self.label.clone(),
            snapshot: Arc::clone(&self.snapshot),
            schema: Arc::clone(&self.schema),
            source: match &self.conditions {
                Some(conditions) => Source::Unplanned(conditions.clone()),
                None => Source::Walk,
            },
            prefix,
            lower: bounds.start,
            upper: bounds.end,
            done: false,
            strict: self.strict.clone(),
            selected: with_strict(&self.projected, &self.strict),
            projected: Arc::clone(&self.projected),
            reads: RowReads::Unresolved,
        };
        Box::pin(RecordBatchStreamAdapter::new(
            Arc::clone(&self.schema),
            futures::stream::unfold(state, |mut st| async move {
                next_label_batch(&mut st).map(|b| (b, st))
            }),
        ))
    }
}

impl DisplayAs for LabelPartition {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LabelPartition({})", self.label)
    }
}

/// Where a scan's candidate ordinals come from.
enum Source {
    /// A filtered scan before its first batch: the native conditions the
    /// index plan is asked for.
    Unplanned(Vec<rushdb_query::Expr>),
    /// The index plan's candidates.
    Planned(Box<PlannedCandidates>),
    /// The label's membership keys, streamed.
    Walk,
}

struct LabelScanState {
    lmpg: Arc<Lmpg>,
    label: String,
    snapshot: Arc<ReadView>,
    schema: SchemaRef,
    source: Source,
    /// Exact membership-key prefix for this label. The scan range is a byte
    /// prefix, so a label whose name prefixes another (`user` vs `user_admin`)
    /// also matches the other label's rows; every row is re-validated against
    /// this prefix and its fixed-width ordinal suffix.
    prefix: Vec<u8>,
    lower: Bound<Vec<u8>>,
    upper: Bound<Vec<u8>>,
    done: bool,
    /// Conjuncts checked per row; emptied when the index plan answers them.
    strict: Pushdown,
    /// The properties each row is rebuilt with: the projected ones and the
    /// strict conjuncts' columns.
    selected: Arc<HashSet<String>>,
    projected: Arc<HashSet<String>>,
    /// How `selected` is read.
    reads: RowReads,
}

/// How a scan reads its selected columns.
enum RowReads {
    /// Decided on the first batch, once the selected columns are final.
    Unresolved,
    /// One point read per column: every one is a scalar property.
    Points(rushdb_query::PointFields),
    /// From the rebuilt record (`__id`, arrays).
    Rebuild,
}

/// `projected` plus the columns `strict` checks.
fn with_strict(projected: &Arc<HashSet<String>>, strict: &Pushdown) -> Arc<HashSet<String>> {
    if strict.is_empty() {
        return Arc::clone(projected);
    }
    let mut selected = (**projected).clone();
    selected.extend(strict.iter().map(|(column, _, _)| column.clone()));
    Arc::new(selected)
}

fn selected_properties(
    schema: &SchemaRef,
    projection: Option<&Vec<usize>>,
    pushdown: Option<&Pushdown>,
) -> HashSet<String> {
    let mut selected = HashSet::new();
    let fields = projection
        .map(|indices| {
            indices
                .iter()
                .filter_map(|index| schema.fields().get(*index))
        })
        .map(Iterator::collect::<Vec<_>>);
    if let Some(fields) = fields {
        selected.extend(fields.into_iter().map(|field| field.name().clone()));
    } else {
        selected.extend(schema.fields().iter().map(|field| field.name().clone()));
    }
    if let Some(pd) = pushdown {
        selected.extend(pd.iter().map(|(name, _, _)| name.clone()));
    }
    selected
}

fn admit_row_json_bytes(
    current: usize,
    row: &serde_json::Value,
) -> Result<usize, datafusion::common::DataFusionError> {
    let bytes = serde_json::to_vec(row).map_err(|error| {
        datafusion::common::DataFusionError::Execution(format!(
            "SQL row JSON encoding failed: {error}"
        ))
    })?;
    if bytes.len() > MAX_BATCH_JSON_BYTES
        || current.saturating_add(bytes.len()) > MAX_BATCH_JSON_BYTES
    {
        return Err(datafusion::common::DataFusionError::Execution(format!(
            "SQL Arrow batch exceeds {MAX_BATCH_JSON_BYTES} byte JSON budget"
        )));
    }
    Ok(current.saturating_add(bytes.len()))
}

fn next_label_batch(
    state: &mut LabelScanState,
) -> Option<std::result::Result<RecordBatch, datafusion::common::DataFusionError>> {
    if state.done {
        return None;
    }
    let ordinals = match next_ordinals(state) {
        Ok(ordinals) => ordinals,
        Err(error) => {
            state.done = true;
            return Some(Err(error));
        }
    };
    if state.selected.is_empty() && state.strict.is_empty() {
        // Nothing to read or check (`count(*)`, `EXISTS`): every candidate
        // is a live row of the label, so only how many there are matters.
        return Some(Ok(null_rows(&state.schema, ordinals.len())));
    }
    let mut rows = Vec::with_capacity(BATCH_ROWS);
    let mut estimated_json_bytes = 0usize;
    if matches!(state.reads, RowReads::Unresolved) {
        match rushdb_query::PointFields::resolve(
            &state.snapshot,
            state.lmpg.tenant,
            state.selected.iter().map(String::as_str),
        ) {
            Ok(points) => state.reads = points.map_or(RowReads::Rebuild, RowReads::Points),
            Err(error) => {
                state.done = true;
                return Some(Err(datafusion::common::DataFusionError::External(
                    Box::new(error),
                )));
            }
        }
    }
    let mut reconstruct = reconstruct::ReconstructSession::new(&*state.snapshot, state.lmpg.tenant);
    for ordinal in ordinals {
        let external = match read_row(state, &mut reconstruct, ordinal) {
            Ok(Some(row)) => row,
            Ok(None) => continue,
            Err(error) => {
                state.done = true;
                return Some(Err(error));
            }
        };
        crate::record_work(|work| {
            work.reconstructed_rows = work.reconstructed_rows.saturating_add(1);
        });
        if !eval_pushdown(&external, &state.strict) {
            crate::record_work(|work| {
                work.filtered_rows = work.filtered_rows.saturating_add(1);
            });
            continue;
        }
        // Reject before retaining another reconstructed row. The later
        // Arrow/NDJSON conversion has the same ceiling, but checking here
        // keeps a batch of many large JSON values from accumulating its full
        // transient working set before that conversion runs.
        match admit_row_json_bytes(estimated_json_bytes, &external) {
            Ok(next) => estimated_json_bytes = next,
            Err(error) => {
                state.done = true;
                return Some(Err(error));
            }
        }
        rows.push(external);
    }
    if rows.is_empty() {
        // An empty filtered batch is not end-of-stream when unvisited members
        // remain. Returning an explicit empty batch lets the stream continue
        // and avoids truncating rows after a selective first page.
        return (!state.done).then(|| Ok(empty_batch(&state.schema)));
    }
    Some(batch_from_rows(&state.schema, rows))
}

/// One candidate's selected columns: by point reads when every selected
/// column is a scalar property, else from the rebuilt record. `None` when
/// the record is gone.
fn read_row(
    state: &LabelScanState,
    session: &mut reconstruct::ReconstructSession<'_, ReadView>,
    ordinal: u64,
) -> std::result::Result<Option<Json>, datafusion::common::DataFusionError> {
    if let RowReads::Points(points) = &state.reads {
        return points
            .row(&state.snapshot, state.lmpg.tenant, ordinal)
            .map(|row| Some(Json::Object(row)))
            .map_err(|error| datafusion::common::DataFusionError::External(Box::new(error)));
    }
    session
        .reconstruct_record_selected(ordinal, &state.selected)
        .map(|row| row.map(crate::externalize_record))
        .map_err(|error| datafusion::common::DataFusionError::External(Box::new(error)))
}

/// The next batch of candidate ordinals, ascending; marks the scan done with
/// its last batch. A filtered scan plans its candidates on its first batch.
fn next_ordinals(
    state: &mut LabelScanState,
) -> std::result::Result<Vec<u64>, datafusion::common::DataFusionError> {
    let external = |error: rushdb_query::QueryError| {
        datafusion::common::DataFusionError::External(Box::new(error))
    };
    // The request's deadline stops a long scan between batches; a
    // `count(*)` over millions of records yields no result batch until it
    // ends, so the executor's own poll would come too late.
    if state.lmpg.limits().expired() {
        return Err(external(rushdb_query::QueryError::Cancelled));
    }
    if let Source::Unplanned(conditions) = &state.source {
        let planned = rushdb_query::planned_candidates(
            &state.lmpg,
            &state.snapshot,
            std::slice::from_ref(&state.label),
            conditions,
            &|| false,
        )
        .map_err(external)?;
        if planned.as_ref().is_some_and(PlannedCandidates::exact) {
            // Every candidate matches every condition: the strict
            // conjuncts are answered.
            state.strict.clear();
            state.selected = Arc::clone(&state.projected);
        }
        state.source = planned.map_or(Source::Walk, |candidates| {
            Source::Planned(Box::new(candidates))
        });
    }
    let rows = if state.selected.is_empty() && state.strict.is_empty() {
        COUNT_BATCH_ROWS
    } else {
        BATCH_ROWS
    };
    if let Source::Planned(candidates) = &mut state.source {
        let ordinals: Vec<u64> = candidates.by_ref().take(rows).collect();
        state.done = ordinals.len() < rows;
        let candidate_rows = ordinals.len() as u64;
        crate::record_work(|work| {
            work.candidate_rows = work.candidate_rows.saturating_add(candidate_rows);
        });
        return Ok(ordinals);
    }
    let mut cursor = state
        .snapshot
        .scan(
            ScanBounds {
                start: state.lower.clone(),
                end: state.upper.clone(),
            },
            ScanProjection::Keys,
        )
        .map_err(|error| datafusion::common::DataFusionError::External(Box::new(error)))?;
    let batch = cursor
        .next_batch(rows, 1024 * 1024)
        .map_err(|error| datafusion::common::DataFusionError::External(Box::new(error)))?;
    let candidate_rows = batch.rows.len() as u64;
    let posting_bytes = batch.rows.iter().map(|row| row.key.len() as u64).sum();
    crate::record_work(|work| {
        work.candidate_rows = work.candidate_rows.saturating_add(candidate_rows);
        work.posting_bytes = work.posting_bytes.saturating_add(posting_bytes);
    });
    if let Some(last) = batch.rows.last() {
        state.lower = Bound::Excluded(last.key.clone());
    }
    state.done = batch.done;
    // The label is length-prefixed, so the scan prefix owns exactly this
    // label's rows; the ordinal ends each key.
    batch
        .rows
        .iter()
        .map(|member| {
            rushdb_types::keys::read_ord_tail(&member.key, state.prefix.len()).ok_or_else(|| {
                datafusion::common::DataFusionError::External(Box::new(
                    rushdb_storage::StorageError::CorruptLog,
                ))
            })
        })
        .collect()
}

pub(crate) fn batch_from_rows(
    schema: &Schema,
    rows: Vec<serde_json::Value>,
) -> std::result::Result<RecordBatch, datafusion::common::DataFusionError> {
    let mut ndjson = Vec::new();
    for row in rows {
        serde_json::to_writer(&mut ndjson, &row)
            .map_err(|e| datafusion::common::DataFusionError::External(Box::new(e)))?;
        ndjson.push(b'\n');
        if ndjson.len() > MAX_BATCH_JSON_BYTES {
            return Err(datafusion::common::DataFusionError::ResourcesExhausted(
                format!("SQL Arrow batch exceeds {MAX_BATCH_JSON_BYTES} byte JSON budget"),
            ));
        }
    }
    let reader = arrow_json::ReaderBuilder::new(Arc::new(schema.clone()))
        .with_batch_size(1024)
        .with_coerce_primitive(true)
        .build(std::io::Cursor::new(ndjson))
        .map_err(|e| datafusion::common::DataFusionError::External(Box::new(e)))?;
    let mut batches = reader
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| datafusion::common::DataFusionError::External(Box::new(e)))?;
    match batches.pop() {
        Some(batch) => Ok(batch),
        None => Ok(empty_batch(schema)),
    }
}

/// An unfiltered scan that reads no column (`count(*)`): `rows` rows of no
/// columns, from the label counter, in batches of [`COUNT_BATCH_ROWS`].
fn counted_rows(rows: usize) -> DFResult<Arc<dyn ExecutionPlan>> {
    let schema = Arc::new(Schema::empty());
    let options = |n: usize| arrow::array::RecordBatchOptions::new().with_row_count(Some(n));
    let batches = (0..rows)
        .step_by(COUNT_BATCH_ROWS)
        .map(|start| {
            RecordBatch::try_new_with_options(
                Arc::clone(&schema),
                Vec::new(),
                &options(COUNT_BATCH_ROWS.min(rows - start)),
            )
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(datafusion::datasource::memory::MemorySourceConfig::try_new_exec(&[batches], schema, None)?)
}

/// `rows` rows of nulls: a batch for a scan that reads no column.
fn null_rows(schema: &SchemaRef, rows: usize) -> RecordBatch {
    RecordBatch::try_new_with_options(
        Arc::clone(schema),
        schema
            .fields()
            .iter()
            .map(|f| arrow::array::new_null_array(f.data_type(), rows))
            .collect(),
        &arrow::array::RecordBatchOptions::new().with_row_count(Some(rows)),
    )
    .expect("null batch with known schema")
}

fn empty_batch(schema: &Schema) -> RecordBatch {
    RecordBatch::try_new(
        Arc::new(schema.clone()),
        schema
            .fields()
            .iter()
            .map(|f| arrow::array::new_empty_array(f.data_type()))
            .collect(),
    )
    .expect("empty batch with known schema")
}
#[cfg(test)]
mod tests {
    use super::*;
    use arrow::datatypes::{DataType, Field};

    #[test]
    fn label_table_reports_the_exact_derived_row_count() {
        // DataFusion gets an exact row count from the derived
        // label catalog, not a guess.
        let store = Arc::new(rushdb_storage::Store::new_in_memory());
        let tenant = TenantId::single();
        let lmpg = rushdb_lmpg::Lmpg::new(store, tenant);
        for _ in 0..5 {
            lmpg.create_record("Doc", None, vec![]).unwrap();
        }
        let snap = Arc::new(lmpg.store.read_view().unwrap());
        let table = LabelTable::try_new(
            Arc::new(Lmpg::new(Arc::clone(&lmpg.store), tenant)),
            "Doc".into(),
            Arc::clone(&snap),
        )
        .unwrap();
        let stats = table.statistics().expect("statistics present");
        assert_eq!(stats.num_rows, Precision::Exact(5));
        assert_eq!(
            rushdb_lmpg::derived::label_count(&snap, tenant, "Doc").unwrap(),
            5
        );
    }

    #[test]
    fn translate_filters_pushes_and_joined_comparisons() {
        use datafusion::common::ScalarValue;
        use datafusion_expr::Expr;
        use datafusion_expr::{col, lit, Operator};
        let filters = [Expr::BinaryExpr(datafusion_expr::BinaryExpr::new(
            Box::new(col("score").gt_eq(lit(10i64))),
            Operator::And,
            Box::new(col("active").eq(lit(true))),
        ))];
        let pd = translate_filters(&filters);
        assert_eq!(pd.len(), 2);
        assert_eq!(pd[0].0, "score");
        assert_eq!(pd[0].1, CmpOp::Gte);
        assert_eq!(pd[0].2, ScalarValue::Int64(Some(10)));
        assert_eq!(pd[1].0, "active");
        assert_eq!(pd[1].1, CmpOp::Eq);
    }

    #[test]
    fn selected_properties_include_projection_and_pushdown_columns() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("__id", DataType::Utf8, true),
            Field::new("name", DataType::Utf8, true),
            Field::new("score", DataType::Int64, true),
        ]));
        let pushdown = vec![(
            "score".to_owned(),
            CmpOp::Gte,
            datafusion::common::ScalarValue::Int64(Some(10)),
        )];
        let selected = selected_properties(&schema, Some(&vec![1]), Some(&pushdown));
        assert!(selected.contains("name"));
        assert!(selected.contains("score"));
        assert!(!selected.contains("__id"));
    }

    #[test]
    fn translate_filters_keeps_the_conjuncts_beside_an_or() {
        use datafusion_expr::Expr;
        use datafusion_expr::{col, lit, Operator};
        let or = Expr::BinaryExpr(datafusion_expr::BinaryExpr::new(
            Box::new(col("a").eq(lit(1i64))),
            Operator::Or,
            Box::new(col("b").eq(lit(2i64))),
        ));
        assert!(
            translate_filters(std::slice::from_ref(&or)).is_empty(),
            "OR must defer to DataFusion"
        );
        let filters = [Expr::BinaryExpr(datafusion_expr::BinaryExpr::new(
            Box::new(col("c").eq(lit(3i64))),
            Operator::And,
            Box::new(or),
        ))];
        let pd = translate_filters(&filters);
        assert_eq!(pd.len(), 1);
        assert_eq!(pd[0].0, "c");
    }

    /// The schema rows imply: every member rebuilt and inferred, as the
    /// label table did before SQL-2.
    fn schema_of_rows(lmpg: &Lmpg, snap: &ReadView, label: &str) -> Vec<(String, DataType)> {
        let rows: Vec<Json> = try_access::label_members(snap, lmpg.tenant, label)
            .unwrap()
            .into_iter()
            .map(|ord| {
                crate::externalize_record(
                    reconstruct::reconstruct_record(snap, lmpg.tenant, ord)
                        .unwrap()
                        .unwrap(),
                )
            })
            .collect();
        let schema = infer_json_schema_from_iterator(rows.into_iter().map(Ok)).unwrap();
        let mut fields: Vec<(String, DataType)> = schema
            .fields()
            .iter()
            .map(|f| (f.name().clone(), f.data_type().clone()))
            .collect();
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        fields
    }

    #[test]
    fn the_catalog_schema_matches_the_one_rows_imply() {
        use rushdb_lmpg::plan::FieldChange;
        use serde_json::json;
        let lmpg = Lmpg::new(
            Arc::new(rushdb_storage::Store::new_in_memory()),
            TenantId::single(),
        );
        let records = [
            json!({"n": 1, "mixed": 2, "text": "a", "flag": true, "tags": ["x"]}),
            json!({"n": 1.5, "mixed": "two", "nothing": null, "empty": []}),
            json!({"late": 7, "tags": "solo", "when": "2024-01-02T03:04:05Z"}),
        ];
        for record in records {
            let changes = record
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), FieldChange::Set(v.clone())))
                .collect();
            lmpg.create_record("Doc", None, changes).unwrap();
        }
        // Another label's properties are not this label's columns.
        lmpg.create_record(
            "Other",
            None,
            vec![("only".into(), FieldChange::Set(json!(1)))],
        )
        .unwrap();
        let snap = lmpg.store.read_view().unwrap();
        let (schema, _) = label_columns(&snap, lmpg.tenant, "Doc", &|_| true).unwrap();
        let mut fields: Vec<(String, DataType)> = schema
            .fields()
            .iter()
            .map(|f| (f.name().clone(), f.data_type().clone()))
            .collect();
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        // One known difference: an array that is only ever empty has no
        // element values, so rows type its elements `Null`; the catalog
        // types them by the property's definition. Both read `[]` rows.
        let mut from_rows = schema_of_rows(&lmpg, &snap, "Doc");
        for (name, ty) in &mut from_rows {
            if name == "empty" {
                assert_eq!(
                    *ty,
                    DataType::List(Arc::new(Field::new_list_field(DataType::Null, true)))
                );
                *ty = DataType::List(Arc::new(Field::new_list_field(DataType::Utf8, true)));
            }
        }
        assert_eq!(fields, from_rows);
        assert!(fields.iter().all(|(name, _)| name != "only"));
    }

    #[test]
    fn pushdown_numeric_comparison_preserves_integer_precision() {
        let row = serde_json::json!({"score": 9_007_199_254_740_993i64});
        let pushdown = vec![(
            "score".to_owned(),
            CmpOp::Gt,
            datafusion::common::ScalarValue::Float64(Some(9_007_199_254_740_992.0)),
        )];
        assert!(eval_pushdown(&row, &pushdown));
    }

    #[test]
    fn row_json_budget_rejects_cumulative_batch_growth_before_retention() {
        let row = serde_json::json!({"value": "x".repeat(MAX_BATCH_JSON_BYTES / 2)});
        let first = admit_row_json_bytes(0, &row).expect("first row fits");
        let error = admit_row_json_bytes(first, &row).unwrap_err();
        assert!(error.to_string().contains("JSON budget"));
    }
}
