//! FIFO ticket gate placed in front of a contended mutex.
//!
//! `std::sync::Mutex` does not hand off ownership in arrival order. Under a
//! steady stream of writers, a thread parked on the admission lock can lose
//! every race to threads that keep re-acquiring it. Measured: the first bulk
//! ingest batch of a run waited 15 s while 99 later batches each completed
//! in ≤ 1.7 s. Taking a ticket before the mutex makes admission
//! first-come-first-served without changing the lock itself.
//!
//! Re-measured after the publisher-convoy fix (A/B, 2026-10-08, 200K
//! records, 8 ingest writers, two runs each): with the gate the slowest batch
//! took 1.6 s / 6.0 s; without it 11.4 s / 12.6 s (ingest 3.75K vs 4.3–5.0K
//! rec/s). The starvation is real independent of the convoy, so the gate
//! stays (plan SD-0.1).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};

/// Serves tickets in the order they were taken.
#[derive(Debug, Default)]
pub(crate) struct FairGate {
    next: AtomicU64,
    serving: Mutex<u64>,
    turn: Condvar,
}

/// Holding a [`Turn`] means every earlier ticket holder has passed the gate;
/// dropping it admits the next ticket.
pub(crate) struct Turn<'a>(&'a FairGate);

impl FairGate {
    /// Wait for this caller's turn.
    pub(crate) fn enter(&self) -> Turn<'_> {
        let ticket = self.next.fetch_add(1, Ordering::AcqRel);
        let mut serving = self.serving.lock().unwrap_or_else(PoisonError::into_inner);
        while *serving != ticket {
            serving = self
                .turn
                .wait(serving)
                .unwrap_or_else(PoisonError::into_inner);
        }
        Turn(self)
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        *self
            .0
            .serving
            .lock()
            .unwrap_or_else(PoisonError::into_inner) += 1;
        self.0.turn.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn turns_are_served_in_ticket_order() {
        let gate = Arc::new(FairGate::default());
        let order = Arc::new(Mutex::new(Vec::new()));
        // Hold the first turn so every other ticket queues behind it.
        let first = gate.enter();
        let mut workers = Vec::new();
        for id in 0..8u64 {
            let (worker_gate, worker_order) = (Arc::clone(&gate), Arc::clone(&order));
            workers.push(std::thread::spawn(move || {
                let _turn = worker_gate.enter();
                worker_order.lock().unwrap().push(id);
            }));
            // Take tickets in a known order.
            while gate.next.load(Ordering::Acquire) != id + 2 {
                std::thread::yield_now();
            }
        }
        drop(first);
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), (0..8).collect::<Vec<_>>());
    }
}
