//! `DISTINCT ... ORDER BY ... LIMIT k` in O(k) memory.
//!
//! `DataFusion` plans it as a group-only aggregate over every row, then a
//! top-k sort of the groups: the aggregate holds every distinct row (all of
//! a large join's rows) before the sort keeps k of them. Keeping the k
//! smallest distinct rows as they stream gives the same rows: a set ordered
//! by (sort key, row) holds at most k entries, a duplicate row is the same
//! entry, and a row past the k-th sort key is dropped on sight.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::row::{OwnedRow, RowConverter, SortField};
use datafusion::common::config::ConfigOptions;
use datafusion::common::tree_node::{Transformed, TreeNode};
use datafusion::common::{DataFusionError, Result as DFResult};
use datafusion::execution::memory_pool::MemoryConsumer;
use datafusion::execution::TaskContext;
use datafusion::physical_expr::expressions::Column;
use datafusion::physical_expr::{EquivalenceProperties, PhysicalExpr};
use datafusion::physical_optimizer::PhysicalOptimizerRule;
use datafusion::physical_plan::aggregates::{AggregateExec, AggregateMode};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::sorts::sort::SortExec;
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, Partitioning, PlanProperties,
    SendableRecordBatchStream,
};
use futures::StreamExt;

/// The k smallest distinct rows of `input`'s `groups`, ordered by `order`
/// (positions in `groups`, with their sort options).
#[derive(Debug)]
pub(crate) struct DistinctTopKExec {
    input: Arc<dyn ExecutionPlan>,
    groups: Vec<Arc<dyn PhysicalExpr>>,
    order: Vec<(usize, arrow::compute::SortOptions)>,
    fetch: usize,
    properties: Arc<PlanProperties>,
}

impl DisplayAs for DistinctTopKExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DistinctTopKExec: fetch={}", self.fetch)
    }
}

impl ExecutionPlan for DistinctTopKExec {
    fn name(&self) -> &'static str {
        "DistinctTopKExec"
    }

    fn properties(&self) -> &Arc<PlanProperties> {
        &self.properties
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![&self.input]
    }

    fn with_new_children(
        self: Arc<Self>,
        mut children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> DFResult<Arc<dyn ExecutionPlan>> {
        let input = children
            .pop()
            .ok_or_else(|| DataFusionError::Internal("DistinctTopKExec takes one child".into()))?;
        Ok(Arc::new(Self {
            input,
            groups: self.groups.clone(),
            order: self.order.clone(),
            fetch: self.fetch,
            properties: Arc::clone(&self.properties),
        }))
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> DFResult<SendableRecordBatchStream> {
        // The held rows are charged to the statement's pool, like the
        // aggregate and sort this node replaces.
        let reservation = MemoryConsumer::new(format!("DistinctTopKExec[{partition}]"))
            .register(context.memory_pool());
        let mut input = self.input.execute(partition, context)?;
        let schema = self.schema();
        let (groups, order, fetch) = (self.groups.clone(), self.order.clone(), self.fetch);
        let output_schema = Arc::clone(&schema);
        let output = futures::stream::once(async move {
            let types: Vec<_> = schema
                .fields()
                .iter()
                .map(|field| field.data_type().clone())
                .collect();
            let keys = RowConverter::new(
                order
                    .iter()
                    .map(|(index, options)| {
                        SortField::new_with_options(types[*index].clone(), *options)
                    })
                    .collect(),
            )?;
            let rows = RowConverter::new(types.iter().cloned().map(SortField::new).collect())?;
            let mut top: BTreeSet<(OwnedRow, OwnedRow)> = BTreeSet::new();
            while let Some(batch) = input.next().await {
                let batch = batch?;
                let columns = groups
                    .iter()
                    .map(|group| group.evaluate(&batch)?.into_array(batch.num_rows()))
                    .collect::<DFResult<Vec<_>>>()?;
                let ordered: Vec<_> = order
                    .iter()
                    .map(|(index, _)| Arc::clone(&columns[*index]))
                    .collect();
                let (key_rows, full_rows) = (
                    keys.convert_columns(&ordered)?,
                    rows.convert_columns(&columns)?,
                );
                for row in 0..batch.num_rows() {
                    let key = key_rows.row(row);
                    if top.len() >= fetch && top.last().is_some_and(|(last, _)| key > last.row()) {
                        continue;
                    }
                    let entry = (key.owned(), full_rows.row(row).owned());
                    let size = held(&entry);
                    if top.insert(entry) {
                        reservation.try_grow(size)?;
                    }
                    if top.len() > fetch {
                        if let Some(dropped) = top.pop_last() {
                            reservation.shrink(held(&dropped));
                        }
                    }
                }
            }
            let columns = rows.convert_rows(top.iter().map(|(_, row)| row.row()))?;
            drop(top);
            reservation.free();
            Ok(RecordBatch::try_new(schema, columns)?)
        });
        Ok(Box::pin(RecordBatchStreamAdapter::new(
            output_schema,
            output,
        )))
    }
}

/// The bytes one held entry takes: its two encoded rows and the set's
/// per-entry overhead.
fn held((key, row): &(OwnedRow, OwnedRow)) -> usize {
    key.as_ref().len() + row.as_ref().len() + 2 * std::mem::size_of::<OwnedRow>()
}

/// Replaces a top-k sort over a `DISTINCT` (a group-only aggregate whose
/// order keys are group columns) with a [`DistinctTopKExec`].
#[derive(Debug, Default)]
pub(crate) struct DistinctTopK;

impl PhysicalOptimizerRule for DistinctTopK {
    fn optimize(
        &self,
        plan: Arc<dyn ExecutionPlan>,
        _config: &ConfigOptions,
    ) -> DFResult<Arc<dyn ExecutionPlan>> {
        plan.transform_up(|plan| {
            Ok(match distinct_topk(&plan) {
                Some(replaced) => Transformed::yes(replaced),
                None => Transformed::no(plan),
            })
        })
        .map(|transformed| transformed.data)
    }

    fn name(&self) -> &'static str {
        "rushdb_distinct_topk"
    }

    fn schema_check(&self) -> bool {
        true
    }
}

fn distinct_topk(plan: &Arc<dyn ExecutionPlan>) -> Option<Arc<dyn ExecutionPlan>> {
    let sort = plan.downcast_ref::<SortExec>()?;
    let fetch = sort.fetch()?;
    let aggregate = sort.input().downcast_ref::<AggregateExec>()?;
    if !matches!(
        aggregate.mode(),
        AggregateMode::Single | AggregateMode::SinglePartitioned
    ) || !aggregate.aggr_expr().is_empty()
        || !aggregate.group_expr().is_single()
        || aggregate.group_expr().expr().is_empty()
    {
        return None;
    }
    let order = sort
        .expr()
        .iter()
        .map(|sort| Some((sort.expr.downcast_ref::<Column>()?.index(), sort.options)))
        .collect::<Option<Vec<_>>>()?;
    let schema = aggregate.schema();
    let properties = PlanProperties::new(
        EquivalenceProperties::new(Arc::clone(&schema)),
        Partitioning::UnknownPartitioning(1),
        EmissionType::Final,
        Boundedness::Bounded,
    );
    Some(Arc::new(DistinctTopKExec {
        input: Arc::clone(aggregate.input()),
        groups: aggregate
            .group_expr()
            .expr()
            .iter()
            .map(|(expr, _)| Arc::clone(expr))
            .collect(),
        order,
        fetch,
        properties: Arc::new(properties),
    }))
}
