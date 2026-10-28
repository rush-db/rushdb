//! Vector-index metadata reads: existence and raw rows for write-path alias
//! resolution, and the readiness/coverage statistics served by `describe`.
//! Statistics come from the derived catalog and binding rows, never from a
//! reconstruction of every record (C9, cloud-capacity F10).

use super::{access, Arc, RequestScope};

impl RequestScope<'_> {
    /// Exact number of `label` records carrying record property `property`,
    /// from the derived support catalog (summed over the property family's
    /// typed definitions).
    fn property_support_in_label(
        &self,
        snap: &rushdb_storage::ReadView,
        label: &str,
        property: &str,
    ) -> Result<u64, String> {
        let tenant = self.tenant();
        let sig = rushdb_lmpg::reader::keys_ext::family_sig_key(
            tenant,
            "default",
            rushdb_types::ids::OwnerKind::Record,
            &rushdb_types::value::normalize_name(property),
        );
        let Some(raw) = snap.get(&sig).map_err(|e| e.to_string())? else {
            return Ok(0);
        };
        let family = raw
            .get(..8)
            .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
            .map(u64::from_be_bytes)
            .ok_or("corrupt property family signature")?;
        let mut total = 0u64;
        for definition in
            access::definitions_of_family(snap, tenant, family).map_err(|e| e.to_string())?
        {
            total = total.saturating_add(
                rushdb_lmpg::derived::prop_support_label(
                    snap,
                    tenant,
                    rushdb_types::ids::OwnerKind::Record,
                    definition.property_id,
                    label,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        Ok(total)
    }

    /// Raw vector-index metadata rows (no readiness stats): the cheap form
    /// for alias resolution on the write path.
    pub(super) fn vector_index_rows(&self) -> Result<Vec<serde_json::Value>, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        let prefix = rushdb_types::keys::vector_index_meta_prefix(self.tenant()).into_bytes();
        snap.range_prefix(&prefix)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(_, value)| {
                serde_json::from_slice::<serde_json::Value>(&value)
                    .map_err(|e| format!("corrupt vector index: {e}"))
            })
            .collect()
    }

    /// Whether a vector index named `alias` exists (one point read).
    pub(super) fn vector_index_exists(&self, alias: &str) -> Result<bool, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        Ok(snap
            .get(self.vector_index_key(alias).as_bytes())
            .map_err(|e| e.to_string())?
            .is_some())
    }

    /// Describe vector index coverage.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn vector_index_stats(&self, id: &str) -> Result<serde_json::Value, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.vector_index_stats_in_view(id, &snap)
    }

    pub(crate) fn vector_index_stats_in_view(
        &self,
        id: &str,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<serde_json::Value, String> {
        let raw = snap
            .get(self.vector_index_key(id).as_bytes())
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "index not found".to_string())?;
        let row: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|error| format!("corrupt vector index metadata: {error}"))?;
        let property = row
            .get("property")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let label = row
            .get("label")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let vectors = self.vector_engine()?;
        let space = vectors
            .lookup_space_in_view(snap, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "index space missing".to_string())?;
        // indexedRecords = READY bindings only, matching the ANN search set
        // (build_generation filters `status == Ready`). Retired/Stale bindings
        // (deleted/updated sources) are not indexed.
        let counts = vectors
            .binding_counts_in_view(snap, space.space_id)
            .map_err(|e| e.to_string())?;
        let (indexed, stale, retired, tracked) =
            (counts.ready, counts.stale, counts.retired, counts.tracked);
        let external = counts.total.saturating_sub(tracked);

        // totalRecords = records of `label` that carry `property` (bound or
        // not); indexedRecords = bound vectors. The count comes from the exact
        // derived property-support catalog, not a reconstruction of every
        // record of the label (C9, cloud-capacity F10). Inline/external
        // vectors are not stored properties, so for them the bound vectors
        // are the records that carry the property.
        let supported = usize::try_from(self.property_support_in_label(snap, &label, &property)?)
            .unwrap_or(usize::MAX);
        let total = supported.max(indexed + stale);
        let unindexed = total.saturating_sub(indexed);
        let status = if total == 0 {
            "awaiting_vectors"
        } else if unindexed == 0 && stale == 0 {
            "ready"
        } else {
            "behind"
        };
        let ann_lifecycle = vectors
            .ann_lifecycle_in_view(snap, &space)
            .map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "totalRecords": total,
            "indexedRecords": indexed,
            "unindexedRecords": unindexed,
            "staleBindings": stale,
            "retiredBindings": retired,
            "sourceTrackedBindings": tracked,
            "externalBindings": external,
            "status": status,
            "annLifecycle": ann_lifecycle,
            "index": row,
        }))
    }
}
