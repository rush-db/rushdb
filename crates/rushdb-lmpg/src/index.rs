//! Declared ordered indexes: definitions, expansion of the index
//! mutations, apply-time maintenance of `INDEX_ENTRY` rows, and the read API
//! the planner uses.
//!
//! Entries are derived rows. For an index `I` and a record `r`, the
//! entries are `{(value, r) : value ∈ OCC(r, pids(I.property))}` when `r` has
//! `I.label` and `I.state` covers `r`'s ordinal, and nothing otherwise. Every
//! commit recomputes that set for exactly the `(I, r)` pairs it can change —
//! records whose occurrences or header changed, and records whose coverage
//! moved with `I`'s cursor — so entries always equal a from-scratch
//! derivation of the base rows, during a build or drop as well.

use std::collections::{BTreeMap, BTreeSet};

use rushdb_storage::{DeriveRead, StorageError, Txn};
use rushdb_types::command::Mutation;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::index::{IndexDef, IndexKind, IndexState, TEXT_ALL_PROPERTIES};
use rushdb_types::keys::{self as k, key};
use rushdb_types::model::{LogicalType, TypeState};
use rushdb_types::occurrence::{decode_occ_value, OccurrenceValue};

use crate::reader::{keys_ext, TryRead};

fn index_seq_key(t: TenantId) -> Vec<u8> {
    key(t, &[b"meta", b"index-seq"])
}

fn decode_def(raw: &[u8]) -> Result<IndexDef, StorageError> {
    serde_json::from_slice(raw).map_err(|_| StorageError::CorruptLog)
}

fn encode_def(def: &IndexDef) -> Vec<u8> {
    serde_json::to_vec(def).expect("index definitions serialize")
}

fn builder(message: impl Into<String>) -> StorageError {
    StorageError::Builder(message.into())
}

/// Smallest structural embedding.
pub const STRUCTURAL_MIN_DIMENSIONS: u32 = 8;
/// Largest structural embedding.
pub const STRUCTURAL_MAX_DIMENSIONS: u32 = 4096;
/// A record with more relationships than this is a hub: a change next to
/// it does not mark its neighbours one by one; the structural indexes of
/// its tenant are rebuilt instead (every member marked again).
pub const STRUCTURAL_HUB_DEGREE: usize = 10_000;

fn def_by_id(txn: &Txn, t: TenantId, id: u32) -> Result<IndexDef, StorageError> {
    let raw = txn
        .get(&k::index_def_key(t, id))?
        .ok_or_else(|| builder(format!("index {id} does not exist")))?;
    decode_def(&raw)
}

fn id_by_name(txn: &Txn, t: TenantId, name: &str) -> Result<Option<u32>, StorageError> {
    txn.get(&k::index_name_key(t, name))?
        .map(|raw| {
            <[u8; 4]>::try_from(raw.as_slice())
                .map(u32::from_be_bytes)
                .map_err(|_| StorageError::CorruptLog)
        })
        .transpose()
}

/// Expand one index mutation.
///
/// # Errors
/// Returns [`StorageError::Builder`] for a duplicate or unknown name, an
/// empty label/property, or a transition the current state does not allow.
pub(crate) fn expand(
    t: TenantId,
    ci: u64,
    m: &Mutation,
    txn: &mut Txn,
) -> Result<(), StorageError> {
    match m {
        Mutation::DefineIndex {
            name,
            label,
            property,
            kind,
        } => define(t, name, label, property, *kind, txn),
        Mutation::IndexBackfill { id, through } => backfill(t, ci, *id, *through, txn),
        Mutation::DropIndex { name } => {
            let id = id_by_name(txn, t, name)?
                .ok_or_else(|| builder(format!("index {name} does not exist")))?;
            let mut def = def_by_id(txn, t, id)?;
            if !matches!(def.state, IndexState::Dropping { .. }) {
                def.state = IndexState::Dropping { cursor: 0 };
                txn.put(k::index_def_key(t, id), encode_def(&def));
            }
            Ok(())
        }
        Mutation::IndexDrop { id, through } => drop_step(t, *id, *through, txn),
        Mutation::IndexApplied {
            index,
            upto,
            owners,
        } => {
            for owner in owners {
                let pending = k::index_pending_key(t, *index, *owner);
                let Some(raw) = txn.get(&pending)? else {
                    continue;
                };
                let marked = <[u8; 8]>::try_from(raw.as_slice())
                    .map(u64::from_be_bytes)
                    .map_err(|_| StorageError::CorruptLog)?;
                // A change committed after the applier's snapshot stays
                // pending.
                if marked <= *upto {
                    txn.del(pending);
                }
            }
            Ok(())
        }
        _ => Err(builder("not an index mutation")),
    }
}

fn define(
    t: TenantId,
    name: &str,
    label: &str,
    property: &str,
    kind: IndexKind,
    txn: &mut Txn,
) -> Result<(), StorageError> {
    let property = if property == TEXT_ALL_PROPERTIES {
        if kind == IndexKind::Ordered {
            return Err(builder("an ordered index covers one named property"));
        }
        property.to_owned()
    } else if let IndexKind::Structural { .. } = kind {
        return Err(builder(format!(
            "a structural index reads topology: its property is `{TEXT_ALL_PROPERTIES}`"
        )));
    } else {
        rushdb_types::value::normalize_name(property)
    };
    if let IndexKind::Structural { dimensions } = kind {
        if !(STRUCTURAL_MIN_DIMENSIONS..=STRUCTURAL_MAX_DIMENSIONS).contains(&dimensions) {
            return Err(builder(format!(
                "structural index dimensions must be {STRUCTURAL_MIN_DIMENSIONS}..={STRUCTURAL_MAX_DIMENSIONS}"
            )));
        }
    }
    if name.trim().is_empty() || label.trim().is_empty() || property.is_empty() {
        return Err(builder("an index needs a name, a label and a property"));
    }
    if id_by_name(txn, t, name)?.is_some() {
        return Err(builder(format!("index {name} already exists")));
    }
    let id = match txn.get(&index_seq_key(t))? {
        Some(raw) => <[u8; 4]>::try_from(raw.as_slice())
            .map(u32::from_be_bytes)
            .map_err(|_| StorageError::CorruptLog)?
            .checked_add(1)
            .ok_or_else(|| builder("index ids exhausted"))?,
        None => 1,
    };
    txn.put(index_seq_key(t), id.to_be_bytes().to_vec());
    txn.put(k::index_name_key(t, name), id.to_be_bytes().to_vec());
    txn.put(
        k::index_def_key(t, id),
        encode_def(&IndexDef {
            id,
            name: name.to_owned(),
            label: label.to_owned(),
            property,
            kind,
            state: IndexState::Building { cursor: 0 },
        }),
    );
    Ok(())
}

fn backfill(
    t: TenantId,
    ci: u64,
    id: u32,
    through: Option<u64>,
    txn: &mut Txn,
) -> Result<(), StorageError> {
    let mut def = def_by_id(txn, t, id)?;
    let IndexState::Building { cursor } = def.state else {
        return Err(builder(format!("index {} is not building", def.name)));
    };
    def.state = match through {
        Some(next) => IndexState::Building {
            cursor: cursor.max(next),
        },
        None => IndexState::Ready,
    };
    if def.kind.applied() {
        // Text documents and structural vectors live outside the store: the
        // newly covered members become pending for the applier.
        for owner in label_members_after(txn, t, &def.label, cursor, through)? {
            txn.put(
                k::index_pending_key(t, id, owner),
                ci.to_be_bytes().to_vec(),
            );
        }
    }
    txn.put(k::index_def_key(t, id), encode_def(&def));
    Ok(())
}

fn drop_step(
    t: TenantId,
    id: u32,
    through: Option<u64>,
    txn: &mut Txn,
) -> Result<(), StorageError> {
    let mut def = def_by_id(txn, t, id)?;
    let IndexState::Dropping { cursor } = def.state else {
        return Err(builder(format!("index {} is not dropping", def.name)));
    };
    if def.kind.applied() {
        // Pending rows of members past the drop cursor go with it.
        for owner in label_members_after(txn, t, &def.label, cursor, through)? {
            txn.del(k::index_pending_key(t, id, owner));
        }
        if through.is_none() {
            for key in txn.keys_prefix(&k::index_pending_prefix(t, id))? {
                txn.del(key);
            }
        }
    }
    if let Some(next) = through {
        def.state = IndexState::Dropping {
            cursor: cursor.max(next),
        };
        txn.put(k::index_def_key(t, id), encode_def(&def));
    } else {
        txn.del(k::index_def_key(t, id));
        txn.del(k::index_name_key(t, &def.name));
    }
    Ok(())
}

/// Members of `label` with ordinal in `[from, to)` (`to` `None` =
/// unbounded).
fn label_members_after(
    txn: &Txn,
    t: TenantId,
    label: &str,
    from: u64,
    to: Option<u64>,
) -> Result<Vec<u64>, StorageError> {
    let lo = k::label_member_key(t, label, from);
    let hi = match to {
        Some(to) => k::label_member_key(t, label, to),
        None => rushdb_storage::prefix_range(&k::label_member_prefix(t, label)).1,
    };
    txn.range(&lo, &hi)?
        .iter()
        .map(|(key, _)| {
            k::parse_label_member_key(key)
                .map(|(_, owner)| owner)
                .ok_or(StorageError::CorruptLog)
        })
        .collect()
}

/// Mark `owners` pending on every text index of their labels (each owner
/// with the labels it held during the commit) that covers them.
///
/// # Errors
/// Returns [`StorageError`] if the catalog or a header cannot be read.
pub(crate) fn mark_text_pending(
    t: TenantId,
    ci: u64,
    owners: &BTreeMap<u64, BTreeSet<String>>,
    txn: &mut Txn,
) -> Result<(), StorageError> {
    if owners.is_empty() {
        return Ok(());
    }
    let defs: Vec<IndexDef> = txn
        .range_prefix(&k::index_def_prefix(t))?
        .iter()
        .map(|(_, raw)| decode_def(raw))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|def| {
            matches!(def.kind, IndexKind::Text { .. })
                && !matches!(def.state, IndexState::Dropping { .. })
        })
        .collect();
    if defs.is_empty() {
        return Ok(());
    }
    let ci = ci.to_be_bytes().to_vec();
    for (owner, labels) in owners {
        let current =
            crate::reader::try_access::record_by_ord(&*txn, t, *owner)?.map(|header| header.label);
        for def in &defs {
            let held = labels.contains(&def.label) || current.as_deref() == Some(&def.label);
            if held && def.state.covers(*owner) {
                txn.put(k::index_pending_key(t, def.id, *owner), ci.clone());
            }
        }
    }
    Ok(())
}

/// Mark pending, on every structural index, the records whose embedding a
/// commit changed: each record in `touched` (created, deleted, retyped, or
/// an endpoint of a created or deleted relationship, with the labels it held
/// during the commit) and every record adjacent to it, since a structural
/// embedding reads two hops. Next to a hub ([`STRUCTURAL_HUB_DEGREE`]) the
/// index restarts its build instead.
///
/// # Errors
/// Returns [`StorageError`] if the catalog, a header or an adjacency list
/// cannot be read.
pub(crate) fn mark_structural_pending(
    t: TenantId,
    ci: u64,
    touched: &BTreeMap<u64, BTreeSet<String>>,
    txn: &mut Txn,
) -> Result<(), StorageError> {
    if touched.is_empty() {
        return Ok(());
    }
    let defs: Vec<IndexDef> = txn
        .range_prefix(&k::index_def_prefix(t))?
        .iter()
        .map(|(_, raw)| decode_def(raw))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|def| {
            matches!(def.kind, IndexKind::Structural { .. })
                && !matches!(def.state, IndexState::Dropping { .. })
        })
        .collect();
    if defs.is_empty() {
        return Ok(());
    }
    let mut affected = touched.clone();
    let mut hub = false;
    for owner in touched.keys() {
        match crate::reader::try_access::neighbours_limited(
            &*txn,
            t,
            *owner,
            STRUCTURAL_HUB_DEGREE,
        )? {
            Some(neighbours) => {
                for neighbour in neighbours {
                    affected.entry(neighbour).or_default();
                }
            }
            None => hub = true,
        }
    }
    let ci = ci.to_be_bytes().to_vec();
    for mut def in defs {
        if hub {
            def.state = IndexState::Building { cursor: 0 };
            txn.put(k::index_def_key(t, def.id), encode_def(&def));
            continue;
        }
        for (owner, labels) in &affected {
            let current = crate::reader::try_access::record_by_ord(&*txn, t, *owner)?
                .map(|header| header.label);
            let held = labels.contains(&def.label) || current.as_deref() == Some(&def.label);
            if held && def.state.covers(*owner) {
                txn.put(k::index_pending_key(t, def.id, *owner), ci.clone());
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Read API
// ---------------------------------------------------------------------------

/// Every index of a tenant, in id order.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a definition is corrupt.
pub fn definitions<R: TryRead + ?Sized>(r: &R, t: TenantId) -> Result<Vec<IndexDef>, StorageError> {
    r.range_prefix(&k::index_def_prefix(t))?
        .iter()
        .map(|(_, raw)| decode_def(raw))
        .collect()
}

/// The index called `name`, if any.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is corrupt.
pub fn by_name<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    name: &str,
) -> Result<Option<IndexDef>, StorageError> {
    let Some(raw) = r.get(&k::index_name_key(t, name))? else {
        return Ok(None);
    };
    let id = <[u8; 4]>::try_from(raw.as_slice())
        .map(u32::from_be_bytes)
        .map_err(|_| StorageError::CorruptLog)?;
    r.get(&k::index_def_key(t, id))?
        .map(|raw| decode_def(&raw))
        .transpose()
}

/// The `Ready` ordered index on `(label, property)` (normalized), if any.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a definition is corrupt.
pub fn ready_ordered<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    label: &str,
    property: &str,
) -> Result<Option<IndexDef>, StorageError> {
    Ok(definitions(r, t)?.into_iter().find(|def| {
        def.kind == IndexKind::Ordered
            && def.state == IndexState::Ready
            && def.label == label
            && def.property == property
    }))
}

/// `(entry, owner)` rows of index `id` with `start <= entry < end` (entry is
/// `tag ‖ order-preserving payload`; an empty `end` is unbounded).
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a key is malformed.
pub fn entries<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    id: u32,
    start: &[u8],
    end: &[u8],
) -> Result<Vec<(Vec<u8>, u64)>, StorageError> {
    let mut rows = Vec::new();
    for_each_entry(r, t, id, start, end, &mut |entry, owner| {
        rows.push((entry.to_vec(), owner));
        true
    })?;
    Ok(rows)
}

/// Streams the `(entry, owner)` rows of index `id` with
/// `start <= entry < end` to `visit` in entry order (owners ascending within
/// one entry) until it returns `false`, without collecting them.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a key is malformed.
pub fn for_each_entry<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    id: u32,
    start: &[u8],
    end: &[u8],
    visit: &mut dyn FnMut(&[u8], u64) -> bool,
) -> Result<(), StorageError> {
    let prefix = k::index_entry_prefix(t, id);
    let mut lo = prefix.clone();
    lo.extend_from_slice(start);
    let hi = if end.is_empty() {
        rushdb_storage::prefix_range(&prefix).1
    } else {
        let mut hi = prefix.clone();
        hi.extend_from_slice(end);
        hi
    };
    let mut corrupt = false;
    r.for_each_key(&lo, &hi, &mut |key| {
        if let Some((entry, owner)) = k::parse_index_entry_key(key, &prefix) {
            visit(entry, owner)
        } else {
            corrupt = true;
            false
        }
    })?;
    if corrupt {
        return Err(StorageError::CorruptLog);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Apply-time maintenance
// ---------------------------------------------------------------------------

/// What one commit touched that can change index entries.
#[derive(Default)]
pub(crate) struct IndexWork {
    /// `(tenant, record ordinal, pid)` whose occurrences changed.
    pub(crate) occurrences: BTreeSet<(TenantId, u64, u64)>,
    /// `(tenant, record ordinal)` whose header (label) changed.
    pub(crate) headers: BTreeSet<(TenantId, u64)>,
    /// `(tenant, index id)` whose definition changed.
    pub(crate) definitions: BTreeSet<(TenantId, u32)>,
}

impl IndexWork {
    fn tenants(&self) -> BTreeSet<TenantId> {
        self.occurrences
            .iter()
            .map(|(t, _, _)| *t)
            .chain(self.headers.iter().map(|(t, _)| *t))
            .chain(self.definitions.iter().map(|(t, _)| *t))
            .collect()
    }
}

/// Typed definition ids of property `norm` (record scope), read from `r`.
fn property_pids(
    r: &dyn DeriveRead,
    t: TenantId,
    norm: &str,
) -> Result<BTreeSet<u64>, StorageError> {
    Ok(typed_pids(&|key| r.get(key), t, norm)?
        .into_iter()
        .map(|(_, pid)| pid)
        .collect())
}

/// `(type, pid)` of every typed definition of record property `name`.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is corrupt.
pub fn record_property_types<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    name: &str,
) -> Result<Vec<(LogicalType, u64)>, StorageError> {
    typed_pids(
        &|key| r.get(key),
        t,
        &rushdb_types::value::normalize_name(name),
    )
}

type Get<'a> = dyn Fn(&[u8]) -> Result<Option<Vec<u8>>, StorageError> + 'a;

fn typed_pids(
    get: &Get<'_>,
    t: TenantId,
    norm: &str,
) -> Result<Vec<(LogicalType, u64)>, StorageError> {
    let Some(raw) = get(&keys_ext::family_sig_key(
        t,
        crate::plan::DEFAULT_NAMESPACE,
        OwnerKind::Record,
        norm,
    ))?
    else {
        return Ok(Vec::new());
    };
    let fid = <[u8; 8]>::try_from(raw.as_slice())
        .map(u64::from_be_bytes)
        .map_err(|_| StorageError::CorruptLog)?;
    let mut pids = Vec::new();
    for ty in [
        LogicalType::Boolean,
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::Utf8,
        LogicalType::DateTime,
    ] {
        if let Some(raw) = get(&keys_ext::definition_key(t, fid, TypeState::Known(ty)))? {
            let pid = <[u8; 8]>::try_from(raw.as_slice())
                .map(u64::from_be_bytes)
                .map_err(|_| StorageError::CorruptLog)?;
            pids.push((ty, pid));
        }
    }
    Ok(pids)
}

fn record_label(
    r: &dyn DeriveRead,
    t: TenantId,
    owner: u64,
) -> Result<Option<String>, StorageError> {
    r.get(&k::record_hdr_key(t, owner))?
        .map(|raw| {
            k::decode_record_hdr(&raw)
                .map(|(_, label)| label.to_owned())
                .ok_or(StorageError::CorruptLog)
        })
        .transpose()
}

/// The entries `def` holds for `owner` in state `r`.
fn owner_entries(
    r: &dyn DeriveRead,
    t: TenantId,
    def: &IndexDef,
    pids: &BTreeSet<u64>,
    owner: u64,
) -> Result<BTreeSet<Vec<u8>>, StorageError> {
    let mut out = BTreeSet::new();
    if !def.state.covers(owner) || record_label(r, t, owner)?.as_deref() != Some(&def.label) {
        return Ok(out);
    }
    for &pid in pids {
        for (_, raw) in r.range_prefix(&k::occ_pid_prefix(t, OwnerKind::Record, owner, pid))? {
            let occ = decode_occ_value(&raw).map_err(|_| StorageError::CorruptLog)?;
            if matches!(occ.value, OccurrenceValue::Stored { .. }) {
                out.insert(occ.entry.to_vec());
            }
        }
    }
    Ok(out)
}

/// Record ordinals of `label` in `[start, end)` present in either state.
fn members_in(
    before: &dyn DeriveRead,
    after: &dyn DeriveRead,
    t: TenantId,
    label: &str,
    (start, end): (u64, u64),
) -> Result<BTreeSet<u64>, StorageError> {
    if start >= end {
        return Ok(BTreeSet::new());
    }
    let lo = k::label_member_key(t, label, start);
    let hi = if end == u64::MAX {
        rushdb_storage::prefix_range(&k::label_member_prefix(t, label)).1
    } else {
        k::label_member_key(t, label, end)
    };
    let mut owners = BTreeSet::new();
    for view in [before, after] {
        for (key, _) in view.range(&lo, &hi)? {
            let (_, owner) = k::parse_label_member_key(&key).ok_or(StorageError::CorruptLog)?;
            owners.insert(owner);
        }
    }
    Ok(owners)
}

/// Ordinal intervals whose coverage differs between two states.
fn coverage_changes(before: Option<IndexState>, after: Option<IndexState>) -> Vec<(u64, u64)> {
    let interval = |state: Option<IndexState>| state.map_or((0, 0), IndexState::covered);
    let ((b0, b1), (a0, a1)) = (interval(before), interval(after));
    let mut cuts = vec![b0, b1, a0, a1];
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .map(|w| (w[0], w[1]))
        .filter(|&(s, _)| (b0 <= s && s < b1) != (a0 <= s && s < a1))
        .collect()
}

/// Derived `INDEX_ENTRY` changes of one commit, as final row values
/// (`Some(empty)` = present, `None` = absent).
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is corrupt.
pub(crate) fn derive(
    work: &IndexWork,
    before: &dyn DeriveRead,
    after: &dyn DeriveRead,
    rows: &mut BTreeMap<Vec<u8>, Option<Vec<u8>>>,
) -> Result<(), StorageError> {
    for t in work.tenants() {
        let load = |r: &dyn DeriveRead| -> Result<BTreeMap<u32, IndexDef>, StorageError> {
            r.range_prefix(&k::index_def_prefix(t))?
                .iter()
                .map(|(_, raw)| decode_def(raw).map(|def| (def.id, def)))
                .collect()
        };
        let defs_after = load(after)?;
        let touched_defs: Vec<u32> = work
            .definitions
            .iter()
            .filter(|(tenant, _)| *tenant == t)
            .map(|(_, id)| *id)
            .collect();
        if defs_after.is_empty() && touched_defs.is_empty() {
            continue;
        }
        let defs_before = load(before)?;
        let ids: BTreeSet<u32> = defs_before
            .keys()
            .chain(defs_after.keys())
            .copied()
            .collect();
        for id in ids {
            let (old_def, new_def) = (defs_before.get(&id), defs_after.get(&id));
            let def = new_def.or(old_def).ok_or(StorageError::CorruptLog)?;
            if def.kind != IndexKind::Ordered {
                continue;
            }
            let pids_before = property_pids(before, t, &def.property)?;
            let pids_after = property_pids(after, t, &def.property)?;
            let mut owners: BTreeSet<u64> = work
                .occurrences
                .iter()
                .filter(|(tenant, _, pid)| {
                    *tenant == t && (pids_before.contains(pid) || pids_after.contains(pid))
                })
                .map(|(_, owner, _)| *owner)
                .collect();
            owners.extend(
                work.headers
                    .iter()
                    .filter(|(tenant, _)| *tenant == t)
                    .map(|(_, owner)| *owner),
            );
            if touched_defs.contains(&id) {
                for range in
                    coverage_changes(old_def.map(|def| def.state), new_def.map(|def| def.state))
                {
                    owners.extend(members_in(before, after, t, &def.label, range)?);
                }
            }
            for owner in owners {
                let old = match old_def {
                    Some(def) => owner_entries(before, t, def, &pids_before, owner)?,
                    None => BTreeSet::new(),
                };
                let new = match new_def {
                    Some(def) => owner_entries(after, t, def, &pids_after, owner)?,
                    None => BTreeSet::new(),
                };
                for entry in old.difference(&new) {
                    rows.insert(k::index_entry_key(t, id, entry, owner), None);
                }
                for entry in new.difference(&old) {
                    rows.insert(k::index_entry_key(t, id, entry, owner), Some(Vec::new()));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_changes_are_the_symmetric_difference() {
        let building = |cursor| Some(IndexState::Building { cursor });
        assert!(coverage_changes(None, building(0)).is_empty());
        assert_eq!(coverage_changes(building(10), building(20)), vec![(10, 20)]);
        assert_eq!(
            coverage_changes(building(20), Some(IndexState::Ready)),
            vec![(20, u64::MAX)]
        );
        assert_eq!(
            coverage_changes(
                Some(IndexState::Ready),
                Some(IndexState::Dropping { cursor: 5 })
            ),
            vec![(0, 5)]
        );
        assert!(coverage_changes(Some(IndexState::Ready), Some(IndexState::Ready)).is_empty());
    }
}

// ---------------------------------------------------------------------------
// Facade: definition, drop, and the online build/drop driver
// ---------------------------------------------------------------------------

/// Outcome of one [`crate::Lmpg::index_step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexStep {
    /// The cursor advanced to this record ordinal.
    Advanced(u64),
    /// The build reached `Ready`, or the drop removed the definition.
    Finished,
    /// Nothing to do (the index is `Ready`, or gone).
    Idle,
}

impl crate::Lmpg {
    fn index_commit(&self, mutation: Mutation) -> crate::LmpgResult<()> {
        let tenant = self.tenant;
        crate::run_mutation(self, None, move |ci, _txn| {
            let mut batch = crate::empty_batch(tenant, ci);
            batch.mutations.push(mutation);
            Ok((batch, ()))
        })
    }

    /// Declare an ordered index on `(label, property)`. It starts `Building`;
    /// [`Self::index_step`] (or the server's builder) backfills it.
    ///
    /// # Errors
    /// Returns [`crate::LmpgError`] for a duplicate name, an empty field, or a
    /// failed commit.
    pub fn define_index(
        &self,
        name: &str,
        label: &str,
        property: &str,
        kind: IndexKind,
    ) -> crate::LmpgResult<IndexDef> {
        self.index_commit(Mutation::DefineIndex {
            name: name.to_owned(),
            label: label.to_owned(),
            property: property.to_owned(),
            kind,
        })?;
        let view = self
            .store
            .read_view()
            .map_err(|e| crate::LmpgError::Conflict(e.to_string()))?;
        by_name(&view, self.tenant, name)
            .map_err(|e| crate::LmpgError::Conflict(e.to_string()))?
            .ok_or_else(|| crate::LmpgError::Conflict(format!("index {name} vanished")))
    }

    /// Start dropping index `name`; [`Self::index_step`] removes its entries
    /// in batches, then the definition.
    ///
    /// # Errors
    /// Returns [`crate::LmpgError`] if the index does not exist or the commit
    /// fails.
    pub fn drop_index(&self, name: &str) -> crate::LmpgResult<()> {
        self.index_commit(Mutation::DropIndex {
            name: name.to_owned(),
        })
    }

    /// Advance a building or dropping index by at most `batch` records.
    ///
    /// # Errors
    /// Returns [`crate::LmpgError`] if a read or the commit fails.
    pub fn index_step(&self, id: u32, batch: usize) -> crate::LmpgResult<IndexStep> {
        let storage = |e: StorageError| crate::LmpgError::Conflict(e.to_string());
        let view = self.store.read_view().map_err(storage)?;
        let Some(def) = definitions(&view, self.tenant)
            .map_err(storage)?
            .into_iter()
            .find(|def| def.id == id)
        else {
            return Ok(IndexStep::Idle);
        };
        let cursor = match def.state {
            IndexState::Ready => return Ok(IndexStep::Idle),
            IndexState::Building { cursor } | IndexState::Dropping { cursor } => cursor,
        };
        let prefix = k::label_member_prefix(self.tenant, &def.label);
        let mut cursor_scan = view
            .scan(
                rushdb_storage::ScanBounds {
                    start: std::ops::Bound::Included(k::label_member_key(
                        self.tenant,
                        &def.label,
                        cursor,
                    )),
                    end: std::ops::Bound::Excluded(rushdb_storage::prefix_range(&prefix).1),
                },
                rushdb_storage::ScanProjection::Keys,
            )
            .map_err(storage)?;
        let rows = cursor_scan
            .next_batch(batch.max(1), usize::MAX)
            .map_err(storage)?
            .rows;
        let last = match rows.last() {
            Some(row) => Some(
                k::parse_label_member_key(&row.key)
                    .ok_or_else(|| storage(StorageError::CorruptLog))?
                    .1,
            ),
            None => None,
        };
        drop(cursor_scan);
        drop(view);
        // Fewer than a batch left: finish in this commit (the deriver covers
        // the remaining records, at most a batch).
        let (through, step) = match last {
            Some(ord) if rows.len() >= batch.max(1) => {
                let next = ord.saturating_add(1);
                (Some(next), IndexStep::Advanced(next))
            }
            _ => (None, IndexStep::Finished),
        };
        self.index_commit(match def.state {
            IndexState::Dropping { .. } => Mutation::IndexDrop { id, through },
            _ => Mutation::IndexBackfill { id, through },
        })?;
        Ok(step)
    }
}

// ---------------------------------------------------------------------------
// Text indexes: pending rows
// ---------------------------------------------------------------------------

/// Up to `limit` owners pending on text index `id`, in owner order.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a key is malformed.
pub fn pending<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    id: u32,
    limit: usize,
) -> Result<Vec<u64>, StorageError> {
    let prefix = k::index_pending_prefix(t, id);
    r.range_prefix_limited(&prefix, limit)?
        .iter()
        .map(|(key, _)| k::read_u64(key, prefix.len()).ok_or(StorageError::CorruptLog))
        .collect()
}

impl crate::Lmpg {
    /// Record that the text applier indexed `owners` of text index `index` as
    /// of snapshot `upto`; their pending rows go unless re-marked since.
    ///
    /// # Errors
    /// Returns [`crate::LmpgError`] if the commit fails.
    pub fn index_applied(&self, index: u32, upto: u64, owners: Vec<u64>) -> crate::LmpgResult<()> {
        self.index_commit(Mutation::IndexApplied {
            index,
            upto,
            owners,
        })
    }
}
