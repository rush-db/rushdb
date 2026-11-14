//! Ordered group publication: the publisher drains contiguous pending
//! commits into one WAL append + fsync and one backend transaction (base plus
//! derived rows, DC12), then records the conflict window in the same critical
//! section.

use std::collections::BTreeMap;

use super::backend::CommitOps;
use super::{
    derive, wal, CommitPhase, DeriveRead, Op, Ordering, PendingCommit, PreparedCommand,
    StorageError, Store,
};

impl Store {
    /// V2 WAL batching: hold the publish lock briefly so concurrent commits
    /// can join this group and share one fsync. A no-op when
    /// `max_group_delay_ms` is 0. Returns early once the count cap is reached.
    pub(super) fn linger_for_group(&self) {
        let delay_ms = self.resource_config.max_group_delay_ms;
        if delay_ms == 0 {
            return;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(delay_ms);
        let mut pending = self.health.cleanup(self.pending.lock());
        loop {
            let cap = self.resource_config.max_group_count;
            if cap > 0 && pending.len() >= cap {
                return;
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return;
            }
            let (guard, _) = self
                .health
                .cleanup(self.pending_cv.wait_timeout(pending, remaining));
            pending = guard;
        }
    }

    /// Group-commit publisher: drain the contiguous prefix from the published
    /// watermark, persist ALL drained op-sets in ONE redb write transaction,
    /// then apply them in commit-index order and ack each waiter.
    pub(super) fn drain_and_publish(&self) {
        self.drain_and_publish_until(None);
    }

    /// Publish contiguous pending commits. With `until = Some(ci)` the caller
    /// stops as soon as commit `ci` is published and leaves later commits to
    /// the next committer. Otherwise the thread that happened to win
    /// `publish_lock` kept publishing every other writer's groups for as long
    /// as new commits kept arriving, and its own already-durable request
    /// waited for the whole burst (a publisher convoy: the first concurrent
    /// ingest batch took 15 s while every other batch took ≤ 1.7 s).
    pub(super) fn drain_and_publish_until(&self, until: Option<u64>) {
        self.linger_for_group();
        loop {
            if until.is_some_and(|ci| self.published.load(Ordering::SeqCst) >= ci) {
                return;
            }
            let mut batch: Vec<PendingCommit> = Vec::new();
            let mut next = self.published.load(Ordering::SeqCst) + 1;
            let mut group_bytes = 0usize;
            {
                let mut pending = self.health.cleanup(self.pending.lock());
                while let Some(pc) = pending.remove(&next) {
                    let ops_bytes: usize = pc.ops.iter().map(Op::encoded_len).sum();
                    // A command expanded against the sequencer overlay
                    // already read every predecessor, and the group applies
                    // in commit-index order, so it may share the group (and
                    // its fsync) with commits it depends on. Any other
                    // command keeps the barrier: if it overlaps an earlier
                    // member, publish the collected prefix first and retry it
                    // in the next group against the newly published state.
                    // Without the exemption every dependent single-record
                    // commit of one tenant paid its own fsync (measured: one
                    // tenant capped at ~90 commits/s regardless of writers).
                    if !pc.prepared.sequenced
                        && batch
                            .iter()
                            .any(|prior| prior.prepared.conflicts_with(&pc.prepared))
                    {
                        pending.insert(next, pc);
                        break;
                    }
                    // P04 bounded writer: respect the configured group count
                    // and byte caps so a burst cannot balloon one redb txn.
                    if !batch.is_empty() {
                        if self.resource_config.max_group_count > 0
                            && batch.len() >= self.resource_config.max_group_count
                        {
                            pending.insert(next, pc);
                            break;
                        }
                        if self.resource_config.max_group_bytes > 0
                            && group_bytes + ops_bytes > self.resource_config.max_group_bytes
                        {
                            pending.insert(next, pc);
                            break;
                        }
                    }
                    group_bytes += ops_bytes;
                    batch.push(pc);
                    next += 1;
                }
            }
            if batch.is_empty() {
                return; // nothing contiguous ready; wait for the gap to fill
            }
            debug_assert!(batch.iter().all(|pc| {
                pc.prepared.commit_index == pc.ci
                    && pc.prepared.writes == PreparedCommand::from_ops(pc.ci, &pc.ops).writes
            }));
            // DC12: derive each commit's derived rows against the state before
            // it (its shard backend plus earlier commits of this group). The
            // WAL still receives base ops only (`batch`), the backend receives
            // base + derived (`pairs`).
            let derive_started = std::time::Instant::now();
            let derived = match self.derive_group(&batch) {
                Ok(derived) => {
                    self.observe_phase(CommitPhase::Derive, derive_started);
                    derived
                }
                Err(error) => {
                    self.fail_publication(batch, &error);
                    return;
                }
            };
            let pairs: Vec<CommitOps<'_>> = batch
                .iter()
                .zip(&derived)
                .map(|(pc, derived)| CommitOps {
                    ci: pc.ci,
                    base: &pc.ops,
                    derived,
                })
                .collect();
            let index_writes = index_writes(&pairs);
            let publication = self.persist_many_sharded(&pairs, &batch);
            drop(pairs);
            drop(derived);
            match publication {
                Ok(()) => {
                    let publish_started = std::time::Instant::now();
                    {
                        // Record write sets before `published` advances, so a
                        // validator that observes the new index always finds
                        // the summary (C4: in-memory, see `conflict_window`).
                        let mut window = self.health.cleanup(self.conflicts.lock());
                        for pc in &mut batch {
                            window.record_keys(pc.ci, std::mem::take(&mut pc.prepared.writes));
                        }
                    }
                    // Likewise before `published` advances: a reader of the
                    // new snapshot never reuses a set these commits changed.
                    for (ci, touched) in index_writes {
                        if !touched.is_empty() {
                            self.owner_sets.note_writes(ci, touched);
                        }
                    }
                    self.published
                        .store(batch.last().unwrap().ci, Ordering::SeqCst);
                    self.group_commits.fetch_add(1, Ordering::Relaxed);
                    self.last_commit_ms.store(wal::now_ms(), Ordering::Relaxed);
                    for pc in batch {
                        let _ = pc.ack.send(Ok(()));
                    }
                    self.observe_phase(CommitPhase::Publish, publish_started);
                }
                Err(e) => {
                    self.fail_publication(batch, &e);
                    return;
                }
            }
            self.pending_cv.notify_all();
        }
    }

    /// Per commit of a publish group, the ops derived from it (empty when
    /// nothing was derived), each derived against the state before it: its
    /// shard backend plus the earlier commits of the group. That prior state
    /// borrows the group's ops; nothing is copied into it.
    fn derive_group(&self, batch: &[PendingCommit]) -> Result<Vec<Vec<Op>>, StorageError> {
        let Some(deriver) = self.deriver() else {
            return Ok(vec![Vec::new(); batch.len()]);
        };
        // Each commit's derived ops are written once into its cell and never
        // move, so later commits' layered state can borrow them.
        let outputs: Vec<std::cell::OnceCell<Vec<Op>>> =
            batch.iter().map(|_| std::cell::OnceCell::new()).collect();
        {
            let mut bases: BTreeMap<usize, Box<dyn DeriveRead>> = BTreeMap::new();
            let mut buffers: BTreeMap<usize, BTreeMap<&[u8], Option<&[u8]>>> = BTreeMap::new();
            for (position, (pc, cell)) in batch.iter().zip(&outputs).enumerate() {
                if let std::collections::btree_map::Entry::Vacant(slot) = bases.entry(pc.shard) {
                    slot.insert(match self.backend(pc.shard) {
                        Some(backend) => Box::new(derive::BackendRead(backend.read_view()?)),
                        None => Box::new(derive::Empty),
                    });
                }
                let buffer = buffers.entry(pc.shard).or_default();
                let ops = {
                    let before = derive::Layered {
                        base: bases[&pc.shard].as_ref(),
                        buffer,
                    };
                    Self::derive_ops(Some(&deriver), pc.ci, &pc.ops, &before)?
                };
                let ops = cell.get_or_init(|| ops);
                // The last commit has no successor in this group to read it.
                if position + 1 < batch.len() {
                    derive::fold(buffer, pc.ops.iter().chain(ops));
                }
            }
        }
        Ok(outputs
            .into_iter()
            .map(|cell| cell.into_inner().unwrap_or_default())
            .collect())
    }

    /// Fence the publisher after a failed group: fail this group and every
    /// queued commit with the same error.
    fn fail_publication(&self, batch: Vec<PendingCommit>, error: &StorageError) {
        let mut pending = self.health.cleanup(self.pending.lock());
        *self.health.cleanup(self.publisher_failure.lock()) = Some(error.clone());
        for pc in batch {
            let _ = pc.ack.send(Err(error.clone()));
        }
        for (_, pc) in std::mem::take(&mut *pending) {
            let _ = pc.ack.send(Err(error.clone()));
        }
        self.pending_cv.notify_all();
    }
}

/// The indexes each commit writes (their entries are derived rows), for the
/// owner-set cache's versions (READ-2).
fn index_writes(pairs: &[CommitOps<'_>]) -> Vec<(u64, Vec<crate::owner_sets::IndexRef>)> {
    pairs
        .iter()
        .map(|pair| {
            let mut touched: Vec<crate::owner_sets::IndexRef> = pair
                .base
                .iter()
                .chain(pair.derived.iter())
                .filter_map(|op| rushdb_types::keys::parse_index_entry_index(op.key()))
                .collect();
            touched.sort_unstable();
            touched.dedup();
            (pair.ci, touched)
        })
        .collect()
}
