//! CHK-1: what a query names that the catalog of its snapshot does not
//! hold: labels without records, properties no record of those labels
//! carries, and string literals compared by equality that the property
//! never stores. Each is one counter or dictionary probe; suggestions are
//! the caller's (they are only worth reading on a miss).

use rushdb_lmpg::reader::TryRead;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::model::{LogicalType, TypeState};
use serde_json::Value as Json;

use crate::planner::{family_id, typed_pids};
use crate::QueryError;

/// What a query names that its snapshot does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogMiss {
    /// No record carries this label.
    Label(String),
    /// No record of `labels` (any, when empty) carries this property.
    Property {
        /// The labels it was looked for under.
        labels: Vec<String>,
        /// The property as the query spells it.
        property: String,
    },
    /// No record of `labels` stores this text in the property.
    Value {
        /// The labels it was looked for under.
        labels: Vec<String>,
        /// The property as the query spells it.
        property: String,
        /// The literal.
        value: String,
    },
}

/// Logical operators whose operands are predicates of the same records.
const LOGICAL: [&str; 5] = ["$and", "$or", "$nor", "$not", "$xor"];

/// The misses of a search query document's `labels` and `where` (or
/// `predicate`) clause. A key of the clause that names a label with
/// records is a traversal block, checked against that label.
///
/// # Errors
/// Returns [`QueryError`] if a catalog row cannot be read.
pub fn check_search<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    labels: &[String],
    clause: Option<&Json>,
) -> Result<Vec<CatalogMiss>, QueryError> {
    let mut misses = Vec::new();
    for label in labels {
        if rushdb_lmpg::derived::label_count(snap, tenant, label)? == 0 {
            misses.push(CatalogMiss::Label(label.clone()));
        }
    }
    if let Some(Json::Object(clause)) = clause {
        let mut checker = Checker {
            snap,
            tenant,
            misses: &mut misses,
        };
        checker.clause(labels, clause)?;
    }
    misses.dedup();
    Ok(misses)
}

struct Checker<'a, R> {
    snap: &'a R,
    tenant: TenantId,
    misses: &'a mut Vec<CatalogMiss>,
}

impl<R: TryRead> Checker<'_, R> {
    fn clause(
        &mut self,
        labels: &[String],
        clause: &serde_json::Map<String, Json>,
    ) -> Result<(), QueryError> {
        for (key, value) in clause {
            if LOGICAL.contains(&key.as_str()) {
                for operand in operands(value) {
                    if let Json::Object(operand) = operand {
                        self.clause(labels, operand)?;
                    }
                }
                continue;
            }
            if key.starts_with('$') {
                continue;
            }
            // A traversal block: `{"COMPANY": {"name": "Acme"}}`.
            if value.is_object()
                && rushdb_lmpg::derived::label_count(self.snap, self.tenant, key)? > 0
            {
                if let Json::Object(block) = value {
                    self.clause(std::slice::from_ref(key), block)?;
                }
                continue;
            }
            self.property(labels, key, value)?;
        }
        Ok(())
    }

    fn property(&mut self, labels: &[String], name: &str, value: &Json) -> Result<(), QueryError> {
        let missing = || CatalogMiss::Property {
            labels: labels.to_vec(),
            property: name.to_owned(),
        };
        let Some(fid) = family_id(self.snap, self.tenant, &crate::normalize(name))? else {
            self.misses.push(missing());
            return Ok(());
        };
        let typed = typed_pids(self.snap, self.tenant, fid)?;
        let mut carried = false;
        for (_, pid) in &typed {
            if self.support(labels, *pid)? > 0 {
                carried = true;
                break;
            }
        }
        if !carried {
            self.misses.push(missing());
            return Ok(());
        }
        for literal in equality_literals(value) {
            if !self.stores(labels, &typed, literal)? {
                self.misses.push(CatalogMiss::Value {
                    labels: labels.to_vec(),
                    property: name.to_owned(),
                    value: literal.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// Records of `labels` (any, when empty) carrying `pid`.
    fn support(&self, labels: &[String], pid: u64) -> Result<u64, QueryError> {
        if labels.is_empty() {
            return Ok(rushdb_lmpg::derived::prop_support_total(
                self.snap,
                self.tenant,
                OwnerKind::Record,
                pid,
            )?);
        }
        let mut total = 0u64;
        for label in labels {
            total = total.saturating_add(rushdb_lmpg::derived::prop_support_label(
                self.snap,
                self.tenant,
                OwnerKind::Record,
                pid,
                label,
            )?);
        }
        Ok(total)
    }

    /// Whether some record of `labels` stores `literal` in the property,
    /// or another of its types reads the literal as a value of its own (a
    /// date, a datetime): then it is not a spelling to check.
    fn stores(
        &self,
        labels: &[String],
        typed: &[(TypeState, u64)],
        literal: &str,
    ) -> Result<bool, QueryError> {
        let text = Json::String(literal.to_owned());
        let mut utf8 = None;
        for (state, pid) in typed {
            match state {
                TypeState::Known(LogicalType::Utf8) => utf8 = Some(*pid),
                TypeState::Known(ty) => {
                    if rushdb_types::types::coerce_literal(*ty, &text).is_some() {
                        return Ok(true);
                    }
                }
                TypeState::Unresolved => {}
            }
        }
        let Some(pid) = utf8 else {
            return Ok(false);
        };
        let mut entry = vec![LogicalType::Utf8.tag()];
        entry.extend(
            rushdb_types::encoding::encode_order_preserving(
                LogicalType::Utf8,
                &rushdb_types::value::Scalar::Utf8(literal.to_owned()),
            )
            .map_err(|_| QueryError::InvalidPredicate(literal.to_owned()))?,
        );
        // Long values have no dictionary row to probe: not checked.
        if entry.len() > rushdb_types::keys::DICT_MAX_VALUE_BYTES {
            return Ok(true);
        }
        let scope = if labels.is_empty() {
            rushdb_lmpg::derived::labels_of_pid(self.snap, self.tenant, OwnerKind::Record, pid)?
        } else {
            labels.to_vec()
        };
        for label in &scope {
            if rushdb_lmpg::derived::value_count(
                self.snap,
                self.tenant,
                OwnerKind::Record,
                label,
                pid,
                &entry,
            )? > 0
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// The operands of a logical operator: an array, or one object.
fn operands(value: &Json) -> Vec<&Json> {
    match value {
        Json::Array(items) => items.iter().collect(),
        other => vec![other],
    }
}

/// The text literals a property condition compares by equality: a bare
/// string, an array of them (`$in`), and the operands of `$eq`, `$ne`,
/// `$in` and `$nin`.
fn equality_literals(value: &Json) -> Vec<&str> {
    fn texts(value: &Json) -> Vec<&str> {
        match value {
            Json::String(text) => vec![text.as_str()],
            Json::Array(items) => items.iter().filter_map(Json::as_str).collect(),
            _ => Vec::new(),
        }
    }
    match value {
        Json::Object(operators) => operators
            .iter()
            .filter(|(op, _)| matches!(op.as_str(), "$eq" | "$ne" | "$in" | "$nin"))
            .flat_map(|(_, operand)| texts(operand))
            .collect(),
        other => texts(other),
    }
}
