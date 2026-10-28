use std::collections::{BTreeMap, BTreeSet};

use rushdb_storage::{Deriver as _, StorageError, Txn};
use rushdb_types::command::{CommitBatch, Mutation};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{
    ann_manifest_meta_prefix, emb_alias_key, emb_bindings_prefix, emb_reclaim_key,
    emb_reclaim_prefix, emb_reclaim_value, emb_vector_key, key, parse_emb_reclaim_value,
    security_generation_key, security_policy_key, security_prefix, u64be, vector_index_meta_prefix,
    P_EMB_VECTOR, P_META,
};
use rushdb_types::occurrence::{
    decode_occ_value, encode_occ_value, OccurrenceRow, PropertyDefinition,
};

use crate::reader::keys_ext as kx;

const ADJ_OUT: &[u8] = b"out";
const ADJ_IN: &[u8] = b"in";

/// Maximum embedding-slot reclamations erased in one commit; the driver loops
/// commits so the batch stays bounded.
const EMB_SLOT_RECLAIM_BATCH: usize = 512;

/// Key of the named per-tenant allocation counter in the meta keyspace.
#[must_use]
pub fn meta_counter_key(tenant: TenantId, name: &str) -> Vec<u8> {
    key(tenant, &[P_META, b"alloc", name.as_bytes()])
}

/// Bounded set of the smallest remaining purgeable keys for one purge chunk:
/// tenant keys excluding the security subrange (data purge must never erase
/// enforcement) and the usage-bytes row, then tenant-owned vector/ANN metadata stored outside the
/// binary tenant keyspace. Shared by the chunk expansion and the chunk driver
/// so both agree on when a purge is complete.
pub(crate) fn purge_batch_keys(
    txn: &Txn,
    t: TenantId,
    max_keys: usize,
) -> Result<Vec<Vec<u8>>, StorageError> {
    let mut out = txn.keys_prefix_limited(&key(t, &[]), max_keys)?;
    // The usage-bytes row stays too: it is the tenant's running total, and the
    // purged rows are subtracted from it as they go.
    out.retain(|k| !k.starts_with(&security_prefix(t)) && !rushdb_storage::is_usage_bytes_key(k));
    // Only reach for external metadata once tenant data is exhausted, so the
    // driver cannot park on metadata while tenant data remains.
    if out.len() < max_keys {
        for external in [vector_index_meta_prefix(t), ann_manifest_meta_prefix(t)] {
            if out.len() >= max_keys {
                break;
            }
            let take = max_keys - out.len();
            out.extend(txn.keys_prefix_limited(external.as_bytes(), take)?);
        }
    }
    Ok(out)
}

/// Deterministic derivation of every canonical and derived key implied by a
/// committed batch. Runs on the forward publish path and again during crash
/// recovery replay; pure with respect to (state, batch).
///
/// # Errors
/// Returns [`StorageError`] if the batch targets an unsupported command
/// version, a mutation is internally inconsistent with stored state, or a
/// write fails.
#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive match over Mutation variants is the single source of truth for key layout and must stay in one deterministic sequence"
)]
pub fn expand_commit(ci: u64, batch: &CommitBatch, txn: &mut Txn) -> Result<(), StorageError> {
    if batch.command_version != rushdb_types::command::COMMAND_VERSION {
        return Err(StorageError::UnsupportedCommandVersion(
            batch.command_version,
        ));
    }
    let t = batch.tenant;
    // Records touched by this commit, with the labels named by its header
    // mutations (a retype or delete leaves the old label's text indexes).
    let mut text_owners: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    // Records whose place in the graph this commit changed (structural
    // indexes): created, deleted or retyped records, and the endpoints of
    // created or deleted relationships.
    let mut topology: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    for m in &batch.mutations {
        match m {
            Mutation::SetSecurityPolicy { policy, mode }
            | Mutation::SetSecurityPolicyThreshold { policy, mode, .. } => {
                use rushdb_types::security::{policy_fingerprint_with_min_count, ActivePolicy};
                let min_count = match m {
                    Mutation::SetSecurityPolicyThreshold { min_count, .. } => *min_count,
                    _ => 0,
                };
                let generation_key = security_generation_key(t);
                if let Some(raw) = txn.get(&generation_key)? {
                    let previous = u64::from_be_bytes(
                        raw.as_slice()
                            .try_into()
                            .map_err(|_| StorageError::CorruptLog)?,
                    );
                    if previous >= ci {
                        return Err(StorageError::CorruptLog);
                    }
                }
                let active = ActivePolicy {
                    generation: ci,
                    policy: policy.clone(),
                    mode: *mode,
                    fingerprint: policy_fingerprint_with_min_count(policy, *mode, min_count),
                    min_count,
                };
                txn.put(generation_key, ci.to_be_bytes().to_vec());
                txn.put(
                    security_policy_key(t),
                    serde_json::to_vec(&active).map_err(ser_err)?,
                );
            }
            Mutation::CreateRecordHeader {
                record_id,
                ordinal,
                label,
            } => {
                text_owners
                    .entry(*ordinal)
                    .or_default()
                    .insert(label.clone());
                topology.entry(*ordinal).or_default().insert(label.clone());
                txn.put(
                    rushdb_types::keys::record_id_key(t, record_id),
                    u64be(*ordinal).to_vec(),
                );
                txn.put(
                    rushdb_types::keys::record_hdr_key(t, *ordinal),
                    rushdb_types::keys::encode_record_hdr(record_id, label),
                );
                txn.put(
                    rushdb_types::keys::label_member_key(t, label, *ordinal),
                    Vec::new(),
                );
                ensure_counter(txn, t, "record", ordinal + 1)?;
            }
            Mutation::DeleteRecordHeader {
                record_id,
                ordinal,
                label,
            } => {
                text_owners
                    .entry(*ordinal)
                    .or_default()
                    .insert(label.clone());
                topology.entry(*ordinal).or_default().insert(label.clone());
                txn.del(rushdb_types::keys::record_id_key(t, record_id));
                txn.del(rushdb_types::keys::record_hdr_key(t, *ordinal));
                txn.del(rushdb_types::keys::label_member_key(t, label, *ordinal));
            }
            Mutation::RetypeRecordHeader {
                record_id,
                ordinal,
                from_label,
                to_label,
            } => {
                for owners in [&mut text_owners, &mut topology] {
                    let held = owners.entry(*ordinal).or_default();
                    held.insert(from_label.clone());
                    held.insert(to_label.clone());
                }
                let hdr_key = rushdb_types::keys::record_hdr_key(t, *ordinal);
                let raw = txn.get(&hdr_key)?.ok_or(StorageError::CorruptLog)?;
                let (stored_id, stored_label) =
                    rushdb_types::keys::decode_record_hdr(&raw).ok_or(StorageError::CorruptLog)?;
                if stored_id != *record_id || stored_label != from_label {
                    return Err(StorageError::CorruptLog);
                }
                // Owner support, presence and dictionary contributions move to
                // the new label in the deriver.
                txn.put(
                    hdr_key,
                    rushdb_types::keys::encode_record_hdr(record_id, to_label),
                );
                txn.del(rushdb_types::keys::label_member_key(
                    t, from_label, *ordinal,
                ));
                txn.put(
                    rushdb_types::keys::label_member_key(t, to_label, *ordinal),
                    Vec::new(),
                );
            }
            Mutation::CreateRelationshipHeader {
                relationship_id,
                ordinal,
                rel_type,
                source_ord,
                target_ord,
                source_id,
                target_id,
            } => {
                let hdr = rushdb_types::occurrence::RelationshipHeader {
                    relationship_id: *relationship_id,
                    ordinal: *ordinal,
                    rel_type: rel_type.clone(),
                    source_ord: *source_ord,
                    target_ord: *target_ord,
                    source_id: *source_id,
                    target_id: *target_id,
                };
                txn.put(
                    kx::rel_row_key(t, relationship_id),
                    serde_json::to_vec(&hdr).map_err(ser_err)?,
                );
                txn.put(kx::rel_by_ord_key(t, *ordinal), relationship_id.to_vec());
                adj_put(
                    txn,
                    t,
                    ADJ_OUT,
                    *source_ord,
                    rel_type,
                    *target_ord,
                    *ordinal,
                );
                adj_put(txn, t, ADJ_IN, *target_ord, rel_type, *source_ord, *ordinal);
                // Relationship catalog: capture endpoint labels now so the
                // decrement on delete survives a cascade that removed records.
                if let (Some(src_label), Some(tgt_label)) = (
                    record_label(txn, t, *source_ord)?,
                    record_label(txn, t, *target_ord)?,
                ) {
                    let captured = serde_json::to_vec(&(src_label.clone(), tgt_label.clone()))
                        .map_err(ser_err)?;
                    // The relationship catalog keys on these creation-time
                    // labels, live and on replay alike; the deriver
                    // counts it from this row.
                    txn.set(kx::rel_group_key(t, relationship_id), captured);
                }
                ensure_counter(txn, t, "relationship", ordinal + 1)?;
                topology.entry(*source_ord).or_default();
                topology.entry(*target_ord).or_default();
            }
            Mutation::DeleteRelationshipHeader {
                relationship_id,
                ordinal,
                rel_type,
                source_ord,
                target_ord,
            } => {
                txn.del(kx::rel_row_key(t, relationship_id));
                txn.del(kx::rel_by_ord_key(t, *ordinal));
                adj_del(
                    txn,
                    t,
                    ADJ_OUT,
                    *source_ord,
                    rel_type,
                    *target_ord,
                    *ordinal,
                );
                adj_del(txn, t, ADJ_IN, *target_ord, rel_type, *source_ord, *ordinal);
                txn.del(kx::rel_group_key(t, relationship_id));
                topology.entry(*source_ord).or_default();
                topology.entry(*target_ord).or_default();
            }
            Mutation::EnsureFamily {
                family_id,
                namespace,
                owner_scope,
                normalized_name,
                display_name,
            } => {
                txn.put(
                    kx::family_sig_key(t, namespace, *owner_scope, normalized_name),
                    u64be(*family_id).to_vec(),
                );
                txn.put(
                    kx::family_key(t, *family_id),
                    serde_json::to_vec(&rushdb_types::occurrence::PropertyFamily {
                        family_id: *family_id,
                        namespace: namespace.clone(),
                        owner_scope: *owner_scope,
                        normalized_name: normalized_name.clone(),
                        display_name: display_name.clone(),
                    })
                    .map_err(ser_err)?,
                );
                ensure_counter(txn, t, "family", family_id + 1)?;
            }
            Mutation::EnsureDefinition {
                property_id,
                family_id,
                type_state,
                cardinality,
                nullability,
            } => {
                txn.put(
                    kx::definition_key(t, *family_id, *type_state),
                    u64be(*property_id).to_vec(),
                );
                txn.put(
                    kx::definition_row_key(t, *property_id),
                    serde_json::to_vec(&rushdb_types::occurrence::PropertyDefinition {
                        property_id: *property_id,
                        family_id: *family_id,
                        type_state: *type_state,
                        cardinality: *cardinality,
                        nullability: *nullability,
                    })
                    .map_err(ser_err)?,
                );
                ensure_counter(txn, t, "property-definition", property_id + 1)?;
            }
            Mutation::DeleteFamily {
                family_id,
                namespace,
                owner_scope,
                normalized_name,
                display_name: _display_name,
            } => {
                let fid = *family_id;
                // Remove every occurrence of the family first (records and
                // relationships), so reconstruction never references a deleted
                // family: for each of its pids, walk the owners present under
                // every label (presence rows) and delete their occurrences.
                // Delete-property is a rare, project-wide operation.
                let pids: Vec<u64> = txn
                    .range_prefix(&key(t, &[b"property-row", b""]))
                    .map_err(|e| StorageError::Builder(e.to_string()))?
                    .iter()
                    .map(|(_, raw)| {
                        serde_json::from_slice::<PropertyDefinition>(raw).map_err(ser_err)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .filter(|def| def.family_id == fid)
                    .map(|def| def.property_id)
                    .collect();
                for pid in pids {
                    for (label, _) in
                        crate::derived::prop_support_by_label(txn, t, *owner_scope, pid)?
                    {
                        for nullness in [
                            rushdb_types::keys::NULLNESS_VALUE,
                            rushdb_types::keys::NULLNESS_NULL,
                        ] {
                            for owner in crate::derived::presence_owners(
                                txn,
                                t,
                                *owner_scope,
                                &label,
                                pid,
                                nullness,
                            )? {
                                let prefix =
                                    rushdb_types::keys::occ_pid_prefix(t, *owner_scope, owner, pid);
                                for (occ_key, _) in txn.range_prefix(&prefix)? {
                                    txn.del(occ_key);
                                }
                            }
                        }
                    }
                }
                // Remove definitions (row + by-type index) for this family.
                let def_rows: Vec<(u64, rushdb_types::model::TypeState)> = txn
                    .range_prefix(&key(t, &[b"property-row", b""]))
                    .map_err(|e| StorageError::Builder(e.to_string()))?
                    .iter()
                    .map(|(_, raw)| {
                        serde_json::from_slice::<PropertyDefinition>(raw).map_err(ser_err)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .filter(|def| def.family_id == fid)
                    .map(|def| (def.property_id, def.type_state))
                    .collect();
                for (pid, ts) in def_rows {
                    txn.del(kx::definition_key(t, fid, ts));
                    txn.del(kx::definition_row_key(t, pid));
                }
                txn.del(kx::family_sig_key(
                    t,
                    namespace,
                    *owner_scope,
                    normalized_name,
                ));
                txn.del(kx::family_key(t, fid));
            }
            Mutation::PutOccurrence {
                row,
                canonical_value_bytes,
            } => {
                if row.owner_kind == OwnerKind::Record {
                    text_owners.entry(row.owner_ord).or_default();
                }
                put_occurrence(txn, t, row, canonical_value_bytes.as_deref())?;
            }
            Mutation::DeleteOccurrence { row } => {
                if row.owner_kind == OwnerKind::Record {
                    text_owners.entry(row.owner_ord).or_default();
                }
                delete_occurrence(txn, t, row);
            }
            Mutation::ClearEmbeddingSpace { alias, space_id } => {
                let alias_key = emb_alias_key(t, alias);
                let raw = txn.get(&alias_key)?.ok_or_else(|| {
                    StorageError::Builder("embedding alias no longer exists".into())
                })?;
                let current = u64::from_be_bytes(
                    raw.as_slice()
                        .try_into()
                        .map_err(|_| StorageError::CorruptLog)?,
                );
                if current != *space_id {
                    return Err(StorageError::Builder(
                        "embedding alias identifies a different space".into(),
                    ));
                }
                // Never round-trip binary tenant IDs or slot ordinals through
                // UTF-8. Both scans run under the logical writer admission.
                for prefix in [
                    emb_bindings_prefix(t, *space_id),
                    key(t, &[P_EMB_VECTOR, &u64be(*space_id)]),
                ] {
                    for key in txn.keys_prefix(&prefix)? {
                        txn.del(key);
                    }
                }
                txn.del(rushdb_types::keys::emb_space_version_key(t, *space_id));
                txn.del(rushdb_types::keys::emb_ready_count_key(t, *space_id));
                txn.del(alias_key);
            }
            Mutation::SetMetaValue { key, value } => {
                txn.put(key.as_bytes().to_vec(), value.to_be_bytes().to_vec());
            }
            Mutation::SetMetaBlob { key, value } => {
                txn.put(key.as_bytes().to_vec(), value.clone());
            }
            Mutation::DeleteMetaBlob { key } => {
                txn.del(key.as_bytes().to_vec());
            }
            Mutation::DeleteRawKey { key } => {
                if !key.starts_with(&rushdb_types::keys::key(t, &[])) {
                    return Err(StorageError::Builder(
                        "maintenance key is outside tenant scope".into(),
                    ));
                }
                if crate::derived::LmpgDeriver.is_derived_key(key) {
                    // DC12: derived rows are computed at apply time, never
                    // written by a logged command.
                    return Err(StorageError::Builder(
                        "maintenance key is a derived row".into(),
                    ));
                }
                txn.del(key.clone());
            }
            Mutation::PutRawKey { key, value } => {
                if !key.starts_with(&rushdb_types::keys::key(t, &[])) {
                    return Err(StorageError::Builder(
                        "maintenance key is outside tenant scope".into(),
                    ));
                }
                if crate::derived::LmpgDeriver.is_derived_key(key) {
                    // DC12: derived rows are computed at apply time, never
                    // written by a logged command.
                    return Err(StorageError::Builder(
                        "maintenance key is a derived row".into(),
                    ));
                }
                txn.put(key.clone(), value.clone());
            }
            Mutation::ReclaimEmbeddingSlots { through_ci } => {
                // Erase the oldest reclaimable slots (and their reclamation
                // rows) in one bounded batch. Rows are ordered by `after_ci`,
                // so once one is newer than the floor the rest are too.
                let prefix = emb_reclaim_prefix(t);
                for reclaim_key in txn.keys_prefix_limited(&prefix, EMB_SLOT_RECLAIM_BATCH)? {
                    let Some(raw) = txn.get(&reclaim_key)? else {
                        continue;
                    };
                    let Some((after_ci, space, slot)) = parse_emb_reclaim_value(&raw) else {
                        continue;
                    };
                    if after_ci > *through_ci {
                        break;
                    }
                    txn.del(emb_vector_key(t, space, slot));
                    txn.del(reclaim_key);
                }
            }
            Mutation::RegisterTenant { scope } => {
                use rushdb_storage::tenant_dir::{self, TenantState};
                let by_scope = rushdb_types::keys::tenant_by_scope_key(scope);
                let active = match txn.get(&by_scope)? {
                    Some(raw) => tenant_dir::decode_by_scope(&raw)?.1 == TenantState::Active,
                    None => false,
                };
                if !active {
                    let seq_key = rushdb_types::keys::tenant_seq_key();
                    let next = tenant_dir::decode_seq(txn.get(&seq_key)?.as_deref())?;
                    let tenant = rushdb_types::ids::TenantId(next);
                    txn.put(
                        by_scope,
                        tenant_dir::encode_by_scope(tenant, TenantState::Active),
                    );
                    txn.put(
                        rushdb_types::keys::tenant_by_ord_key(tenant),
                        tenant_dir::encode_by_ord(scope, TenantState::Active),
                    );
                    let after = next.checked_add(1).ok_or_else(|| {
                        StorageError::ResourceExhausted("tenant ordinals exhausted".into())
                    })?;
                    txn.set(seq_key.to_vec(), after.to_be_bytes().to_vec());
                }
            }
            Mutation::DefineIndex { .. }
            | Mutation::IndexBackfill { .. }
            | Mutation::DropIndex { .. }
            | Mutation::IndexDrop { .. }
            | Mutation::IndexApplied { .. } => crate::index::expand(t, ci, m, txn)?,
            Mutation::RetireTenant => {
                use rushdb_storage::tenant_dir::{self, TenantState};
                let by_ord = rushdb_types::keys::tenant_by_ord_key(t);
                if let Some(raw) = txn.get(&by_ord)? {
                    let (scope, _) = tenant_dir::decode_by_ord(&raw)?;
                    txn.put(
                        by_ord,
                        tenant_dir::encode_by_ord(&scope, TenantState::Purged),
                    );
                    txn.put(
                        rushdb_types::keys::tenant_by_scope_key(&scope),
                        tenant_dir::encode_by_scope(t, TenantState::Purged),
                    );
                }
            }
            Mutation::PurgeTenantChunk { max_keys } => {
                // Bounded purge step: erase at most `max_keys` of the smallest
                // remaining purgeable keys without retaining MVCC history (the
                // values are intentionally destroyed, and retaining versions
                // for them is what ballooned storage). The driver loops
                // commits; `purge_batch_keys` is shared with the driver so the
                // decision to stop matches what this arm actually deletes.
                let max_keys = usize::try_from((*max_keys).max(1)).unwrap_or(usize::MAX);
                for k in purge_batch_keys(txn, t, max_keys)? {
                    txn.purge(k);
                }
            }
            Mutation::EnsureEmbeddingSpace { alias, info, new } => {
                if *new {
                    txn.put(
                        rushdb_types::keys::emb_alias_key(t, alias),
                        u64be(info.space_id).to_vec(),
                    );
                }
                txn.put(
                    rushdb_types::keys::emb_space_key(t, info.space_id),
                    serde_json::to_vec(info).map_err(ser_err)?,
                );
                ensure_counter(txn, t, "space", info.space_id + 1)?;
            }
            Mutation::UpsertEmbeddingBinding {
                binding,
                vector,
                expect_live_hash,
            } => {
                let bkey = rushdb_types::keys::emb_binding_key(
                    t,
                    binding.space_id,
                    binding.owner_kind,
                    binding.owner_ord,
                    binding.pid,
                    binding.elem,
                );
                let previous: Option<rushdb_types::occurrence::EmbeddingBinding> = txn
                    .get(&bkey)?
                    .map(|raw| serde_json::from_slice(&raw).map_err(ser_err))
                    .transpose()?;
                if let Some(vec) = vector {
                    // Re-binding allocates a fresh slot; the superseded slot's
                    // bytes stay readable by pinned views until the GC
                    // watermark passes this commit, so defer its deletion.
                    if let Some(old) = &previous {
                        if old.vector_slot != binding.vector_slot {
                            txn.put(
                                emb_reclaim_key(t, ci, binding.space_id, old.vector_slot),
                                emb_reclaim_value(ci, binding.space_id, old.vector_slot),
                            );
                        }
                    }
                    txn.put(
                        emb_vector_key(t, binding.space_id, binding.vector_slot),
                        vec.iter()
                            .flat_map(|f| f.to_le_bytes())
                            .collect::<Vec<u8>>(),
                    );
                    ensure_counter(
                        txn,
                        t,
                        &format!("vecslot/{}", binding.space_id),
                        binding.vector_slot + 1,
                    )?;
                }
                let status = if *expect_live_hash {
                    match lookup_live_hash(txn, t, binding)? {
                        None => rushdb_types::occurrence::BindingStatus::Retired,
                        Some(h) if h != binding.source_hash => {
                            rushdb_types::occurrence::BindingStatus::Stale
                        }
                        Some(_) => rushdb_types::occurrence::BindingStatus::Ready,
                    }
                } else {
                    binding.status
                };
                let mut to_store = binding.clone();
                to_store.status = status;
                txn.put(bkey, serde_json::to_vec(&to_store).map_err(ser_err)?);
                bump_space_version(txn, t, binding.space_id, ci)?;
                let ready = rushdb_types::occurrence::BindingStatus::Ready;
                let was_ready = previous.is_some_and(|old| old.status == ready);
                adjust_ready_count(txn, t, binding.space_id, was_ready, status == ready)?;
            }
        }
    }
    crate::index::mark_text_pending(t, ci, &text_owners, txn)?;
    crate::index::mark_structural_pending(t, ci, &topology, txn)?;
    Ok(())
}

/// Record that commit `ci` changed `space`'s bindings. Several
/// binds in one commit keep the version from before the commit as `prev`.
fn bump_space_version(
    txn: &mut Txn<'_>,
    t: TenantId,
    space: u64,
    ci: u64,
) -> Result<(), StorageError> {
    let key = rushdb_types::keys::emb_space_version_key(t, space);
    let current = match txn.get(&key)? {
        Some(raw) => {
            rushdb_types::keys::parse_emb_space_version(&raw).ok_or(StorageError::CorruptLog)?
        }
        None => (0, 0),
    };
    if current.1 != ci {
        txn.put(
            key,
            rushdb_types::keys::emb_space_version_value(current.1, ci),
        );
    }
    Ok(())
}

/// Keep `space`'s ready-vector count equal to its `Ready` bindings across a
/// binding write that moves one binding from `was_ready` to `is_ready`.
fn adjust_ready_count(
    txn: &mut Txn<'_>,
    t: TenantId,
    space: u64,
    was_ready: bool,
    is_ready: bool,
) -> Result<(), StorageError> {
    let key = rushdb_types::keys::emb_ready_count_key(t, space);
    let Some(raw) = txn.get(&key)? else {
        // A space bound before the count existed: count its bindings once.
        // The scan already sees this commit's write, so no delta applies.
        let mut ready = 0u64;
        for (_, value) in txn.range_prefix(&emb_bindings_prefix(t, space))? {
            let binding: rushdb_types::occurrence::EmbeddingBinding =
                serde_json::from_slice(&value).map_err(ser_err)?;
            if binding.status == rushdb_types::occurrence::BindingStatus::Ready {
                ready += 1;
            }
        }
        txn.put(key, ready.to_be_bytes().to_vec());
        return Ok(());
    };
    if was_ready == is_ready {
        return Ok(());
    }
    let current = u64::from_be_bytes(
        raw.as_slice()
            .try_into()
            .map_err(|_| StorageError::CorruptLog)?,
    );
    let next = if is_ready {
        current + 1
    } else {
        current.checked_sub(1).ok_or(StorageError::CorruptLog)?
    };
    txn.put(key, next.to_be_bytes().to_vec());
    Ok(())
}

fn ser_err(_: serde_json::Error) -> StorageError {
    StorageError::CorruptLog
}

fn adj_put(
    txn: &mut Txn,
    t: TenantId,
    dir: &[u8],
    node: u64,
    rel_type: &str,
    other: u64,
    rel_ord: u64,
) {
    let k = key(
        t,
        &[
            b"adj",
            dir,
            &u64be(node),
            rel_type.as_bytes(),
            &u64be(other),
            &u64be(rel_ord),
        ],
    );
    txn.put(k, vec![1]);
}

fn adj_del(
    txn: &mut Txn,
    t: TenantId,
    dir: &[u8],
    node: u64,
    rel_type: &str,
    other: u64,
    rel_ord: u64,
) {
    let k = key(
        t,
        &[
            b"adj",
            dir,
            &u64be(node),
            rel_type.as_bytes(),
            &u64be(other),
            &u64be(rel_ord),
        ],
    );
    txn.del(k);
}

fn ensure_counter(txn: &mut Txn, t: TenantId, name: &str, next: u64) -> Result<(), StorageError> {
    let ckey = meta_counter_key(t, name);
    let cur = raw_u64(txn.get(&ckey)?.as_deref())?.unwrap_or(0);
    let want = std::cmp::max(cur, next);
    txn.set(ckey, want.to_be_bytes().to_vec());
    Ok(())
}

fn raw_u64(v: Option<&[u8]>) -> Result<Option<u64>, StorageError> {
    v.map(|b| {
        b.try_into()
            .map(u64::from_be_bytes)
            .map_err(|_| StorageError::CorruptLog)
    })
    .transpose()
}

/// Prefixes `bytes` with its length as a big-endian `u64`, producing a
/// self-delimiting encoding for composite keys.
#[must_use]
pub fn len_prefixed(bytes: &[u8]) -> Vec<u8> {
    let mut out = u64be(bytes.len() as u64).to_vec();
    out.extend_from_slice(bytes);
    out
}

/// Resolve a record's primary label by ordinal, if the record exists.
fn record_label(txn: &Txn, t: TenantId, ordinal: u64) -> Result<Option<String>, StorageError> {
    let Some(raw) = txn.get(&rushdb_types::keys::record_hdr_key(t, ordinal))? else {
        return Ok(None);
    };
    let (_, label) = rushdb_types::keys::decode_record_hdr(&raw).ok_or(StorageError::CorruptLog)?;
    Ok(Some(label.to_owned()))
}

/// Value hash of the live occurrence a binding points at, if any.
fn lookup_live_hash(
    txn: &Txn<'_>,
    t: TenantId,
    b: &rushdb_types::occurrence::EmbeddingBinding,
) -> Result<Option<u64>, StorageError> {
    let Some(raw) = txn.get(&rushdb_types::keys::occ_key(
        t,
        b.owner_kind,
        b.owner_ord,
        b.pid,
        b.elem,
    ))?
    else {
        return Ok(None);
    };
    Ok(Some(
        decode_occ_value(&raw)
            .map_err(|_| StorageError::CorruptLog)?
            .value_hash(),
    ))
}

/// Write one canonical occurrence row. Presence, dictionary refcounts,
/// support counters and array markers are derived from it by the deriver.
fn put_occurrence(
    txn: &mut Txn,
    t: TenantId,
    row: &OccurrenceRow,
    canonical_value_bytes: Option<&[u8]>,
) -> Result<(), StorageError> {
    if matches!(
        row.occurrence_value,
        rushdb_types::occurrence::OccurrenceValue::Stored { .. }
    ) && canonical_value_bytes.is_none()
    {
        return Err(StorageError::CorruptLog);
    }
    let occ_key = rushdb_types::keys::occ_key(
        t,
        row.owner_kind,
        row.owner_ord,
        row.property_id,
        row.elem_ordinal.unwrap_or(0),
    );
    txn.put(
        occ_key,
        encode_occ_value(
            row.elem_ordinal.is_some(),
            row.occurrence_value,
            canonical_value_bytes,
        ),
    );
    Ok(())
}

/// Delete one canonical occurrence row. The planner stages deletes only for
/// occurrences it read as live, so no read-before-delete is needed.
fn delete_occurrence(txn: &mut Txn, t: TenantId, row: &OccurrenceRow) {
    txn.del(rushdb_types::keys::occ_key(
        t,
        row.owner_kind,
        row.owner_ord,
        row.property_id,
        row.elem_ordinal.unwrap_or(0),
    ));
}
