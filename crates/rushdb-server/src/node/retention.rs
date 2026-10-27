//! Retention bookkeeping: the consumer registry, the protected history floor,
//! embedding-slot reclamation, and durable tenant enumeration. The
//! maintenance tick enumerates tenants once and passes the list to every
//! consumer (C2, cloud-capacity F2).

use std::sync::Arc;

use rushdb_types::ids::TenantId;

use super::Node;

impl Node {
    /// Register or advance a consumer's history acknowledgement floor.
    ///
    /// # Errors
    /// [`ConsumerResyncRequired`](crate::retention::ConsumerResyncRequired)
    /// when retention already reclaimed `floor` (the consumer is lost and
    /// must rebuild from a fresh base).
    ///
    /// A poisoned retention registry lock is recovered, not propagated.
    pub fn acknowledge_retention_consumer(
        &self,
        consumer: impl Into<String>,
        floor: u64,
    ) -> Result<(), crate::retention::ConsumerResyncRequired> {
        let (acknowledged, floors) = {
            let mut consumers = self
                .retention_consumers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let acknowledged = consumers.acknowledge(consumer, floor, crate::retention::unix_ms());
            (acknowledged, consumers.snapshot())
        };
        if let Err(error) = self.persist_retention_consumers(&floors) {
            tracing::warn!(%error, "failed to persist retention consumer acknowledgement");
        }
        acknowledged
    }

    /// Whether retention passed `consumer` and it has not resynced.
    ///
    /// A poisoned retention registry lock is recovered, not propagated.
    #[cfg(test)]
    pub(crate) fn retention_consumer_lost(&self, consumer: &str) -> bool {
        self.retention_consumers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_lost(consumer)
    }

    /// Retention reclaimed everything below `floor` (hard cap): drop
    /// every consumer it passed, logging each, and persist the registry.
    ///
    /// A poisoned retention registry lock is recovered, not propagated.
    pub(crate) fn enforce_retention_floor(&self, floor: u64) {
        let (lost, floors) = {
            let mut consumers = self
                .retention_consumers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (consumers.enforce_floor(floor), consumers.snapshot())
        };
        if lost.is_empty() {
            return;
        }
        for consumer in &lost {
            tracing::warn!(
                consumer = consumer.as_str(),
                oldest_available_lsn = floor,
                "retention cap passed a consumer; it must resync"
            );
        }
        if let Err(error) = self.persist_retention_consumers(&floors) {
            tracing::warn!(%error, "failed to persist lost retention consumers");
        }
    }

    /// Release a consumer so it no longer protects retained history.
    ///
    /// A poisoned retention registry lock is recovered, not propagated.
    #[must_use]
    pub fn release_retention_consumer(&self, consumer: &str) -> bool {
        let (released, floors) = {
            let mut consumers = self
                .retention_consumers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let released = consumers.release(consumer);
            (released, consumers.snapshot())
        };
        if released {
            if let Err(error) = self.persist_retention_consumers(&floors) {
                tracing::warn!(%error, "failed to persist retention consumer release");
            }
        }
        released
    }

    /// Expire retention consumers whose lease lapsed without renewal, persist
    /// the registry, and return how many were dropped.
    /// Called by the maintenance tick before the retention floor is sampled.
    pub(crate) fn expire_retention_consumers(&self) -> usize {
        let (expired, floors) = {
            let mut consumers = self
                .retention_consumers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let expired = consumers.expire(crate::retention::unix_ms());
            (expired, consumers.snapshot())
        };
        if expired.is_empty() {
            return 0;
        }
        for consumer in &expired {
            tracing::warn!(
                consumer = consumer.as_str(),
                "retention consumer lease expired; it no longer pins the WAL"
            );
        }
        if let Err(error) = self.persist_retention_consumers(&floors) {
            tracing::warn!(%error, "failed to persist expired retention consumers");
        }
        expired.len()
    }

    /// Lowest acknowledged floor currently protected by a live consumer.
    ///
    /// Text indexes never pin the WAL: their applier resumes from
    /// pending rows, not from history, so the floor is the retention
    /// consumers' and live read pins' alone.
    ///
    /// A poisoned retention registry lock is recovered, not propagated.
    #[must_use]
    pub fn protected_retention_floor(&self) -> Option<u64> {
        self.consumer_and_pin_floor()
    }

    fn consumer_and_pin_floor(&self) -> Option<u64> {
        let consumer_floor = self
            .retention_consumers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .protected_floor();
        let read_pin_floor = self.store.retention_summary().0;
        [consumer_floor, read_pin_floor].into_iter().flatten().min()
    }

    pub(crate) fn retain_ephemeral_cursor(
        &self,
        consumer: String,
        floor: u64,
    ) -> Result<(), String> {
        self.retention_consumers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .acknowledge(consumer, floor, crate::retention::unix_ms())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn release_ephemeral_retention_consumer(&self, consumer: &str) {
        self.retention_consumers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .release(consumer);
    }

    /// Background ANN segment maintenance: freeze full hot segments
    /// and run size-tiered merges for every resident tenant's cached
    /// spaces. Best-effort; a failure is logged and retried next tick.
    pub(crate) fn maintain_vector_segments(&self) {
        for engine in self.residency.resident_vector_engines() {
            if let Err(error) = engine.maintain_segments() {
                tracing::warn!(%error, "vector segment maintenance failed");
            }
        }
    }

    /// Durable key for the (global) retention-consumer registry. Persisted once
    /// under the base tenant; the registry is not per request scope.
    pub(crate) const RETENTION_CONSUMERS_KEY: &'static str = "retention/consumers/state";

    pub(crate) fn persist_retention_consumers(
        &self,
        floors: &std::collections::BTreeMap<String, crate::retention::ConsumerLease>,
    ) -> Result<(), String> {
        self.put_meta_blob_for(
            self.base_tenant,
            Self::RETENTION_CONSUMERS_KEY,
            &serde_json::json!(floors),
        )
    }

    pub(crate) fn restore_retention_consumers(&self) -> Result<(), String> {
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        let Some(raw) = view
            .get(Self::RETENTION_CONSUMERS_KEY.as_bytes())
            .map_err(|e| e.to_string())?
        else {
            return Ok(());
        };
        let floors = serde_json::from_slice::<
            std::collections::BTreeMap<String, crate::retention::ConsumerLease>,
        >(&raw)
        .map_err(|e| format!("corrupt retention consumer state: {e}"))?;
        self.retention_consumers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(floors);
        Ok(())
    }

    /// Reclaim embedding vector slots superseded at or below `through_ci` for
    /// every tenant discovered in the durable keyspace. This deliberately
    /// does not depend on the resident vector-engine cache: a cold tenant's
    /// slots must be reclaimable after restart as well.
    ///
    /// # Errors
    /// Returns a message when a tenant's reclamation commit fails.
    ///
    /// A poisoned vector-engine cache mutex is recovered, not propagated.
    pub fn reclaim_vector_slots(&self, through_ci: u64) -> Result<usize, String> {
        self.reclaim_vector_slots_for(&self.durable_tenants()?, through_ci)
    }

    /// [`Self::reclaim_vector_slots`] over a caller-supplied tenant list.
    ///
    /// # Errors
    /// Returns a message when a tenant's reclamation commit fails.
    pub(crate) fn reclaim_vector_slots_for(
        &self,
        tenants: &[TenantId],
        through_ci: u64,
    ) -> Result<usize, String> {
        let mut total = 0usize;
        for &tenant in tenants {
            let lmpg = rushdb_lmpg::Lmpg::new(Arc::clone(&self.store), tenant);
            let engine = rushdb_vector::VectorEngine::new(Arc::new(lmpg));
            total += engine
                .reclaim_slots(through_ci)
                .map_err(|e| e.to_string())?;
        }
        Ok(total)
    }

    /// Every tenant with at least one durable key, in id order.
    ///
    /// # Errors
    /// Returns a message when the read view cannot be opened or a tenant key
    /// is malformed.
    pub(crate) fn durable_tenants(&self) -> Result<Vec<TenantId>, String> {
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        Ok(Self::enumerate_tenants(&view)?
            .into_iter()
            .map(|(tenant, _)| tenant)
            .collect())
    }

    /// Every tenant that owns data, in ordinal order: the directory's active
    /// tenants (DC8, O(tenants) rows) plus the single-tenant ordinal when it
    /// holds any key. Read at `view`'s snapshot.
    pub(crate) fn enumerate_tenants(
        view: &rushdb_storage::ReadView,
    ) -> Result<Vec<(TenantId, rushdb_types::ids::TenantScope)>, String> {
        use rushdb_storage::tenant_dir::{decode_by_ord, TenantState};
        let mut tenants = Vec::new();
        let single = rushdb_types::keys::tenant_prefix(TenantId::SINGLE);
        let mut cursor = view
            .scan(
                rushdb_storage::ScanBounds::prefix(&single),
                rushdb_storage::ScanProjection::Keys,
            )
            .map_err(|e| e.to_string())?;
        if !cursor
            .next_batch(1, 64 * 1024)
            .map_err(|e| e.to_string())?
            .rows
            .is_empty()
        {
            tenants.push((TenantId::SINGLE, rushdb_types::ids::TenantScope::single()));
        }
        let prefix = rushdb_types::keys::tenant_by_ord_prefix();
        for (key, raw) in view.range_prefix(&prefix).map_err(|e| e.to_string())? {
            let ordinal: [u8; 4] = key
                .get(prefix.len()..)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or("malformed tenant directory key")?;
            let (scope, state) = decode_by_ord(&raw).map_err(|e| e.to_string())?;
            if state == TenantState::Active {
                tenants.push((TenantId(u32::from_be_bytes(ordinal)), scope));
            }
        }
        Ok(tenants)
    }
}
