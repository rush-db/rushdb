use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::cmp::Ordering;

use crate::model::LogicalType;

/// A single canonical scalar value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Scalar {
    /// Boolean value.
    Boolean(bool),
    /// Signed 64-bit integer.
    Int64(i64),
    /// IEEE-754 double-precision float.
    Float64(f64),
    /// UTF-8 string.
    Utf8(String),
    /// UTC epoch milliseconds.
    DateTime(i64),
}

/// Compare canonical scalars without widening integers through an imprecise
/// floating-point representation. Integer/float equality is accepted only
/// when the float is finite, integral, in range, and round-trips exactly.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "exact round-trip checks are the definition of canonical integer/float equality"
)]
pub fn scalar_equal(left: &Scalar, right: &Scalar) -> bool {
    match (left, right) {
        (Scalar::Boolean(a), Scalar::Boolean(b)) => a == b,
        (Scalar::Utf8(a), Scalar::Utf8(b)) => a == b,
        (Scalar::DateTime(a), Scalar::DateTime(b)) | (Scalar::Int64(a), Scalar::Int64(b)) => a == b,
        (Scalar::Float64(a), Scalar::Float64(b)) => a.is_finite() && b.is_finite() && a == b,
        (Scalar::Int64(integer), Scalar::Float64(float))
        | (Scalar::Float64(float), Scalar::Int64(integer)) => {
            if !float.is_finite() || float.fract() != 0.0 {
                return false;
            }
            // Rust saturates float-to-integer casts at the integer bounds.
            // Reject values outside the signed domain before casting so
            // `2^63` cannot alias `i64::MAX` (and likewise at the negative
            // boundary).
            if *float >= 9_223_372_036_854_775_808.0 || *float < -9_223_372_036_854_775_808.0 {
                return false;
            }
            let converted = *float as i64;
            converted == *integer && (converted as f64) == *float
        }
        _ => false,
    }
}

/// Compare canonical scalars without converting large integers to `f64`.
/// Mixed integer/float comparisons preserve the exact ordering represented by
/// the IEEE-754 value; non-numeric or mismatched domains return `None`.
#[must_use]
pub fn scalar_cmp(left: &Scalar, right: &Scalar) -> Option<Ordering> {
    match (left, right) {
        (Scalar::Boolean(a), Scalar::Boolean(b)) => Some(a.cmp(b)),
        (Scalar::Utf8(a), Scalar::Utf8(b)) => Some(a.cmp(b)),
        (Scalar::DateTime(a), Scalar::DateTime(b)) | (Scalar::Int64(a), Scalar::Int64(b)) => {
            Some(a.cmp(b))
        }
        (Scalar::Float64(a), Scalar::Float64(b)) => a.partial_cmp(b),
        (Scalar::Int64(integer), Scalar::Float64(float)) => cmp_int_float(*integer, *float),
        (Scalar::Float64(float), Scalar::Int64(integer)) => {
            cmp_int_float(*integer, *float).map(Ordering::reverse)
        }
        _ => None,
    }
}

/// Compare JSON scalar values using the canonical integer/float ordering.
/// Non-scalar values and mismatched domains return `None`.
#[must_use]
pub fn json_scalar_cmp(left: &Json, right: &Json) -> Option<Ordering> {
    let (_, left) = infer_scalar(left)?;
    let (_, right) = infer_scalar(right)?;
    scalar_cmp(&left, &right)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "bounds and exact integer-part comparison are intentional"
)]
fn cmp_int_float(integer: i64, float: f64) -> Option<Ordering> {
    if !float.is_finite() {
        return float.partial_cmp(&0.0).map(Ordering::reverse);
    }
    let truncated = float.trunc();
    // Use the exact signed-domain boundary rather than `i64::MAX as f64`,
    // which rounds up to 2^63 and would make the saturating cast below alias
    // the largest signed integer.
    if truncated >= 9_223_372_036_854_775_808.0 {
        return Some(Ordering::Less);
    }
    if truncated < -9_223_372_036_854_775_808.0 {
        return Some(Ordering::Greater);
    }
    let integer_part = truncated as i64;
    match integer.cmp(&integer_part) {
        Ordering::Equal if float.fract() > 0.0 => Some(Ordering::Less),
        Ordering::Equal if float.fract() < 0.0 => Some(Ordering::Greater),
        ordering => Some(ordering),
    }
}

/// Normalize a property name for lookup: ASCII-uppercase folds to lowercase,
/// other alphanumerics are kept, and every other character becomes `_`.
///
/// # Examples
///
/// ```
/// use rushdb_types::value::normalize_name;
///
/// assert_eq!(normalize_name("FirstName"), "firstname");
/// assert_eq!(normalize_name("first-name"), "first_name");
/// ```
#[must_use]
pub fn normalize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push(c.to_ascii_lowercase());
        } else if c.is_alphanumeric() {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

/// The record or relationship id in a reconstructed entity and in language
/// results (SQL, Cypher, GQL, Gremlin, Bolt).
pub const ID_FIELD: &str = "__id";
/// The record label in a reconstructed record.
pub const LABEL_FIELD: &str = "__label";
/// The relationship type in a reconstructed relationship.
pub const TYPE_FIELD: &str = "__type";
/// The source record id in a reconstructed relationship.
pub const SOURCE_FIELD: &str = "__source_id";
/// The target record id in a reconstructed relationship.
pub const TARGET_FIELD: &str = "__target_id";

/// Whether a normalized property name is reserved for the engine: every name
/// starting with `__` ([`ID_FIELD`], [`LABEL_FIELD`], `__proptypes`,
/// `__score`, …). No record or relationship stores a property under such a
/// name, so system fields never collide with user properties; `_id` is an
/// ordinary property.
///
/// # Examples
///
/// ```
/// use rushdb_types::value::{is_reserved_name, normalize_name};
///
/// assert!(is_reserved_name("__id"));
/// assert!(is_reserved_name(&normalize_name("__Score")));
/// assert!(!is_reserved_name("_id"));
/// ```
#[must_use]
pub fn is_reserved_name(normalized: &str) -> bool {
    normalized.starts_with("__")
}

/// Convert a canonical scalar to its JSON representation.
#[must_use]
pub fn scalar_to_json(s: &Scalar) -> Json {
    match s {
        Scalar::Boolean(b) => Json::Bool(*b),
        Scalar::Int64(i) => Json::Number((*i).into()),
        Scalar::Float64(f) => serde_json::Number::from_f64(*f).map_or(Json::Null, Json::Number),
        Scalar::Utf8(st) => Json::String(st.clone()),
        Scalar::DateTime(ms) => Json::Number((*ms).into()),
    }
}

/// Infer a logical type and scalar from a JSON value.
///
/// Returns `None` for JSON nulls, arrays and objects, which have no scalar
/// interpretation.
#[must_use]
pub fn infer_scalar(v: &Json) -> Option<(LogicalType, Scalar)> {
    match v {
        Json::Bool(b) => Some((LogicalType::Boolean, Scalar::Boolean(*b))),
        Json::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some((LogicalType::Int64, Scalar::Int64(i)))
            } else if n.as_u64().is_some() {
                // The engine's canonical integer domain is signed i64. Do
                // not silently widen an out-of-domain JSON integer through
                // f64: values above 2^63-1 would lose identity and could
                // match the wrong index entry.
                None
            } else {
                n.as_f64()
                    .map(|f| (LogicalType::Float64, Scalar::Float64(f)))
            }
        }
        Json::String(s) => Some((LogicalType::Utf8, Scalar::Utf8(s.clone()))),
        _ => None,
    }
}

/// Name of the JSON kind of a value (`"null"`, `"boolean"`, ...).
#[must_use]
pub fn json_type_name(v: &Json) -> &'static str {
    match v {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_ordering_preserves_integer_precision() {
        assert_eq!(
            scalar_cmp(
                &Scalar::Int64(9_007_199_254_740_993),
                &Scalar::Float64(9_007_199_254_740_992.0)
            ),
            Some(Ordering::Greater)
        );
        assert_eq!(
            scalar_cmp(&Scalar::Int64(2), &Scalar::Float64(2.5)),
            Some(Ordering::Less)
        );
    }

    #[test]
    fn out_of_domain_unsigned_json_integer_is_not_widened() {
        let value = serde_json::json!(u64::MAX);
        assert!(infer_scalar(&value).is_none());
    }

    #[test]
    fn json_scalar_ordering_preserves_integer_precision() {
        let left = Json::Number(9_007_199_254_740_993i64.into());
        let right = Json::Number(9_007_199_254_740_992i64.into());
        assert_eq!(json_scalar_cmp(&left, &right), Some(Ordering::Greater));
        assert_eq!(json_scalar_cmp(&Json::String("a".into()), &right), None);
    }

    #[test]
    fn scalar_equality_preserves_integer_precision() {
        assert!(scalar_equal(&Scalar::Int64(1), &Scalar::Float64(1.0)));
        assert!(!scalar_equal(
            &Scalar::Int64(9_007_199_254_740_993),
            &Scalar::Float64(9_007_199_254_740_992.0)
        ));
        assert!(!scalar_equal(
            &Scalar::Float64(f64::NAN),
            &Scalar::Float64(f64::NAN)
        ));
        assert!(!scalar_equal(&Scalar::Utf8("1".into()), &Scalar::Int64(1)));
        assert!(!scalar_equal(
            &Scalar::Int64(i64::MAX),
            &Scalar::Float64(9_223_372_036_854_775_808.0)
        ));
    }

    #[test]
    fn scalar_ordering_rejects_saturating_integer_boundary_aliases() {
        assert_eq!(
            scalar_cmp(
                &Scalar::Int64(i64::MAX),
                &Scalar::Float64(9_223_372_036_854_775_808.0)
            ),
            Some(Ordering::Less)
        );
        assert_eq!(
            scalar_cmp(
                &Scalar::Int64(i64::MIN),
                &Scalar::Float64(-9_223_372_036_854_775_808.0)
            ),
            Some(Ordering::Equal)
        );
    }
}
