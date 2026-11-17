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
        Ok(Some(self.shape(&record)))
    }

    /// The rows of `ords`, in order, when every field is a point read
    /// (`None` otherwise): read in fixed-size parts on the shared thread
    /// budget.
    pub(crate) fn point_rows(
        &self,
        snap: &QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        ords: &[u64],
    ) -> Result<Option<Vec<Json>>, QueryError> {
        /// Rows one part reads.
        const PART_ROWS: usize = 512;
        let Some(point) = &self.point else {
            return Ok(None);
        };
        let parts = rushdb_storage::parallel::run_parts(ords.len().div_ceil(PART_ROWS), |part| {
            let end = ((part + 1) * PART_ROWS).min(ords.len());
            ords[part * PART_ROWS..end]
                .iter()
                .map(|&ord| Ok(self.shape(&point.row(snap, tenant, ord)?)))
                .collect::<Result<Vec<Json>, QueryError>>()
        })?;
        Ok(Some(parts.into_iter().flatten().collect()))
    }

    /// A row of the projected fields `record` holds, under their output
    /// names, in `select` order.
    fn shape(&self, record: &Map<String, Json>) -> Json {
        let mut row = Map::new();
        for (output, field) in self.fields {
            if let Some(value) = resolve_field(record, field) {
                row.insert(output.clone(), value);
            }
        }
        Json::Object(row)
    }
}
