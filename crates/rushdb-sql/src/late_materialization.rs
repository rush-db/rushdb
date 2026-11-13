//! Late materialization: a top-k query reads the columns it orders and
//! deduplicates by, and fetches its other label columns only for the rows it
//! returns.
//!
//! `SELECT DISTINCT r.__id, r.stars, r.text, b.name ... ORDER BY r.stars
//! LIMIT 100` would read every matching review's `text` and group on it, to
//! keep 100 rows. A label column is fixed by its record's `__id`, so when the
//! output carries that `__id` and the column takes no part in filtering,
//! joining or ordering, the top-k can run without it (deduplicating on the
//! `__id` is the same as on the column too) and the column is fetched for the
//! k survivors by id afterwards (a lookup join, `lookup_join.rs`).

use datafusion::common::tree_node::{Transformed, TreeNode, TreeNodeRecursion};
use datafusion::common::{Column, Result, TableReference};
use datafusion::datasource::source_as_provider;
use datafusion::optimizer::{OptimizerConfig, OptimizerRule};
use datafusion_expr::logical_plan::{Aggregate, JoinType, LogicalPlan, LogicalPlanBuilder, Sort};
use datafusion_expr::{Expr, SortExpr};
use rushdb_types::value::ID_FIELD;

use crate::scan_provider::LabelTable;

#[derive(Debug, Default)]
pub(crate) struct LateMaterialization;

impl OptimizerRule for LateMaterialization {
    fn name(&self) -> &'static str {
        "rushdb_late_materialization"
    }

    fn rewrite(
        &self,
        plan: LogicalPlan,
        _config: &dyn OptimizerConfig,
    ) -> Result<Transformed<LogicalPlan>> {
        plan.transform_down(|plan| {
            Ok(match defer(&plan)? {
                Some(rewritten) => Transformed::new(rewritten, true, TreeNodeRecursion::Jump),
                None => Transformed::no(plan),
            })
        })
    }
}

/// The base column an output column of a projection carries unchanged.
fn carried(expr: &Expr) -> Option<&Column> {
    match expr {
        Expr::Column(column) => Some(column),
        Expr::Alias(alias) => carried(&alias.expr),
        _ => None,
    }
}

/// The top of a top-k query: an optional `DISTINCT` (a group-only
/// aggregate) over an optional projection of carried columns.
struct Carrier<'p> {
    aggregate: Option<&'p Aggregate>,
    projection: Option<&'p datafusion_expr::logical_plan::Projection>,
    base: &'p LogicalPlan,
}

fn carrier(plan: &LogicalPlan) -> Option<Carrier<'_>> {
    let (aggregate, below) = match plan {
        LogicalPlan::Aggregate(aggregate)
            if aggregate.aggr_expr.is_empty()
                && aggregate
                    .group_expr
                    .iter()
                    .all(|expr| matches!(expr, Expr::Column(_))) =>
        {
            (Some(aggregate), aggregate.input.as_ref())
        }
        LogicalPlan::Aggregate(_) => return None,
        other => (None, other),
    };
    match below {
        LogicalPlan::Projection(projection)
            if projection.expr.iter().all(|expr| carried(expr).is_some()) =>
        {
            Some(Carrier {
                aggregate,
                projection: Some(projection),
                base: projection.input.as_ref(),
            })
        }
        base if aggregate.is_some() => Some(Carrier {
            aggregate,
            projection: None,
            base,
        }),
        _ => None,
    }
}

impl Carrier<'_> {
    /// Each output column of the top as (output column, base column).
    fn outputs(&self) -> Vec<(Column, Column)> {
        let projected: Vec<(Column, Column)> = match self.projection {
            Some(projection) => projection
                .schema
                .columns()
                .into_iter()
                .zip(&projection.expr)
                .filter_map(|(out, expr)| Some((out, carried(expr)?.clone())))
                .collect(),
            None => self
                .base
                .schema()
                .columns()
                .into_iter()
                .map(|column| (column.clone(), column))
                .collect(),
        };
        match self.aggregate {
            None => projected,
            Some(aggregate) => aggregate
                .group_expr
                .iter()
                .filter_map(|expr| match expr {
                    Expr::Column(column) => projected
                        .iter()
                        .find(|(out, _)| out == column)
                        .map(|(_, base)| (column.clone(), base.clone())),
                    _ => None,
                })
                .collect(),
        }
    }
}

/// The label scan the base reads under `qualifier`, if it is one.
fn label_scan<'p>(
    base: &'p LogicalPlan,
    qualifier: &TableReference,
) -> Option<&'p datafusion_expr::logical_plan::TableScan> {
    let mut found = None;
    base.apply(|plan| {
        let scan = match plan {
            LogicalPlan::SubqueryAlias(alias) if &alias.alias == qualifier => {
                match alias.input.as_ref() {
                    LogicalPlan::TableScan(scan) => Some(scan),
                    _ => None,
                }
            }
            LogicalPlan::TableScan(scan) if &scan.table_name == qualifier => Some(scan),
            _ => None,
        };
        if let Some(scan) = scan {
            let is_label = source_as_provider(&scan.source)
                .is_ok_and(|provider| provider.downcast_ref::<LabelTable>().is_some());
            if is_label {
                found = Some(scan);
            }
            return Ok(TreeNodeRecursion::Stop);
        }
        Ok(TreeNodeRecursion::Continue)
    })
    .ok()?;
    found
}

/// Whether an outer join of the base can pad the table under `qualifier`
/// with nulls. Its deferred columns would come back by an inner join on its
/// `__id`, which a null `__id` never matches: the padded rows would be lost.
fn padded_by_outer_join(base: &LogicalPlan, qualifier: &TableReference) -> bool {
    let mut padded = false;
    let _ = base.apply(|plan| {
        if let LogicalPlan::Join(join) = plan {
            let (left, right) = match join.join_type {
                JoinType::Inner => (false, false),
                JoinType::Left => (false, true),
                JoinType::Right => (true, false),
                // Full pads both sides; semi, anti and mark joins are not
                // worth deferring through.
                _ => (true, true),
            };
            padded = (left && label_scan(&join.left, qualifier).is_some())
                || (right && label_scan(&join.right, qualifier).is_some());
        }
        Ok(if padded {
            TreeNodeRecursion::Stop
        } else {
            TreeNodeRecursion::Continue
        })
    });
    padded
}

/// Whether the base computes with `column` (a join key, a filter, an
/// expression), beyond reading and passing it on.
fn computes_with(base: &LogicalPlan, column: &Column) -> bool {
    let mut used = false;
    let _ = base.apply(|plan| {
        let exprs: Vec<Expr> = match plan {
            LogicalPlan::TableScan(_) | LogicalPlan::SubqueryAlias(_) => Vec::new(),
            LogicalPlan::Projection(projection) => projection
                .expr
                .iter()
                .filter(|expr| carried(expr).is_none())
                .cloned()
                .collect(),
            other => other.expressions(),
        };
        used = exprs.iter().any(|expr| expr.column_refs().contains(column));
        Ok(if used {
            TreeNodeRecursion::Stop
        } else {
            TreeNodeRecursion::Continue
        })
    });
    used
}

/// The columns of one label table a top-k fetches after its cut.
struct Deferred {
    qualifier: TableReference,
    /// The top's output carrying the table's `__id`.
    id_out: Column,
    /// The deferred columns as (output, base column).
    columns: Vec<(Column, Column)>,
}

/// The top's label columns that can wait for the cut: carried unchanged,
/// fixed by a record `__id` the top also outputs, not ordered by, and never
/// null-padded by an outer join.
fn deferrable(
    top: &Carrier<'_>,
    outputs: &[(Column, Column)],
    order: &[SortExpr],
) -> Vec<Deferred> {
    let ordered: Vec<Column> = order
        .iter()
        .flat_map(|sort| sort.expr.column_refs().into_iter().cloned())
        .collect();
    let mut deferred: Vec<Deferred> = Vec::new();
    for (out, base) in outputs {
        let Some(qualifier) = &base.relation else {
            continue;
        };
        if base.name == ID_FIELD || ordered.contains(out) || computes_with(top.base, base) {
            continue;
        }
        let Some((id_out, _)) = outputs.iter().find(|(_, other)| {
            other.relation.as_ref() == Some(qualifier) && other.name == ID_FIELD
        }) else {
            continue;
        };
        if label_scan(top.base, qualifier).is_none() || padded_by_outer_join(top.base, qualifier) {
            continue;
        }
        match deferred
            .iter_mut()
            .find(|table| &table.qualifier == qualifier)
        {
            Some(table) => table.columns.push((out.clone(), base.clone())),
            None => deferred.push(Deferred {
                qualifier: qualifier.clone(),
                id_out: id_out.clone(),
                columns: vec![(out.clone(), base.clone())],
            }),
        }
    }
    deferred
}

/// `narrow` joined to each deferred table's columns by `__id`, and the
/// alias each table's columns read from.
type Fetched = (LogicalPlan, Vec<(TableReference, Vec<(Column, Column)>)>);

fn fetch_deferred(
    base: &LogicalPlan,
    narrow: LogicalPlan,
    deferred: &[Deferred],
) -> Result<Option<Fetched>> {
    let mut joined = narrow;
    let mut late = Vec::new();
    for (index, table) in deferred.iter().enumerate() {
        let Some(scan) = label_scan(base, &table.qualifier) else {
            return Ok(None);
        };
        let provider_schema = scan.source.schema();
        let mut indices = vec![provider_schema.index_of(ID_FIELD)?];
        for (_, column) in &table.columns {
            indices.push(provider_schema.index_of(&column.name)?);
        }
        let alias = TableReference::bare(format!("__rushdb_late_{index}"));
        let rescan =
            LogicalPlanBuilder::scan(scan.table_name.clone(), scan.source.clone(), Some(indices))?
                .alias(alias.clone())?
                .build()?;
        joined = LogicalPlanBuilder::from(joined)
            .join_with_expr_keys(
                rescan,
                JoinType::Inner,
                (
                    vec![Expr::Column(table.id_out.clone())],
                    vec![Expr::Column(Column::new(Some(alias.clone()), ID_FIELD))],
                ),
                None,
            )?
            .build()?;
        late.push((alias, table.columns.clone()));
    }
    Ok(Some((joined, late)))
}

/// `plan` rewritten to defer its wide label columns past its top-k, or
/// `None` when it is not a top-k over carried columns or nothing defers.
fn defer(plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
    let LogicalPlan::Sort(Sort {
        expr: order,
        input,
        fetch: Some(fetch),
    }) = plan
    else {
        return Ok(None);
    };
    let Some(top) = carrier(input) else {
        return Ok(None);
    };
    let outputs = top.outputs();
    let deferred = deferrable(&top, &outputs, order);
    if deferred.is_empty() {
        return Ok(None);
    }
    let is_deferred = |out: &Column| {
        deferred
            .iter()
            .any(|table| table.columns.iter().any(|(o, _)| o == out))
    };
    // The top without the deferred columns.
    let narrow = {
        let mut builder = LogicalPlanBuilder::from(top.base.clone());
        if let Some(projection) = top.projection {
            let kept: Vec<Expr> = projection
                .schema
                .columns()
                .into_iter()
                .zip(&projection.expr)
                .filter(|(out, _)| !is_deferred(out))
                .map(|(_, expr)| expr.clone())
                .collect();
            builder = builder.project(kept)?;
        }
        if let Some(aggregate) = top.aggregate {
            let groups: Vec<Expr> = aggregate
                .group_expr
                .iter()
                .filter(|expr| !matches!(expr, Expr::Column(column) if is_deferred(column)))
                .cloned()
                .collect();
            builder = builder.aggregate(groups, Vec::<Expr>::new())?;
        }
        builder
            .sort_with_limit(order.clone(), Some(*fetch))?
            .build()?
    };
    // The deferred columns, fetched by id for the k rows.
    let Some((joined, late)) = fetch_deferred(top.base, narrow, &deferred)? else {
        return Ok(None);
    };
    // The top's columns in its order, the deferred ones from the fetch.
    let columns: Vec<Expr> = outputs
        .iter()
        .map(|(out, _)| {
            late.iter()
                .find_map(|(alias, columns)| {
                    columns.iter().find(|(o, _)| o == out).map(|(_, base)| {
                        Expr::Column(Column::new(Some(alias.clone()), base.name.clone()))
                            .alias_qualified(out.relation.clone(), out.name.clone())
                    })
                })
                .unwrap_or_else(|| Expr::Column(out.clone()))
        })
        .collect();
    Ok(Some(
        LogicalPlanBuilder::from(joined)
            .project(columns)?
            .sort_with_limit(order.clone(), Some(*fetch))?
            .build()?,
    ))
}

/// Counting distinct rows by key: `SELECT count(*) FROM (SELECT DISTINCT
/// r.__id, r.text, ...)` counts the same rows when the `DISTINCT` groups by
/// `r.__id` alone instead of `r.__id` and every other column of `r`: a
/// label column is fixed by its record's `__id`. An aggregate that reads no
/// column of its input (a count of rows) lets the `DISTINCT` under it drop
/// those columns, so it neither reads nor holds them.
#[derive(Debug, Default)]
pub(crate) struct DistinctKeys;

impl OptimizerRule for DistinctKeys {
    fn name(&self) -> &'static str {
        "rushdb_distinct_keys"
    }

    fn rewrite(
        &self,
        plan: LogicalPlan,
        _config: &dyn OptimizerConfig,
    ) -> Result<Transformed<LogicalPlan>> {
        plan.transform_down(|plan| {
            Ok(match reduce_counted_distinct(&plan)? {
                Some(rewritten) => Transformed::new(rewritten, true, TreeNodeRecursion::Jump),
                None => Transformed::no(plan),
            })
        })
    }
}

/// The group-only aggregate under `plan` through nodes that keep its rows
/// (aliases, carried projections, unlimited sorts).
fn distinct_below(plan: &LogicalPlan) -> Option<&Aggregate> {
    match plan {
        LogicalPlan::SubqueryAlias(alias) => distinct_below(&alias.input),
        LogicalPlan::Projection(projection)
            if projection.expr.iter().all(|expr| carried(expr).is_some()) =>
        {
            distinct_below(&projection.input)
        }
        LogicalPlan::Sort(sort) if sort.fetch.is_none() => distinct_below(&sort.input),
        LogicalPlan::Aggregate(aggregate)
            if aggregate.aggr_expr.is_empty() && !aggregate.group_expr.is_empty() =>
        {
            Some(aggregate)
        }
        _ => None,
    }
}

fn reduce_counted_distinct(plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
    let LogicalPlan::Aggregate(counting) = plan else {
        return Ok(None);
    };
    let reads_columns = counting
        .group_expr
        .iter()
        .chain(&counting.aggr_expr)
        .any(|expr| !expr.column_refs().is_empty());
    if reads_columns {
        return Ok(None);
    }
    let Some(distinct) = distinct_below(&counting.input) else {
        return Ok(None);
    };
    let distinct_plan = LogicalPlan::Aggregate(distinct.clone());
    let Some(top) = carrier(&distinct_plan) else {
        return Ok(None);
    };
    let outputs = top.outputs();
    let mut kept = Vec::new();
    let mut dropped = false;
    for (out, base) in &outputs {
        let fixed_by_id = base.name != ID_FIELD
            && base.relation.as_ref().is_some_and(|qualifier| {
                outputs.iter().any(|(_, other)| {
                    other.relation.as_ref() == Some(qualifier) && other.name == ID_FIELD
                }) && label_scan(top.base, qualifier).is_some()
            });
        if fixed_by_id {
            dropped = true;
        } else {
            kept.push(Expr::Column(out.clone()));
        }
    }
    if !dropped {
        return Ok(None);
    }
    let reduced = LogicalPlanBuilder::from(distinct.input.as_ref().clone())
        .aggregate(kept, Vec::<Expr>::new())?
        .aggregate(counting.group_expr.clone(), counting.aggr_expr.clone())?
        .build()?;
    Ok(Some(reduced))
}
