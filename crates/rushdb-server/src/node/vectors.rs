//! Node vectors operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use super::{
    access, append_bounded_json_response, key, normalize_vector_owner, similarity_from_str,
    validate_index_alias, Arc, HashSet, Node, OwnerKind, RequestScope, P_FAMILY_SIG,
};

impl Node {
    /// Whether every configured vector index has caught up with its source
    /// records. Used by `/ready` when `require_index_warmup` is set so traffic
    /// is not routed to a node whose ANN indexes are still warming.
    ///
    /// Multitenant mode reports `true`: there is no request scope on the
    /// readiness path, so a cross-tenant scan is neither possible nor desired.
    /// A read failure fails closed (`false`).
    #[must_use]
    pub fn indexes_warmed(&self) -> bool {
        if self.multitenant {
            return true;
        }
        let scope = RequestScope::for_tenant(self, self.base_tenant_id(), None);
        match scope.vector_indexes_all() {
            Ok(indexes) => indexes.iter().all(|index| {
                // `ready` (all bindings current) and `awaiting_vectors` (no
                // source values yet) are both settled; `behind` is not.
                index.get("status").and_then(serde_json::Value::as_str) != Some("behind")
            }),
            Err(_) => false,
        }
    }

    /// Tenant-explicit metadata write for callers that must not depend on a
    /// per-request scope (for example global retention state persisted by a
    /// background worker that has no single "current" tenant). Takes the
    /// tenant as an explicit argument rather than through `RequestScope`, so
    /// it stays callable from genuinely tenant-free maintenance paths.
    pub(super) fn put_meta_blob_for(
        &self,
        tenant: rushdb_types::ids::TenantId,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        let blob = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        let k = key.to_string();
        let outcome = self
            .store
            .commit_for_tenant(&tenant, uuid::Uuid::now_v7(), move |_ci, _txn| {
                #[derive(serde::Serialize, serde::Deserialize)]
                struct P {}
                let batch = rushdb_types::command::CommitBatch {
                    command_version: rushdb_types::command::COMMAND_VERSION,
                    tenant,
                    transaction_id: String::new(),
                    commit_key: uuid::Uuid::nil(),
                    start_index: 0,
                    isolation: rushdb_types::command::IsolationMode::Snapshot,
                    preconditions: vec![],
                    mutations: vec![rushdb_types::command::Mutation::SetMetaBlob {
                        key: k.clone(),
                        value: blob,
                    }],
                };
                Ok((batch, P {}))
            })
            .map_err(|e| e.to_string())?;
        match outcome {
            rushdb_storage::CommitOutcome::Committed { .. } => Ok(()),
            rushdb_storage::CommitOutcome::Conflicted(r) => Err(r),
        }
    }
}

impl RequestScope<'_> {
    // ---- AI data-plane surface (embedding-index registry) ----------------
    // The DB owns vector storage/index metadata and exact search. Embedding
    // generation, NL->query, and schema-gen are the NestJS control plane's
    // responsibility; the DB never calls a model.

    pub(super) fn vector_index_key(&self, alias: &str) -> String {
        format!(
            "{}{alias}",
            rushdb_types::keys::vector_index_meta_prefix(self.tenant())
        )
    }

    pub(super) fn put_meta_blob(&self, key: &str, value: &serde_json::Value) -> Result<(), String> {
        let tenant = self.tenant();
        self.put_meta_blob_for(tenant, key, value)
    }

    pub(super) fn vector_indexes_all_in_view(
        &self,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let prefix = rushdb_types::keys::vector_index_meta_prefix(self.tenant()).into_bytes();
        let mut out = Vec::new();
        for (_, v) in snap.range_prefix(&prefix).map_err(|e| e.to_string())? {
            let mut row = serde_json::from_slice::<serde_json::Value>(&v)
                .map_err(|e| format!("corrupt vector index: {e}"))?;
            let alias = row
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or("corrupt vector index: missing name")?;
            // Index metadata is durable, but readiness is derived from the
            // canonical bindings and source records at the read snapshot. Do
            // not persist a mutable "ready" bit that can survive a source
            // update or restart incorrectly.
            let stats = self.vector_index_stats_in_view(alias, snap)?;
            row["status"] = stats["status"].clone();
            row["indexedRecords"] = stats["indexedRecords"].clone();
            row["totalRecords"] = stats["totalRecords"].clone();
            row["unindexedRecords"] = stats["unindexedRecords"].clone();
            row["staleBindings"] = stats["staleBindings"].clone();
            row["retiredBindings"] = stats["retiredBindings"].clone();
            row["sourceTrackedBindings"] = stats["sourceTrackedBindings"].clone();
            row["externalBindings"] = stats["externalBindings"].clone();
            out.push(row);
        }
        Ok(out)
    }

    pub(super) fn vector_indexes_all(&self) -> Result<Vec<serde_json::Value>, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.vector_indexes_all_in_view(&snap)
    }

    /// Create or resolve a vector index over a record property.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    #[expect(
        clippy::too_many_lines,
        reason = "index definition validation, metadata publication, and retention acknowledgement form one atomic lifecycle"
    )]
    pub fn create_vector_index(
        &self,
        definition: &rushdb_protocol::VectorIndexDefinition,
    ) -> Result<serde_json::Value, String> {
        let property = definition.property.clone();
        let label = definition.label.clone();
        let similarity = definition.similarity.clone();
        let dims = definition.dimensions;
        if label.trim().is_empty() || property.trim().is_empty() || dims == 0 {
            return Err(
                "vector index label/property must be set and dimensions must be positive".into(),
            );
        }

        let sim = similarity_from_str(&similarity)?;
        let owner = normalize_vector_owner(definition.owner.as_deref())?;
        // Ensure the owner-scoped property family exists so external vectors
        // can be bound and the backfill helper can resolve the property id.
        if owner == "relationship" {
            self.lmpg()
                .ensure_relationship_family(&property)
                .map_err(|e| e.to_reason())?;
        } else {
            self.lmpg()
                .ensure_record_family(&property)
                .map_err(|e| e.to_reason())?;
        }
        // Stable opaque name: the caller's `name` when provided, else a
        // deterministic alias required by Core's current `emb__...` contract.
        // The engine
        // carries vectors only; label/property/similarity/dims live in the
        // metadata record below, not in the name.
        let derived = rushdb_lmpg::import::external_space_alias(
            &label,
            &property,
            &similarity,
            dims as usize,
        );
        let name = match &definition.name {
            Some(explicit) => {
                validate_index_alias(explicit, &derived)?;
                explicit.clone()
            }
            None if self.require_vector_index_name => {
                return Err(
                    "vector index requires an explicit `name`; the derived alias cannot \
                     distinguish two same-shape embedding models \
                     (resources.require-vector-index-name is on)"
                        .into(),
                );
            }
            None => derived,
        };
        // A name's definition is immutable: a different model would mix
        // incomparable vector spaces, and a different label, property or
        // owner would silently repoint the index. Re-declaring the same
        // definition is a no-op that keeps the generation, so in-flight
        // embedding jobs stay valid.
        let mut unchanged = None;
        if let Some(raw) = self
            .store
            .try_get(self.vector_index_key(&name).as_bytes())
            .map_err(|e| e.to_string())?
        {
            let existing = serde_json::from_slice::<serde_json::Value>(&raw)
                .map_err(|e| format!("corrupt vector index: {e}"))?;
            let existing_model = existing.get("model").and_then(|v| v.as_str());
            if existing_model != definition.model.as_deref() {
                return Err(format!(
                    "vector index `{name}` already exists with a different model \
                     ({existing_model:?} != {:?})",
                    definition.model
                ));
            }
            for (field, requested) in [
                ("label", label.as_str()),
                ("property", property.as_str()),
                ("owner", owner),
            ] {
                let stored = existing.get(field).and_then(|v| v.as_str());
                if stored != Some(requested) {
                    return Err(format!(
                        "vector index `{name}` already exists for a different {field} \
                         ({stored:?} != {requested:?}); drop it first"
                    ));
                }
            }
            let same_shape = existing
                .get("dimensions")
                .and_then(serde_json::Value::as_u64)
                == Some(u64::from(dims))
                && existing.get("similarity").and_then(|v| v.as_str()) == Some(similarity.as_str());
            if same_shape {
                unchanged = Some(existing);
            }
        }
        self.vector_engine()?
            .ensure_space_similarity(&name, dims, sim)
            .map_err(|e| e.to_string())?;

        let vectors = self.vector_engine()?;
        let space = vectors
            .lookup_space_by_alias(&name)
            .map_err(|e| e.to_string())?;
        let ready = match space {
            Some(sp) => !vectors
                .bindings_of(sp.space_id)
                .map_err(|e| e.to_string())?
                .is_empty(),
            None => false,
        };
        let status = if ready { "ready" } else { "awaiting_vectors" };
        let name_source = if definition.name.is_some() {
            "explicit"
        } else {
            "derived"
        };
        let row = if let Some(existing) = unchanged {
            existing
        } else {
            let row = serde_json::json!({
                "id": name,
                "name": name,
                "label": label,
                "property": property,
                "similarity": similarity,
                "dimensions": dims,
                "status": status,
                "model": definition.model,
                "nameSource": name_source,
                "owner": owner,
                "generation": uuid::Uuid::now_v7().to_string(),
            });
            self.put_meta_blob(&self.vector_index_key(&name), &row)?;
            row
        };
        // A re-declaration can happen after records were added or bindings
        // became stale. Reconcile the published metadata with the same live
        // coverage calculation exposed by `vector_index_stats`, rather than
        // treating the presence of one binding as proof of readiness.
        let live_status = self
            .vector_index_stats(&name)?
            .get("status")
            .and_then(|value| value.as_str())
            .map(str::to_owned);
        if let Some(live_status) = live_status {
            if row.get("status").and_then(|value| value.as_str()) != Some(live_status.as_str()) {
                let mut reconciled = row;
                reconciled["status"] = serde_json::Value::String(live_status);
                self.put_meta_blob(&self.vector_index_key(&name), &reconciled)?;
                return Ok(reconciled);
            }
        }
        Ok(row)
    }

    pub(crate) fn create_vector_index_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        definition: &rushdb_protocol::VectorIndexDefinition,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.create_vector_index(definition)?;
        Ok(result)
    }

    /// Reconcile a managed vector index's binding statuses against live data:
    /// records whose source occurrence is gone become `Retired`, changed values
    /// become `Stale`. Core calls this for managed indexes during schema
    /// recalculation so a stale `ready` index reports accurate counts and can
    /// be returned to `pending` for re-embedding.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn revalidate_vector_index(&self, id: &str) -> Result<serde_json::Value, String> {
        let vectors = self.vector_engine()?;
        // Status reconciliation is the durable, primary effect. The ANN
        // rebuild is a derived-cache warmup: a build failure must not fail the
        // operation after the binding statuses have already been committed, and
        // the index is rebuilt lazily on the next ANN search. Surface the
        // failure in the response instead of returning an error over a partial
        // mutation.
        let (stale, retired) = vectors.revalidate_bindings(id).map_err(|e| e.to_string())?;
        let (ann_entries, ann_rebuild_error) = match vectors.build_ann(id) {
            Ok(generation) => (Some(generation.entries.len()), None),
            Err(error) => (None, Some(error)),
        };
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        let space = vectors
            .lookup_space_in_view(&snap, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "index space missing".to_string())?;
        let ann_lifecycle = vectors
            .ann_lifecycle_in_view(&snap, &space)
            .map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "stale": stale,
            "retired": retired,
            "annEntries": ann_entries,
            "annLifecycle": ann_lifecycle,
            "annRebuildError": ann_rebuild_error,
        }))
    }

    pub(crate) fn revalidate_vector_index_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id: &str,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.revalidate_vector_index(id)?;
        Ok(result)
    }

    /// List vector indexes in the active scope.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn list_vector_indexes(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({ "data": self.vector_indexes_all()? }))
    }

    pub(crate) fn list_vector_indexes_in_view(
        &self,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "data": self.vector_indexes_all_in_view(snap)?
        }))
    }

    /// Remove bindings, the vector-space alias, and its registry entry.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn drop_vector_index(&self, id: &str) -> Result<serde_json::Value, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        if snap
            .get(self.vector_index_key(id).as_bytes())
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err("index not found".to_string());
        }
        let cleared = self
            .vector_engine()?
            .clear_space(id)
            .map_err(|e| e.to_string())?;
        // remove the registry row
        let k = self.vector_index_key(id);
        let tenant = self.tenant();
        let outcome = self
            .store
            .commit_for_tenant(&tenant, uuid::Uuid::now_v7(), move |_ci, _txn| {
                #[derive(serde::Serialize, serde::Deserialize)]
                struct P {}
                let batch = rushdb_types::command::CommitBatch {
                    command_version: rushdb_types::command::COMMAND_VERSION,
                    tenant,
                    transaction_id: String::new(),
                    commit_key: uuid::Uuid::nil(),
                    start_index: 0,
                    isolation: rushdb_types::command::IsolationMode::Snapshot,
                    preconditions: vec![],
                    mutations: vec![rushdb_types::command::Mutation::DeleteMetaBlob { key: k }],
                };
                Ok((batch, P {}))
            })
            .map_err(|e| e.to_string())?;
        match outcome {
            rushdb_storage::CommitOutcome::Committed { .. } => {}
            rushdb_storage::CommitOutcome::Conflicted(r) => return Err(r),
        }
        Ok(serde_json::json!({ "message": "Index deleted", "cleared": cleared }))
    }

    pub(crate) fn drop_vector_index_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id: &str,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.drop_vector_index(id)?;
        Ok(result)
    }

    /// Upsert caller-supplied vectors into an index.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn upsert_vector_index(
        &self,
        id: &str,
        items: &[rushdb_protocol::VectorItem],
    ) -> Result<serde_json::Value, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        let raw = snap
            .get(self.vector_index_key(id).as_bytes())
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "index not found".to_string())?;
        let row: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|error| format!("corrupt vector index metadata: {error}"))?;
        let property = row
            .get("property")
            .and_then(|x| x.as_str())
            .ok_or_else(|| "corrupt vector index metadata: missing property".to_string())?
            .to_string();
        let dimensions = row
            .get("dimensions")
            .and_then(serde_json::Value::as_u64)
            .and_then(|d| u32::try_from(d).ok())
            .ok_or_else(|| "corrupt vector index metadata: invalid dimensions".to_string())?;
        let actual_generation = row
            .get("generation")
            .and_then(|value| value.as_str())
            .map(str::to_string);

        // Validate the batch identity before publishing any binding. A stale
        // completion in the same request must not leave earlier items
        // partially attached to the current generation.
        for item in items {
            if let Some(expected_generation) = item.index_generation.as_deref() {
                let Some(actual_generation) = actual_generation.as_deref() else {
                    return Err("vector index metadata has no generation".into());
                };
                if expected_generation != actual_generation {
                    return Err(format!(
                        "stale vector index generation (expected `{expected_generation}`, current `{actual_generation}`)"
                    ));
                }
            }
            if u32::try_from(item.vector.len()).ok() != Some(dimensions) {
                return Err(format!(
                    "vector dimension {} does not match index dimension {dimensions}",
                    item.vector.len()
                ));
            }
        }

        let lmpg = self.lmpg();
        let owner = row
            .get("owner")
            .and_then(|v| v.as_str())
            .unwrap_or("record");
        if owner == "relationship" {
            lmpg.ensure_relationship_family(&property)
                .map_err(|e| e.to_reason())?;
        } else {
            lmpg.ensure_record_family(&property)
                .map_err(|e| e.to_reason())?;
        }
        let mut updated = 0usize;
        // Untracked record vectors (the common backfill shape) bind in one
        // commit; source-hash-guarded and relationship items keep their
        // per-item guarded path.
        let batched: Vec<(&str, &[f32])> = items
            .iter()
            .filter(|item| owner != "relationship" && item.source_hash.is_none())
            .map(|item| (item.record_id.as_str(), item.vector.as_slice()))
            .collect();
        if !batched.is_empty() {
            updated += self
                .bind_field_vectors_external(id, &property, &batched)?
                .len();
        }
        for item in items {
            if owner != "relationship" && item.source_hash.is_none() {
                continue;
            }
            if owner == "relationship" {
                if let Some(expected_hash) = item.source_hash {
                    self.bind_relationship_vector_if_source_hash(
                        id,
                        &item.record_id,
                        &property,
                        expected_hash,
                        &item.vector,
                    )?;
                } else {
                    self.bind_relationship_vector_external(
                        id,
                        &item.record_id,
                        &property,
                        &item.vector,
                    )?;
                }
            } else if let Some(expected_hash) = item.source_hash {
                self.bind_field_vector_if_source_hash(
                    id,
                    &item.record_id,
                    &property,
                    expected_hash,
                    &item.vector,
                )?;
            }
            updated += 1;
        }
        Ok(serde_json::json!({ "updated": updated, "requested": items.len() }))
    }

    pub(crate) fn upsert_vector_index_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        id: &str,
        items: &[rushdb_protocol::VectorItem],
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.upsert_vector_index(id, items)?;
        Ok(result)
    }

    /// Records of `label` carrying `property` that are **not yet bound** to the
    /// index `index` (the canonical space alias). The control plane backfills
    /// these: fetch a batch, embed the text, then `upsertVectors`.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn vector_index_candidates(
        &self,
        query: &rushdb_protocol::VectorIndexCandidatesQuery,
    ) -> Result<(Vec<serde_json::Value>, u64), String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.vector_index_candidates_in_view(query, &snap)
    }

    /// Enumerate vector backfill candidates against a caller-owned pinned view.
    pub(crate) fn vector_index_candidates_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::VectorIndexCandidatesQuery,
    ) -> Result<(Vec<serde_json::Value>, u64), String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.vector_index_candidates_in_view(query, context.read_view())?;
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::too_many_lines,
        reason = "these casts narrow engine counters and sizes that are bounded by the process address space or clamped before conversion, so truncation cannot occur on supported 64-bit targets; the owner-kind branch is one linear candidate scan"
    )]
    pub(super) fn vector_index_candidates_in_view(
        &self,
        query: &rushdb_protocol::VectorIndexCandidatesQuery,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<(Vec<serde_json::Value>, u64), String> {
        let tenant = self.tenant();

        let (owner, generation) = match snap
            .get(self.vector_index_key(&query.index).as_bytes())
            .map_err(|e| e.to_string())?
        {
            None => return Err("vector index not found".to_string()),
            Some(raw) => {
                let row: serde_json::Value = serde_json::from_slice(&raw)
                    .map_err(|error| format!("corrupt vector index metadata: {error}"))?;
                let owner = row
                    .get("owner")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .ok_or_else(|| "corrupt vector index metadata: missing owner".to_string())?;
                (owner, row.get("generation").cloned().unwrap_or_default())
            }
        };
        let owner_kind = if owner == "relationship" {
            OwnerKind::Relationship
        } else {
            OwnerKind::Record
        };

        let norm = rushdb_types::value::normalize_name(&query.property);
        let sig = key(
            tenant,
            &[
                P_FAMILY_SIG,
                b"default",
                &[owner_kind.as_byte()],
                norm.as_bytes(),
            ],
        );
        let pid = match snap.get(&sig).map_err(|e| e.to_string())? {
            Some(raw) => u64::from_be_bytes(
                raw.as_slice()
                    .try_into()
                    .map_err(|_| "corrupt property family signature")?,
            ),
            None => return Ok((Vec::new(), 0)), // property family does not exist yet
        };

        let vectors = self.vector_engine()?;
        let space = vectors
            .lookup_space_in_view(snap, &query.index)
            .map_err(|e| e.to_string())?;

        let active_policy = rushdb_authz::load_active_policy_in_view(snap.as_ref(), tenant)
            .map_err(|error| error.to_string())?;
        let authz = rushdb_lmpg::AuthorizedView::new(snap.as_ref(), tenant, active_policy.as_ref());
        // Relationships have no property-mask concept; the ordinal set below
        // is already policy-filtered by endpoint label and relationship
        // type. Records share one label/property pair for this whole scan,
        // so a single up-front check covers every candidate.
        if owner_kind == OwnerKind::Record && !authz.property_allowed(&query.label, &query.property)
        {
            return Err("vector index candidates denied by policy".to_string());
        }

        let ords: Vec<u64> = if owner_kind == OwnerKind::Relationship {
            self.relationship_ordinals_matching(snap, &query.label, &serde_json::json!({}))?
                .into_iter()
                .collect()
        } else {
            access::label_members(snap, tenant, &query.label).map_err(|e| e.to_string())?
        };
        let mut out: Vec<serde_json::Value> = Vec::new();
        let mut encoded_bytes = 0usize;
        let mut unbound = 0u64;
        let skip = query.skip as usize;
        let limit = query.limit as usize;

        for ord in ords {
            let rec = if owner_kind == OwnerKind::Relationship {
                rushdb_lmpg::reconstruct::reconstruct_relationship(snap, tenant, ord)
                    .map_err(|e| e.to_string())?
            } else {
                rushdb_lmpg::reconstruct::reconstruct_record(snap, tenant, ord)
                    .map_err(|e| e.to_string())?
            };
            let Some(rec) = rec else {
                continue;
            };
            let obj = rec.as_object().cloned().unwrap_or_default();
            let Some(value) = obj
                .iter()
                .find(|(k, _)| rushdb_types::value::normalize_name(k) == norm)
                .map(|(_, v)| v.clone())
            else {
                continue; // property absent on this owner
            };
            if value.is_null() {
                continue;
            }
            // already bound to this index space?
            let bound = match space.as_ref() {
                Some(sp) => vectors
                    .binding_of_in_view(
                        snap,
                        sp.space_id,
                        rushdb_vector::SourceRef {
                            kind: owner_kind,
                            owner_ord: ord,
                            pid,
                            elem: 0,
                        },
                    )
                    .map_err(|e| e.to_string())?
                    .is_some(),
                None => false,
            };
            if bound {
                continue;
            }
            let position = unbound as usize;
            unbound += 1;
            if position >= skip && out.len() < limit {
                // What a guarded upsert sends back, so a completion computed
                // from this value is rejected if the value changes meanwhile.
                let source_hash = rushdb_vector::source_hash(
                    snap.as_ref(),
                    tenant,
                    rushdb_vector::SourceRef {
                        kind: owner_kind,
                        owner_ord: ord,
                        pid,
                        elem: 0,
                    },
                )
                .map_err(|e| e.to_string())?;
                append_bounded_json_response(
                    &mut out,
                    &mut encoded_bytes,
                    serde_json::json!({
                        "id": obj.get(rushdb_types::value::ID_FIELD).cloned().unwrap_or(serde_json::Value::Null),
                        "value": value,
                        "sourceHash": source_hash,
                        "indexGeneration": generation,
                    }),
                    self.max_response_bytes(),
                )?;
            }
        }
        Ok((out, unbound))
    }

    /// Exact vector search. The caller supplies the query vector; the database
    /// never invokes an embedding provider.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn search_vectors(
        &self,
        query: &rushdb_protocol::VectorQuery,
    ) -> Result<serde_json::Value, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.search_vectors_in_view(query, &snap)
    }

    /// Execute vector search against a caller-owned pinned view and work
    /// budget. Response-byte admission remains at the protocol boundary.
    pub(crate) fn search_vectors_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::VectorQuery,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.search_vectors_in_view(query, context.read_view())?;
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::too_many_lines,
        reason = "these casts narrow engine counters and sizes that are bounded by the process address space or clamped before conversion, so truncation cannot occur on supported 64-bit targets; the owner-kind branch is one linear candidate scan and output loop"
    )]
    pub(super) fn search_vectors_in_view(
        &self,
        query: &rushdb_protocol::VectorQuery,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<serde_json::Value, String> {
        let labels = query.labels.clone();
        let label = labels.first().cloned().unwrap_or_default();
        let property = query.property.clone();
        let similarity = query.similarity.clone();
        let skip = query.offset as usize;
        let limit = query.limit as usize;
        if query.vector.is_empty() {
            return Err("vector query requires a non-empty vector".to_string());
        }
        if let Some(active) = rushdb_authz::load_active_policy_in_view(snap, self.tenant())
            .map_err(|error| error.to_string())?
        {
            if active.mode == rushdb_authz::AuthorizationMode::PolicyEnforced
                && !(if labels.is_empty() {
                    active.policy.allows_every_label()
                } else {
                    labels.iter().all(|label| active.policy.allows_label(label))
                })
            {
                return Err("vector query denied by policy".into());
            }
        }
        let dims = query.vector.len();
        // An explicit index name is the single space identity; only fall back
        // to the platform's derived alias when the caller did not name the space.
        let alias = match query.index.as_deref() {
            Some(index) => index.to_string(),
            None if self.require_vector_index_name => {
                return Err("vector query requires an explicit `index` \
                     (resources.require-vector-index-name is on)"
                    .into());
            }
            None => rushdb_lmpg::import::external_space_alias(&label, &property, &similarity, dims),
        };
        let lmpg = self.lmpg();
        let Some(row_raw) = snap
            .get(self.vector_index_key(&alias).as_bytes())
            .map_err(|e| e.to_string())?
        else {
            return Err(format!(
                "embedding index not found for label `{label}` and property `{property}`"
            ));
        };
        let row: serde_json::Value = serde_json::from_slice(&row_raw)
            .map_err(|error| format!("corrupt vector index metadata: {error}"))?;
        let owner = row
            .get("owner")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or_else(|| "corrupt vector index metadata: missing owner".to_string())?;
        let owner_kind = if owner == "relationship" {
            OwnerKind::Relationship
        } else {
            OwnerKind::Record
        };
        // No `labels` means the index's own label, not "no label matches".
        let labels = if labels.is_empty() {
            row.get("label")
                .and_then(serde_json::Value::as_str)
                .map(|label| vec![label.to_string()])
                .unwrap_or_default()
        } else {
            labels
        };

        // Resolve the predicate first and use the matching owner ordinals as
        // the vector candidate set. Filtering after top-k ranking can silently
        // lose valid matches and leak owners outside `where`.
        // An empty predicate is not a filter: vector spaces are label-scoped,
        // so each gathered candidate only needs a membership point lookup.
        // Materializing every label member up front made each unfiltered
        // search O(label size) (C9, cloud-capacity F11: 62% of concurrent ANN
        // CPU).
        let unfiltered = query.predicate.is_null()
            || query
                .predicate
                .as_object()
                .is_some_and(serde_json::Map::is_empty);
        let allowed: Option<HashSet<u64>> = if owner_kind == OwnerKind::Relationship {
            Some(self.relationship_ordinals_matching(snap, &label, &query.predicate)?)
        } else if unfiltered {
            None
        } else {
            let mut filter_document = serde_json::json!({ "labels": labels });
            if !query.predicate.is_null() {
                filter_document["where"] = query.predicate.clone();
            }
            let parsed = rushdb_query::document::parse_document(&filter_document)
                .map_err(|e| e.to_string())?;
            // Ordinals directly from the cardinality planner (same access
            // path `find_records` uses) — no JSON record reconstruction and
            // no second by-id lookup just to recover the ordinal we already
            // had before the predicate was evaluated.
            let ords =
                rushdb_query::matching_ordinals(&lmpg, &parsed.search, &|| false, Arc::clone(snap))
                    .map_err(|e| e.to_string())?;
            Some(ords.into_iter().collect())
        };
        let tenant = self.tenant();
        let lookup_error = std::cell::RefCell::new(None::<String>);
        let candidate = |source: rushdb_vector::SourceRef| {
            if source.kind != owner_kind {
                return false;
            }
            match &allowed {
                Some(set) => set.contains(&source.owner_ord),
                None => labels.iter().any(|label| {
                    let membership =
                        rushdb_types::keys::label_member_key(tenant, label, source.owner_ord);
                    match snap.get(&membership) {
                        Ok(row) => row.is_some(),
                        Err(error) => {
                            lookup_error.borrow_mut().get_or_insert(error.to_string());
                            false
                        }
                    }
                }),
            }
        };
        let vectors = self.vector_engine()?;
        let space = vectors
            .lookup_space_in_view(snap, &alias)
            .map_err(|e| e.to_string())?
            .ok_or("index space missing")?;
        let (hits, report) = self.run_vector_search(
            &vectors,
            snap,
            &space,
            &super::vector_mode::VectorSearch {
                query: &query.vector,
                k: skip.saturating_add(limit),
                mode: query.mode,
                ef: query.ef,
                min_score: query.score_threshold,
            },
            Some(&candidate),
        )?;
        if let Some(error) = lookup_error.into_inner() {
            return Err(error);
        }
        let mut data = Vec::new();
        let mut encoded_bytes = 0usize;
        let proptypes = self.proptypes_cached(snap)?;
        let active = rushdb_authz::load_active_policy_in_view(snap.as_ref(), self.tenant())
            .map_err(|error| error.to_string())?;
        let authz = rushdb_lmpg::AuthorizedView::new(snap.as_ref(), self.tenant(), active.as_ref());
        for h in hits.into_iter().skip(skip).take(limit) {
            if h.source.kind != owner_kind {
                continue;
            }
            if owner_kind == OwnerKind::Relationship {
                if let Some(rel) = authz
                    .reconstruct_relationship(h.source.owner_ord)
                    .map_err(|e| e.to_string())?
                {
                    let mut obj = rel.as_object().cloned().unwrap_or_default();
                    obj.insert("score".to_string(), serde_json::json!(h.score));
                    append_bounded_json_response(
                        &mut data,
                        &mut encoded_bytes,
                        serde_json::Value::Object(obj),
                        self.max_response_bytes(),
                    )?;
                }
                continue;
            }
            if let Some(hdr) = access::record_by_ord(snap, self.tenant(), h.source.owner_ord)
                .map_err(|e| e.to_string())?
            {
                let Some(rec) = authz
                    .reconstruct_record(hdr.ordinal)
                    .map_err(|e| e.to_string())?
                else {
                    continue;
                };
                let rec = Self::canonical_record_with(&rec, &proptypes);
                let mut obj = rec.as_object().cloned().unwrap_or_default();
                obj.insert("score".to_string(), serde_json::json!(h.score));
                append_bounded_json_response(
                    &mut data,
                    &mut encoded_bytes,
                    serde_json::Value::Object(obj),
                    self.max_response_bytes(),
                )?;
            }
        }
        Ok(serde_json::json!({ "data": data, "vectorSearch": report }))
    }

    /// Relationship ordinals of `rel_type` whose properties satisfy
    /// `predicate` (empty predicate = all). Mirrors `scan_relationships`
    /// filtering for the relationship-owned vector candidate set.
    pub(super) fn relationship_ordinals_matching(
        &self,
        snap: &Arc<rushdb_storage::ReadView>,
        rel_type: &str,
        predicate: &serde_json::Value,
    ) -> Result<HashSet<u64>, String> {
        use rushdb_types::keys::{key, P_REL};
        let tenant = self.tenant();
        let predicate = match predicate {
            serde_json::Value::Null => None,
            serde_json::Value::Object(map) if map.is_empty() => None,
            where_ => {
                Some(rushdb_query::parse_property_predicate(where_).map_err(|e| e.to_string())?)
            }
        };
        let active_policy = rushdb_authz::load_active_policy_in_view(snap, tenant)
            .map_err(|error| error.to_string())?;
        let authz = rushdb_lmpg::AuthorizedView::new(snap.as_ref(), tenant, active_policy.as_ref());
        let mut out = HashSet::new();
        for (_, raw) in snap
            .range_prefix(&key(tenant, &[P_REL, b""]))
            .map_err(|e| e.to_string())?
        {
            let hdr: rushdb_types::occurrence::RelationshipHeader = serde_json::from_slice(&raw)
                .map_err(|e| format!("corrupt relationship header: {e}"))?;
            if hdr.rel_type != rel_type {
                continue;
            }
            if !authz.rel_type_allowed(&hdr.rel_type) {
                continue;
            }
            let source_label = access::record_by_ord(snap, tenant, hdr.source_ord)
                .map_err(|e| e.to_string())?
                .map(|record| record.label);
            let target_label = access::record_by_ord(snap, tenant, hdr.target_ord)
                .map_err(|e| e.to_string())?
                .map(|record| record.label);
            let label_allowed =
                |label: Option<String>| label.is_some_and(|label| authz.label_allowed(&label));
            if !label_allowed(source_label) || !label_allowed(target_label) {
                continue;
            }
            let rel_obj =
                rushdb_lmpg::reconstruct::reconstruct_relationship(snap, tenant, hdr.ordinal)
                    .map_err(|e| e.to_string())?
                    .and_then(|v| v.as_object().cloned())
                    .unwrap_or_default();
            let matches = match &predicate {
                Some(predicate) => rushdb_query::matches_properties(predicate, &rel_obj)
                    .map_err(|e| e.to_string())?,
                None => true,
            };
            if matches {
                out.insert(hdr.ordinal);
            }
        }
        Ok(out)
    }
}
