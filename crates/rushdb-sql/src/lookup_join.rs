//! Lookup joins: an inner equi-join against a store table on the key the
//! store can find by (a record's `__id`, a relationship's `__source_id` or
//! `__target_id`) fetches the matching rows for each batch of the other
//! side, instead of scanning the table into a hash table.
//!
//! `DataFusion` plans every join as a hash join that collects one side in
//! memory and scans the other in full. Joined on such a key, a store table
//! answers the other side's keys directly: records by id, relationships by
//! their endpoints' adjacency. The join then reads only the rows that
//! match, holds one batch at a time, and never builds a hash table of a
//! large label, so its cost follows the other side, not the table.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use arrow::array::{Array, RecordBatch, UInt32Array};
use arrow::datatypes::SchemaRef;
use datafusion::common::config::ConfigOptions;
use datafusion::common::stats::Precision;
use datafusion::common::tree_node::{Transformed, TreeNode};
use datafusion::common::{DataFusionError, JoinType, Result as DFResult, Statistics};
use datafusion::execution::memory_pool::{MemoryConsumer, MemoryReservation};
use datafusion::execution::TaskContext;
use datafusion::physical_expr::expressions::Column;
use datafusion::physical_expr::{EquivalenceProperties, PhysicalExpr};
use datafusion::physical_optimizer::PhysicalOptimizerRule;
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::filter::FilterExec;
use datafusion::physical_plan::joins::HashJoinExec;
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, Partitioning, PlanProperties,
    SendableRecordBatchStream,
};
use datafusion_expr::Expr;
use futures::StreamExt;
use rushdb_types::value::{ID_FIELD, SOURCE_FIELD, TARGET_FIELD};

use crate::edge_provider::EdgeLookup;
use crate::scan_provider::LabelLookup;

/// Keys of the other side looked up at a time: bounds the rows one lookup
/// holds (a relationship lookup returns every edge of each key).
const LOOKUP_KEYS: usize = 512;

/// The table a scan reads.
#[derive(Clone, Debug)]
pub(crate) enum ScanTarget {
    Label(LabelLookup),
    Edge(EdgeLookup),
}

/// A store table scan: the streaming scan itself, plus what a lookup join
/// needs to fetch the same table's rows by key and the row count the
/// planner knows for it.
#[derive(Debug)]
pub(crate) struct RushScanExec {
    inner: Arc<dyn ExecutionPlan>,
    target: ScanTarget,
    projection: Option<Vec<usize>>,
    filters: Vec<Expr>,
    rows: usize,
    statistics: Arc<Statistics>,
}

impl RushScanExec {
    pub(crate) fn new(
        inner: Arc<dyn ExecutionPlan>,
        target: ScanTarget,
        projection: Option<Vec<usize>>,
        filters: Vec<Expr>,
        rows: usize,
    ) -> Self {
        let statistics = Arc::new(Statistics {
            num_rows: Precision::Inexact(rows),
            ..Statistics::new_unknown(&inner.schema())
        });
        Self {
            inner,
            target,
            projection,
            filters,
            rows,
            statistics,
        }
    }

    /// The column a lookup can find this table's rows by, if `name` is one.
    fn lookup_key(&self, name: &str) -> bool {
        match self.target {
            ScanTarget::Label(_) => name == ID_FIELD,
            ScanTarget::Edge(_) => name == SOURCE_FIELD || name == TARGET_FIELD,
        }
    }

    /// The rows of this scan whose `key` column is one of `keys`, in
    /// batches, with each row's key.
    fn fetch(&self, keys: Vec<String>, key: &str) -> DFResult<Fetched> {
        Ok(match &self.target {
            ScanTarget::Label(label) => {
                let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                Fetched::Label(Some(label.fetch(
                    &keys,
                    self.projection.as_ref(),
                    &self.filters,
                )?))
            }
            ScanTarget::Edge(edge) => {
                Fetched::Edge(Box::new(edge.fetch(keys, key, self.projection.as_ref())))
            }
        })
    }

    fn expired(&self) -> bool {
        match &self.target {
            ScanTarget::Label(label) => label.expired(),
            ScanTarget::Edge(edge) => edge.expired(),
        }
    }
}

/// A fetch's batches: a label's records of one chunk of keys (at most one
/// per key), or a relationship type's edges a scan batch at a time.
enum Fetched {
    Label(Option<(RecordBatch, Vec<String>)>),
    Edge(Box<crate::edge_provider::EdgeFetch>),
}

impl Iterator for Fetched {
    type Item = DFResult<(RecordBatch, Vec<String>)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Label(batch) => batch.take().map(Ok),
            Self::Edge(edges) => edges.next(),
        }
    }
}

impl DisplayAs for RushScanExec {
    fn fmt_as(&self, t: DisplayFormatType, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target {
            ScanTarget::Label(label) => write!(f, "RushScan: label={}, ", label.label())?,
            ScanTarget::Edge(edge) => write!(f, "RushScan: relationship={}, ", edge.rel_type())?,
        }
        self.inner.fmt_as(t, f)
    }
}

impl ExecutionPlan for RushScanExec {
    fn name(&self) -> &'static str {
        "RushScanExec"
    }

    fn properties(&self) -> &Arc<PlanProperties> {
        self.inner.properties()
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        Vec::new()
    }

    fn with_new_children(
        self: Arc<Self>,
        _children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> DFResult<Arc<dyn ExecutionPlan>> {
        Ok(self)
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> DFResult<SendableRecordBatchStream> {
        self.inner.execute(partition, context)
    }

    fn partition_statistics(&self, _partition: Option<usize>) -> DFResult<Arc<Statistics>> {
        Ok(Arc::clone(&self.statistics))
    }
}

/// An inner equi-join whose `scan` side is fetched by key for each batch of
/// the `stream` side. Output columns are the hash join's: left then right
/// (`scan_on_left` says which the scan is), then its projection.
#[derive(Debug)]
pub(crate) struct LookupJoinExec {
    stream: Arc<dyn ExecutionPlan>,
    stream_key: usize,
    scan: Arc<RushScanExec>,
    scan_key: String,
    /// A filter the hash join applied to the scan's rows (the table's
    /// conditions the scan could not answer itself).
    scan_filter: Option<Arc<dyn PhysicalExpr>>,
    scan_on_left: bool,
    projection: Option<Vec<usize>>,
    properties: Arc<PlanProperties>,
}

impl DisplayAs for LookupJoinExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LookupJoinExec: key={}, scan=", self.scan_key)?;
        match &self.scan.target {
            ScanTarget::Label(label) => write!(f, "label {}", label.label()),
            ScanTarget::Edge(edge) => write!(f, "relationship {}", edge.rel_type()),
        }
    }
}

impl ExecutionPlan for LookupJoinExec {
    fn name(&self) -> &'static str {
        "LookupJoinExec"
    }

    fn properties(&self) -> &Arc<PlanProperties> {
        &self.properties
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![&self.stream]
    }

    fn with_new_children(
        self: Arc<Self>,
        mut children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> DFResult<Arc<dyn ExecutionPlan>> {
        let stream = children
            .pop()
            .ok_or_else(|| DataFusionError::Internal("LookupJoinExec takes one child".into()))?;
        Ok(Arc::new(Self {
            stream,
            stream_key: self.stream_key,
            scan: Arc::clone(&self.scan),
            scan_key: self.scan_key.clone(),
            scan_filter: self.scan_filter.clone(),
            scan_on_left: self.scan_on_left,
            projection: self.projection.clone(),
            properties: Arc::clone(&self.properties),
        }))
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> DFResult<SendableRecordBatchStream> {
        let reservation = MemoryConsumer::new(format!("LookupJoinExec[{partition}]"))
            .register(context.memory_pool());
        let input = self.stream.execute(partition, context)?;
        let join = Lookup {
            stream_key: self.stream_key,
            scan: Arc::clone(&self.scan),
            scan_key: self.scan_key.clone(),
            scan_filter: self.scan_filter.clone(),
            scan_on_left: self.scan_on_left,
            projection: self.projection.clone(),
            schema: self.schema(),
        };
        let state = LookupState {
            input,
            join,
            reservation,
            chunk: None,
            done: false,
        };
        let output = futures::stream::unfold(state, |mut state| async move {
            let next = state.next().await;
            if matches!(next, Some(Err(_))) {
                state.done = true;
            }
            next.map(|batch| (batch, state))
        });
        Ok(Box::pin(RecordBatchStreamAdapter::new(
            self.schema(),
            output,
        )))
    }

    fn partition_statistics(&self, _partition: Option<usize>) -> DFResult<Arc<Statistics>> {
        Ok(Arc::new(Statistics::new_unknown(&self.schema())))
    }
}

/// What one lookup join does to each batch of its stream side.
struct Lookup {
    stream_key: usize,
    scan: Arc<RushScanExec>,
    scan_key: String,
    scan_filter: Option<Arc<dyn PhysicalExpr>>,
    scan_on_left: bool,
    projection: Option<Vec<usize>>,
    schema: SchemaRef,
}

/// The stream batch being joined, a chunk of its keys at a time.
struct Chunk {
    batch: RecordBatch,
    keys: Vec<String>,
    /// Rows with a key, not yet looked up.
    rows: Vec<usize>,
    /// The current chunk: its rows by key, and its fetch.
    by_key: HashMap<String, Vec<u32>>,
    fetched: Option<Fetched>,
}

struct LookupState {
    input: SendableRecordBatchStream,
    join: Lookup,
    /// What the step in flight holds (the fetched rows and the joined
    /// output), charged to the statement's pool.
    reservation: MemoryReservation,
    chunk: Option<Chunk>,
    done: bool,
}

impl LookupState {
    /// The next joined batch: one fetched batch of the current chunk joined
    /// to its stream rows, so a key with many matches is joined a scan
    /// batch at a time.
    async fn next(&mut self) -> Option<DFResult<RecordBatch>> {
        while !self.done {
            if self.join.scan.expired() {
                return Some(Err(DataFusionError::External(Box::new(
                    crate::SqlError::Cancelled,
                ))));
            }
            let Some(chunk) = &mut self.chunk else {
                match self.input.next().await? {
                    Ok(batch) => match self.join.chunk(batch) {
                        Ok(chunk) => self.chunk = Some(chunk),
                        Err(error) => return Some(Err(error)),
                    },
                    Err(error) => return Some(Err(error)),
                }
                continue;
            };
            if chunk.fetched.is_none() {
                let rows: Vec<usize> = chunk
                    .rows
                    .drain(..chunk.rows.len().min(LOOKUP_KEYS))
                    .collect();
                if rows.is_empty() {
                    self.chunk = None;
                    continue;
                }
                chunk.by_key.clear();
                for &row in &rows {
                    chunk
                        .by_key
                        .entry(chunk.keys[row].clone())
                        .or_default()
                        .push(u32::try_from(row).unwrap_or(u32::MAX));
                }
                let keys = chunk.by_key.keys().cloned().collect();
                match self.join.scan.fetch(keys, &self.join.scan_key) {
                    Ok(fetched) => chunk.fetched = Some(fetched),
                    Err(error) => return Some(Err(error)),
                }
            }
            let Some(found) = chunk.fetched.as_mut().and_then(Iterator::next) else {
                chunk.fetched = None;
                continue;
            };
            let joined = found.and_then(|(found, found_keys)| {
                self.reservation.try_resize(found.get_array_memory_size())?;
                let joined = self
                    .join
                    .join(&chunk.batch, &chunk.by_key, found, &found_keys)?;
                if let Some(joined) = &joined {
                    self.reservation
                        .try_resize(joined.get_array_memory_size())?;
                }
                Ok(joined)
            });
            match joined {
                Ok(Some(batch)) => return Some(Ok(batch)),
                Ok(None) => {}
                Err(error) => return Some(Err(error)),
            }
        }
        None
    }
}

impl Lookup {
    /// `batch` ready to join: its keys (by position: a join chain's output
    /// repeats unqualified names such as `__id`, so a name can point at
    /// another table's key) and the rows that have one.
    fn chunk(&self, batch: RecordBatch) -> DFResult<Chunk> {
        let keys = crate::scan_provider::string_column(&batch, self.stream_key)?;
        let nulls = batch.column(self.stream_key).logical_nulls();
        let rows: Vec<usize> = (0..batch.num_rows())
            .filter(|&row| nulls.as_ref().is_none_or(|nulls| nulls.is_valid(row)))
            .collect();
        Ok(Chunk {
            batch,
            keys,
            rows,
            by_key: HashMap::new(),
            fetched: None,
        })
    }

    /// The fetched rows `found` (with their keys) joined to the stream rows
    /// of `batch` that share a key, or `None` when none survives.
    fn join(
        &self,
        batch: &RecordBatch,
        by_key: &HashMap<String, Vec<u32>>,
        mut found: RecordBatch,
        found_keys: &[String],
    ) -> DFResult<Option<RecordBatch>> {
        let mut kept: Vec<&str> = found_keys.iter().map(String::as_str).collect();
        if let Some(filter) = &self.scan_filter {
            let mask = filter.evaluate(&found)?.into_array(found.num_rows())?;
            let mask = arrow::array::as_boolean_array(&mask);
            kept = kept
                .into_iter()
                .enumerate()
                .filter(|(row, _)| mask.is_valid(*row) && mask.value(*row))
                .map(|(_, key)| key)
                .collect();
            found = arrow::compute::filter_record_batch(&found, mask)?;
        }
        let (mut stream_rows, mut scan_rows) = (Vec::new(), Vec::new());
        for (found_row, key) in kept.iter().enumerate() {
            for &row in by_key.get(*key).map_or(&[][..], Vec::as_slice) {
                stream_rows.push(row);
                scan_rows.push(u32::try_from(found_row).unwrap_or(u32::MAX));
            }
        }
        if stream_rows.is_empty() {
            return Ok(None);
        }
        let (stream_rows, scan_rows) =
            (UInt32Array::from(stream_rows), UInt32Array::from(scan_rows));
        let take =
            |source: &RecordBatch, rows: &UInt32Array| -> DFResult<Vec<arrow::array::ArrayRef>> {
                source
                    .columns()
                    .iter()
                    .map(|column| Ok(arrow::compute::take(column, rows, None)?))
                    .collect()
            };
        let (stream_columns, scan_columns) =
            (take(batch, &stream_rows)?, take(&found, &scan_rows)?);
        let (mut columns, rest) = if self.scan_on_left {
            (scan_columns, stream_columns)
        } else {
            (stream_columns, scan_columns)
        };
        columns.extend(rest);
        let columns = match &self.projection {
            Some(projection) => projection
                .iter()
                .map(|&i| Arc::clone(&columns[i]))
                .collect(),
            None => columns,
        };
        // `count(*)` joins project no column: the rows are still counted.
        let options =
            arrow::array::RecordBatchOptions::new().with_row_count(Some(stream_rows.len()));
        Ok(Some(RecordBatch::try_new_with_options(
            Arc::clone(&self.schema),
            columns,
            &options,
        )?))
    }
}

/// Replaces an inner hash join on a store table's lookup key with a
/// [`LookupJoinExec`] when the other side is not larger than the table
/// (or its size is unknown: a lookup join holds no hash table, so it never
/// costs more memory than the join it replaces).
#[derive(Debug, Default)]
pub(crate) struct LookupJoins;

impl PhysicalOptimizerRule for LookupJoins {
    fn optimize(
        &self,
        plan: Arc<dyn ExecutionPlan>,
        _config: &ConfigOptions,
    ) -> DFResult<Arc<dyn ExecutionPlan>> {
        plan.transform_up(|plan| {
            Ok(match lookup_join(&plan) {
                Some(join) => Transformed::yes(join),
                None => Transformed::no(plan),
            })
        })
        .map(|transformed| transformed.data)
    }

    fn name(&self) -> &'static str {
        "rushdb_lookup_joins"
    }

    fn schema_check(&self) -> bool {
        true
    }
}

/// A scan a lookup join can fetch from, and the filter applied over it.
type ScanSide = (Arc<RushScanExec>, Option<Arc<dyn PhysicalExpr>>);

/// The scan under `plan` (a scan, or a filter over one) and the filter.
fn scan_of(plan: &Arc<dyn ExecutionPlan>) -> Option<ScanSide> {
    if plan.downcast_ref::<RushScanExec>().is_some() {
        return downcast_arc(plan).map(|scan| (scan, None));
    }
    let filter = plan.downcast_ref::<FilterExec>()?;
    if filter.projection().is_some() {
        return None;
    }
    downcast_arc(filter.input()).map(|scan| (scan, Some(Arc::clone(filter.predicate()))))
}

fn downcast_arc(plan: &Arc<dyn ExecutionPlan>) -> Option<Arc<RushScanExec>> {
    let scan = plan.downcast_ref::<RushScanExec>()?;
    Some(Arc::new(RushScanExec {
        inner: Arc::clone(&scan.inner),
        target: scan.target.clone(),
        projection: scan.projection.clone(),
        filters: scan.filters.clone(),
        rows: scan.rows,
        statistics: Arc::clone(&scan.statistics),
    }))
}

fn column_name(expr: &Arc<dyn PhysicalExpr>) -> Option<(usize, &str)> {
    let column = expr.downcast_ref::<Column>()?;
    Some((column.index(), column.name()))
}

fn known_rows(plan: &Arc<dyn ExecutionPlan>) -> Option<usize> {
    plan.partition_statistics(None)
        .ok()
        .and_then(|stats| stats.num_rows.get_value().copied())
}

fn lookup_join(plan: &Arc<dyn ExecutionPlan>) -> Option<Arc<dyn ExecutionPlan>> {
    let join = plan.downcast_ref::<HashJoinExec>()?;
    if join.join_type != JoinType::Inner || join.filter.is_some() || join.null_aware {
        return None;
    }
    let [(left_key, right_key)] = join.on.as_slice() else {
        return None;
    };
    // Each side the other could be looked up from: (scan side is left,
    // the scan, its filter, the scan's key column, the other side, the
    // other side's key index).
    let mut options = Vec::new();
    for (scan_on_left, side, side_key, other, other_key) in [
        (false, &join.right, right_key, &join.left, left_key),
        (true, &join.left, left_key, &join.right, right_key),
    ] {
        let (Some((scan, filter)), Some((_, key)), Some((other_index, _))) =
            (scan_of(side), column_name(side_key), column_name(other_key))
        else {
            continue;
        };
        if !scan.lookup_key(key) {
            continue;
        }
        let other_rows = known_rows(other);
        if other_rows.is_some_and(|rows| rows > scan.rows) {
            continue;
        }
        options.push((
            scan_on_left,
            scan,
            filter,
            key.to_owned(),
            Arc::clone(other),
            other_index,
        ));
    }
    // Look up the larger table, streaming the smaller side.
    let (scan_on_left, scan, scan_filter, scan_key, stream, stream_key) =
        options.into_iter().max_by_key(|option| option.1.rows)?;
    Some(Arc::new(LookupJoinExec {
        stream_key,
        scan,
        scan_key,
        scan_filter,
        scan_on_left,
        projection: join
            .projection
            .as_ref()
            .map(|projection| projection.to_vec()),
        // Output follows each fetch's order, not either input's: claim no
        // ordering, and the stream's partition count.
        properties: Arc::new(PlanProperties::new(
            EquivalenceProperties::new(join.schema()),
            Partitioning::UnknownPartitioning(stream.properties().partitioning.partition_count()),
            EmissionType::Incremental,
            Boundedness::Bounded,
        )),
        stream,
    }))
}
