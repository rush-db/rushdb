//! CHK-1: a query's labels, properties and equality literals checked
//! against the catalog of the snapshot it reads, with what it most likely
//! meant. The query crate finds the misses (one probe each); suggestions
//! are read only for a miss, through the same access check as `describe`.

use rushdb_protocol::{
    Operation, ProtocolError, Query, QueryWarning, QueryWarningKind, QueryWarningSuggestion,
    Response, StatementLanguage, ValueMatch, ValueSearch,
};
use rushdb_query::catalog_check::CatalogMiss;

use rushdb_lmpg::CatalogAccess;

use super::{execution_failed, ProtocolExecutor};

/// Suggestions per warning.
const SUGGESTIONS: usize = 5;

impl ProtocolExecutor<'_> {
    /// `strict`: the request is refused, before it runs, when the check
    /// finds a miss.
    pub(super) fn refuse_strict(
        &self,
        input: Option<&CheckInput>,
        check: rushdb_protocol::CheckMode,
        context: &mut rushdb_engine::ExecutionContext,
        access: &CatalogAccess,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(), ProtocolError> {
        let Some(input) = input.filter(|_| check == rushdb_protocol::CheckMode::Strict) else {
            return Ok(());
        };
        let warnings = self.catalog_warnings(input, context, access, is_cancelled)?;
        if warnings.is_empty() {
            return Ok(());
        }
        Err(ProtocolError::new(
            rushdb_protocol::ErrorCode::InvalidRequest,
            "the query names labels, properties or values that no record holds",
        )
        .with_details(serde_json::json!({ "warnings": warnings })))
    }

    /// `warn` (the default), once the request ran: its warnings, read at
    /// the snapshot it answered from, unless its answer shows there can be
    /// none ([`CheckInput::answered`]).
    pub(super) fn answer_warnings(
        &self,
        input: Option<&CheckInput>,
        check: rushdb_protocol::CheckMode,
        response: &Response,
        context: &mut rushdb_engine::ExecutionContext,
        access: &CatalogAccess,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<QueryWarning>, ProtocolError> {
        match input {
            Some(input)
                if check == rushdb_protocol::CheckMode::Warn && !input.answered(response) =>
            {
                self.catalog_warnings(input, context, access, is_cancelled)
            }
            _ => Ok(Vec::new()),
        }
    }

    /// The warnings of `input`.
    pub(super) fn catalog_warnings(
        &self,
        input: &CheckInput,
        context: &mut rushdb_engine::ExecutionContext,
        access: &CatalogAccess,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<QueryWarning>, ProtocolError> {
        let view = std::sync::Arc::clone(context.read_view());
        let from_sql;
        let checked = match input {
            CheckInput::Sql(statement) => {
                from_sql = sql_parts(&view, context.tenant(), statement)?;
                &from_sql
            }
            CheckInput::Parts { parts, .. } => parts,
        };
        let mut misses = Vec::new();
        for (labels, clause) in checked {
            misses.extend(
                rushdb_query::catalog_check::check_search(
                    view.as_ref(),
                    context.tenant(),
                    labels,
                    clause.as_ref(),
                )
                .map_err(|error| execution_failed(error.to_string()))?,
            );
        }
        misses
            .into_iter()
            .map(|miss| self.warning(miss, context, access, is_cancelled))
            .collect()
    }

    fn warning(
        &self,
        miss: CatalogMiss,
        context: &mut rushdb_engine::ExecutionContext,
        access: &CatalogAccess,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<QueryWarning, ProtocolError> {
        let names = |text: &str| {
            self.node
                .suggest_names_in_view(context.read_view(), text, SUGGESTIONS * 2, access)
                .map_err(execution_failed)
        };
        Ok(match miss {
            CatalogMiss::Label(label) => QueryWarning {
                kind: QueryWarningKind::UnknownLabel,
                labels: Vec::new(),
                suggestions: names(&label)?
                    .into_iter()
                    .filter(|name| name.property.is_none() && name.label != label)
                    .take(SUGGESTIONS)
                    .map(|name| suggestion(Some(name.label), None, None, "spelling"))
                    .collect(),
                label: Some(label),
                property: None,
                value: None,
            },
            CatalogMiss::Property { labels, property } => {
                let text = match labels.as_slice() {
                    [label] => format!("{label}.{property}"),
                    _ => property.clone(),
                };
                QueryWarning {
                    kind: QueryWarningKind::UnknownProperty,
                    suggestions: names(&text)?
                        .into_iter()
                        .filter(|name| name.property.is_some())
                        .take(SUGGESTIONS)
                        .map(|name| suggestion(Some(name.label), name.property, None, "spelling"))
                        .collect(),
                    labels,
                    label: None,
                    property: Some(property),
                    value: None,
                }
            }
            CatalogMiss::Value {
                labels,
                property,
                value,
            } => {
                let suggestions = self.value_suggestions(
                    &labels,
                    &property,
                    &value,
                    context,
                    access,
                    is_cancelled,
                )?;
                QueryWarning {
                    kind: QueryWarningKind::UnknownValue,
                    labels,
                    label: None,
                    property: Some(property),
                    value: Some(value),
                    suggestions,
                }
            }
        })
    }

    /// The stored spelling of `value` in the property, other properties of
    /// the labels that store it, and stored values containing it.
    fn value_suggestions(
        &self,
        labels: &[String],
        property: &str,
        value: &str,
        context: &mut rushdb_engine::ExecutionContext,
        access: &CatalogAccess,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<QueryWarningSuggestion>, ProtocolError> {
        let wanted = rushdb_query::normalize(property);
        let mut out = Vec::new();
        for matching in [ValueMatch::Exact, ValueMatch::Contains] {
            let search = ValueSearch {
                text: value.to_owned(),
                matching,
                case_sensitive: false,
                domain_on_miss: None,
                min_similarity: None,
            };
            let rushdb_protocol::Response::ValueMatches { matches, .. } = self
                .query_search_values(
                    &search,
                    labels,
                    Some(u64::try_from(SUGGESTIONS * 4).unwrap_or(u64::MAX)),
                    is_cancelled,
                    context,
                    access,
                )?
            else {
                continue;
            };
            for row in matches {
                let text = match &row.value {
                    serde_json::Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                let same_property = rushdb_query::normalize(&row.property) == wanted;
                let found = match (matching, same_property) {
                    (ValueMatch::Exact, true) => suggestion(None, None, Some(text), "spelling"),
                    (ValueMatch::Exact, false) => {
                        suggestion(Some(row.label), Some(row.property), Some(text), "storedIn")
                    }
                    (_, true) => suggestion(None, None, Some(text), "contains"),
                    _ => continue,
                };
                // A value found both ways is suggested once, with the
                // first (stronger) reason: the stored spelling also
                // contains the value it spells.
                if !out.iter().any(|known: &QueryWarningSuggestion| {
                    known.label == found.label
                        && known.property == found.property
                        && known.value == found.value
                }) {
                    out.push(found);
                }
            }
        }
        out.truncate(SUGGESTIONS);
        Ok(out)
    }
}

fn suggestion(
    label: Option<String>,
    property: Option<String>,
    value: Option<String>,
    reason: &str,
) -> QueryWarningSuggestion {
    QueryWarningSuggestion {
        label,
        property,
        value,
        reason: reason.to_owned(),
    }
}

/// Labels and the filter clause to check against them.
type Checked = Vec<(Vec<String>, Option<serde_json::Value>)>;

/// What the catalog check reads of a request, taken before it runs
/// (running it consumes the operation).
pub(super) enum CheckInput {
    /// The labels and filter clauses of a native or Mongo query.
    Parts {
        parts: Checked,
        /// A records query on at most one label, without a global aggregate,
        /// whose filter is positive: if it finds anything, nothing it names
        /// is missing ([`CheckInput::answered`]).
        provable: bool,
    },
    /// A SQL statement.
    Sql(String),
}

impl CheckInput {
    /// The input of `operation`, when it is a query the check reads.
    pub(super) fn of(operation: &Operation) -> Option<Self> {
        if let Operation::Query {
            query:
                Query::Statement {
                    language: StatementLanguage::Sql,
                    statement,
                    ..
                },
        } = operation
        {
            return Some(Self::Sql(statement.clone()));
        }
        let (labels, clause) = checked_parts(operation)?;
        let provable = matches!(
            operation,
            Operation::Query { query: Query::Records { query, .. } }
                if query.labels.len() <= 1 && !global_aggregate(query)
        ) && clause.as_ref().is_none_or(positive);
        Some(Self::Parts {
            parts: vec![(labels, clause)],
            provable,
        })
    }

    /// Whether `response` shows that the check would find nothing: a
    /// positive filter on one label that found a record holds that label,
    /// every property it names and every literal it compares (a missing
    /// one would have matched nothing). A filter that can match without
    /// them (`$or`, `$in`, a negation, `$exists`, several labels) and a
    /// global aggregate (one row even when nothing matched) are checked.
    pub(super) fn answered(&self, response: &Response) -> bool {
        let Self::Parts { provable: true, .. } = self else {
            return false;
        };
        match response {
            Response::Records { records, total, .. } => *total > 0 || !records.is_empty(),
            Response::Rows { rows, .. } => !rows.is_empty(),
            _ => false,
        }
    }
}

/// Operators every match of which holds the property with a value the
/// condition names or bounds.
const POSITIVE: [&str; 9] = [
    "$eq",
    "$gt",
    "$gte",
    "$lt",
    "$lte",
    "$contains",
    "$startsWith",
    "$endsWith",
    "$within",
];

/// Whether every record a filter clause matches holds each label, property
/// and literal it names: conditions joined by `$and` (or side by side),
/// each a value, positive operators on one value, or a traversal block.
fn positive(clause: &serde_json::Value) -> bool {
    let serde_json::Value::Object(clause) = clause else {
        return false;
    };
    clause.iter().all(|(key, value)| match key.as_str() {
        "$and" => match value {
            serde_json::Value::Array(operands) => operands.iter().all(positive),
            operand => positive(operand),
        },
        key if key.starts_with('$') => false,
        _ => positive_condition(value),
    })
}

fn positive_condition(value: &serde_json::Value) -> bool {
    match value {
        // Any of several values.
        serde_json::Value::Array(_) => false,
        serde_json::Value::Object(operators)
            if !operators.is_empty() && operators.keys().all(|op| op.starts_with('$')) =>
        {
            operators
                .iter()
                .all(|(op, operand)| POSITIVE.contains(&op.as_str()) && !operand.is_array())
        }
        // A traversal block.
        block @ serde_json::Value::Object(_) => positive(block),
        _ => true,
    }
}

/// A `select` of aggregates without `groupBy`: one row even when nothing
/// matched.
fn global_aggregate(query: &rushdb_protocol::QueryDocument) -> bool {
    query.group_by.is_empty()
        && query
            .select
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .is_some_and(|select| select.values().any(serde_json::Value::is_object))
}

/// The labels and texts a SQL statement compares by equality, one clause
/// per table that names a stored label (a table that names none fails
/// planning on its own).
fn sql_parts(
    view: &rushdb_storage::ReadView,
    tenant: rushdb_types::ids::TenantId,
    sql: &str,
) -> Result<Checked, ProtocolError> {
    let texts = rushdb_sql::compared_texts::compared_texts(sql);
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let stored = rushdb_lmpg::reader::try_access::labels(view, tenant)
        .map_err(|error| execution_failed(error.to_string()))?;
    let mut by_label: std::collections::BTreeMap<
        String,
        serde_json::Map<String, serde_json::Value>,
    > = std::collections::BTreeMap::new();
    for text in texts {
        // As a SELECT resolves a table: the label of that spelling, else the
        // first stored label equal to it ignoring case.
        let Some(label) = stored
            .iter()
            .find(|label| **label == text.table)
            .or_else(|| {
                stored
                    .iter()
                    .find(|label| label.eq_ignore_ascii_case(&text.table))
            })
        else {
            continue;
        };
        let values = by_label
            .entry(label.clone())
            .or_default()
            .entry(text.column)
            .or_insert_with(|| serde_json::json!({"$in": []}));
        if let Some(list) = values["$in"].as_array_mut() {
            list.push(text.value.into());
        }
    }
    Ok(by_label
        .into_iter()
        .map(|(label, clause)| (vec![label], Some(serde_json::Value::Object(clause))))
        .collect())
}

/// The labels and the filter clause a checked query names: a records or
/// property-values query document, or a Mongo `find`/`count`/`aggregate`
/// statement's collection and filter.
fn checked_parts(operation: &Operation) -> Option<(Vec<String>, Option<serde_json::Value>)> {
    let Operation::Query { query } = operation else {
        return None;
    };
    match query {
        Query::Records { query, .. } | Query::PropertyValues { query, .. } => {
            Some((query.labels.clone(), Some(query.predicate.clone())))
        }
        Query::Statement {
            language: StatementLanguage::Mongo,
            statement,
            ..
        } => {
            let statement: serde_json::Value = serde_json::from_str(statement).ok()?;
            let collection = statement.get("collection")?.as_str()?.to_owned();
            let filter = ["find", "filter", "count"]
                .iter()
                .find_map(|key| statement.get(*key).filter(|value| value.is_object()))
                .cloned();
            Some((vec![collection], filter))
        }
        _ => None,
    }
}
