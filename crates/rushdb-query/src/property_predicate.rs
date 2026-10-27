//! Predicates over one object's own properties, evaluated without the
//! graph around it (relationship properties): the full field-operator set
//! of the record predicate, but no traversal blocks.

use crate::document::{self, QueryDocumentError};
use crate::{eval_pred, Expr, Json, Map, QueryError};

/// Parse a property predicate that is evaluated on one object without the
/// graph around it (relationship properties): the full operator set, but
/// no traversal blocks.
///
/// # Errors
/// Returns [`QueryDocumentError::Parse`] for a malformed predicate or a
/// traversal block.
pub fn parse_property_predicate(where_: &Json) -> Result<Expr, QueryDocumentError> {
    fn traverses(expr: &Expr) -> bool {
        match expr {
            Expr::And(items) | Expr::Or(items) | Expr::Nor(items) => items.iter().any(traverses),
            Expr::Xor(a, b) => traverses(a) || traverses(b),
            Expr::Not(inner) => traverses(inner),
            Expr::Field(_) => false,
            Expr::Related(_) => true,
        }
    }
    let expr = document::parse_where(where_)?;
    if traverses(&expr) {
        return Err(QueryDocumentError::Parse(
            "a relationship predicate filters the relationship's own properties; \
             traversal blocks are not supported"
                .into(),
        ));
    }
    Ok(expr)
}

/// Evaluate a [`parse_property_predicate`] expression on one object. `$type`
/// tests the stored JSON shape (there is no declared type to consult).
///
/// # Errors
/// Returns [`QueryError`] when evaluation fails.
pub fn matches_properties(expr: &Expr, obj: &Map<String, Json>) -> Result<bool, QueryError> {
    Ok(match expr {
        Expr::And(items) => {
            for item in items {
                if !matches_properties(item, obj)? {
                    return Ok(false);
                }
            }
            true
        }
        Expr::Or(items) | Expr::Nor(items) => {
            let mut any = false;
            for item in items {
                if matches_properties(item, obj)? {
                    any = true;
                    break;
                }
            }
            any != matches!(expr, Expr::Nor(_))
        }
        Expr::Xor(a, b) => matches_properties(a, obj)? != matches_properties(b, obj)?,
        Expr::Not(inner) => !matches_properties(inner, obj)?,
        Expr::Field(pred) => eval_pred(&|_| Ok(None), pred, obj)?,
        Expr::Related(_) => false,
    })
}
