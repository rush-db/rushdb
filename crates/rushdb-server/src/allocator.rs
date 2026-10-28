//! Process-wide allocator policy.
//!
//! The default system allocator (glibc malloc) retains freed bulk-load memory
//! in per-thread arenas with no decay, so resident set size stays at its
//! high-water mark long after peak allocations are freed. With the `jemalloc`
//! feature (on by default), the server instead uses jemalloc with background
//! purge threads and jemalloc's default dirty/muzzy decay: steady-state
//! allocation stays fast, while idle pages are returned to the OS.
//!
//! This module also exposes allocator introspection for `/metrics` so the
//! managed-budget gauges (`rushdb_budget_*`, which track reservation tokens,
//! not bytes actually mapped) are complemented by real resident numbers.

#[cfg(all(feature = "jemalloc", not(feature = "alloc-accounting")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// Measurement builds only (`alloc-accounting`): jemalloc wrapped with a
/// cumulative allocated-bytes counter, so benchmarks can report allocation
/// *churn* per record. `stats.allocated` is a live-bytes gauge, so its delta
/// cannot measure churn. Off by default; release builds are unchanged.
#[cfg(feature = "alloc-accounting")]
#[global_allocator]
static GLOBAL: accounting::Counting = accounting::Counting;

#[cfg(feature = "alloc-accounting")]
mod accounting {
    use std::alloc::{GlobalAlloc, Layout};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    const SHARDS: usize = 64;

    /// One cache line per shard so threads never contend on a counter.
    #[repr(align(64))]
    pub(super) struct Shard(pub(super) AtomicU64);

    pub(super) static TOTALS: [Shard; SHARDS] = [const { Shard(AtomicU64::new(0)) }; SHARDS];
    static NEXT_SLOT: AtomicUsize = AtomicUsize::new(0);

    thread_local! {
        static SLOT: Cell<usize> = const { Cell::new(usize::MAX) };
    }

    pub(super) struct Counting;

    fn count(bytes: usize) {
        let slot = SLOT
            .try_with(|slot| {
                if slot.get() == usize::MAX {
                    slot.set(NEXT_SLOT.fetch_add(1, Ordering::Relaxed) % SHARDS);
                }
                slot.get()
            })
            .unwrap_or(0);
        TOTALS[slot]
            .0
            .fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::Relaxed);
    }

    pub(super) fn total() -> u64 {
        TOTALS
            .iter()
            .map(|shard| shard.0.load(Ordering::Relaxed))
            .fold(0, u64::saturating_add)
    }

    // SAFETY: every method forwards to jemalloc with the caller's arguments
    // unchanged; the counter has no effect on allocation behavior.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            count(layout.size());
            // SAFETY: forwarded contract.
            unsafe { tikv_jemallocator::Jemalloc.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            count(layout.size());
            // SAFETY: forwarded contract.
            unsafe { tikv_jemallocator::Jemalloc.alloc_zeroed(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: forwarded contract.
            unsafe { tikv_jemallocator::Jemalloc.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            count(new_size);
            // SAFETY: forwarded contract.
            unsafe { tikv_jemallocator::Jemalloc.realloc(ptr, layout, new_size) }
        }
    }
}

/// Cumulative bytes requested from the allocator since start, when built
/// with `alloc-accounting`; `None` otherwise.
#[must_use]
pub fn bytes_allocated_total() -> Option<u64> {
    #[cfg(feature = "alloc-accounting")]
    {
        Some(accounting::total())
    }
    #[cfg(not(feature = "alloc-accounting"))]
    {
        None
    }
}

/// Which allocator backs this build. Reported via metrics and startup logs.
#[must_use]
pub fn name() -> &'static str {
    if cfg!(feature = "jemalloc") {
        "jemalloc"
    } else {
        "system"
    }
}

/// Apply the allocator policy. Must run early in `main`, before worker
/// threads spawn. Never panics: mallctl failures are logged and the server
/// keeps running with allocator defaults.
pub fn configure() {
    #[cfg(feature = "jemalloc")]
    {
        // Background purge threads return decayed pages to the OS off the
        // request path. Decay itself stays at jemalloc's default (10 s): the
        // 1 s override was plumbing for per-commit churn that the cloud-fit work
        // removed (allocation per ingested record 858 KB → ~179 KB), so RSS
        // now stays bounded without it.
        if let Err(error) = tikv_jemalloc_ctl::background_thread::write(true) {
            eprintln!("allocator: background_thread unavailable: {error}");
        }
    }
}

/// Allocator introspection snapshot for metrics. All values are bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct AllocatorStats {
    /// Bytes allocated by the application (`stats.allocated`).
    pub allocated: usize,
    /// Bytes in active pages (`stats.active`).
    pub active: usize,
    /// Bytes used for allocator metadata (`stats.metadata`).
    pub metadata: usize,
    /// Bytes physically resident (`stats.resident`).
    pub resident: usize,
    /// Bytes retained in arenas but not active (`stats.retained`).
    pub retained: usize,
}

/// Read current allocator counters. Returns `None` on the system allocator
/// (no introspection API) or if the epoch cannot advance.
#[must_use]
pub fn stats() -> Option<AllocatorStats> {
    #[cfg(feature = "jemalloc")]
    {
        if tikv_jemalloc_ctl::epoch::advance().is_err() {
            return None;
        }
        let read = |value: Result<usize, _>| value.unwrap_or(0);
        Some(AllocatorStats {
            allocated: read(tikv_jemalloc_ctl::stats::allocated::read()),
            active: read(tikv_jemalloc_ctl::stats::active::read()),
            metadata: read(tikv_jemalloc_ctl::stats::metadata::read()),
            resident: read(tikv_jemalloc_ctl::stats::resident::read()),
            retained: read(tikv_jemalloc_ctl::stats::retained::read()),
        })
    }
    #[cfg(not(feature = "jemalloc"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn name_matches_feature_flag() {
        assert_eq!(
            super::name(),
            if cfg!(feature = "jemalloc") {
                "jemalloc"
            } else {
                "system"
            }
        );
    }

    #[test]
    fn configure_never_panics_and_stats_are_consistent() {
        super::configure();
        if let Some(stats) = super::stats() {
            assert!(stats.allocated > 0, "allocator reports no bytes");
            assert!(
                stats.resident >= stats.allocated,
                "resident {} < allocated {}",
                stats.resident,
                stats.allocated
            );
        }
    }
}
