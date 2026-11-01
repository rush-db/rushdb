//! Per-adapter resource budgets: the vector adapter's own cache budget
//! (`vector_cache_bytes`, a child of the managed limit), and a named child of
//! the shared phase budget per query adapter (SQL, graph), so each adapter's
//! `peak`/`used` (a real, measured high-water mark — see
//! [`crate::resources::ByteBudget::snapshot`]) is individually observable.
//! Each phase child's limit equals its parent's, so those are pure
//! attribution: they do not narrow the shared capacity the adapters contend
//! on (see [`crate::resources::ByteBudget::child`]).

use crate::resources::ByteBudget;
use crate::{StorageError, Store};

/// The vector cache budget and one named attribution child per query
/// adapter. `None` exactly when its budget (`vector_cache_bytes` /
/// `phase_resources`) is disabled.
pub(crate) struct AdapterBudgets {
    vector_cache: Option<ByteBudget>,
    sql_phase: Option<ByteBudget>,
    graph_phase: Option<ByteBudget>,
}

impl AdapterBudgets {
    pub(crate) fn new(
        vector_cache: Option<ByteBudget>,
        phase: Option<&ByteBudget>,
    ) -> Result<Self, StorageError> {
        let child = |parent: Option<&ByteBudget>, name: &'static str| {
            parent
                .map(|parent| parent.child(name, parent.snapshot().limit))
                .transpose()
                .map_err(|e| StorageError::InvalidConfiguration(e.to_string()))
        };
        Ok(Self {
            vector_cache,
            sql_phase: child(phase, "sql-phase")?,
            graph_phase: child(phase, "graph-phase")?,
        })
    }
}

impl Store {
    /// The vector adapter's cache budget (`vector_cache_bytes`).
    #[must_use]
    pub fn vector_cache_budget(&self) -> Option<ByteBudget> {
        self.adapter_budgets.vector_cache.clone()
    }

    /// The SQL adapter's named child of [`Store::phase_budget`].
    #[must_use]
    pub fn sql_phase_budget(&self) -> Option<ByteBudget> {
        self.adapter_budgets.sql_phase.clone()
    }

    /// The native graph/query adapter's named child of [`Store::phase_budget`]
    #[must_use]
    pub fn graph_phase_budget(&self) -> Option<ByteBudget> {
        self.adapter_budgets.graph_phase.clone()
    }
}
