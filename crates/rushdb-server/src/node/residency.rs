//! Tenant residency on demand: one registry owns the per-tenant
//! in-memory handles (vector engine, transaction coordinator). Idle tenants
//! cost about nothing:
//!
//! - an entry is created when a request first needs it;
//! - an entry nobody holds, with no open transaction, idle longer than
//!   `tenant-idle-seconds`, is evicted (and rebuilt on the next request —
//!   all of its state is durable);
//! - over `tenant-resident-bytes`, least-recently-used idle entries go
//!   first;
//! - when every resident entry is active, admitting a new tenant fails with
//!   a retryable `tenant_capacity` error (HTTP 503): a load signal, not a
//!   fixed tenant count.
//!
//! Text indexes, planner statistics and the index catalog are store-wide
//! and are bounded there, not here.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rushdb_types::ids::TenantId;

/// Fixed accounting charge per resident tenant (registry entry, coordinator
/// and engine shells), on top of what the handles report.
pub(crate) const ENTRY_OVERHEAD_BYTES: usize = 16 * 1024;

/// Monotonic clock, injectable for tests.
pub(crate) type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

#[derive(Default)]
struct Resident {
    vector: Option<Arc<rushdb_vector::VectorEngine>>,
    tx: Option<Arc<rushdb_tx::TxCoordinator>>,
    last_used: Option<Instant>,
}

impl Resident {
    fn bytes(&self) -> usize {
        ENTRY_OVERHEAD_BYTES
            + self
                .vector
                .as_ref()
                .map_or(0, |engine| engine.resident_bytes())
    }

    /// No request holds a handle and no transaction is open.
    fn idle(&self) -> bool {
        self.vector
            .as_ref()
            .is_none_or(|engine| Arc::strong_count(engine) == 1)
            && self.tx.as_ref().is_none_or(|coordinator| {
                Arc::strong_count(coordinator) == 1 && coordinator.active_count() == 0
            })
    }
}

/// Error when every resident tenant is active and the budget is spent.
pub(crate) fn capacity_error(budget: usize) -> String {
    format!(
        "tenant_capacity: every resident tenant is active and the {budget}-byte residency budget is spent; retry later"
    )
}

/// The node's tenant residency registry.
pub(crate) struct TenantResidency {
    entries: Mutex<HashMap<TenantId, Resident>>,
    idle: Duration,
    budget: usize,
    clock: Clock,
}

impl TenantResidency {
    pub(crate) fn new(idle: Duration, budget: usize, clock: Clock) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            idle,
            budget,
            clock,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<TenantId, Resident>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Bytes charged by every resident entry.
    pub(crate) fn resident_bytes(&self) -> usize {
        self.lock().values().map(Resident::bytes).sum()
    }

    /// Resident tenants, most recently used first.
    /// Every resident vector engine (for background segment maintenance).
    pub(crate) fn resident_vector_engines(&self) -> Vec<Arc<rushdb_vector::VectorEngine>> {
        self.lock()
            .values()
            .filter_map(|resident| resident.vector.clone())
            .collect()
    }

    pub(crate) fn resident_tenants(&self) -> Vec<TenantId> {
        let entries = self.lock();
        let mut tenants: Vec<(TenantId, Option<Instant>)> = entries
            .iter()
            .map(|(tenant, resident)| (*tenant, resident.last_used))
            .collect();
        tenants.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        tenants.into_iter().map(|(tenant, _)| tenant).collect()
    }

    /// Make room for `tenant`'s entry: evict idle entries least recently
    /// used first until the budget holds one more entry, or fail when only
    /// active entries remain.
    fn admit(
        &self,
        entries: &mut HashMap<TenantId, Resident>,
        tenant: TenantId,
    ) -> Result<(), String> {
        if entries.contains_key(&tenant) {
            return Ok(());
        }
        loop {
            let used: usize = entries.values().map(Resident::bytes).sum();
            if used.saturating_add(ENTRY_OVERHEAD_BYTES) <= self.budget {
                return Ok(());
            }
            let victim = entries
                .iter()
                .filter(|(_, resident)| resident.idle())
                .min_by_key(|(_, resident)| resident.last_used)
                .map(|(tenant, _)| *tenant);
            match victim {
                Some(victim) => {
                    entries.remove(&victim);
                }
                None => return Err(capacity_error(self.budget)),
            }
        }
    }

    /// Record that `tenant` is being served. A handle-less entry is always
    /// idle, so a touch never refuses admission; over budget it evicts idle
    /// entries least recently used first, and is itself skipped when only
    /// active entries remain.
    pub(crate) fn touch(&self, tenant: TenantId) {
        let now = (self.clock)();
        let mut entries = self.lock();
        if !entries.contains_key(&tenant) && self.admit(&mut entries, tenant).is_err() {
            return;
        }
        entries.entry(tenant).or_default().last_used = Some(now);
    }

    /// The tenant's vector engine, created by `make` on first use.
    pub(crate) fn vector(
        &self,
        tenant: TenantId,
        make: impl FnOnce() -> rushdb_vector::VectorEngine,
    ) -> Result<Arc<rushdb_vector::VectorEngine>, String> {
        let now = (self.clock)();
        let mut entries = self.lock();
        self.admit(&mut entries, tenant)?;
        let resident = entries.entry(tenant).or_default();
        resident.last_used = Some(now);
        Ok(Arc::clone(
            resident.vector.get_or_insert_with(|| Arc::new(make())),
        ))
    }

    /// The tenant's transaction coordinator, created by `make` on first use.
    pub(crate) fn tx(
        &self,
        tenant: TenantId,
        make: impl FnOnce() -> Result<rushdb_tx::TxCoordinator, String>,
    ) -> Result<Arc<rushdb_tx::TxCoordinator>, String> {
        let now = (self.clock)();
        let mut entries = self.lock();
        self.admit(&mut entries, tenant)?;
        let resident = entries.entry(tenant).or_default();
        resident.last_used = Some(now);
        if let Some(coordinator) = &resident.tx {
            return Ok(Arc::clone(coordinator));
        }
        let coordinator = Arc::new(make()?);
        resident.tx = Some(Arc::clone(&coordinator));
        Ok(coordinator)
    }

    /// The tenant's coordinator if resident (never creates one).
    pub(crate) fn existing_tx(&self, tenant: TenantId) -> Option<Arc<rushdb_tx::TxCoordinator>> {
        self.lock()
            .get(&tenant)
            .and_then(|resident| resident.tx.clone())
    }

    /// Every resident coordinator (for the expiry sweep).
    pub(crate) fn coordinators(&self) -> Vec<Arc<rushdb_tx::TxCoordinator>> {
        self.lock()
            .values()
            .filter_map(|resident| resident.tx.clone())
            .collect()
    }

    /// Evict idle entries unused for longer than the idle TTL, then idle
    /// entries least recently used first while over budget. Returns the
    /// evicted tenants.
    pub(crate) fn evict_idle(&self) -> Vec<TenantId> {
        let now = (self.clock)();
        let mut entries = self.lock();
        let mut evicted: Vec<TenantId> = entries
            .iter()
            .filter(|(_, resident)| {
                resident.idle()
                    && resident
                        .last_used
                        .is_none_or(|used| now.saturating_duration_since(used) >= self.idle)
            })
            .map(|(tenant, _)| *tenant)
            .collect();
        for tenant in &evicted {
            entries.remove(tenant);
        }
        while entries.values().map(Resident::bytes).sum::<usize>() > self.budget {
            let Some(victim) = entries
                .iter()
                .filter(|(_, resident)| resident.idle())
                .min_by_key(|(_, resident)| resident.last_used)
                .map(|(tenant, _)| *tenant)
            else {
                break;
            };
            entries.remove(&victim);
            evicted.push(victim);
        }
        evicted
    }

    /// Forget a purged tenant (handles held by in-flight requests stay
    /// valid until dropped).
    pub(crate) fn remove(&self, tenant: TenantId) {
        self.lock().remove(&tenant);
    }

    /// Number of resident tenants.
    pub(crate) fn len(&self) -> usize {
        self.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn clock() -> (Clock, Arc<AtomicU64>) {
        let base = Instant::now();
        let offset = Arc::new(AtomicU64::new(0));
        let read = Arc::clone(&offset);
        (
            Arc::new(move || base + Duration::from_secs(read.load(Ordering::Relaxed))),
            offset,
        )
    }

    fn engine(store: &Arc<rushdb_storage::Store>, tenant: TenantId) -> rushdb_vector::VectorEngine {
        rushdb_vector::VectorEngine::new(Arc::new(rushdb_lmpg::Lmpg::new(
            Arc::clone(store),
            tenant,
        )))
    }

    #[test]
    fn idle_tenant_is_evicted_and_transparently_reloaded() {
        let store = Arc::new(rushdb_storage::Store::new_in_memory());
        let (clock, now) = clock();
        let residency = TenantResidency::new(Duration::from_secs(300), 1 << 30, clock);
        let tenant = TenantId::from_ordinal(9);
        let first = residency.vector(tenant, || engine(&store, tenant)).unwrap();
        now.store(1_000, Ordering::Relaxed);
        assert!(
            residency.evict_idle().is_empty(),
            "a held handle is never evicted"
        );
        drop(first);
        now.store(1_299, Ordering::Relaxed);
        residency.vector(tenant, || engine(&store, tenant)).unwrap();
        now.store(1_299 + 299, Ordering::Relaxed);
        assert!(
            residency.evict_idle().is_empty(),
            "used 299 s ago: still resident"
        );
        now.store(1_299 + 300, Ordering::Relaxed);
        assert_eq!(residency.evict_idle(), vec![tenant]);
        assert_eq!((residency.len(), residency.resident_bytes()), (0, 0));
        // The next request rebuilds the handle from durable state.
        assert!(residency.vector(tenant, || engine(&store, tenant)).is_ok());
        assert_eq!(residency.len(), 1);
    }

    #[test]
    fn ten_thousand_tenants_round_robin_stay_within_budget() {
        let store = Arc::new(rushdb_storage::Store::new_in_memory());
        let (clock, now) = clock();
        let budget = 1_000 * ENTRY_OVERHEAD_BYTES;
        let residency = TenantResidency::new(Duration::from_secs(300), budget, clock);
        for round in 0..2u64 {
            for ordinal in 0..10_000u64 {
                now.store(round * 10_000 + ordinal, Ordering::Relaxed);
                let tenant = TenantId::from_ordinal(ordinal + 2);
                let handle = residency.vector(tenant, || engine(&store, tenant)).unwrap();
                drop(handle);
                assert!(residency.resident_bytes() <= budget);
            }
        }
        assert_eq!(
            residency.len(),
            1_000,
            "LRU keeps the budget's worth resident"
        );
        // The most recently used tenants are the resident ones.
        assert_eq!(
            residency.resident_tenants()[0],
            TenantId::from_ordinal(10_001)
        );
    }

    #[test]
    fn active_tenants_exhausting_the_budget_are_refused_until_one_goes_idle() {
        let store = Arc::new(rushdb_storage::Store::new_in_memory());
        let (clock, _) = clock();
        let residency =
            TenantResidency::new(Duration::from_secs(300), 2 * ENTRY_OVERHEAD_BYTES, clock);
        let (a, b, c) = (
            TenantId::from_ordinal(2),
            TenantId::from_ordinal(3),
            TenantId::from_ordinal(4),
        );
        let held_a = residency.vector(a, || engine(&store, a)).unwrap();
        let held_b = residency.vector(b, || engine(&store, b)).unwrap();
        let Err(error) = residency.vector(c, || engine(&store, c)) else {
            panic!("all resident tenants are active")
        };
        assert!(error.contains("tenant_capacity"));
        drop(held_a);
        assert!(residency.vector(c, || engine(&store, c)).is_ok());
        assert_eq!(residency.resident_tenants().len(), 2);
        drop(held_b);
    }

    #[test]
    fn a_coordinator_with_an_open_transaction_is_not_evicted() {
        let store = Arc::new(rushdb_storage::Store::new_in_memory());
        let (clock, now) = clock();
        let residency = TenantResidency::new(Duration::from_secs(1), 1 << 30, clock);
        let tenant = TenantId::from_ordinal(5);
        drop(rushdb_lmpg::Lmpg::new(Arc::clone(&store), tenant));
        let coordinator = residency
            .tx(tenant, || {
                rushdb_tx::TxCoordinator::new(Arc::clone(&store), tenant, Duration::from_secs(60))
            })
            .unwrap();
        let tx = coordinator.begin().unwrap();
        drop(coordinator);
        now.store(10, Ordering::Relaxed);
        assert!(
            residency.evict_idle().is_empty(),
            "its transaction state is in memory"
        );
        residency
            .existing_tx(tenant)
            .unwrap()
            .rollback(&tx)
            .unwrap();
        assert_eq!(residency.evict_idle(), vec![tenant]);
    }
}
