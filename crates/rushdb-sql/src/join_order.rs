//! Join order from the store's counts: a chain of inner equi-joins over
//! store tables is rebuilt from its smallest table outward.
//!
//! `DataFusion` joins tables in the order the statement names them and has
//! no row counts for them, so `review JOIN edges JOIN business WHERE
//! business.is_open = 0` builds its first join over every review and every
//! edge before the filter on business narrows anything. The store knows
//! each table's size exactly (label and relationship-type counters, the
//! planner's count for a pushed condition), so the chain can start where the
//! fewest rows are and reach the large tables only through join keys, which
//! the lookup joins (`lookup_join.rs`) then answer by key.

use std::collections::BTreeSet;

use datafusion::common::tree_node::{Transformed, TreeNode, TreeNodeRecursion};
use datafusion::common::{NullEquality, Result};
use datafusion::datasource::source_as_provider;
use datafusion::optimizer::{OptimizerConfig, OptimizerRule};
use datafusion_expr::logical_plan::{Join, JoinType, LogicalPlan, LogicalPlanBuilder};
use datafusion_expr::Expr;

use crate::edge_provider::EdgeTable;
use crate::scan_provider::LabelTable;

#[derive(Debug, Default)]
pub(crate) struct JoinOrder;

impl OptimizerRule for JoinOrder {
    fn name(&self) -> &'static str {
        "rushdb_join_order"
    }

    fn rewrite(
        &self,
        plan: LogicalPlan,
        _config: &dyn OptimizerConfig,
    ) -> Result<Transformed<LogicalPlan>> {
        plan.transform_down(|plan| {
            Ok(match reorder(&plan)? {
                // The rebuilt chain is already in order: skip its joins.
                Some(reordered) => Transformed::new(reordered, true, TreeNodeRecursion::Jump),
                None => Transformed::no(plan),
            })
        })
    }
}

/// An inner equi-join without a residual filter, which can be reordered
/// with the joins under it.
fn chain_join(plan: &LogicalPlan) -> Option<&Join> {
    match plan {
        LogicalPlan::Join(join)
            if join.join_type == JoinType::Inner
                && join.filter.is_none()
                && !join.on.is_empty()
                && join.null_equality == NullEquality::NullEqualsNothing =>
        {
            Some(join)
        }
        _ => None,
    }
}

/// `plan` without the column-pruning projections the optimizer puts
/// between chained joins: a projection of plain columns over a chain join
/// only drops columns, which the rebuilt chain's final projection drops too.
fn through_pruning(plan: &LogicalPlan) -> &LogicalPlan {
    match plan {
        LogicalPlan::Projection(projection)
            if chain_join(&projection.input).is_some()
                && projection
                    .expr
                    .iter()
                    .all(|expr| matches!(expr, Expr::Column(_))) =>
        {
            through_pruning(&projection.input)
        }
        _ => plan,
    }
}

/// The relations of the join chain under `plan`, left to right, and its
/// join conditions.
fn flatten(plan: &LogicalPlan, relations: &mut Vec<LogicalPlan>, on: &mut Vec<(Expr, Expr)>) {
    let plan = through_pruning(plan);
    match chain_join(plan) {
        Some(join) => {
            flatten(&join.left, relations, on);
            flatten(&join.right, relations, on);
            on.extend(join.on.iter().cloned());
        }
        None => relations.push(plan.clone()),
    }
}

/// Whether the chain under `plan` joins each relation in turn onto the
/// ones before it (every right input is a relation, not a join).
fn left_deep(plan: &LogicalPlan) -> bool {
    match chain_join(through_pruning(plan)) {
        Some(join) => chain_join(through_pruning(&join.right)).is_none() && left_deep(&join.left),
        None => true,
    }
}

/// The rows the store expects from `plan` when it reads one store table
/// (through aliases, projections and filters); `None` otherwise.
fn estimate(plan: &LogicalPlan) -> Result<Option<usize>> {
    match plan {
        LogicalPlan::SubqueryAlias(alias) => estimate(&alias.input),
        LogicalPlan::Projection(projection) => estimate(&projection.input),
        LogicalPlan::Filter(filter) => estimate(&filter.input),
        LogicalPlan::TableScan(scan) => {
            let Ok(provider) = source_as_provider(&scan.source) else {
                return Ok(None);
            };
            if let Some(label) = provider.downcast_ref::<LabelTable>() {
                return label.estimate(&scan.filters).map(Some);
            }
            Ok(provider.downcast_ref::<EdgeTable>().map(EdgeTable::rows))
        }
        _ => Ok(None),
    }
}

/// The relation (by position) whose columns `expr` reads, when it reads
/// one relation's only.
fn relation_of(expr: &Expr, relations: &[LogicalPlan]) -> Option<usize> {
    let columns = expr.column_refs();
    if columns.is_empty() {
        return None;
    }
    let mut owners = relations
        .iter()
        .enumerate()
        .filter(|(_, relation)| {
            columns
                .iter()
                .all(|column| relation.schema().has_column(column))
        })
        .map(|(index, _)| index);
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner)
}

/// The chain under `plan` rebuilt from its smallest relation outward, or
/// `None` when it is not a chain of three or more relations, a condition
/// does not join two of them, or it is in that order already.
fn reorder(plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
    if chain_join(plan).is_none() {
        return Ok(None);
    }
    let (mut relations, mut on) = (Vec::new(), Vec::new());
    flatten(plan, &mut relations, &mut on);
    if relations.len() < 3 {
        return Ok(None);
    }
    // Each condition as (left relation, left key, right relation, right key).
    let mut conditions = Vec::new();
    for (left, right) in on {
        let (Some(a), Some(b)) = (
            relation_of(&left, &relations),
            relation_of(&right, &relations),
        ) else {
            return Ok(None);
        };
        if a == b {
            return Ok(None);
        }
        conditions.push((a, left, b, right));
    }
    // Unknown sizes go last, in statement order.
    let sizes: Vec<usize> = relations
        .iter()
        .map(|relation| Ok(estimate(relation)?.unwrap_or(usize::MAX)))
        .collect::<Result<_>>()?;
    let first = (0..relations.len())
        .min_by_key(|&index| (sizes[index], index))
        .unwrap_or(0);
    let mut order = vec![first];
    let mut joined: BTreeSet<usize> = BTreeSet::from([first]);
    while joined.len() < relations.len() {
        let next = (0..relations.len())
            .filter(|index| !joined.contains(index))
            .filter(|&index| {
                conditions.iter().any(|(a, _, b, _)| {
                    (*a == index && joined.contains(b)) || (*b == index && joined.contains(a))
                })
            })
            .min_by_key(|&index| (sizes[index], index));
        let Some(next) = next else {
            // A relation no condition reaches: a cross join, left alone.
            return Ok(None);
        };
        order.push(next);
        joined.insert(next);
    }
    if order.iter().copied().eq(0..relations.len()) && left_deep(plan) {
        return Ok(None);
    }
    let mut builder = LogicalPlanBuilder::from(relations[order[0]].clone());
    let mut done = BTreeSet::from([order[0]]);
    for &next in &order[1..] {
        let (mut left_keys, mut right_keys) = (Vec::new(), Vec::new());
        for (a, left, b, right) in &conditions {
            if *a == next && done.contains(b) {
                left_keys.push(right.clone());
                right_keys.push(left.clone());
            } else if *b == next && done.contains(a) {
                left_keys.push(left.clone());
                right_keys.push(right.clone());
            }
        }
        builder = builder.join_with_expr_keys(
            relations[next].clone(),
            JoinType::Inner,
            (left_keys, right_keys),
            None,
        )?;
        done.insert(next);
    }
    // The chain's columns in the order the statement's join produced them.
    let columns: Vec<Expr> = plan
        .schema()
        .columns()
        .into_iter()
        .map(Expr::Column)
        .collect();
    Ok(Some(builder.project(columns)?.build()?))
}
