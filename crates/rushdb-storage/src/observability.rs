//! Live store observability hooks: per-commit latency, backend lock
//! contention, and the conflict window's footprint (C0, cloud-capacity F12).

use std::sync::Arc;

use super::Store;

/// One timed step of the commit pipeline. Committer-side phases are
/// observed once per commit; publisher-side phases once per publish group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitPhase {
    /// Waiting for the admission (sequencer) lock.
    AdmissionWait,
    /// Running the caller's planner/build closure.
    Build,
    /// Deterministic expansion into ops, including slot-reservation retries.
    Expand,
    /// Enqueue until the durable acknowledgement (the end-to-end latency the
    /// client waits for after expansion).
    Durable,
    /// Publisher: encoding a group's WAL records.
    WalEncode,
    /// Publisher: appending and fsyncing a group's WAL records.
    WalAppendSync,
    /// Publisher: applying a group's ops to the backend materialization.
    BackendApply,
    /// Publisher: advancing derived state for a group.
    Derive,
    /// Publisher: conflict-window recording, watermark advance and acks.
    Publish,
}

impl CommitPhase {
    /// Every phase, in pipeline order.
    pub const ALL: [Self; 9] = [
        Self::AdmissionWait,
        Self::Build,
        Self::Expand,
        Self::Durable,
        Self::WalEncode,
        Self::WalAppendSync,
        Self::BackendApply,
        Self::Derive,
        Self::Publish,
    ];

    /// Stable metric label for this phase.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AdmissionWait => "admission_wait",
            Self::Build => "build",
            Self::Expand => "expand",
            Self::Durable => "durable",
            Self::WalEncode => "wal_encode",
            Self::WalAppendSync => "wal_append_sync",
            Self::BackendApply => "backend_apply",
            Self::Derive => "derive",
            Self::Publish => "publish",
        }
    }
}

/// Callback receiving each timed commit phase, in seconds.
pub type CommitObserver = dyn Fn(CommitPhase, f64) + Send + Sync;

/// Cumulative backend database-lock timings, summed over every shard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackendLockTimes {
    /// Nanoseconds spent waiting to acquire the lock.
    pub wait_ns: u64,
    /// Nanoseconds the lock was held.
    pub hold_ns: u64,
    /// Longest single hold, in nanoseconds.
    pub hold_max_ns: u64,
}

/// What holds a backend's single writer: one redb write transaction, from
/// `begin_write` to its commit. Every other writer waits meanwhile, so the
/// longest holds are what stall other tenants' commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriterHolder {
    /// Publishing commit groups into the materialization.
    Publish,
    /// One batch of history garbage collection.
    HistoryGc,
    /// A durable checkpoint (fsync).
    Checkpoint,
    /// Writes paused for a consistent copy (backup).
    Pause,
}

impl WriterHolder {
    /// Every holder.
    pub const ALL: [Self; 4] = [
        Self::Publish,
        Self::HistoryGc,
        Self::Checkpoint,
        Self::Pause,
    ];

    /// Stable metric label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::HistoryGc => "history_gc",
            Self::Checkpoint => "checkpoint",
            Self::Pause => "pause",
        }
    }

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Publish => 0,
            Self::HistoryGc => 1,
            Self::Checkpoint => 2,
            Self::Pause => 3,
        }
    }
}

/// Upper bounds, in seconds, of the writer-hold histogram buckets.
pub const WRITER_HOLD_BOUNDS: [f64; 12] = [
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// One holder's writer holds since the store opened.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WriterHolds {
    /// Holds per bucket of [`WRITER_HOLD_BOUNDS`] (not cumulative); the last
    /// slot counts holds above every bound.
    pub counts: [u64; WRITER_HOLD_BOUNDS.len() + 1],
    /// Total seconds held.
    pub sum_seconds: f64,
}

impl WriterHolds {
    /// Fold `other` into `self`.
    #[must_use]
    pub fn plus(mut self, other: &Self) -> Self {
        for (total, count) in self.counts.iter_mut().zip(other.counts) {
            *total = total.saturating_add(count);
        }
        self.sum_seconds += other.sum_seconds;
        self
    }
}

impl Store {
    /// Install the callback that receives every timed commit phase.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn set_commit_observer(&self, observer: Arc<CommitObserver>) {
        *self.health.cleanup(self.commit_observer.write()) = Some(observer);
    }

    /// Report the time since `started` for `phase` to the installed observer.
    pub(crate) fn observe_phase(&self, phase: CommitPhase, started: std::time::Instant) {
        let observer = self.health.cleanup(self.commit_observer.read()).clone();
        if let Some(observer) = observer {
            observer(phase, started.elapsed().as_secs_f64());
        }
    }

    /// Backend lock timings summed over all shards (live, no scan).
    #[must_use]
    pub fn backend_lock_times(&self) -> BackendLockTimes {
        self.all_backends()
            .iter()
            .map(|backend| backend.lock_times())
            .fold(BackendLockTimes::default(), |total, times| {
                BackendLockTimes {
                    wait_ns: total.wait_ns.saturating_add(times.wait_ns),
                    hold_ns: total.hold_ns.saturating_add(times.hold_ns),
                    hold_max_ns: total.hold_max_ns.max(times.hold_max_ns),
                }
            })
    }

    /// Writer holds by holder, summed over every shard (live, no scan).
    #[must_use]
    pub fn writer_holds(&self) -> Vec<(WriterHolder, WriterHolds)> {
        let mut totals = [WriterHolds::default(); WriterHolder::ALL.len()];
        for backend in self.all_backends() {
            for (holder, holds) in backend.writer_holds() {
                totals[holder.index()] = totals[holder.index()].plus(&holds);
            }
        }
        WriterHolder::ALL.into_iter().zip(totals).collect()
    }

    /// Copy-on-write state of a durable store's data files, recorded when it
    /// opened; `None` in memory.
    #[must_use]
    pub fn copy_on_write(&self) -> Option<crate::CopyOnWrite> {
        self.health.cleanup(self.copy_on_write.lock()).clone()
    }

    /// Directory holding a durable store's files, or `None` in memory.
    #[must_use]
    pub fn data_dir(&self) -> Option<std::path::PathBuf> {
        self.health
            .cleanup(self.data_path.lock())
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf)
    }

    /// Bytes currently retained by the in-memory conflict window.
    #[must_use]
    pub fn conflict_window_bytes(&self) -> usize {
        self.health.cleanup(self.conflicts.lock()).bytes()
    }
}
