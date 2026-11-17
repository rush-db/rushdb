//! The catalog reads every describe and listing repeats, as resident layers
//! (`rushdb_storage::resident`): the label catalog (labels and their counts)
//! and the property dictionary. Each stays valid until a commit writes what
//! it reflects, so commits to other tenants or to unrelated rows keep it.

use rushdb_storage::resident::{Budget, BuildCx, BuildError, Dep, Resident};
use rushdb_storage::{ReadView, StorageError};
use rushdb_types::ids::TenantId;

use super::{Arc, PropertyTypeMap};

/// Labels and per-label counts, in first-appearance order.
pub(super) type LabelCatalog = (Vec<String>, Vec<(String, u64)>);

fn catalog_error(error: String) -> BuildError {
    BuildError::Storage(StorageError::InvalidRead(error))
}

/// The label catalog of a tenant.
#[derive(Default)]
pub(super) struct Labels;

impl Resident for Labels {
    type Key = TenantId;
    type Value = LabelCatalog;
    const NAME: &'static str = "label-catalog";
    const BUDGET: Budget = Budget::Logical;

    fn deps(&self, tenant: &TenantId, _view: &ReadView) -> Result<Vec<Dep>, StorageError> {
        Ok(vec![Dep::Labels(*tenant), Dep::Policy(*tenant)])
    }

    fn build(
        &self,
        tenant: &TenantId,
        view: &ReadView,
        _cx: &BuildCx,
    ) -> Result<Option<LabelCatalog>, BuildError> {
        Ok(Some(label_catalog(view, *tenant)?))
    }

    fn bytes((labels, counts): &LabelCatalog) -> usize {
        labels
            .iter()
            .map(|label| label.len() + 16)
            .chain(counts.iter().map(|(label, _)| label.len() + 24))
            .sum::<usize>()
            + 64
    }
}

/// Labels and counts of `tenant` at `view`, read from the label counters.
pub(super) fn label_catalog(
    view: &ReadView,
    tenant: TenantId,
) -> Result<LabelCatalog, StorageError> {
    let counts = rushdb_lmpg::reader::try_access::label_counts(view, tenant)?;
    let labels = counts.iter().map(|(label, _)| label.clone()).collect();
    Ok((labels, counts))
}

/// The property dictionary of a tenant: normalized name → type name.
#[derive(Default)]
pub(super) struct Properties;

impl Resident for Properties {
    type Key = TenantId;
    type Value = PropertyTypeMap;
    const NAME: &'static str = "property-dictionary";
    const BUDGET: Budget = Budget::Logical;

    fn deps(&self, tenant: &TenantId, _view: &ReadView) -> Result<Vec<Dep>, StorageError> {
        Ok(vec![Dep::Definitions(*tenant), Dep::Policy(*tenant)])
    }

    fn build(
        &self,
        tenant: &TenantId,
        view: &ReadView,
        _cx: &BuildCx,
    ) -> Result<Option<PropertyTypeMap>, BuildError> {
        property_types(view, *tenant)
            .map(Some)
            .map_err(catalog_error)
    }

    fn bytes(map: &PropertyTypeMap) -> usize {
        map.iter()
            .map(|(name, ty)| name.len() + ty.len() + 48)
            .sum::<usize>()
            + 64
    }
}

/// The property dictionary of `tenant` at `view`: each name's first typed
/// definition.
pub(super) fn property_types(view: &ReadView, tenant: TenantId) -> Result<PropertyTypeMap, String> {
    Ok(super::catalog::property_meta(view, tenant)?
        .into_iter()
        .filter_map(|(name, defs)| defs.into_iter().next().map(|(_, _, ty, _)| (name, ty)))
        .collect())
}

/// The resident value of layer `R` for `tenant` at `view`, or the one
/// `read` computes now (then offered to the layer).
pub(super) fn cached<R, E>(
    view: &ReadView,
    tenant: TenantId,
    read: impl FnOnce() -> Result<R::Value, E>,
) -> Result<Arc<R::Value>, E>
where
    R: Resident<Key = TenantId>,
{
    let layer = view.residents().layer::<R>();
    if let Some(value) = layer.get(&tenant, view) {
        return Ok(value);
    }
    let value = Arc::new(read()?);
    layer.offer(tenant, view, Arc::clone(&value));
    Ok(value)
}
