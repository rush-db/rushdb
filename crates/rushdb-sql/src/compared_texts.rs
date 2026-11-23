//! CHK-1 for SQL: the string literals a statement compares a column with by
//! equality (`=`, `<>`, `IN`, `NOT IN`), with the table the column belongs
//! to. Unknown tables and columns already fail planning with suggestions;
//! a text that no row holds runs and returns nothing, which is what the
//! catalog check reports.

use std::collections::BTreeMap;
use std::ops::ControlFlow;

use datafusion::sql::sqlparser::ast::{
    BinaryOperator, Expr, ObjectName, TableFactor, Value, Visit, Visitor,
};
use datafusion::sql::sqlparser::dialect::PostgreSqlDialect;
use datafusion::sql::sqlparser::parser::Parser;

/// A text compared with a column by equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparedText {
    /// The table as the statement names it.
    pub table: String,
    /// The column.
    pub column: String,
    /// The text.
    pub value: String,
}

/// The texts `sql` compares columns with by equality. A column written
/// without a table counts only when the statement reads one table; texts
/// it cannot place are left out (the check never guesses).
#[must_use]
pub fn compared_texts(sql: &str) -> Vec<ComparedText> {
    let Ok(statements) = Parser::parse_sql(&PostgreSqlDialect {}, sql) else {
        return Vec::new();
    };
    let mut collector = Collector::default();
    for statement in &statements {
        let _ = statement.visit(&mut collector);
    }
    let single = match collector.tables.values().collect::<Vec<_>>().as_slice() {
        [only] => Some((*only).clone()),
        _ => None,
    };
    collector
        .compared
        .into_iter()
        .filter_map(|(qualifier, column, value)| {
            let table = match qualifier {
                Some(qualifier) => collector.tables.get(&qualifier.to_lowercase())?.clone(),
                None => single.clone()?,
            };
            Some(ComparedText {
                table,
                column,
                value,
            })
        })
        .collect()
}

#[derive(Default)]
struct Collector {
    /// Table name or alias (lowercased) to the table name.
    tables: BTreeMap<String, String>,
    /// `(qualifier, column, text)`.
    compared: Vec<(Option<String>, String, String)>,
}

impl Visitor for Collector {
    type Break = ();

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<()> {
        if let TableFactor::Table { name, alias, .. } = factor {
            let table = last_part(name);
            self.tables.insert(table.to_lowercase(), table.clone());
            if let Some(alias) = alias {
                self.tables.insert(alias.name.value.to_lowercase(), table);
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        match expr {
            Expr::BinaryOp {
                left,
                op: BinaryOperator::Eq | BinaryOperator::NotEq,
                right,
            } => {
                let pair = column(left)
                    .zip(text(right))
                    .or_else(|| column(right).zip(text(left)));
                if let Some(((qualifier, name), value)) = pair {
                    self.compared.push((qualifier, name, value));
                }
            }
            Expr::InList { expr, list, .. } => {
                if let Some((qualifier, name)) = column(expr) {
                    for value in list.iter().filter_map(text) {
                        self.compared.push((qualifier.clone(), name.clone(), value));
                    }
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

fn last_part(name: &ObjectName) -> String {
    name.0
        .last()
        .and_then(|part| part.as_ident())
        .map_or_else(|| name.to_string(), |ident| ident.value.clone())
}

/// `(qualifier, column)` of a column reference.
fn column(expr: &Expr) -> Option<(Option<String>, String)> {
    match expr {
        Expr::Identifier(ident) => Some((None, ident.value.clone())),
        Expr::CompoundIdentifier(parts) => match parts.as_slice() {
            [.., table, column] => Some((Some(table.value.clone()), column.value.clone())),
            _ => None,
        },
        Expr::Nested(inner) => column(inner),
        _ => None,
    }
}

/// The text of a string literal.
fn text(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Value(value) => match &value.value {
            Value::SingleQuotedString(text) => Some(text.clone()),
            _ => None,
        },
        Expr::Nested(inner) => text(inner),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_are_placed_by_table_alias_or_the_only_table() {
        let found = compared_texts(
            "SELECT p.name FROM \"Person\" p JOIN \"City\" c ON p.city = c.name \
             WHERE p.city = 'berlin' AND c.country IN ('DE', 'FR') AND 'x' <> p.name",
        );
        let found: Vec<(String, String, String)> = found
            .into_iter()
            .map(|t| (t.table, t.column, t.value))
            .collect();
        assert_eq!(
            found,
            [
                ("Person".into(), "city".into(), "berlin".into()),
                ("City".into(), "country".into(), "DE".into()),
                ("City".into(), "country".into(), "FR".into()),
                ("Person".into(), "name".into(), "x".into()),
            ]
        );
        let single = compared_texts("SELECT * FROM \"Person\" WHERE city = 'berlin'");
        assert_eq!(single[0].table, "Person");
        // Two tables, an unqualified column: not placed.
        assert!(compared_texts("SELECT * FROM a, b WHERE city = 'x'").is_empty());
        assert!(compared_texts("not sql").is_empty());
    }
}
