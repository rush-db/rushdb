//! Ordered KV substrate shared by every node: buffered transactions over a
//! committed B-tree view, atomic redb publication of expanded op sets, a
//! serialized command log, multi-version snapshot reads, and
//! serializable-optimistic dependency validation.

// The storage boundary deliberately exposes byte-oriented iterator and callback
// signatures. Naming each nested trait-object form would obscure that API.
#![allow(clippy::type_complexity)]

mod adapter_budgets;
pub mod backend;
pub mod cache;
mod health;
pub mod read_accounting;
mod transaction_view;
pub use transaction_view::Txn;
use transaction_view::TxnBase;
mod commit;
mod copy_on_write;
pub use copy_on_write::CopyOnWrite;
mod conflict_window;
mod derive;
mod fair_gate;
pub mod history_v2;
mod layout;
mod log_export;
pub mod log_store;
mod maintenance;
mod memory_backend;
mod observability;
mod overlay_view;
mod persist;
pub mod planner_stats;
mod publish;
mod read_view;
mod receipt;
mod redb_backend;
pub mod resources;
mod temp_path;
pub mod tenant_dir;
mod usage;
pub use temp_path::TempPath;
pub use usage::is_usage_bytes_key;
pub mod wal;
mod wal_replay;
use backend::StorageBackend;
pub use derive::{DeriveRead, Deriver};
pub use maintenance::CompactOutcome;
pub use observability::{
    BackendLockTimes, CommitObserver, CommitPhase, WriterHolder, WriterHolds, WRITER_HOLD_BOUNDS,
};
pub use overlay_view::OverlayView;
pub use planner_stats::PlannerStats;
pub use read_view::{
    ReadCancellation, ReadLimits, ReadPin, ReadView, ScanBatch, ScanBounds, ScanCursor,
    ScanProjection, ScanRow,
};

#[cfg(test)]
mod derive_tests;
#[cfg(test)]
mod read_view_tests;
#[cfg(test)]
mod receipt_tests;
#[cfg(test)]
mod regression_tests;
#[cfg(test)]
mod sequencer_tests;
#[cfg(test)]
mod writer_tests;

/// Remove the materialization file at `path`. The WAL directory and layout
/// manifest are left alone.
///
/// # Errors
/// Returns the I/O error of the removal.
pub fn remove_store(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

use rushdb_types::command::CommitBatch;
use rushdb_types::ids::TenantId;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Included, Unbounded};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use thiserror::Error;
use uuid::Uuid;

/// Per-tenant outer admission shards, retained for replication call ordering.
/// Local expansion additionally takes the single logical writer mutex.
const N_SHARDS: usize = 64;

/// A fully-admitted op-set awaiting batched durable publication, keyed by
/// its reserved commit index. The publisher drains the contiguous prefix from
/// the published watermark in one redb write transaction.
struct PendingCommit {
    ci: u64,
    /// Physical shard this commit belongs to (tenant routing). The publisher
    /// fans out durability across shard backends so different tenants persist
    /// in parallel once `shard_count > 1`.
    shard: usize,
    ops: Vec<Op>,
    prepared: PreparedCommand,
    /// The originating `CommitBatch`, pre-serialized: paired with
    /// `ops` in the WAL record so the command journal is derivable from the
    /// WAL alone, without a separate `__log__` redb row.
    command_payload: Vec<u8>,
    ack: mpsc::Sender<Result<(), StorageError>>,
}

/// Pending-overlay state for the commit sequencer.
///
/// A committed `base` plus the net key effect of every assigned-but-unpublished
/// commit, so an expansion can read through `Txn`'s overlay instead of waiting
/// for the predecessor to be published. It is guarded by the writer
/// `admission_lock`, so every writer that can assign a commit index observes
/// one consistent overlay.
///
/// `buffer` is this state's *own* copy (never the publisher's `pending` map), so
/// it cannot race the publisher removing a drained entry. Rebase only needs the
/// generation (commit index) at which each key was last written, so the buffer
/// stays O(distinct in-flight keys), not O(in-flight ops).
#[derive(Default)]
struct SequencerState {
    /// `key -> (commit index that last wrote it, value or tombstone)` for
    /// assigned-but-unpublished commits. The committed base is opened fresh
    /// (and dropped) per critical section, so this state holds no read pin
    /// between commits and cannot stall GC.
    buffer: BTreeMap<Vec<u8>, (u64, Option<Vec<u8>>)>,
}

impl std::fmt::Debug for SequencerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequencerState")
            .field("in_flight_keys", &self.buffer.len())
            .finish()
    }
}

impl SequencerState {
    /// Fold an assigned commit's expanded ops into the pending buffer.
    fn record(&mut self, ci: u64, ops: &[Op]) {
        for op in ops {
            match op {
                Op::Put(key, value) | Op::Set(key, value) => {
                    self.buffer.insert(key.clone(), (ci, Some(value.clone())));
                }
                Op::Del(key) | Op::Purge(key) => {
                    self.buffer.insert(key.clone(), (ci, None));
                }
            }
        }
    }
}

pub(crate) const LOG_PREFIX: &[u8] = b"__log__/";
const LAST_APPLIED_KEY: &[u8] = b"meta/last-applied";
/// Durable watermark: the highest LSN retention has retired.
/// Bounds `Store::truncate_log`, so the retention floor (hard caps and
/// consumer protection) decides which WAL segments may go.
pub(crate) const RETENTION_CURSOR_KEY: &[u8] = b"meta/retention-cursor";

/// Durable byte accounting by key family, for growth diagnosis and operator
/// metrics. All values are exact sums of stored key+value lengths for the
/// family, taken from one read view; they exclude redb's copy-on-write slack
/// (compare against [`Store::storage_file_bytes`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StorageStatistics {
    /// Canonical data rows (records, occurrences, postings, catalogs, …).
    pub data_keys: u64,
    /// Total key+value bytes of canonical data rows.
    pub data_bytes: u64,
    /// Retained MVCC history rows.
    pub history_keys: u64,
    /// Total key+encoded-history bytes.
    pub history_bytes: u64,
    /// Retained command-log rows (`__log__/<lsn>`).
    pub log_keys: u64,
    /// Total command-log bytes.
    pub log_bytes: u64,
    /// Retry-receipt rows (receipts plus their expiry index).
    pub receipt_keys: u64,
    /// Total retry-receipt bytes.
    pub receipt_bytes: u64,
    /// Number of bounded admission-lock waits (including timed-out waits).
    pub admission_waits: u64,
    /// Milliseconds spent waiting for admission-lock ownership.
    pub admission_wait_ms: u64,
    /// Number of explicit durability checkpoints.
    pub checkpoints: u64,
    /// Milliseconds spent in explicit durability checkpoints.
    pub checkpoint_wait_ms: u64,
}

/// Errors returned by the storage layer: failed preconditions, corrupt or
/// unsupported durable state, resource exhaustion, and invalid read requests.
#[derive(Debug, Clone, Error)]
pub enum StorageError {
    /// A poisoned storage lock fenced the store; reopen is required.
    #[error("storage unhealthy after lock poisoning; reopen required")]
    Unhealthy,
    /// A transaction precondition was violated; `path` is the offending key.
    #[error("precondition failed on {path}")]
    Precondition {
        /// Key whose precondition failed.
        path: String,
    },
    /// The deterministic expander failed while building a commit.
    #[error("builder error: {0}")]
    Builder(String),
    /// A durable record could not be decoded or verified.
    #[error("corrupt log entry")]
    CorruptLog,
    /// A log record carries a command version this build does not support.
    #[error("unsupported command version {0}")]
    UnsupportedCommandVersion(u32),
    /// The stored outcome payload is missing or cannot be deserialized.
    #[error("committed outcome payload is unavailable or incompatible")]
    OutcomePayload,
    /// The physical backend failed while applying or reading durable state.
    #[error("durable storage failure: {0}")]
    Durability(String),
    /// The requested snapshot predates the oldest retained commit.
    #[error("snapshot {requested} expired; oldest supported commit is {oldest}")]
    SnapshotExpired {
        /// Snapshot commit index that was requested.
        requested: u64,
        /// Oldest commit index still readable.
        oldest: u64,
    },
    /// The requested snapshot is newer than the visible commit index.
    #[error("snapshot {requested} is newer than visible commit {visible}")]
    FutureSnapshot {
        /// Snapshot commit index that was requested.
        requested: u64,
        /// Commit index currently visible.
        visible: u64,
    },
    /// A managed byte budget refused the request.
    #[error("storage resource exhausted: {0}")]
    ResourceExhausted(String),
    /// The supplied storage configuration is not valid.
    #[error("invalid storage configuration: {0}")]
    InvalidConfiguration(String),
    /// A pinned read was cancelled by its caller.
    #[error("storage read cancelled")]
    Cancelled,
    /// Scan bounds or batch limits are invalid.
    #[error("invalid scan bounds or limits: {0}")]
    InvalidRead(String),
    /// Physical compaction was requested while protected read views are open.
    #[error("physical compaction requires all protected read views to close")]
    ReadersActive,
    /// No conflict summary is retained for a commit that must be probed.
    #[error(
        "actual write summary unavailable for commit {commit_index}; retry with a fresh snapshot"
    )]
    ConflictHistoryUnavailable {
        /// Commit index whose conflict summary is missing.
        commit_index: u64,
    },
}

/// Operation emitted into the atomic publish set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Insert or overwrite a key, retaining the superseded value as history.
    Put(Vec<u8>, Vec<u8>),
    /// Remove a key, retaining the removed value as history.
    Del(Vec<u8>),
    /// Overwrite a key **without** retaining a superseded version (and drop
    /// any history already accumulated for it). Used for internal bookkeeping
    /// keys whose values are not part of user-visible time travel: counters,
    /// allocation watermarks, degree counters, the label catalog and the
    /// commit watermark. Without this, every overwrite of a hot counter
    /// rewrites its whole history vector, which is quadratic in the number of
    /// writes.
    Set(Vec<u8>, Vec<u8>),
    /// Erase a key together with its current-value MVCC metadata and retained
    /// history, without appending a new version. Destructive by design: used
    /// by tenant purge, where the values are intentionally destroyed and
    /// keeping versions for them would bloat storage without bound.
    Purge(Vec<u8>),
}

impl Op {
    /// Approximate encoded size (key + value bytes) for group-commit byte
    /// accounting (P04 bounded writer).
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        match self {
            Op::Put(k, v) | Op::Set(k, v) => k.len() + v.len(),
            Op::Del(k) | Op::Purge(k) => k.len(),
        }
    }
}

/// Reason prefix of a [`CommitOutcome::Conflicted`] caused by a read
/// dependency that a later commit wrote (first-committer-wins).
pub const SERIALIZABLE_CONFLICT: &str = "serializable_conflict";

/// Serializable-mode read dependencies recorded against a transaction.
/// Zero-width entries are point reads; otherwise half-open [start,end).
#[derive(Default, Clone, Debug)]
pub struct DependencySet {
    /// Recorded read ranges as `(start, end)` pairs; a zero-width pair is a
    /// point dependency.
    pub ranges: Vec<(Vec<u8>, Vec<u8>)>,
    whole_keyspace: bool,
}

impl DependencySet {
    /// Whether no point, range, or whole-keyspace dependency was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty() && !self.whole_keyspace
    }

    /// Record a half-open `[start, end)` range dependency.
    pub fn record(&mut self, start: Vec<u8>, end: Vec<u8>) {
        self.ranges.push((start, end));
    }

    /// Prefix tracking is explicit so an empty prefix (all keys) is distinct
    /// from a point dependency on the empty key.
    pub fn record_prefix(&mut self, prefix: &[u8]) {
        if prefix.is_empty() {
            self.whole_keyspace = true;
        } else {
            let (start, end) = prefix_range(prefix);
            self.record(start, end);
        }
    }

    /// Merge dependencies captured by another preparation phase.
    pub fn extend(&mut self, other: Self) {
        self.ranges.extend(other.ranges);
        self.whole_keyspace |= other.whole_keyspace;
    }

    fn intersects(&self, key: &[u8]) -> bool {
        self.whole_keyspace
            || self.ranges.iter().any(|(s, e)| {
                if s == e {
                    key == s.as_slice()
                } else {
                    key >= s.as_slice() && (e.is_empty() || key < e.as_slice())
                }
            })
    }

    /// Whether `key` falls in any recorded read range (serializable conflict
    /// probe for transaction commit).
    #[must_use]
    pub fn intersects_key(&self, key: &[u8]) -> bool {
        self.intersects(key)
    }
}

/// Prepared metadata for one admitted command. The publisher keeps this
/// alongside expanded operations so an admission overlay can validate
/// read/write conflicts without re-expanding the command.
#[derive(Clone, Debug, Default)]
pub struct PreparedCommand {
    /// Reserved commit index, preserving command-log order.
    pub commit_index: u64,
    /// Canonical keys written by the expanded operation set.
    pub writes: BTreeSet<Vec<u8>>,
    /// Keys/ranges read while preparing the command.
    pub reads: DependencySet,
    /// Expanded under writer admission against the sequencer overlay, so its
    /// reads already observed every lower-index commit (published or
    /// pending). Such a command can share a publish group with the commits
    /// it read from: the group applies in commit-index order.
    pub sequenced: bool,
}

impl PreparedCommand {
    /// Build prepared metadata from an expanded operation set.
    #[must_use]
    pub fn from_ops(commit_index: u64, ops: &[Op]) -> Self {
        let writes = ops
            .iter()
            .map(|op| match op {
                Op::Put(key, _) | Op::Set(key, _) | Op::Del(key) | Op::Purge(key) => key,
            })
            .cloned()
            .collect();
        Self {
            commit_index,
            writes,
            reads: DependencySet::default(),
            sequenced: false,
        }
    }

    /// Whether this command's reads overlap writes from `other`, or vice
    /// versa. The check is symmetric and suitable for optimistic admission.
    ///
    /// The write-write check (large-transaction support, Part F) uses
    /// `writes` being a `BTreeSet` directly — `other.writes.contains(key)`
    /// is O(log n), not the nested linear scan this used to be. Filtering
    /// only `self`'s side is still exactly equivalent to filtering both:
    /// `is_coordination_write` is a pure function of the key's own bytes, so
    /// if `key` already passed the filter, an identical byte sequence in
    /// `other.writes` has the same classification — there is no case where
    /// filtering the other side too would change the answer.
    ///
    /// Read sets are probed **against** the ordered write set — one range
    /// query per recorded read — rather than testing every written key
    /// against a linear list of read ranges. The old form was
    /// O(writes × reads) per pair and, applied pairwise inside every publish
    /// group, starved bulk ingest for minutes (cloud-capacity F4 follow-up).
    #[must_use]
    pub fn conflicts_with(&self, other: &Self) -> bool {
        reads_hit_writes(&other.reads, &self.writes)
            || reads_hit_writes(&self.reads, &other.writes)
            || {
                let (small, large) = if self.writes.len() <= other.writes.len() {
                    (&self.writes, &other.writes)
                } else {
                    (&other.writes, &self.writes)
                };
                small
                    .iter()
                    .filter(|key| !is_coordination_write(key))
                    .any(|key| large.contains(key))
            }
    }
}

/// Whether any recorded read (point, range, or whole keyspace) covers a
/// non-coordination key of `writes`. O(reads × log writes).
fn reads_hit_writes(reads: &DependencySet, writes: &BTreeSet<Vec<u8>>) -> bool {
    use std::ops::Bound::Excluded;
    let application = |key: &&Vec<u8>| !is_coordination_write(key);
    if reads.whole_keyspace {
        return writes.iter().any(|key| application(&key));
    }
    reads.ranges.iter().any(|(start, end)| {
        if start == end {
            writes.contains(start) && !is_coordination_write(start)
        } else if end.is_empty() {
            writes
                .range::<[u8], _>((Included(start.as_slice()), Unbounded))
                .any(|key| application(&key))
        } else if start < end {
            writes
                .range::<[u8], _>((Included(start.as_slice()), Excluded(end.as_slice())))
                .any(|key| application(&key))
        } else {
            false
        }
    })
}

/// Ordered publication owns these metadata rows; they are rewritten by every
/// commit as part of the command log and do not represent application-level
/// read/write overlap for group admission.
fn is_coordination_write(key: &[u8]) -> bool {
    key == LAST_APPLIED_KEY
        || key.starts_with(LOG_PREFIX)
        || key.starts_with(receipt::RECEIPT_PREFIX)
        || key.starts_with(receipt::RECEIPT_EXP_PREFIX)
}

/// Inclusive start / exclusive end range covering all keys under a prefix.
/// An empty end bound means unbounded above.
#[must_use]
pub fn prefix_range(prefix: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut end = prefix.to_vec();
    loop {
        match end.last_mut() {
            Some(last) if *last != 0xFF => {
                *last += 1;
                return (prefix.to_vec(), end);
            }
            Some(_) => {
                end.pop();
            }
            None => return (prefix.to_vec(), Vec::new()),
        }
    }
}

fn log_key(seq: u64) -> Vec<u8> {
    let mut k = LOG_PREFIX.to_vec();
    k.extend_from_slice(format!("{seq:020}").as_bytes());
    k
}

/// Terminal status of a commit key, as stored durably.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeKind {
    /// The commit succeeded at the given commit index.
    Committed(u64),
    /// The commit was terminally aborted and must not be retried under the
    /// same commit key.
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutcomeStatus {
    Committed,
    Aborted,
}

/// Result of a commit attempt: either the durable commit index plus the
/// caller payload, or a conflict reason.
#[derive(Debug)]
pub enum CommitOutcome<Payload> {
    /// The commit was published at `commit_index` with the caller's `payload`.
    Committed {
        /// Durable commit index assigned to this commit.
        commit_index: u64,
        /// Caller-supplied response payload persisted with the outcome.
        payload: Payload,
    },
    /// The commit was rejected; the string explains the conflict.
    Conflicted(String),
}

/// Deterministic state-machine expansion callback: derives every canonical
/// and derived key implied by one committed batch (document 07).
pub type ExpandFn = Box<dyn Fn(u64, &CommitBatch, &mut Txn) -> Result<(), String> + Send + Sync>;

/// Accumulated state of one in-flight cross-service transaction. The buffer is
/// the transaction's uncommitted write overlay (read-buffer-first like `Txn`);
/// `deps` records the public reads performed inside the transaction;
/// `mutations`/`preconditions` replay the writes atomically at commit.
#[derive(Clone)]
pub struct TxAccumulator {
    /// Commit index the transaction's snapshot reads are based on.
    pub base_ci: u64,
    /// Tenant that owns the transaction.
    pub tenant: TenantId,
    /// Uncommitted write overlay, keyed by storage key.
    pub buffer: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    /// Public read dependencies recorded inside the transaction.
    pub deps: DependencySet,
    /// Mutations accumulated across statements, replayed at commit.
    pub mutations: Vec<rushdb_types::command::Mutation>,
    /// Preconditions accumulated across statements, checked at commit.
    pub preconditions: Vec<rushdb_types::command::Precondition>,
    /// Pinned snapshot all transaction reads resolve at. Cross-request
    /// coordinators set this at BEGIN so statements and expansion observe the
    /// transaction's snapshot rather than current committed state. `None`
    /// keeps current-state reads for embedded/short-lived callers.
    pub base_view: Option<std::sync::Arc<ReadView>>,
}

impl TxAccumulator {
    /// Create an empty accumulator for `tenant` whose snapshot is `base_ci`.
    #[must_use]
    pub fn new(tenant: TenantId, base_ci: u64) -> Self {
        TxAccumulator {
            base_ci,
            tenant,
            buffer: BTreeMap::new(),
            deps: DependencySet::default(),
            mutations: Vec::new(),
            preconditions: Vec::new(),
            base_view: None,
        }
    }
}

/// Replication seam: when installed on a `Store`, local commits build the
/// canonical `CommitBatch` and route it through the leader. The leader
/// replicates to the quorum and every node (leader included) applies it
/// through `Store::apply_replicated_batch`, which returns after the batch is
/// durably applied locally. Serialized `payload` is stored with the outcome so
/// idempotent retries can reproduce the caller's response.
pub trait Replicator: Send + Sync {
    /// Replicate `batch` with its serialized `payload` to the quorum.
    ///
    /// # Errors
    /// Returns a human-readable reason if replication fails.
    fn replicate(&self, batch: &CommitBatch, payload: &[u8]) -> Result<(), String>;

    /// Highest commit index durably acknowledged by the replication quorum.
    ///
    /// Implementations that cannot expose a watermark return `None`; the
    /// retention worker then leaves the replica consumer unregistered rather
    /// than guessing that a lagging replica is caught up.
    fn acknowledged_commit_index(&self) -> Option<u64> {
        None
    }
}

/// One superseded value of a key, covering the half-open range [from,to).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VersionEntry {
    /// Commit index at which this version became current.
    pub from_ci: u64,
    /// Commit index that superseded this version; `0` means still current.
    pub to_ci: u64,
    /// Stored value; `None` represents a deletion.
    pub value: Option<Vec<u8>>,
}

/// Ordered key/value store shared by every node: admission, deterministic
/// expansion, group-commit publication, multi-version reads, and retention.
pub struct Store {
    health: Arc<health::StoreHealth>,
    resources: resources::ByteBudget,
    transaction_resources: resources::ByteBudget,
    /// Persistent shared budget for retained logical caches.
    logical_cache_resources: Option<resources::ByteBudget>,
    /// Shared reservation pool for in-flight query/adapter phases.
    phase_resources: Option<resources::ByteBudget>,
    /// Per-adapter attribution children of the two budgets above.
    adapter_budgets: adapter_budgets::AdapterBudgets,
    planner_stats: Arc<planner_stats::PlannerStats>,
    resource_config: resources::StorageResources,
    data: Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>,
    /// creation commit index per live key (multi-version reads).
    created: Arc<RwLock<BTreeMap<Vec<u8>, u64>>>,
    /// superseded values per key for time-travel reads.
    history: Arc<RwLock<BTreeMap<Vec<u8>, Vec<VersionEntry>>>>,
    /// lowest commit index that must remain readable (GC watermark).
    gc_watermark: AtomicU64,
    /// Logical retention leases (held under `publish_lock` so a successful
    /// view cannot race GC). Each entry maps a pinned CI to its [`ReadPin`]
    /// (count, last-lease-ms, generation) so an abandoned reader can be
    /// expired by age (P06 safe cancellation-before-release).
    read_pins: Arc<Mutex<BTreeMap<u64, ReadPin>>>,
    /// Cancellation handles corresponding to each retained read lease. GC
    /// cancels a lease before removing its retention pin, so an expired view
    /// cannot continue reading after history is reclaimed.
    read_cancellations: Arc<Mutex<BTreeMap<u64, Vec<ReadCancellation>>>>,
    /// Transitional single logical writer. Planning and expansion see the
    /// fully published predecessor, including global metadata shared by tenants.
    /// Also owns the sequencer's pending overlay (`SequencerState`).
    admission_lock: Mutex<SequencerState>,
    /// A failed fsync can have an uncertain durable outcome. Fence all later
    /// writes until reopen; never retry publication under another CI in-process.
    publisher_failure: Mutex<Option<StorageError>>,
    #[cfg(test)]
    fail_next_persist: std::sync::atomic::AtomicBool,
    /// Per-tenant outer admission shards (replication planning order).
    shards: Vec<Mutex<()>>,
    /// Serializes durable persist + in-memory publish in commit-index order.
    publish_lock: Mutex<()>,
    /// FIFO gate in front of `admission_lock` (see `fair_gate`).
    admission_queue: fair_gate::FairGate,
    /// Write sets of recently published commits for optimistic conflict
    /// validation (in memory only; see `conflict_window`).
    conflicts: Mutex<conflict_window::ConflictWindow>,
    /// Fully-admitted op-sets awaiting batched publication, keyed by their
    /// reserved commit index (drained from `published+1`).
    pending: Mutex<BTreeMap<u64, PendingCommit>>,
    pending_cv: Condvar,
    /// Commit-index reservation counter (1-based). Reservation happens at
    /// admission; only successful commits consume a slot so the log stays
    /// dense and replay stays position-indexed.
    next_commit_index: AtomicU64,
    /// Last durably persisted + applied index. `committed_version()` reads
    /// this watermark (a reservation may run ahead of publication).
    published: AtomicU64,
    expander: RwLock<Option<Arc<ExpandFn>>>,
    /// Called with each durable commit's latency in seconds (metrics hook).
    commit_observer: RwLock<Option<Arc<CommitObserver>>>,
    /// Apply-time deriver; `None` applies base ops unchanged.
    deriver: RwLock<Option<Arc<dyn Deriver>>>,
    /// Bounded tenant-directory resolution cache.
    tenant_cache: Mutex<tenant_dir::TenantCache>,
    /// Set when WAL recovery applied records with no deriver installed, so a
    /// later `set_deriver` fences the store instead of serving missing
    /// derived rows.
    underived_replay: std::sync::atomic::AtomicBool,
    /// One durable backend per physical shard. `shard_count == 1` (default)
    /// keeps a single element, preserving today's single-file layout. A future
    /// per-shard commit-index split (W4 phase 2b) routes each tenant's commits
    /// to its shard's backend for parallel writers.
    durable: RwLock<Box<[Option<Arc<dyn StorageBackend>>]>>,
    /// Log-first segmented WAL: when present, the durable truth — a
    /// commit acks after its op-set is appended+fsynced here, and redb is a
    /// checkpointed materialization replayed from it. `None` for in-memory.
    log: Mutex<Option<log_store::LogStore>>,
    /// Optional replication seam (`OpenRaft` leader): when set, local commits
    /// route through the leader instead of publishing locally; every node
    /// (including this one) applies the result via `apply_replicated_batch`.
    replicator: RwLock<Option<Arc<dyn Replicator>>>,
    /// Resumable GC cursor: the last history key the background worker
    /// processed. `None` means no sweep is in progress.
    gc_cursor: Mutex<Option<Vec<u8>>>,
    /// Primary durable file path (shard 0), set by `attach_redb`. Used only
    /// for operator-facing size metrics.
    data_path: Mutex<Option<std::path::PathBuf>>,
    /// Copy-on-write state of the data files, recorded at open.
    copy_on_write: Mutex<Option<CopyOnWrite>>,
    /// Set at shutdown so long maintenance scans stop promptly and the store
    /// closes cleanly instead of waiting for a full keyspace scan.
    maintenance_abort: Arc<AtomicBool>,
    /// Operator counters for retention/compaction sweeps.
    log_rows_pruned: AtomicU64,
    receipts_swept: AtomicU64,
    /// Clock of retry receipts (unix ms); injectable so tests can age them.
    receipt_clock: RwLock<Arc<dyn Fn() -> u64 + Send + Sync>>,
    compactions: AtomicU64,
    /// Number of group-commit redb write transactions persisted (one fsync
    /// each under `Local`); the coalescing metric for the linger knob.
    group_commits: AtomicU64,
    admission_waits: AtomicU64,
    admission_wait_ms: AtomicU64,
    checkpoints: AtomicU64,
    checkpoint_wait_ms: AtomicU64,
    /// Wall-clock ms of the last published commit; compaction quiescence gate.
    last_commit_ms: AtomicU64,
    /// File bytes immediately after the last compaction; growth-ratio gate.
    last_compacted_file_bytes: AtomicU64,
    /// Keys erased by purges (tenant purge) since the last compaction: a
    /// purge frees pages without growing the file, so it triggers idle
    /// compaction on its own.
    purged_since_compaction: AtomicU64,
}

impl Default for Store {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl Store {
    /// Create an in-memory store with default resource budgets.
    ///
    /// # Panics
    /// Panics if the default storage resources are invalid, which is a
    /// compile-time constant and cannot happen at runtime.
    #[must_use]
    pub fn new_in_memory() -> Self {
        Self::new_with_resources(resources::StorageResources::default())
            .expect("valid default storage resources")
    }

    /// Create an in-memory store with an explicit resource configuration.
    ///
    /// # Errors
    /// Returns [`StorageError::InvalidConfiguration`] when `config` fails
    /// validation, or [`StorageError::ResourceExhausted`] when a byte budget
    /// cannot be created.
    pub fn new_with_resources(config: resources::StorageResources) -> Result<Self, StorageError> {
        config
            .validate()
            .map_err(|e| StorageError::InvalidConfiguration(e.to_string()))?;
        let resources = resources::ByteBudget::new("store", config.managed_limit_bytes)
            .map_err(|e| StorageError::ResourceExhausted(e.to_string()))?;
        let transaction_resources = resources
            .child("transaction-staging", config.transaction_pool_bytes)
            .map_err(|e| StorageError::InvalidConfiguration(e.to_string()))?;
        // A zero-sized pool is disabled; otherwise it is capped by the root.
        let optional_child = |name: &'static str, bytes: usize| {
            (bytes > 0)
                .then(|| resources.child(name, bytes.min(config.managed_limit_bytes)))
                .transpose()
                .map_err(|e| StorageError::InvalidConfiguration(e.to_string()))
        };
        let phase_resources = optional_child("query-phases", config.phase_budget_bytes)?;
        let logical_cache_resources = optional_child("logical-cache", config.logical_cache_bytes)?;
        let adapter_budgets = adapter_budgets::AdapterBudgets::new(
            logical_cache_resources.as_ref(),
            phase_resources.as_ref(),
        )?;
        let planner_stats = Arc::new(planner_stats::PlannerStats::new(
            &config,
            logical_cache_resources.as_ref(),
            Arc::new(std::time::Instant::now),
        )?);
        // The in-memory store is a real `StorageBackend`. `Store` keeps
        // shared handles to the same maps for diagnostics/replay; the backend
        // owns the read view and the write path.
        let health = Arc::new(health::StoreHealth::default());
        let maintenance_abort = Arc::new(AtomicBool::new(false));
        let maps = memory_backend::MemoryMaps::new();
        let memory_backends: Box<[Option<Arc<dyn StorageBackend>>]> = (0..config.shard_count)
            .map(|_| {
                Some(Arc::new(memory_backend::MemoryBackend::new(
                    Arc::clone(&health),
                    memory_backend::MemoryMaps {
                        data: Arc::clone(&maps.data),
                        created: Arc::clone(&maps.created),
                        history: Arc::clone(&maps.history),
                    },
                    Arc::clone(&maintenance_abort),
                )) as Arc<dyn StorageBackend>)
            })
            .collect();
        let conflicts = conflict_window::ConflictWindow::charged_to(&resources);
        Ok(Store {
            resources,
            transaction_resources,
            logical_cache_resources,
            phase_resources,
            adapter_budgets,
            planner_stats,
            resource_config: config,
            data: Arc::clone(&maps.data),
            created: Arc::clone(&maps.created),
            history: Arc::clone(&maps.history),
            gc_watermark: AtomicU64::new(0),
            read_pins: Arc::new(Mutex::new(BTreeMap::new())),
            read_cancellations: Arc::new(Mutex::new(BTreeMap::new())),
            admission_lock: Mutex::new(SequencerState::default()),
            health,
            publisher_failure: Mutex::new(None),
            #[cfg(test)]
            fail_next_persist: std::sync::atomic::AtomicBool::new(false),
            shards: (0..N_SHARDS).map(|_| Mutex::new(())).collect(),
            publish_lock: Mutex::new(()),
            admission_queue: fair_gate::FairGate::default(),
            conflicts: Mutex::new(conflicts),
            pending: Mutex::new(BTreeMap::new()),
            pending_cv: Condvar::new(),
            next_commit_index: AtomicU64::new(1),
            published: AtomicU64::new(0),
            expander: RwLock::new(None),
            commit_observer: RwLock::new(None),
            deriver: RwLock::new(None),
            tenant_cache: Mutex::new(tenant_dir::TenantCache::default()),
            underived_replay: std::sync::atomic::AtomicBool::new(false),
            durable: RwLock::new(memory_backends),
            log: Mutex::new(None),
            replicator: RwLock::new(None),
            gc_cursor: Mutex::new(None),
            data_path: Mutex::new(None),
            copy_on_write: Mutex::new(None),
            maintenance_abort,
            log_rows_pruned: AtomicU64::new(0),
            receipts_swept: AtomicU64::new(0),
            receipt_clock: RwLock::new(Arc::new(wal::now_ms)),
            compactions: AtomicU64::new(0),
            group_commits: AtomicU64::new(0),
            admission_waits: AtomicU64::new(0),
            admission_wait_ms: AtomicU64::new(0),
            checkpoints: AtomicU64::new(0),
            checkpoint_wait_ms: AtomicU64::new(0),
            last_commit_ms: AtomicU64::new(0),
            last_compacted_file_bytes: AtomicU64::new(0),
            purged_since_compaction: AtomicU64::new(0),
        })
    }

    /// The store's managed-byte budget.
    pub fn resource_budget(&self) -> resources::ByteBudget {
        self.resources.clone()
    }

    /// The configuration this store was opened with.
    pub fn resource_config(&self) -> resources::StorageResources {
        self.resource_config
    }

    /// The byte budget charged for transaction staging.
    pub fn transaction_budget(&self) -> resources::ByteBudget {
        self.transaction_resources.clone()
    }

    /// Approximate bytes resident in the writer's current-value and history
    /// maps. This is the transitional RAM mirror P04 removes; the baseline
    /// harness records it so the "no full RAM mirror" milestone is
    /// measurable and comparable across revisions.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn approximate_map_bytes(&self) -> (u64, u64) {
        let data = self.health.cleanup(self.data.read());
        let current: u64 = data.iter().map(|(k, v)| (k.len() + v.len()) as u64).sum();
        drop(data);
        let history = self.health.cleanup(self.history.read());
        let hist: u64 = history
            .iter()
            .map(|(k, v)| {
                k.len() as u64
                    + v.iter()
                        .map(|e| e.value.as_ref().map_or(0, std::vec::Vec::len) as u64)
                        .sum::<u64>()
            })
            .sum();
        (current, hist)
    }

    /// Shard index for a tenant; commits without a tenant use shard 0.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the hash is immediately reduced modulo N_SHARDS, so truncating its high bits cannot change routing correctness"
    )]
    fn shard_for(tenant: Option<&TenantId>) -> usize {
        match tenant {
            Some(t) => {
                let mut h = 2_166_136_261u64;
                for b in &t.to_be_bytes() {
                    h ^= u64::from(*b);
                    h = h.wrapping_mul(16_777_619);
                }
                (h as usize) % N_SHARDS
            }
            None => 0,
        }
    }

    /// Open a crash-safe redb-backed store. The materialized keyspace and
    /// MVCC metadata are restored before the store is returned.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the resource configuration is invalid, the
    /// backend cannot be opened, or its durable watermark cannot be read.
    pub fn open(path: &std::path::Path) -> Result<Self, StorageError> {
        Self::open_with_resources(path, resources::StorageResources::default())
    }

    /// Open a redb-backed store with an explicit resource configuration.
    ///
    /// # Errors
    /// Returns [`StorageError`] if `config` is invalid, the shard backends
    /// cannot be opened, or their durable watermarks cannot be read.
    pub fn open_with_resources(
        path: &std::path::Path,
        config: resources::StorageResources,
    ) -> Result<Self, StorageError> {
        Self::open_with_deriver(path, config, None)
    }

    /// Open a redb-backed store with the apply-time deriver installed
    /// *before* WAL recovery, so replayed commits derive the same rows live
    /// publication would have.
    ///
    /// # Errors
    /// Returns [`StorageError`] if `config` is invalid, the shard backends
    /// cannot be opened, their watermarks cannot be read, or replay fails.
    pub fn open_with_deriver(
        path: &std::path::Path,
        config: resources::StorageResources,
        deriver: Option<Arc<dyn Deriver>>,
    ) -> Result<Self, StorageError> {
        let store = Self::new_with_resources(config)?;
        if let Some(deriver) = deriver {
            store.set_deriver(deriver);
        }
        store.attach_redb(path)?;
        Ok(store)
    }

    fn attach_redb(&self, path: &std::path::Path) -> Result<(), StorageError> {
        self.check_health()?;
        let shard_count = self.resource_config.shard_count;
        let layout_info =
            layout::validate_layout(path, shard_count).map_err(StorageError::Durability)?;
        // Before any file is created, so the shard files and the WAL inherit
        // it (SCALE-2: checkpoint fsyncs on copy-on-write stall every tenant).
        if let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .map_err(|error| StorageError::Durability(error.to_string()))?;
        }
        copy_on_write::opt_out(path);
        let cache_pool = self
            .resources
            .child("redb-cache", self.resource_config.page_cache_bytes)
            .map_err(|e| StorageError::ResourceExhausted(e.to_string()))?;
        let mut backends: Vec<Option<Arc<dyn StorageBackend>>> = Vec::with_capacity(shard_count);
        for shard in 0..shard_count {
            // Shard 0 keeps the historical single-file name; shard N uses a
            // suffixed file. With shard_count == 1 the layout is unchanged.
            let shard_path = if shard == 0 {
                path.to_path_buf()
            } else {
                let mut os = path.as_os_str().to_os_string();
                os.push(format!("-{shard}"));
                std::path::PathBuf::from(os)
            };
            // Split the configured page cache across physical shards.
            let shard_cache_bytes = self.resource_config.page_cache_bytes / shard_count;
            let cache = cache_pool
                .child("shard", shard_cache_bytes)
                .and_then(|pool| pool.try_reserve(shard_cache_bytes))
                .map_err(|e| StorageError::ResourceExhausted(e.to_string()))?;
            let backend =
                backend::open_backend(&shard_path, cache).map_err(StorageError::Durability)?;
            backends.push(Some(backend));
        }
        // Each shard file holds its tenants' data plus the global
        // log/outcome/last-applied rows for commits routed to it. The durable
        // LSN and GC floor are read for the watermark; the data/created/history
        // maps are NOT materialized: a redb-backed store reads natively
        // and stays memory-bounded regardless of dataset size.
        let mut gc_watermark = 0u64;
        let mut commit_index = 0u64;
        let mut materialization: Vec<u64> = Vec::with_capacity(shard_count);
        for backend in backends.iter().flatten() {
            let (lsn, floor) = backend.load_watermark().map_err(StorageError::Durability)?;
            materialization.push(lsn);
            gc_watermark = gc_watermark.max(floor);
            commit_index = commit_index.max(lsn);
        }
        // Log-first: open the segmented WAL (the durable truth) and replay any
        // tail beyond each shard's materialization.
        let (log, recovered) = self.open_log_and_replay(path, &backends, &materialization)?;
        commit_index = commit_index.max(recovered);
        self.gc_watermark.store(gc_watermark, Ordering::SeqCst);
        self.next_commit_index
            .store(commit_index + 1, Ordering::SeqCst);
        self.published.store(commit_index, Ordering::SeqCst);
        *self.health.guard(self.log.lock())? = Some(log);
        *self.health.guard(self.durable.write())? = backends.into_boxed_slice();
        *self.health.guard(self.data_path.lock())? = Some(path.to_path_buf());
        *self.health.guard(self.copy_on_write.lock())? = Some(copy_on_write::opt_out(path));
        // Idle compaction measures growth from the file as opened, not from
        // zero: with a zero baseline every boot compacted the whole file
        // under the publish lock (27 min at 10M records, SCALE-1).
        self.last_compacted_file_bytes
            .store(self.storage_file_bytes().unwrap_or(0), Ordering::Relaxed);
        if layout_info.is_none() {
            layout::create_manifest(path, shard_count).map_err(StorageError::Durability)?;
        }
        Ok(())
    }

    /// The durable backend for a physical shard, if this store has one.
    fn backend(&self, shard: usize) -> Option<Arc<dyn StorageBackend>> {
        self.health
            .cleanup(self.durable.read())
            .get(shard)
            .and_then(std::clone::Clone::clone)
    }

    /// Shard 0 backend (the global single-file layout).
    fn primary_backend(&self) -> Option<Arc<dyn StorageBackend>> {
        self.backend(0)
    }

    /// The planner-statistics cache: idle eviction and metrics.
    #[must_use]
    pub fn planner_stats(&self) -> &PlannerStats {
        &self.planner_stats
    }

    /// The persistent shared byte budget for logical caches, or `None`
    /// when `resources.logical-cache-bytes` is 0. Every retained cache/cursor
    /// reservation from this store contends on this same child.
    #[must_use]
    pub fn cache_budget(&self) -> Option<resources::ByteBudget> {
        self.logical_cache_resources.clone()
    }

    /// Shared byte budget for active query/adapter phases: one persistent
    /// child of the resource tree, so concurrent requests share one cap.
    #[must_use]
    pub fn phase_budget(&self) -> Option<resources::ByteBudget> {
        self.phase_resources.clone()
    }

    /// All open shard backends, in shard order.
    fn all_backends(&self) -> Vec<Arc<dyn StorageBackend>> {
        self.health
            .cleanup(self.durable.read())
            .iter()
            .flatten()
            .cloned()
            .collect()
    }

    /// Run `op` on every shard backend and collect the results in shard order,
    /// with the first error (in shard order) winning. Shards are independent
    /// files, so with more than one shard the work runs concurrently — this is
    /// what makes multi-shard GC/compaction scale instead of serializing.
    fn for_each_backend_parallel<T, F>(&self, op: F) -> Result<Vec<T>, StorageError>
    where
        T: Send,
        F: Fn(&dyn StorageBackend) -> Result<T, String> + Sync,
    {
        self.check_health()?;
        let backends = self.all_backends();
        if backends.len() <= 1 {
            let mut out = Vec::with_capacity(backends.len());
            for backend in &backends {
                out.push(op(backend.as_ref()).map_err(StorageError::Durability)?);
            }
            return Ok(out);
        }
        std::thread::scope(|scope| {
            let handles: Vec<_> = backends
                .iter()
                .map(|backend| scope.spawn(|| op(backend.as_ref())))
                .collect();
            let mut out = Vec::with_capacity(handles.len());
            let mut first_err: Option<String> = None;
            for handle in handles {
                match handle.join() {
                    Ok(Ok(value)) => out.push(value),
                    Ok(Err(error)) => {
                        if first_err.is_none() {
                            first_err = Some(error);
                        }
                    }
                    Err(_) => {
                        if first_err.is_none() {
                            first_err = Some("backend worker panicked".to_string());
                        }
                    }
                }
            }
            match first_err {
                Some(error) => Err(StorageError::Durability(error)),
                None => Ok(out),
            }
        })
    }

    /// Acquire the admission lock, honoring the configured acquisition timeout.
    /// A bounded wait turns unbounded writer queueing into a retryable
    /// `resource exhausted` instead of an unbounded stall (config #4).
    fn acquire_admission(&self) -> Result<std::sync::MutexGuard<'_, SequencerState>, StorageError> {
        self.check_health()?;
        let timeout_ms = self.resource_config().lock_acquisition_timeout_ms;
        if timeout_ms == 0 {
            // FIFO: the turn is held until the mutex is acquired, so waiting
            // writers acquire admission in arrival order (no starvation).
            let _turn = self.admission_queue.enter();
            return self.health.guard(self.admission_lock.lock());
        }
        let started = std::time::Instant::now();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            match self.admission_lock.try_lock() {
                Ok(guard) => {
                    let waited = started.elapsed();
                    if waited > std::time::Duration::ZERO {
                        self.admission_waits.fetch_add(1, Ordering::Relaxed);
                        self.admission_wait_ms.fetch_add(
                            u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
                            Ordering::Relaxed,
                        );
                    }
                    return Ok(guard);
                }
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    drop(poisoned);
                    self.health.fence();
                    return Err(StorageError::Unhealthy);
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    if std::time::Instant::now() >= deadline {
                        let waited = started.elapsed();
                        self.admission_waits.fetch_add(1, Ordering::Relaxed);
                        self.admission_wait_ms.fetch_add(
                            u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
                            Ordering::Relaxed,
                        );
                        return Err(StorageError::ResourceExhausted(
                            "admission lock acquisition timed out".into(),
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        }
    }

    /// Drop the in-flight prefix the published base now subsumes. Called at the
    /// top of every writer's admission critical section. It reads `published`
    /// once; if the publisher advances in between, the overlay just carries an
    /// already-published op redundantly until the next trim, which is harmless
    /// (the base holds the same value).
    fn trim_sequencer(&self, state: &mut SequencerState) {
        let published = self.published.load(Ordering::SeqCst);
        state.buffer.retain(|_, (ci, _)| *ci > published);
    }

    /// Assemble the pending overlay (current committed base + in-flight writes)
    /// used to expand a command without waiting for its predecessors to
    /// publish. The base is the live committed view, opened here and dropped
    /// with the overlay, so no read pin outlives the commit. The caller must
    /// hold the admission guard that owns `state`.
    fn sequencer_overlay(&self, state: &SequencerState) -> Result<Arc<OverlayView>, StorageError> {
        let base = Arc::new(self.read_view()?);
        let mut buffer: BTreeMap<Vec<u8>, Option<Vec<u8>>> = BTreeMap::new();
        for (key, (_, value)) in &state.buffer {
            buffer.insert(key.clone(), value.clone());
        }
        Ok(Arc::new(OverlayView::with_buffer(base, buffer)))
    }

    fn check_health(&self) -> Result<(), StorageError> {
        if self.data.is_poisoned()
            || self.created.is_poisoned()
            || self.history.is_poisoned()
            || self.publish_lock.is_poisoned()
            || self.admission_lock.is_poisoned()
            || self.pending.is_poisoned()
            || self.publisher_failure.is_poisoned()
            || self.read_pins.is_poisoned()
            || self.read_cancellations.is_poisoned()
            || self.expander.is_poisoned()
            || self.durable.is_poisoned()
            || self.log.is_poisoned()
            || self.replicator.is_poisoned()
            || self.gc_cursor.is_poisoned()
            || self.data_path.is_poisoned()
            || self.shards.iter().any(Mutex::is_poisoned)
        {
            self.health.fence();
        }
        self.health.check()
    }

    fn check_publisher(&self) -> Result<(), StorageError> {
        self.check_health()?;
        match self.health.guard(self.publisher_failure.lock())?.clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Return the terminal publisher failure, if durable publication has been
    /// fenced. Readiness callers use this to avoid routing traffic to a store
    /// whose later commits cannot become durable.
    ///
    /// Lock poisoning fences the store until reopen.
    #[must_use]
    pub fn publisher_failure(&self) -> Option<String> {
        if let Err(error) = self.check_health() {
            return Some(error.to_string());
        }
        self.health
            .cleanup(self.publisher_failure.lock())
            .as_ref()
            .map(ToString::to_string)
    }

    fn native_transaction_view(&self) -> Result<Option<redb_backend::MergedViews>, StorageError> {
        self.check_health()?;
        let backends = self.all_backends();
        if backends.is_empty() {
            return Ok(None);
        }
        let mut views: Vec<Box<dyn crate::backend::StorageReadView>> =
            Vec::with_capacity(backends.len());
        for backend in &backends {
            views.push(backend.read_view()?);
        }
        Ok(Some(redb_backend::MergedViews::new(views)))
    }

    /// Install the deterministic expander used to derive canonical and
    /// derived keys for every committed batch.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn set_expander(&self, expander: Arc<ExpandFn>) {
        *self.health.cleanup(self.expander.write()) = Some(expander);
    }

    /// Whether this store persists to a durable backend. A construction
    /// property (was a file opened?), not a property of the map contents.
    #[must_use]
    pub fn is_durable(&self) -> bool {
        self.health.cleanup(self.data_path.lock()).is_some()
    }

    /// Install a replication seam (e.g. an `OpenRaft` leader). When set, local
    /// commits route through the leader instead of publishing locally.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn set_replicator(&self, replicator: Option<Arc<dyn Replicator>>) {
        *self.health.cleanup(self.replicator.write()) = replicator;
    }

    fn replicator(&self) -> Option<Arc<dyn Replicator>> {
        self.health.cleanup(self.replicator.read()).clone()
    }

    /// Return the replication watermark when the configured replicator can
    /// expose one. A missing watermark is deliberately fail-closed for
    /// retention: history is retained until coverage is observable.
    #[must_use]
    pub fn replication_acknowledged_commit(&self) -> Option<u64> {
        self.replicator()
            .and_then(|replicator| replicator.acknowledged_commit_index())
    }

    /// Last durably persisted and applied commit index.
    pub fn committed_version(&self) -> u64 {
        self.published.load(Ordering::SeqCst)
    }

    /// Public idempotency probe: durable commit index for a commit key.
    pub fn committed_index_for(&self, commit_key: Uuid) -> Option<u64> {
        self.lookup_outcome(commit_key)
            .filter(|(st, _)| matches!(st, OutcomeStatus::Committed))
            .map(|(_, ci)| ci)
    }

    /// Public outcome probe for transaction coordinators: distinguishes a
    /// durable committed index from a durable aborted outcome.
    pub fn outcome_for(&self, commit_key: Uuid) -> Option<OutcomeKind> {
        self.lookup_outcome(commit_key).map(|(st, ci)| match st {
            OutcomeStatus::Committed => OutcomeKind::Committed(ci),
            OutcomeStatus::Aborted => OutcomeKind::Aborted,
        })
    }

    /// Durably record an aborted outcome under a commit key (used by
    /// rollback so a later commit attempt discovers `aborted`, never false
    /// success). No data is published.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the durable outcome write fails.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn record_aborted_outcome(&self, commit_key: Uuid) -> Result<(), StorageError> {
        self.check_health()?;
        let _admission = self.acquire_admission()?;
        self.record_aborted_outcome_admitted(commit_key)
    }

    fn record_aborted_outcome_admitted(&self, commit_key: Uuid) -> Result<(), StorageError> {
        self.check_health()?;
        let _guard = self.health.guard(self.publish_lock.lock())?;
        if self.lookup_outcome_checked(commit_key)?.is_some() {
            return Ok(());
        }
        // An aborted tx consumes no commit index: the terminal aborted outcome
        // is keyed by the commit key and does not occupy a sequence slot, so
        // the contiguous publisher never has to fill a hole for it.
        self.record_outcome_locked(commit_key, OutcomeStatus::Aborted, 0)?;
        Ok(())
    }

    /// Fallible latest-state point probe. Multi-read consumers must instead
    /// hold a `ReadView`. Write admission uses this under the logical writer.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed or the native
    /// backend read fails.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn try_get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.check_health()?;
        self.check_publisher()?;
        let native = self
            .native_transaction_view()?
            .ok_or(StorageError::Unhealthy)?;
        native.get_current(key, ReadLimits::default(), ScanProjection::KeyValue)
    }

    /// Fallible outcome probe that distinguishes committed from aborted and
    /// surfaces corrupt durable state.
    ///
    /// # Errors
    /// Returns [`StorageError::CorruptLog`] if the stored outcome row is
    /// malformed, or a backend error if the read fails.
    pub fn try_outcome_for(&self, commit_key: Uuid) -> Result<Option<OutcomeKind>, StorageError> {
        self.check_health()?;
        Ok(self
            .lookup_outcome_checked(commit_key)?
            .map(|(status, ci)| match status {
                OutcomeStatus::Committed => OutcomeKind::Committed(ci),
                OutcomeStatus::Aborted => OutcomeKind::Aborted,
            }))
    }

    /// Fallible exact-width `u64` read for engine callers.
    ///
    /// # Errors
    /// Returns [`StorageError::CorruptLog`] when a present value is not exactly
    /// eight bytes, or when the durable read fails.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn try_get_u64(&self, k: &[u8]) -> Result<Option<u64>, StorageError> {
        self.check_health()?;
        let bytes = self
            .native_transaction_view()?
            .ok_or(StorageError::Unhealthy)?
            .get_current(k, ReadLimits::default(), ScanProjection::KeyValue)?;
        let Some(bytes) = bytes else {
            return Ok(None);
        };
        let raw: [u8; 8] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| StorageError::CorruptLog)?;
        Ok(Some(u64::from_be_bytes(raw)))
    }

    /// Fallible historical prefix scan for engine callers.
    ///
    /// # Errors
    /// Returns backend or read-limit failures.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn try_range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        self.check_health()?;
        let view = self
            .native_transaction_view()?
            .ok_or(StorageError::Unhealthy)?;
        let (s, e) = prefix_range(prefix);
        let rows: Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> = view
            .current_range(&s, &e, ReadLimits::default(), ScanProjection::KeyValue)?
            .collect();
        rows
    }

    /// Fallible historical point read for engine callers.
    ///
    /// # Errors
    /// Returns backend or read-limit failures.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn try_get_at(&self, k: &[u8], snap: u64) -> Result<Option<Vec<u8>>, StorageError> {
        self.check_health()?;
        self.native_transaction_view()?
            .ok_or(StorageError::Unhealthy)?
            .get_at(k, snap, ReadLimits::default(), ScanProjection::KeyValue)
    }

    /// Fallible historical prefix scan for engine callers.
    ///
    /// # Errors
    /// Returns backend or read-limit failures.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn try_range_prefix_at(
        &self,
        prefix: &[u8],
        snap: u64,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        self.check_health()?;
        // Read directly through the backend view at `snap` (like `try_get_at`):
        // this is a fallible legacy probe, not the pinned `ReadView` path. It
        // therefore does not require the store's published watermark to have
        // caught up, which raw-apply test/recovery callers rely on.
        let view = self
            .native_transaction_view()?
            .ok_or(StorageError::Unhealthy)?;
        let limits = ReadLimits::default();
        let (s, e) = prefix_range(prefix);
        let mut lower = Included(s);
        let upper = if e.is_empty() {
            Unbounded
        } else {
            std::ops::Bound::Excluded(e)
        };
        let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        let mut bytes = 0usize;
        while let Some(key) = view.next_key(&lower, &upper, limits, true)? {
            if let Some(value) = view.get_at(&key, snap, limits, ScanProjection::KeyValue)? {
                bytes = bytes.saturating_add(key.len() + value.len());
                if bytes > 64 * 1024 * 1024 {
                    return Err(StorageError::ResourceExhausted(
                        "materialized read exceeds configured limit; use a cursor".into(),
                    ));
                }
                out.push((key.clone(), value));
            }
            lower = std::ops::Bound::Excluded(key);
        }
        Ok(out)
    }

    /// Advances retention: superseded versions covered before the watermark
    /// are reclaimed; versions still needed by any newer snapshot stay.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the durable reclamation fails.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn retire_before(&self, watermark: u64) -> Result<usize, StorageError> {
        self.check_health()?;
        let _publisher = self.health.guard(self.publish_lock.lock())?;
        self.retire_before_locked(watermark)
    }

    /// Bounded, resumable GC: reclaim history superseded before `watermark`
    /// for at most `max_keys` per shard in this call. Returns the next key to
    /// resume from (or `None` when every shard finished the walk). A
    /// background driver calls this repeatedly; it never performs one
    /// monolithic redb txn.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed or a shard's
    /// durable reclamation fails.
    ///
    /// # Panics
    /// Panics if the read-pin mutex or history `RwLock` is poisoned by a
    /// previous panic.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "public signature is a compatibility contract; the cursor is cloned per shard so taking it by value cannot change behavior"
    )]
    pub fn retire_before_batch(
        &self,
        watermark: u64,
        cursor: Option<Vec<u8>>,
        max_keys: usize,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        self.check_health()?;
        self.check_publisher()?;
        // Clamp identically to the monolithic path: never reclaim a version a
        // pinned read may still need, never advance the floor past the
        // committed version, and never move the floor backwards. The clamp is
        // taken under `read_pins`, the same lock reads use for their floor
        // check and pin insert, so no read can admit a snapshot below the new
        // floor.
        let watermark = {
            let pins = self.health.guard(self.read_pins.lock())?;
            let watermark = watermark.min(self.committed_version());
            let watermark = pins
                .first_key_value()
                .map_or(watermark, |(ci, _)| watermark.min(*ci));
            let watermark = watermark.max(self.gc_watermark());
            // Publish the floor *while holding* `read_pins`. A reader that
            // acquires the lock after this sees the new floor and is rejected;
            // a reader that acquired it before this is already reflected in
            // `pins`, so the clamp cannot be below its snapshot. Storing after
            // unlocking would let a reader admit a below-floor snapshot that
            // GC is about to reclaim.
            self.gc_watermark
                .store(watermark, std::sync::atomic::Ordering::SeqCst);
            watermark
        };
        // Reclaim holds no read lock, so readers are never blocked behind a GC
        // write transaction.
        // A single cursor is shared across shards. Resume from the smallest
        // per-shard cursor so no shard skips keys, and treat the sweep as
        // complete only when every shard reports no further work.
        let mut last: Option<Vec<u8>> = None;
        let mut all_done = true;
        for backend in self.all_backends() {
            if let Some(k) = backend
                .retire_before_batch(watermark, cursor.clone(), max_keys)
                .map_err(StorageError::Durability)?
            {
                all_done = false;
                last = Some(match last {
                    Some(prev) if prev <= k => prev,
                    _ => k,
                });
            }
        }
        if all_done {
            // Full walk completed on every shard: apply the same in-memory
            // retention the monolithic path applies after durable reclamation.
            let mut hist = self.health.guard(self.history.write())?;
            for entries in hist.values_mut() {
                entries.retain(|e| e.to_ci > watermark || e.to_ci == 0);
            }
            hist.retain(|_, v| !v.is_empty());
        }
        Ok(last)
    }

    /// One tick of the background GC loop: reclaim a bounded batch toward
    /// the current committed watermark. Returns `true` when the whole history
    /// table was swept this tick (a full GC pass completed and a checkpoint
    /// was published per shard), `false` when the sweep is mid-walk and the
    /// caller should tick again. Safe to call concurrently: each tick is a
    /// small redb write transaction.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed or a shard's
    /// durable reclamation fails.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn gc_tick(&self, max_keys: usize) -> Result<bool, StorageError> {
        self.check_health()?;
        let watermark = self.committed_version();
        let cursor = self.health.guard(self.gc_cursor.lock())?.clone();
        let next = self.retire_before_batch(watermark, cursor, max_keys)?;
        if let Some(k) = next {
            *self.health.guard(self.gc_cursor.lock())? = Some(k);
            Ok(false)
        } else {
            // Sweep complete: reset the cursor so the next pass starts
            // from the beginning (new histories are reclaimed too).
            *self.health.guard(self.gc_cursor.lock())? = None;
            Ok(true)
        }
    }

    /// [`Self::gc_tick`] in batches of `max_keys` until the history sweep
    /// completes or `budget` has elapsed, so reclamation keeps up with the
    /// write rate instead of retiring a fixed number of keys per maintenance
    /// tick (SOAK-1: 512 keys per 5 s fell behind ~750 rewrites/s and history
    /// grew without bound). Returns whether the sweep completed.
    ///
    /// # Errors
    /// See [`Self::gc_tick`].
    pub fn gc_tick_for(
        &self,
        max_keys: usize,
        budget: std::time::Duration,
    ) -> Result<bool, StorageError> {
        let started = std::time::Instant::now();
        loop {
            if self.gc_tick(max_keys)? {
                return Ok(true);
            }
            if started.elapsed() >= budget {
                return Ok(false);
            }
        }
    }

    fn retire_before_locked(&self, watermark: u64) -> Result<usize, StorageError> {
        self.check_health()?;
        self.check_publisher()?;
        let watermark = {
            let pins = self.health.guard(self.read_pins.lock())?;
            let watermark = watermark.min(self.committed_version());
            let watermark = pins
                .first_key_value()
                .map_or(watermark, |(ci, _)| watermark.min(*ci));
            let watermark = watermark.max(self.gc_watermark());
            // Fence reads below the new floor while still holding `read_pins`
            // (see `retire_before_batch`). A failed durable reclamation leaves
            // the floor advanced — a conservative, fail-closed state — never
            // the reverse.
            self.gc_watermark
                .store(watermark, std::sync::atomic::Ordering::SeqCst);
            watermark
        };
        // Prune the in-memory maps first so the reclaimed-version count is
        // observable (the durable maps are empty; the backend does its own
        // reclamation below). `MemoryBackend::retire_before` then re-prunes the
        // same maps idempotently.
        let mut removed = 0usize;
        {
            let mut hist = self.health.guard(self.history.write())?;
            for entries in hist.values_mut() {
                let before = entries.len();
                entries.retain(|e| e.to_ci > watermark || e.to_ci == 0);
                removed += before - entries.len();
            }
            hist.retain(|_, v| !v.is_empty());
        }
        self.for_each_backend_parallel(|backend| backend.retire_before(watermark))?;
        Ok(removed)
    }

    /// Oldest supported logical commit index, inclusive. New historical views
    /// below this floor fail, even if an individual key still has older bytes.
    pub fn gc_watermark(&self) -> u64 {
        self.gc_watermark.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Expire retained read leases older than `max_age_ms` (P06: reader age
    /// limits with safe cancellation-before-release). An abandoned reader that
    /// never drops its view cannot block history advancement forever; the GC
    /// worker calls this so the floor can move. Returns the count of leases
    /// released (CI entries whose remaining pin count reached zero).
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn expire_read_pins(&self, max_age_ms: u64) -> usize {
        if self.check_health().is_err() {
            return 0;
        }
        let now = wal::now_ms();
        let mut pins = self.health.cleanup(self.read_pins.lock());
        let mut cancellations = self.health.cleanup(self.read_cancellations.lock());
        let mut released = 0usize;
        pins.retain(|ci, pin| {
            if now.saturating_sub(pin.lease_ms) > max_age_ms {
                if let Some(handles) = cancellations.remove(ci) {
                    for handle in handles {
                        handle.cancel();
                    }
                }
                released += 1;
                false
            } else {
                true
            }
        });
        released
    }

    /// Observability: the oldest active pin (inclusive CI floor) and how
    /// many distinct CIs are retained. Lets operators see long-reader retention
    /// and confirm it releases after handles close.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn retention_summary(&self) -> (Option<u64>, usize) {
        let pins = self.health.cleanup(self.read_pins.lock());
        (pins.first_key_value().map(|(ci, _)| *ci), pins.len())
    }

    /// Number of keys in the sequencer's assigned-but-unpublished overlay.
    /// Observability for the single-writer in-flight window; returns 0
    /// when the admission lock is poisoned rather than panicking a scrape.
    #[must_use]
    pub fn sequencer_inflight_keys(&self) -> usize {
        self.admission_lock
            .lock()
            .map_or(0, |state| state.buffer.len())
    }

    /// Reclaim versions superseded at or before the requested watermark,
    /// clamped by protected read views and the committed CI. Legacy unpinned
    /// readers must still coordinate their own retention. Returns reclaimed
    /// version count; canonical current data and command logs are unchanged.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the durable reclamation fails.
    pub fn gc(&self, watermark: u64) -> Result<usize, StorageError> {
        self.check_health()?;
        self.retire_before(watermark)
    }

    fn record_outcome_locked(
        &self,
        commit_key: Uuid,
        status: OutcomeStatus,
        ci: u64,
    ) -> Result<(), StorageError> {
        self.check_health()?;
        let ops: Vec<Op> = receipt::rows(commit_key, status, ci, self.receipt_now(), None)
            .into_iter()
            .map(|(key, value)| Op::Put(key, value))
            .collect();
        // Aborted-outcome idempotency markers are local metadata, not part of
        // the committed log sequence (they carry no commit index), so they are
        // written to the materialization directly rather than the WAL.
        self.persist_ops_local(&ops, ci)
    }

    /// Current time of the receipt clock (unix ms).
    fn receipt_now(&self) -> u64 {
        (self.health.cleanup(self.receipt_clock.read()).clone())()
    }

    /// Replace the receipt clock (tests age receipts with a mock clock).
    #[doc(hidden)]
    pub fn set_receipt_clock(&self, clock: Arc<dyn Fn() -> u64 + Send + Sync>) {
        *self.health.cleanup(self.receipt_clock.write()) = clock;
    }

    /// Stage the receipt of `commit_key` in `txn`.
    fn put_receipt(
        &self,
        txn: &mut Txn<'_>,
        commit_key: Uuid,
        status: OutcomeStatus,
        ci: u64,
        payload: Option<&[u8]>,
    ) {
        for (key, value) in receipt::rows(commit_key, status, ci, self.receipt_now(), payload) {
            txn.put(key, value);
        }
    }

    /// The live receipt of `commit_key`: `None` when there is none or it is
    /// past the idempotency window (a retry then executes as a new commit).
    fn live_receipt(&self, commit_key: Uuid) -> Result<Option<receipt::Receipt>, StorageError> {
        let Some(raw) = self.try_get(&receipt::receipt_key(commit_key))? else {
            return Ok(None);
        };
        let receipt = receipt::decode(&raw)?;
        let window_ms = self
            .resource_config
            .idempotency_window_seconds
            .saturating_mul(1000);
        Ok((!receipt.expired(self.receipt_now(), window_ms)).then_some(receipt))
    }

    /// Apply an op-set to the materialization without a WAL record. Used only
    /// for local idempotency metadata that is not part of the commit log.
    fn persist_ops_local(&self, ops: &[Op], commit_index: u64) -> Result<(), StorageError> {
        self.check_health()?;
        self.check_publisher()?;
        if let Some(backend) = self.primary_backend() {
            backend
                .apply(ops, commit_index)
                .map_err(StorageError::Durability)?;
        }
        Ok(())
    }

    fn lookup_outcome(&self, commit_key: Uuid) -> Option<(OutcomeStatus, u64)> {
        self.lookup_outcome_checked(commit_key).ok().flatten()
    }

    fn lookup_outcome_checked(
        &self,
        commit_key: Uuid,
    ) -> Result<Option<(OutcomeStatus, u64)>, StorageError> {
        self.check_health()?;
        Ok(self
            .live_receipt(commit_key)?
            .map(|receipt| (receipt.status, receipt.commit_index)))
    }

    /// The stored response of a committed `commit_key` (its retry answer).
    fn committed_payload<P: serde::de::DeserializeOwned>(
        &self,
        commit_key: Uuid,
    ) -> Result<P, StorageError> {
        let payload = self
            .live_receipt(commit_key)?
            .and_then(|receipt| receipt.payload)
            .ok_or(StorageError::OutcomePayload)?;
        serde_json::from_slice(&payload).map_err(|_| StorageError::OutcomePayload)
    }

    /// Rebuilds state by replaying the full serialized command log through
    /// the registered deterministic expander.
    ///
    /// # Errors
    /// Returns [`StorageError::CorruptLog`] for malformed input or missing
    /// expansion support, and [`StorageError::Builder`] when deterministic
    /// expansion rejects an entry. No replayed state is published on decode or
    /// expansion failure.
    ///
    /// # Panics
    /// Panics only if an internal lock is poisoned by a previous panic.
    pub fn try_replay(&self, log: &[Vec<u8>]) -> Result<(), StorageError> {
        self.check_health()?;
        // A replay input is a committed command log, so silently dropping a
        // malformed entry would renumber every later commit and produce a
        // state that looks valid while diverging from the durable history.
        // Recovery callers that intentionally trim a torn tail must do so
        // before invoking replay (as the WAL reader does).
        let entries: Vec<CommitBatch> = log
            .iter()
            .map(|raw| {
                rushdb_types::command_codec::decode(raw).map_err(|_| StorageError::CorruptLog)
            })
            .collect::<Result<_, _>>()?;
        let expander = self.health.guard(self.expander.read())?.clone();
        let mut prepared = Vec::with_capacity(entries.len());
        // Expansion of a later command may legitimately depend on canonical
        // state produced by an earlier command (for example, a text
        // watermark refers to a profile created in the preceding commit).
        // Build a private shadow map while preparing the complete replay so
        // those dependencies are visible without publishing partial state.
        let mut shadow = self.health.guard(self.data.read())?.clone();

        for (i, batch) in entries.iter().enumerate() {
            let ci = i as u64 + 1;
            let Some(expand) = expander.as_ref() else {
                return Err(StorageError::CorruptLog);
            };
            let ops;
            {
                // Full command replay still builds the reference state, so
                // the base is always the live shadow map, never native.
                let mut txn = Txn {
                    base: TxnBase::Live(&shadow),
                    buffer: BTreeMap::new(),
                    purge_keys: BTreeSet::default(),
                    set_keys: BTreeSet::default(),
                    deps: None,
                };
                expand(ci, batch, &mut txn).map_err(StorageError::Builder)?;
                self.put_receipt(
                    &mut txn,
                    batch.commit_key,
                    OutcomeStatus::Committed,
                    ci,
                    None,
                );
                txn.set(LAST_APPLIED_KEY.to_vec(), wal::encode_last_applied(0, ci));
                ops = txn.drain();
            }
            // Command replay expands against a base-only shadow; fold the
            // derived rows in too, so an expander that reads derived counters
            // (DeleteFamily) sees what a live expansion would.
            let derived =
                Self::derive_ops(self.deriver().as_ref(), ci, &ops, &derive::MapRead(&shadow))?;
            for op in derived.as_deref().unwrap_or(&ops) {
                match op {
                    Op::Put(key, value) | Op::Set(key, value) => {
                        shadow.insert(key.clone(), value.clone());
                    }
                    Op::Del(key) | Op::Purge(key) => {
                        shadow.remove(key);
                    }
                }
            }
            prepared.push((ci, ops, rushdb_types::command_codec::encode(batch)));
        }
        for (ci, ops, command_payload) in prepared {
            // Apply through the backend: one write path for memory and
            // durable replay; `persist_ops` derives the same derived rows the
            // live publisher would, never shipped over the wire.
            self.persist_ops(&ops, ci, Some(&command_payload))?;
            self.health.guard(self.conflicts.lock())?.record(ci, &ops);
        }
        {
            let applied = entries.len() as u64;
            self.next_commit_index.store(applied + 1, Ordering::SeqCst);
            self.published.store(applied, Ordering::SeqCst);
            self.last_commit_ms.store(wal::now_ms(), Ordering::Relaxed);
            self.persist_ops(
                &[Op::Set(
                    LAST_APPLIED_KEY.to_vec(),
                    wal::encode_last_applied(0, applied),
                )],
                applied,
                None,
            )?;
        }
        Ok(())
    }

    /// Apply a command batch already committed by `OpenRaft`. This bypasses
    /// admission-time planning but uses exactly the same deterministic
    /// expander and durable publication path as a local commit.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, the batch has a
    /// terminal aborted outcome, or expansion/publication fails.
    pub fn apply_replicated_batch(&self, batch: &CommitBatch) -> Result<u64, StorageError> {
        self.check_health()?;
        self.apply_replicated_batch_with_payload(batch, None)
    }

    /// Apply a replicated batch and durably store its serialized payload with
    /// the outcome so idempotent retries on the leader reproduce the original
    /// caller response.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, no expander is
    /// installed, the batch has a terminal aborted outcome, or publication
    /// fails.
    ///
    /// # Panics
    /// Panics only if `batch` fails to serialize (never, for a well-formed
    /// `CommitBatch`). Lock poisoning fences the store until reopen.
    pub fn apply_replicated_batch_with_payload(
        &self,
        batch: &CommitBatch,
        payload: Option<&[u8]>,
    ) -> Result<u64, StorageError> {
        self.check_health()?;
        let _admission = self.acquire_admission()?;
        self.check_publisher()?;
        if let Some((OutcomeStatus::Committed, commit_index)) =
            self.lookup_outcome_checked(batch.commit_key)?
        {
            return Ok(commit_index);
        }

        if matches!(
            self.try_outcome_for(batch.commit_key)?,
            Some(OutcomeKind::Aborted)
        ) {
            return Err(StorageError::Builder(
                "commit key has a terminal aborted outcome".into(),
            ));
        }
        let commit_index = self.next_commit_index.load(Ordering::SeqCst);
        let ops = {
            let guard = self.health.guard(self.data.read())?;
            let native = self.native_transaction_view()?;
            let base = TxnBase::resolve(None, None, native, Some(&guard));
            let mut txn = Txn {
                base,
                buffer: BTreeMap::new(),
                purge_keys: BTreeSet::default(),
                set_keys: BTreeSet::default(),
                deps: None,
            };
            let expander = self.health.guard(self.expander.read())?.clone();
            let Some(expand) = expander.as_ref() else {
                return Err(StorageError::CorruptLog);
            };
            expand(commit_index, batch, &mut txn).map_err(StorageError::Builder)?;
            self.put_receipt(
                &mut txn,
                batch.commit_key,
                OutcomeStatus::Committed,
                commit_index,
                payload,
            );
            txn.set(
                LAST_APPLIED_KEY.to_vec(),
                wal::encode_last_applied(0, commit_index),
            );
            txn.drain()
        };
        // Large transactions: the same ceiling `commit_sharded`
        // enforces before its own CAS reservation (see `commit.rs`) applies
        // here too, before `next_commit_index` advances — this path has its
        // own, separate reservation (no CAS loop; a replicated batch's index
        // is assigned directly from the leader's already-ordered stream).
        // Operationally this means a follower configured with a *smaller*
        // ceiling than the leader will stall (loudly, retried forever by
        // `replication.rs`'s follower loop) on a commit the leader already
        // accepted, rather than silently diverging or truncating it —
        // deliberately: this is a config-mismatch to fix, not a case to
        // paper over. Chaining itself doesn't need this check to write a
        // large commit correctly; it's policy consistency, not correctness.
        let command_payload = rushdb_types::command_codec::encode(batch);
        let expanded_bytes = wal::op_set_encoded_len(&ops).saturating_add(command_payload.len());
        if expanded_bytes > self.resource_config.max_expanded_commit_bytes {
            return Err(StorageError::ResourceExhausted(format!(
                "expanded mutation batch exceeds {} bytes (got {expanded_bytes})",
                self.resource_config.max_expanded_commit_bytes
            )));
        }
        self.next_commit_index
            .store(commit_index + 1, Ordering::SeqCst);
        let shard =
            resources::physical_shard_for(Some(&batch.tenant), self.resource_config.shard_count);
        self.enqueue_and_publish(commit_index, shard, ops, command_payload)?;
        Ok(commit_index)
    }
}
