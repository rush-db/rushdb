//! The materializers (PUB-1): one thread per redb shard applies the shard's
//! published commits from its [`Gap`] to redb in commit order, off the
//! publish path, and keeps each durable redb commit bounded by pacing the
//! shard's checkpoints (PERF-2: a durable commit writes out and syncs every
//! page buffered since the last one while it holds the only writer).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use crate::backend::CommitOps;
use crate::gap::{Gap, GapCommit};
use crate::{CommitObserver, CommitPhase, StorageBackend, StorageError, Store};

/// Gap bytes one materializer transaction takes at most (at least one
/// commit). Larger batches amortize redb's per-transaction cost; this keeps
/// one batch's writer hold short beside the checkpoints.
const BATCH_BYTES: usize = 16 << 20;

/// The store's materializer threads.
#[derive(Default)]
pub(crate) struct Materializers {
    running: Mutex<Vec<(Arc<Gap>, std::thread::JoinHandle<()>)>>,
    paced_checkpoints: Arc<AtomicU64>,
}

impl Materializers {
    /// Start the materializer of one shard.
    pub(crate) fn start(
        &self,
        shard: usize,
        gap: Arc<Gap>,
        backend: Arc<dyn StorageBackend>,
        observer: Arc<RwLock<Option<Arc<CommitObserver>>>>,
    ) -> Result<(), StorageError> {
        let paced = Arc::clone(&self.paced_checkpoints);
        let worker = Arc::clone(&gap);
        let handle = std::thread::Builder::new()
            .name(format!("rushdb-materialize-{shard}"))
            .spawn(move || run(&worker, backend.as_ref(), &paced, &observer))
            .map_err(|error| StorageError::Durability(format!("materializer: {error}")))?;
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((gap, handle));
        Ok(())
    }

    /// Checkpoints a materializer ran because its shard's buffered writes
    /// passed their bound, cumulative.
    pub(crate) fn paced_checkpoints(&self) -> u64 {
        self.paced_checkpoints.load(Ordering::Relaxed)
    }

    /// Let every materializer finish its queue (at most the gap bound of
    /// work), then wait for it to exit. Its writes are buffered: what the
    /// last checkpoint did not cover replays from the WAL at the next open.
    pub(crate) fn stop(&self) {
        let running =
            std::mem::take(&mut *self.running.lock().unwrap_or_else(PoisonError::into_inner));
        for (gap, _) in &running {
            gap.stop();
        }
        for (_, handle) in running {
            let _ = handle.join();
        }
    }
}

impl Drop for Materializers {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(
    gap: &Gap,
    backend: &dyn StorageBackend,
    paced: &AtomicU64,
    observer: &RwLock<Option<Arc<CommitObserver>>>,
) {
    while let Some(batch) = gap.next_batch(BATCH_BYTES) {
        let started = std::time::Instant::now();
        let materialized = materialize(gap, backend, &batch);
        let observer = observer
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(observer) = observer {
            observer(CommitPhase::BackendApply, started.elapsed().as_secs_f64());
        }
        if let Err(error) = materialized {
            tracing::error!(%error, "materialization failed; the store is fenced");
            gap.fail(error);
            return;
        }
        #[cfg(test)]
        crate::kill_point::hit("materialized");
        if backend.take_flush_due() {
            let newest = batch.last().map_or(0, |commit| commit.ci);
            match backend.checkpoint(newest) {
                Ok(true) => {
                    paced.fetch_add(1, Ordering::Relaxed);
                }
                Ok(false) => {}
                // The maintenance tick's checkpoint retries it.
                Err(error) => tracing::warn!(%error, "paced checkpoint failed"),
            }
        }
        let count = batch.len();
        // The batch holds the commits' charges: release them before the
        // gap reports them done.
        drop(batch);
        gap.materialized(count);
    }
}

fn materialize(
    gap: &Gap,
    backend: &dyn StorageBackend,
    batch: &[Arc<GapCommit>],
) -> Result<(), String> {
    #[cfg(debug_assertions)]
    let expected = expected_usage(gap, backend, batch)?;
    #[cfg(not(debug_assertions))]
    let _ = gap;
    let pairs: Vec<CommitOps<'_>> = batch
        .iter()
        .map(|commit| CommitOps {
            ci: commit.ci,
            base: &commit.base,
            derived: &commit.derived,
        })
        .collect();
    backend.apply_many(&pairs)?;
    #[cfg(debug_assertions)]
    check_usage(backend, expected)?;
    Ok(())
}

/// The usage rows the gap answers after `batch`, computed before the apply.
#[cfg(debug_assertions)]
fn expected_usage(
    gap: &Gap,
    backend: &dyn StorageBackend,
    batch: &[Arc<GapCommit>],
) -> Result<Vec<(Vec<u8>, Option<Vec<u8>>)>, String> {
    let snapshot = gap.snapshot();
    let base = backend.read_view().map_err(|error| error.to_string())?;
    let view = crate::gap::GapView::new(base, snapshot).map_err(|error| error.to_string())?;
    let last = batch.last().map_or(0, |commit| commit.ci);
    let mut keys: Vec<&[u8]> = batch
        .iter()
        .flat_map(|commit| commit.usage_keys())
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter()
        .map(|key| {
            view.current_at(key, last)
                .map(|value| (key.to_vec(), value))
                .map_err(|error| error.to_string())
        })
        .collect()
}

/// The apply computed each tenant's usage row itself; the gap answered
/// it on demand until now. They must agree.
#[cfg(debug_assertions)]
fn check_usage(
    backend: &dyn StorageBackend,
    expected: Vec<(Vec<u8>, Option<Vec<u8>>)>,
) -> Result<(), String> {
    use crate::read_view::{ReadLimits, ScanProjection};
    if expected.is_empty() {
        return Ok(());
    }
    let view = backend.read_view().map_err(|error| error.to_string())?;
    for (key, value) in expected {
        let applied = view
            .read_value(
                &key,
                0,
                true,
                ReadLimits::default(),
                ScanProjection::KeyValue,
            )
            .map_err(|error| error.to_string())?;
        if applied != value {
            return Err(format!(
                "usage row {key:?}: published {value:?}, materialized {applied:?}"
            ));
        }
    }
    Ok(())
}

/// Bytes of published, not yet materialized commits a store holds before
/// publishers wait for its materializers (PUB-1): a quarter of the managed
/// budget, at most 256 MiB. At the bound a commit waits as it did when
/// publication included the apply.
fn gap_bound(managed_limit_bytes: usize) -> usize {
    (managed_limit_bytes / 4).min(256 << 20)
}

impl Store {
    /// Give every shard a gap and a materializer (PUB-1), after recovery
    /// replayed the WAL tail straight into the backends.
    pub(crate) fn start_materializers(&self) -> Result<(), StorageError> {
        let backends: Vec<Option<Arc<dyn StorageBackend>>> =
            self.health.guard(self.durable.read())?.to_vec();
        let shards = backends.len().max(1);
        let pool = self
            .resources
            .child(
                "publish-gap",
                gap_bound(self.resource_config.managed_limit_bytes),
            )
            .map_err(|e| StorageError::ResourceExhausted(e.to_string()))?;
        let per_shard = pool.snapshot().limit / shards;
        let mut gaps = Vec::with_capacity(backends.len());
        for (shard, backend) in backends.into_iter().enumerate() {
            let Some(backend) = backend else {
                gaps.push(None);
                continue;
            };
            let budget = pool
                .child("shard", per_shard)
                .map_err(|e| StorageError::ResourceExhausted(e.to_string()))?;
            let gap = Arc::new(Gap::new(budget));
            self.materializers.start(
                shard,
                Arc::clone(&gap),
                backend,
                Arc::clone(&self.commit_observer),
            )?;
            gaps.push(Some(gap));
        }
        *self.health.guard(self.gaps.write())? = gaps.into_boxed_slice();
        Ok(())
    }

    /// The gap of a physical shard, when the store publishes ahead of
    /// materialization.
    pub(crate) fn gap(&self, shard: usize) -> Option<Arc<Gap>> {
        self.health
            .cleanup(self.gaps.read())
            .get(shard)
            .and_then(std::clone::Clone::clone)
    }

    pub(crate) fn all_gaps(&self) -> Vec<Arc<Gap>> {
        self.health
            .cleanup(self.gaps.read())
            .iter()
            .flatten()
            .cloned()
            .collect()
    }

    /// A read view of one physical shard at its newest published commit:
    /// the backend's view overlaid with the shard's not yet materialized
    /// commits. The gap is snapshotted first, so the backend view holds
    /// everything the snapshot lacks.
    pub(crate) fn shard_read_view(
        &self,
        shard: usize,
        backend: &dyn StorageBackend,
    ) -> Result<Box<dyn crate::backend::StorageReadView>, StorageError> {
        let index = self.gap(shard).map(|gap| gap.snapshot());
        let base = backend.read_view()?;
        match index {
            Some(index) if !index.is_empty() => {
                Ok(Box::new(crate::gap::GapView::new(base, index)?))
            }
            _ => Ok(base),
        }
    }

    /// The newest commit index at or below which every published commit is
    /// in its shard's backend: what a checkpoint may record and the WAL may
    /// be truncated to.
    #[must_use]
    pub fn materialized_version(&self) -> u64 {
        let published = self.committed_version();
        self.all_gaps()
            .iter()
            .map(|gap| gap.materialized_through())
            .fold(published, u64::min)
    }

    /// Wait until every commit published so far is materialized.
    ///
    /// # Errors
    /// Returns the materializer's failure.
    pub fn wait_materialized(&self) -> Result<(), StorageError> {
        let published = self.committed_version();
        for gap in self.all_gaps() {
            gap.wait_materialized(published)?;
        }
        Ok(())
    }

    /// Hold (or release) every shard's materializer, so a test can read
    /// commits that are published but not in the backends.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn pause_materialization(&self, paused: bool) {
        for gap in self.all_gaps() {
            gap.set_paused(paused);
        }
    }

    /// Bytes of published commits not yet materialized, all shards.
    #[must_use]
    pub fn unmaterialized_bytes(&self) -> usize {
        self.all_gaps().iter().map(|gap| gap.bytes()).sum()
    }
}
