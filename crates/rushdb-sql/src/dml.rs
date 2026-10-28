//! SQL-3: `INSERT`/`UPDATE`/`DELETE` on labels of any size.
//!
//! A DML statement is rewritten into the read query that finds the rows it
//! touches: `UPDATE` selects each matching row's `__id` with the old and new
//! value of every assigned column, `DELETE` selects the matching `__id`s, and
//! `INSERT` runs its `VALUES` or `SELECT` source. The read runs on the
//! streamed label tables of the read path, so its filter is planned on
//! the indexes like any `SELECT`, and only the touched rows are decoded. The
//! implied record writes are committed atomically at the snapshot version, as
//! before; their size (not the label's) is charged to the request budget.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;

use datafusion::sql::parser::{DFParser, Statement as DFStatement};
use datafusion::sql::sqlparser::ast::{
    AssignmentTarget, Expr, FromTable, Ident, ObjectName, SetExpr, Statement, TableFactor,
    TableObject, UpdateTableFromKind, Value,
};
use datafusion::sql::sqlparser::dialect::PostgreSqlDialect;
use futures::StreamExt;
use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::{BatchOp, BatchTx, FieldChange, Lmpg, RecordRef};
use rushdb_storage::ReadView;
use rushdb_textsearch::TextIndexes;
use rushdb_types::value::ID_FIELD;
use serde_json::Value as Json;

use crate::{
    append_batch_rows_with_budget, bind_parameters, canonical_uuid, context_in_view_with_budget,
    map_stream_error, memory_cap, planning_error, with_table_hint, Row, SqlError,
    SqlMutationResult,
};

/// Output column holding the touched row's `__id`.
const ROW_ID: &str = "__rushdb_id";
/// Output column prefixes for an `UPDATE` assignment's old and new value.
const OLD: &str = "__rushdb_old_";
const NEW: &str = "__rushdb_new_";
/// Fixed charge per planned write, beside its encoded values.
const WRITE_OVERHEAD: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DmlKind {
    Insert,
    Update,
    Delete,
}

/// A DML statement as the read that finds its rows.
#[derive(Debug)]
pub(crate) struct DmlPlan {
    pub(crate) kind: DmlKind,
    /// Target table, lowercased.
    pub(crate) target: String,
    /// The read query.
    pub(crate) query: String,
    /// `UPDATE`: the assigned properties, in assignment order. `INSERT`: the
    /// listed target columns (empty: the table's columns, in order).
    pub(crate) columns: Vec<String>,
    /// Bare placeholders bound directly to a target column, in placeholder
    /// order. `Describe` uses this to give the client the column's type when
    /// the rewritten read (a `VALUES` source or an assignment expression)
    /// cannot infer it on its own.
    pub(crate) parameter_columns: Vec<(u64, ParameterTarget)>,
}

/// Where a bare `$n` parameter's value lands in the target table.
#[derive(Debug, Clone)]
pub(crate) enum ParameterTarget {
    /// The column named by the statement's column list or assignment target.
    Column(String),
    /// The table's column at this position (an `INSERT` without a list).
    Position(usize),
}

fn unsupported(message: impl Into<String>) -> SqlError {
    SqlError::UnsupportedMutation(message.into())
}

/// The column an identifier names, as the planner resolves it: quoted
/// identifiers keep their case, unquoted ones are lowercased.
fn column_name(ident: &Ident) -> String {
    if ident.quote_style.is_some() {
        ident.value.clone()
    } else {
        ident.value.to_ascii_lowercase()
    }
}

fn last_ident(name: &ObjectName) -> Result<&Ident, SqlError> {
    name.0
        .last()
        .and_then(|part| part.as_ident())
        .ok_or_else(|| unsupported("computed column names are unsupported"))
}

/// How the select list refers to the target table: its alias, else its name.
fn qualifier(relation: &TableFactor) -> String {
    match relation {
        TableFactor::Table {
            alias: Some(alias), ..
        } => alias.name.to_string(),
        TableFactor::Table { name, .. } => name.to_string(),
        other => other.to_string(),
    }
}

/// Parse one DML statement and plan the read that finds its rows.
///
/// One match over the three statement kinds keeps each rewrite next to its
/// own validation.
///
/// # Errors
/// [`SqlError::UnsupportedMutation`] for anything but a single supported
/// `INSERT`, `UPDATE` or `DELETE` on one record table;
/// [`SqlError::InvalidIdentity`] for an `UPDATE` of `__id`.
#[expect(
    clippy::too_many_lines,
    reason = "one match arm per DML kind; each arm is its own rewrite"
)]
pub(crate) fn plan(sql: &str) -> Result<DmlPlan, SqlError> {
    let statements = DFParser::parse_sql_with_dialect(sql, &PostgreSqlDialect {})
        .map_err(|error| SqlError::Parse(error.to_string()))?;
    if statements.len() != 1 {
        return Err(unsupported(
            "exactly one INSERT, UPDATE, or DELETE statement is required",
        ));
    }
    let DFStatement::Statement(statement) = &statements[0] else {
        return Err(unsupported(statements[0].to_string()));
    };
    match statement.as_ref() {
        Statement::Insert(insert) => {
            let TableObject::TableName(name) = &insert.table else {
                return Err(unsupported("INSERT target must be a record table"));
            };
            let target = crate::unqualified_table_name(name)?;
            if insert.returning.is_some()
                || insert.on.is_some()
                || insert.or.is_some()
                || insert.ignore
                || insert.overwrite
                || insert.replace_into
                || !insert.assignments.is_empty()
                || insert.partitioned.is_some()
            {
                return Err(unsupported(
                    "INSERT supports a column list with VALUES or SELECT only",
                ));
            }
            let source = insert
                .source
                .as_ref()
                .ok_or_else(|| unsupported("INSERT needs VALUES or a SELECT"))?;
            let columns: Vec<String> = insert
                .columns
                .iter()
                .map(|name| last_ident(name).map(column_name))
                .collect::<Result<_, _>>()?;
            let mut parameter_columns = Vec::new();
            if let SetExpr::Values(values) = source.body.as_ref() {
                for row in &values.rows {
                    for (index, expression) in row.iter().enumerate() {
                        let Expr::Value(value) = expression else {
                            continue;
                        };
                        let Value::Placeholder(placeholder) = &value.value else {
                            continue;
                        };
                        let Some(position) = placeholder
                            .strip_prefix('$')
                            .and_then(|value| value.parse::<u64>().ok())
                        else {
                            continue;
                        };
                        let target = if columns.is_empty() {
                            Some(ParameterTarget::Position(index))
                        } else {
                            columns
                                .get(index)
                                .map(|column| ParameterTarget::Column(column.clone()))
                        };
                        let Some(target) = target else { continue };
                        if !parameter_columns.iter().any(|(seen, _)| *seen == position) {
                            parameter_columns.push((position, target));
                        }
                    }
                }
            }
            Ok(DmlPlan {
                kind: DmlKind::Insert,
                target,
                query: source.to_string(),
                columns,
                parameter_columns,
            })
        }
        Statement::Update(update) => {
            if !update.table.joins.is_empty() {
                return Err(unsupported("UPDATE target joins are unsupported"));
            }
            if update.returning.is_some()
                || update.output.is_some()
                || update.or.is_some()
                || !update.order_by.is_empty()
                || update.limit.is_some()
            {
                return Err(unsupported(
                    "UPDATE with RETURNING, OR, ORDER BY or LIMIT is unsupported",
                ));
            }
            let relation = &update.table.relation;
            let target = crate::table_factor_name(relation)?;
            let q = qualifier(relation);
            let mut select = vec![format!("{q}.\"{ID_FIELD}\" AS \"{ROW_ID}\"")];
            let mut columns = Vec::new();
            let mut parameter_columns = Vec::new();
            for (i, assignment) in update.assignments.iter().enumerate() {
                let AssignmentTarget::ColumnName(name) = &assignment.target else {
                    return Err(unsupported("tuple assignments are unsupported"));
                };
                let ident = last_ident(name)?;
                let column = column_name(ident);
                if column == ID_FIELD {
                    return Err(SqlError::InvalidIdentity(
                        "UPDATE cannot change the stable __id column".into(),
                    ));
                }
                if let Expr::Value(value) = &assignment.value {
                    if let Value::Placeholder(placeholder) = &value.value {
                        if let Some(position) = placeholder
                            .strip_prefix('$')
                            .and_then(|value| value.parse::<u64>().ok())
                        {
                            parameter_columns
                                .push((position, ParameterTarget::Column(column.clone())));
                        }
                    }
                }
                select.push(format!("{q}.{ident} AS \"{OLD}{i}\""));
                select.push(format!("({}) AS \"{NEW}{i}\"", assignment.value));
                columns.push(column);
            }
            let mut from = relation.to_string();
            if let Some(
                UpdateTableFromKind::BeforeSet(tables) | UpdateTableFromKind::AfterSet(tables),
            ) = &update.from
            {
                for table in tables {
                    from.push_str(", ");
                    from.push_str(&table.to_string());
                }
            }
            let mut query = format!("SELECT {} FROM {from}", select.join(", "));
            if let Some(selection) = &update.selection {
                let _ = write!(query, " WHERE {selection}");
            }
            Ok(DmlPlan {
                kind: DmlKind::Update,
                target,
                query,
                columns,
                parameter_columns,
            })
        }
        Statement::Delete(delete) => {
            let tables = match &delete.from {
                FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
            };
            if tables.len() != 1 || !tables[0].joins.is_empty() || !delete.tables.is_empty() {
                return Err(unsupported("DELETE must target one record table"));
            }
            if delete.using.is_some()
                || delete.returning.is_some()
                || delete.output.is_some()
                || !delete.order_by.is_empty()
                || delete.limit.is_some()
            {
                return Err(unsupported(
                    "DELETE with USING, RETURNING, ORDER BY or LIMIT is unsupported",
                ));
            }
            let relation = &tables[0].relation;
            let target = crate::table_factor_name(relation)?;
            let q = qualifier(relation);
            let mut query = format!("SELECT {q}.\"{ID_FIELD}\" AS \"{ROW_ID}\" FROM {relation}");
            if let Some(selection) = &delete.selection {
                let _ = write!(query, " WHERE {selection}");
            }
            Ok(DmlPlan {
                kind: DmlKind::Delete,
                target,
                query,
                columns: Vec::new(),
                parameter_columns: Vec::new(),
            })
        }
        _ => Err(unsupported(
            "only INSERT, UPDATE, and DELETE are accepted by the mutation endpoint",
        )),
    }
}

/// The stored label a lowercased table name refers to.
pub(crate) fn target_label(
    lmpg: &Lmpg,
    snapshot: &ReadView,
    target: &str,
) -> Result<String, SqlError> {
    let labels = access::labels(snapshot, lmpg.tenant)?;
    if let Some(label) = labels
        .iter()
        .find(|label| label.to_ascii_lowercase() == target)
    {
        return Ok(label.clone());
    }
    let closest = rushdb_types::names::closest(target, labels.iter().map(String::as_str), 3);
    let hint = if closest.is_empty() {
        String::new()
    } else {
        let quoted: Vec<String> = closest.iter().map(|label| format!("`{label}`")).collect();
        format!("; did you mean {}?", quoted.join(" or "))
    };
    Err(unsupported(format!(
        "DML target `{target}` is not a tenant-visible record table{hint}"
    )))
}

fn record_of(lmpg: &Lmpg, snapshot: &ReadView, id: &str) -> Result<Option<RecordRef>, SqlError> {
    let uuid = uuid::Uuid::parse_str(id)
        .map_err(|_| SqlError::InvalidIdentity(format!("invalid __id `{id}`")))?;
    Ok(access::record_by_id(
        snapshot,
        lmpg.tenant,
        &rushdb_types::ids::RecordId(*uuid.as_bytes()),
    )?
    .map(|header| RecordRef {
        record_id: header.record_id,
        ordinal: header.ordinal,
    }))
}

fn row_id(row: &Row) -> Result<String, SqlError> {
    match row.get(ROW_ID) {
        Some(Json::String(value)) => canonical_uuid(value),
        other => Err(SqlError::InvalidIdentity(format!(
            "stored __id must be a UUID string, got {}",
            other.cloned().unwrap_or(Json::Null)
        ))),
    }
}

/// Plans writes from the rows the read returns, charging their size.
struct Writes<'a> {
    lmpg: &'a Lmpg,
    snapshot: &'a ReadView,
    plan: &'a DmlPlan,
    label: String,
    insert_columns: Vec<String>,
    batch: BatchTx,
    seen: BTreeSet<String>,
    affected: u64,
    charged: usize,
    limit: Option<usize>,
}

impl Writes<'_> {
    fn charge(&mut self, fields: &[(String, FieldChange)]) -> Result<(), SqlError> {
        let mut bytes = WRITE_OVERHEAD;
        for (name, change) in fields {
            bytes = bytes.saturating_add(name.len());
            if let FieldChange::Set(value) = change {
                bytes = bytes.saturating_add(
                    serde_json::to_vec(value)
                        .map_err(|error| SqlError::Engine(error.to_string()))?
                        .len(),
                );
            }
        }
        self.charged = self.charged.saturating_add(bytes);
        match self.limit {
            Some(limit) if self.charged > limit => Err(SqlError::ResourceLimit(self.charged)),
            _ => Ok(()),
        }
    }

    fn row(&mut self, row: &Row, names: &[String]) -> Result<(), SqlError> {
        match self.plan.kind {
            DmlKind::Update => {
                let id = row_id(row)?;
                if !self.seen.insert(id.clone()) {
                    return Ok(()); // UPDATE ... FROM may match a row more than once
                }
                self.affected += 1;
                let record = record_of(self.lmpg, self.snapshot, &id)?
                    .ok_or_else(|| SqlError::InvalidIdentity(format!("record `{id}` vanished")))?;
                let mut fields = Vec::new();
                for (i, column) in self.plan.columns.iter().enumerate() {
                    let old = row.get(&format!("{OLD}{i}")).unwrap_or(&Json::Null);
                    let new = row.get(&format!("{NEW}{i}")).unwrap_or(&Json::Null);
                    if old != new {
                        fields.push((column.clone(), FieldChange::Set(new.clone())));
                    }
                }
                if !fields.is_empty() {
                    self.charge(&fields)?;
                    self.batch.ops.push(BatchOp::UpdateFields {
                        owner: rushdb_types::ids::OwnerKind::Record,
                        owner_ord: record.ordinal,
                        owner_public_id: record.record_id,
                        fields,
                    });
                }
            }
            DmlKind::Delete => {
                let id = row_id(row)?;
                if !self.seen.insert(id.clone()) {
                    return Ok(());
                }
                self.affected += 1;
                let record = record_of(self.lmpg, self.snapshot, &id)?
                    .ok_or_else(|| SqlError::InvalidIdentity(format!("record `{id}` vanished")))?;
                self.charge(&[])?;
                self.batch.ops.push(BatchOp::DeleteRecord { record });
            }
            DmlKind::Insert => {
                if names.len() > self.insert_columns.len() {
                    return Err(unsupported(
                        "INSERT has more expressions than target columns",
                    ));
                }
                let mut id = None;
                let mut fields = Vec::new();
                for (name, column) in names.iter().zip(&self.insert_columns) {
                    let value = row.get(name).cloned().unwrap_or(Json::Null);
                    if column == ID_FIELD {
                        match value {
                            Json::String(value) => id = Some(canonical_uuid(&value)?),
                            Json::Null => {}
                            other => {
                                return Err(SqlError::InvalidIdentity(format!(
                                    "__id must be a UUID string, got {other}"
                                )))
                            }
                        }
                    } else {
                        fields.push((column.clone(), FieldChange::Set(value)));
                    }
                }
                let id = match id {
                    Some(id) => {
                        if record_of(self.lmpg, self.snapshot, &id)?.is_some() {
                            return Err(SqlError::InvalidIdentity(format!(
                                "INSERT __id `{id}` already exists"
                            )));
                        }
                        id
                    }
                    None => uuid::Uuid::now_v7().to_string(),
                };
                if !self.seen.insert(id.clone()) {
                    return Err(SqlError::InvalidIdentity(format!(
                        "duplicate __id `{id}` in SQL result"
                    )));
                }
                let uuid = uuid::Uuid::parse_str(&id)
                    .map_err(|_| SqlError::InvalidIdentity(format!("invalid __id `{id}`")))?;
                self.affected += 1;
                self.charge(&fields)?;
                self.batch.ops.push(BatchOp::CreateRecord {
                    label: self.label.clone(),
                    explicit_id: Some(*uuid.as_bytes()),
                    fields,
                });
            }
        }
        Ok(())
    }
}

/// Execute one DML statement against a pinned view (see the module docs).
///
/// # Errors
/// Parse, planning, identity, resource (`max_result_bytes` bounds the
/// statement's memory and its planned writes), storage and commit errors.
pub(crate) async fn execute(
    lmpg: &Lmpg,
    snapshot: Arc<ReadView>,
    sql: &str,
    parameters: Option<&Json>,
    max_result_bytes: Option<usize>,
    text: Option<&Arc<TextIndexes>>,
) -> Result<SqlMutationResult, SqlError> {
    let plan = plan(sql)?;
    let snapshot_version = snapshot.commit_index();
    let label = target_label(lmpg, &snapshot, &plan.target)?;
    let mut context = context_in_view_with_budget(lmpg, &snapshot, max_result_bytes, text)?;
    let insert_columns = if plan.kind == DmlKind::Insert && plan.columns.is_empty() {
        let provider = context
            .context
            .table_provider(plan.target.as_str())
            .await
            .map_err(planning_error)?;
        provider
            .schema()
            .fields()
            .iter()
            .map(|field| field.name().clone())
            .collect()
    } else {
        plan.columns.clone()
    };
    let mut dataframe = context
        .context
        .sql(&plan.query)
        .await
        .map_err(|error| with_table_hint(planning_error(error), &context.context))?;
    if let Some(parameters) = parameters {
        dataframe = bind_parameters(dataframe, parameters)?;
    }
    let names: Vec<String> = dataframe
        .schema()
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect();
    if plan.kind == DmlKind::Insert && names.iter().collect::<BTreeSet<_>>().len() != names.len() {
        return Err(unsupported(
            "INSERT source columns must have distinct names",
        ));
    }
    let mut stream = dataframe
        .execute_stream()
        .await
        .map_err(|error| map_stream_error(&error, max_result_bytes, memory_cap(lmpg)))?;
    let mut writes = Writes {
        lmpg,
        snapshot: &snapshot,
        plan: &plan,
        label,
        insert_columns,
        batch: BatchTx { ops: Vec::new() },
        seen: BTreeSet::new(),
        affected: 0,
        charged: 0,
        limit: max_result_bytes,
    };
    while let Some(batch) = stream.next().await {
        let batch =
            batch.map_err(|error| map_stream_error(&error, max_result_bytes, memory_cap(lmpg)))?;
        let mut rows = Vec::new();
        let mut decoded = 0usize;
        append_batch_rows_with_budget(&batch, &mut rows, &mut decoded, None)?;
        for row in &rows {
            writes.row(row, &names)?;
        }
    }
    context.record_real_memory_peak();
    let affected = writes.affected;
    let commit_index = if writes.batch.ops.is_empty() {
        snapshot_version
    } else {
        lmpg.run_batch_at_version(writes.batch, snapshot_version)
            .map_err(|error| match error {
                rushdb_lmpg::LmpgError::WriteConflict(reason) => SqlError::WriteConflict(reason),
                other => SqlError::Commit(other.to_string()),
            })?
            .commit_index
    };
    let mut count = Row::new();
    count.insert("count".into(), Json::from(affected));
    Ok(SqlMutationResult {
        rows: vec![count],
        affected,
        commit_index,
    })
}
