//! Bounded blocking work scheduler for embedded engine callers.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use rushdb_types::ids::TenantId;

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Scheduler admission failures are explicit instead of silently growing an
/// unbounded blocking-work queue.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SchedulerError {
    /// The bounded queue has no free slot.
    #[error("engine scheduler queue is full")]
    QueueFull,
    /// All workers have exited or the scheduler is shutting down.
    #[error("engine scheduler is unavailable")]
    Unavailable,
    /// The admitted job panicked. The panic was contained (the worker was kept
    /// alive) and reported as this typed failure instead of a generic
    /// availability error.
    #[error("engine scheduler job panicked")]
    Panicked,
}

/// Small bounded worker pool used by the embedded engine facade.
pub struct BoundedScheduler {
    sender: Option<SyncSender<Job>>,
    workers: Vec<JoinHandle<()>>,
    tenant_usage: Arc<Mutex<HashMap<TenantId, usize>>>,
    max_per_tenant: usize,
    inflight: Arc<AtomicUsize>,
    panics: Arc<AtomicUsize>,
    queue_capacity: usize,
}

impl BoundedScheduler {
    /// Create a scheduler with `workers` threads and `queue_capacity` pending
    /// jobs. Zero values are rejected so the scheduler cannot accept work it
    /// has no ability to service.
    ///
    /// # Errors
    /// Returns [`SchedulerError::Unavailable`] when either capacity is zero.
    pub fn new(workers: usize, queue_capacity: usize) -> Result<Self, SchedulerError> {
        Self::new_with_tenant_limit(workers, queue_capacity, queue_capacity)
    }

    /// Create a scheduler with an explicit per-tenant queued/in-flight cap.
    /// The cap is checked before the global queue, so one tenant cannot
    /// consume every pending slot while other tenants are admitted.
    ///
    /// # Errors
    /// Returns [`SchedulerError::Unavailable`] when any capacity is zero.
    pub fn new_with_tenant_limit(
        workers: usize,
        queue_capacity: usize,
        max_per_tenant: usize,
    ) -> Result<Self, SchedulerError> {
        if workers == 0 || queue_capacity == 0 || max_per_tenant == 0 {
            return Err(SchedulerError::Unavailable);
        }
        let (sender, receiver) = mpsc::sync_channel::<Job>(queue_capacity);
        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));
        let panics = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let receiver = std::sync::Arc::clone(&receiver);
            let panics = Arc::clone(&panics);
            handles.push(thread::spawn(move || loop {
                let job = receiver.lock().ok().and_then(|guard| guard.recv().ok());
                let Some(job) = job else { break };
                // A request is untrusted work from the scheduler's point of
                // view. Keep a panic in one job from silently reducing the
                // worker pool and turning a transient bug into an outage.
                if catch_unwind(AssertUnwindSafe(job)).is_err() {
                    panics.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        Ok(Self {
            sender: Some(sender),
            workers: handles,
            tenant_usage: Arc::new(Mutex::new(HashMap::new())),
            max_per_tenant,
            inflight: Arc::new(AtomicUsize::new(0)),
            panics,
            queue_capacity,
        })
    }

    /// Execute a job on a bounded worker and wait for its result. Admission
    /// is nonblocking: callers receive `QueueFull` instead of extending an
    /// unbounded queue under load.
    ///
    /// # Errors
    /// Returns [`SchedulerError::QueueFull`] when no queue slot is available,
    /// or [`SchedulerError::Unavailable`] after worker shutdown.
    pub fn execute<R: Send + 'static>(
        &self,
        job: impl FnOnce() -> R + Send + 'static,
    ) -> Result<R, SchedulerError> {
        let sender = self.sender.as_ref().ok_or(SchedulerError::Unavailable)?;
        self.inflight.fetch_add(1, Ordering::Relaxed);
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        let panics = Arc::clone(&self.panics);
        let task: Job = Box::new(move || {
            // Catch at the task boundary so a panicking job reports a typed
            // `Panicked` error to its caller; the worker keeps running.
            if let Ok(value) = catch_unwind(AssertUnwindSafe(job)) {
                let _ = result_tx.send(Ok(value));
            } else {
                panics.fetch_add(1, Ordering::Relaxed);
                let _ = result_tx.send(Err(SchedulerError::Panicked));
            }
        });
        let result = match sender.try_send(task) {
            Ok(()) => result_rx
                .recv()
                .map_err(|_| SchedulerError::Unavailable)
                .and_then(|result| result),
            Err(TrySendError::Full(_)) => Err(SchedulerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SchedulerError::Unavailable),
        };
        self.inflight.fetch_sub(1, Ordering::Relaxed);
        result
    }

    /// Execute work with a per-tenant queued/in-flight admission limit.
    ///
    /// # Errors
    /// Returns [`SchedulerError::QueueFull`] when either the tenant or global
    /// queue is at capacity, or [`SchedulerError::Unavailable`] after
    /// shutdown.
    pub fn execute_for_tenant<R: Send + 'static>(
        &self,
        tenant: TenantId,
        job: impl FnOnce() -> R + Send + 'static,
    ) -> Result<R, SchedulerError> {
        let sender = self.sender.as_ref().ok_or(SchedulerError::Unavailable)?;
        {
            let mut usage = self
                .tenant_usage
                .lock()
                .map_err(|_| SchedulerError::Unavailable)?;
            let current = usage.get(&tenant).copied().unwrap_or(0);
            if current >= self.max_per_tenant {
                return Err(SchedulerError::QueueFull);
            }
            usage.insert(tenant, current + 1);
        }
        self.inflight.fetch_add(1, Ordering::Relaxed);
        let usage = Arc::clone(&self.tenant_usage);
        let panics = Arc::clone(&self.panics);
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        let task: Job = Box::new(move || {
            if let Ok(value) = catch_unwind(AssertUnwindSafe(job)) {
                let _ = result_tx.send(Ok(value));
            } else {
                panics.fetch_add(1, Ordering::Relaxed);
                let _ = result_tx.send(Err(SchedulerError::Panicked));
            }
        });
        let result = match sender.try_send(task) {
            Ok(()) => result_rx
                .recv()
                .map_err(|_| SchedulerError::Unavailable)
                .and_then(|result| result),
            Err(TrySendError::Full(_)) => Err(SchedulerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SchedulerError::Unavailable),
        };
        if let Ok(mut counts) = usage.lock() {
            if let Some(count) = counts.get_mut(&tenant) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    counts.remove(&tenant);
                }
            }
        }
        self.inflight.fetch_sub(1, Ordering::Relaxed);
        result
    }

    /// Number of jobs currently admitted, including queued and running work.
    #[must_use]
    pub fn inflight(&self) -> usize {
        self.inflight.load(Ordering::Relaxed)
    }

    /// Number of jobs whose panic was contained by a worker.
    #[must_use]
    pub fn panics(&self) -> usize {
        self.panics.load(Ordering::Relaxed)
    }

    /// Maximum number of jobs the global queue can admit at once.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.queue_capacity
    }
}

/// Join `workers` under one deadline. Returns the number of workers that did
/// not finish in time and were detached. A worker blocked in an uncooperative
/// job cannot be killed; detaching keeps process shutdown bounded.
fn join_bounded(workers: Vec<JoinHandle<()>>, timeout: std::time::Duration) -> usize {
    let deadline = std::time::Instant::now() + timeout;
    let mut detached = 0;
    for worker in workers {
        while !worker.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if worker.is_finished() {
            let _ = worker.join();
        } else {
            detached += 1;
        }
    }
    detached
}

impl Drop for BoundedScheduler {
    fn drop(&mut self) {
        // Drop the sender so idle workers exit once the queue drains, then join
        // under a single deadline.
        self.sender.take();
        let workers = std::mem::take(&mut self.workers);
        let detached = join_bounded(workers, std::time::Duration::from_secs(5));
        if detached > 0 {
            eprintln!(
                "rushdb scheduler: {detached} worker(s) did not stop within the shutdown deadline; detached"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn scheduler_executes_jobs_and_shuts_down_workers() {
        let scheduler = BoundedScheduler::new(2, 2).unwrap();
        assert_eq!(scheduler.execute(|| 21 + 21).unwrap(), 42);
        let completed = Arc::new(AtomicUsize::new(0));
        let completed_for_job = Arc::clone(&completed);
        scheduler
            .execute(move || completed_for_job.fetch_add(1, Ordering::Relaxed))
            .unwrap();
        assert_eq!(completed.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn zero_capacity_is_rejected() {
        assert!(matches!(
            BoundedScheduler::new(1, 0),
            Err(SchedulerError::Unavailable)
        ));
    }

    #[test]
    fn per_tenant_cap_rejects_work_before_global_queue_is_full() {
        let scheduler = Arc::new(BoundedScheduler::new_with_tenant_limit(1, 4, 1).unwrap());
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let worker = Arc::clone(&scheduler);
        let handle = thread::spawn(move || {
            worker.execute_for_tenant(TenantId::single(), move || {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                7
            })
        });
        started_rx.recv().unwrap();
        assert_eq!(
            scheduler.execute_for_tenant(TenantId::single(), || 8),
            Err(SchedulerError::QueueFull)
        );
        release_tx.send(()).unwrap();
        assert_eq!(handle.join().unwrap().unwrap(), 7);
    }

    #[test]
    fn bounded_join_detaches_a_blocked_worker_without_hanging() {
        let (release_tx, release_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let _ = release_rx.recv();
        });
        let started = std::time::Instant::now();
        let detached = join_bounded(vec![handle], std::time::Duration::from_millis(50));
        assert_eq!(detached, 1, "blocked worker must be detached");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "join must respect the deadline"
        );
        release_tx.send(()).unwrap();
    }

    #[test]
    fn panic_in_one_job_does_not_remove_worker_capacity() {
        let scheduler = BoundedScheduler::new(1, 2).unwrap();
        // A panicking job is reported as a typed failure, not as the worker
        // becoming unavailable.
        assert_eq!(
            scheduler.execute(|| panic!("injected scheduler panic")),
            Err(SchedulerError::Panicked)
        );
        assert_eq!(scheduler.execute(|| 42), Ok(42));
        assert_eq!(scheduler.panics(), 1);
    }

    #[test]
    fn panic_in_tenant_job_is_typed_and_releases_the_tenant_slot() {
        let scheduler = BoundedScheduler::new_with_tenant_limit(1, 2, 1).unwrap();
        assert_eq!(
            scheduler.execute_for_tenant(TenantId::single(), || panic!("injected")),
            Err(SchedulerError::Panicked)
        );
        // The tenant admission slot was released, so the next job is admitted.
        assert_eq!(
            scheduler.execute_for_tenant(TenantId::single(), || 7),
            Ok(7)
        );
        assert_eq!(scheduler.panics(), 1);
    }
}
