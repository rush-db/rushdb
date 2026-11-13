//! Node relationships operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use super::key_join::JoinSide;
use super::{access, bytes_from_hex, Arc, RecordId, RequestScope};

/// Commit `batch` as one atomic commit, unless it stages nothing.
fn run_nonempty(lmpg: &rushdb_lmpg::Lmpg, batch: rushdb_lmpg::BatchTx) -> Result<(), String> {
    if !batch.ops.is_empty() {
        lmpg.run_batch(batch).map_err(|e| e.to_reason())?;
    }
    Ok(())
}

/// Record ids of endpoint ordinals, read from their headers once per chunk.
#[derive(Default)]
struct RecordIds(std::collections::HashMap<u64, [u8; 16]>);

impl RecordIds {
    /// The record ref of `ord` at `snap`.
    fn record(
        &mut self,
        snap: &rushdb_storage::ReadView,
        tenant: rushdb_types::ids::TenantId,
        ord: u64,
    ) -> Result<rushdb_lmpg::RecordRef, String> {
        let record_id = if let Some(id) = self.0.get(&ord) {
            *id
        } else {
            let header = access::record_by_ord(snap, tenant, ord)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("record ordinal {ord} has no header"))?;
            self.0.insert(ord, header.record_id);
            header.record_id
        };
        Ok(rushdb_lmpg::RecordRef {
            record_id,
            ordinal: ord,
        })
    }

    fn clear(&mut self) {
        self.0.clear();
    }
}

/// Relationships an unlink deletes or a by-id update rewrites, collected
/// against one snapshot (each at most once) and written in one atomic commit.
#[derive(Default)]
pub(super) struct MatchedEdges {
    seen: std::collections::HashSet<u64>,
    rels: Vec<rushdb_lmpg::RelationshipRef>,
}

impl MatchedEdges {
    /// Every `rel_type` edge from `from` to `to`.
    pub(super) fn edges_between(
        &mut self,
        snap: &rushdb_storage::ReadView,
        tenant: rushdb_types::ids::TenantId,
        from: u64,
        to: u64,
        rel_type: Option<&str>,
    ) -> Result<(), String> {
        for (_, other, rel_ord) in
            access::adj_out(snap, tenant, from, rel_type).map_err(|e| e.to_string())?
        {
            if other != to || !self.seen.insert(rel_ord) {
                continue;
            }
            if let Some(rh) =
                access::relationship_by_ord(snap, tenant, rel_ord).map_err(|e| e.to_string())?
            {
                self.rels.push(rushdb_lmpg::RelationshipRef {
                    relationship_id: rh.relationship_id,
                    ordinal: rh.ordinal,
                });
            }
        }
        Ok(())
    }

    /// Delete them all in one commit; the count deleted.
    fn delete(self, lmpg: &rushdb_lmpg::Lmpg) -> Result<usize, String> {
        let deleted = self.rels.len();
        let mut batch = lmpg.begin_batch();
        batch.ops.extend(
            self.rels
                .into_iter()
                .map(|rel| rushdb_lmpg::BatchOp::DeleteRelationship { rel }),
        );
        run_nonempty(lmpg, batch)?;
        Ok(deleted)
    }

    /// Apply `fields` to them all in one commit; the count updated. With no
    /// fields nothing is written.
    pub(super) fn update(
        self,
        lmpg: &rushdb_lmpg::Lmpg,
        fields: &[(String, rushdb_lmpg::plan::FieldChange)],
    ) -> Result<usize, String> {
        let updated = self.rels.len();
        if fields.is_empty() {
            return Ok(updated);
        }
        let mut batch = lmpg.begin_batch();
        batch.ops.extend(
            self.rels
                .into_iter()
                .map(|rel| rushdb_lmpg::BatchOp::UpdateFields {
                    owner: rushdb_types::ids::OwnerKind::Relationship,
                    owner_ord: rel.ordinal,
                    owner_public_id: rel.relationship_id,
                    fields: fields.to_vec(),
                }),
        );
        run_nonempty(lmpg, batch)?;
        Ok(updated)
    }
}

impl RequestScope<'_> {
    #[cfg(test)]
    pub(super) fn relationship_exists(
        &self,
        src_ord: u64,
        rel_type: &str,
        tgt_ord: u64,
    ) -> Result<bool, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        access::edge_exists(&snap, self.tenant(), src_ord, rel_type, tgt_ord)
            .map_err(|e| e.to_string())
    }

    /// Execute a property join against the request's pinned view and budget.
    /// The transport path uses this entry point so a join cannot silently
    /// switch snapshots or bypass the shared execution deadline.
    pub(crate) fn query_join_pairs_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::JoinPairsQuery,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let snap = Arc::clone(context.read_view());
        let result = self.query_join_pairs_in_view(query, &snap, Some(context), Some(is_cancelled));
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        result
    }

    pub(super) fn query_join_pairs_in_view(
        &self,
        query: &rushdb_protocol::JoinPairsQuery,
        snap: &Arc<rushdb_storage::ReadView>,
        mut context: Option<&mut rushdb_engine::ExecutionContext>,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<u64, String> {
        let (left_property, right_property) = match &query.predicate {
            rushdb_protocol::JoinPredicate::PropertyOverlap {
                left_property,
                right_property,
            } => (left_property.as_str(), right_property.as_str()),
        };
        if query.left.label.trim().is_empty()
            || query.right.label.trim().is_empty()
            || left_property.trim().is_empty()
            || right_property.trim().is_empty()
        {
            return Err("join labels and properties must be non-empty".to_string());
        }
        if !query.left.filter.is_object() || !query.right.filter.is_object() {
            return Err("join filters must be objects".to_string());
        }
        if !(1..=10_000).contains(&query.limit) {
            return Err("join limit must be between 1 and 10000".to_string());
        }
        let never = || false;
        let is_cancelled = is_cancelled.unwrap_or(&never);
        let deadline = context
            .as_deref()
            .and_then(rushdb_engine::ExecutionContext::deadline);
        let mut meter = self.working_set(is_cancelled, deadline);
        let limit = usize::try_from(query.limit).unwrap_or(usize::MAX);
        let mut pairs = std::collections::HashSet::<(u64, u64)>::new();
        self.for_each_join_chunk(
            snap,
            &JoinSide {
                label: &query.left.label,
                predicate: &query.left.filter,
                key: Some(left_property),
            },
            &JoinSide {
                label: &query.right.label,
                predicate: &query.right.filter,
                key: Some(right_property),
            },
            query.exclude_same_record,
            &mut meter,
            &mut |chunk| {
                for &pair in chunk {
                    if pairs.len() >= limit {
                        return Ok(false);
                    }
                    if pairs.insert(pair) {
                        if let Some(context) = context.as_deref_mut() {
                            context
                                .charge_bytes(std::mem::size_of::<(u64, u64)>())
                                .map_err(|error| error.to_string())?;
                        }
                    }
                }
                Ok(pairs.len() < limit)
            },
        )?;
        Ok(pairs.len() as u64)
    }

    /// Match source and target record sets, then create typed relationships
    /// with relationship-owned properties.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    ///
    /// # Panics
    ///
    /// Panics if an internal invariant is violated, for example a poisoned lock.
    pub fn link_records(
        &self,
        command: &rushdb_protocol::RelationshipMutation,
    ) -> Result<serde_json::Value, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.link_records_in_view(command, &snap, None, None)
    }

    pub(super) fn link_records_in_view(
        &self,
        command: &rushdb_protocol::RelationshipMutation,
        snap: &Arc<rushdb_storage::ReadView>,
        is_cancelled: Option<&dyn Fn() -> bool>,
        deadline: Option<std::time::Instant>,
    ) -> Result<serde_json::Value, String> {
        let rel_type = command.relation_type.clone();
        let direction = command.direction;
        let many_to_many = command.allow_cartesian;
        let props: Vec<(String, rushdb_lmpg::plan::FieldChange)> = command
            .properties
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            if v.is_null() {
                                rushdb_lmpg::plan::FieldChange::ExplicitNull
                            } else {
                                rushdb_lmpg::plan::FieldChange::Set(v.clone())
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        let source_label = command.source.label.clone();
        let target_label = command.target.label.clone();
        let source_key = command.source.property.clone();
        let target_key = command.target.property.clone();

        let has_join_keys = source_key.is_some() && target_key.is_some();
        if !has_join_keys && !many_to_many {
            return Err(
                "source.property and target.property are required unless allowCartesian is true"
                    .to_string(),
            );
        }
        let source_has_where = command
            .source
            .predicate
            .as_object()
            .is_some_and(|value| !value.is_empty());
        let target_has_where = command
            .target
            .predicate
            .as_object()
            .is_some_and(|value| !value.is_empty());
        if many_to_many && !has_join_keys && (!source_has_where || !target_has_where) {
            return Err(
                "allowCartesian without join properties requires a non-empty `predicate` for both source and target"
                    .to_string(),
            );
        }

        // Each chunk of matched pairs commits on its own (at most
        // `JOIN_CHUNK` new edges per commit), so a join of any size runs in
        // bounded memory. A pair whose edge exists, in the view or from an
        // earlier chunk, is skipped: a retry after a failure creates only
        // the edges still missing. Every pair comes once per call, so the
        // pinned view is enough to tell existing edges from new ones.
        let lmpg = self.lmpg();
        let never = || false;
        let is_cancelled = is_cancelled.unwrap_or(&never);
        let mut meter = self.working_set(is_cancelled, deadline);
        let mut created = 0usize;
        let mut skipped_existing = 0usize;
        let mut ids = RecordIds::default();
        self.for_each_join_chunk(
            snap,
            &JoinSide {
                label: &source_label,
                predicate: &command.source.predicate,
                key: source_key.as_deref(),
            },
            &JoinSide {
                label: &target_label,
                predicate: &command.target.predicate,
                key: target_key.as_deref(),
            },
            !has_join_keys,
            &mut meter,
            &mut |pairs| {
                let mut batch = lmpg.begin_batch();
                for &(sord, tord) in pairs {
                    // direction: out -> source->target ; in -> target->source
                    let (a_ord, b_ord) = directed(direction, sord, tord);
                    if access::edge_exists(snap, self.tenant(), a_ord, &rel_type, b_ord)
                        .map_err(|e| e.to_string())?
                    {
                        skipped_existing += 1;
                        continue;
                    }
                    batch.ops.push(rushdb_lmpg::BatchOp::CreateRelationship {
                        rel_type: rel_type.clone(),
                        source: ids.record(snap, self.tenant(), a_ord)?,
                        target: ids.record(snap, self.tenant(), b_ord)?,
                        fields: props.clone(),
                    });
                    created += 1;
                }
                run_nonempty(&lmpg, batch)?;
                ids.clear();
                Ok(true)
            },
        )?;

        Ok(serde_json::json!({
            "message": "Relations have been successfully created",
            "created": created,
            "skipped_existing": skipped_existing,
        }))
    }

    /// Query-based relationship linking: matched against the caller-owned
    /// execution view, stopped by `is_cancelled` and the context's deadline
    /// between chunks.
    pub(crate) fn link_records_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        command: &rushdb_protocol::RelationshipMutation,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        self.link_records_in_view(
            command,
            context.read_view(),
            Some(is_cancelled),
            context.deadline(),
        )
    }

    /// Match record pairs, then delete typed relationships between them.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn unlink_records(
        &self,
        command: &rushdb_protocol::RelationshipMutation,
    ) -> Result<serde_json::Value, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.unlink_records_in_view(command, &snap, None, None)
    }

    pub(super) fn unlink_records_in_view(
        &self,
        command: &rushdb_protocol::RelationshipMutation,
        snap: &Arc<rushdb_storage::ReadView>,
        is_cancelled: Option<&dyn Fn() -> bool>,
        deadline: Option<std::time::Instant>,
    ) -> Result<serde_json::Value, String> {
        let rel_type = command.relation_type.clone();
        let source_label = command.source.label.clone();
        let target_label = command.target.label.clone();
        let source_key = command.source.property.clone();
        let target_key = command.target.property.clone();
        let many_to_many = command.allow_cartesian;

        let has_join_keys = source_key.is_some() && target_key.is_some();
        if !has_join_keys && !many_to_many {
            return Err(
                "source.property and target.property are required unless allowCartesian is true"
                    .to_string(),
            );
        }
        let source_has_where = command
            .source
            .predicate
            .as_object()
            .is_some_and(|value| !value.is_empty());
        let target_has_where = command
            .target
            .predicate
            .as_object()
            .is_some_and(|value| !value.is_empty());
        if many_to_many && !has_join_keys && (!source_has_where || !target_has_where) {
            return Err(
                "allowCartesian without join properties requires a non-empty `predicate` for both source and target"
                    .to_string(),
            );
        }

        // Like link: each chunk of matched pairs deletes its edges in one
        // commit, and a retry finds only the edges still there.
        let lmpg = self.lmpg();
        let never = || false;
        let is_cancelled = is_cancelled.unwrap_or(&never);
        let mut meter = self.working_set(is_cancelled, deadline);
        let mut deleted = 0usize;
        self.for_each_join_chunk(
            snap,
            &JoinSide {
                label: &source_label,
                predicate: &command.source.predicate,
                key: source_key.as_deref(),
            },
            &JoinSide {
                label: &target_label,
                predicate: &command.target.predicate,
                key: target_key.as_deref(),
            },
            !has_join_keys,
            &mut meter,
            &mut |pairs| {
                let mut doomed = MatchedEdges::default();
                for &(sord, tord) in pairs {
                    // `direction` names which endpoint the edge leaves, as in link.
                    let (from, to) = directed(command.direction, sord, tord);
                    doomed.edges_between(snap, self.tenant(), from, to, Some(&rel_type))?;
                }
                deleted += doomed.delete(&lmpg)?;
                Ok(true)
            },
        )?;
        Ok(
            serde_json::json!({ "message": "Relations have been successfully deleted", "deleted": deleted }),
        )
    }

    /// Query-based relationship deletion using the caller-owned execution
    /// view for matching and relationship reads.
    pub(crate) fn unlink_records_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        command: &rushdb_protocol::RelationshipMutation,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        self.unlink_records_in_view(
            command,
            context.read_view(),
            Some(is_cancelled),
            context.deadline(),
        )
    }

    /// Link records by explicit record IDs (attach semantics). Endpoints are
    /// named directly (dashed UUID or compact hex `_id`); a missing endpoint is
    /// an error. Idempotent per pair.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn link_records_by_id(
        &self,
        command: &rushdb_protocol::RelationshipIdsMutation,
    ) -> Result<serde_json::Value, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        self.link_records_by_id_in_view(command, &snap)
    }

    pub(super) fn link_records_by_id_in_view(
        &self,
        command: &rushdb_protocol::RelationshipIdsMutation,
        snap: &rushdb_storage::ReadView,
    ) -> Result<serde_json::Value, String> {
        let rel_type = command.relation_type.clone();
        let direction = command.direction;
        let props = super::relationship_updates::typed_property_changes(
            "linkRecordsById",
            "properties",
            command
                .properties
                .as_object()
                .unwrap_or(&serde_json::Map::new()),
            &command.types,
        )?;

        let mut endpoints: Vec<(rushdb_lmpg::RecordRef, u64)> = Vec::new();
        for id_hex in command.source_ids.iter().chain(command.target_ids.iter()) {
            let id = RecordId(bytes_from_hex(id_hex)?);
            let hdr = rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("record not found: {id_hex}"))?;
            endpoints.push((
                rushdb_lmpg::RecordRef {
                    record_id: hdr.record_id,
                    ordinal: hdr.ordinal,
                },
                hdr.ordinal,
            ));
        }
        let sources = endpoints[..command.source_ids.len()].to_vec();
        let targets = endpoints[command.source_ids.len()..].to_vec();

        // E1/E5: stage every new edge into ONE batch commit instead of one
        // commit per edge. Endpoint resolution above already failed fast on
        // dangling ids (nothing staged yet); the single commit below makes
        // the write half all-or-nothing — a concurrent endpoint delete or
        // precondition conflict fails the whole batch with nothing applied.
        let lmpg = self.lmpg();
        let mut batch = lmpg.begin_batch();
        let mut staged_pairs = std::collections::HashSet::new();
        let mut created = 0usize;
        let mut skipped_existing = 0usize;
        for (sref, sord) in &sources {
            for (tref, tord) in &targets {
                let (a_ord, a_ref, b_ord, b_ref) = if direction == rushdb_protocol::Direction::In {
                    (*tord, *tref, *sord, *sref)
                } else {
                    (*sord, *sref, *tord, *tref)
                };
                // Existence against the same pre-write snapshot (no extra
                // read views), plus in-batch dedup so a repeated pair in one
                // call cannot create a duplicate edge.
                let edge_exists =
                    access::edge_exists(&snap, self.tenant(), a_ord, &rel_type, b_ord)
                        .map_err(|e| e.to_string())?;
                if edge_exists || !staged_pairs.insert((a_ord, b_ord)) {
                    skipped_existing += 1;
                    continue;
                }
                batch
                    .ops
                    .push(rushdb_lmpg::BatchOp::CreateRelationshipById {
                        rel_type: rel_type.clone(),
                        source_id: a_ref.record_id,
                        target_id: b_ref.record_id,
                        fields: props.clone(),
                    });
                created += 1;
            }
        }
        if created > 0 {
            lmpg.run_batch(batch).map_err(|e| e.to_reason())?;
        }
        Ok(serde_json::json!({
            "message": "Relations created",
            "created": created,
            "skipped_existing": skipped_existing,
        }))
    }

    /// Link explicit record IDs using the caller-owned execution view.
    pub(crate) fn link_records_by_id_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        command: &rushdb_protocol::RelationshipIdsMutation,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.link_records_by_id_in_view(command, context.read_view())?;
        Ok(result)
    }

    /// Pairwise batch link: one edge per explicit (source, target) pair —
    /// no cross-product expansion. Same contract as `link_records_by_id`:
    /// fail-fast endpoint resolution on one snapshot, then a single atomic
    /// batch commit; existing edges (same type) are skipped and reported.
    /// A per-pair `relation_type` overrides the batch default, so one call
    /// can carry heterogeneous edge types.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn link_records_pairs(
        &self,
        command: &rushdb_protocol::RelationshipPairsMutation,
    ) -> Result<serde_json::Value, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        self.link_records_pairs_in_view(command, &snap)
    }

    pub(super) fn link_records_pairs_in_view(
        &self,
        command: &rushdb_protocol::RelationshipPairsMutation,
        snap: &rushdb_storage::ReadView,
    ) -> Result<serde_json::Value, String> {
        let direction = command.direction;
        let props = super::relationship_updates::typed_property_changes(
            "linkRecordsPairs",
            "properties",
            command
                .properties
                .as_object()
                .unwrap_or(&serde_json::Map::new()),
            &command.types,
        )?;

        // Resolve every endpoint before staging anything: a dangling id
        // fails the whole call with zero partials.
        let mut resolved: Vec<(rushdb_lmpg::RecordRef, rushdb_lmpg::RecordRef, String)> =
            Vec::with_capacity(command.pairs.len());
        for pair in &command.pairs {
            let sid = RecordId(bytes_from_hex(&pair.source_id)?);
            let tid = RecordId(bytes_from_hex(&pair.target_id)?);
            let shdr = rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &sid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("record not found: {}", pair.source_id))?;
            let thdr = rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &tid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("record not found: {}", pair.target_id))?;
            let rel_type = pair
                .relation_type
                .clone()
                .unwrap_or_else(|| command.relation_type.clone());
            if rel_type.trim().is_empty() {
                return Err("pair relation_type must be non-empty".to_string());
            }
            let (a_ref, b_ref) = if direction == rushdb_protocol::Direction::In {
                (
                    rushdb_lmpg::RecordRef {
                        record_id: thdr.record_id,
                        ordinal: thdr.ordinal,
                    },
                    rushdb_lmpg::RecordRef {
                        record_id: shdr.record_id,
                        ordinal: shdr.ordinal,
                    },
                )
            } else {
                (
                    rushdb_lmpg::RecordRef {
                        record_id: shdr.record_id,
                        ordinal: shdr.ordinal,
                    },
                    rushdb_lmpg::RecordRef {
                        record_id: thdr.record_id,
                        ordinal: thdr.ordinal,
                    },
                )
            };
            resolved.push((a_ref, b_ref, rel_type));
        }

        let lmpg = self.lmpg();
        let mut batch = lmpg.begin_batch();
        // (source ord, type, target ord): same pair+type twice in one call
        // stages once; same pair under *different* types stages twice
        // (distinct edges, matching the catalog model).
        let mut staged = std::collections::HashSet::new();
        let mut created = 0usize;
        let mut skipped_existing = 0usize;
        for (a_ref, b_ref, rel_type) in &resolved {
            let edge_exists =
                access::edge_exists(&snap, self.tenant(), a_ref.ordinal, rel_type, b_ref.ordinal)
                    .map_err(|e| e.to_string())?;
            if edge_exists || !staged.insert((a_ref.ordinal, rel_type.clone(), b_ref.ordinal)) {
                skipped_existing += 1;
                continue;
            }
            batch
                .ops
                .push(rushdb_lmpg::BatchOp::CreateRelationshipById {
                    rel_type: rel_type.clone(),
                    source_id: a_ref.record_id,
                    target_id: b_ref.record_id,
                    fields: props.clone(),
                });
            created += 1;
        }
        if created > 0 {
            lmpg.run_batch(batch).map_err(|e| e.to_reason())?;
        }
        Ok(serde_json::json!({
            "message": "Relations created",
            "created": created,
            "skipped_existing": skipped_existing,
        }))
    }

    /// Link explicit record pairs using the caller-owned execution view.
    pub(crate) fn link_records_pairs_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        command: &rushdb_protocol::RelationshipPairsMutation,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.link_records_pairs_in_view(command, context.read_view())?;
        Ok(result)
    }

    pub(super) fn unlink_records_by_id_in_view(
        &self,
        command: &rushdb_protocol::UnlinkRecordsMutation,
        snap: &rushdb_storage::ReadView,
    ) -> Result<serde_json::Value, String> {
        let rel_types: Vec<Option<&str>> = if command.relation_types.is_empty() {
            vec![None]
        } else {
            command
                .relation_types
                .iter()
                .map(|t| Some(t.as_str()))
                .collect()
        };
        let mut endpoints: Vec<(RecordId, u64)> = Vec::new();
        for id_hex in command.source_ids.iter().chain(command.target_ids.iter()) {
            let id = RecordId(bytes_from_hex(id_hex)?);
            let hdr = rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("record not found: {id_hex}"))?;
            endpoints.push((RecordId(hdr.record_id), hdr.ordinal));
        }
        let sources = endpoints[..command.source_ids.len()].to_vec();
        let targets = endpoints[command.source_ids.len()..].to_vec();

        let mut doomed = MatchedEdges::default();
        for (_sid, sord) in &sources {
            for (_tid, tord) in &targets {
                let pairs = match command.direction {
                    Some(direction) => vec![directed(direction, *sord, *tord)],
                    None => vec![(*sord, *tord), (*tord, *sord)],
                };
                for (from, to) in pairs {
                    for rel_type in &rel_types {
                        doomed.edges_between(snap, self.tenant(), from, to, *rel_type)?;
                    }
                }
            }
        }
        let deleted = doomed.delete(&self.lmpg())?;
        Ok(serde_json::json!({ "message": "Relations deleted", "deleted": deleted }))
    }

    /// Unlink explicit record IDs using the caller-owned execution view.
    pub(crate) fn unlink_records_by_id_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        command: &rushdb_protocol::UnlinkRecordsMutation,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.unlink_records_by_id_in_view(command, context.read_view())?;
        Ok(result)
    }

    /// Rename the relationship type between two labels. Every edge whose
    /// endpoints are a `source_label` record and a `target_label` record and
    /// whose type is `source_type` is re-created as `target_type` (endpoints
    /// preserved, direction honored), then removed. Returns the number of
    /// relationships retyped. Mirrors the Neo4j `retype_existing_relationship`
    /// used by relationship-pattern approval.
    pub(super) fn retype_relations_in_view(
        &self,
        command: &rushdb_protocol::RetypeRelationsMutation,
        snap: &rushdb_storage::ReadView,
    ) -> Result<serde_json::Value, String> {
        use rushdb_types::keys::{key, P_REL};
        let tenant = self.tenant();
        let lmpg = self.lmpg();

        // collect (source_ord, target_ord, rel_ref) for every matching edge
        let mut edges: Vec<(u64, u64, rushdb_lmpg::RelationshipRef)> = Vec::new();
        for (_, raw) in snap
            .range_prefix(&key(tenant, &[P_REL, b""]))
            .map_err(|e| e.to_string())?
        {
            let hdr: rushdb_types::occurrence::RelationshipHeader = serde_json::from_slice(&raw)
                .map_err(|e| format!("corrupt relationship header: {e}"))?;
            if hdr.rel_type != command.source_type {
                continue;
            }
            let src_hdr = access::record_by_ord(&snap, tenant, hdr.source_ord)
                .map_err(|e| e.to_string())?
                .ok_or("corrupt relationship: missing source")?;
            let tgt_hdr = access::record_by_ord(&snap, tenant, hdr.target_ord)
                .map_err(|e| e.to_string())?
                .ok_or("corrupt relationship: missing target")?;
            if src_hdr.label != command.source_label || tgt_hdr.label != command.target_label {
                continue;
            }
            edges.push((
                hdr.source_ord,
                hdr.target_ord,
                rushdb_lmpg::RelationshipRef {
                    relationship_id: hdr.relationship_id,
                    ordinal: hdr.ordinal,
                },
            ));
        }

        // group matching edges by endpoint pair; retype one edge per pair
        // (MERGE semantics), then remove every old edge of that pair
        let mut by_pair: std::collections::BTreeMap<(u64, u64), Vec<rushdb_lmpg::RelationshipRef>> =
            std::collections::BTreeMap::new();
        for (source_ord, target_ord, rel_ref) in edges {
            let pair = if command.direction == rushdb_protocol::Direction::In {
                (target_ord, source_ord)
            } else {
                (source_ord, target_ord)
            };
            by_pair.entry(pair).or_default().push(rel_ref);
        }

        // One atomic commit for the whole retype: a conflict or an
        // oversized commit leaves every edge as it was. Each endpoint pair
        // ends with one `target_type` edge (MERGE semantics) carrying the
        // properties of the edges merged into it (REL-1), merged by
        // `mergeStrategy`; an existing `target_type` edge keeps its identity.
        let mut batch = lmpg.begin_batch();
        let mut retyped = 0usize;
        let mut merged = 0usize;
        let mut conflicts = RetypeConflicts::default();
        for (pair, rels) in by_pair {
            retyped += rels.len();
            merged += retype_pair(
                snap,
                tenant,
                command,
                pair,
                rels,
                &mut batch.ops,
                &mut conflicts,
            )?;
        }
        if command.merge_strategy == rushdb_protocol::RetypeMergeStrategy::Fail
            && conflicts.count > 0
        {
            return Err(conflicts.refusal(snap, tenant)?);
        }
        run_nonempty(&lmpg, batch)?;

        Ok(serde_json::json!({
            "message": "Relations have been successfully retyped",
            "retyped": retyped,
            "merged": merged,
            "propertyConflicts": conflicts.count,
        }))
    }

    /// Retype relationships using the caller-owned execution view.
    pub(crate) fn retype_relations_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        command: &rushdb_protocol::RetypeRelationsMutation,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.retype_relations_in_view(command, context.read_view())?;
        Ok(result)
    }
}

/// The (from, to) ordinals of an edge between a selected source and target:
/// `In` means the edge points from the target to the source.
pub(super) fn directed(
    direction: rushdb_protocol::Direction,
    source: u64,
    target: u64,
) -> (u64, u64) {
    if direction == rushdb_protocol::Direction::In {
        (target, source)
    } else {
        (source, target)
    }
}

type RelField = (String, serde_json::Value, rushdb_lmpg::plan::FieldChange);

/// How many property conflicts a retype reports in detail (`fail`).
const RETYPE_CONFLICT_DETAILS: usize = 20;

/// One property two merged relationships held with different values.
struct RetypeConflict {
    pair: (u64, u64),
    property: String,
    kept: serde_json::Value,
    other: serde_json::Value,
}

/// Property conflicts of a retype: their number, and the first few.
#[derive(Default)]
struct RetypeConflicts {
    count: usize,
    first: Vec<RetypeConflict>,
}

impl RetypeConflicts {
    fn push(&mut self, conflict: RetypeConflict) {
        self.count += 1;
        if self.first.len() < RETYPE_CONFLICT_DETAILS {
            self.first.push(conflict);
        }
    }

    /// The `fail` refusal: the count and the first conflicts, with record ids.
    fn refusal(
        &self,
        snap: &rushdb_storage::ReadView,
        tenant: rushdb_types::ids::TenantId,
    ) -> Result<String, String> {
        let mut details = Vec::with_capacity(self.first.len());
        for conflict in &self.first {
            let id = |ord| -> Result<String, String> {
                Ok(access::record_by_ord(snap, tenant, ord)
                    .map_err(|e| e.to_string())?
                    .map_or_else(
                        || ord.to_string(),
                        |header| uuid::Uuid::from_bytes(header.record_id).to_string(),
                    ))
            };
            details.push(format!(
                "`{}` between {} and {} ({} vs {})",
                conflict.property,
                id(conflict.pair.0)?,
                id(conflict.pair.1)?,
                conflict.kept,
                conflict.other
            ));
        }
        Ok(format!(
            "retypeRelations refused by mergeStrategy `fail`: {} property conflict(s), nothing \
             was written; first {}: {}",
            self.count,
            details.len(),
            details.join("; ")
        ))
    }
}

/// One endpoint pair of a retype: leave a single `target_type` edge carrying
/// the properties of every edge merged into it, merged by `strategy`, and
/// delete `rels`. The kept edge is the pair's existing `target_type` edge,
/// else a new one seeded with the oldest retyped edge's properties. Returns
/// how many of `rels` were folded into another edge; conflicts (a property
/// two merged edges held with different values) go to `conflicts`.
fn retype_pair(
    snap: &rushdb_storage::ReadView,
    tenant: rushdb_types::ids::TenantId,
    command: &rushdb_protocol::RetypeRelationsMutation,
    (a_ord, b_ord): (u64, u64),
    mut rels: Vec<rushdb_lmpg::RelationshipRef>,
    ops: &mut Vec<rushdb_lmpg::BatchOp>,
    conflicts: &mut RetypeConflicts,
) -> Result<usize, String> {
    use rushdb_lmpg::plan::FieldChange;
    use rushdb_protocol::RetypeMergeStrategy;
    let (target_type, strategy) = (command.target_type.as_str(), command.merge_strategy);
    rels.sort_by_key(|rel| rel.ordinal);
    let existing = access::adj_out(snap, tenant, a_ord, Some(target_type))
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|(_, other, _)| *other == b_ord)
        .map(|(_, _, rel_ord)| rel_ord);
    let original = match existing {
        Some(rel_ord) => relationship_fields(snap, tenant, rel_ord)?,
        None => Vec::new(),
    };
    // Incoming edges in age order; without an existing edge the oldest
    // seeds the new one.
    let mut incoming = Vec::with_capacity(rels.len());
    for rel in &rels {
        incoming.push(relationship_fields(snap, tenant, rel.ordinal)?);
    }
    let retyped_names: std::collections::BTreeSet<String> = incoming
        .iter()
        .flatten()
        .map(|(name, _, _)| name.clone())
        .collect();
    let mut incoming = incoming.into_iter();
    let mut fields: Vec<RelField> = match existing {
        Some(_) => original.clone(),
        None => incoming.next().unwrap_or_default(),
    };
    let overwrite = matches!(
        strategy,
        RetypeMergeStrategy::Append | RetypeMergeStrategy::Rewrite
    );
    for edge in incoming {
        merge_fields(&mut fields, edge, overwrite, (a_ord, b_ord), conflicts);
    }
    let Some(rel_ord) = existing else {
        let src_hdr = access::record_by_ord(snap, tenant, a_ord)
            .map_err(|e| e.to_string())?
            .ok_or("corrupt relationship: missing source")?;
        let tgt_hdr = access::record_by_ord(snap, tenant, b_ord)
            .map_err(|e| e.to_string())?
            .ok_or("corrupt relationship: missing target")?;
        ops.push(rushdb_lmpg::BatchOp::CreateRelationship {
            rel_type: target_type.to_string(),
            source: rushdb_lmpg::RecordRef {
                record_id: src_hdr.record_id,
                ordinal: src_hdr.ordinal,
            },
            target: rushdb_lmpg::RecordRef {
                record_id: tgt_hdr.record_id,
                ordinal: tgt_hdr.ordinal,
            },
            fields: fields
                .into_iter()
                .map(|(name, _, change)| (name, change))
                .collect(),
        });
        let merged = rels.len() - 1;
        ops.extend(
            rels.into_iter()
                .map(|rel| rushdb_lmpg::BatchOp::DeleteRelationship { rel }),
        );
        return Ok(merged);
    };
    // `rewrite`: the existing edge keeps only what the retyped edges hold.
    if strategy == RetypeMergeStrategy::Rewrite {
        fields.retain(|(name, _, _)| retyped_names.contains(name));
    }
    let mut writes: Vec<(String, FieldChange)> = Vec::new();
    for (name, value, change) in fields.iter().cloned() {
        if !original
            .iter()
            .any(|(held, held_value, _)| *held == name && *held_value == value)
        {
            writes.push((name, change));
        }
    }
    for (name, _, _) in &original {
        if !fields.iter().any(|(kept, _, _)| kept == name) {
            writes.push((name.clone(), FieldChange::Delete));
        }
    }
    if !writes.is_empty() {
        let header = access::relationship_by_ord(snap, tenant, rel_ord)
            .map_err(|e| e.to_string())?
            .ok_or("corrupt relationship: missing header")?;
        ops.push(rushdb_lmpg::BatchOp::UpdateFields {
            owner: rushdb_types::ids::OwnerKind::Relationship,
            owner_ord: rel_ord,
            owner_public_id: header.relationship_id,
            fields: writes,
        });
    }
    let merged = rels.len();
    ops.extend(
        rels.into_iter()
            .map(|rel| rushdb_lmpg::BatchOp::DeleteRelationship { rel }),
    );
    Ok(merged)
}

/// Fold one merged edge's properties into `fields`: a property `fields`
/// lacks is added; one held with a different value is a conflict, and
/// `overwrite` lets the merged edge's value win it.
fn merge_fields(
    fields: &mut Vec<RelField>,
    edge: Vec<RelField>,
    overwrite: bool,
    pair: (u64, u64),
    conflicts: &mut RetypeConflicts,
) {
    for (name, value, change) in edge {
        match fields.iter_mut().find(|(held, _, _)| *held == name) {
            Some(held) if held.1 != value => {
                conflicts.push(RetypeConflict {
                    pair,
                    property: name.clone(),
                    kept: held.1.clone(),
                    other: value.clone(),
                });
                if overwrite {
                    *held = (name, value, change);
                }
            }
            Some(_) => {}
            None => fields.push((name, value, change)),
        }
    }
}

/// A relationship's properties as `(name, value, write)`, the write
/// re-creating the value on another relationship: a scalar datetime stays a
/// datetime (its JSON form is a number), and a null stays an explicit null.
fn relationship_fields(
    snap: &rushdb_storage::ReadView,
    tenant: rushdb_types::ids::TenantId,
    ordinal: u64,
) -> Result<Vec<RelField>, String> {
    use rushdb_lmpg::plan::FieldChange;
    let Some(serde_json::Value::Object(document)) =
        rushdb_lmpg::reconstruct::reconstruct_relationship(snap, tenant, ordinal)
            .map_err(|e| e.to_string())?
    else {
        return Ok(Vec::new());
    };
    let datetimes: std::collections::BTreeSet<String> =
        super::relationship_reads::PropertyNames::default()
            .of(snap, tenant, ordinal)?
            .into_iter()
            .filter(|(_, (ty, array))| *ty == rushdb_types::model::LogicalType::DateTime && !array)
            .map(|(name, _)| name)
            .collect();
    Ok(document
        .into_iter()
        .filter(|(name, _)| !rushdb_types::value::is_reserved_name(name))
        .map(|(name, value)| {
            let change = match &value {
                serde_json::Value::Null => FieldChange::ExplicitNull,
                serde_json::Value::Number(n) if datetimes.contains(&name) => n
                    .as_i64()
                    .map_or_else(|| FieldChange::Set(value.clone()), FieldChange::SetDatetime),
                _ => FieldChange::Set(value.clone()),
            };
            (name, value, change)
        })
        .collect())
}
