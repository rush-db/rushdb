//! Transitional fallible, pinned reads. Persistent views own one native redb
//! transaction per physical shard (merged on read); the memory reference uses
//! logical MVCC without cloning a root. Legacy callers remain separate until
//! the LMPG/query API migration.

use std::collections::BTreeMap;
use std::ops::Bound;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::{
    prefix_range, redb_backend::MergedViews, resources::physical_shard_for, StorageError, Store,
};
use rushdb_types::ids::TenantId;

/// Per-read byte ceilings enforced when copying values out of the store.
#[derive(Clone, Copy, Debug)]
pub struct ReadLimits {
    /// Maximum copied key plus value for a single result.
    pub max_item_bytes: usize,
    /// Maximum encoded bytes in one current MVCC history block.
    pub max_history_block_bytes: usize,
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            max_item_bytes: 8 * 1024 * 1024,
            max_history_block_bytes: 16 * 1024 * 1024,
        }
    }
}

impl ReadLimits {
    pub(crate) fn check_item(
        &self,
        key_bytes: usize,
        value_bytes: usize,
    ) -> Result<(), StorageError> {
        if key_bytes
            .checked_add(value_bytes)
            .is_none_or(|size| size > self.max_item_bytes)
        {
            return Err(StorageError::ResourceExhausted(
                "read item exceeds max_item_bytes".into(),
            ));
        }
        Ok(())
    }
}

/// Shared cancellation flag for one or more protected read views.
#[derive(Clone, Default, Debug)]
pub struct ReadCancellation(Arc<AtomicBool>);

impl ReadCancellation {
    /// Request cancellation of reads sharing this flag.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn check(&self) -> Result<(), StorageError> {
        if self.0.load(Ordering::Acquire) {
            Err(StorageError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// One commit index's retained read lease.
///
/// `generation` identifies the set of live views sharing this entry. It changes
/// whenever the entry is re-created after being emptied or expired, so a stale
/// `ReadView` that drops *after* GC expired its lease cannot decrement a newer,
/// unrelated pin that happens to sit at the same commit index.
#[derive(Debug, Clone, Copy)]
pub struct ReadPin {
    /// Number of live views currently pinned at this commit index.
    pub count: usize,
    /// Wall-clock millis of the most recent pin admission (lease age base).
    pub lease_ms: u64,
    /// Identity of this pin generation (see type docs).
    pub generation: u64,
}

static PIN_GENERATION: AtomicU64 = AtomicU64::new(1);

fn next_pin_generation() -> u64 {
    PIN_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// Owns all resources needed to read; it can outlive the Store and be shared
/// with asynchronous providers without self-referential borrowed lifetimes.
pub struct ReadView {
    health: Arc<crate::health::StoreHealth>,
    pins: Arc<Mutex<BTreeMap<u64, ReadPin>>>,
    cancellations: Arc<Mutex<BTreeMap<u64, Vec<ReadCancellation>>>>,
    ci: u64,
    // The store's apply-time deriver, so an overlay over this view can derive
    // its staged writes. Derived rows of committed state are ordinary
    // rows of the snapshot.
    deriver: Option<Arc<dyn crate::Deriver>>,
    // The store's planner-statistics cache; estimates only.
    planner_stats: Arc<crate::PlannerStats>,
    // Generation of the `ReadPin` entry this view admitted into. Guards Drop
    // against decrementing a newer pin at the same CI after lease expiry.
    pin_generation: u64,
    // Whether the logical cap equals the backend view's current CI. This
    // remains true across later writes because the backend pins its pages.
    native_current: bool,
    // One read view per physical shard, merged on read. A key routes to exactly
    // one shard, so no in-memory mirror is needed. `None` on a bounded child,
    // which delegates to its parent.
    views: Option<MergedViews>,
    // Must drop after `views`: their cache charge covers the read views too.
    backends: Vec<Arc<dyn crate::StorageBackend>>,
    limits: ReadLimits,
    cancellation: ReadCancellation,
    // A bounded child keeps the parent's pinned storage transaction alive and
    // only narrows copying limits. This avoids reopening a later snapshot.
    parent: Option<Arc<ReadView>>,
    materialization_limit: usize,
}

impl Store {
    /// Open a pinned read view of the latest committed state.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, the native
    /// backend read fails, or the default read limits are invalid.
    pub fn read_view(&self) -> Result<ReadView, StorageError> {
        self.read_with_limits(None, ReadLimits::default())
    }

    /// Open a pinned read view restricted to the physical shard that owns
    /// `tenant`. This avoids opening every shard for tenant-scoped engine
    /// requests while preserving the same snapshot and lease semantics.
    ///
    /// # Errors
    /// Returns the same errors as [`Store::read_view`].
    pub fn read_view_for_tenant(&self, tenant: &TenantId) -> Result<ReadView, StorageError> {
        self.read_with_limits_for_shard(None, ReadLimits::default(), Some(tenant))
    }

    /// Open a pinned read view of the state at `commit_index`.
    ///
    /// **Internal retention/backup mechanism, not a client contract.** There is
    /// no protocol surface for arbitrary snapshots (see); callers are
    /// limited to storage internals, offline PITR export, and the transaction
    /// coordinator's current-snapshot pin. It is `pub` only because those
    /// callers live in other crates.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, the native
    /// backend read fails, or the requested snapshot is in the future or
    /// expired.
    #[doc(hidden)]
    pub fn read_at(&self, commit_index: u64) -> Result<ReadView, StorageError> {
        self.read_with_limits(Some(commit_index), ReadLimits::default())
    }

    /// Open a pinned read view with explicit `limits`, optionally at a
    /// requested commit index.
    ///
    /// # Errors
    /// Returns [`StorageError::InvalidRead`] for zero limits,
    /// [`StorageError::FutureSnapshot`] when `requested` is newer than the
    /// visible commit, [`StorageError::SnapshotExpired`] when it predates the
    /// GC floor, or a backend error if the native view cannot be opened.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn read_with_limits(
        &self,
        requested: Option<u64>,
        limits: ReadLimits,
    ) -> Result<ReadView, StorageError> {
        self.read_with_limits_for_shard(requested, limits, None)
    }

    fn read_with_limits_for_shard(
        &self,
        requested: Option<u64>,
        limits: ReadLimits,
        tenant: Option<&TenantId>,
    ) -> Result<ReadView, StorageError> {
        if limits.max_item_bytes == 0 || limits.max_history_block_bytes == 0 {
            return Err(StorageError::InvalidRead(
                "read byte limits must be positive".into(),
            ));
        }
        // Publication does not need the writer's publish lock: a native read
        // transaction pins its own physical snapshot, and `committed_version`
        // is published only after the durable commit. Admission is serialized
        // with GC floor advancement on `read_pins` alone (see
        // `retire_before_locked`), so opening a view no longer contends with an
        // in-flight fsync.
        self.check_health()?;
        self.check_publisher()?;
        // Native direct reads open one transaction per physical shard. A single
        // shard uses the optimized current-read path; multiple shards merge
        // their native views per key (the global LSN is unchanged), so reads
        // stay memory-bounded with no in-memory mirror.
        let backends: Vec<Arc<dyn crate::StorageBackend>> = match tenant {
            Some(tenant) if self.resource_config().shard_count > 1 => self
                .backend(physical_shard_for(
                    Some(tenant),
                    self.resource_config().shard_count,
                ))
                .into_iter()
                .collect(),
            _ if self.resource_config().shard_count == 1 => {
                self.primary_backend().into_iter().collect()
            }
            _ => self.all_backends(),
        };
        let has_backends = !backends.is_empty();
        let mut views: Vec<Box<dyn crate::backend::StorageReadView>> =
            Vec::with_capacity(backends.len());
        for backend in &backends {
            views.push(backend.read_view()?);
        }
        let merged = MergedViews::new(views);
        let (visible, floor) = match (has_backends, tenant) {
            // A tenant-routed native transaction may lag the global watermark
            // because another tenant committed on a different shard. The
            // routed shard is still complete for this tenant, so retain the
            // store-wide snapshot identity and retention floor.
            (true, Some(_)) | (false, _) => (self.committed_version(), self.gc_watermark()),
            (true, None) => (merged.visible_ci()?, merged.history_floor()?),
        };
        let ci = requested.unwrap_or(visible);
        if ci > visible {
            return Err(StorageError::FutureSnapshot {
                requested: ci,
                visible,
            });
        }
        // Serialize the floor check and pin insert with GC floor advancement.
        // GC stores the new watermark before it reclaims, so any pin inserted
        // here after the floor moved is rejected; any pin inserted before is
        // included in GC's clamp.
        let mut pins = self.health.guard(self.read_pins.lock())?;
        self.check_publisher()?;
        let floor = floor.max(self.gc_watermark());
        if ci < floor {
            return Err(StorageError::SnapshotExpired {
                requested: ci,
                oldest: floor,
            });
        }
        // The pin carries a lease timestamp so GC can expire abandoned readers
        // by age instead of blocking history forever. Refresh it on every
        // admission: a long-lived entry with churning readers is still active
        // and must not be expired out from under them.
        let lease_ms = crate::wal::now_ms();
        let cancellation = ReadCancellation::default();
        self.health
            .guard(self.read_cancellations.lock())?
            .entry(ci)
            .or_default()
            .push(cancellation.clone());
        let pin_generation = {
            let entry = pins.entry(ci).or_insert_with(|| ReadPin {
                count: 0,
                lease_ms,
                generation: next_pin_generation(),
            });
            entry.count += 1;
            entry.lease_ms = lease_ms;
            entry.generation
        };
        let deriver = self.deriver();
        Ok(ReadView {
            health: Arc::clone(&self.health),
            pins: self.read_pins.clone(),
            cancellations: self.read_cancellations.clone(),
            ci,
            deriver,
            planner_stats: Arc::clone(&self.planner_stats),
            pin_generation,
            native_current: ci == visible,
            views: Some(merged),
            backends,
            limits,
            cancellation,
            parent: None,
            materialization_limit: 64 * 1024 * 1024,
        })
    }
}

impl Drop for ReadView {
    fn drop(&mut self) {
        // Bounded children share the root pin through their parent Arc.
        if self.parent.is_some() {
            return;
        }
        // Release the native pages before admitting physical compaction.
        self.views.take();
        self.backends.clear();
        let mut pins = self.health.cleanup(self.pins.lock());
        if let Some(entry) = pins.get_mut(&self.ci) {
            // Only release a pin this view actually admitted. If GC expired our
            // lease and a new generation was admitted at the same CI, touching
            // it here would drop a live reader's retention.
            if entry.generation == self.pin_generation {
                entry.count = entry.count.saturating_sub(1);
                if entry.count == 0 {
                    pins.remove(&self.ci);
                }
            }
        }
        let mut cancellations = self.health.cleanup(self.cancellations.lock());
        if let Some(entries) = cancellations.get_mut(&self.ci) {
            entries.retain(|entry| !entry.same_as(&self.cancellation));
            if entries.is_empty() {
                cancellations.remove(&self.ci);
            }
        }
    }
}

impl ReadView {
    /// The store's planner-statistics cache. Estimates only; exact
    /// answers never come from it.
    #[must_use]
    pub fn planner_stats_cache(&self) -> &crate::PlannerStats {
        &self.planner_stats
    }

    fn storage_view(&self) -> &ReadView {
        self.parent.as_deref().map_or(self, ReadView::storage_view)
    }

    #[cfg(test)]
    pub(crate) fn native_backend_count(&self) -> usize {
        self.backends.len()
    }

    /// Process-local Store identity, independent of tenant and commit index.
    /// Prevents accidentally pairing a view with another engine's catalogs.
    pub fn belongs_to(&self, store: &Store) -> bool {
        Arc::ptr_eq(&self.pins, &store.read_pins)
    }

    /// Commit index this view reads at.
    #[must_use]
    pub fn commit_index(&self) -> u64 {
        self.ci
    }

    /// The store's apply-time deriver, if one is installed.
    #[must_use]
    pub fn deriver(&self) -> Option<&Arc<dyn crate::Deriver>> {
        self.deriver.as_ref()
    }

    /// A cloneable handle to this view's cancellation flag.
    #[must_use]
    pub fn cancellation(&self) -> ReadCancellation {
        self.cancellation.clone()
    }

    /// Create a child view over this exact pinned snapshot with narrower
    /// per-item and materialized-prefix limits. The child has its own limits,
    /// while cancellation remains shared with the parent.
    #[must_use]
    pub fn with_materialization_limit(self: &Arc<Self>, bytes: usize) -> Arc<ReadView> {
        Arc::new(ReadView {
            health: Arc::clone(&self.health),
            pins: self.pins.clone(),
            cancellations: self.cancellations.clone(),
            ci: self.ci,
            deriver: self.deriver.clone(),
            planner_stats: Arc::clone(&self.planner_stats),
            pin_generation: self.pin_generation,
            native_current: self.native_current,
            views: None,
            backends: Vec::new(),
            limits: ReadLimits {
                max_item_bytes: self.limits.max_item_bytes.min(bytes),
                max_history_block_bytes: self.limits.max_history_block_bytes,
            },
            cancellation: self.cancellation.clone(),
            parent: Some(self.clone()),
            materialization_limit: self.materialization_limit.min(bytes),
        })
    }

    /// Check cancellation between non-storage operations (for example,
    /// serializing output) without performing an otherwise unnecessary read.
    ///
    /// # Errors
    /// Returns [`StorageError::Cancelled`] if the view has been cancelled.
    pub fn check_cancelled(&self) -> Result<(), StorageError> {
        self.health.check()?;
        self.cancellation.check()
    }

    /// Read a single key at this view's snapshot.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the view is cancelled, the item exceeds the
    /// read limit, or the backend read fails.
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.read_value(key, ScanProjection::KeyValue)
    }

    /// Transitional materializing helper for consumers migrating to cursors.
    /// The local ceiling is not a substitute for shared query admission.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the view is cancelled, the item exceeds the
    /// read limit, or the materialized result exceeds the 64 MiB ceiling.
    ///
    /// # Panics
    /// Panics if a key/value projection batch unexpectedly yields a row
    /// without a value.
    pub fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        self.materialize_scan(ScanBounds::prefix(prefix))
    }

    /// Materialize every key/value pair with `start <= key < end`, in key
    /// order. An empty `end` means unbounded above. Shares the prefix path's
    /// aggregate materialization ceiling.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the view is cancelled, the item exceeds the
    /// read limit, or the materialized result exceeds the 64 MiB ceiling.
    pub fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let bounds = ScanBounds {
            start: Bound::Included(start.to_vec()),
            end: if end.is_empty() {
                Bound::Unbounded
            } else {
                Bound::Excluded(end.to_vec())
            },
        };
        self.materialize_scan(bounds)
    }

    fn materialize_scan(
        &self,
        bounds: ScanBounds,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let mut cursor = self.scan(bounds, ScanProjection::KeyValue)?;
        let mut rows = Vec::new();
        let mut bytes = 0usize;
        loop {
            // Keep each fetch within the remaining aggregate cap so a large
            // prefix cannot transiently materialize beyond the child budget.
            let remaining = self.materialization_limit.saturating_sub(bytes);
            let batch = cursor.next_batch(1024, remaining.max(1))?;
            bytes = bytes.checked_add(batch.bytes).ok_or_else(|| {
                StorageError::ResourceExhausted("materialized read size overflow".into())
            })?;
            if bytes > self.materialization_limit {
                return Err(StorageError::ResourceExhausted(
                    "materialized read exceeds configured limit; use a cursor".into(),
                ));
            }
            rows.extend(
                batch
                    .rows
                    .into_iter()
                    .map(|row| (row.key, row.value.expect("key/value projection"))),
            );
            if batch.done {
                return Ok(rows);
            }
        }
    }

    fn read_value(
        &self,
        key: &[u8],
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        self.read_value_with_limits(key, projection, self.limits)
    }

    fn read_value_with_limits(
        &self,
        key: &[u8],
        projection: ScanProjection,
        limits: ReadLimits,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        self.health.check()?;
        self.cancellation.check()?;
        limits.check_item(key.len(), 0)?;
        let storage = self.storage_view();
        let views = storage
            .views
            .as_ref()
            .expect("root read view owns backend views");
        // The `current` fast path is only valid when the backend physically
        // pins the snapshot (redb). The live memory maps do not, so a view must
        // resolve reads by its `ci` even at the latest commit.
        let current = self.native_current && views.snapshot_pinned();
        views.read_value(key, self.ci, current, limits, projection)
    }

    fn next_key(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        self.health.check()?;
        self.cancellation.check()?;
        let storage = self.storage_view();
        let views = storage
            .views
            .as_ref()
            .expect("root read view owns backend views");
        views.next_key(lower, upper, self.limits, self.native_current)
    }

    /// `ScanCursor`'s v2 native-iterator fast path (B7 follow-up): a real
    /// backend range cursor over `(lower, upper)` — one seek for the whole
    /// scan instead of one per row (`next_key` + `read_value_with_limits`,
    /// still the fallback). `Ok(None)` when this view cannot use the
    /// current-value fast path (same gate `read_value_with_limits`'s
    /// `current` flag already needs: a non-current/pinned-older snapshot,
    /// or a backend whose live maps aren't physically pinned pages), or
    /// when a backend has no such cursor.
    fn native_range(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        projection: ScanProjection,
    ) -> Result<
        Option<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + '_>>,
        StorageError,
    > {
        self.health.check()?;
        self.cancellation.check()?;
        let storage = self.storage_view();
        let views = storage
            .views
            .as_ref()
            .expect("root read view owns backend views");
        // Same gate as `read_value_with_limits`'s `current` fast path: valid
        // only when this view's ci is the backend's live, physically pinned
        // current state.
        if !(self.native_current && views.snapshot_pinned()) {
            return Ok(None);
        }
        Ok(views
            .native_range(lower, upper, self.limits, projection)?
            .map(|merged| Box::new(merged) as Box<dyn Iterator<Item = _> + '_>))
    }

    /// Open a forward scan cursor over `bounds` with the given `projection`.
    ///
    /// # Errors
    /// Returns [`StorageError::Cancelled`] if the view is cancelled,
    /// [`StorageError::InvalidRead`] if the bounds are inverted, or
    /// [`StorageError::ResourceExhausted`] if a bound key exceeds the read
    /// limit.
    pub fn scan(
        &self,
        bounds: ScanBounds,
        projection: ScanProjection,
    ) -> Result<ScanCursor<'_>, StorageError> {
        self.health.check()?;
        self.cancellation.check()?;
        for bound in [&bounds.start, &bounds.end] {
            if let Bound::Included(key) | Bound::Excluded(key) = bound {
                self.limits.check_item(key.len(), 0)?;
            }
        }
        let empty = bounds.is_empty()?;
        Ok(ScanCursor {
            view: self,
            lower: bounds.start,
            upper: bounds.end,
            projection,
            done: empty,
            native: None,
            native_checked: false,
            pending_native: Vec::new(),
        })
    }
}

/// Byte-ordered scan bounds as an inclusive/exclusive pair.
#[derive(Clone, Debug)]
pub struct ScanBounds {
    /// Lower bound of the scan.
    pub start: Bound<Vec<u8>>,
    /// Upper bound of the scan.
    pub end: Bound<Vec<u8>>,
}

impl ScanBounds {
    /// Bounds covering every key under `prefix`.
    #[must_use]
    pub fn prefix(prefix: &[u8]) -> Self {
        let (start, end) = prefix_range(prefix);
        Self {
            start: Bound::Included(start),
            end: if end.is_empty() {
                Bound::Unbounded
            } else {
                Bound::Excluded(end)
            },
        }
    }

    fn is_empty(&self) -> Result<bool, StorageError> {
        use Bound::{Excluded, Included};
        match (&self.start, &self.end) {
            (Included(a) | Excluded(a), Included(b) | Excluded(b)) => {
                if a > b {
                    return Err(StorageError::InvalidRead("start exceeds end".into()));
                }
                Ok(a == b
                    && (!matches!(self.start, Included(_)) || !matches!(self.end, Included(_))))
            }
            _ => Ok(false),
        }
    }
}

/// Which columns a scan should materialize for each row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanProjection {
    /// Return keys only; row values are empty.
    Keys,
    /// Return both key and value.
    KeyValue,
}

/// One materialized scan row.
#[derive(Debug, PartialEq, Eq)]
pub struct ScanRow {
    /// Row key.
    pub key: Vec<u8>,
    /// None for keys-only projection; an empty stored value is Some(vec![]).
    pub value: Option<Vec<u8>>,
}

/// One batch of scan rows together with its byte accounting and done flag.
#[derive(Debug)]
pub struct ScanBatch {
    /// Materialized rows in ascending key order.
    pub rows: Vec<ScanRow>,
    /// Copied key/value bytes, excluding allocation/container overhead.
    pub bytes: usize,
    /// False may require another call to discover exhaustion at a batch edge.
    pub done: bool,
}

/// Forward cursor tied to one protected view. No full candidate collection.
/// v2 (B7 follow-up): when the view's current-value fast path is valid
/// (see [`ReadView::native_range`]), one real backend cursor serves the
/// whole scan — built lazily on the first [`Self::next_batch`] call and
/// held across calls (`native`/`native_checked`). Otherwise falls back to
/// v1's ordered seek-per-row (`next_key` + `read_value_with_limits`).
/// Each batch plus at most one bounded lookahead row is allocated. Callers
/// must release consumed batches; shared outstanding-byte admission is P01.
pub struct ScanCursor<'view> {
    view: &'view ReadView,
    lower: Bound<Vec<u8>>,
    upper: Bound<Vec<u8>>,
    projection: ScanProjection,
    done: bool,
    /// `None` until the first `next_batch` call decides eligibility; stays
    /// `None` for the cursor's lifetime when ineligible or unavailable
    /// (v1 fallback), `Some` once a native cursor is built (v2).
    native: Option<
        std::iter::Peekable<
            Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'view>,
        >,
    >,
    native_checked: bool,
    /// Undo buffer for the native path's error contract — see
    /// `next_batch_native`.
    pending_native: Vec<ScanRow>,
}

impl ScanCursor<'_> {
    /// Fetch up to `row_limit` rows bounded by `byte_limit` copied bytes.
    ///
    /// # Errors
    /// Returns [`StorageError::InvalidRead`] for zero limits,
    /// [`StorageError::Cancelled`] if the view was cancelled, or
    /// [`StorageError::ResourceExhausted`] if a single row exceeds
    /// `byte_limit`.
    pub fn next_batch(
        &mut self,
        row_limit: usize,
        byte_limit: usize,
    ) -> Result<ScanBatch, StorageError> {
        let before = self.lower.clone();
        let was_done = self.done;
        let result = self.next_batch_inner(row_limit, byte_limit);
        if result.is_err() {
            // No batch was delivered, so none of its rows are consumed.
            self.lower = before;
            self.done = was_done;
        }
        result
    }

    fn next_batch_inner(
        &mut self,
        row_limit: usize,
        byte_limit: usize,
    ) -> Result<ScanBatch, StorageError> {
        self.view.check_cancelled()?;
        if row_limit == 0 || byte_limit == 0 {
            return Err(StorageError::InvalidRead(
                "batch limits must be positive".into(),
            ));
        }
        if !self.native_checked {
            self.native = self
                .view
                .native_range(&self.lower, &self.upper, self.projection)?
                .map(std::iter::Iterator::peekable);
            self.native_checked = true;
        }
        if self.native.is_some() {
            return self.next_batch_native(row_limit, byte_limit);
        }
        let mut batch = ScanBatch {
            rows: Vec::new(),
            bytes: 0,
            done: self.done,
        };
        while !self.done && batch.rows.len() < row_limit {
            let Some(key) = self.view.next_key(&self.lower, &self.upper)? else {
                self.done = true;
                break;
            };
            let remaining = byte_limit - batch.bytes;
            let mut limits = self.view.limits;
            let bounded_by_batch = remaining < limits.max_item_bytes;
            limits.max_item_bytes = limits.max_item_bytes.min(remaining);
            let Some(value) = (match self
                .view
                .read_value_with_limits(&key, self.projection, limits)
            {
                Ok(value) => value,
                Err(_error) if bounded_by_batch && !batch.rows.is_empty() => break,
                Err(error) => return Err(error),
            }) else {
                self.lower = Bound::Excluded(key);
                continue;
            };
            let bytes = key.len() + value.len(); // checked by read_value
            if bytes > byte_limit - batch.bytes {
                if batch.rows.is_empty() {
                    return Err(StorageError::ResourceExhausted(
                        "scan row exceeds batch byte limit".into(),
                    ));
                }
                break; // keep continuation before the unconsumed row
            }
            self.lower = Bound::Excluded(key.clone());
            batch.bytes += bytes;
            batch.rows.push(ScanRow {
                key,
                value: (self.projection == ScanProjection::KeyValue).then_some(value),
            });
            // An inclusive upper bound can leave equal excluded/included
            // bounds after the last row; avoid asking a backend to traverse it.
            if matches!(&self.upper, Bound::Included(end) if batch.rows.last().is_some_and(|row| &row.key == end))
            {
                self.done = true;
            }
        }
        batch.done = self.done;
        Ok(batch)
    }

    /// v2 fast path: pull rows from the already-built native cursor
    /// (`self.native`) instead of seeking per row. A row that doesn't fit
    /// the *remaining* per-batch byte budget is left un-consumed (peeked,
    /// not advanced) and deferred to the next call — mirroring v1's own
    /// "oversized row" deferral — because unlike v1's position-based
    /// `self.lower`, this cursor's position lives inside the iterator
    /// itself: a row must never be consumed and then discarded by an
    /// error return, or [`Self::next_batch`]'s rollback-and-retry would
    /// silently skip it (the iterator can't be "seeked back").
    fn next_batch_native(
        &mut self,
        row_limit: usize,
        byte_limit: usize,
    ) -> Result<ScanBatch, StorageError> {
        let mut batch = ScanBatch {
            rows: Vec::new(),
            bytes: 0,
            done: self.done,
        };
        // Rows a *previous* call already consumed from the iterator but
        // could not deliver (it hit a hard error on a later row and had to
        // propagate that error — see below) are delivered first. Unlike
        // v1's position-based retry, this iterator cannot be "seeked
        // back", so an already-consumed row that isn't returned here would
        // be lost forever; `pending_native` is the explicit undo buffer
        // that makes this cursor's error contract match v1's exactly
        // (a failed batch still delivers everything it actually read on a
        // later call) despite the underlying iterator being stateful.
        while !self.pending_native.is_empty() && batch.rows.len() < row_limit {
            let bytes = self.pending_native[0].key.len()
                + self.pending_native[0].value.as_ref().map_or(0, Vec::len);
            if bytes > byte_limit - batch.bytes {
                if batch.rows.is_empty() {
                    // Every pending row already passed the absolute
                    // `max_item_bytes` check when it was first read; the
                    // only way it can still be too big is a tighter
                    // `byte_limit` on *this* call. Matches v1's own
                    // "first row already exceeds this call's budget" case
                    // exactly — an empty `Ok` here would silently never
                    // make progress instead of surfacing that.
                    return Err(StorageError::ResourceExhausted(
                        "scan row exceeds batch byte limit".into(),
                    ));
                }
                break;
            }
            batch.bytes += bytes;
            batch.rows.push(self.pending_native.remove(0));
        }
        if !self.pending_native.is_empty() {
            batch.done = false;
            return Ok(batch);
        }
        let iter = self
            .native
            .as_mut()
            .expect("next_batch_native only called when native is Some");
        while !self.done && batch.rows.len() < row_limit {
            let bytes = match iter.peek() {
                Some(Ok((key, value))) => key.len() + value.len(),
                Some(Err(error)) => {
                    if !batch.rows.is_empty() {
                        // A hard error (e.g. an absolute per-item size
                        // violation) on a row *after* some already-consumed
                        // rows in this same call: stash them so the next
                        // call delivers them instead of losing them, then
                        // propagate — matching v1's "this call errors, but
                        // the rows it already read still arrive later".
                        self.pending_native = std::mem::take(&mut batch.rows);
                    }
                    return Err(error.clone());
                }
                None => {
                    self.done = true;
                    break;
                }
            };
            let remaining = byte_limit - batch.bytes;
            if bytes > remaining {
                if batch.rows.is_empty() {
                    return Err(StorageError::ResourceExhausted(
                        "scan row exceeds batch byte limit".into(),
                    ));
                }
                break; // left un-consumed (peeked only) for the next call
            }
            let Some(Ok((key, value))) = iter.next() else {
                unreachable!("peek just confirmed Some(Ok(_))")
            };
            batch.bytes += bytes;
            // Same inclusive-upper-bound edge v1 handles: avoid asking the
            // backend to traverse past it.
            let at_upper_edge = matches!(&self.upper, Bound::Included(end) if &key == end);
            batch.rows.push(ScanRow {
                key,
                value: (self.projection == ScanProjection::KeyValue).then_some(value),
            });
            if at_upper_edge {
                self.done = true;
            }
        }
        batch.done = self.done;
        Ok(batch)
    }
}
