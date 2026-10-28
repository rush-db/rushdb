//! Cross-store invariant scrubber over storage format v3 (DC11/DC12).
//!
//! The derived rows of a tenant (presence, value dictionary, counters, array
//! markers) must equal what [`LmpgDeriver`] derives **from scratch** from the
//! tenant's base rows — the same golden invariant the layout tests assert at
//! every commit. Base rows are cross-checked too: every occurrence has a live
//! owner and a known property, label membership matches record headers, the
//! public-id index matches its header, and adjacency is symmetric. An empty
//! result doubles as an invariant proof after any workload.

use std::collections::{BTreeMap, BTreeSet};

use rushdb_storage::{DeriveRead, Deriver, Op, StorageError};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{self as k, TENANT_PREFIX_LEN};
use rushdb_types::occurrence::decode_occ_value;

use crate::derived::LmpgDeriver;
use crate::reader::{keys_ext as kx, try_access, TryRead};

/// One invariant violation found by [`scrub`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Machine-readable violation category.
    pub what: String,
    /// Human-readable context for the violation.
    pub detail: String,
}

/// Coverage and completion metadata for one scrub pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrubReport {
    /// Invariant violations found during the pass.
    pub violations: Vec<Violation>,
    /// Number of occurrence rows inspected.
    pub occurrence_rows: usize,
    /// Number of record header rows inspected.
    pub record_rows: usize,
    /// Number of derived rows compared against the from-scratch derivation.
    pub derived_rows: usize,
    /// True only after every requested range and point read completed.
    pub complete: bool,
}

/// Read-only candidate list for a bounded orphan repair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanRepairPlan {
    /// Orphaned occurrence keys that a repair commit would remove.
    pub keys: Vec<Vec<u8>>,
    /// Whether the bounded scan reached the end of the occurrence range.
    pub complete: bool,
}

/// The empty prior state a from-scratch derivation starts from.
struct Nothing;

impl DeriveRead for Nothing {
    fn get(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(None)
    }

    fn range_prefix(&self, _prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        Ok(Vec::new())
    }

    fn range(&self, _start: &[u8], _end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        Ok(Vec::new())
    }
}

/// Full-scan consistency check of one tenant at the attached read view.
///
/// # Errors
///
/// Returns the underlying storage error when a range or point read cannot be
/// completed. A corrupt row that can be read is reported as a violation.
pub fn scrub<R: TryRead>(r: &R, t: TenantId) -> Result<Vec<Violation>, StorageError> {
    Ok(scrub_report(r, t)?.violations)
}

/// Derived rows a tenant's base rows imply, derived from scratch.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a base row is corrupt.
pub fn derive_from_scratch<R: TryRead>(
    r: &R,
    t: TenantId,
) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, StorageError> {
    let deriver = LmpgDeriver;
    let base: Vec<Op> = r
        .range_prefix(&k::tenant_prefix(t))?
        .into_iter()
        .filter(|(key, _)| !deriver.is_derived_key(key))
        .map(|(key, value)| Op::Put(key, value))
        .collect();
    let mut out = BTreeMap::new();
    for op in deriver.derive(0, &base, &Nothing)? {
        match op {
            Op::Put(key, value) | Op::Set(key, value) => {
                out.insert(key, value);
            }
            Op::Del(key) | Op::Purge(key) => {
                out.remove(&key);
            }
        }
    }
    Ok(out)
}

/// Run a full scrub and return both violations and explicit coverage.
///
/// # Errors
///
/// Returns the underlying storage error when any scan or point read fails.
pub fn scrub_report<R: TryRead>(r: &R, t: TenantId) -> Result<ScrubReport, StorageError> {
    let rows = r.range_prefix(&k::tenant_prefix(t))?;
    let (mut violations, derived_rows) = derived_violations(r, t, &rows)?;
    let mut flag = |what: &str, detail: String| {
        violations.push(Violation {
            what: what.into(),
            detail,
        });
    };
    let mut occurrence_rows = 0usize;
    let mut record_rows = 0usize;
    for (key, value) in &rows {
        match key.get(TENANT_PREFIX_LEN) {
            Some(&k::F_OCC) => {
                occurrence_rows += 1;
                let Some((_, kind, owner, pid, _)) = k::parse_occ_key(key) else {
                    flag("occurrence_key_malformed", format!("{key:?}"));
                    continue;
                };
                if decode_occ_value(value).is_err() {
                    flag("occurrence_value_malformed", format!("{key:?}"));
                }
                if !owner_exists(r, t, kind, owner)? {
                    flag("occurrence_orphaned", format!("{kind:?}/{owner}/{pid}"));
                }
                if try_access::definition(r, t, pid)?.is_none() {
                    flag(
                        "occurrence_unknown_property",
                        format!("{kind:?}/{owner}/{pid}"),
                    );
                }
            }
            Some(&k::F_RECORD_HDR) => {
                record_rows += 1;
                let ordinal = k::parse_record_hdr_key(key).unwrap_or(u64::MAX);
                let Some((id, label)) = k::decode_record_hdr(value) else {
                    flag("record_header_malformed", format!("{ordinal}"));
                    continue;
                };
                if r.get(&k::label_member_key(t, label, ordinal))?.is_none() {
                    flag("label_membership_missing", format!("{label}/{ordinal}"));
                }
                let indexed = r.get(&k::record_id_key(t, &id))?;
                if indexed.as_deref() != Some(&ordinal.to_be_bytes()[..]) {
                    flag("record_id_index_mismatch", format!("{ordinal}"));
                }
            }
            Some(&k::F_RECORD_ID) => {
                // Each identity row must point at a header carrying that id;
                // a surplus row would resolve a second id to one record.
                let id: Option<[u8; 16]> = key
                    .get(TENANT_PREFIX_LEN + 1..)
                    .and_then(|id| id.try_into().ok());
                let ordinal = <[u8; 8]>::try_from(value.as_slice())
                    .ok()
                    .map(u64::from_be_bytes);
                let consistent = match (id, ordinal) {
                    (Some(id), Some(ordinal)) => try_access::record_by_ord(r, t, ordinal)?
                        .is_some_and(|header| header.record_id == id),
                    _ => false,
                };
                if !consistent {
                    flag("record_id_index_orphaned", k::describe_key(key));
                }
            }
            Some(&k::F_LABEL) => {
                let Some((label, ordinal)) = k::parse_label_member_key(key) else {
                    flag("label_membership_malformed", format!("{key:?}"));
                    continue;
                };
                match try_access::record_by_ord(r, t, ordinal)? {
                    Some(header) if header.label == label => {}
                    _ => flag("label_membership_stale", format!("{label}/{ordinal}")),
                }
            }
            _ => {}
        }
    }
    for violation in adjacency_violations(r, t)? {
        violations.push(violation);
    }
    Ok(ScrubReport {
        violations,
        occurrence_rows,
        record_rows,
        derived_rows,
        complete: true,
    })
}

/// Stored derived rows compared with [`derive_from_scratch`]: violations,
/// and the number of stored derived rows.
fn derived_violations<R: TryRead>(
    r: &R,
    t: TenantId,
    rows: &[(Vec<u8>, Vec<u8>)],
) -> Result<(Vec<Violation>, usize), StorageError> {
    let deriver = LmpgDeriver;
    let mut violations = Vec::new();
    let mut flag = |what: &str, detail: String| {
        violations.push(Violation {
            what: what.into(),
            detail,
        });
    };
    let stored: BTreeMap<&[u8], &[u8]> = rows
        .iter()
        .filter(|(key, _)| deriver.is_derived_key(key))
        .map(|(key, value)| (key.as_slice(), value.as_slice()))
        .collect();
    let expected = derive_from_scratch(r, t)?;
    for (key, value) in &expected {
        match stored.get(key.as_slice()) {
            None => flag("derived_missing", k::describe_key(key)),
            Some(found) if found != value => flag(
                "derived_mismatch",
                format!(
                    "{}: stored {found:?}, derived {value:?}",
                    k::describe_key(key)
                ),
            ),
            Some(_) => {}
        }
    }
    for key in stored.keys() {
        if !expected.contains_key(*key) {
            flag("derived_extra", k::describe_key(key));
        }
    }
    Ok((violations, stored.len()))
}

fn owner_exists<R: TryRead>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    owner: u64,
) -> Result<bool, StorageError> {
    Ok(match kind {
        OwnerKind::Record => r.get(&k::record_hdr_key(t, owner))?.is_some(),
        OwnerKind::Relationship => r.get(&kx::rel_by_ord_key(t, owner))?.is_some(),
    })
}

/// A directed adjacency entry: `(rel_type, source, target, rel_ordinal)`.
type Edge = (String, u64, u64, u64);

fn adjacency_violations<R: TryRead>(r: &R, t: TenantId) -> Result<Vec<Violation>, StorageError> {
    let mut out_edges: BTreeSet<Edge> = BTreeSet::new();
    let mut in_edges: BTreeSet<Edge> = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    for (key, _) in r.range_prefix(&k::key(t, &[b"adj"]))? {
        let Some(rest) = key
            .get(TENANT_PREFIX_LEN..)
            .and_then(|rest| rest.strip_prefix(b"/adj/"))
        else {
            continue;
        };
        let (outgoing, rest) = if let Some(rest) = rest.strip_prefix(b"out/") {
            (true, rest)
        } else if let Some(rest) = rest.strip_prefix(b"in/") {
            (false, rest)
        } else {
            continue;
        };
        if let Some(node) = rest.get(..8).and_then(|bytes| bytes.try_into().ok()) {
            nodes.insert((u64::from_be_bytes(node), outgoing));
        }
    }
    for (node, outgoing) in nodes {
        let edges = if outgoing {
            try_access::adj_out(r, t, node, None)?
        } else {
            try_access::adj_in(r, t, node, None)?
        };
        for (rel_type, other, rel) in edges {
            if outgoing {
                out_edges.insert((rel_type, node, other, rel));
            } else {
                in_edges.insert((rel_type, other, node, rel));
            }
        }
    }
    let mut violations = Vec::new();
    for edge in out_edges.symmetric_difference(&in_edges) {
        violations.push(Violation {
            what: "adjacency_asymmetric".into(),
            detail: format!("{edge:?}"),
        });
    }
    for (rel_type, source, target, rel) in &out_edges {
        match try_access::relationship_by_ord(r, t, *rel)? {
            Some(header)
                if header.rel_type == *rel_type
                    && header.source_ord == *source
                    && header.target_ord == *target => {}
            _ => violations.push(Violation {
                what: "adjacency_without_relationship".into(),
                detail: format!("{rel_type}/{source}/{target}/{rel}"),
            }),
        }
    }
    Ok(violations)
}

/// Build a bounded, read-only repair plan: occurrence rows whose owner no
/// longer exists. The returned keys are only candidates;
/// [`repair_orphan_occurrences`] revalidates them inside its commit.
///
/// # Errors
///
/// Returns the underlying storage error if the scan or point reads fail.
pub fn plan_orphan_occurrences<R: TryRead>(
    r: &R,
    t: TenantId,
    max_keys: usize,
) -> Result<OrphanRepairPlan, StorageError> {
    let mut keys = Vec::new();
    let mut complete = true;
    if max_keys == 0 {
        return Ok(OrphanRepairPlan { keys, complete });
    }
    for kind in [OwnerKind::Record, OwnerKind::Relationship] {
        for (key, _) in r.range_prefix(&k::occ_kind_prefix(t, kind))? {
            let orphan = match k::parse_occ_key(&key) {
                Some((_, kind, owner, _, _)) => !owner_exists(r, t, kind, owner)?,
                None => true,
            };
            if orphan {
                if keys.len() < max_keys {
                    keys.push(key);
                } else {
                    complete = false;
                    return Ok(OrphanRepairPlan { keys, complete });
                }
            }
        }
    }
    Ok(OrphanRepairPlan { keys, complete })
}

/// Delete at most `max_keys` orphaned occurrence rows in one bounded commit.
/// This is an explicit repair operation; scrubbing itself remains read-only.
/// A zero limit performs no mutation.
///
/// # Errors
///
/// Returns the storage error if the repair scan or commit fails.
pub fn repair_orphan_occurrences(
    store: &rushdb_storage::Store,
    t: TenantId,
    max_keys: usize,
) -> Result<usize, StorageError> {
    if max_keys == 0 {
        return Ok(0);
    }
    let outcome = store.commit_for_tenant(&t, uuid::Uuid::now_v7(), |_ci, txn| {
        let orphaned = plan_orphan_occurrences(txn, t, max_keys)
            .map_err(|e| e.to_string())?
            .keys;
        let batch = rushdb_types::command::CommitBatch {
            command_version: rushdb_types::command::COMMAND_VERSION,
            tenant: t,
            transaction_id: "scrub-repair".into(),
            commit_key: uuid::Uuid::nil(),
            start_index: 0,
            isolation: rushdb_types::command::IsolationMode::Snapshot,
            preconditions: Vec::new(),
            mutations: orphaned
                .iter()
                .cloned()
                .map(|key| rushdb_types::command::Mutation::DeleteRawKey { key })
                .collect(),
        };
        Ok((batch, orphaned.len()))
    })?;
    match outcome {
        rushdb_storage::CommitOutcome::Committed { payload, .. } => Ok(payload),
        rushdb_storage::CommitOutcome::Conflicted(reason) => Err(StorageError::Builder(reason)),
    }
}
