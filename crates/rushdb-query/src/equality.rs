//! Equality between a reconstructed field value and a predicate literal, as
//! the evaluator applies it for `$eq`, `$ne`, `$in` and `$nin`
//! (`rushdb_types::types::doc_equals_literal`).

use serde_json::Value as Json;

/// Apply a matcher to a scalar value or to every element of an array value
/// (`RushDB` array semantics: a condition matches when any element matches).
pub(crate) fn any_element(value: &Json, matcher: impl Fn(&Json) -> bool) -> bool {
    if let Json::Array(items) = value {
        items.iter().any(matcher)
    } else {
        matcher(value)
    }
}

#[cfg(test)]
pub(crate) fn strings_equal(a: &Json, b: &Json) -> bool {
    rushdb_types::types::doc_equals_literal(a, b)
}
