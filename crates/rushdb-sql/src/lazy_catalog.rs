//! Lazy `DataFusion` schema providers for the SQL read path.
//!
//! The read path used to reconstruct every label's records (schema inference)
//! and materialize every relationship table on every query, even `SELECT 1`.
//! These providers defer that work to the first time a table is actually
//! resolved, so a query pays only for the labels/edge types it references.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use datafusion::catalog::{SchemaProvider, TableProvider};
use datafusion::common::DataFusionError;
use rushdb_lmpg::Lmpg;
use rushdb_storage::ReadView;
use rushdb_types::ids::TenantId;

use crate::edge_provider::EdgeTable;
use crate::scan_provider::LabelTable;
use crate::{relationship_types_for_tenant, SqlError};
use rushdb_lmpg::CatalogAccess;

fn df_err(error: &SqlError) -> DataFusionError {
    DataFusionError::Execution(error.to_string())
}

/// Lazily builds one label table per referenced label.
pub(crate) struct LazyLabelSchema {
    /// The tenant's facade for the index planner (store and tenant only).
    lmpg: Arc<Lmpg>,
    snapshot: Arc<ReadView>,
    names: Vec<String>,
    visible: CatalogAccess,
    cache: Mutex<HashMap<String, Arc<dyn TableProvider>>>,
}

impl fmt::Debug for LazyLabelSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyLabelSchema")
            .field("labels", &self.names)
            .finish_non_exhaustive()
    }
}

impl LazyLabelSchema {
    /// Enumerate the tenant's labels (a catalog read, not a record scan).
    ///
    /// # Errors
    /// Returns a storage error when the label catalog cannot be read.
    /// Under an enforced policy only its allowed labels are listed.
    pub(crate) fn new(
        lmpg: &Lmpg,
        snapshot: Arc<ReadView>,
        visible: CatalogAccess,
    ) -> Result<Self, SqlError> {
        let mut names = rushdb_lmpg::reader::try_access::labels(&snapshot, lmpg.tenant)?;
        names.retain(|label| visible.label(label));
        Ok(Self {
            // The request's handle: its bounds reach the scans.
            lmpg: Arc::new(lmpg.handle()),
            snapshot,
            names,
            visible,
            cache: Mutex::new(HashMap::new()),
        })
    }
}

#[async_trait]
impl SchemaProvider for LazyLabelSchema {
    fn table_names(&self) -> Vec<String> {
        self.names.clone()
    }

    fn table_exist(&self, name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        self.names
            .iter()
            .any(|candidate| candidate.to_ascii_lowercase() == lower)
    }

    async fn table(&self, name: &str) -> Result<Option<Arc<dyn TableProvider>>, DataFusionError> {
        let lower = name.to_ascii_lowercase();
        if let Some(provider) = self
            .cache
            .lock()
            .map_err(|_| DataFusionError::Execution("label schema cache poisoned".into()))?
            .get(&lower)
        {
            return Ok(Some(Arc::clone(provider)));
        }
        let Some(canonical) = self
            .names
            .iter()
            .find(|candidate| candidate.to_ascii_lowercase() == lower)
        else {
            return Ok(None);
        };
        let provider: Arc<dyn TableProvider> = Arc::new(
            LabelTable::try_new_visible(
                Arc::clone(&self.lmpg),
                canonical.clone(),
                Arc::clone(&self.snapshot),
                &self.visible,
            )
            .map_err(|error| df_err(&error))?,
        );
        self.cache
            .lock()
            .map_err(|_| DataFusionError::Execution("label schema cache poisoned".into()))?
            .insert(lower, Arc::clone(&provider));
        Ok(Some(provider))
    }
}

/// Lazily discovers relationship types (from the relationship catalog) and
/// resolves one streamed edge table per referenced type.
pub(crate) struct LazyRelationshipSchema {
    tenant: TenantId,
    snapshot: Arc<ReadView>,
    types: OnceLock<Vec<String>>,
    visible: CatalogAccess,
    cache: Mutex<HashMap<String, Arc<dyn TableProvider>>>,
}

impl fmt::Debug for LazyRelationshipSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyRelationshipSchema")
            .finish_non_exhaustive()
    }
}

impl LazyRelationshipSchema {
    pub(crate) fn new(tenant: TenantId, snapshot: Arc<ReadView>, visible: CatalogAccess) -> Self {
        Self {
            tenant,
            snapshot,
            types: OnceLock::new(),
            visible,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn canonical_types(&self) -> Result<&Vec<String>, DataFusionError> {
        if let Some(types) = self.types.get() {
            return Ok(types);
        }
        let types: Vec<String> = relationship_types_for_tenant(&self.snapshot, self.tenant)
            .map_err(|error| df_err(&error))?
            .into_iter()
            .filter(|rel_type| self.visible.rel_type(rel_type))
            .collect();
        Ok(self.types.get_or_init(|| types))
    }
}

#[async_trait]
impl SchemaProvider for LazyRelationshipSchema {
    fn table_names(&self) -> Vec<String> {
        self.canonical_types()
            .map(|types| types.iter().map(|t| t.to_ascii_lowercase()).collect())
            .unwrap_or_default()
    }

    fn table_exist(&self, name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        self.canonical_types()
            .is_ok_and(|types| types.iter().any(|t| t.to_ascii_lowercase() == lower))
    }

    async fn table(&self, name: &str) -> Result<Option<Arc<dyn TableProvider>>, DataFusionError> {
        let lower = name.to_ascii_lowercase();
        if let Some(provider) = self
            .cache
            .lock()
            .map_err(|_| DataFusionError::Execution("edge schema cache poisoned".into()))?
            .get(&lower)
        {
            return Ok(Some(Arc::clone(provider)));
        }
        let canonical = self
            .canonical_types()?
            .iter()
            .find(|t| t.to_ascii_lowercase() == lower)
            .cloned();
        let Some(canonical) = canonical else {
            return Ok(None);
        };
        let provider: Arc<dyn TableProvider> = Arc::new(
            EdgeTable::new(
                Arc::clone(&self.snapshot),
                self.tenant,
                &canonical,
                self.visible.clone(),
            )
            .map_err(|error| df_err(&error))?,
        );
        self.cache
            .lock()
            .map_err(|_| DataFusionError::Execution("edge schema cache poisoned".into()))?
            .insert(lower, Arc::clone(&provider));
        Ok(Some(provider))
    }
}
