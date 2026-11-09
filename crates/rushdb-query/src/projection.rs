//! A listing's projection: the rows of a `select` made of plain field
//! references, read field by field instead of rebuilding each record.

use std::collections::{BTreeSet, HashSet};

use rushdb_lmpg::reconstruct::ReconstructSession;
use serde_json::Map;

use crate::{normalize, resolve_field, Json, PointFields, QueryError, QuerySnapshot};

/// How a page's projected rows are read: one point read per field when every
/// field is a scalar property ([`PointFields`]), else the record folded into
/// just the named properties.
pub(crate) struct Projection<'r> {
    fields: &'r [(String, String)],
    point: Option<PointFields>,
    selected: HashSet<String>,
}

impl<'r> Projection<'r> {
    /// The reader for `fields` (`(output name, field)`); `None` without a
    /// projection (the page returns whole records).
    pub(crate) fn resolve(
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        fields: &'r [(String, String)],
    ) -> Result<Option<Self>, QueryError> {
        if fields.is_empty() {
            return Ok(None);
        }
        let names: BTreeSet<&str> = fields
            .iter()
            .map(|(_, field)| field.strip_prefix("$record.").unwrap_or(field))
            .collect();
        let point = PointFields::resolve(snap, tenant, names.iter().copied())?;
        let selected = names
            .iter()
            .flat_map(|name| [(*name).to_owned(), normalize(name)])
            .collect();
        Ok(Some(Self {
            fields,
            point,
            selected,
        }))
    }

    /// Record `ord`'s row: each projected field it holds under its output
    /// name, in `select` order. `None` when the record is gone.
    pub(crate) fn row(
        &self,
        session: &mut ReconstructSession<'_, QuerySnapshot>,
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        ord: u64,
    ) -> Result<Option<Json>, QueryError> {
        let record = match &self.point {
            Some(point) => point.row(snap, tenant, ord)?,
            None => match session.reconstruct_record_selected(ord, &self.selected)? {
                Some(Json::Object(record)) => record,
                _ => return Ok(None),
            },
        };
        let mut row = Map::new();
        for (output, field) in self.fields {
            if let Some(value) = resolve_field(&record, field) {
                row.insert(output.clone(), value);
            }
        }
        Ok(Some(Json::Object(row)))
    }
}
