//! Unique indexes (#148): one claim row per `(index, value)` naming the
//! record that holds the value. Commit expansion runs under the writer, one
//! commit after another, so a claim checked and written there cannot race
//! another commit: of two writers of one value, the second fails with a
//! conflict that names the label, the property and the record holding it
//! (never the value). Every write path stages its values as occurrences,
//! so every one is checked here.

use std::collections::BTreeSet;

use rushdb_storage::{StorageError, Txn};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::index::{IndexDef, IndexKind, IndexState};
use rushdb_types::keys as k;
use rushdb_types::occurrence::{decode_occ_value, OccurrenceRow, OccurrenceValue};

/// The prefix of every unique-constraint conflict message; transports
/// recognize it (pgwire `23505`, structured `details`).
pub const UNIQUE_CONFLICT: &str = "unique constraint conflict";

/// The unique indexes of one tenant with their property's definitions,
/// read once per commit.
#[derive(Default)]
pub(crate) struct UniqueRules {
    loaded: Option<Vec<(IndexDef, BTreeSet<u64>)>>,
}

impl UniqueRules {
    fn rules(
        &mut self,
        txn: &Txn,
        t: TenantId,
    ) -> Result<&[(IndexDef, BTreeSet<u64>)], StorageError> {
        if self.loaded.is_none() {
            let mut rules = Vec::new();
            for (_, raw) in txn.range_prefix(&k::index_def_prefix(t))? {
                let def: IndexDef =
                    serde_json::from_slice(&raw).map_err(|_| StorageError::CorruptLog)?;
                if def.kind != IndexKind::Unique || matches!(def.state, IndexState::Dropping { .. })
                {
                    continue;
                }
                let pids = crate::index::record_property_pids(txn, t, &def.property)?;
                rules.push((def, pids));
            }
            self.loaded = Some(rules);
        }
        Ok(self.loaded.as_deref().unwrap_or_default())
    }

    /// Forget the rules (a commit that defines or drops an index).
    pub(crate) fn reset(&mut self) {
        self.loaded = None;
    }

    /// The stored bytes of `row` before its delete, when a unique index
    /// covers its property (read only then).
    pub(crate) fn before_delete(
        &mut self,
        txn: &Txn,
        t: TenantId,
        row: &OccurrenceRow,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        if row.owner_kind != OwnerKind::Record
            || !self
                .rules(txn, t)?
                .iter()
                .any(|(_, pids)| pids.contains(&row.property_id))
        {
            return Ok(None);
        }
        txn.get(&k::occ_key(
            t,
            row.owner_kind,
            row.owner_ord,
            row.property_id,
            row.elem_ordinal.unwrap_or(0),
        ))
    }

    /// Claim the value `encoded` (an occurrence row's stored bytes) of a
    /// record for every unique index on its label and property; refuse it
    /// when another record holds it.
    pub(crate) fn claim(
        &mut self,
        txn: &mut Txn,
        t: TenantId,
        row: &OccurrenceRow,
        encoded: &[u8],
    ) -> Result<(), StorageError> {
        if row.owner_kind != OwnerKind::Record
            || !matches!(row.occurrence_value, OccurrenceValue::Stored { .. })
        {
            return Ok(());
        }
        let owner = row.owner_ord;
        let rules: Vec<(IndexDef, bool)> = self
            .rules(txn, t)?
            .iter()
            .map(|(def, pids)| (def.clone(), pids.contains(&row.property_id)))
            .filter(|(_, applies)| *applies)
            .collect();
        if rules.is_empty() {
            return Ok(());
        }
        let Some(label) = record_label(txn, t, owner)? else {
            return Ok(());
        };
        let entry = decode_occ_value(encoded)
            .map_err(|_| StorageError::CorruptLog)?
            .entry
            .to_vec();
        for (def, _) in rules.iter().filter(|(def, _)| def.label == label) {
            let key = k::unique_claim_key(t, def.id, &entry);
            match txn.get(&key)? {
                Some(holder) if decode_owner(&holder)? != owner => {
                    let holder = decode_owner(&holder)?;
                    return Err(StorageError::Builder(conflict(txn, t, def, holder)?));
                }
                Some(_) => {}
                None => txn.put(key, owner.to_be_bytes().to_vec()),
            }
        }
        Ok(())
    }

    /// Release the claims of a record value `old` (the occurrence's bytes
    /// before its delete) that the record no longer holds.
    pub(crate) fn release(
        &mut self,
        txn: &mut Txn,
        t: TenantId,
        row: &OccurrenceRow,
        old: &[u8],
    ) -> Result<(), StorageError> {
        if row.owner_kind != OwnerKind::Record {
            return Ok(());
        }
        let owner = row.owner_ord;
        let ids: Vec<u32> = self
            .rules(txn, t)?
            .iter()
            .filter(|(_, pids)| pids.contains(&row.property_id))
            .map(|(def, _)| def.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let old = decode_occ_value(old).map_err(|_| StorageError::CorruptLog)?;
        if !matches!(old.value, OccurrenceValue::Stored { .. }) {
            return Ok(());
        }
        let entry = old.entry.to_vec();
        // An array may hold the value twice: the claim stays while one does.
        for (_, raw) in txn.range_prefix(&k::occ_pid_prefix(
            t,
            OwnerKind::Record,
            owner,
            row.property_id,
        ))? {
            let held = decode_occ_value(&raw).map_err(|_| StorageError::CorruptLog)?;
            if held.entry == entry.as_slice() {
                return Ok(());
            }
        }
        for id in ids {
            let key = k::unique_claim_key(t, id, &entry);
            if let Some(holder) = txn.get(&key)? {
                if decode_owner(&holder)? == owner {
                    txn.del(key);
                }
            }
        }
        Ok(())
    }
}

/// Claim every value of `owners` (records of `def`'s label) for unique index
/// `def`, as its build covers them. `Ok(false)` when a value is held by two
/// of them: the index cannot be built.
pub(crate) fn backfill_claims(
    txn: &mut Txn,
    t: TenantId,
    def: &IndexDef,
    owners: &[u64],
) -> Result<bool, StorageError> {
    let pids = crate::index::record_property_pids(txn, t, &def.property)?;
    for &owner in owners {
        for &pid in &pids {
            let entries: Vec<Vec<u8>> = txn
                .range_prefix(&k::occ_pid_prefix(t, OwnerKind::Record, owner, pid))?
                .into_iter()
                .filter_map(|(_, raw)| {
                    decode_occ_value(&raw).ok().and_then(|occ| {
                        matches!(occ.value, OccurrenceValue::Stored { .. })
                            .then(|| occ.entry.to_vec())
                    })
                })
                .collect();
            for entry in entries {
                let key = k::unique_claim_key(t, def.id, &entry);
                match txn.get(&key)? {
                    Some(holder) if decode_owner(&holder)? != owner => return Ok(false),
                    Some(_) => {}
                    None => txn.put(key, owner.to_be_bytes().to_vec()),
                }
            }
        }
    }
    Ok(true)
}

/// Remove every claim of unique index `id` (its drop).
pub(crate) fn drop_claims(txn: &mut Txn, t: TenantId, id: u32) -> Result<(), StorageError> {
    for key in txn.keys_prefix(&k::unique_claim_prefix(t, id))? {
        txn.del(key);
    }
    Ok(())
}

fn decode_owner(raw: &[u8]) -> Result<u64, StorageError> {
    <[u8; 8]>::try_from(raw)
        .map(u64::from_be_bytes)
        .map_err(|_| StorageError::CorruptLog)
}

fn record_label(txn: &Txn, t: TenantId, owner: u64) -> Result<Option<String>, StorageError> {
    txn.get(&k::record_hdr_key(t, owner))?
        .map(|raw| {
            k::decode_record_hdr(&raw)
                .map(|(_, label)| label.to_owned())
                .ok_or(StorageError::CorruptLog)
        })
        .transpose()
}

/// The conflict message: label, property and the holding record's id.
fn conflict(txn: &Txn, t: TenantId, def: &IndexDef, holder: u64) -> Result<String, StorageError> {
    let id = txn
        .get(&k::record_hdr_key(t, holder))?
        .and_then(|raw| k::decode_record_hdr(&raw).map(|(id, _)| id))
        .map_or_else(
            || holder.to_string(),
            |id| uuid::Uuid::from_bytes(id).to_string(),
        );
    Ok(format!(
        "{UNIQUE_CONFLICT} on `{}`.`{}`: record {id} holds the value",
        def.label, def.property
    ))
}
