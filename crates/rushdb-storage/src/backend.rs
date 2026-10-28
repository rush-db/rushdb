//! Narrow storage-substrate seam (research D6, plan W3).
//!
//! `StorageBackend` captures the durability/write surface the [`Store`] needs
//! from its physical substrate. The synchronous redb implementation
//! (`RedbBackend`) satisfies it today; an async backend (`io_uring` / direct IO)
//! or a future `RushDB` `PageStore` can replace it without touching LMPG, query,
//! search, or the replication/transaction layers, because those layers depend
//! only on this trait and on the concrete read-view type.
//!
//! Greenfield note: the read view (`RedbReadView`) remains concrete in V1 — it
//! is the synchronous backend's owned snapshot. An async backend brings its own
//! view type behind this same seam; the point of the trait is that the write /
//! durability contract is already isolated.

use crate::read_view::{ReadLimits, ScanProjection};
use crate::resources::ByteReservation;
use crate::{Op, StorageError, StorageStatistics, VersionEntry};
use std::collections::BTreeMap;
use std::ops::Bound;
use std::path::Path;
use std::sync::Arc;

/// Object-safe read snapshot of one physical backend.
///
/// Both the redb view (`RedbReadView`) and the in-memory view
/// (`MemoryReadView`) implement it, so [`crate::ReadView`] has one read path
/// regardless of substrate. Multi-shard stores merge the per-shard
/// views in `MergedViews`.
pub(crate) trait StorageReadView: Send + Sync {
    /// Point read at `ci`; `current` selects the latest-commit fast path.
    fn read_value(
        &self,
        key: &[u8],
        ci: u64,
        current: bool,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError>;

    /// Smallest key in `(lower, upper)`; `include_history` widens to retained
    /// versions for a non-current snapshot.
    fn next_key(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        include_history: bool,
    ) -> Result<Option<Vec<u8>>, StorageError>;

    /// Ordered current-value range with the requested projection.
    fn current_range<'a>(
        &'a self,
        start: &[u8],
        end: &[u8],
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>, StorageError>;

    /// `ScanCursor`'s v2 native-iterator fast path (B7 follow-up): a genuine
    /// backend range cursor over `(lower, upper)` matching `ScanCursor`'s own
    /// `Bound` representation exactly, unlike [`Self::current_range`]'s
    /// simplified included-start/excluded-end-or-unbounded convention. A
    /// real cursor walks consecutively (one seek for the whole scan, not one
    /// per row); callers must already know this view's `current` fast path
    /// is valid — same gate [`Self::read_value`]'s `current` flag needs,
    /// since this reads the live current-value table.
    ///
    /// `Ok(None)` means this backend has no such cursor; the caller falls
    /// back to [`Self::next_key`] + [`Self::read_value`] per row. The
    /// default is `Ok(None)` so adding this is additive — a backend opts in
    /// explicitly.
    fn native_range<'a>(
        &'a self,
        _lower: &Bound<Vec<u8>>,
        _upper: &Bound<Vec<u8>>,
        _limits: ReadLimits,
        _projection: ScanProjection,
    ) -> Result<
        Option<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>>,
        StorageError,
    > {
        Ok(None)
    }

    /// Durable LSN this view can serve.
    fn visible_ci(&self) -> Result<u64, StorageError>;

    /// Oldest commit for which this view still retains history.
    fn history_floor(&self) -> Result<u64, StorageError>;

    /// Whether this view is a physically pinned snapshot, so the `current`
    /// fast path (ignore `ci`, read the live table) stays valid after later
    /// commits. Redb read transactions pin pages (true); the live in-memory
    /// maps do not (false), so a view must resolve reads by `ci`.
    fn snapshot_pinned(&self) -> bool {
        false
    }
}

/// Backend-loaded state used to bootstrap a store.
#[derive(Debug, Default)]
pub struct LoadedState {
    /// Current key/value data.
    pub data: BTreeMap<Vec<u8>, Vec<u8>>,
    /// Creation commit index per live key.
    pub created: BTreeMap<Vec<u8>, u64>,
    /// Retained superseded versions per key.
    pub history: BTreeMap<Vec<u8>, Vec<VersionEntry>>,
    /// Durable GC floor.
    pub gc_watermark: u64,
}

/// [`StorageBackend::compact`]'s error when a read transaction is open: the
/// caller retries with every lock released.
pub(crate) const COMPACT_BUSY: &str = "compaction busy: read transactions open";

/// Write/durability contract of the physical storage substrate.
///
/// Implementations must make `apply`/`apply_many` atomic and, when the
/// configured durability is `Local`, durable before returning. `retire_before`
/// reclaims retained versions; `compact` releases freed pages.
///
/// Visibility is `pub(crate)` in W3: the seam already isolates the write/read
/// surface so a future async backend (`io_uring` / direct IO) or `RushDB`
/// `PageStore` can be swapped in without touching LMPG/query/search. The trait
/// becomes `pub` once an external backend implementation needs to name it.
pub(crate) trait StorageBackend: Send + Sync {
    /// Load the full current state (data, MVCC metadata, GC watermark).
    fn load(&self) -> Result<LoadedState, String>;

    /// Read only the durable-LSN watermark and GC floor: startup needs
    /// these to restore the committed version WITHOUT materializing the full
    /// dataset into the RAM mirror. Implementations should read only the two
    /// metadata keys; the default falls back to a full `load`.
    fn load_watermark(&self) -> Result<(u64, u64), String> {
        let state = self.load()?;
        let lsn = match state.data.get(crate::LAST_APPLIED_KEY) {
            Some(value) => crate::wal::decode_last_applied(value)
                .map_err(|error| format!("corrupt last-applied watermark: {error}"))?,
            None => 0,
        };
        Ok((lsn, state.gc_watermark))
    }

    /// Apply one logical op-set at `commit_index`.
    fn apply(&self, ops: &[Op], commit_index: u64) -> Result<(), String>;

    /// Apply many op-sets (ascending commit-index order) in ONE write
    /// transaction: a single durable acknowledgement covers the whole batch.
    fn apply_many(&self, batches: &[(u64, &[Op])]) -> Result<(), String>;

    /// Reclaim retained versions superseded before `watermark`.
    fn retire_before(&self, watermark: u64) -> Result<(), String>;

    /// Reclaim a bounded batch of history keys superseded before `watermark`,
    /// resuming after `cursor`. Returns the next key to resume from, or `None`
    /// when the walk is complete. Each call is one write transaction, so GC is
    /// bounded and resumable instead of one monolithic txn.
    fn retire_before_batch(
        &self,
        watermark: u64,
        _cursor: Option<Vec<u8>>,
        _max_keys: usize,
    ) -> Result<Option<Vec<u8>>, String> {
        // Default: fall back to the monolithic reclamation. Implementations
        // that support bounded GC override this.
        self.retire_before(watermark)?;
        Ok(None)
    }

    /// Signal in-flight maintenance scans to stop promptly (shutdown). Backends
    /// with no long scans treat this as a no-op.
    fn begin_shutdown(&self) {}

    /// Force queued writes durable (fsync) without changing logical state, and
    /// durably record `durable_lsn` as the materialization checkpoint. Under
    /// log-first the WAL is the durable truth and the backend is buffered, so a
    /// checkpoint is what makes the materialization durable and lets WAL
    /// segments at or below `durable_lsn` be truncated. Backends with no
    /// buffered writes treat this as a no-op.
    fn checkpoint(&self, _durable_lsn: u64) -> Result<(), String> {
        Ok(())
    }

    /// Run `run` while this backend commits nothing (readers continue), so
    /// its file can be copied consistently. Backends without a file just run
    /// it.
    fn while_writes_paused(&self, run: &mut dyn FnMut()) -> Result<(), String> {
        run();
        Ok(())
    }

    /// Compact the backend, releasing freed pages. Returns [`COMPACT_BUSY`]
    /// when a read transaction is open; it never waits for one.
    fn compact(&self) -> Result<(), String>;

    /// Durable byte accounting by key family. Default implementations return
    /// an explicit error (unavailable); file-backed backends override it.
    fn storage_stats(&self) -> Result<StorageStatistics, String> {
        Err("backend does not provide storage statistics".into())
    }

    /// Open an owned read view (snapshot) of the current committed state.
    fn read_view(&self) -> Result<Box<dyn StorageReadView>, StorageError>;

    /// Cumulative time spent waiting for and holding the backend's database
    /// lock. Backends without such a lock report zeros.
    fn lock_times(&self) -> crate::BackendLockTimes {
        crate::BackendLockTimes::default()
    }
    /// Writer holds by holder. Backends without a single writer
    /// report none.
    fn writer_holds(&self) -> Vec<(crate::WriterHolder, crate::WriterHolds)> {
        Vec::new()
    }
}

/// Open the redb backend as a [`StorageBackend`] handle.
pub(crate) fn open_backend(path: &Path, cache: ByteReservation) -> Result<BackendHandle, String> {
    Ok(Arc::new(crate::redb_backend::RedbBackend::open(
        path, cache,
    )?))
}

/// Convenience alias used by [`Store`] for its durable backend handle.
pub(crate) type BackendHandle = Arc<dyn StorageBackend>;
