//! Tenant-fair ticket gate placed in front of the admission mutex.
//!
//! `std::sync::Mutex` does not hand off ownership in arrival order. Under a
//! steady stream of writers, a thread parked on the admission lock can lose
//! every race to threads that keep re-acquiring it. Measured: the first bulk
//! ingest batch of a run waited 15 s while 99 later batches each completed
//! in ≤ 1.7 s. Taking a ticket before the mutex fixes the order without
//! changing the lock itself (plan SD-0.1).
//!
//! Tickets wait in one lane per tenant (and one for store-wide commits). A
//! turn goes to the next lane, round robin, that has a waiter: each lane is
//! served first come, first served, and no lane waits behind more than one
//! turn of every other lane. A light tenant's commit therefore gets the next
//! commit index after at most one bulk commit per busy tenant instead of
//! after the whole bulk queue (SCALE-2 F1). Commit indices are assigned
//! inside the admitted section, so the publisher still sees one contiguous,
//! ascending run of them, and a tenant's commits keep their order.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

/// The queue a commit waits in: a tenant's ordinal, or `None` for a commit
/// with no tenant (store-wide metadata, replicated apply).
pub(crate) type Lane = Option<u64>;

/// The lane of `tenant`'s commits.
pub(crate) fn tenant_lane(tenant: rushdb_types::ids::TenantId) -> u64 {
    u64::from(tenant.0)
}

#[derive(Debug, Default)]
struct State {
    next_ticket: u64,
    lanes: BTreeMap<Lane, VecDeque<u64>>,
    /// The ticket whose turn it is; `None` when the gate is free.
    serving: Option<u64>,
    /// The lane of the last ticket served: the round robin resumes after it.
    last_lane: Option<Lane>,
}

impl State {
    /// Give the turn to the head of the next non-empty lane after the last
    /// one served, wrapping around.
    fn advance(&mut self) {
        let after_last = self.last_lane.and_then(|last| {
            self.lanes
                .range((std::ops::Bound::Excluded(last), std::ops::Bound::Unbounded))
                .find(|(_, queue)| !queue.is_empty())
                .map(|(lane, _)| *lane)
        });
        let next = after_last.or_else(|| {
            self.lanes
                .iter()
                .find(|(_, queue)| !queue.is_empty())
                .map(|(lane, _)| *lane)
        });
        self.serving = next.and_then(|lane| {
            self.last_lane = Some(lane);
            let queue = self.lanes.get_mut(&lane)?;
            let ticket = queue.pop_front();
            if queue.is_empty() {
                self.lanes.remove(&lane);
            }
            ticket
        });
    }
}

/// Serves tickets round robin across lanes, in order within a lane.
#[derive(Debug, Default)]
pub(crate) struct FairGate {
    state: Mutex<State>,
    turn: Condvar,
}

/// Holding a [`Turn`] means it is this caller's turn; dropping it admits the
/// next one.
pub(crate) struct Turn<'a>(&'a FairGate);

impl FairGate {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wait for this caller's turn in `lane`.
    pub(crate) fn enter(&self, lane: Lane) -> Turn<'_> {
        self.enter_until(lane, None)
            .unwrap_or_else(|| unreachable!("an untimed wait always gets its turn"))
    }

    /// Wait for this caller's turn in `lane` until `deadline`; `None` when it
    /// passes first (the ticket is withdrawn).
    pub(crate) fn enter_until(&self, lane: Lane, deadline: Option<Instant>) -> Option<Turn<'_>> {
        let mut state = self.lock();
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        state.lanes.entry(lane).or_default().push_back(ticket);
        if state.serving.is_none() {
            state.advance();
        }
        while state.serving != Some(ticket) {
            let Some(deadline) = deadline else {
                state = self
                    .turn
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            };
            let now = Instant::now();
            if now >= deadline {
                if let Some(queue) = state.lanes.get_mut(&lane) {
                    queue.retain(|waiting| *waiting != ticket);
                    if queue.is_empty() {
                        state.lanes.remove(&lane);
                    }
                }
                return None;
            }
            state = self
                .turn
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        Some(Turn(self))
    }

    /// Whether a caller holds the turn (it may still be waiting on the lock
    /// the gate guards).
    #[cfg(test)]
    pub(crate) fn turn_held(&self) -> bool {
        self.lock().serving.is_some()
    }

    #[cfg(test)]
    pub(crate) fn waiting(&self) -> usize {
        self.lock().lanes.values().map(VecDeque::len).sum()
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.0.lock().advance();
        self.0.turn.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    /// Run one waiter per entry of `lanes` (tickets taken in that order)
    /// through the gate while a first turn is held; return the service order.
    fn served(lanes: &[Lane]) -> Vec<usize> {
        let gate = Arc::new(FairGate::default());
        let order = Arc::new(Mutex::new(Vec::new()));
        let first = gate.enter(Some(u64::MAX));
        let mut workers = Vec::new();
        for (id, lane) in lanes.iter().copied().enumerate() {
            let (worker_gate, worker_order) = (Arc::clone(&gate), Arc::clone(&order));
            workers.push(std::thread::spawn(move || {
                let _turn = worker_gate.enter(lane);
                worker_order.lock().unwrap().push(id);
            }));
            while gate.waiting() != id + 1 {
                std::thread::yield_now();
            }
        }
        drop(first);
        for worker in workers {
            worker.join().unwrap();
        }
        let order = order.lock().unwrap().clone();
        order
    }

    #[test]
    fn one_lane_is_served_in_ticket_order() {
        assert_eq!(served(&[Some(1); 8]), (0..8).collect::<Vec<_>>());
    }

    #[test]
    fn a_late_tenant_is_served_after_one_turn_of_a_busy_one() {
        // Tenant 1 queues five commits, then tenant 2 one: tenant 2 goes
        // second, not sixth, and tenant 1 keeps its own order.
        let order = served(&[Some(1), Some(1), Some(1), Some(1), Some(1), Some(2)]);
        assert_eq!(order, vec![0, 5, 1, 2, 3, 4]);
    }

    #[test]
    fn lanes_take_turns_in_order_and_store_wide_commits_have_their_own() {
        // The held first turn was lane u64::MAX, so the round robin wraps to
        // the lowest lane: `None` (store-wide), then tenants 1, 2, 3, ...
        let order = served(&[Some(1), Some(1), Some(2), Some(2), None, Some(3)]);
        assert_eq!(order, vec![4, 0, 2, 5, 1, 3]);
    }

    #[test]
    fn a_timed_wait_withdraws_its_ticket_and_the_gate_moves_on() {
        let gate = FairGate::default();
        let held = gate.enter(Some(1));
        let deadline = Instant::now() + Duration::from_millis(20);
        assert!(gate.enter_until(Some(2), Some(deadline)).is_none());
        assert_eq!(gate.waiting(), 0, "the withdrawn ticket is not left queued");
        drop(held);
        assert!(gate.enter_until(Some(3), Some(Instant::now())).is_some());
    }
}
