//! CHK-1: a query's labels, properties and equality literals checked
//! against the catalog of the snapshot it reads, with what it most likely
//! meant. The query crate finds the misses (one probe each); suggestions
//! are read only for a miss, through the same access check as `describe`.

use rushdb_protocol::{
    Operation, ProtocolError, Query, QueryWarning, QueryWarningKind, QueryWarningSuggestion,
    StatementLanguage, ValueMatch, ValueSearch,
};
use rushdb_query::catalog_check::CatalogMiss;

use rushdb_lmpg::CatalogAccess;

use super::{execution_failed, ProtocolExecutor};

/// Suggestions per warning.
const SUGGESTIONS: usize = 5;

impl ProtocolExecutor<'_> {
    /// The warnings of `operation` (none for an operation that is not a
    /// query this check reads).
    pub(super) fn catalog_warnings(
        &self,
        operation: &Operation,
        context: &mut rushdb_engine::ExecutionContext,
        access: &CatalogAccess,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<QueryWarning>, ProtocolError> {
        let Some((labels, clause)) = checked_parts(operation) else {
            return Ok(Vec::new());
        };
        let view = std::sync::Arc::clone(context.read_view());
        let misses = rushdb_query::catalog_check::check_search(
            view.as_ref(),
            context.tenant(),
            &labels,
            clause.as_ref(),
        )
        .map_err(|error| execution_failed(error.to_string()))?;
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
                if !out.contains(&found) {
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
