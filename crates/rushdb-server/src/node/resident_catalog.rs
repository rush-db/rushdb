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

/// One `describe.properties` request without a filter: the tenant and the
/// request document (labels, detail, entry window), serialized.
pub(super) type DescribeKey = (TenantId, String);

/// Answers of unfiltered `describe.properties` requests (and the property
/// part of `describe.schema`), as the request computed them. The answer is
/// a function of the snapshot alone: it reads the property definitions,
/// the label catalog (when no label is named), and per described label its
/// members (samples), its
/// label-scoped property rows (support, array shape, value dictionary:
/// ranges and distinct counts) and the occurrences of the properties it
/// holds. A write to any of those drops it; a write to another label's
/// rows of properties it does not hold keeps it. A policy acts before (the
/// labels in the key) and after (masking), never inside.
///
/// Entries are offered by the request that computed them, not built in the
/// background (`build` declines): the computation needs the request scope.
#[derive(Default)]
pub(super) struct Describes;

impl Resident for Describes {
    type Key = DescribeKey;
    type Value = Vec<rushdb_protocol::PropertyDescription>;
    const NAME: &'static str = "describe";
    const BUDGET: Budget = Budget::Logical;

    fn deps(
        &self,
        (tenant, document): &DescribeKey,
        view: &ReadView,
    ) -> Result<Vec<Dep>, StorageError> {
        let tenant = *tenant;
        let mut deps = vec![Dep::Definitions(tenant)];
        let named: Vec<String> = serde_json::from_str::<serde_json::Value>(document)
            .ok()
            .and_then(|document| document.get("labels").cloned())
            .and_then(|labels| serde_json::from_value(labels).ok())
            .unwrap_or_default();
        // Every label is described when none is named: then the label
        // catalog decides which.
        let labels = if named.is_empty() {
            deps.push(Dep::Labels(tenant));
            label_catalog(view, tenant)?.0
        } else {
            named
        };
        for label in &labels {
            let hash = rushdb_storage::resident::label_hash(label);
            deps.push(Dep::Label(tenant, hash));
            deps.push(Dep::LabelProperties(tenant, hash));
        }
        // The occurrences a value change writes are not label-scoped: depend
        // on every property the described labels hold (a property new to a
        // label writes its label-scoped support row).
        for pid in property_ids(view, tenant)? {
            let held =
                rushdb_lmpg::reader::try_access::prop_support_label_counts(view, tenant, pid)?
                    .iter()
                    .any(|(label, _)| labels.contains(label));
            if held {
                deps.push(Dep::Pid(tenant, pid));
            }
        }
        Ok(deps)
    }

    fn build(
        &self,
        _key: &DescribeKey,
        _view: &ReadView,
        _cx: &BuildCx,
    ) -> Result<Option<Self::Value>, BuildError> {
        Ok(None)
    }

    fn bytes(value: &Self::Value) -> usize {
        serde_json::to_vec(value).map_or(usize::MAX, |encoded| encoded.len() + 64)
    }
}

/// Every property id `tenant` defines at `view`, typed or not.
fn property_ids(view: &ReadView, tenant: TenantId) -> Result<Vec<u64>, StorageError> {
    use rushdb_types::keys::key;
    view.range_prefix(&key(tenant, &[b"property-row", b""]))?
        .into_iter()
        .map(|(_, raw)| {
            serde_json::from_slice::<rushdb_types::occurrence::PropertyDefinition>(&raw)
                .map(|definition| definition.property_id)
                .map_err(|error| {
                    StorageError::InvalidRead(format!("corrupt property definition: {error}"))
                })
        })
        .collect()
}
