//! Node records operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use std::sync::Arc;

use rushdb_lmpg::reader::try_access as access;
use rushdb_types::ids::RecordId;

use super::{bytes_from_hex, hex_simple, IndexedRecord, RequestScope};

impl RequestScope<'_> {
    // ---- record mutation ------------------------------------------------

    /// Parse a canonical record mutation into a BFS plan.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn create_record(
        &self,
        label: &str,
        properties: &serde_json::Value,
        vectors: &[serde_json::Value],
        options: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use rushdb_lmpg::import::{
            import_bfs, BfsPlan, BfsRecord, ImportOptions, InlineVectorEntry,
        };
        ImportOptions::validate_json(options)?;
        let options = ImportOptions::from_json(options);
        let property_object = properties
            .as_object()
            .ok_or_else(|| "record properties must be an object".to_string())?;
        let props = property_object
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    rushdb_lmpg::import::coerce_change(value, &options)
                        .unwrap_or_else(|| rushdb_lmpg::plan::FieldChange::Set(value.clone())),
                )
            })
            .collect();

        let draft_id = *uuid::Uuid::now_v7().as_bytes();
        let mut plan = BfsPlan::default();
        plan.records.push(BfsRecord {
            draft_id,
            label: label.to_string(),
            properties: props,
            explicit_id: None,
        });
        if !vectors.is_empty() {
            let entries: Vec<InlineVectorEntry> =
                serde_json::from_value(serde_json::Value::Array(vectors.to_vec()))
                    .map_err(|error| format!("invalid inline vector: {error}"))?;
            if !entries.is_empty() {
                plan.vector_drafts.push(rushdb_lmpg::import::VectorDraft {
                    draft_id,
                    label: label.to_string(),
                    entries,
                });
            }
        }

        self.validate_inline_vectors(&plan)?;
        let idmap = import_bfs(&self.lmpg(), &plan, &options).map_err(|e| e.to_reason())?;
        self.bind_inline_vectors(&plan, &idmap)?;
        let persisted = idmap
            .iter()
            .find(|m| m.draft_id == draft_id)
            .map_or(draft_id, |m| m.persisted_id);
        self.record_by_id_hex(&persisted)
    }

    /// Create a record while enforcing the caller-owned execution deadline.
    /// Creation has no existing-record lookup, so the pinned view is used for
    /// admission/deadline ownership rather than target selection.
    pub(crate) fn create_record_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id: Option<&str>,
        label: &str,
        properties: &serde_json::Value,
        vectors: &[serde_json::Value],
        options: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use rushdb_lmpg::import::{
            import_bfs_with_cancel, BfsPlan, BfsRecord, ImportOptions, InlineVectorEntry,
        };
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        ImportOptions::validate_json(options)?;
        let options = ImportOptions::from_json(options);
        let property_object = properties
            .as_object()
            .ok_or_else(|| "record properties must be an object".to_string())?;
        let props = property_object
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    rushdb_lmpg::import::coerce_change(value, &options)
                        .unwrap_or_else(|| rushdb_lmpg::plan::FieldChange::Set(value.clone())),
                )
            })
            .collect();
        // A supplied id is honoured; creating over an existing id
        // fails, as `create` promises.
        let explicit_id = id.map(bytes_from_hex).transpose()?;
        if let Some(explicit) = explicit_id {
            if rushdb_lmpg::reader::try_access::record_by_id(
                context.read_view(),
                self.tenant(),
                &RecordId(explicit),
            )
            .map_err(|e| e.to_string())?
            .is_some()
            {
                return Err(format!("record {} already exists", id.unwrap_or_default()));
            }
        }
        let draft_id = explicit_id.unwrap_or_else(|| *uuid::Uuid::now_v7().as_bytes());
        let mut plan = BfsPlan::default();
        plan.records.push(BfsRecord {
            draft_id,
            label: label.to_string(),
            properties: props,
            explicit_id,
        });
        if !vectors.is_empty() {
            let entries: Vec<InlineVectorEntry> =
                serde_json::from_value(serde_json::Value::Array(vectors.to_vec()))
                    .map_err(|error| format!("invalid inline vector: {error}"))?;
            if !entries.is_empty() {
                plan.vector_drafts.push(rushdb_lmpg::import::VectorDraft {
                    draft_id,
                    label: label.to_string(),
                    entries,
                });
            }
        }
        self.validate_inline_vectors(&plan)?;
        let deadline = context.deadline();
        let idmap = import_bfs_with_cancel(&self.lmpg(), &plan, &options, &|| {
            deadline.is_some_and(|at| std::time::Instant::now() >= at)
        })
        .map_err(|e| e.to_reason())?;
        if let Err(bind_error) = self.bind_inline_vectors_with_cancel(&plan, &idmap, &|| {
            deadline.is_some_and(|at| std::time::Instant::now() >= at)
        }) {
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
        let persisted = idmap
            .iter()
            .find(|m| m.draft_id == draft_id)
            .map_or(draft_id, |m| m.persisted_id);
        let record = self.record_by_id_hex(&persisted)?;
        Ok(record)
    }

    /// Upsert without a label (`create` with `mergeBy`/`mergeStrategy` and no
    /// `label`): update the record of any label whose merge keys match, as
    /// `merge` (or `replace` under `mergeStrategy: "rewrite"`) would. Without
    /// a label nothing is created.
    pub(crate) fn upsert_unlabeled_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        properties: &serde_json::Value,
        vectors: &[serde_json::Value],
        options: &serde_json::Value,
    ) -> Result<UnlabeledUpsert, String> {
        use rushdb_lmpg::import::ImportOptions;

        ImportOptions::validate_json(options)?;
        let options = ImportOptions::from_json(options);
        let props: Vec<(String, rushdb_lmpg::plan::FieldChange)> = properties
            .as_object()
            .ok_or_else(|| "record properties must be an object".to_string())?
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    rushdb_lmpg::import::coerce_change(value, &options)
                        .unwrap_or_else(|| rushdb_lmpg::plan::FieldChange::Set(value.clone())),
                )
            })
            .collect();
        let mut matches =
            rushdb_lmpg::import::find_by_keys_any_label(&self.lmpg(), &props, &options)
                .map_err(|e| e.to_reason())?;
        if matches.len() > 1 {
            return Ok(UnlabeledUpsert::Ambiguous(
                matches.into_iter().map(|(label, _)| label).collect(),
            ));
        }
        let Some((_, record)) = matches.pop() else {
            return Ok(UnlabeledUpsert::NoMatch);
        };
        let id = uuid::Uuid::from_bytes(record.record_id)
            .simple()
            .to_string();
        if options.merge_strategy.as_deref() == Some("rewrite") {
            self.replace_record_with_execution_context(context, &id, properties, vectors)
        } else {
            self.merge_record_with_execution_context(context, &id, None, properties, vectors)
        }
        .map(UnlabeledUpsert::Updated)
    }

    /// Whether a record with this id exists at the request's snapshot.
    pub(crate) fn record_exists(
        &self,
        context: &rushdb_engine::ExecutionContext,
        id_hex: &str,
    ) -> Result<bool, String> {
        let id = RecordId(bytes_from_hex(id_hex)?);
        Ok(
            rushdb_lmpg::reader::try_access::record_by_id(context.read_view(), self.tenant(), &id)
                .map_err(|e| e.to_string())?
                .is_some(),
        )
    }

    /// Fetch one canonical record by compact hex id.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn get_record(&self, id_hex: &str) -> Result<serde_json::Value, String> {
        let id = RecordId(bytes_from_hex(id_hex)?);
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        self.get_record_in_view(&view, id)
    }

    /// Fetch one record using the caller's pinned execution view.
    pub(crate) fn get_record_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id_hex: &str,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let id = RecordId(bytes_from_hex(id_hex)?);
        let value = self.get_record_in_view(context.read_view(), id)?;
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        Ok(value)
    }

    pub(super) fn get_record_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        id: RecordId,
    ) -> Result<serde_json::Value, String> {
        let hdr = rushdb_lmpg::reader::try_access::record_by_id(view, self.tenant(), &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "record not found".to_string())?;
        self.record_value_in_view(view, hdr.ordinal)
    }

    /// PATCH semantics: merge incoming properties onto the existing record.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn merge_record(
        &self,
        id_hex: &str,
        properties: &serde_json::Value,
        vectors: &[serde_json::Value],
    ) -> Result<serde_json::Value, String> {
        use rushdb_lmpg::import::{BfsPlan, InlineVectorEntry, UpsertIdMap, VectorDraft};

        let id_bytes = bytes_from_hex(id_hex)?;
        let id = RecordId(id_bytes);
        let lmpg = self.lmpg();
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        let hdr = rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "record not found".to_string())?;
        let props = Self::props_from_properties(properties)?;
        let rr = rushdb_lmpg::RecordRef {
            record_id: hdr.record_id,
            ordinal: hdr.ordinal,
        };
        let mut vector_plan = BfsPlan::default();
        if !vectors.is_empty() {
            let entries: Vec<InlineVectorEntry> =
                serde_json::from_value(serde_json::Value::Array(vectors.to_vec()))
                    .map_err(|error| format!("invalid inline vectors: {error}"))?;
            if !entries.is_empty() {
                vector_plan.vector_drafts.push(VectorDraft {
                    draft_id: id_bytes,
                    label: hdr.label.clone(),
                    entries,
                });
            }
        }
        self.validate_inline_vectors(&vector_plan)?;
        lmpg.update_fields(rr, props).map_err(|e| e.to_reason())?;
        if let Err(error) = self.bind_inline_vectors(
            &vector_plan,
            &[UpsertIdMap {
                draft_id: id_bytes,
                persisted_id: id_bytes,
            }],
        ) {
            return Err(format!(
                "vector binding failed after partial merge; retry is idempotent: {error}"
            ));
        }
        self.record_value(hdr.ordinal)
    }

    /// PATCH semantics using the caller-owned request snapshot for target
    /// lookup and reconstruction inputs.
    /// Merge updates the record when `id_hex` exists and otherwise creates
    /// it with that id under `label`, as `PutMode::Merge` promises.
    pub(crate) fn merge_record_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id_hex: &str,
        label: Option<&str>,
        properties: &serde_json::Value,
        vectors: &[serde_json::Value],
    ) -> Result<serde_json::Value, String> {
        use rushdb_lmpg::import::{BfsPlan, InlineVectorEntry, UpsertIdMap, VectorDraft};

        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let id_bytes = bytes_from_hex(id_hex)?;
        let id = RecordId(id_bytes);
        let lmpg = self.lmpg();
        let Some(hdr) =
            rushdb_lmpg::reader::try_access::record_by_id(context.read_view(), self.tenant(), &id)
                .map_err(|e| e.to_string())?
        else {
            let label = label.ok_or_else(|| {
                "record not found; merge creates a missing record only when `label` is given"
                    .to_string()
            })?;
            return self.create_record_with_execution_context(
                context,
                Some(id_hex),
                label,
                properties,
                vectors,
                &serde_json::json!({}),
            );
        };
        let props = Self::props_from_properties(properties)?;
        let rr = rushdb_lmpg::RecordRef {
            record_id: hdr.record_id,
            ordinal: hdr.ordinal,
        };
        let mut vector_plan = BfsPlan::default();
        if !vectors.is_empty() {
            let entries: Vec<InlineVectorEntry> =
                serde_json::from_value(serde_json::Value::Array(vectors.to_vec()))
                    .map_err(|error| format!("invalid inline vectors: {error}"))?;
            if !entries.is_empty() {
                vector_plan.vector_drafts.push(VectorDraft {
                    draft_id: id_bytes,
                    label: hdr.label.clone(),
                    entries,
                });
            }
        }
        self.validate_inline_vectors(&vector_plan)?;
        lmpg.update_fields(rr, props).map_err(|e| e.to_reason())?;
        if let Err(error) = self.bind_inline_vectors(
            &vector_plan,
            &[UpsertIdMap {
                draft_id: id_bytes,
                persisted_id: id_bytes,
            }],
        ) {
            return Err(format!(
                "vector binding failed after partial merge; retry is idempotent: {error}"
            ));
        }
        self.record_value(hdr.ordinal)
    }

    /// PUT semantics: replace all properties on the existing record.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn replace_record(
        &self,
        id_hex: &str,
        properties: &serde_json::Value,
        vectors: &[serde_json::Value],
    ) -> Result<serde_json::Value, String> {
        use rushdb_lmpg::import::{BfsPlan, InlineVectorEntry, UpsertIdMap, VectorDraft};

        let id_bytes = bytes_from_hex(id_hex)?;
        let id = RecordId(id_bytes);
        let lmpg = self.lmpg();
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        let hdr = rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "record not found".to_string())?;
        let rr = rushdb_lmpg::RecordRef {
            record_id: hdr.record_id,
            ordinal: hdr.ordinal,
        };

        let mut vector_plan = BfsPlan::default();
        if !vectors.is_empty() {
            let entries: Vec<InlineVectorEntry> =
                serde_json::from_value(serde_json::Value::Array(vectors.to_vec()))
                    .map_err(|error| format!("invalid inline vectors: {error}"))?;
            if !entries.is_empty() {
                vector_plan.vector_drafts.push(VectorDraft {
                    draft_id: id_bytes,
                    label: hdr.label.clone(),
                    entries,
                });
            }
        }
        self.validate_inline_vectors(&vector_plan)?;

        // remove current properties then write the provided set
        let existing: Vec<String> =
            rushdb_lmpg::reconstruct::reconstruct_record(&snap, self.tenant(), hdr.ordinal)
                .map_err(|e| e.to_string())?
                .and_then(|r| r.as_object().cloned())
                .map(|o| {
                    o.keys()
                        .filter(|k| !rushdb_types::value::is_reserved_name(k))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
        // One commit: the new properties, plus a delete of every existing
        // one they do not name. A refused commit leaves the record as it was.
        let mut fields = Self::props_from_properties(properties)?;
        let named: std::collections::HashSet<String> =
            fields.iter().map(|(name, _)| name.clone()).collect();
        fields.extend(
            existing
                .into_iter()
                .filter(|name| !named.contains(name))
                .map(|name| (name, rushdb_lmpg::plan::FieldChange::Delete)),
        );
        lmpg.update_fields(rr, fields).map_err(|e| e.to_reason())?;
        if let Err(error) = self.bind_inline_vectors(
            &vector_plan,
            &[UpsertIdMap {
                draft_id: id_bytes,
                persisted_id: id_bytes,
            }],
        ) {
            return Err(format!(
                "vector binding failed after partial replacement; retry is idempotent: {error}"
            ));
        }
        self.record_value(hdr.ordinal)
    }

    /// PUT semantics using the caller-owned request snapshot for target and
    /// existing-property discovery.
    pub(crate) fn replace_record_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id_hex: &str,
        properties: &serde_json::Value,
        vectors: &[serde_json::Value],
    ) -> Result<serde_json::Value, String> {
        use rushdb_lmpg::import::{BfsPlan, InlineVectorEntry, UpsertIdMap, VectorDraft};

        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let id_bytes = bytes_from_hex(id_hex)?;
        let id = RecordId(id_bytes);
        let lmpg = self.lmpg();
        let snap = context.read_view();
        let hdr = rushdb_lmpg::reader::try_access::record_by_id(snap, self.tenant(), &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "record not found".to_string())?;
        let rr = rushdb_lmpg::RecordRef {
            record_id: hdr.record_id,
            ordinal: hdr.ordinal,
        };
        let mut vector_plan = BfsPlan::default();
        if !vectors.is_empty() {
            let entries: Vec<InlineVectorEntry> =
                serde_json::from_value(serde_json::Value::Array(vectors.to_vec()))
                    .map_err(|error| format!("invalid inline vectors: {error}"))?;
            if !entries.is_empty() {
                vector_plan.vector_drafts.push(VectorDraft {
                    draft_id: id_bytes,
                    label: hdr.label.clone(),
                    entries,
                });
            }
        }
        self.validate_inline_vectors(&vector_plan)?;
        let existing: Vec<String> =
            rushdb_lmpg::reconstruct::reconstruct_record(snap, self.tenant(), hdr.ordinal)
                .map_err(|e| e.to_string())?
                .and_then(|r| r.as_object().cloned())
                .map(|o| {
                    o.keys()
                        .filter(|k| !rushdb_types::value::is_reserved_name(k))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
        // One commit: the new properties, plus a delete of every existing
        // one they do not name. A refused commit leaves the record as it was.
        let mut fields = Self::props_from_properties(properties)?;
        let named: std::collections::HashSet<String> =
            fields.iter().map(|(name, _)| name.clone()).collect();
        fields.extend(
            existing
                .into_iter()
                .filter(|name| !named.contains(name))
                .map(|name| (name, rushdb_lmpg::plan::FieldChange::Delete)),
        );
        lmpg.update_fields(rr, fields).map_err(|e| e.to_reason())?;
        if let Err(error) = self.bind_inline_vectors(
            &vector_plan,
            &[UpsertIdMap {
                draft_id: id_bytes,
                persisted_id: id_bytes,
            }],
        ) {
            return Err(format!(
                "vector binding failed after partial replacement; retry is idempotent: {error}"
            ));
        }
        self.record_value(hdr.ordinal)
    }

    /// Delete one record by compact hex id.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn delete_record(&self, id_hex: &str) -> Result<serde_json::Value, String> {
        let id = RecordId(bytes_from_hex(id_hex)?);
        let lmpg = self.lmpg();
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        let hdr = rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "record not found".to_string())?;
        lmpg.delete_record(rushdb_lmpg::RecordRef {
            record_id: hdr.record_id,
            ordinal: hdr.ordinal,
        })
        .map_err(|e| e.to_reason())?;
        Ok(serde_json::json!({ "message": "Record deleted" }))
    }

    /// Delete one record using the caller-owned execution view. The lookup is
    /// pinned to the request snapshot so a concurrent write cannot change the
    /// identity selected by the protocol between authorization and mutation.
    pub(crate) fn delete_record_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id_hex: &str,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let id = RecordId(bytes_from_hex(id_hex)?);
        let lmpg = self.lmpg();
        let hdr =
            rushdb_lmpg::reader::try_access::record_by_id(context.read_view(), self.tenant(), &id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "record not found".to_string())?;
        lmpg.delete_record(rushdb_lmpg::RecordRef {
            record_id: hdr.record_id,
            ordinal: hdr.ordinal,
        })
        .map_err(|e| e.to_reason())?;
        Ok(serde_json::json!({ "message": "Record deleted" }))
    }

    /// Change a record's primary label in one engine commit. The supplied old
    /// label is an optimistic precondition so a stale client cannot overwrite
    /// another reclassification.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn retype_record(
        &self,
        id_hex: &str,
        from_label: &str,
        to_label: &str,
    ) -> Result<serde_json::Value, String> {
        let id = RecordId(bytes_from_hex(id_hex)?);
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        let header = rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "record not found".to_string())?;
        self.lmpg()
            .retype_record(
                rushdb_lmpg::RecordRef {
                    record_id: header.record_id,
                    ordinal: header.ordinal,
                },
                from_label,
                to_label,
            )
            .map_err(|e| e.to_reason())?;
        self.record_value(header.ordinal)
    }

    /// Retype a record selected from the caller-owned execution view.
    pub(crate) fn retype_record_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id_hex: &str,
        from_label: &str,
        to_label: &str,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let id = RecordId(bytes_from_hex(id_hex)?);
        let header =
            rushdb_lmpg::reader::try_access::record_by_id(context.read_view(), self.tenant(), &id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "record not found".to_string())?;
        self.lmpg()
            .retype_record(
                rushdb_lmpg::RecordRef {
                    record_id: header.record_id,
                    ordinal: header.ordinal,
                },
                from_label,
                to_label,
            )
            .map_err(|e| e.to_reason())?;
        self.record_value(header.ordinal)
    }

    /// Bulk record delete in bounded pages, honoring the caller's window.
    /// The match set is snapshotted once (ordinals, not reconstructed JSON)
    /// and each page commits atomically (`ceil(deleted / 500)` commits).
    /// `skip` records are left in place; `limit` caps the total deleted
    /// (`None` drains every match). Inserts that land after the snapshot are
    /// not deleted. Records that vanish before their page commits are skipped.
    /// Returns the number of deleted records.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn delete_records(&self, body: &serde_json::Value) -> Result<u64, String> {
        self.delete_records_with_cancel(body, &|| false)
    }

    /// [`Self::delete_records`] polling `is_cancelled` during the match scan and
    /// between page commits, so a long delete can be aborted mid-flight (E4
    /// operator trust). Pages already committed stay applied; the call returns
    /// `"operation cancelled"`.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn delete_records_with_cancel(
        &self,
        body: &serde_json::Value,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, String> {
        const DELETE_PAGE_SIZE: usize = 500;
        let document_result =
            rushdb_query::document::parse_document(body).map_err(|e| e.to_string())?;
        let skip = document_result.search.pagination.skip;
        let limit = document_result.search.pagination.limit;
        if limit == Some(0) {
            return Ok(0);
        }
        if is_cancelled() {
            return Err("operation cancelled".into());
        }
        // One snapshot of the match set. Pagination is applied here, not inside
        // the matcher, so a label delete does not rescan after every page.
        // Inserts that land after this snapshot are not deleted.
        let lmpg = self.lmpg();
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        let ords = rushdb_query::matching_ordinals(
            &lmpg,
            &document_result.search,
            is_cancelled,
            snap.clone(),
        )
        .map_err(|e| {
            if matches!(e, rushdb_query::QueryError::Cancelled) {
                "operation cancelled".to_string()
            } else {
                e.to_string()
            }
        })?;
        let start = usize::try_from(skip).unwrap_or(usize::MAX).min(ords.len());
        let take = limit.map_or(usize::MAX, |n| usize::try_from(n).unwrap_or(usize::MAX));
        let selected = &ords[start..];
        let selected = &selected[..selected.len().min(take)];
        let mut refs = Vec::with_capacity(selected.len());
        for &ord in selected {
            if is_cancelled() {
                return Err("operation cancelled".into());
            }
            let Some(hdr) =
                access::record_by_ord(&snap, self.tenant(), ord).map_err(|e| e.to_string())?
            else {
                continue;
            };
            refs.push(rushdb_lmpg::RecordRef {
                record_id: hdr.record_id,
                ordinal: hdr.ordinal,
            });
        }
        let mut affected = 0u64;
        for chunk in refs.chunks(DELETE_PAGE_SIZE) {
            if is_cancelled() {
                return Err("operation cancelled".into());
            }
            let page_snap = self.store.read_view().map_err(|e| e.to_string())?;
            let mut batch = lmpg.begin_batch();
            let mut staged = 0u64;
            for rec in chunk {
                if access::record_by_ord(&page_snap, self.tenant(), rec.ordinal)
                    .map_err(|e| e.to_string())?
                    .is_none()
                {
                    continue;
                }
                batch
                    .ops
                    .push(rushdb_lmpg::BatchOp::DeleteRecord { record: *rec });
                staged += 1;
            }
            if staged == 0 {
                continue;
            }
            lmpg.run_batch(batch).map_err(|e| e.to_reason())?;
            affected += staged;
        }
        Ok(affected)
    }

    /// Bulk deletion using the caller-owned execution view for matching and
    /// record identity resolution. Page commits still validate liveness against
    /// the latest store state so concurrent deletes remain idempotent.
    pub(crate) fn delete_records_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        body: &serde_json::Value,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, String> {
        const DELETE_PAGE_SIZE: usize = 500;
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let document_result =
            rushdb_query::document::parse_document(body).map_err(|e| e.to_string())?;
        let skip = document_result.search.pagination.skip;
        let limit = document_result.search.pagination.limit;
        if limit == Some(0) {
            return Ok(0);
        }
        if is_cancelled() {
            return Err("operation cancelled".into());
        }
        let lmpg = self.lmpg();
        let snap = Arc::clone(context.read_view());
        let ords = rushdb_query::matching_ordinals(
            &lmpg,
            &document_result.search,
            is_cancelled,
            snap.clone(),
        )
        .map_err(|e| {
            if matches!(e, rushdb_query::QueryError::Cancelled) {
                "operation cancelled".to_string()
            } else {
                e.to_string()
            }
        })?;
        let start = usize::try_from(skip).unwrap_or(usize::MAX).min(ords.len());
        let take = limit.map_or(usize::MAX, |n| usize::try_from(n).unwrap_or(usize::MAX));
        let selected = &ords[start..];
        let selected = &selected[..selected.len().min(take)];
        let mut refs = Vec::with_capacity(selected.len());
        for &ord in selected {
            if is_cancelled() {
                return Err("operation cancelled".into());
            }
            let Some(hdr) =
                access::record_by_ord(&snap, self.tenant(), ord).map_err(|e| e.to_string())?
            else {
                continue;
            };
            refs.push(rushdb_lmpg::RecordRef {
                record_id: hdr.record_id,
                ordinal: hdr.ordinal,
            });
        }
        let mut affected = 0u64;
        for chunk in refs.chunks(DELETE_PAGE_SIZE) {
            context
                .check_deadline()
                .map_err(|error| error.to_string())?;
            if is_cancelled() {
                return Err("operation cancelled".into());
            }
            let page_snap = self.store.read_view().map_err(|e| e.to_string())?;
            let mut batch = lmpg.begin_batch();
            let mut staged = 0u64;
            for rec in chunk {
                if access::record_by_ord(&page_snap, self.tenant(), rec.ordinal)
                    .map_err(|e| e.to_string())?
                    .is_none()
                {
                    continue;
                }
                batch
                    .ops
                    .push(rushdb_lmpg::BatchOp::DeleteRecord { record: *rec });
                staged += 1;
            }
            if staged == 0 {
                continue;
            }
            lmpg.run_batch(batch).map_err(|e| e.to_reason())?;
            affected += staged;
        }
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        Ok(affected)
    }

    /// Remove a property family (field) from the tenant atomically: all
    /// occurrences on records and relationships, the family row/signature, and
    /// its definitions. Returns the number of occurrences removed.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn delete_property(&self, name: &str) -> Result<serde_json::Value, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        self.delete_property_in_view(name, &snap)?
            .ok_or_else(|| format!("property `{name}` does not exist"))
    }

    /// `None` when no property has this name.
    pub(super) fn delete_property_in_view(
        &self,
        name: &str,
        snap: &rushdb_storage::ReadView,
    ) -> Result<Option<serde_json::Value>, String> {
        use rushdb_types::keys::{key, P_FAMILY};
        use rushdb_types::value::normalize_name;
        let tenant = self.tenant();
        let normalized = normalize_name(name);
        let pref = key(tenant, &[P_FAMILY, b""]);
        // A name has one family per owner scope (records, relationships):
        // deleting the property deletes every one of them, in one commit.
        let mut families: Vec<rushdb_types::occurrence::PropertyFamily> = Vec::new();
        for (_, raw) in snap.range_prefix(&pref).map_err(|e| e.to_string())? {
            let fam: rushdb_types::occurrence::PropertyFamily =
                serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
            if fam.namespace == "default" && fam.normalized_name == normalized {
                families.push(fam);
            }
        }
        if families.is_empty() {
            return Ok(None);
        }
        // Owners carrying the property, from the on-disk support counters of
        // each typed definition (no occurrence scan).
        let mut affected = 0u64;
        for fam in &families {
            for definition in
                rushdb_lmpg::reader::try_access::definitions_of_family(snap, tenant, fam.family_id)
                    .map_err(|e| e.to_string())?
            {
                affected = affected.saturating_add(
                    rushdb_lmpg::derived::prop_support_total(
                        snap,
                        tenant,
                        fam.owner_scope,
                        definition.property_id,
                    )
                    .map_err(|e| e.to_string())?,
                );
            }
        }

        let lmpg = self.lmpg();
        lmpg.store
            .commit_for_tenant(&tenant, uuid::Uuid::now_v7(), move |ci, _txn| {
                let mut batch = rushdb_types::command::CommitBatch {
                    command_version: rushdb_types::command::COMMAND_VERSION,
                    tenant,
                    transaction_id: String::new(),
                    commit_key: uuid::Uuid::nil(),
                    start_index: ci.saturating_sub(1),
                    isolation: rushdb_types::command::IsolationMode::Snapshot,
                    preconditions: vec![],
                    mutations: vec![],
                };
                for fam in families {
                    batch
                        .mutations
                        .push(rushdb_types::command::Mutation::DeleteFamily {
                            family_id: fam.family_id,
                            namespace: fam.namespace,
                            owner_scope: fam.owner_scope,
                            normalized_name: fam.normalized_name,
                            display_name: fam.display_name,
                        });
                }
                Ok((batch, ()))
            })
            .map_err(|e| e.to_string())?;
        Ok(Some(serde_json::json!({
            "message": "Property deleted",
            "deleted": affected,
        })))
    }

    /// Remove a property family using the caller-owned execution view;
    /// `None` when no property has this name.
    pub(crate) fn delete_property_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        name: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.delete_property_in_view(name, context.read_view())?;
        Ok(result)
    }

    pub(super) fn props_from_properties(
        properties: &serde_json::Value,
    ) -> Result<Vec<(String, rushdb_lmpg::plan::FieldChange)>, String> {
        use rushdb_lmpg::plan::FieldChange;
        let changes: Vec<(String, rushdb_lmpg::plan::FieldChange)> = properties
            .as_object()
            .ok_or_else(|| "record properties must be an object".to_string())?
            .iter()
            .map(|(name, value)| {
                let change = rushdb_lmpg::import::coerce_change(
                    value,
                    &rushdb_lmpg::import::ImportOptions::default(),
                )
                .unwrap_or(if value.is_null() {
                    FieldChange::ExplicitNull
                } else {
                    FieldChange::Set(value.clone())
                });
                (name.clone(), change)
            })
            .collect();
        Ok(changes)
    }

    pub(super) fn record_by_id_hex(&self, id: &[u8; 16]) -> Result<serde_json::Value, String> {
        self.get_record(&hex_simple(id))
    }

    // ---- relationships: create-many / delete-many / search --------------

    /// Enumerate records of a label matching an optional `where`, returning
    /// (`record_id`, ordinal, reconstructed object).
    #[cfg(test)]
    pub(super) fn enumerate_records(
        &self,
        label: &str,
        where_: Option<&serde_json::Value>,
    ) -> Result<Vec<IndexedRecord>, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.enumerate_records_in_view(label, where_, &snap)
    }

    pub(super) fn enumerate_records_in_view(
        &self,
        label: &str,
        where_: Option<&serde_json::Value>,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<Vec<IndexedRecord>, String> {
        let document = match where_ {
            Some(w) => serde_json::json!({
                "labels": [label],
                "where": w,
                "pagination": { "skip": 0, "limit": 100_000 }
            }),
            None => serde_json::json!({
                "labels": [label],
                "pagination": { "skip": 0, "limit": 100_000 }
            }),
        };
        let parsed =
            rushdb_query::document::parse_document(&document).map_err(|e| e.to_string())?;
        let lmpg = self.lmpg();
        let page =
            rushdb_query::find_records_in_view(&lmpg, &parsed.search, &|| false, Arc::clone(snap))
                .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for rec in &page.records {
            let obj = rec.as_object().cloned().unwrap_or_default();
            let Some(id_hex) = obj
                .get(rushdb_types::value::ID_FIELD)
                .and_then(|v| v.as_str())
            else {
                continue;
            };
            let Some(bytes) = rushdb_lmpg::import::decode_simple_hex(id_hex) else {
                continue;
            };
            if let Some(hdr) =
                rushdb_lmpg::reader::try_access::record_by_id(snap, self.tenant(), &RecordId(bytes))
                    .map_err(|e| e.to_string())?
            {
                out.push((RecordId(bytes), hdr.ordinal, obj));
            }
        }
        Ok(out)
    }
}

/// Outcome of [`Node::upsert_unlabeled_with_execution_context`].
pub(crate) enum UnlabeledUpsert {
    /// The one matching record, after the update.
    Updated(serde_json::Value),
    /// No record matches the merge keys.
    NoMatch,
    /// Records of these labels match; the caller must name one.
    Ambiguous(Vec<String>),
}
