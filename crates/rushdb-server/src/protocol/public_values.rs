//! Values as a response shows them: rows published in their public form,
//! and the extrema of a value list.

use rushdb_protocol::Response;

/// Rows a response returns as the query produced them (statement rows,
/// aggregate and projection rows, text hits) in their public form: a
/// temporal value, duration or point carried in its tagged form inside the
/// engine becomes its literal. Records and relationships are rendered so
/// where they are built.
pub(super) fn publish_rows(response: &mut Response) {
    if let Response::Rows { rows, .. }
    | Response::TextHits { rows, .. }
    | Response::StatementMutation { rows, .. } = response
    {
        for row in rows.iter_mut() {
            *row = rushdb_types::types::doc_to_public(std::mem::take(row));
        }
    }
}

/// E3 `Range` mode extrema: numeric order for numbers, lexical order for
/// strings/ISO datetimes (the same ordering `describe` uses for its
/// per-property min/max). Non-comparable values (bool/null/array/object)
/// never become extrema; with no comparable scalars both are `None`.
/// Min and max of one ordered class of `values`: the numbers if there are
/// any, else the texts that are datetimes (by instant), else the texts.
/// Classes never mix (a text `min` beside a datetime `max` was returned for
/// a property stored both ways).
pub(super) fn values_range(
    values: &[serde_json::Value],
) -> (Option<serde_json::Value>, Option<serde_json::Value>) {
    if !values.iter().any(serde_json::Value::is_number) {
        let dated: Vec<(i64, &serde_json::Value)> = values
            .iter()
            .filter_map(|v| {
                v.as_str()?;
                let rushdb_types::value::Scalar::DateTime(instant) =
                    rushdb_types::types::coerce_literal(
                        rushdb_types::model::LogicalType::DateTime,
                        v,
                    )?
                else {
                    return None;
                };
                Some((instant.micros, v))
            })
            .collect();
        if !dated.is_empty() {
            let min = dated
                .iter()
                .min_by_key(|(ms, _)| *ms)
                .map(|(_, v)| (*v).clone());
            let max = dated
                .iter()
                .max_by_key(|(ms, _)| *ms)
                .map(|(_, v)| (*v).clone());
            return (min, max);
        }
    }
    let numbers = values.iter().any(serde_json::Value::is_number);
    let comparable = |v: &serde_json::Value| {
        if numbers {
            v.is_number()
        } else {
            v.is_string()
        }
    };
    let cmp = |a: &serde_json::Value, b: &serde_json::Value| -> std::cmp::Ordering {
        match (a, b) {
            (serde_json::Value::Number(_), serde_json::Value::Number(_)) => {
                rushdb_types::value::json_scalar_cmp(a, b).unwrap_or(std::cmp::Ordering::Equal)
            }
            (serde_json::Value::String(x), serde_json::Value::String(y)) => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        }
    };
    let mut min: Option<&serde_json::Value> = None;
    let mut max: Option<&serde_json::Value> = None;
    for v in values.iter().filter(|v| comparable(v)) {
        if min.is_none_or(|m| cmp(v, m) == std::cmp::Ordering::Less) {
            min = Some(v);
        }
        if max.is_none_or(|m| cmp(v, m) == std::cmp::Ordering::Greater) {
            max = Some(v);
        }
    }
    (min.cloned(), max.cloned())
}
