//! Declared ordered indexes on the node: DDL, listing, and the
//! online builder that advances building and dropping indexes in bounded
//! batches on the maintenance tick.

use std::time::{Duration, Instant};

use rushdb_lmpg::index::{self, IndexStep};
use rushdb_types::index::{IndexDef, IndexKind, IndexState, TextConsistency};

use super::{Node, RequestScope};

/// Records per build or drop commit.
pub(crate) const INDEX_BUILD_BATCH: usize = 1_000;

/// Pending rows an index listing counts at most.
const PENDING_COUNT_LIMIT: usize = 1_000_000;

fn describe(def: &IndexDef) -> serde_json::Value {
    let mut out = serde_json::json!({
        "name": def.name,
        "label": def.label,
        "property": def.property,
        "kind": "ordered",
        "state": def.state.label(),
    });
    match def.kind {
        IndexKind::Ordered => {}
        IndexKind::Text { consistency } => {
            out["kind"] = "text".into();
            out["consistency"] = match consistency {
                TextConsistency::Immediate => "immediate",
                TextConsistency::Eventual => "eventual",
            }
            .into();
        }
        IndexKind::Structural { dimensions } => {
            out["kind"] = "structural".into();
            out["dimensions"] = dimensions.into();
            out["model"] = rushdb_vector::structural::STRUCTURAL_MODEL.into();
        }
    }
    out
}

/// Vector registry row of a structural index, so `query.vector` with
/// `index: <name>` searches it like any vector index.
fn structural_vector_row(def: &IndexDef, dimensions: u32) -> serde_json::Value {
    serde_json::json!({
        "id": def.name,
        "name": def.name,
        "label": def.label,
        "property": def.property,
        "similarity": "cosine",
        "dimensions": dimensions,
        "status": "awaiting_vectors",
        "model": rushdb_vector::structural::STRUCTURAL_MODEL,
        "nameSource": "explicit",
        "owner": "record",
        "generation": uuid::Uuid::now_v7().to_string(),
    })
}

/// Parse a protocol index kind (`ordered` | `text` with a consistency |
/// `structural` with dimensions).
fn kind_of(definition: &rushdb_protocol::IndexDefinition) -> Result<IndexKind, String> {
    match (definition.kind.as_str(), definition.dimensions) {
        ("structural", Some(dimensions)) if definition.consistency.is_none() => {
            return Ok(IndexKind::Structural { dimensions });
        }
        ("structural", _) => {
            return Err("a structural index takes `dimensions` and no `consistency`".into());
        }
        (_, Some(_)) => return Err("`dimensions` applies to structural indexes only".into()),
        _ => {}
    }
    match (definition.kind.as_str(), definition.consistency.as_deref()) {
        ("ordered", None) => Ok(IndexKind::Ordered),
        ("ordered", Some(_)) => Err("`consistency` applies to text indexes only".into()),
        ("text", None | Some("eventual")) => Ok(IndexKind::Text {
            consistency: TextConsistency::Eventual,
        }),
        ("text", Some("immediate")) => Ok(IndexKind::Text {
            consistency: TextConsistency::Immediate,
        }),
        ("text", Some(other)) => Err(format!(
            "unknown text consistency `{other}`; use `immediate` or `eventual`"
        )),
        (other, _) => Err(format!(
            "unsupported index kind `{other}`; declared indexes are `ordered`, `text` or `structural` (vector indexes use createVector)"
        )),
    }
}

impl RequestScope<'_> {
    /// Declare an ordered index in the request's tenant.
    ///
    /// # Errors
    /// Returns a message for an unknown kind, a duplicate name, or a failed
    /// commit.
    pub(crate) fn create_index(
        &self,
        definition: &rushdb_protocol::IndexDefinition,
    ) -> Result<serde_json::Value, String> {
        let kind = kind_of(definition)?;
        if let IndexKind::Structural { .. } = kind {
            // Its vectors are searched by name through the vector registry.
            if self
                .store
                .try_get(self.vector_index_key(&definition.name).as_bytes())
                .map_err(|e| e.to_string())?
                .is_some()
            {
                return Err(format!("vector index `{}` already exists", definition.name));
            }
        }
        let def = self
            .lmpg()
            .define_index(
                &definition.name,
                &definition.label,
                &definition.property,
                kind,
            )
            .map_err(|e| e.to_string())?;
        if let IndexKind::Structural { dimensions } = def.kind {
            self.vector_engine()?
                .ensure_space_similarity(
                    &def.name,
                    dimensions,
                    rushdb_types::occurrence::Similarity::Cosine,
                )
                .map_err(|e| e.to_string())?;
            self.put_meta_blob(
                &self.vector_index_key(&def.name),
                &structural_vector_row(&def, dimensions),
            )?;
        }
        self.index_build_wake.notify_one();
        Ok(describe(&def))
    }

    /// Start dropping a declared index in the request's tenant.
    ///
    /// # Errors
    /// Returns a message when the index does not exist or the commit fails.
    pub(crate) fn drop_index(&self, name: &str) -> Result<(), String> {
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        let structural = index::by_name(&view, self.tenant(), name)
            .map_err(|e| e.to_string())?
            .is_some_and(|def| matches!(def.kind, IndexKind::Structural { .. }));
        drop(view);
        self.lmpg().drop_index(name).map_err(|e| e.to_string())?;
        if structural {
            // Its vectors and registry row go with it.
            self.drop_vector_index(name)?;
        }
        self.index_build_wake.notify_one();
        Ok(())
    }

    /// Declared indexes of the request's tenant at `view`.
    ///
    /// # Errors
    /// Returns a message when the catalog cannot be read.
    pub(crate) fn list_indexes_in_view(
        &self,
        view: &rushdb_storage::ReadView,
    ) -> Result<Vec<serde_json::Value>, String> {
        index::definitions(view, self.tenant())
            .map_err(|e| e.to_string())?
            .iter()
            .map(|def| {
                let mut out = describe(def);
                if def.kind.applied() {
                    // Freshness: records whose change the applier has not
                    // processed yet (counted up to a bound).
                    out["pending"] =
                        index::pending(view, self.tenant(), def.id, PENDING_COUNT_LIMIT)
                            .map_err(|e| e.to_string())?
                            .len()
                            .into();
                }
                Ok(out)
            })
            .collect()
    }
}

impl Node {
    /// Advance every building or dropping index of every tenant, a batch at a
    /// time, until done or `budget` elapses. Returns the commits made.
    pub(crate) fn step_index_builds(&self, budget: Duration) -> usize {
        let started = Instant::now();
        let tenants = match self.durable_tenants() {
            Ok(tenants) => tenants,
            Err(error) => {
                tracing::warn!(%error, "index builder cannot enumerate tenants");
                return 0;
            }
        };
        let mut commits = 0;
        for tenant in tenants {
            let pending: Vec<(u32, bool)> = match self
                .store
                .read_view()
                .map_err(|e| e.to_string())
                .and_then(|view| index::definitions(&view, tenant).map_err(|e| e.to_string()))
            {
                Ok(defs) => defs
                    .into_iter()
                    .filter(|def| def.state != IndexState::Ready)
                    .map(|def| {
                        let text_drop = matches!(def.kind, IndexKind::Text { .. })
                            && matches!(def.state, IndexState::Dropping { .. });
                        (def.id, text_drop)
                    })
                    .collect(),
                Err(error) => {
                    tracing::warn!(%error, %tenant, "index builder cannot read the catalog");
                    continue;
                }
            };
            if pending.is_empty() {
                continue;
            }
            // Only a tenant with work gets an engine handle (constructing one
            // re-installs the store's expander and deriver).
            let lmpg = rushdb_lmpg::Lmpg::new(std::sync::Arc::clone(&self.store), tenant);
            for (id, text_drop) in pending {
                loop {
                    if started.elapsed() >= budget {
                        return commits;
                    }
                    match lmpg.index_step(id, INDEX_BUILD_BATCH) {
                        Ok(IndexStep::Advanced(_)) => commits += 1,
                        Ok(IndexStep::Finished) => {
                            commits += 1;
                            // A dropped text index's files go with it.
                            if text_drop {
                                if let Err(error) = self.text.remove(tenant, id) {
                                    tracing::warn!(%error, %tenant, id, "cannot remove text index files");
                                }
                            }
                            break;
                        }
                        Ok(IndexStep::Idle) => break,
                        Err(error) => {
                            tracing::warn!(%error, %tenant, id, "index build step failed");
                            break;
                        }
                    }
                }
            }
        }
        commits
    }
}

impl Node {
    /// Apply pending changes of every tenant's structural indexes, within
    /// `budget`: embed the marked records and bind their vectors. Tenants
    /// without structural indexes cost one catalog read. Returns the records
    /// processed.
    pub(crate) fn apply_structural_indexes(&self, budget: Duration) -> usize {
        let started = Instant::now();
        let tenants = match self.durable_tenants() {
            Ok(tenants) => tenants,
            Err(error) => {
                tracing::warn!(%error, "structural applier cannot enumerate tenants");
                return 0;
            }
        };
        let mut applied = 0usize;
        for tenant in tenants {
            let defs: Vec<IndexDef> = match self
                .store
                .read_view()
                .map_err(|e| e.to_string())
                .and_then(|view| index::definitions(&view, tenant).map_err(|e| e.to_string()))
            {
                Ok(defs) => defs
                    .into_iter()
                    .filter(|def| {
                        matches!(def.kind, IndexKind::Structural { .. })
                            && !matches!(def.state, IndexState::Dropping { .. })
                    })
                    .collect(),
                Err(error) => {
                    tracing::warn!(%error, %tenant, "structural applier cannot read the catalog");
                    continue;
                }
            };
            if defs.is_empty() {
                continue;
            }
            let scope = RequestScope::for_tenant(self, tenant, None);
            let engine = match scope.vector_engine() {
                Ok(engine) => engine,
                Err(error) => {
                    tracing::warn!(%error, %tenant, "structural applier has no vector engine");
                    continue;
                }
            };
            for def in &defs {
                while started.elapsed() < budget {
                    match engine.apply_structural(def) {
                        Ok(0) => break,
                        Ok(count) => applied = applied.saturating_add(count),
                        Err(error) => {
                            tracing::warn!(%error, %tenant, index = %def.name, "structural apply failed");
                            break;
                        }
                    }
                }
            }
        }
        applied
    }
}
