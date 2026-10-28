//! Equality between a reconstructed field value and a predicate literal, as
//! the evaluator applies it for `$eq`, `$ne`, `$in` and `$nin`.

use rushdb_types::value::{infer_scalar, scalar_equal};
use serde_json::Value as Json;

use crate::{DeclaredType, QueryError};

/// Apply a matcher to a scalar value or to every element of an array value
/// (`RushDB` array semantics: a condition matches when any element matches).
pub(crate) fn any_element(value: &Json, matcher: impl Fn(&Json) -> bool) -> bool {
    if let Json::Array(items) = value {
        items.iter().any(matcher)
    } else {
        matcher(value)
    }
}

pub(crate) fn is_datetime(
    declared_type: &DeclaredType<'_>,
    name: &str,
) -> Result<bool, QueryError> {
    Ok(declared_type(name)? == Some(rushdb_types::model::LogicalType::DateTime))
}

/// Equality of a stored value with a literal. A datetime is reconstructed as
/// its epoch milliseconds, so ISO-8601 text equals it when it names the same
/// instant.
pub(crate) fn value_equals(actual: &Json, expected: &Json, datetime: bool) -> bool {
    strings_equal(actual, expected)
        || (datetime
            && actual.as_i64().is_some_and(|ms| {
                expected.as_str().and_then(crate::datetime::parse_iso_8601) == Some(ms)
            }))
}

pub(crate) fn strings_equal(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::String(x), Json::String(y)) => x == y,
        (Json::Bool(x), Json::Bool(y)) => x == y,
        (Json::Number(_), Json::Number(_)) => match (infer_scalar(a), infer_scalar(b)) {
            (Some((_, left)), Some((_, right))) => scalar_equal(&left, &right),
            _ => false,
        },
        (Json::Null, Json::Null) => true,
        _ => false,
    }
}
