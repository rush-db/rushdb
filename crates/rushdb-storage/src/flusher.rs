//! Paced checkpoints (PERF-2): a shard whose buffered materialization writes
//! pass its bound is checkpointed by this store's flusher thread right away,
//! instead of at the next maintenance tick. Each durable redb commit writes
//! out and syncs everything buffered since the previous one while it holds
//! the only writer, so keeping that amount bounded keeps every commit's
//! wait behind it bounded. The thread is started by the first due shard
//! (in-memory stores never start it) and joined when the store drops.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, PoisonError};

use crate::StorageBackend;

struct Request {
    backend: Arc<dyn StorageBackend>,
    durable_lsn: u64,
}

#[derive(Default)]
pub(crate) struct Flusher {
    queue: Mutex<Option<mpsc::Sender<Request>>>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    written: Arc<AtomicU64>,
}

impl Flusher {
    /// Queue a checkpoint of `backend` at `durable_lsn` (its newest applied
    /// commit). Never waits for it.
    pub(crate) fn request(&self, backend: Arc<dyn StorageBackend>, durable_lsn: u64) {
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        let request = Request {
            backend,
            durable_lsn,
        };
        let request = match queue.as_ref() {
            Some(sender) => match sender.send(request) {
                Ok(()) => return,
                Err(mpsc::SendError(request)) => request,
            },
            None => request,
        };
        // First request, or the thread is gone (it never exits while the
        // sender lives, so only after a panic): start one.
        let (sender, receiver) = mpsc::channel::<Request>();
        let written = Arc::clone(&self.written);
        let spawned = std::thread::Builder::new()
            .name("rushdb-flusher".into())
            .spawn(move || {
                for request in receiver {
                    match request.backend.checkpoint(request.durable_lsn) {
                        Ok(true) => {
                            written.fetch_add(1, Ordering::Relaxed);
                        }
                        Ok(false) => {}
                        Err(error) => {
                            // The maintenance tick's checkpoint retries it.
                            tracing::warn!(%error, "paced checkpoint failed");
                        }
                    }
                }
            });
        match spawned {
            Ok(handle) => {
                let _ = sender.send(request);
                *queue = Some(sender);
                let previous = self
                    .worker
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .replace(handle);
                if let Some(previous) = previous {
                    let _ = previous.join();
                }
            }
            Err(error) => {
                tracing::warn!(%error, "paced checkpoint thread did not start");
            }
        }
    }

    /// Paced checkpoints that wrote, cumulative.
    pub(crate) fn written(&self) -> u64 {
        self.written.load(Ordering::Relaxed)
    }

    /// Stop taking requests and wait for the queued ones. Called before the
    /// store's final checkpoint and on drop, so no flusher still holds a
    /// backend when the store closes.
    pub(crate) fn stop(&self) {
        drop(
            self.queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
    }
}

impl Drop for Flusher {
    fn drop(&mut self) {
        self.stop();
    }
}
