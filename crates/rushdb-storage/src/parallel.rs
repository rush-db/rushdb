//! Intra-query parallelism under one process-wide thread budget.
//!
//! A long read (an aggregate over a million records, an exact vector scan)
//! can split its key range into parts and run them on scoped threads. Every
//! query draws helper threads from one shared budget, sized to the server's
//! worker threads (`RUSHDB_THREADS`), so concurrent queries never run more
//! threads than the host was given: a query that finds the budget spent runs
//! its parts on its own thread. Parts are numbered and their results are
//! returned in part order, so a caller that merges them in order gets the
//! same answer as a sequential run whatever the timing.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Helper threads not currently lent to a query.
static FREE: AtomicUsize = AtomicUsize::new(0);
/// The configured budget (for diagnostics and tests).
static BUDGET: AtomicUsize = AtomicUsize::new(0);

/// Set the helper-thread budget: the host's worker threads less the one
/// each query already runs on. Called once at startup; `0` = run every part
/// on the calling thread.
pub fn set_threads(threads: usize) {
    let helpers = threads.saturating_sub(1);
    let previous = BUDGET.swap(helpers, Ordering::SeqCst);
    if helpers >= previous {
        FREE.fetch_add(helpers - previous, Ordering::SeqCst);
    } else {
        let _ = FREE.try_update(Ordering::SeqCst, Ordering::SeqCst, |free| {
            Some(free.saturating_sub(previous - helpers))
        });
    }
}

/// The helper-thread budget [`set_threads`] configured.
#[must_use]
pub fn budget() -> usize {
    BUDGET.load(Ordering::SeqCst)
}

/// The threads helpers run on, started on first use. As many as the host has
/// cores, or the budget if larger: the lease, not the pool, bounds how many
/// helpers run at once.
fn pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        rayon::ThreadPoolBuilder::new()
            .num_threads(cores.max(budget()).max(1))
            .thread_name(|index| format!("rushdb-part-{index}"))
            .build()
            .expect("the part thread pool starts")
    })
}

/// Helper threads lent to one call, returned when dropped.
struct Lease(usize);

impl Lease {
    fn take(wanted: usize) -> Self {
        let mut taken = 0;
        let _ = FREE.try_update(Ordering::SeqCst, Ordering::SeqCst, |free| {
            taken = free.min(wanted);
            Some(free - taken)
        });
        Self(taken)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        FREE.fetch_add(self.0, Ordering::SeqCst);
    }
}

/// One helper thread for a background job (a resident build), returned
/// when dropped. Waits until the budget has one free, so background work
/// never runs beside a budget's worth of query parts. With no budget
/// configured (`set_threads` never called) the job runs at once.
pub(crate) fn background_lease() -> impl Drop + Send {
    let mut wait = std::time::Duration::from_micros(200);
    loop {
        if budget() == 0 {
            return Lease(0);
        }
        let lease = Lease::take(1);
        if lease.0 == 1 {
            return lease;
        }
        drop(lease);
        std::thread::sleep(wait);
        wait = (wait * 2).min(std::time::Duration::from_millis(20));
    }
}

/// Run `part(0..parts)` and return the results in part order. Parts run on
/// the calling thread plus as many helper threads as the shared budget lends
/// right now (at most `parts - 1`); with none free, they run one after
/// another here. The first error (by part order) is returned.
///
/// # Errors
/// Returns the error of the lowest-numbered failing part.
///
/// # Panics
/// Re-raises a panic of a part; a part's slot is always filled before the
/// threads join.
pub fn run_parts<T, E, F>(parts: usize, part: F) -> Result<Vec<T>, E>
where
    T: Send,
    E: Send,
    F: Fn(usize) -> Result<T, E> + Sync,
{
    if parts <= 1 {
        return (0..parts).map(&part).collect();
    }
    let lease = Lease::take(parts - 1);
    if lease.0 == 0 {
        return (0..parts).map(&part).collect();
    }
    let next = AtomicUsize::new(0);
    let workers = lease.0 + 1;
    let mut slots: Vec<Option<Result<T, E>>> = (0..parts).map(|_| None).collect();
    let results = std::sync::Mutex::new(&mut slots);
    let work = || loop {
        let index = next.fetch_add(1, Ordering::Relaxed);
        if index >= parts {
            break;
        }
        let outcome = part(index);
        let mut guard = results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard[index] = Some(outcome);
    };
    // Helpers run on a persistent pool (a thread per call cost more than a
    // small part's work); the caller runs its own share on its own thread.
    // Reads a helper does count toward the caller's per-thread read
    // accounting, so a query's tally does not depend on which thread ran
    // which part.
    let helper_reads = std::sync::Mutex::new(Vec::with_capacity(workers - 1));
    pool().in_place_scope(|scope| {
        for _ in 1..workers {
            scope.spawn(|_| {
                work();
                let reads = crate::read_accounting::take_local();
                helper_reads
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(reads);
            });
        }
        work();
    });
    let helper_reads = helper_reads
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for reads in helper_reads {
        crate::read_accounting::add_local(reads);
    }
    drop(lease);
    slots
        .into_iter()
        .map(|slot| slot.expect("every part ran"))
        .collect()
}

/// Split `0..len` into at most `parts` contiguous ranges of near-equal size,
/// in order (no empty ranges).
#[must_use]
pub fn split(len: u64, parts: usize) -> Vec<std::ops::Range<u64>> {
    let parts = u64::try_from(parts.max(1)).unwrap_or(1).min(len.max(1));
    let size = len.div_ceil(parts);
    (0..parts)
        .map(|i| (i * size)..((i + 1) * size).min(len))
        .filter(|range| !range.is_empty())
        .collect()
}

/// How many parts a job of `units` should split into: one per helper the
/// budget has plus the caller, and none smaller than `min_units`.
#[must_use]
pub fn parts_for(units: u64, min_units: u64) -> usize {
    let by_size = usize::try_from(units / min_units.max(1)).unwrap_or(usize::MAX);
    by_size.clamp(1, budget() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The budget is process-wide: tests that lend from it run one at a
    /// time.
    static BUDGET_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        BUDGET_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn parts_return_in_order_and_errors_by_lowest_part() {
        let _serial = exclusive();
        set_threads(4);
        let out: Result<Vec<usize>, ()> = run_parts(16, |i| Ok(i * 2));
        assert_eq!(out.unwrap(), (0..16).map(|i| i * 2).collect::<Vec<_>>());
        let failed: Result<Vec<usize>, usize> =
            run_parts(16, |i| if i % 5 == 3 { Err(i) } else { Ok(i) });
        assert_eq!(failed, Err(3));
        // Every lent thread came back.
        assert_eq!(FREE.load(Ordering::SeqCst), budget());
    }

    #[test]
    fn reads_on_helper_threads_count_for_the_caller() {
        let _serial = exclusive();
        set_threads(4);
        crate::read_accounting::enable(true);
        let _ = crate::read_accounting::take_local();
        let parts: Result<Vec<()>, ()> = run_parts(8, |part| {
            crate::read_accounting::record("get", &[0x01, 0, 0, 0, 1, u8::try_from(part).unwrap()]);
            Ok(())
        });
        parts.unwrap();
        let counted: u64 = crate::read_accounting::take_local().values().sum();
        assert_eq!(counted, 8, "every part's read is the caller's");
    }

    #[test]
    fn split_covers_the_range_without_gaps() {
        let ranges = split(10, 3);
        assert_eq!(ranges, vec![0..4, 4..8, 8..10]);
        assert_eq!(split(2, 8), vec![0..1, 1..2]);
        assert!(split(0, 4).is_empty());
    }
}
