//! An unindexed leaf's owners, checked in fixed-size parts.
//!
//! A leaf no declared index answers (`payment = card`, `fare > 10`) is
//! resolved by reading each owner's occurrence of the property and keeping
//! the owners whose value matches. The owners to check are either the
//! candidates earlier leaves left ([`LeafScope::candidates`]) or the
//! property's presence rows under the requested labels. Either is cut into
//! parts of about [`WALK_PART_OWNERS`] owners (the candidates by rank, the
//! presence rows by ranges of owner ordinals that each part reads itself):
//! the bounds depend on nothing but the data, never on the threads.
//! Parts run on the threads [`rushdb_storage::parallel`] lends, and their
//! matches merge into the owner set in part order, so the set (and whether
//! it passes the owner budget) is the same whether one thread ran every part
//! or many did. The caller's cancellation is polled between rounds of parts;
//! a part that fails stops the others of its round.

use std::sync::atomic::{AtomicBool, Ordering};

use rushdb_lmpg::reader::TryRead;

use super::{
    labels_for, BoundedOwners, LeafScope, OwnerSet, PLANNER_MATERIALIZE_CAP, UNINDEXED_WALKS,
    UNINDEXED_WALK_THRESHOLD,
};
use crate::{access, QueryError};

/// Owners checked per part.
pub(super) const WALK_PART_OWNERS: usize = 4096;

/// Which occurrences of a property keep their owner.
pub(super) type Keep<'a> = &'a (dyn Fn(&access::Occurrence) -> bool + Sync);

/// Owners under `scope.labels` (all when empty) carrying a non-null value of
/// `pid` for which `keep` holds, added to `owners`. When earlier leaves
/// already narrowed the owners to fewer candidates than `pid`'s population,
/// only those candidates are checked; otherwise the population is walked.
/// Returns `false` (scan instead) when the population to walk exceeds the
/// materialization cap.
pub(super) fn owners_matching<R: TryRead + Sync>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
    pid: u64,
    scope: &LeafScope<'_>,
    owners: &mut OwnerSet,
    keep: Keep<'_>,
) -> Result<bool, QueryError> {
    let kind = rushdb_types::ids::OwnerKind::Record;
    let labels = labels_for(snap, tenant, pid, scope.labels)?;
    let mut population = 0u64;
    for label in &labels {
        population = population.saturating_add(rushdb_lmpg::derived::prop_support_label(
            snap, tenant, kind, pid, label,
        )?);
    }
    // Labels whose owners hold `pid` as an array (maintained per label): only
    // those need every element; elsewhere the value is the single row at
    // element 0, read with one point get instead of a range scan per owner.
    let (_, array_labels) = rushdb_lmpg::derived::array_markers(snap, tenant, kind, pid)?;
    let check = Check {
        snap,
        tenant,
        pid,
        keep,
    };
    let part_len = u64::try_from(WALK_PART_OWNERS).unwrap_or(u64::MAX);
    if let Some(candidates) = scope.candidates.filter(|c| c.len() < population) {
        let scalar = array_labels.is_empty();
        let parts = usize::try_from(candidates.len().div_ceil(part_len)).unwrap_or(usize::MAX);
        merge_parts(parts, scope, owners, |part, seen, stop| {
            let first = u64::try_from(part)
                .unwrap_or(u64::MAX)
                .saturating_mul(part_len);
            let Some(start) = candidates.select(first) else {
                return Ok(Some(Vec::new()));
            };
            let mut iter = candidates.iter();
            iter.advance_to(start);
            check.part(iter.take(WALK_PART_OWNERS).map(|o| (o, scalar)), seen, stop)
        })?;
        return Ok(true);
    }
    if population > PLANNER_MATERIALIZE_CAP {
        return Ok(false);
    }
    if population > UNINDEXED_WALK_THRESHOLD {
        UNINDEXED_WALKS.fetch_add(1, Ordering::Relaxed);
    }
    // The population is walked in ranges of `width` ordinals below the
    // tenant's ordinal watermark, scaled by how sparse the property's owners
    // are among the ordinals so a part holds about [`WALK_PART_OWNERS`]
    // owners. Each part reads its own range of every label's presence rows
    // (keys only), so no list of the whole population is gathered first.
    let prefixes: Vec<(Vec<u8>, bool)> = labels
        .iter()
        .map(|label| {
            let prefix = rushdb_types::keys::presence_nullness_prefix(
                tenant,
                kind,
                label,
                pid,
                rushdb_types::keys::NULLNESS_VALUE,
            );
            (prefix, !array_labels.contains(label))
        })
        .collect();
    let (width, parts) = match crate::scan::ordinal_end(snap, tenant)? {
        Some(end) if end > 0 => {
            let width = part_len.saturating_mul(end.div_ceil(population.max(1)).max(1));
            (width, end.div_ceil(width))
        }
        // No allocator watermark: one part reads every row.
        _ => (u64::MAX, 1),
    };
    let parts = usize::try_from(parts).unwrap_or(usize::MAX);
    merge_parts(parts, scope, owners, |part, seen, stop| {
        let part_u64 = u64::try_from(part).unwrap_or(u64::MAX);
        // The first part starts at the prefix and the last one runs to its
        // end, so every row is read whatever the watermark says.
        let from = (part > 0).then(|| part_u64.saturating_mul(width));
        let to = (part + 1 < parts).then(|| part_u64.saturating_add(1).saturating_mul(width));
        let mut ords = Vec::new();
        for (prefix, scalar) in &prefixes {
            each_presence_owner(snap, prefix, from, to, &mut |owner| {
                ords.push((owner, *scalar));
                Ok(())
            })?;
        }
        check.part(ords.into_iter(), seen, stop)
    })?;
    Ok(true)
}

/// Visit every owner of the presence rows under `prefix` whose ordinal is
/// at least `from` and below `to` (unbounded when `None`), in owner order,
/// reading keys only. An ordinal's key encoding sorts by value, so the
/// bounds are one key range.
pub(super) fn each_presence_owner<R: TryRead + ?Sized>(
    snap: &R,
    prefix: &[u8],
    from: Option<u64>,
    to: Option<u64>,
    visit: &mut dyn FnMut(u64) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    let bound = |ord: Option<u64>| {
        ord.map(|ord| {
            let mut key = prefix.to_vec();
            rushdb_types::keys::push_ord(&mut key, ord);
            key
        })
    };
    let (prefix_start, prefix_end) = rushdb_storage::prefix_range(prefix);
    let start = bound(from).unwrap_or(prefix_start);
    let end = bound(to).unwrap_or(prefix_end);
    let mut failed = None;
    snap.for_each_key(&start, &end, &mut |key| {
        let result = rushdb_types::keys::read_ord_tail(key, prefix.len())
            .ok_or_else(|| rushdb_storage::StorageError::CorruptLog.into())
            .and_then(&mut *visit);
        match result {
            Ok(()) => true,
            Err(error) => {
                failed = Some(error);
                false
            }
        }
    })?;
    failed.map_or(Ok(()), Err)
}

/// The reads that decide whether one owner matches.
struct Check<'a, R> {
    snap: &'a R,
    tenant: rushdb_types::ids::TenantId,
    pid: u64,
    keep: Keep<'a>,
}

impl<R: TryRead> Check<'_, R> {
    /// Whether `owner` holds a kept value: one point get when it holds the
    /// property as a scalar, every element otherwise.
    fn matches(&self, owner: u64, scalar: bool) -> Result<bool, QueryError> {
        let kind = rushdb_types::ids::OwnerKind::Record;
        Ok(if scalar {
            access::occurrence(self.snap, self.tenant, kind, owner, self.pid, 0)?
                .is_some_and(|occ| (self.keep)(&occ))
        } else {
            access::occurrences_of_pid(self.snap, self.tenant, kind, owner, self.pid)?
                .iter()
                .any(self.keep)
        })
    }

    /// The matching owners of one part, skipping those already in `seen`;
    /// `None` when another part failed first.
    fn part(
        &self,
        owners: impl Iterator<Item = (u64, bool)>,
        seen: &OwnerSet,
        stop: &AtomicBool,
    ) -> Result<Option<Vec<u64>>, QueryError> {
        let mut out = Vec::new();
        for (i, (owner, scalar)) in owners.enumerate() {
            if i % 256 == 0 && stop.load(Ordering::Relaxed) {
                return Ok(None);
            }
            if !seen.contains(owner) && self.matches(owner, scalar)? {
                out.push(owner);
            }
        }
        Ok(Some(out))
    }
}

/// Run `part(0..parts)` in rounds of as many parts as there are threads to
/// lend, polling the caller's cancellation between rounds, and add each
/// round's matches to `owners` in part order under the owner budget. A part
/// sees the owners as they stood before its round and returns `None` when
/// another part of the round failed.
fn merge_parts<F>(
    parts: usize,
    scope: &LeafScope<'_>,
    owners: &mut OwnerSet,
    part: F,
) -> Result<(), QueryError>
where
    F: Fn(usize, &OwnerSet, &AtomicBool) -> Result<Option<Vec<u64>>, QueryError> + Sync,
{
    let round = rushdb_storage::parallel::budget().saturating_add(1);
    let stop = AtomicBool::new(false);
    let mut start = 0;
    while start < parts {
        if (scope.is_cancelled)() {
            return Err(QueryError::Cancelled);
        }
        let end = start.saturating_add(round).min(parts);
        let seen: &OwnerSet = owners;
        let outs = rushdb_storage::parallel::run_parts(end - start, |i| {
            let out = part(start + i, seen, &stop);
            if out.is_err() {
                stop.store(true, Ordering::SeqCst);
            }
            Ok::<_, std::convert::Infallible>(out)
        })
        .unwrap_or_else(|never| match never {});
        // The first failure in part order wins; a part stops only after
        // another one failed.
        let mut matched = Vec::with_capacity(outs.len());
        for out in outs {
            matched.push(out?);
        }
        let mut bounded = BoundedOwners::new(owners, scope.owner_limit);
        for part in matched {
            // Every part ran to its end: nothing failed, so nothing stopped.
            let Some(part) = part else {
                return Err(QueryError::Cancelled);
            };
            for owner in part {
                bounded.insert(owner)?;
            }
        }
        start = end;
    }
    Ok(())
}
