//! Node ingest operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use super::{import, RequestScope};

impl RequestScope<'_> {
    /// Import one JSON document under `label` and return its persisted record reference.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn import_json_document(
        &self,
        label: &str,
        doc: &serde_json::Value,
    ) -> Result<rushdb_lmpg::RecordRef, String> {
        import::import_document(&self.lmpg(), label, doc).map_err(|e| e.to_reason())
    }

    /// Import newline-delimited JSON documents under `label`.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn import_jsonl_bytes(
        &self,
        label: &str,
        bytes: &[u8],
    ) -> Result<Vec<rushdb_lmpg::RecordRef>, String> {
        let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
        let mut records = Vec::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let doc: serde_json::Value =
                serde_json::from_str(line).map_err(|error| error.to_string())?;
            records.push(self.import_json_document(label, &doc)?);
        }
        Ok(records)
    }

    // ---- native document ingest -----------------------------------------

    /// Ingest schema-free nested documents with explicit merge semantics and
    /// optional caller-supplied vectors.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn ingest_documents(
        &self,
        label: Option<&str>,
        documents: &serde_json::Value,
        options: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.ingest_documents_with_cancel(label, documents, options, &|| false)
    }

    /// Context-owned ingest boundary used by network transports. The legacy
    /// convenience method remains available for embedded callers, while
    /// transport ingest inherits the request deadline and cancellation state
    /// across each durable phase.
    pub(crate) fn ingest_documents_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        label: Option<&str>,
        documents: &serde_json::Value,
        options: &serde_json::Value,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        if is_cancelled() {
            return Err("operation cancelled".into());
        }
        let deadline = context.deadline();
        let result = self.ingest_documents_with_cancel(label, documents, options, &|| {
            is_cancelled() || deadline.is_some_and(|at| std::time::Instant::now() >= at)
        })?;
        if is_cancelled() {
            return Err("operation cancelled".into());
        }
        Ok(result)
    }

    /// Cancellation-aware document ingest. The import planner polls the
    /// callback before each bounded record/relationship phase; vector binding
    /// and result reconstruction are checked at their phase boundaries too.
    pub(crate) fn ingest_documents_with_cancel(
        &self,
        label: Option<&str>,
        documents: &serde_json::Value,
        options: &serde_json::Value,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<serde_json::Value, String> {
        use rushdb_lmpg::import::{serialize_bfs, ImportOptions};
        ImportOptions::validate_json(options)?;
        let options = ImportOptions::from_json(options);
        let plan = serialize_bfs(documents, label.unwrap_or_default(), &options);
        self.validate_inline_vectors(&plan)?;
        let idmap = rushdb_lmpg::import::import_bfs_with_cancel(
            &self.lmpg(),
            &plan,
            &options,
            is_cancelled,
        )
        .map_err(|e| e.to_reason())?;
        if is_cancelled() {
            return Err("import cancelled".into());
        }

        if let Err(bind_error) = self.bind_inline_vectors_with_cancel(&plan, &idmap, is_cancelled) {
            // The create-only ingest path can compensate its record commit if
            // a later vector commit fails. Merge/update imports may have
            // touched pre-existing records, so they return explicit partial
            // progress instead of deleting unrelated data.
            if options.merge_by.is_none() && options.merge_strategy.is_none() {
                if let Err(rollback_error) = self.rollback_ingest_records(&idmap) {
                    return Err(format!(
                        "vector binding failed after ingest ({bind_error}); rollback also failed ({rollback_error})"
                    ));
                }
                return Err(format!(
                    "vector binding failed; ingest rolled back: {bind_error}"
                ));
            }
            return Err(format!(
                "vector binding failed after partial ingest; retry is idempotent: {bind_error}"
            ));
        }

        if is_cancelled() {
            return Err("import cancelled".into());
        }

        let count = plan.records.len();
        if options.return_result && count <= 1000 {
            let records: Vec<serde_json::Value> = {
                let snap = self.store.read_view().map_err(|e| e.to_string())?;
                let proptypes = self.proptypes_cached(&snap)?;
                let active = rushdb_authz::load_active_policy_in_view(&snap, self.tenant())
                    .map_err(|error| error.to_string())?;
                let authz = rushdb_lmpg::AuthorizedView::new(&snap, self.tenant(), active.as_ref());
                idmap
                    .iter()
                    .map(|m| {
                        let h = rushdb_lmpg::reader::try_access::record_by_id(
                            &snap,
                            self.tenant(),
                            &rushdb_types::ids::RecordId(m.persisted_id),
                        )
                        .map_err(|e| e.to_string())?
                        .ok_or("imported record not found")?;
                        let rec = authz
                            .reconstruct_record(h.ordinal)
                            .map_err(|e| e.to_string())?;
                        Ok::<_, String>(
                            rec.map(|rec| Self::canonical_record_with(&rec, &proptypes)),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect()
            };
            Ok(serde_json::json!({
                "count": count,
                "relationships": plan.relations.len(),
                "data": records
            }))
        } else if options.return_result {
            Ok(serde_json::json!({
                "message": format!("Import complete. {count} records processed."),
                "count": count,
                "relationships": plan.relations.len()
            }))
        } else {
            Ok(serde_json::json!({
                "count": count,
                "relationships": plan.relations.len(),
                "data": true
            }))
        }
    }

    /// Bind inline vector drafts through the shared deterministic path after
    /// draft ids are remapped to persisted ids.
    pub(super) fn bind_inline_vectors(
        &self,
        plan: &rushdb_lmpg::import::BfsPlan,
        idmap: &[rushdb_lmpg::import::UpsertIdMap],
    ) -> Result<(), String> {
        self.bind_inline_vectors_with_cancel(plan, idmap, &|| false)
    }

    pub(super) fn bind_inline_vectors_with_cancel(
        &self,
        plan: &rushdb_lmpg::import::BfsPlan,
        idmap: &[rushdb_lmpg::import::UpsertIdMap],
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        // Group the batch's inline vectors per (index, property) and bind each
        // group in ONE commit: one durable commit per vector made an ingest
        // batch of N vectors cost N fsyncs (C9, cloud-capacity F10).
        let mut groups: std::collections::BTreeMap<(String, String), Vec<InlineVector>> =
            std::collections::BTreeMap::new();
        for vd in &plan.vector_drafts {
            let persisted = idmap
                .iter()
                .find(|m| m.draft_id == vd.draft_id)
                .map_or(vd.draft_id, |m| m.persisted_id);
            let hex = crate::node::hex_simple(&persisted);
            for entry in &vd.entries {
                if is_cancelled() {
                    return Err("import cancelled".into());
                }
                let alias = self.resolve_inline_vector_alias(&vd.label, entry)?;
                groups
                    .entry((alias, entry.property_name.clone()))
                    .or_default()
                    .push((hex.clone(), entry.vector.clone()));
            }
        }
        let lmpg = self.lmpg();
        for ((alias, property), vectors) in groups {
            if is_cancelled() {
                return Err("import cancelled".into());
            }
            // The embedding property family may not exist (vectors are
            // external data, not stored occurrences): ensure it first.
            lmpg.ensure_record_family(&property)
                .map_err(|e| e.to_string())?;
            let items: Vec<(&str, &[f32])> = vectors
                .iter()
                .map(|(hex, vector)| (hex.as_str(), vector.as_slice()))
                .collect();
            self.bind_field_vectors_external(&alias, &property, &items)?;
        }
        Ok(())
    }

    pub(super) fn rollback_ingest_records(
        &self,
        idmap: &[rushdb_lmpg::import::UpsertIdMap],
    ) -> Result<(), String> {
        let lmpg = self.lmpg();
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        for mapping in idmap {
            let Some(header) = rushdb_lmpg::reader::try_access::record_by_id(
                &view,
                self.tenant(),
                &rushdb_types::ids::RecordId(mapping.persisted_id),
            )
            .map_err(|e| e.to_string())?
            else {
                continue;
            };
            lmpg.delete_record(rushdb_lmpg::RecordRef {
                record_id: mapping.persisted_id,
                ordinal: header.ordinal,
            })
            .map_err(|e| e.to_reason())?;
        }
        Ok(())
    }

    /// Resolve every inline vector against the scope-local index registry.
    /// Validation happens before record import so a bad or ambiguous vector
    /// cannot leave a partially-created record behind.
    pub(super) fn validate_inline_vectors(
        &self,
        plan: &rushdb_lmpg::import::BfsPlan,
    ) -> Result<(), String> {
        for draft in &plan.vector_drafts {
            for entry in &draft.entries {
                self.resolve_inline_vector_alias(&draft.label, entry)?;
            }
        }
        Ok(())
    }

    pub(super) fn resolve_inline_vector_alias(
        &self,
        label: &str,
        entry: &rushdb_lmpg::import::InlineVectorEntry,
    ) -> Result<String, String> {
        // An explicit index name is authoritative: validate it exists so a bad
        // vector fails before any record is created, then use it directly
        // (no shape guessing among same-dimension siblings).
        // Resolution is a metadata lookup, never a readiness-stats computation:
        // computing stats per vector made binding cost O(index size) per vector
        // (C9, cloud-capacity F10).
        if let Some(index) = entry.index.as_deref() {
            if !self.vector_index_exists(index)? {
                return Err(format!("no embedding index named `{index}`"));
            }
            return Ok(index.to_string());
        }
        if self.require_vector_index_name {
            return Err("inline `$vectors` require an explicit `index` \
                 (resources.require-vector-index-name is on)"
                .into());
        }
        let mut matches: Vec<String> = self
            .vector_index_rows()?
            .into_iter()
            .filter(|index| {
                index.get("label").and_then(|v| v.as_str()) == Some(label)
                    && index.get("property").and_then(|v| v.as_str())
                        == Some(entry.property_name.as_str())
                    && index.get("dimensions").and_then(serde_json::Value::as_u64)
                        == Some(entry.vector.len() as u64)
                    && entry
                        .similarity_function
                        .as_deref()
                        .is_none_or(|similarity| {
                            index.get("similarity").and_then(|v| v.as_str()) == Some(similarity)
                        })
            })
            .filter_map(|index| {
                index
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect();
        matches.sort();
        matches.dedup();

        match matches.as_slice() {
            [alias] => Ok(alias.clone()),
            [] => Err(format!(
                "no matching external embedding index for label `{label}`, property `{}`, and {} dimensions",
                entry.property_name,
                entry.vector.len()
            )),
            _ => Err(format!(
                "multiple external embedding indexes match label `{label}` and property `{}`; similarityFunction is required",
                entry.property_name
            )),
        }
    }
}

/// One inline vector awaiting its batch bind: `(record id hex, components)`.
type InlineVector = (String, Vec<f32>);
