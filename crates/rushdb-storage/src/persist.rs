//! Durable persistence of published commits: WAL append (+ fsync) followed by
//! the backend materialization, fanned out per physical shard.

use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(test)]
use super::Ordering;
use super::{
    derive, log_store, resources, wal, CommitPhase, Op, PendingCommit, StorageBackend,
    StorageError, Store,
};

impl Store {
    pub(super) fn persist_ops(
        &self,
        ops: &[Op],
        commit_index: u64,
        command_payload: Option<&[u8]>,
    ) -> Result<(), StorageError> {
        self.check_health()?;
        self.check_publisher()?;
        self.append_log(&[(commit_index, 0, ops, command_payload)])?;
        if let Some(backend) = self.primary_backend() {
            let before = derive::BackendRead(backend.read_view()?);
            let derived = Self::derive_ops(self.deriver().as_ref(), commit_index, ops, &before)?;
            drop(before);
            if let Err(error) = backend
                .apply(derived.as_deref().unwrap_or(ops), commit_index)
                .map_err(StorageError::Durability)
            {
                *self.health.guard(self.publisher_failure.lock())? = Some(error.clone());
                return Err(error);
            }
        }
        Ok(())
    }

    /// Append expanded op-sets to the segmented WAL and, under `Local`
    /// durability, fsync before returning. This is the durable acknowledgement
    /// boundary (Law 4); redb is a buffered materialization replayed from the
    /// WAL. A no-op for in-memory stores (no log). Each record's
    /// `command_payload`, when given, pairs its op-set with the originating
    /// `CommitBatch` so the command journal is WAL-derivable.
    ///
    /// A commit whose encoded op-set (plus command) exceeds the WAL's
    /// per-record ceiling is split into several framed records sharing one
    /// LSN (large-transaction support, see `wal::encode_chained_ops_record`)
    /// — transparent to every caller: `LogStore::append_batch` groups them
    /// back into one atomic unit, and reconstructs them as one commit on
    /// replay/replication. The overwhelmingly common case (everything fits
    /// in one record) produces byte-identical output to before.
    pub(super) fn append_log(
        &self,
        records: &[(u64, u32, &[Op], Option<&[u8]>)],
    ) -> Result<(), StorageError> {
        let encode_started = std::time::Instant::now();
        let encoded: Vec<(u64, Vec<u8>)> = records
            .iter()
            .flat_map(|(ci, shard, ops, command_payload)| {
                wal::encode_chained_ops_record(
                    *shard,
                    *ci,
                    ops,
                    *command_payload,
                    log_store::MAX_RECORD_BYTES,
                )
                .into_iter()
                .map(|framed| (*ci, framed))
            })
            .collect();
        self.observe_phase(CommitPhase::WalEncode, encode_started);
        let sync_started = std::time::Instant::now();
        let mut guard = self.health.guard(self.log.lock())?;
        if let Some(log) = guard.as_mut() {
            log.append_batch(&encoded)
                .map_err(|error| StorageError::Durability(format!("WAL append: {error}")))?;
            if self.resource_config.durability == resources::Durability::Local {
                log.sync()
                    .map_err(|error| StorageError::Durability(format!("WAL sync: {error}")))?;
            }
        }
        drop(guard);
        self.observe_phase(CommitPhase::WalAppendSync, sync_started);
        Ok(())
    }

    /// Persist a drained publisher batch, fanning out durability to each
    /// commit's physical shard backend. At `shard_count == 1` this is exactly
    /// the historical single-backend `apply_many`; with `> 1` shards,
    /// different tenants' commits persist in parallel.
    pub(super) fn persist_many_sharded(
        &self,
        pairs: &[(u64, &[Op])],
        batch: &[PendingCommit],
    ) -> Result<(), StorageError> {
        self.check_health()?;
        self.check_publisher()?;
        #[cfg(test)]
        if self.fail_next_persist.swap(false, Ordering::SeqCst) {
            return Err(StorageError::Durability(
                "injected persistence failure".into(),
            ));
        }
        let purged = pairs
            .iter()
            .flat_map(|(_, ops)| ops.iter())
            .filter(|op| matches!(op, Op::Purge(_)))
            .count();
        if purged > 0 {
            self.purged_since_compaction
                .fetch_add(purged as u64, std::sync::atomic::Ordering::Relaxed);
        }
        debug_assert_eq!(pairs.len(), batch.len());
        // Log-first: append the group's expanded op-sets to the segmented WAL
        // and make them durable BEFORE writing the materialization. This is the
        // durability acknowledgement boundary (Law 4). `Async` skips the fsync
        // and relies on a later checkpoint; the redb backends are always
        // buffered and replayed from the WAL on recovery.
        {
            let records: Vec<(u64, u32, &[Op], Option<&[u8]>)> = batch
                .iter()
                .map(|pc| {
                    let shard = u32::try_from(pc.shard).expect("shard index fits u32");
                    (
                        pc.ci,
                        shard,
                        pc.ops.as_slice(),
                        Some(pc.command_payload.as_slice()),
                    )
                })
                .collect();
            self.append_log(&records)?;
        }
        let apply_started = std::time::Instant::now();
        let applied = self.apply_sharded(pairs, batch);
        self.observe_phase(CommitPhase::BackendApply, apply_started);
        applied
    }

    /// Apply a durable group to each commit's physical shard backend.
    pub(super) fn apply_sharded(
        &self,
        pairs: &[(u64, &[Op])],
        batch: &[PendingCommit],
    ) -> Result<(), StorageError> {
        let mut by_shard: BTreeMap<usize, Vec<(u64, &[Op])>> = BTreeMap::new();
        for (pc, pair) in batch.iter().zip(pairs.iter()) {
            by_shard.entry(pc.shard).or_default().push(*pair);
        }
        // Persist shard groups in parallel: each shard backend
        // serializes its own redb writer, and groups touch disjoint files,
        // so parallel persistence keeps the same contract — one durable
        // acknowledgement per group, first error (in shard order) wins.
        // The single-shard path stays exactly sequential.
        if by_shard.len() == 1 {
            let (shard, shard_pairs) = by_shard.into_iter().next().unwrap();
            if let Some(backend) = self.backend(shard) {
                backend
                    .apply_many(&shard_pairs)
                    .map_err(StorageError::Durability)?;
            }
            return Ok(());
        }
        let backends: Vec<(usize, Option<Arc<dyn StorageBackend>>, Vec<(u64, &[Op])>)> = by_shard
            .into_iter()
            .map(|(shard, shard_pairs)| (shard, self.backend(shard), shard_pairs))
            .collect();
        std::thread::scope(|scope| {
            let handles: Vec<_> = backends
                .iter()
                .map(|(_, backend, shard_pairs)| {
                    scope.spawn(move || {
                        backend
                            .as_ref()
                            .map(|backend| {
                                backend
                                    .apply_many(shard_pairs)
                                    .map_err(StorageError::Durability)
                            })
                            .transpose()
                    })
                })
                .collect();
            let mut first_err = None;
            for handle in handles {
                match handle.join() {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        if first_err.is_none() {
                            first_err = Some(error);
                        }
                    }
                    Err(_) => {
                        if first_err.is_none() {
                            first_err = Some(StorageError::Durability(
                                "shard persist thread panicked".into(),
                            ));
                        }
                    }
                }
            }
            if let Some(error) = first_err {
                return Err(error);
            }
            Ok(())
        })
    }
}
