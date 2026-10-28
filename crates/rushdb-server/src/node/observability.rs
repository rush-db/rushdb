//! Node observability operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use rushdb_types::ids::TenantId;

use super::Node;

impl Node {
    // ---- hybrid ----------------------------------------------------------

    // ---- operations surface ------------------------------------------

    /// Request counters snapshot consumed by the metrics endpoint.
    /// Record one request observation (route + latency) into the histogram.
    pub fn observe(&self, route: &str, seconds: f64) {
        self.metrics.observe(route, seconds);
    }

    /// Record rows returned by a query route (pure data-queried metric).
    pub fn observe_rows(&self, route: &str, rows: u64) {
        self.metrics.observe_rows(route, rows);
    }

    /// Record one maintenance-tick step's duration (`step` is a stable
    /// label: `read_pins`, `vector_segments`, `housekeeping`, `history_gc`,
    /// `retention`, `receipts`, `vector_slots`, `checkpoint`).
    /// Returns the next step's start.
    pub(crate) fn observe_maintenance_step(
        &self,
        step: &str,
        started: std::time::Instant,
    ) -> std::time::Instant {
        let now = std::time::Instant::now();
        self.metrics
            .observe_maintenance_step(step, (now - started).as_secs_f64());
        now
    }

    /// Record one maintenance tick's duration (histogram).
    pub(crate) fn observe_gc_tick(&self, seconds: f64) {
        self.metrics.observe_gc_tick(seconds);
    }

    /// Record the HTTP request-body bytes currently reserved in flight.
    pub(crate) fn observe_inflight_body_bytes(&self, bytes: usize) {
        self.metrics.observe_inflight_body_bytes(bytes);
    }

    /// Record bounded work performed by an operation (for example candidate
    /// rows, reconstructed rows, payload gets, or posting bytes).
    pub fn observe_work(&self, operation: &str, counter: &str, units: u64) {
        self.metrics.observe_work(operation, counter, units);
    }

    /// Record one boot-phase duration for startup attribution.
    pub fn observe_boot_phase(&self, phase: &str, seconds: f64) {
        self.metrics.observe_boot_phase(phase, seconds);
    }

    /// Render the Prometheus text exposition.
    /// Refresh the cached [`rushdb_storage::StorageStatistics`] if older than
    /// `max_age`. A no-op otherwise, so callers (the metrics scrape) never
    /// trigger a key-space scan more than once per interval.
    ///
    /// A poisoned stats mutex is recovered, not propagated.
    pub fn refresh_storage_stats(&self, max_age: std::time::Duration) {
        {
            let guard = self
                .storage_stats
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if guard.as_ref().is_some_and(|(at, _)| at.elapsed() < max_age) {
                return;
            }
        }
        // Compute WITHOUT holding the cache lock: the scan is O(keyspace) and
        // would otherwise block every concurrent `storage_stats()` reader
        // (i.e. a `/metrics` scrape) for its whole duration.
        if let Ok(stats) = self.store.storage_stats() {
            *self
                .storage_stats
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((std::time::Instant::now(), stats));
        }
    }

    /// Last computed durable byte accounting, if any.
    ///
    /// A poisoned stats mutex is recovered, not propagated.
    pub fn storage_stats(&self) -> Option<rushdb_storage::StorageStatistics> {
        self.storage_stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|(_, s)| *s)
    }

    /// Prometheus text exposition for `/metrics`, including the cached durable
    /// byte accounting.
    ///
    /// A poisoned retention-consumer registry lock is recovered.
    pub fn metrics_render(&self) -> String {
        // Serve the last durable byte accounting snapshot (refreshed off the
        // request path by the storage-stats worker). Never scan here: the scan
        // is O(keyspace) and would block the scrape for tens of seconds on a
        // multi-GB store.
        self.metrics.observe_state(
            self.storage_stats(),
            &self
                .retention_consumers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            &self.scheduler,
        );
        // Store-level gauges are cheap atomics (no keyspace scan): derived cache
        // bytes, sequencer in-flight keys, group commits, oldest live reader.
        self.metrics.observe_store(&self.store);

        let text_metric = self.text_pending_by_index();
        let records = self.records_by_tenant_cached();
        let per_tenant: Option<Vec<TenantId>> = self.metrics_per_tenant.map(|limit| {
            let mut tenants = self.residency.resident_tenants();
            tenants.truncate(limit);
            tenants
        });
        self.metrics
            .observe_residency(self.residency.len(), self.residency.resident_bytes());
        self.metrics.render(
            &self.store,
            &text_metric,
            &records,
            per_tenant.as_deref(),
            self.metrics_prefix.as_deref(),
            self.metrics_filter.as_deref(),
        )
    }

    /// Backlog of every text index, by index name: the base tenant's in
    /// single-tenant mode; in multitenant mode summed over the resident
    /// tenants (bounded work per scrape; an evicted tenant's backlog is
    /// reported again once a request makes it resident).
    fn text_pending_by_index(&self) -> std::collections::BTreeMap<String, u64> {
        let mut pending = std::collections::BTreeMap::new();
        let Ok(view) = self.store.read_view() else {
            return pending;
        };
        let tenants = if self.multitenant {
            self.residency.resident_tenants()
        } else {
            vec![self.base_tenant_id()]
        };
        for tenant in tenants {
            let Ok(definitions) = rushdb_lmpg::index::definitions(&view, tenant) else {
                continue;
            };
            for def in definitions {
                if !matches!(def.kind, rushdb_types::index::IndexKind::Text { .. }) {
                    continue;
                }
                let queued = rushdb_lmpg::index::pending(&view, tenant, def.id, 1_000_000)
                    .map_or(0, |queue| queue.len() as u64);
                *pending.entry(def.name).or_insert(0) += queued;
            }
        }
        pending
    }

    /// Last cached per-tenant record counts (empty until the background
    /// refresh runs).
    ///
    /// A poisoned cache mutex is recovered, not propagated.
    pub(super) fn records_by_tenant_cached(&self) -> std::collections::BTreeMap<u64, u64> {
        self.records_by_tenant_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map_or_else(std::collections::BTreeMap::default, |(_, map)| map.clone())
    }

    /// Refresh the cached per-tenant record counts if older than `max_age`.
    /// Computed without holding the cache lock, since it scans the keyspace.
    ///
    /// A poisoned cache mutex is recovered, not propagated.
    pub fn refresh_records_by_tenant(&self, max_age: std::time::Duration) {
        {
            let guard = self
                .records_by_tenant_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if guard.as_ref().is_some_and(|(at, _)| at.elapsed() < max_age) {
                return;
            }
        }
        let map = self.records_by_tenant();
        *self
            .records_by_tenant_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((std::time::Instant::now(), map));
    }

    /// Records per tenant ordinal, from the label counters (O(labels) per
    /// tenant at the current snapshot; no keyspace scan).
    pub(super) fn records_by_tenant(&self) -> std::collections::BTreeMap<u64, u64> {
        let mut out = std::collections::BTreeMap::new();
        let Ok(tenants) = self.durable_tenants() else {
            return out;
        };
        let Ok(view) = self.store.read_view() else {
            return out;
        };
        for tenant in tenants {
            match rushdb_lmpg::reader::try_access::label_counts(&view, tenant) {
                Ok(counts) => {
                    out.insert(
                        u64::from(tenant.0),
                        counts.iter().map(|(_, count)| count).sum(),
                    );
                }
                Err(error) => tracing::warn!(%error, %tenant, "cannot count tenant records"),
            }
        }
        out
    }
}
