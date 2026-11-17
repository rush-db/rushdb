//! Conflict scans of a commit against the commits published after its
//! snapshot (first committer wins).

use super::{DependencySet, Ordering, SequencerState, StorageError, Store, TxAccumulator};

impl Store {
    /// The first commit after `snapshot_ci` that wrote a key `deps` covers:
    /// published commits from the conflict window, in-flight ones from the
    /// admission buffer. The caller holds admission (so no commit is
    /// assigned meanwhile) and has drained publication.
    pub(crate) fn first_conflict_since(
        &self,
        admission: &SequencerState,
        snapshot_ci: u64,
        deps: &DependencySet,
    ) -> Result<Option<u64>, StorageError> {
        let published = self.published.load(Ordering::SeqCst);
        let deps = deps.index();
        let window = self.health.guard(self.conflicts.lock())?;
        for probe in (snapshot_ci + 1)..=published {
            let key_set = window
                .keys(probe)
                .ok_or(StorageError::ConflictHistoryUnavailable {
                    commit_index: probe,
                })?;
            if key_set.iter().any(|key| deps.contains(key)) {
                return Ok(Some(probe));
            }
        }
        drop(window);
        Ok(admission
            .layers
            .iter()
            .filter(|layer| layer.ci() > snapshot_ci)
            .find(|layer| layer.keys().any(|key| deps.contains(key)))
            .map(|layer| layer.ci()))
    }

    /// The first commit after `after`, up to the latest published one, whose
    /// writes the transaction read or also wrote. Returns the last commit
    /// scanned and the conflicting one, if any. The window lock is taken per
    /// commit, so the publisher is not held up by a long scan.
    pub(crate) fn scan_conflicts(
        &self,
        tx: &TxAccumulator,
        reads: &crate::DependencyIndex<'_>,
        after: u64,
    ) -> Result<(u64, Option<u64>), StorageError> {
        let published = self.published.load(Ordering::SeqCst);
        for probe in (after + 1)..=published {
            let key_set = self
                .health
                .guard(self.conflicts.lock())?
                .keys(probe)
                .ok_or(StorageError::ConflictHistoryUnavailable {
                    commit_index: probe,
                })?;
            if key_set
                .iter()
                .any(|key| reads.contains(key) || writes_conflicting(tx, key))
            {
                return Ok((probe, Some(probe)));
            }
        }
        Ok((published.max(after), None))
    }
}

/// Whether a key written by a later commit collides with the transaction's
/// own writes. A monotone counter both advance is not a collision.
pub(crate) fn writes_conflicting(tx: &TxAccumulator, key: &[u8]) -> bool {
    tx.buffer.contains_key(key) && !tx.watermarks.contains(key)
}
