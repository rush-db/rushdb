//! PostgreSQL's session information functions (`current_database()`,
//! `current_user`, `current_schema()`, `version()`, …) answered from the
//! caller's session rather than by `DataFusion`, which has none of them.
//!
//! A transport that has a session (the PostgreSQL wire) binds each zero-
//! argument call to its value as a text literal before the statement is
//! planned, so the functions work anywhere an expression does: next to
//! another column, in a `WHERE`, in a subquery.

use std::ops::ControlFlow;

use datafusion::sql::sqlparser::ast::{
    Expr, FunctionArguments, Ident, Query, SelectItem, SetExpr, Value, VisitMut, VisitorMut,
};
use datafusion::sql::sqlparser::dialect::PostgreSqlDialect;
use datafusion::sql::sqlparser::parser::Parser;

/// `sql` with each zero-argument call of a function named in `values`
/// (case-insensitively) replaced by its value, or `None` when it calls none
/// of them or does not parse (the statement then fails or runs as written).
/// A replaced call that is a whole select-list item keeps the function's
/// name as its column name, as in PostgreSQL.
#[must_use]
pub fn bind_session_functions(sql: &str, values: &[(&str, &str)]) -> Option<String> {
    let lowered = sql.to_ascii_lowercase();
    if !values.iter().any(|(name, _)| lowered.contains(name)) {
        return None;
    }
    let mut statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).ok()?;
    let mut binder = Binder {
        values,
        bound: false,
    };
    let _ = statements.visit(&mut binder);
    binder.bound.then(|| {
        statements
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    })
}

struct Binder<'a> {
    values: &'a [(&'a str, &'a str)],
    bound: bool,
}

impl Binder<'_> {
    /// The function name and session value `expr` calls for, if any.
    fn value(&self, expr: &Expr) -> Option<(&str, &str)> {
        let Expr::Function(function) = expr else {
            return None;
        };
        let no_arguments = match &function.args {
            FunctionArguments::None => true,
            FunctionArguments::List(list) => list.args.is_empty(),
            FunctionArguments::Subquery(_) => false,
        };
        let [part] = function.name.0.as_slice() else {
            return None;
        };
        let name = &part.as_ident()?.value;
        (no_arguments && function.over.is_none() && function.filter.is_none())
            .then(|| {
                self.values
                    .iter()
                    .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
                    .map(|(candidate, value)| (*candidate, *value))
            })
            .flatten()
    }

    fn alias_projection(&mut self, body: &mut SetExpr) {
        match body {
            SetExpr::Select(select) => {
                for item in &mut select.projection {
                    let SelectItem::UnnamedExpr(expr) = item else {
                        continue;
                    };
                    let Some((name, value)) = self.value(expr) else {
                        continue;
                    };
                    *item = SelectItem::ExprWithAlias {
                        expr: literal(value),
                        alias: Ident::new(name),
                    };
                    self.bound = true;
                }
            }
            SetExpr::SetOperation { left, right, .. } => {
                self.alias_projection(left);
                self.alias_projection(right);
            }
            _ => {}
        }
    }
}

impl VisitorMut for Binder<'_> {
    type Break = ();

    fn pre_visit_query(&mut self, query: &mut Query) -> ControlFlow<()> {
        self.alias_projection(&mut query.body);
        ControlFlow::Continue(())
    }

    fn post_visit_expr(&mut self, expr: &mut Expr) -> ControlFlow<()> {
        if let Some((_, value)) = self.value(expr) {
            *expr = literal(value);
            self.bound = true;
        }
        ControlFlow::Continue(())
    }
}

fn literal(value: &str) -> Expr {
    Expr::value(Value::SingleQuotedString(value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION: &[(&str, &str)] =
        &[("current_database", "tenant-a"), ("current_user", "rushdb")];

    #[test]
    fn session_functions_bind_anywhere_an_expression_does() {
        assert_eq!(
            bind_session_functions("SELECT current_database(), current_user", SESSION).as_deref(),
            Some("SELECT 'tenant-a' AS current_database, 'rushdb' AS current_user")
        );
        assert_eq!(
            bind_session_functions(
                "SELECT name, upper(CURRENT_DATABASE()) AS db FROM t WHERE owner = current_user",
                SESSION
            )
            .as_deref(),
            Some("SELECT name, upper('tenant-a') AS db FROM t WHERE owner = 'rushdb'")
        );
        // A call with arguments, or a column of that name, is not the
        // function.
        assert_eq!(
            bind_session_functions("SELECT current_database FROM t", SESSION),
            None
        );
        assert_eq!(
            bind_session_functions("SELECT current_database(1)", SESSION),
            None
        );
        assert_eq!(bind_session_functions("SELECT 1", SESSION), None);
    }
}
