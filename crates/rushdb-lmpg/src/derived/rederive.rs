//! Re-deriving one label's record rows from its base rows (SELF-HEAL).
//!
//! A `RederiveLabel` commit deletes the label's (never stored) marker key;
//! the deriver then recomputes, from the base rows alone, every row the
//! label's records derive: its record count, presence rows, per-label and
//! per-property support, value dictionary and distinct counts, long values,
//! array markers and ordered-index entries. Each is compared with the stored
//! row, and the difference goes into the commit's [`Changes`] as the deltas
//! and rows an ordinary commit would produce, so the totals built on them
//! (tenant usage, distinct counts, property totals) move consistently.
//!
//! Null counts are kept per property across labels, so the label's
//! properties' null counts are recounted over every record of the tenant.
//! Every node that applies the commit computes the same rows from the same
//! base rows.

use std::collections::{BTreeMap, BTreeSet};

use rushdb_storage::{DeriveRead, StorageError};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys as k;

use super::{owner_label, pid_state, rows_by_pid, Changes};

/// A counter row's value in `view` (absent is zero).
fn stored(view: &dyn DeriveRead, key: &[u8]) -> Result<i64, StorageError> {
    let value = k::decode_counter(view.get(key)?.as_deref()).ok_or(StorageError::CorruptLog)?;
    i64::try_from(value).map_err(|_| StorageError::CorruptLog)
}

/// One refcounted value row of the label: its distinct counter, the count
/// its base rows imply and the count stored.
struct Refcount {
    distinct: Vec<u8>,
    expected: i64,
    stored: i64,
}

/// What `label`'s records' base rows imply, per property.
#[derive(Default)]
struct Expected {
    presence: BTreeSet<Vec<u8>>,
    support: BTreeMap<u64, i64>,
    markers: BTreeMap<u64, i64>,
    long_values: BTreeMap<u64, i64>,
    values: BTreeMap<Vec<u8>, Refcount>,
}

/// Recompute `label`'s record rows in `view` (the state the re-derive
/// commit applies to) into `changes`.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is corrupt.
#[expect(
    clippy::too_many_lines,
    reason = "one pass over one label: its base rows, then each derived family compared in turn"
)]
pub(super) fn rederive_label(
    view: &dyn DeriveRead,
    t: TenantId,
    label: &str,
    changes: &mut Changes,
) -> Result<(), StorageError> {
    let kind = OwnerKind::Record;
    let members: Vec<u64> = view
        .range_prefix(&k::label_member_prefix(t, label))?
        .into_iter()
        .map(|(key, _)| {
            k::parse_label_member_key(&key)
                .map(|(_, owner)| owner)
                .ok_or(StorageError::CorruptLog)
        })
        .collect::<Result<_, _>>()?;
    let count_key = k::label_count_key(t, label);
    let members_count = i64::try_from(members.len()).map_err(|_| StorageError::CorruptLog)?;
    changes.bump(count_key.clone(), members_count - stored(view, &count_key)?);

    let mut expected = Expected::default();
    let lookup = |key: &[u8]| view.get(key);
    for &owner in &members {
        // An owner derives under the label its header names.
        if owner_label(&lookup, t, kind, owner)?.as_deref() != Some(label) {
            continue;
        }
        let rows = view.range_prefix(&k::occ_owner_prefix(t, kind, owner))?;
        for (pid, rows) in rows_by_pid(rows)? {
            let state = pid_state(&rows)?;
            if let Some(presence) = state.presence {
                expected.presence.insert(k::presence_key(
                    t,
                    kind,
                    label,
                    pid,
                    presence.nullness(),
                    owner,
                ));
                *expected.support.entry(pid).or_default() += 1;
            }
            if state.array {
                *expected.markers.entry(pid).or_default() += 1;
            }
            for entry in &state.entries {
                let (key, distinct) = value_keys(t, kind, label, pid, entry);
                if entry.len() > k::DICT_MAX_VALUE_BYTES {
                    *expected.long_values.entry(pid).or_default() += 1;
                }
                expected
                    .values
                    .entry(key)
                    .or_insert(Refcount {
                        distinct,
                        expected: 0,
                        stored: 0,
                    })
                    .expected += 1;
            }
        }
    }

    // Presence rows: present exactly when the base rows imply them.
    let stored_presence: BTreeSet<Vec<u8>> = view
        .range_prefix(&k::presence_label_prefix(t, kind, label))?
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    for key in stored_presence.difference(&expected.presence) {
        changes.rows.insert(key.clone(), None);
    }
    for key in expected.presence.difference(&stored_presence) {
        changes.rows.insert(key.clone(), Some(Vec::new()));
    }

    // Refcounted values as stored, with every property they belong to.
    let mut pids: BTreeSet<u64> = expected
        .support
        .keys()
        .chain(expected.markers.keys())
        .chain(expected.long_values.keys())
        .copied()
        .collect();
    let dict_prefix = k::value_dict_label_prefix(t, kind, label);
    for (key, value) in view.range_prefix(&dict_prefix)? {
        let (pid, _) =
            k::parse_value_dict_key(&key, &dict_prefix).ok_or(StorageError::CorruptLog)?;
        pids.insert(pid);
        stored_value(
            &mut expected,
            key,
            &value,
            k::distinct_key(t, kind, label, pid),
        )?;
    }
    let long_prefix = k::long_value_label_prefix(t, kind, label);
    for (key, value) in view.range_prefix(&long_prefix)? {
        let pid = k::parse_long_value_pid(&key, &long_prefix).ok_or(StorageError::CorruptLog)?;
        pids.insert(pid);
        stored_value(
            &mut expected,
            key,
            &value,
            k::long_distinct_key(t, kind, label, pid),
        )?;
    }
    let support_prefix = k::support_by_label_prefix(t, kind, label);
    for (key, _) in view.range_prefix(&support_prefix)? {
        let pid = key
            .strip_prefix(support_prefix.as_slice())
            .and_then(|tail| <[u8; 8]>::try_from(tail).ok())
            .map(u64::from_be_bytes)
            .ok_or(StorageError::CorruptLog)?;
        pids.insert(pid);
    }

    // Counters per property: each moved to what the base rows imply.
    for &pid in &pids {
        let support = expected.support.get(&pid).copied().unwrap_or(0);
        let by_label = k::support_by_label_key(t, kind, label, pid);
        changes.bump(by_label.clone(), support - stored(view, &by_label)?);
        // The property total is the sum of its labels' supports.
        let by_pid = k::support_by_pid_key(t, kind, pid, label);
        let moved = support - stored(view, &by_pid)?;
        changes.bump(by_pid, moved);
        changes.bump(k::support_total_key(t, kind, pid), moved);
        let marker = k::array_label_marker_key(t, kind, pid, label);
        let markers = expected.markers.get(&pid).copied().unwrap_or(0);
        changes.bump(marker.clone(), markers - stored(view, &marker)?);
        let long = k::long_values_key(t, kind, label, pid);
        let longs = expected.long_values.get(&pid).copied().unwrap_or(0);
        changes.bump(long.clone(), longs - stored(view, &long)?);
    }

    // Null counts of the label's properties, over every record that holds
    // them: an explicit null occurrence counts once, as the deriver counts it.
    let mut nulls: BTreeMap<u64, i64> = pids.iter().map(|&pid| (pid, 0)).collect();
    for (key, _) in view.range_prefix(&k::record_hdr_prefix(t))? {
        let owner = k::parse_record_hdr_key(&key).ok_or(StorageError::CorruptLog)?;
        for (pid, rows) in rows_by_pid(view.range_prefix(&k::occ_owner_prefix(t, kind, owner))?)? {
            if let Some(count) = nulls.get_mut(&pid) {
                *count += pid_state(&rows)?.nulls;
            }
        }
    }
    for (pid, expected) in nulls {
        let key = k::null_count_key(t, kind, pid);
        changes.bump(key.clone(), expected - stored(view, &key)?);
    }

    // Values: refcount deltas (materializing them moves each distinct
    // counter by the rows that appear or vanish), then whatever the stored
    // distinct counter is off by beyond that.
    let mut distinct: BTreeMap<Vec<u8>, (i64, i64)> = BTreeMap::new();
    for (key, value) in expected.values {
        let (rows, moves) = distinct.entry(value.distinct.clone()).or_default();
        *rows += i64::from(value.expected > 0);
        *moves += i64::from(value.expected > 0) - i64::from(value.stored > 0);
        let delta = value.expected - value.stored;
        if delta != 0 {
            changes.dict.insert(key, (value.distinct, delta));
        }
    }
    for (key, (rows, moves)) in distinct {
        let off = rows - stored(view, &key)? - moves;
        changes.bump(key, off);
    }
    Ok(())
}

/// The refcount row of `entry` and its distinct counter, as the deriver
/// keys them: short values in the dictionary, long ones by digest.
fn value_keys(
    t: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
    entry: &[u8],
) -> (Vec<u8>, Vec<u8>) {
    if entry.len() > k::DICT_MAX_VALUE_BYTES {
        (
            k::long_value_key(t, kind, label, pid, &k::long_value_digest(entry)),
            k::long_distinct_key(t, kind, label, pid),
        )
    } else {
        (
            k::value_dict_key(t, kind, label, pid, entry),
            k::distinct_key(t, kind, label, pid),
        )
    }
}

fn stored_value(
    expected: &mut Expected,
    key: Vec<u8>,
    raw: &[u8],
    distinct: Vec<u8>,
) -> Result<(), StorageError> {
    let count = k::decode_counter(Some(raw)).ok_or(StorageError::CorruptLog)?;
    expected
        .values
        .entry(key)
        .or_insert(Refcount {
            distinct,
            expected: 0,
            stored: 0,
        })
        .stored = i64::try_from(count).map_err(|_| StorageError::CorruptLog)?;
    Ok(())
}
