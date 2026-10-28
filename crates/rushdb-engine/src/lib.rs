//! Thin in-process facade over storage, LMPG, and the query executor.
//!
//! [`Engine`] is a convenience surface for embedding a single store without
//! the transport layer. It is deliberately narrower than the protocol
//! executor in `rushdb-server`: it exposes document import, search, and
//! aggregation, not the full mutation/describe/index/transaction surface.
use std::sync::Arc;

use rushdb_lmpg::{Lmpg, LmpgError};
use rushdb_query::{Page, QueryError, SearchRequest};
use rushdb_types::ids::TenantId;

pub mod context;
pub mod scheduler;
pub use context::{ExecutionContext, ExecutionError, OperationClass};
pub use scheduler::{BoundedScheduler, SchedulerError};

/// Storage engine boundary. An optional opaque scope is mapped to an internal
/// tenant before any lookup or planning. Application resource identifiers do
/// not cross this boundary.
pub struct Engine {
    /// Shared storage backing this engine and its tenant facades.
    pub store: Arc<rushdb_storage::Store>,
    scheduler: Arc<BoundedScheduler>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

fn response_bytes<T: serde::Serialize>(value: &T) -> Result<usize, QueryError> {
    serde_json::to_vec(value)
        .map(|encoded| encoded.len())
        .map_err(|error| {
            QueryError::ResourceLimit(format!("response serialization failed: {error}"))
        })
}

/// Estimate the memory needed to lower a JSON document without allocating.
/// Import staging retains the input tree, lowered scalar copies, and transaction
/// operations, so admission reserves a conservative multiple before cloning it.
fn import_memory_estimate(value: &serde_json::Value) -> usize {
    import_memory_estimate_at_depth(value, 0)
}

fn import_memory_estimate_at_depth(value: &serde_json::Value, depth: usize) -> usize {
    const NODE_OVERHEAD: usize = 32;
    const PROPERTY_METADATA: usize = 64;
    let (payload, children) = match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            (0, 0)
        }
        serde_json::Value::String(value) => (value.len(), 0),
        serde_json::Value::Array(values) => (
            0,
            values.iter().fold(0usize, |total, value| {
                total.saturating_add(import_memory_estimate_at_depth(
                    value,
                    depth.saturating_add(1),
                ))
            }),
        ),
        serde_json::Value::Object(values) => (
            values.iter().fold(0usize, |total, (key, _)| {
                total
                    .saturating_add(key.len())
                    .saturating_add(PROPERTY_METADATA)
            }),
            values.iter().fold(0usize, |total, (_, value)| {
                total.saturating_add(import_memory_estimate_at_depth(
                    value,
                    depth.saturating_add(1),
                ))
            }),
        ),
    };

    // Each node is retained by the input tree and copied into staging. Weight
    // the node itself by its depth, then add child costs once. Multiplying a
    // recursively estimated subtree by depth at every ancestor compounds the
    // factors and makes a narrow document appear factorially large.
    NODE_OVERHEAD
        .saturating_add(payload)
        .saturating_mul(depth.saturating_add(1))
        .saturating_add(children)
}

fn import_admission_bytes(label: &str, doc: &serde_json::Value) -> usize {
    // The multiplier covers the cloned JSON tree, lowered values, and staged
    // operation vectors. The depth weighting accounts for child subtrees cloned
    // at each recursive import level. This is a managed admission estimate, not
    // an exact RSS bound; LMPG owns the final staging layout.
    import_memory_estimate(doc)
        .saturating_mul(8)
        .saturating_add(label.len().saturating_mul(2))
        .saturating_add(4096)
}

impl Engine {
    /// Creates an engine backed by fresh, empty in-memory storage.
    ///
    /// # Panics
    /// Panics only if the built-in nonzero scheduler configuration is invalid.
    #[must_use]
    pub fn new_in_memory() -> Self {
        Engine {
            store: Arc::new(rushdb_storage::Store::new_in_memory()),
            scheduler: Arc::new(
                BoundedScheduler::new(2, 64).expect("default scheduler configuration is valid"),
            ),
        }
    }

    /// Builds a facade over externally recovered storage.
    ///
    /// # Panics
    /// Panics when deterministic replay rejects the supplied log. Use
    /// [`Engine::try_open_recovered`] for fallible recovery.
    #[must_use]
    pub fn open_recovered(log: &[Vec<u8>], scope: Option<&str>) -> Self {
        Self::try_open_recovered(log, scope).expect("deterministic replay")
    }

    /// Fallible constructor for embedded crash recovery.
    ///
    /// # Errors
    /// Returns [`rushdb_storage::StorageError`] when the supplied command log
    /// is malformed or cannot be deterministically expanded.
    pub fn try_open_recovered(
        log: &[Vec<u8>],
        scope: Option<&str>,
    ) -> Result<Self, rushdb_storage::StorageError> {
        let engine = Engine::new_in_memory();
        engine.try_replay(log, scope)?;
        Ok(engine)
    }

    /// Every canonical storage key carries the resolved tenant prefix. A
    /// missing scope selects the isolated single-tenant domain; a scope is
    /// resolved through the tenant directory, registering it on first use
    ///
    /// # Errors
    /// Returns [`rushdb_storage::StorageError`] when the directory cannot be
    /// read or the registration commit fails.
    pub fn tenant(&self, scope: Option<&str>) -> Result<Lmpg, rushdb_storage::StorageError> {
        let tenant = self.resolve(scope)?;
        Ok(Lmpg::new(Arc::clone(&self.store), tenant))
    }

    fn resolve(&self, scope: Option<&str>) -> Result<TenantId, rushdb_storage::StorageError> {
        scope.map_or(Ok(TenantId::SINGLE), |scope| {
            rushdb_lmpg::tenancy::resolve_or_register_scope(&self.store, scope)
        })
    }

    /// Create a typed, pinned execution context for an already-resolved scope.
    ///
    /// # Errors
    /// Returns [`ExecutionError::Storage`] when the current read view cannot
    /// be opened.
    pub fn context(
        &self,
        scope: Option<&str>,
        operation: OperationClass,
        deadline: Option<std::time::Duration>,
        byte_budget: Option<usize>,
    ) -> Result<ExecutionContext, ExecutionError> {
        let tenant = self.resolve(scope).map_err(ExecutionError::Storage)?;
        ExecutionContext::new(&self.store, tenant, operation, deadline, byte_budget)
    }

    /// Fallible crash-recovery entry point. Malformed or unsupported command
    /// logs are rejected before any replayed state is published.
    ///
    /// # Errors
    /// Returns [`rushdb_storage::StorageError`] when the log cannot be decoded
    /// or deterministic expansion rejects an entry.
    pub fn try_replay(
        &self,
        log: &[Vec<u8>],
        scope: Option<&str>,
    ) -> Result<(), rushdb_storage::StorageError> {
        // Wire the expander through a throwaway tenant facade so replays use
        // the same deterministic path as forward commits.
        let _ = self.tenant(scope);
        // Use the fallible storage boundary directly: it validates and
        // prepares the complete log before publishing any replayed state.
        self.store.try_replay(log)
    }

    // ---- convenience frontends ----------------------------------------

    /// Imports one JSON document under `label` and returns its record reference.
    ///
    /// # Errors
    ///
    /// Returns [`LmpgError`] if the document cannot be lowered into record
    /// fields or the underlying store rejects the write.
    pub fn import_document(
        &self,
        scope: Option<&str>,
        label: &str,
        doc: &serde_json::Value,
    ) -> Result<rushdb_lmpg::RecordRef, LmpgError> {
        let mut context = self
            .context(scope, OperationClass::Write, None, None)
            .map_err(|error| LmpgError::Conflict(error.to_string()))?;
        self.import_document_with_context(&mut context, label, doc)
    }

    /// Imports one document through a caller-owned execution context.
    ///
    /// The context supplies the resolved tenant, deadline, and shared phase
    /// reservation; the bounded scheduler supplies the global and per-tenant
    /// admission. This is the embedded equivalent of the transport mutation
    /// boundary and keeps product identity outside the engine.
    ///
    /// # Errors
    /// Returns [`LmpgError`] when admission, deadline, planning, or storage
    /// rejects the mutation.
    pub fn import_document_with_context(
        &self,
        context: &mut ExecutionContext,
        label: &str,
        doc: &serde_json::Value,
    ) -> Result<rushdb_lmpg::RecordRef, LmpgError> {
        if !context.belongs_to(&self.store) {
            return Err(LmpgError::Conflict(
                ExecutionError::ForeignStore.to_string(),
            ));
        }
        context
            .check_active()
            .map_err(|error| LmpgError::Conflict(error.to_string()))?;
        let admission_bytes = import_admission_bytes(label, doc);
        context
            .charge_bytes(admission_bytes)
            .map_err(|error| LmpgError::Conflict(error.to_string()))?;
        let tenant = context.tenant();
        let store = Arc::clone(&self.store);
        let label = label.to_owned();
        let doc = doc.clone();
        let deadline = context.deadline();
        let read_view = Arc::clone(context.read_view());
        let base_ci = context.read_view().commit_index();
        let result = self
            .scheduler
            .execute_for_tenant(tenant, move || {
                let lmpg = Lmpg::new(Arc::clone(&store), tenant);
                let mut tx = rushdb_storage::TxAccumulator::new(tenant, base_ci);
                let record = rushdb_lmpg::import::import_document_tx_with_cancel(
                    &lmpg,
                    &label,
                    &doc,
                    &mut tx,
                    &|| {
                        read_view.check_cancelled().is_err()
                            || deadline.is_some_and(|at| std::time::Instant::now() >= at)
                    },
                )?;
                if read_view.check_cancelled().is_err()
                    || deadline.is_some_and(|at| std::time::Instant::now() >= at)
                {
                    return Err(LmpgError::Conflict("import cancelled".into()));
                }
                store
                    .tx_commit(uuid::Uuid::now_v7(), &tx)
                    .map_err(|error| LmpgError::Conflict(error.to_string()))?;
                Ok(record)
            })
            .map_err(|error| LmpgError::Conflict(error.to_string()));
        context.release_bytes(admission_bytes);
        let result = result?;
        // A deadline can elapse after tx_commit publishes. Returning an error
        // here would falsely tell callers that the mutation did not commit.
        result
    }

    /// Runs a search request against the resolved tenant and returns its page.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError`] if the request is invalid or execution fails.
    pub fn search(&self, scope: Option<&str>, req: &SearchRequest) -> Result<Page, QueryError> {
        let mut context = self
            .context(scope, OperationClass::Read, None, None)
            .map_err(|error| QueryError::ResourceLimit(error.to_string()))?;
        self.search_with_context(&mut context, req)
    }

    /// Runs a search with a caller-owned execution context. The context pins
    /// the read view used by candidate selection and reconstruction, checks
    /// the deadline around scheduler admission, and charges the materialized
    /// response against its byte budget.
    ///
    /// # Errors
    /// Returns query, scheduler, deadline, storage, or budget errors.
    pub fn search_with_context(
        &self,
        context: &mut ExecutionContext,
        req: &SearchRequest,
    ) -> Result<Page, QueryError> {
        context
            .check_active()
            .map_err(|error| QueryError::ResourceLimit(error.to_string()))?;
        if !context.belongs_to(&self.store) {
            return Err(QueryError::Storage(
                rushdb_storage::StorageError::InvalidRead(
                    "execution context belongs to another Store".into(),
                ),
            ));
        }
        let tenant = context.tenant();
        let store = Arc::clone(&self.store);
        let request = req.clone();
        let snap = Arc::clone(context.read_view());
        let cancellation = Arc::clone(context.read_view());
        let work_budget = context.remaining_budget();
        let deadline = context.deadline();
        self.scheduler
            .execute_for_tenant(tenant, move || {
                let is_cancelled = || {
                    cancellation.check_cancelled().is_err()
                        || deadline.is_some_and(|at| std::time::Instant::now() >= at)
                };
                rushdb_query::find_records_in_view_with_budget(
                    &Lmpg::new(store, tenant),
                    &request,
                    &is_cancelled,
                    snap,
                    work_budget,
                )
            })
            .map_err(|error| QueryError::ResourceLimit(error.to_string()))?
            .and_then(|page| {
                context
                    .check_active()
                    .map_err(|error| QueryError::ResourceLimit(error.to_string()))?;
                let bytes = response_bytes(&page.records)?;
                context
                    .charge_bytes(bytes)
                    .map(|()| page)
                    .map_err(|error| QueryError::ResourceLimit(error.to_string()))
            })
    }

    /// Runs an aggregate request against the resolved tenant and returns the
    /// resulting group buckets.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError`] if the request is invalid or execution fails.
    pub fn aggregate(
        &self,
        scope: Option<&str>,
        req: &rushdb_query::AggregateRequest,
    ) -> Result<Vec<rushdb_query::GroupBucket>, QueryError> {
        let mut context = self
            .context(scope, OperationClass::Read, None, None)
            .map_err(|error| QueryError::ResourceLimit(error.to_string()))?;
        self.aggregate_with_context(&mut context, req)
    }

    /// Runs an aggregate with a caller-owned pinned read view and response
    /// byte budget.
    ///
    /// # Errors
    /// Returns query, scheduler, deadline, storage, or budget errors.
    pub fn aggregate_with_context(
        &self,
        context: &mut ExecutionContext,
        req: &rushdb_query::AggregateRequest,
    ) -> Result<Vec<rushdb_query::GroupBucket>, QueryError> {
        context
            .check_active()
            .map_err(|error| QueryError::ResourceLimit(error.to_string()))?;
        if !context.belongs_to(&self.store) {
            return Err(QueryError::Storage(
                rushdb_storage::StorageError::InvalidRead(
                    "execution context belongs to another Store".into(),
                ),
            ));
        }
        let tenant = context.tenant();
        let store = Arc::clone(&self.store);
        let request = req.clone();
        let snap = Arc::clone(context.read_view());
        let cancellation = Arc::clone(context.read_view());
        let deadline = context.deadline();
        let work_budget = context.remaining_budget();
        self.scheduler
            .execute_for_tenant(tenant, move || {
                let is_cancelled = || {
                    cancellation.check_cancelled().is_err()
                        || deadline.is_some_and(|at| std::time::Instant::now() >= at)
                };
                rushdb_query::aggregate_with_cancel_in_view_with_budget(
                    &Lmpg::new(store, tenant),
                    &request,
                    &is_cancelled,
                    snap,
                    work_budget,
                )
                .map(|(buckets, _)| buckets)
            })
            .map_err(|error| QueryError::ResourceLimit(error.to_string()))?
            .and_then(|buckets| {
                context
                    .check_active()
                    .map_err(|error| QueryError::ResourceLimit(error.to_string()))?;
                let bytes = response_bytes(&buckets)?;
                context
                    .charge_bytes(bytes)
                    .map(|()| buckets)
                    .map_err(|error| QueryError::ResourceLimit(error.to_string()))
            })
    }
}

#[cfg(test)]
mod response_tests {
    use super::response_bytes;

    #[test]
    fn response_serialization_errors_fail_closed_instead_of_charging_zero() {
        struct Fails;
        impl serde::Serialize for Fails {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(serde::ser::Error::custom("synthetic serialization failure"))
            }
        }

        let error = response_bytes(&Fails).expect_err("serialization should fail");
        assert!(error.to_string().contains("response serialization failed"));
    }
}
