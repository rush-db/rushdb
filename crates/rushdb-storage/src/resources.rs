//! Shared managed-byte accounting. Reservations are ownership tokens, not
//! allocations, an RSS limit, or a substitute for charging each consumer.
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc, Mutex, MutexGuard,
};
use thiserror::Error;

/// Errors from managed-byte accounting: invalid budgets, exhausted limits,
/// and reservation ownership violations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResourceError {
    /// The requested budget or limit is invalid.
    #[error("invalid resource budget: {0}")]
    Invalid(&'static str),
    /// A budget could not admit the requested bytes.
    #[error("resource budget `{budget}` exhausted: requested {requested} bytes, used {used}, limit {limit}")]
    Exhausted {
        /// Budget that refused the request.
        budget: &'static str,
        /// Bytes requested.
        requested: usize,
        /// Bytes currently charged.
        used: usize,
        /// Configured byte limit.
        limit: usize,
    },
    /// The two reservations belong to different resource trees.
    #[error("reservation belongs to a different resource tree")]
    DifferentTree,
    /// A reservation operation would exceed the bytes it owns.
    #[error("reservation operation exceeds its owned bytes")]
    InvalidOwnership,
}

/// Point-in-time usage of one budget: configured limit, current and peak use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetSnapshot {
    /// Configured byte limit.
    pub limit: usize,
    /// Bytes currently charged.
    pub used: usize,
    /// Highest usage observed.
    pub peak: usize,
}

/// Admission error for a [`FairAdmission::acquire`] attempt. A successful call
/// returns the owned [`ByteReservation`]; failure returns one of these without
/// charging bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionError {
    /// The deadline expired before bytes were available; no charge was made.
    TimedOut,
    /// The gate was cancelled before admission; no charge was made.
    Cancelled,
    /// The budget could not admit the request at this instant.
    Exhausted,
}

/// Monotonic ticket source for fair-admission waiters.
static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

struct Node {
    name: &'static str,
    limit: usize,
    used: AtomicUsize,
    peak: AtomicUsize,
    depth: usize,
    parent: Option<Arc<Node>>,
    ledger: Arc<Mutex<()>>,
}

/// Child usage is included in every ancestor; creating children does not
/// multiply the parent limit. Names are static component names, not tenant
/// identifiers or unbounded metric labels. Dropping the last child handle and
/// reservation releases its metadata without retaining a central registry.
#[derive(Clone)]
pub struct ByteBudget(Arc<Node>);

impl ByteBudget {
    /// Create a root budget labelled `name` with the given byte `limit`.
    ///
    /// # Errors
    /// Returns [`ResourceError::Invalid`] if `limit` is zero.
    pub fn new(name: &'static str, limit: usize) -> Result<Self, ResourceError> {
        if limit == 0 {
            return Err(ResourceError::Invalid("limit must be positive"));
        }
        Ok(Self(Arc::new(Node {
            name,
            limit,
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            depth: 0,
            parent: None,
            ledger: Arc::new(Mutex::new(())),
        })))
    }

    /// Create a child budget whose usage is also charged to this budget.
    ///
    /// # Errors
    /// Returns [`ResourceError::Invalid`] if `limit` is zero or exceeds the
    /// parent limit, or if nesting would exceed 32 levels.
    pub fn child(&self, name: &'static str, limit: usize) -> Result<Self, ResourceError> {
        if limit == 0 || limit > self.0.limit {
            return Err(ResourceError::Invalid(
                "child limit must be positive and no greater than parent",
            ));
        }
        if self.0.depth >= 32 {
            return Err(ResourceError::Invalid("budget nesting exceeds 32 levels"));
        }
        Ok(Self(Arc::new(Node {
            name,
            limit,
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            depth: self.0.depth + 1,
            parent: Some(self.0.clone()),
            ledger: self.0.ledger.clone(),
        })))
    }

    /// Serialize *growth* (validate + add) across the whole budget tree.
    /// Releases and reads are lock-free atomics, so this lock is held only on
    /// the admission path and never blocks a `Drop`. Recover a poisoned mutex
    /// so a panicked admission cannot wedge future reservations.
    fn lock(&self) -> MutexGuard<'_, ()> {
        self.0
            .ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn ancestors(&self) -> impl Iterator<Item = &Node> {
        std::iter::successors(Some(self.0.as_ref()), |node| node.parent.as_deref())
    }

    fn validate_add(&self, bytes: usize) -> Result<(), ResourceError> {
        for node in self.ancestors() {
            let used = node.used.load(Ordering::Acquire);
            if bytes > node.limit - used {
                return Err(ResourceError::Exhausted {
                    budget: node.name,
                    requested: bytes,
                    used,
                    limit: node.limit,
                });
            }
        }
        Ok(())
    }

    fn add(&self, bytes: usize) {
        for node in self.ancestors() {
            let used = node.used.fetch_add(bytes, Ordering::AcqRel) + bytes;
            node.peak.fetch_max(used, Ordering::Relaxed);
        }
    }

    /// Release a charge. Atomic and lock-free: a release can only lower usage,
    /// so it cannot race a concurrent admission into over-subscription. Callers
    /// hold `ledger` only when growing.
    fn subtract(&self, bytes: usize) {
        for node in self.ancestors() {
            node.used.fetch_sub(bytes, Ordering::Release);
        }
    }

    /// Snapshot this budget's limit, current usage, and peak usage.
    #[must_use]
    pub fn snapshot(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            limit: self.0.limit,
            used: self.0.used.load(Ordering::Acquire),
            peak: self.0.peak.load(Ordering::Relaxed),
        }
    }

    /// Available in this budget AND every parent at one accounting instant.
    ///
    /// # Panics
    /// Panics if the ancestor chain is empty, which cannot happen because a
    /// budget always has at least itself.
    #[must_use]
    pub fn available(&self) -> usize {
        self.ancestors()
            .map(|node| node.limit - node.used.load(Ordering::Acquire))
            .min()
            .unwrap()
    }

    /// Immediate admission, before allocation. Queueing, deadlines and fair
    /// scheduling are deliberately separate work; this never blocks for bytes.
    ///
    /// # Errors
    /// Returns [`ResourceError::Exhausted`] if this budget or any ancestor
    /// cannot admit `bytes`.
    pub fn try_reserve(&self, bytes: usize) -> Result<ByteReservation, ResourceError> {
        let _guard = self.lock();
        self.validate_add(bytes)?;
        self.add(bytes);
        Ok(ByteReservation {
            budget: self.clone(),
            bytes,
        })
    }
}

/// Not Clone: exactly one owner releases each charge, including during unwind.
/// Move this with the allocated buffer; do not drop it while retaining data.
pub struct ByteReservation {
    budget: ByteBudget,
    bytes: usize,
}

impl ByteReservation {
    /// Bytes currently owned by this reservation.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Change the owned byte count, charging or releasing the difference.
    ///
    /// # Errors
    /// Returns [`ResourceError::Exhausted`] if growing the reservation cannot
    /// be admitted.
    pub fn resize(&mut self, bytes: usize) -> Result<(), ResourceError> {
        if bytes > self.bytes {
            // Growing must be atomic against other admissions.
            let _guard = self.budget.lock();
            self.budget.validate_add(bytes - self.bytes)?;
            self.budget.add(bytes - self.bytes);
        } else {
            // Shrinking only releases; lock-free.
            self.budget.subtract(self.bytes - bytes);
        }
        self.bytes = bytes;
        Ok(())
    }

    /// Split ownership without releasing or charging the same bytes twice.
    ///
    /// # Errors
    /// Returns [`ResourceError::InvalidOwnership`] if `bytes` exceeds the
    /// owned amount.
    pub fn split(&mut self, bytes: usize) -> Result<Self, ResourceError> {
        if bytes > self.bytes {
            return Err(ResourceError::InvalidOwnership);
        }
        self.bytes -= bytes;
        Ok(Self {
            budget: self.budget.clone(),
            bytes,
        })
    }

    /// Transfer a live charge atomically, including shared-parent accounting.
    /// Failure leaves the source reservation and all counters unchanged.
    ///
    /// # Errors
    /// Returns [`ResourceError::DifferentTree`] if `destination` is in another
    /// tree, or [`ResourceError::Exhausted`] if it cannot admit the bytes.
    pub fn transfer(&mut self, destination: &ByteBudget) -> Result<(), ResourceError> {
        if !Arc::ptr_eq(&self.budget.0.ledger, &destination.0.ledger) {
            return Err(ResourceError::DifferentTree);
        }
        {
            let _guard = self.budget.lock();
            self.budget.subtract(self.bytes);
            if let Err(error) = destination.validate_add(self.bytes) {
                self.budget.add(self.bytes);
                return Err(error);
            }
            destination.add(self.bytes);
        }
        self.budget = destination.clone();
        Ok(())
    }
}

impl Drop for ByteReservation {
    fn drop(&mut self) {
        // Lock-free release: this is the hottest accounting path (every
        // reservation, cache eviction, and rollback).
        self.budget.subtract(self.bytes);
    }
}

/// Bounded fair admission gate: serializes requests for bytes from a
/// budget and admits them in arrival order when capacity frees up, honoring a
/// caller deadline and an external cancellation flag. `try_reserve` remains
/// the non-blocking primitive; this adds bounded queueing, deadlines, and
/// cancellation for work that must wait rather than fail immediately (e.g.
/// group-commit staging, transaction buffers).
///
/// Fairness is FIFO by arrival; a cancelled waiter releases its slot so a
/// blocked call cannot be starved by a never-completing predecessor.
pub struct FairAdmission {
    budget: ByteBudget,
    /// FIFO of queued waiters, each tracking the bytes it wants and its
    /// deadline (ms since epoch) / cancellation flag.
    queue: Mutex<std::collections::VecDeque<Waiter>>,
    /// Broadcast when bytes are released or a waiter is removed.
    changed: std::sync::Condvar,
}

struct Waiter {
    deadline_ms: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    ticket: u64,
}

impl FairAdmission {
    /// Create a fair-admission gate over `budget`.
    #[must_use]
    pub fn new(budget: ByteBudget) -> Self {
        Self {
            budget,
            queue: Mutex::new(std::collections::VecDeque::new()),
            changed: std::sync::Condvar::new(),
        }
    }

    /// Try to admit immediately without queueing (non-blocking).
    ///
    /// # Errors
    /// Returns [`ResourceError::Exhausted`] if the budget cannot admit
    /// `bytes` right now.
    pub fn try_acquire(&self, bytes: usize) -> Result<ByteReservation, ResourceError> {
        self.budget.try_reserve(bytes)
    }

    /// Queue until the budget admits `bytes`, the deadline (ms since epoch)
    /// passes, or the caller's cancellation flag is set. FIFO: a request ahead
    /// in the queue is served first; it may time out and release its slot.
    ///
    /// `cancelled` is an `Arc` the caller shares with this queue; setting it
    /// wakes any queued waiter that shares it and removes it from the queue.
    ///
    /// # Errors
    /// Returns [`AdmissionError::TimedOut`] on deadline expiry,
    /// [`AdmissionError::Cancelled`] when the flag is observed, or
    /// [`AdmissionError::Exhausted`] if the budget cannot admit `bytes`.
    pub fn acquire(
        &self,
        bytes: usize,
        deadline_ms: u64,
        cancelled: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<ByteReservation, AdmissionError> {
        // Fast path: enough capacity now.
        if self.budget.available() >= bytes {
            return self
                .budget
                .try_reserve(bytes)
                .map_err(|_| AdmissionError::Exhausted);
        }
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(AdmissionError::Cancelled);
        }
        if now_ms() >= deadline_ms {
            return Err(AdmissionError::TimedOut);
        }

        // Each waiter carries a unique ticket identity so position tracking is
        // robust to earlier waiters being purged.
        let ticket_id = NEXT_TICKET.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let waiter = Waiter {
            deadline_ms,
            cancelled: Arc::clone(cancelled),
            ticket: ticket_id,
        };
        let mut guard = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.push_back(waiter);

        loop {
            // Drop expired or cancelled waiters that are NOT us (we handle our
            // own timeout/cancel below).
            let my_ticket = ticket_id;
            let now = now_ms();
            let mut changed = false;
            guard.retain(|w| {
                if w.ticket == my_ticket {
                    true // keep ourselves
                } else if w.cancelled.load(std::sync::atomic::Ordering::Relaxed)
                    || w.deadline_ms <= now
                {
                    changed = true;
                    false
                } else {
                    true
                }
            });
            if changed {
                self.changed.notify_all();
            }

            // Am I the head, and does the budget have capacity?
            let is_head = guard.front().is_some_and(|w| w.ticket == my_ticket);
            if is_head && self.budget.available() >= bytes {
                if let Ok(res) = self.budget.try_reserve(bytes) {
                    guard.pop_front();
                    self.changed.notify_all();
                    return Ok(res);
                }
                guard.pop_front();
                self.changed.notify_all();
                return Err(AdmissionError::Exhausted);
            }

            // Own deadline / cancellation check.
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                guard.retain(|w| w.ticket != my_ticket);
                self.changed.notify_all();
                return Err(AdmissionError::Cancelled);
            }
            if now_ms() >= deadline_ms {
                guard.retain(|w| w.ticket != my_ticket);
                self.changed.notify_all();
                return Err(AdmissionError::TimedOut);
            }

            let (guard2, _timeout) = self
                .changed
                .wait_timeout(guard, std::time::Duration::from_millis(2))
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard = guard2;
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Initial embedded storage configuration, not a whole-process RSS promise.
#[derive(Debug, Clone, Copy)]
pub struct StorageResources {
    /// Total managed byte envelope for the store.
    pub managed_limit_bytes: usize,
    /// Bytes reserved for the redb page cache.
    pub page_cache_bytes: usize,
    /// Bytes reserved for transaction staging.
    pub transaction_pool_bytes: usize,
    /// Logical durability contract applied to writes.
    pub durability: Durability,
    /// Number of physical storage shards (independent redb files / writers).
    /// Default 1 preserves today's single-file layout. A value > 1 enables the
    /// per-shard backend array; the writer currently targets shard 0 until the
    /// per-shard commit-index split (W4 phase 2) lands.
    pub shard_count: usize,
    /// Acknowledge that `shard_count > 1` is experimental. Multi-shard commit
    /// groups are persisted as independent redb transactions per shard, so a
    /// crash between shard commits is **not** atomic and can leave a torn
    /// global commit sequence. Until a redo journal makes the group atomic,
    /// multi-shard stores are refused unless this is set explicitly.
    pub experimental_multi_shard: bool,
    /// Max commits coalesced into one group-commit redb write transaction
    /// (P04 bounded writer). 0 = no count cap.
    pub max_group_count: usize,
    /// Max staged op-set bytes coalesced into one group (P04 bounded writer).
    /// 0 = no byte cap.
    pub max_group_bytes: usize,
    /// Max time the group-commit publisher lingers before flushing, giving
    /// concurrent commits a window to join one fsync (V2 WAL batching tuning).
    /// 0 = no linger (flush immediately).
    pub max_group_delay_ms: u64,
    /// Target size of one segmented WAL file before it is sealed and a new
    /// segment starts (log-first). Must be at least 64 KiB.
    pub log_segment_bytes: u64,
    /// Bytes reserved for byte-accounted logical caches (decoded records,
    /// adjacency, property dictionary). Charged to the shared budget tree so
    /// cache growth is bounded and visible to the governor. 0 = disabled.
    pub logical_cache_bytes: usize,
    /// Bytes reserved for concurrent request *phases* (query planning and
    /// execution admission). Each `ExecutionContext` and each `DataFusion`
    /// session reserves a fixed per-request token from this pool, so the pool
    /// size sets the process-wide concurrency ceiling. Decoupled from
    /// `logical_cache_bytes` so a small cache budget does not accidentally cap
    /// concurrency at a handful of requests. 0 = phase admission disabled.
    pub phase_budget_bytes: usize,
    /// Per-query working-set cap in bytes. When non-zero, a structured query
    /// that would materialize more than this returns `resource exhausted`
    /// instead of ballooning RSS (config #3). 0 = off.
    pub query_memory_max_bytes: u64,
    /// Largest record population a filter without a usable index may check
    ///; a query above it is refused at planning with the index to
    /// declare. 0 = off.
    pub unindexed_walk_max_owners: u64,
    /// Maximum time to wait for the admission lock before failing a commit
    /// with `resource exhausted` (retryable). 0 = wait indefinitely.
    pub lock_acquisition_timeout_ms: u64,
    /// Max bytes of one commit's expanded write set (encoded op-set plus the
    /// serialized `CommitBatch` command payload) before the commit index is
    /// reserved (large-transaction support). A single oversized
    /// ingest/mutate request is rejected here, cleanly and without fencing
    /// the store, instead of burning a commit index and only failing much
    /// later when a 64 MiB-capped WAL record can't hold it. Deliberately a
    /// generous, product-level ceiling (hundreds of MB to low GB) — the
    /// WAL's own `log_store::MAX_RECORD_BYTES` stays fixed, and a commit
    /// under this ceiling but over that one is transparently chained across
    /// several physical records (see `wal::encode_chained_ops_record`).
    /// Above this ceiling, the expectation is an external ETL/bulk-load
    /// pipeline, not the live transactional path (matching Postgres `COPY`,
    /// Neo4j's offline bulk importer, MongoDB's `mongoimport`).
    pub max_expanded_commit_bytes: usize,
    /// Byte budget of the planner-statistics cache, a child of the
    /// logical cache budget when that is enabled. 0 = no caching (every
    /// estimate walks the value dictionary).
    pub planner_stats_bytes: usize,
    /// Seconds a planner-statistics entry may sit unused before eviction.
    pub planner_stats_idle_seconds: u64,
    /// Commits after which a cached planner-statistics entry is recomputed.
    pub planner_stats_max_age_commits: u64,
    /// How long a retry receipt answers a repeated commit key. A
    /// retry after the window executes as a new commit.
    pub idempotency_window_seconds: u64,
}

/// Logical durability contract (research D10). `Local` is the default and
/// guarantees the commit is durable on this node before it is acknowledged.
/// `Async` does **not** fsync: with redb 4.x the commit is not persisted until
/// a later `Local` commit or checkpoint, so an acknowledged `Async` commit can
/// be lost on a crash, power loss, or hard kill. It is intended only for
/// re-importable bulk input, not as a weaker consistency mode. `Memory` is only
/// meaningful for in-memory stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Durability {
    /// In-memory only; no durable backend.
    Memory,
    /// Not fsynced and not persisted until a later `Local` commit or
    /// checkpoint; an acknowledged commit can be lost on any crash. For
    /// re-importable bulk input only.
    Async,
    /// Durable on this node before acknowledgement (default).
    #[default]
    Local,
}

impl Default for StorageResources {
    fn default() -> Self {
        Self {
            managed_limit_bytes: 512 * 1024 * 1024,
            page_cache_bytes: 128 * 1024 * 1024,
            transaction_pool_bytes: 128 * 1024 * 1024,
            durability: Durability::Local,
            shard_count: 1,
            experimental_multi_shard: false,
            max_group_count: 0,
            max_group_bytes: 0,
            max_group_delay_ms: 0,
            log_segment_bytes: 64 * 1024 * 1024,
            // 128 MiB holds a useful ANN/Tantivy generation set; the previous
            // 32 MiB shared pool made ANN builds fail at a few thousand
            // vectors at realistic dimensions.
            logical_cache_bytes: 128 * 1024 * 1024,
            // 256 MiB / 8 MiB per request = 32 concurrent phase tokens by
            // default, instead of the previous 4 that reused the 32 MiB cache
            // budget. The parent managed limit still bounds the total.
            phase_budget_bytes: 256 * 1024 * 1024,
            query_memory_max_bytes: 0,
            unindexed_walk_max_owners: 0,
            lock_acquisition_timeout_ms: 0,
            // 512 MiB: generous relative to typical single-request ingest
            // sizes, comfortably bounded relative to process memory, and an
            // explicit product choice (not derived from the WAL's own 64 MiB
            // per-record ceiling, which chaining already decouples this
            // from).
            max_expanded_commit_bytes: 512 * 1024 * 1024,
            planner_stats_bytes: 32 * 1024 * 1024,
            planner_stats_idle_seconds: 600,
            planner_stats_max_age_commits: 10_000,
            idempotency_window_seconds: 24 * 3600,
        }
    }
}

impl StorageResources {
    /// Validate the configuration envelope.
    ///
    /// # Errors
    /// Returns [`ResourceError::Invalid`] when any size is zero or out of
    /// range, the cache and transaction pools do not fit with headroom in the
    /// managed envelope, `Memory` durability is configured, or the shard count
    /// is outside `1..=256`.
    pub fn validate(&self) -> Result<(), ResourceError> {
        if self.managed_limit_bytes == 0
            || self.page_cache_bytes < 64 * 1024
            || self.transaction_pool_bytes == 0
            || self
                .page_cache_bytes
                .checked_add(self.transaction_pool_bytes)
                .is_none_or(|total| total >= self.managed_limit_bytes)
        {
            return Err(ResourceError::Invalid(
                "cache must be at least 64 KiB; positive cache/transaction pools must fit with headroom in the managed envelope",
            ));
        }
        if matches!(self.durability, Durability::Memory) {
            return Err(ResourceError::Invalid(
                "Memory durability is for in-memory stores; use new_in_memory() instead of a configured redb cache",
            ));
        }
        if self.shard_count == 0 {
            return Err(ResourceError::Invalid("shard-count must be >= 1"));
        }
        if self.shard_count > 256 {
            return Err(ResourceError::Invalid("shard-count must be <= 256"));
        }
        if self.shard_count > 1 && !self.experimental_multi_shard {
            return Err(ResourceError::Invalid(
                "shard-count > 1 is experimental: multi-shard commit groups are not crash-atomic \
                 (independent per-shard redb transactions). Set experimental_multi_shard / \
                 --experimental-sharding to acknowledge, or use shard-count 1",
            ));
        }
        if self.log_segment_bytes < 64 * 1024 {
            return Err(ResourceError::Invalid(
                "log-segment-bytes must be at least 64 KiB",
            ));
        }
        if self.max_expanded_commit_bytes == 0 {
            return Err(ResourceError::Invalid(
                "max-expanded-commit-bytes must be positive",
            ));
        }
        Ok(())
    }
}

/// Deterministic tenant → physical shard mapping. Uses a fixed-width FNV-1a
/// over the 32-byte tenant id so routing is restart-stable and independent of
/// the in-memory admission-shard count. Commits without a tenant route to
/// shard 0.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the hash is immediately reduced modulo shard_count, so truncating its high bits cannot change the restart-stable routing"
)]
pub fn physical_shard_for(
    tenant: Option<&rushdb_types::ids::TenantId>,
    shard_count: usize,
) -> usize {
    debug_assert!(shard_count >= 1);
    match tenant {
        Some(t) => {
            let mut h = 2_166_136_261u64;
            for b in &t.to_be_bytes() {
                h ^= u64::from(*b);
                h = h.wrapping_mul(16_777_619);
            }
            (h as usize) % shard_count
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_usage_draws_from_every_ancestor_without_oversubscription() {
        let root = ByteBudget::new("root", 100).unwrap();
        let query = root.child("query", 80).unwrap();
        let leaf = query.child("operator", 60).unwrap();
        let other = root.child("index", 80).unwrap();
        let a = leaf.try_reserve(60).unwrap();
        let b = other.try_reserve(40).unwrap();
        assert_eq!(
            root.snapshot(),
            BudgetSnapshot {
                limit: 100,
                used: 100,
                peak: 100
            }
        );
        assert_eq!(query.snapshot().used, 60);
        assert_eq!(query.available(), 0);
        assert!(matches!(
            query.try_reserve(1),
            Err(ResourceError::Exhausted { budget: "root", .. })
        ));
        assert!(leaf.try_reserve(1).is_err());
        drop(b);
        assert_eq!(query.available(), 20);
        drop(a);
        assert_eq!(root.snapshot().used, 0);
        assert_eq!(root.snapshot().peak, 100);
    }

    #[test]
    fn split_resize_transfer_and_failed_transfer_preserve_ownership() {
        let root = ByteBudget::new("root", 100).unwrap();
        let a = root.child("a", 100).unwrap();
        let b = root.child("b", 30).unwrap();
        let mut owned = a.try_reserve(60).unwrap();
        let mut split = owned.split(20).unwrap();
        assert_eq!(
            (owned.bytes(), split.bytes(), root.snapshot().used),
            (40, 20, 60)
        );
        assert!(owned.split(41).is_err());
        assert!(owned.resize(101).is_err());
        assert_eq!(owned.bytes(), 40);
        assert!(owned.transfer(&b).is_err());
        assert_eq!(
            (a.snapshot().used, b.snapshot().used, root.snapshot().used),
            (60, 0, 60)
        );
        split.transfer(&b).unwrap();
        assert_eq!(
            (a.snapshot().used, b.snapshot().used, root.snapshot().used),
            (40, 20, 60)
        );
        split.transfer(&root).unwrap();
        split.transfer(&a).unwrap();
        split.transfer(&a).unwrap(); // Same-node transfer is not a double charge.
        let independent = ByteBudget::new("independent", 100).unwrap();
        assert_eq!(
            split.transfer(&independent),
            Err(ResourceError::DifferentTree)
        );
        assert_eq!(independent.snapshot().used, 0);
        owned.resize(10).unwrap();
        assert_eq!(root.snapshot().used, 30);
        drop(owned);
        drop(split);
        assert_eq!(root.snapshot().used, 0);
        assert_eq!(root.snapshot().peak, 60);
    }

    #[test]
    fn unwind_and_last_reservation_release_child_metadata() {
        let root = ByteBudget::new("root", 100).unwrap();
        let child = root.child("child", 100).unwrap();
        let weak = Arc::downgrade(&child.0);
        let token = child.try_reserve(50).unwrap();
        drop(child);
        assert!(weak.upgrade().is_some());
        let panic = std::panic::catch_unwind(move || {
            let _token = token;
            panic!("simulated failed work");
        });
        assert!(panic.is_err());
        assert_eq!(root.snapshot().used, 0);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn concurrent_siblings_never_exceed_parent_or_child_limits() {
        let root = ByteBudget::new("root", 64).unwrap();
        let left = root.child("left", 32).unwrap();
        let right = root.child("right", 32).unwrap();
        let ready = std::sync::Barrier::new(16);
        let release = std::sync::Barrier::new(16);
        let admitted = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for i in 0..16 {
                let pool = if i % 2 == 0 { &left } else { &right };
                let ready = &ready;
                let release = &release;
                let admitted = &admitted;
                scope.spawn(move || {
                    ready.wait();
                    let token = pool.try_reserve(8).ok();
                    if token.is_some() {
                        admitted.fetch_add(1, Ordering::Relaxed);
                    }
                    release.wait();
                    assert!(pool.snapshot().used <= 32);
                    drop(token);
                });
            }
        });
        assert_eq!(admitted.load(Ordering::Relaxed), 8);
        assert_eq!(
            root.snapshot(),
            BudgetSnapshot {
                limit: 64,
                used: 0,
                peak: 64
            }
        );
    }

    #[test]
    fn invalid_limits_and_overflow_are_rejected_without_charging() {
        assert!(ByteBudget::new("zero", 0).is_err());
        let root = ByteBudget::new("root", usize::MAX).unwrap();
        let all = root.try_reserve(usize::MAX).unwrap();
        assert!(root.try_reserve(1).is_err());
        assert!(root.child("zero", 0).is_err());
        drop(all);
        assert_eq!(root.snapshot().used, 0);
        let mut child = root;
        for _ in 0..32 {
            child = child.child("depth", 1).unwrap();
        }
        assert!(child.child("too-deep", 1).is_err());
    }

    #[test]
    fn fair_admission_admits_when_capacity_frees_and_times_out_on_deadline() {
        let budget = ByteBudget::new("root", 10).unwrap();
        let gate = FairAdmission::new(budget.clone());
        // Consume 9 of 10 bytes.
        let held = budget.try_reserve(9).unwrap();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        // A request for 5 bytes cannot be admitted now; it must queue. Give it
        // a short deadline that expires before the holder releases.
        let expired = now_ms() + 20;
        let outcome = gate.acquire(5, expired, &cancel);
        assert_eq!(
            outcome.err(),
            Some(AdmissionError::TimedOut),
            "must time out on deadline"
        );
        assert_eq!(
            budget.snapshot().used,
            9,
            "timed-out waiter charges nothing"
        );

        // Now release the holder; a fresh request admits immediately and the
        // caller owns the reservation.
        drop(held);
        let reservation = gate.acquire(5, now_ms() + 1000, &cancel).unwrap();
        assert_eq!(
            budget.snapshot().used,
            5,
            "admitted caller holds the charge"
        );
        drop(reservation);
        assert_eq!(budget.snapshot().used, 0, "drop releases the charge");
    }

    #[test]
    fn fair_admission_cancelled_waiter_is_removed_without_charging() {
        let budget = ByteBudget::new("root", 10).unwrap();
        let gate = std::sync::Arc::new(FairAdmission::new(budget.clone()));
        let _held = budget.try_reserve(9).unwrap();
        // The waiter shares the cancel flag; flipping it must wake and remove
        // the queued waiter with no charge and no deadlock.
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let gate2 = std::sync::Arc::clone(&gate);
        let cancel2 = std::sync::Arc::clone(&cancel);
        let handle = std::thread::spawn(move || gate2.acquire(5, now_ms() + 2000, &cancel2));
        // Give the waiter a moment to enqueue, then cancel it.
        std::thread::sleep(std::time::Duration::from_millis(30));
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        let outcome = handle.join().unwrap();
        assert_eq!(
            outcome.err(),
            Some(AdmissionError::Cancelled),
            "waiter must observe cancellation"
        );
        assert_eq!(
            budget.snapshot().used,
            9,
            "cancelled waiter charges nothing"
        );
    }

    #[test]
    fn fair_admission_fifo_and_release_wakes_next_waiter() {
        let budget = ByteBudget::new("root", 100).unwrap();
        let gate = std::sync::Arc::new(FairAdmission::new(budget.clone()));
        let held = budget.try_reserve(95).unwrap();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel2 = std::sync::Arc::clone(&cancel);
        // A thread waits for 10 bytes (capacity held). Releasing the holder
        // must admit the waiting thread (FIFO wake).
        let gate2 = std::sync::Arc::clone(&gate);
        let handle = std::thread::spawn(move || gate2.acquire(10, now_ms() + 2000, &cancel2));
        std::thread::sleep(std::time::Duration::from_millis(30));
        drop(held);
        let outcome = handle.join().unwrap();
        match outcome {
            Ok(_) => {}
            Err(e) => panic!("release must wake the waiter: {e:?}"),
        }
        assert_eq!(
            budget.snapshot().used,
            10,
            "waiter holds the admitted charge"
        );
    }
}

#[cfg(test)]
mod shard_tests {
    use super::*;
    use rushdb_types::ids::TenantId;

    #[test]
    fn physical_shard_routing_is_deterministic_and_in_range() {
        for count in [1usize, 2, 4, 8, 64] {
            for n in 0..100u64 {
                let t = TenantId::from_ordinal(n);
                let s = physical_shard_for(Some(&t), count);
                assert!(s < count);
                assert_eq!(physical_shard_for(Some(&t), count), s);
            }
        }
        assert_eq!(physical_shard_for(None, 8), 0);
    }

    #[test]
    fn different_tenants_distribute_across_shards() {
        let count = 4usize;
        let mut seen = std::collections::BTreeSet::new();
        for n in 0..256u64 {
            let t = TenantId::from_ordinal(n);
            seen.insert(physical_shard_for(Some(&t), count));
        }
        assert_eq!(seen.len(), count, "tenants should land on every shard");
    }

    #[test]
    fn shard_count_validation() {
        let mut resources = StorageResources::default();
        assert!(resources.validate().is_ok());
        resources.shard_count = 0;
        assert!(resources.validate().is_err());
        resources.shard_count = 257;
        assert!(resources.validate().is_err());
    }

    #[test]
    fn multi_shard_requires_explicit_experimental_acknowledgement() {
        // Multi-shard commit groups are not crash-atomic, so >1 shard must be
        // an explicit, acknowledged choice.
        let mut resources = StorageResources {
            shard_count: 2,
            ..StorageResources::default()
        };
        assert!(
            resources.validate().is_err(),
            "shard_count > 1 must be refused without acknowledgement"
        );
        resources.experimental_multi_shard = true;
        assert!(resources.validate().is_ok());
    }
}
