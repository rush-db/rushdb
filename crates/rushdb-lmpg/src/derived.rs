//! Derived rows on disk (DC1/DC10/DC12, storage format v3).
//!
//! Base rows (`OCC`, `RECORD_HDR`, `LABEL`, adjacency, relationship rows and
//! their creation-time label groups) are the only stored truth. Everything a
//! query needs to count — presence, the per-property value dictionary,
//! counters, array markers — is derived from them by [`LmpgDeriver`]: one
//! deterministic function that turns a commit's base ops plus the state before
//! it into materialized derived rows. Storage applies those rows in the same
//! backend batch and at the same commit index as the base, never logs them,
//! and never puts them in a conflict set. Publication, command replay and WAL
//! replay (crash recovery, PITR, replicas) all call this one function, so every
//! node derives identical rows from the same base log.
//!
//! The read functions below return exact answers at the reader's snapshot
//! (pinned view, transaction overlay or write transaction).

use std::collections::{BTreeMap, BTreeSet};

use rushdb_storage::{DeriveRead, Deriver, Op, StorageError};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{self as k, TENANT_PREFIX_LEN};
use rushdb_types::occurrence::{decode_occ_value, OccurrenceValue, RelationshipHeader};
use rushdb_types::stats::Histogram;

use crate::reader::{keys_ext, TryRead};

// ---------------------------------------------------------------------------
// Deriver
// ---------------------------------------------------------------------------

/// The LMPG apply-time deriver.
#[derive(Debug, Default, Clone, Copy)]
pub struct LmpgDeriver;

/// Base ops of one commit layered over the state before it.
struct After<'a> {
    ops: BTreeMap<&'a [u8], Option<&'a [u8]>>,
    before: &'a dyn DeriveRead,
}

impl<'a> After<'a> {
    fn new(ops: &'a [Op], before: &'a dyn DeriveRead) -> Self {
        let mut map = BTreeMap::new();
        for op in ops {
            match op {
                Op::Put(key, value) | Op::Set(key, value) => {
                    map.insert(key.as_slice(), Some(value.as_slice()));
                }
                Op::Del(key) | Op::Purge(key) => {
                    map.insert(key.as_slice(), None);
                }
            }
        }
        Self { ops: map, before }
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        match self.ops.get(key) {
            Some(value) => Ok(value.map(<[u8]>::to_vec)),
            None => self.before.get(key),
        }
    }

    fn range_prefix(&self, prefix: &[u8]) -> Result<Rows, StorageError> {
        let mut merged: BTreeMap<Vec<u8>, Vec<u8>> =
            self.before.range_prefix(prefix)?.into_iter().collect();
        for (key, value) in self.ops.range::<&[u8], _>(prefix..) {
            if !key.starts_with(prefix) {
                break;
            }
            match value {
                Some(value) => {
                    merged.insert(key.to_vec(), value.to_vec());
                }
                None => {
                    merged.remove(*key);
                }
            }
        }
        Ok(merged.into_iter().collect())
    }
}

/// `(key, value)` rows of a prefix scan.
type Rows = Vec<(Vec<u8>, Vec<u8>)>;

/// A point read against one side (before or after) of a commit.
type Lookup<'a> = dyn Fn(&[u8]) -> Result<Option<Vec<u8>>, StorageError> + 'a;

impl DeriveRead for After<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        After::get(self, key)
    }

    fn range_prefix(&self, prefix: &[u8]) -> Result<Rows, StorageError> {
        After::range_prefix(self, prefix)
    }

    fn range(&self, start: &[u8], end: &[u8]) -> Result<Rows, StorageError> {
        let mut merged: BTreeMap<Vec<u8>, Vec<u8>> =
            self.before.range(start, end)?.into_iter().collect();
        for (key, value) in &self.ops {
            if *key < start || (!end.is_empty() && *key >= end) {
                continue;
            }
            match value {
                Some(value) => {
                    merged.insert(key.to_vec(), value.to_vec());
                }
                None => {
                    merged.remove(*key);
                }
            }
        }
        Ok(merged.into_iter().collect())
    }
}

/// Owner presence state for one pid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Presence {
    Null,
    Value,
}

impl Presence {
    fn nullness(self) -> u8 {
        match self {
            Self::Null => k::NULLNESS_NULL,
            Self::Value => k::NULLNESS_VALUE,
        }
    }
}

/// Accumulated derived changes of one commit.
#[derive(Default)]
struct Changes {
    counters: BTreeMap<Vec<u8>, i64>,
    /// Refcounted value rows (dictionary entries and long-value digests),
    /// each with the distinct counter that tracks its non-zero rows.
    dict: BTreeMap<Vec<u8>, (Vec<u8>, i64)>,
    rows: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl Changes {
    fn bump(&mut self, key: Vec<u8>, delta: i64) {
        if delta != 0 {
            *self.counters.entry(key).or_default() += delta;
        }
    }
}

fn owner_label(
    view: &Lookup<'_>,
    tenant: TenantId,
    kind: OwnerKind,
    owner: u64,
) -> Result<Option<String>, StorageError> {
    match kind {
        OwnerKind::Record => Ok(match view(&k::record_hdr_key(tenant, owner))? {
            Some(raw) => Some(
                k::decode_record_hdr(&raw)
                    .ok_or(StorageError::CorruptLog)?
                    .1
                    .to_owned(),
            ),
            None => None,
        }),
        OwnerKind::Relationship => {
            let Some(id) = view(&keys_ext::rel_by_ord_key(tenant, owner))? else {
                return Ok(None);
            };
            let id: [u8; 16] = id.try_into().map_err(|_| StorageError::CorruptLog)?;
            let Some(row) = view(&keys_ext::rel_row_key(tenant, &id))? else {
                return Ok(None);
            };
            let header: RelationshipHeader =
                serde_json::from_slice(&row).map_err(|_| StorageError::CorruptLog)?;
            Ok(Some(header.rel_type))
        }
    }
}

/// `(type, source label, target label)` of a relationship owner, from its
/// creation-time rel-group row.
type RelShape = (String, String, String);

fn rel_shape(
    view: &Lookup<'_>,
    tenant: TenantId,
    owner: u64,
) -> Result<Option<RelShape>, StorageError> {
    let Some(id) = view(&keys_ext::rel_by_ord_key(tenant, owner))? else {
        return Ok(None);
    };
    let id: [u8; 16] = id.try_into().map_err(|_| StorageError::CorruptLog)?;
    let group_key = keys_ext::rel_group_key(tenant, &id);
    let Some(raw) = view(&group_key)? else {
        return Ok(None);
    };
    rel_group(view, tenant, &group_key, &raw)
}

/// Values of one owner's pid: `(presence, entries, nulls, arrays)`.
struct PidState {
    presence: Option<Presence>,
    entries: Vec<Vec<u8>>,
    nulls: i64,
    array: bool,
}

fn pid_state(rows: &[(Vec<u8>, Vec<u8>)]) -> Result<PidState, StorageError> {
    let mut state = PidState {
        presence: None,
        entries: Vec::new(),
        nulls: 0,
        array: false,
    };
    for (_, raw) in rows {
        let occ = decode_occ_value(raw).map_err(|_| StorageError::CorruptLog)?;
        state.array |= occ.array;
        match occ.value {
            OccurrenceValue::Stored { .. } => {
                state.presence = Some(Presence::Value);
                state.entries.push(occ.entry.to_vec());
            }
            OccurrenceValue::ExplicitNull => {
                state.nulls += 1;
                state.presence.get_or_insert(Presence::Null);
            }
            OccurrenceValue::EmptyArray => {
                state.presence.get_or_insert(Presence::Null);
            }
        }
    }
    Ok(state)
}

/// Relationship type and creation-time endpoint labels of a rel-group row.
fn rel_group(
    view: &Lookup<'_>,
    tenant: TenantId,
    group_key: &[u8],
    group_raw: &[u8],
) -> Result<Option<(String, String, String)>, StorageError> {
    let (source, target) = serde_json::from_slice::<(String, String)>(group_raw)
        .map_err(|_| StorageError::CorruptLog)?;
    let id: [u8; 16] = group_key
        .get(group_key.len() - 16..)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(StorageError::CorruptLog)?;
    let Some(row) = view(&keys_ext::rel_row_key(tenant, &id))? else {
        return Ok(None);
    };
    let header: RelationshipHeader =
        serde_json::from_slice(&row).map_err(|_| StorageError::CorruptLog)?;
    Ok(Some((header.rel_type, source, target)))
}

/// `(node, outgoing)` of an adjacency key `…/adj/<out|in>/<node8>/…`.
fn parse_adj(key: &[u8]) -> Option<(u64, bool)> {
    let rest = key.get(TENANT_PREFIX_LEN..)?.strip_prefix(b"/adj/")?;
    let (outgoing, rest) = if let Some(rest) = rest.strip_prefix(b"out/") {
        (true, rest)
    } else {
        (false, rest.strip_prefix(b"in/")?)
    };
    Some((
        u64::from_be_bytes(rest.get(..8)?.try_into().ok()?),
        outgoing,
    ))
}

fn is_rel_group(key: &[u8]) -> bool {
    key.get(TENANT_PREFIX_LEN..)
        .is_some_and(|rest| rest.starts_with(b"/rel-group/"))
}

/// Owner-pid pairs a commit touched, and records whose header changed.
#[derive(Default)]
struct Touched {
    pids: BTreeSet<(TenantId, OwnerKind, u64, u64)>,
    relabeled: BTreeSet<(TenantId, u64)>,
    index: crate::index::IndexWork,
}

/// Fold one commit's base ops into label, degree and relationship-catalog
/// counters, and collect the owner pids whose occurrence-derived rows must be
/// recomputed.
fn scan_base_ops(
    after: &After<'_>,
    before: &dyn DeriveRead,
    changes: &mut Changes,
) -> Result<Touched, StorageError> {
    let read_before = |key: &[u8]| before.get(key);
    let read_after = |key: &[u8]| after.get(key);
    let mut touched = Touched::default();
    for key in after.ops.keys() {
        let Some(tenant) = k::parse_tenant(key) else {
            continue;
        };
        match key.get(TENANT_PREFIX_LEN) {
            Some(&k::F_OCC) => {
                let (_, kind, owner, pid, _) =
                    k::parse_occ_key(key).ok_or(StorageError::CorruptLog)?;
                touched.pids.insert((tenant, kind, owner, pid));
                if kind == OwnerKind::Record {
                    touched.index.occurrences.insert((tenant, owner, pid));
                }
            }
            Some(&k::F_RECORD_HDR) => {
                let owner = k::parse_record_hdr_key(key).ok_or(StorageError::CorruptLog)?;
                touched.relabeled.insert((tenant, owner));
                touched.index.headers.insert((tenant, owner));
            }
            Some(&k::F_INDEX_DEF) => {
                let id = key
                    .get(TENANT_PREFIX_LEN + 1..)
                    .and_then(|id| <[u8; 4]>::try_from(id).ok())
                    .map(u32::from_be_bytes)
                    .ok_or(StorageError::CorruptLog)?;
                touched.index.definitions.insert((tenant, id));
            }
            Some(&k::F_LABEL) => {
                let (label, _) = k::parse_label_member_key(key).ok_or(StorageError::CorruptLog)?;
                let was = before.get(key)?.is_some();
                let is = after.get(key)?.is_some();
                changes.bump(
                    k::label_count_key(tenant, label),
                    i64::from(is) - i64::from(was),
                );
            }
            _ if parse_adj(key).is_some() => {
                let (node, outgoing) = parse_adj(key).ok_or(StorageError::CorruptLog)?;
                let was = before.get(key)?.is_some();
                let is = after.get(key)?.is_some();
                changes.bump(
                    k::degree_key(tenant, node, outgoing),
                    i64::from(is) - i64::from(was),
                );
            }
            _ if is_rel_group(key) => {
                if let Some(raw) = before.get(key)? {
                    if let Some((rel_type, source, target)) =
                        rel_group(&read_before, tenant, key, &raw)?
                    {
                        changes.bump(k::rel_catalog_key(tenant, &rel_type, &source, &target), -1);
                    }
                }
                if let Some(raw) = after.get(key)? {
                    if let Some((rel_type, source, target)) =
                        rel_group(&read_after, tenant, key, &raw)?
                    {
                        changes.bump(k::rel_catalog_key(tenant, &rel_type, &source, &target), 1);
                    }
                }
            }
            _ => {}
        }
    }
    // A record whose header changed (create, delete, retype) moves every pid
    // it carries between labels.
    for (tenant, owner) in &touched.relabeled {
        let prefix = k::occ_owner_prefix(*tenant, OwnerKind::Record, *owner);
        for (key, _) in before
            .range_prefix(&prefix)?
            .into_iter()
            .chain(after.range_prefix(&prefix)?)
        {
            let (_, kind, owner, pid, _) =
                k::parse_occ_key(&key).ok_or(StorageError::CorruptLog)?;
            touched.pids.insert((*tenant, kind, owner, pid));
        }
    }
    Ok(touched)
}

/// `(label before, label after)` of one owner across a commit.
type OwnerLabels = (Option<String>, Option<String>);

/// One owner's occurrence rows grouped by pid, `(before, after)` a commit.
type PidRows = BTreeMap<u64, Vec<(Vec<u8>, Vec<u8>)>>;
type OwnerOccurrences = (PidRows, PidRows);

/// Group one owner's occurrence rows (key order) by pid.
fn rows_by_pid(rows: Vec<(Vec<u8>, Vec<u8>)>) -> Result<PidRows, StorageError> {
    let mut grouped = PidRows::new();
    for (key, value) in rows {
        let (_, _, _, pid, _) = k::parse_occ_key(&key).ok_or(StorageError::CorruptLog)?;
        grouped.entry(pid).or_default().push((key, value));
    }
    Ok(grouped)
}

impl Deriver for LmpgDeriver {
    fn derive(
        &self,
        _ci: u64,
        ops: &[Op],
        before: &dyn DeriveRead,
    ) -> Result<Vec<Op>, StorageError> {
        // Purge is a destructive tenant wipe that also erases the derived rows
        // themselves; it derives nothing.
        if ops.iter().any(|op| matches!(op, Op::Purge(_))) {
            return Ok(Vec::new());
        }
        let after = After::new(ops, before);
        let read_before = |key: &[u8]| before.get(key);
        let read_after = |key: &[u8]| after.get(key);
        let mut changes = Changes::default();
        let touched = scan_base_ops(&after, before, &mut changes)?;
        let mut labels: BTreeMap<(TenantId, OwnerKind, u64), OwnerLabels> = BTreeMap::new();
        let mut shapes: BTreeMap<(TenantId, u64), (Option<RelShape>, Option<RelShape>)> =
            BTreeMap::new();
        // One owner-wide occurrence scan per view, split by pid in memory:
        // `touched.pids` is ordered by owner, and a create touches every pid
        // of its owner, so this replaces a before and an after scan per pid.
        let mut owner_rows: Option<((TenantId, OwnerKind, u64), OwnerOccurrences)> = None;
        for &(tenant, kind, owner, pid) in &touched.pids {
            let (label_before, label_after) =
                if let Some(labels) = labels.get(&(tenant, kind, owner)) {
                    labels.clone()
                } else {
                    let resolved = (
                        owner_label(&read_before, tenant, kind, owner)?,
                        owner_label(&read_after, tenant, kind, owner)?,
                    );
                    labels.insert((tenant, kind, owner), resolved.clone());
                    resolved
                };
            if owner_rows
                .as_ref()
                .is_none_or(|(at, _)| *at != (tenant, kind, owner))
            {
                let prefix = k::occ_owner_prefix(tenant, kind, owner);
                owner_rows = Some((
                    (tenant, kind, owner),
                    (
                        rows_by_pid(before.range_prefix(&prefix)?)?,
                        rows_by_pid(after.range_prefix(&prefix)?)?,
                    ),
                ));
            }
            let Some((_, (rows_before, rows_after))) = owner_rows.as_ref() else {
                unreachable!("owner rows were just loaded")
            };
            let none = Vec::new();
            let old = pid_state(rows_before.get(&pid).unwrap_or(&none))?;
            let new = pid_state(rows_after.get(&pid).unwrap_or(&none))?;
            derive_pid(
                &mut changes,
                tenant,
                kind,
                owner,
                pid,
                (label_before.as_deref(), &old),
                (label_after.as_deref(), &new),
            );
            if kind == OwnerKind::Relationship {
                let (shape_before, shape_after) = match shapes.entry((tenant, owner)) {
                    std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::btree_map::Entry::Vacant(entry) => entry.insert((
                        rel_shape(&read_before, tenant, owner)?,
                        rel_shape(&read_after, tenant, owner)?,
                    )),
                };
                derive_shape_support(
                    &mut changes,
                    tenant,
                    pid,
                    shape_before.as_ref().filter(|_| old.presence.is_some()),
                    shape_after.as_ref().filter(|_| new.presence.is_some()),
                );
            }
        }
        crate::index::derive(&touched.index, before, &after, &mut changes.rows)?;
        materialize(changes, before)
    }

    fn is_derived_key(&self, key: &[u8]) -> bool {
        k::parse_tenant(key).is_some() && is_derived_family(key.get(TENANT_PREFIX_LEN).copied())
    }

    fn prefix_reaches_derived(&self, prefix: &[u8]) -> bool {
        match prefix.first() {
            None => true,
            Some(&k::TENANT_NS) => {
                prefix.len() <= TENANT_PREFIX_LEN
                    || is_derived_family(prefix.get(TENANT_PREFIX_LEN).copied())
            }
            Some(_) => false,
        }
    }
}

fn is_derived_family(family: Option<u8>) -> bool {
    matches!(
        family,
        Some(k::F_COUNTER | k::F_VALUE_DICT | k::F_MARKER | k::F_PRESENCE | k::F_INDEX_ENTRY)
    )
}

fn derive_pid(
    changes: &mut Changes,
    tenant: TenantId,
    kind: OwnerKind,
    owner: u64,
    pid: u64,
    (label_before, old): (Option<&str>, &PidState),
    (label_after, new): (Option<&str>, &PidState),
) {
    // Null occurrences are label-independent.
    changes.bump(k::null_count_key(tenant, kind, pid), new.nulls - old.nulls);
    // Presence and owner support.
    let had = label_before.zip(old.presence);
    let has = label_after.zip(new.presence);
    if had != has {
        if let Some((label, presence)) = had {
            changes.rows.insert(
                k::presence_key(tenant, kind, label, pid, presence.nullness(), owner),
                None,
            );
            changes.bump(k::support_by_pid_key(tenant, kind, pid, label), -1);
            changes.bump(k::support_by_label_key(tenant, kind, label, pid), -1);
        }
        if let Some((label, presence)) = has {
            changes.rows.insert(
                k::presence_key(tenant, kind, label, pid, presence.nullness(), owner),
                Some(Vec::new()),
            );
            changes.bump(k::support_by_pid_key(tenant, kind, pid, label), 1);
            changes.bump(k::support_by_label_key(tenant, kind, label, pid), 1);
        }
        changes.bump(
            k::support_total_key(tenant, kind, pid),
            i64::from(has.is_some()) - i64::from(had.is_some()),
        );
    }
    // Value dictionary (short values); long values are refcounted by digest,
    // so their distinct count is exact without a scan.
    let mut entry_delta = |label: &str, entry: &[u8], delta: i64| {
        let (key, distinct) = if entry.len() > k::DICT_MAX_VALUE_BYTES {
            changes.bump(k::long_values_key(tenant, kind, label, pid), delta);
            let digest = k::long_value_digest(entry);
            (
                k::long_value_key(tenant, kind, label, pid, &digest),
                k::long_distinct_key(tenant, kind, label, pid),
            )
        } else {
            (
                k::value_dict_key(tenant, kind, label, pid, entry),
                k::distinct_key(tenant, kind, label, pid),
            )
        };
        changes.dict.entry(key).or_insert_with(|| (distinct, 0)).1 += delta;
    };
    if let Some(label) = label_before {
        for entry in &old.entries {
            entry_delta(label, entry, -1);
        }
    }
    if let Some(label) = label_after {
        for entry in &new.entries {
            entry_delta(label, entry, 1);
        }
    }
    // Array markers: an exact count of array-holding owners per label, so
    // the marker is a function of the current base rows.
    let was = label_before.filter(|_| old.array);
    let is = label_after.filter(|_| new.array);
    if was != is {
        if let Some(label) = was {
            changes.bump(k::array_label_marker_key(tenant, kind, pid, label), -1);
        }
        if let Some(label) = is {
            changes.bump(k::array_label_marker_key(tenant, kind, pid, label), 1);
        }
    }
}

/// Relationship property support per shape: a relationship counts
/// under its creation-time `(type, source, target)` for every pid it carries.
fn derive_shape_support(
    changes: &mut Changes,
    tenant: TenantId,
    pid: u64,
    had: Option<&RelShape>,
    has: Option<&RelShape>,
) {
    if had == has {
        return;
    }
    let key = |(rel_type, source, target): &RelShape| {
        k::rel_shape_prop_key(tenant, (rel_type, source, target), pid)
    };
    if let Some(shape) = had {
        changes.bump(key(shape), -1);
    }
    if let Some(shape) = has {
        changes.bump(key(shape), 1);
    }
}

fn apply_delta(current: u64, delta: i64) -> Result<u64, StorageError> {
    current
        .checked_add_signed(delta)
        .ok_or(StorageError::CorruptLog)
}

/// Fold this commit's label, relationship-catalog and record-support counter
/// changes into the tenant-wide usage totals (`C_USAGE`): records and
/// relationships are the sums of their catalogs, labels and properties the
/// number of those counters that are non-zero. A read of the totals is then
/// one `get` per metric, independent of the tenant's size.
fn derive_usage(changes: &mut Changes, before: &dyn DeriveRead) -> Result<(), StorageError> {
    let record_support = [OwnerKind::Record.as_byte()];
    let mut usage: Vec<(Vec<u8>, i64)> = Vec::new();
    for (key, &delta) in &changes.counters {
        if delta == 0 || key.get(TENANT_PREFIX_LEN) != Some(&k::F_COUNTER) {
            continue;
        }
        let Some(tenant) = k::parse_tenant(key) else {
            continue;
        };
        let sub = key.get(TENANT_PREFIX_LEN + 1).copied();
        let transition = |metric: u8, usage: &mut Vec<(Vec<u8>, i64)>| {
            let current =
                k::decode_counter(before.get(key)?.as_deref()).ok_or(StorageError::CorruptLog)?;
            let next = apply_delta(current, delta)?;
            let step = i64::from(next > 0) - i64::from(current > 0);
            if step != 0 {
                usage.push((k::usage_count_key(tenant, metric), step));
            }
            Ok::<_, StorageError>(())
        };
        match sub {
            Some(k::C_LABEL) => {
                usage.push((k::usage_count_key(tenant, k::U_RECORDS), delta));
                transition(k::U_LABELS, &mut usage)?;
            }
            Some(k::C_REL_CATALOG) => {
                usage.push((k::usage_count_key(tenant, k::U_RELATIONSHIPS), delta));
            }
            Some(k::C_SUPPORT_TOTAL)
                if key.get(TENANT_PREFIX_LEN + 2..TENANT_PREFIX_LEN + 3)
                    == Some(&record_support[..]) =>
            {
                transition(k::U_PROPERTIES, &mut usage)?;
            }
            _ => {}
        }
    }
    for (key, delta) in usage {
        changes.bump(key, delta);
    }
    Ok(())
}

fn materialize(mut changes: Changes, before: &dyn DeriveRead) -> Result<Vec<Op>, StorageError> {
    for (key, (distinct, delta)) in std::mem::take(&mut changes.dict) {
        if delta == 0 {
            continue;
        }
        let current =
            k::decode_counter(before.get(&key)?.as_deref()).ok_or(StorageError::CorruptLog)?;
        let next = apply_delta(current, delta)?;
        if current == 0 && next > 0 {
            changes.bump(distinct, 1);
        } else if current > 0 && next == 0 {
            changes.bump(distinct, -1);
        }
        changes
            .rows
            .insert(key, (next > 0).then(|| next.to_be_bytes().to_vec()));
    }
    derive_usage(&mut changes, before)?;
    let mut out = Vec::with_capacity(changes.counters.len() + changes.rows.len());
    for (key, delta) in changes.counters {
        if delta == 0 {
            continue;
        }
        let current =
            k::decode_counter(before.get(&key)?.as_deref()).ok_or(StorageError::CorruptLog)?;
        let next = apply_delta(current, delta)?;
        out.push(if next == 0 {
            Op::Del(key)
        } else {
            Op::Put(key, next.to_be_bytes().to_vec())
        });
    }
    for (key, value) in changes.rows {
        out.push(match value {
            Some(value) => Op::Put(key, value),
            None => Op::Del(key),
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Read API
// ---------------------------------------------------------------------------

fn counter<R: TryRead + ?Sized>(r: &R, key: &[u8]) -> Result<u64, StorageError> {
    k::decode_counter(r.get(key)?.as_deref()).ok_or(StorageError::CorruptLog)
}

/// Direction of an adjacency degree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdjDirection {
    /// Outgoing edges (the node is the source).
    Out,
    /// Incoming edges (the node is the target).
    In,
}

/// A tenant's usage totals at one snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TenantUsage {
    /// Live records.
    pub records: u64,
    /// Live relationships.
    pub relationships: u64,
    /// Labels with at least one live record.
    pub labels: u64,
    /// Property ids carried by at least one live record.
    pub properties: u64,
    /// Logical stored bytes: key + value lengths of the tenant's rows. The
    /// file pages holding them are larger (pages run partly full).
    pub logical_bytes: u64,
    /// Ready vectors per embedding space, in space-id order.
    pub vectors: Vec<SpaceVectors>,
}

/// Ready vectors of one embedding space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceVectors {
    /// Dense space id.
    pub space_id: u64,
    /// The space's index name.
    pub name: String,
    /// Bindings in `Ready` status.
    pub vectors: u64,
}

/// A tenant's usage totals: one `get` per metric plus one per embedding
/// space, never a scan of the tenant's data.
///
/// # Errors
/// Returns [`StorageError`] if a counter or space row cannot be read or is
/// malformed.
pub fn tenant_usage<R: TryRead + ?Sized>(r: &R, t: TenantId) -> Result<TenantUsage, StorageError> {
    let metric = |m: u8| counter(r, &k::usage_count_key(t, m));
    let mut vectors = Vec::new();
    for (_, raw) in r.range_prefix(&k::key(t, &[k::P_EMB_SPACE, b""]))? {
        let space: rushdb_types::occurrence::EmbeddingSpaceInfo =
            serde_json::from_slice(&raw).map_err(|_| StorageError::CorruptLog)?;
        vectors.push(SpaceVectors {
            vectors: counter(r, &k::emb_ready_count_key(t, space.space_id))?,
            space_id: space.space_id,
            name: space.name,
        });
    }
    Ok(TenantUsage {
        records: metric(k::U_RECORDS)?,
        relationships: metric(k::U_RELATIONSHIPS)?,
        labels: metric(k::U_LABELS)?,
        properties: metric(k::U_PROPERTIES)?,
        logical_bytes: counter(r, &k::usage_bytes_key(t))?,
        vectors,
    })
}

/// Live record count of a label.
///
/// # Errors
/// Returns [`StorageError`] if the counter cannot be read or is malformed.
pub fn label_count<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    label: &str,
) -> Result<u64, StorageError> {
    counter(r, &k::label_count_key(t, label))
}

/// Every live label with its count, in first-appearance order: by the
/// ordinal of the label's oldest live record (one bounded seek per label),
/// then name. The order is read from live rows, not stored, so it stays a
/// function of the base state.
///
/// # Errors
/// Returns [`StorageError`] if the counters cannot be read or are malformed.
pub fn label_catalog<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
) -> Result<Vec<(String, u64)>, StorageError> {
    let prefix = k::counter_prefix(t, k::C_LABEL);
    let mut rows = Vec::new();
    for (key, raw) in r.range_prefix(&prefix)? {
        let (label, _) = k::read_str(&key, prefix.len()).ok_or(StorageError::CorruptLog)?;
        let count = k::decode_counter(Some(&raw)).ok_or(StorageError::CorruptLog)?;
        let oldest = match r.first_key(&k::label_member_prefix(t, label))? {
            Some(member) => {
                k::parse_label_member_key(&member)
                    .ok_or(StorageError::CorruptLog)?
                    .1
            }
            None => u64::MAX,
        };
        rows.push((oldest, label.to_owned(), count));
    }
    rows.sort();
    Ok(rows
        .into_iter()
        .map(|(_, label, count)| (label, count))
        .collect())
}

/// Live adjacency degree of a node.
///
/// # Errors
/// Returns [`StorageError`] if the counter cannot be read or is malformed.
pub fn degree<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    direction: AdjDirection,
    node: u64,
) -> Result<u64, StorageError> {
    counter(r, &k::degree_key(t, node, direction == AdjDirection::Out))
}

/// Relationships of `(type, source label, target label)`.
///
/// # Errors
/// Returns [`StorageError`] if the counter cannot be read or is malformed.
pub fn rel_catalog_count<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    rel_type: &str,
    source: &str,
    target: &str,
) -> Result<u64, StorageError> {
    counter(r, &k::rel_catalog_key(t, rel_type, source, target))
}

/// Relationships of `(type, source label, target label)` carrying each pid:
/// `(pid, count)`.
///
/// # Errors
/// Returns [`StorageError`] if the counters cannot be read or are malformed.
pub fn rel_shape_prop_counts<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    rel_type: &str,
    source: &str,
    target: &str,
) -> Result<Vec<(u64, u64)>, StorageError> {
    let prefix = k::rel_shape_prop_prefix(t, rel_type, source, target);
    let mut out = Vec::new();
    for (key, raw) in r.range_prefix(&prefix)? {
        let pid = k::read_u64(&key, prefix.len()).ok_or(StorageError::CorruptLog)?;
        out.push((
            pid,
            k::decode_counter(Some(&raw)).ok_or(StorageError::CorruptLog)?,
        ));
    }
    Ok(out)
}

/// Every relationship catalog row `(type, source, target, count)`, sorted.
///
/// # Errors
/// Returns [`StorageError`] if the counters cannot be read or are malformed.
pub fn rel_catalog<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
) -> Result<Vec<(String, String, String, u64)>, StorageError> {
    let prefix = k::counter_prefix(t, k::C_REL_CATALOG);
    let mut out = Vec::new();
    for (key, raw) in r.range_prefix(&prefix)? {
        let (rel_type, pos) = k::read_str(&key, prefix.len()).ok_or(StorageError::CorruptLog)?;
        let (source, pos) = k::read_str(&key, pos).ok_or(StorageError::CorruptLog)?;
        let (target, _) = k::read_str(&key, pos).ok_or(StorageError::CorruptLog)?;
        let count = k::decode_counter(Some(&raw)).ok_or(StorageError::CorruptLog)?;
        out.push((
            rel_type.to_owned(),
            source.to_owned(),
            target.to_owned(),
            count,
        ));
    }
    out.sort();
    Ok(out)
}

/// Owners of `kind` carrying `pid` (all labels).
///
/// # Errors
/// Returns [`StorageError`] if the counter cannot be read or is malformed.
pub fn prop_support_total<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
) -> Result<u64, StorageError> {
    counter(r, &k::support_total_key(t, kind, pid))
}

/// Owners of `kind` carrying `pid` under each label, by label name.
///
/// # Errors
/// Returns [`StorageError`] if the counters cannot be read or are malformed.
pub fn prop_support_by_label<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
) -> Result<Vec<(String, u64)>, StorageError> {
    let prefix = k::support_by_pid_prefix(t, kind, pid);
    let mut out = Vec::new();
    for (key, raw) in r.range_prefix(&prefix)? {
        let (label, _) = k::read_str(&key, prefix.len()).ok_or(StorageError::CorruptLog)?;
        out.push((
            label.to_owned(),
            k::decode_counter(Some(&raw)).ok_or(StorageError::CorruptLog)?,
        ));
    }
    out.sort();
    Ok(out)
}

/// Owners of `kind` under `label` carrying `pid`.
///
/// # Errors
/// Returns [`StorageError`] if the counter cannot be read or is malformed.
pub fn prop_support_label<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
    label: &str,
) -> Result<u64, StorageError> {
    counter(r, &k::support_by_pid_key(t, kind, pid, label))
}

/// Per-pid owner support of one label (or relationship type): `(pid, count)`.
///
/// # Errors
/// Returns [`StorageError`] if the counters cannot be read or are malformed.
pub fn prop_support_of_label<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    label: &str,
) -> Result<Vec<(u64, u64)>, StorageError> {
    let prefix = k::support_by_label_prefix(t, kind, label);
    let mut out = Vec::new();
    for (key, raw) in r.range_prefix(&prefix)? {
        let pid = k::read_u64(&key, prefix.len()).ok_or(StorageError::CorruptLog)?;
        out.push((
            pid,
            k::decode_counter(Some(&raw)).ok_or(StorageError::CorruptLog)?,
        ));
    }
    Ok(out)
}

/// Explicit-null occurrences of `(kind, pid)`.
///
/// # Errors
/// Returns [`StorageError`] if the counter cannot be read or is malformed.
pub fn null_count<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
) -> Result<u64, StorageError> {
    counter(r, &k::null_count_key(t, kind, pid))
}

/// `(pid holds arrays, labels whose owners hold arrays)`.
///
/// # Errors
/// Returns [`StorageError`] if the markers cannot be read or are malformed.
pub fn array_markers<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
) -> Result<(bool, Vec<String>), StorageError> {
    let prefix = k::array_label_marker_prefix(t, kind, pid);
    let mut labels = Vec::new();
    for (key, _) in r.range_prefix(&prefix)? {
        let (label, _) = k::read_str(&key, prefix.len()).ok_or(StorageError::CorruptLog)?;
        labels.push(label.to_owned());
    }
    Ok((!labels.is_empty(), labels))
}

/// Owners of `(kind, label, pid)` whose value is exactly `entry`
/// (`tag || order-preserving payload`); exact for dictionary-sized values.
///
/// # Errors
/// Returns [`StorageError`] if the dictionary row cannot be read.
pub fn value_count<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
    entry: &[u8],
) -> Result<u64, StorageError> {
    counter(r, &k::value_dict_key(t, kind, label, pid, entry))
}

/// Labels whose owners of `kind` carry `pid`.
///
/// # Errors
/// Returns [`StorageError`] if the counters cannot be read or are malformed.
pub fn labels_of_pid<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
) -> Result<Vec<String>, StorageError> {
    Ok(prop_support_by_label(r, t, kind, pid)?
        .into_iter()
        .map(|(label, _)| label)
        .collect())
}

/// The labels to read `pid`'s per-label rows under: `labels`, or every label
/// whose owners carry it when empty.
fn scope_labels<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
    labels: &[String],
) -> Result<Vec<String>, StorageError> {
    if labels.is_empty() {
        labels_of_pid(r, t, kind, pid)
    } else {
        Ok(labels.to_vec())
    }
}

/// Dictionary entries `(tag || payload, refcount)` of `(kind, pid)` under
/// `labels` (all labels when empty), merged (exact for dictionary-sized
/// values).
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is malformed.
pub fn dictionary<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
    labels: &[String],
) -> Result<BTreeMap<Vec<u8>, u64>, StorageError> {
    let mut merged: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
    for_each_dictionary_entry(r, t, kind, pid, labels, &mut |entry, count| {
        *merged.entry(entry.to_vec()).or_default() += count;
        true
    })?;
    Ok(merged)
}

/// Streams the dictionary entries `(tag || payload, refcount)` of
/// `(kind, pid)` under `labels` (all labels when empty) to `visit`, one
/// label after another and without collecting them, until `visit` returns
/// `false`. An entry held under several labels is visited once per label,
/// so a caller merging them sums the counts, as [`dictionary`] does.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is malformed.
pub fn for_each_dictionary_entry<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
    labels: &[String],
    visit: &mut dyn FnMut(&[u8], u64) -> bool,
) -> Result<(), StorageError> {
    let mut corrupt = false;
    let mut stopped = false;
    for label in scope_labels(r, t, kind, pid, labels)? {
        let prefix = k::value_dict_prefix(t, kind, &label, pid);
        let (start, end) = rushdb_storage::prefix_range(&prefix);
        r.for_each_row(&start, &end, &mut |key, raw| {
            let Some(count) = k::decode_counter(Some(raw)) else {
                corrupt = true;
                return false;
            };
            stopped = !visit(&key[prefix.len()..], count);
            !stopped
        })?;
        if corrupt {
            return Err(StorageError::CorruptLog);
        }
        if stopped {
            break;
        }
    }
    Ok(())
}

/// The largest key starting with `prefix`, or `None`, with forward seeks
/// only (storage has no reverse cursor): byte by byte, a binary search for
/// the highest byte a key under the known head continues with. About nine
/// seeks per byte after `prefix`.
///
/// # Errors
/// Returns [`StorageError`] if a seek fails.
pub fn last_key<R: TryRead + ?Sized>(
    r: &R,
    prefix: &[u8],
) -> Result<Option<Vec<u8>>, StorageError> {
    if r.first_key(prefix)?.is_none() {
        return Ok(None);
    }
    // Invariant: some key starts with `head`.
    let mut head = prefix.to_vec();
    loop {
        let (_, end) = rushdb_storage::prefix_range(&head);
        let at_least = |byte: u8| -> Result<bool, StorageError> {
            let mut start = head.clone();
            start.push(byte);
            Ok(r.first_key_from(&start, &end)?.is_some())
        };
        // No longer key under `head`: `head` is the key.
        if !at_least(0)? {
            return Ok(Some(head));
        }
        let (mut low, mut high) = (0u8, 0xFF_u8);
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            if at_least(mid)? {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        head.push(low);
    }
}

/// The first and last entry of a value dictionary, `(min, max)`.
pub type EntryBounds = (Vec<u8>, Vec<u8>);

/// The smallest and largest value-dictionary entries of `(kind, pid)` under
/// one `label`: the property's minimum and maximum in the order-preserving
/// encoding, read with a few dozen seeks instead of walking the dictionary.
///
/// # Errors
/// Returns [`StorageError`] if a seek fails.
pub fn dictionary_bounds<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
) -> Result<Option<EntryBounds>, StorageError> {
    let prefix = k::value_dict_prefix(t, kind, label, pid);
    let (Some(first), Some(last)) = (r.first_key(&prefix)?, last_key(r, &prefix)?) else {
        return Ok(None);
    };
    Ok(Some((
        first[prefix.len()..].to_vec(),
        last[prefix.len()..].to_vec(),
    )))
}

/// Occurrences of `(kind, pid)` under `labels` (all labels when empty) too
/// long for the value dictionary (their values are only reachable by
/// scanning occurrences).
///
/// # Errors
/// Returns [`StorageError`] if a counter cannot be read or is malformed.
pub fn long_values<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
    labels: &[String],
) -> Result<u64, StorageError> {
    let mut total = 0u64;
    for label in scope_labels(r, t, kind, pid, labels)? {
        total = total.saturating_add(counter(r, &k::long_values_key(t, kind, &label, pid))?);
    }
    Ok(total)
}

/// Whether an owner of `(kind, pid)` under `labels` (all labels when empty)
/// holds an explicit null: the null counter, or one presence row per label.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a counter is malformed.
pub fn has_nulls<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
    labels: &[String],
) -> Result<bool, StorageError> {
    if labels.is_empty() {
        return Ok(null_count(r, t, kind, pid)? > 0);
    }
    for label in labels {
        let prefix = k::presence_nullness_prefix(t, kind, label, pid, k::NULLNESS_NULL);
        if r.first_key(&prefix)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Exact distinct-value count of `(kind, pid)` across labels. Values too
/// long for the dictionary are counted by their digest refcounts (never
/// equal to a dictionary entry: they are longer than any).
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is malformed.
pub fn distinct_count<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
) -> Result<u64, StorageError> {
    let labels = labels_of_pid(r, t, kind, pid)?;
    let long = long_distinct(r, t, kind, pid, &labels)?;
    if labels.len() == 1 {
        return Ok(counter(r, &k::distinct_key(t, kind, &labels[0], pid))? + long);
    }
    let values = dictionary(r, t, kind, pid, &[])?.len();
    Ok(u64::try_from(values).map_err(|_| StorageError::CorruptLog)? + long)
}

/// Exact distinct-value count of `(kind, label, pid)`: dictionary values
/// plus long values, two counters.
///
/// # Errors
/// Returns [`StorageError`] if a counter cannot be read or is malformed.
pub fn label_distinct<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
) -> Result<u64, StorageError> {
    Ok(counter(r, &k::distinct_key(t, kind, label, pid))?
        + counter(r, &k::long_distinct_key(t, kind, label, pid))?)
}

/// Exact distinct count of the values of `(kind, pid)` too long for the
/// dictionary, under `labels` (all labels when empty): one counter for one
/// label, else the union of the labels' digests.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is malformed.
pub fn long_distinct<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    pid: u64,
    labels: &[String],
) -> Result<u64, StorageError> {
    let labels = scope_labels(r, t, kind, pid, labels)?;
    if let [label] = labels.as_slice() {
        return counter(r, &k::long_distinct_key(t, kind, label, pid));
    }
    let mut digests: BTreeSet<Vec<u8>> = BTreeSet::new();
    for label in &labels {
        if counter(r, &k::long_distinct_key(t, kind, label, pid))? == 0 {
            continue;
        }
        let prefix = k::long_value_prefix(t, kind, label, pid);
        for (key, _) in r.range_prefix(&prefix)? {
            digests.insert(key[prefix.len()..].to_vec());
        }
    }
    u64::try_from(digests.len()).map_err(|_| StorageError::CorruptLog)
}

/// The long values of `(kind, label, pid)` as `(digest, occurrences)`, in
/// digest order.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is malformed.
pub fn long_value_counts<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
) -> Result<Vec<([u8; k::LONG_DIGEST_BYTES], u64)>, StorageError> {
    let prefix = k::long_value_prefix(t, kind, label, pid);
    r.range_prefix(&prefix)?
        .into_iter()
        .map(|(key, raw)| {
            let digest = key[prefix.len()..]
                .try_into()
                .map_err(|_| StorageError::CorruptLog)?;
            let count = k::decode_counter(Some(&raw)).ok_or(StorageError::CorruptLog)?;
            Ok((digest, count))
        })
        .collect()
}

/// How much of the value dictionary one distribution walk may read.
#[derive(Clone, Copy, Debug)]
pub struct WalkBudget {
    /// Dictionary rows read, at most.
    pub keys: usize,
    /// Wall-clock time, at most (checked between label batches).
    pub time: std::time::Duration,
}

impl Default for WalkBudget {
    fn default() -> Self {
        Self {
            keys: 100_000,
            time: std::time::Duration::from_millis(20),
        }
    }
}

/// Value distribution of `(kind, pid)` over non-string values (the
/// range-comparable domain) under `label`, or across every label carrying the
/// pid when `label` is `None`; `None` when nothing range-comparable is held.
/// The walk reads at most `budget`; when it stops early the histogram covers
/// the smallest values walked and is marked `partial`.
///
/// # Errors
/// Returns [`StorageError`] if the dictionary cannot be read or is malformed.
pub fn value_distribution<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    label: Option<&str>,
    pid: u64,
    budget: WalkBudget,
) -> Result<Option<Histogram>, StorageError> {
    let started = std::time::Instant::now();
    let utf8 = rushdb_types::encoding::logical_tag(rushdb_types::model::LogicalType::Utf8);
    let labels = match label {
        Some(label) => vec![label.to_owned()],
        None => labels_of_pid(r, t, kind, pid)?,
    };
    // Merge across labels so a value held under two labels is one distinct.
    let mut merged: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
    let mut remaining = budget.keys;
    let mut partial = false;
    for label in &labels {
        if remaining == 0 || started.elapsed() >= budget.time {
            partial = true;
            break;
        }
        let prefix = k::value_dict_prefix(t, kind, label, pid);
        let rows = r.range_prefix_limited(&prefix, remaining)?;
        if rows.len() == remaining {
            // The label may hold more than the budget allowed.
            partial = true;
        }
        remaining -= rows.len();
        for (key, raw) in rows {
            let entry = key.get(prefix.len()..).ok_or(StorageError::CorruptLog)?;
            let count = k::decode_counter(Some(&raw)).ok_or(StorageError::CorruptLog)?;
            *merged.entry(entry.to_vec()).or_default() += count;
        }
    }
    let mut histogram = Histogram {
        partial,
        ..Histogram::default()
    };
    for (entry, count) in merged {
        let Some((&tag, payload)) = entry.split_first() else {
            return Err(StorageError::CorruptLog);
        };
        if tag != utf8 {
            histogram.add(payload, count);
        }
    }
    Ok((histogram.distinct > 0).then_some(histogram))
}

/// Owners of `kind` under `label` that carry `pid` with the given nullness, in
/// owner order.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a key is malformed.
pub fn presence_owners<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
    nullness: u8,
) -> Result<Vec<u64>, StorageError> {
    let prefix = k::presence_nullness_prefix(t, kind, label, pid, nullness);
    r.range_prefix(&prefix)?
        .into_iter()
        .map(|(key, _)| k::read_ord_tail(&key, prefix.len()).ok_or(StorageError::CorruptLog))
        .collect()
}
