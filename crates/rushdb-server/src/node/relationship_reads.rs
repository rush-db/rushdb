//! Node relationship reads: the paged relationship listing (by endpoint
//! predicates or a streamed scan) and the relationship schema description.

use super::{
    access, schema_json_type, type_name_str, Arc, BTreeMap, OwnerKind, RelationshipAggregate,
    RequestScope,
};

impl RequestScope<'_> {
    /// Scan relationships with endpoint, type, owned-property, and pagination
    /// predicates.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn scan_relationships(
        &self,
        query: &rushdb_protocol::RelationshipQuery,
    ) -> Result<serde_json::Value, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.scan_relationships_in_view(query, &snap)
    }

    /// Scan relationships using a caller-owned pinned execution view.
    pub(crate) fn scan_relationships_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::RelationshipQuery,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.scan_relationships_in_view(query, context.read_view())?;
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub(super) fn scan_relationships_in_view(
        &self,
        query: &rushdb_protocol::RelationshipQuery,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<serde_json::Value, String> {
        let type_filter = query.relation_type.as_ref();
        let predicate = match &query.predicate {
            serde_json::Value::Null => None,
            serde_json::Value::Object(map) if map.is_empty() => None,
            where_ => {
                Some(rushdb_query::parse_property_predicate(where_).map_err(|e| e.to_string())?)
            }
        };
        let limit = query.limit.unwrap_or(10_000);
        let skip = query.offset;

        let tenant = self.tenant();
        let active_policy = rushdb_authz::load_active_policy_in_view(snap, tenant)
            .map_err(|error| error.to_string())?;
        let authz = rushdb_lmpg::AuthorizedView::new(snap, tenant, active_policy.as_ref());

        // One relationship against every filter: its row when it matches,
        // reconstructed only when the predicate needs it or it is on the page.
        let mut total = 0u64;
        let mut rows = Vec::new();
        let mut consider =
            |hdr: rushdb_types::occurrence::RelationshipHeader| -> Result<(), String> {
                if type_filter.is_some_and(|t| hdr.rel_type != *t)
                    || !authz.rel_type_allowed(&hdr.rel_type)
                {
                    return Ok(());
                }
                let src_hdr = access::record_by_ord(snap, tenant, hdr.source_ord)
                    .map_err(|e| e.to_string())?
                    .ok_or("corrupt relationship: missing source")?;
                let tgt_hdr = access::record_by_ord(snap, tenant, hdr.target_ord)
                    .map_err(|e| e.to_string())?
                    .ok_or("corrupt relationship: missing target")?;
                let label_ok = |wanted: &[String], label: &String| {
                    wanted.is_empty() || wanted.iter().any(|l| l == label)
                };
                if !label_ok(&query.source_labels, &src_hdr.label)
                    || !label_ok(&query.target_labels, &tgt_hdr.label)
                    || !authz.label_allowed(&src_hdr.label)
                    || !authz.label_allowed(&tgt_hdr.label)
                {
                    return Ok(());
                }
                let on_page = total >= skip && (rows.len() as u64) < limit;
                if predicate.is_none() && !on_page {
                    total += 1;
                    return Ok(());
                }
                let mut rel_obj =
                    rushdb_lmpg::reconstruct::reconstruct_relationship(snap, tenant, hdr.ordinal)
                        .map_err(|e| e.to_string())?
                        .and_then(|v| v.as_object().cloned())
                        .ok_or("corrupt relationship: reconstruction missing")?;
                if let Some(predicate) = &predicate {
                    if !rushdb_query::matches_properties(predicate, &rel_obj)
                        .map_err(|e| e.to_string())?
                    {
                        return Ok(());
                    }
                }
                total += 1;
                if !on_page {
                    return Ok(());
                }
                rel_obj.retain(|name, _| !rushdb_types::value::is_reserved_name(name));
                rows.push(serde_json::json!({
                    "id": uuid::Uuid::from_bytes(hdr.relationship_id).to_string(),
                    "type": hdr.rel_type,
                    "sourceId": uuid::Uuid::from_bytes(hdr.source_id).to_string(),
                    "targetId": uuid::Uuid::from_bytes(hdr.target_id).to_string(),
                    "sourceLabel": src_hdr.label,
                    "targetLabel": tgt_hdr.label,
                    "properties": rel_obj,
                }));
                Ok(())
            };

        if let Some(headers) = self.endpoint_relationships(query, snap)? {
            // Endpoint filters: only the matching records' own links.
            for hdr in headers {
                consider(hdr)?;
            }
        } else {
            for_each_relationship(snap, tenant, &mut consider)?;
        }
        // `total` is the number of relationships matching the filters before
        // pagination (the page is `data`).
        Ok(serde_json::json!({ "data": rows, "total": total }))
    }

    /// The relationships whose source and/or target satisfy the query's
    /// endpoint predicates, found through those records' adjacency, in
    /// creation order; `None` when the query has no endpoint predicate.
    fn endpoint_relationships(
        &self,
        query: &rushdb_protocol::RelationshipQuery,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<Option<Vec<rushdb_types::occurrence::RelationshipHeader>>, String> {
        let tenant = self.tenant();
        let rel_type = query.relation_type.as_deref();
        let ends = |predicate: &Option<serde_json::Value>,
                    labels: &[String],
                    outgoing: bool|
         -> Result<Option<std::collections::BTreeSet<u64>>, String> {
            let Some(predicate) = predicate else {
                return Ok(None);
            };
            let document = serde_json::json!({ "labels": labels, "where": predicate });
            let parsed =
                rushdb_query::document::parse_document(&document).map_err(|e| e.to_string())?;
            let records = rushdb_query::matching_ordinals(
                &self.lmpg(),
                &parsed.search,
                &|| false,
                Arc::clone(snap),
            )
            .map_err(|e| e.to_string())?;
            let mut rels = std::collections::BTreeSet::new();
            for ord in records {
                let edges = if outgoing {
                    access::adj_out(snap, tenant, ord, rel_type)
                } else {
                    access::adj_in(snap, tenant, ord, rel_type)
                }
                .map_err(|e| e.to_string())?;
                rels.extend(edges.into_iter().map(|(_, _, rel)| rel));
            }
            Ok(Some(rels))
        };
        let sources = ends(&query.source_predicate, &query.source_labels, true)?;
        let targets = ends(&query.target_predicate, &query.target_labels, false)?;
        let rels = match (sources, targets) {
            (None, None) => return Ok(None),
            (Some(rels), None) | (None, Some(rels)) => rels,
            (Some(sources), Some(targets)) => sources.intersection(&targets).copied().collect(),
        };
        let mut headers = Vec::with_capacity(rels.len());
        for rel in rels {
            if let Some(hdr) =
                access::relationship_by_ord(snap, tenant, rel).map_err(|e| e.to_string())?
            {
                headers.push(hdr);
            }
        }
        headers.sort_by_key(|hdr| hdr.relationship_id);
        Ok(Some(headers))
    }

    /// Exact relationship topology for the current request scope.
    ///
    /// Unlike the relationship listing operation this is not page-limited: schema
    /// cardinalities describe the complete scoped graph. Concrete edges are reduced
    /// immediately into label/type triples, so the protocol response remains small.
    /// Catalog-backed relationship description: group counts and per-type
    /// property owner counts from maintained rows, with a scan fallback for
    /// data written before the relationship catalog existed.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn describe_relationships(
        &self,
    ) -> Result<Vec<rushdb_protocol::RelationshipDescription>, String> {
        let view = std::sync::Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.describe_relationships_in_view(&view)
    }

    /// Describe relationship topology from a caller-owned pinned snapshot.
    pub(crate) fn describe_relationships_in_view(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
    ) -> Result<Vec<rushdb_protocol::RelationshipDescription>, String> {
        let catalog = rushdb_lmpg::reader::try_access::relationship_catalog(view, self.tenant())
            .map_err(|e| e.to_string())?;
        if catalog.is_empty() {
            return self.describe_relationships_scan_in_view(view);
        }
        let meta = self.relationship_property_meta_in_view(view)?;
        let mut out: Vec<rushdb_protocol::RelationshipDescription> =
            Vec::with_capacity(catalog.len());
        for (relationship_type, source_label, target_label, count) in catalog {
            let mut properties: Vec<rushdb_protocol::RelationshipPropertyDescription> =
                rushdb_lmpg::reader::try_access::rel_prop_counts(
                    view,
                    self.tenant(),
                    (&relationship_type, &source_label, &target_label),
                )
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter_map(|(pid, relationships_count)| {
                    meta.get(&pid).map(|(name, value_type)| {
                        rushdb_protocol::RelationshipPropertyDescription {
                            name: name.clone(),
                            value_type: value_type.clone(),
                            relationships_count,
                        }
                    })
                })
                .collect();
            properties.sort_by(|a, b| a.name.cmp(&b.name));
            out.push(rushdb_protocol::RelationshipDescription {
                source_label,
                target_label,
                relationship_type,
                count,
                properties,
            });
        }
        out.sort_by(|a, b| {
            (&a.source_label, &a.relationship_type, &a.target_label).cmp(&(
                &b.source_label,
                &b.relationship_type,
                &b.target_label,
            ))
        });
        Ok(out)
    }

    /// Map of relationship property id -> (display name, type), from families
    /// scoped to relationships.
    pub(super) fn relationship_property_meta_in_view(
        &self,
        view: &rushdb_storage::ReadView,
    ) -> Result<BTreeMap<u64, (String, String)>, String> {
        use rushdb_types::keys::{key, P_FAMILY};
        let tenant = self.tenant();
        let mut families = BTreeMap::new();
        for (_, raw) in view
            .range_prefix(&key(tenant, &[P_FAMILY, b""]))
            .map_err(|e| e.to_string())?
        {
            let fam: rushdb_types::occurrence::PropertyFamily = serde_json::from_slice(&raw)
                .map_err(|e| format!("corrupt property family: {e}"))?;
            families.insert(fam.family_id, fam);
        }
        let mut out = BTreeMap::new();
        for (_, raw) in view
            .range_prefix(&key(tenant, &[b"property-row", b""]))
            .map_err(|e| e.to_string())?
        {
            let def: rushdb_types::occurrence::PropertyDefinition = serde_json::from_slice(&raw)
                .map_err(|e| format!("corrupt property definition: {e}"))?;
            let Some(fam) = families.get(&def.family_id) else {
                continue;
            };
            if fam.namespace != "default" || fam.owner_scope != OwnerKind::Relationship {
                continue;
            }
            if let rushdb_types::model::TypeState::Known(ty) = def.type_state {
                out.insert(
                    def.property_id,
                    (fam.display_name.clone(), type_name_str(ty)),
                );
            }
        }
        Ok(out)
    }

    pub(super) fn describe_relationships_scan_in_view(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
    ) -> Result<Vec<rushdb_protocol::RelationshipDescription>, String> {
        let result = self.scan_relationships_in_view(
            &rushdb_protocol::RelationshipQuery {
                limit: Some(u64::MAX),
                ..Default::default()
            },
            view,
        )?;
        let rows = result
            .get("data")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut groups = BTreeMap::<(String, String, String), RelationshipAggregate>::new();

        for row in rows {
            let Some(source_label) = row.get("sourceLabel").and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let Some(target_label) = row.get("targetLabel").and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let Some(relationship_type) = row.get("type").and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let group = groups
                .entry((
                    source_label.to_string(),
                    relationship_type.to_string(),
                    target_label.to_string(),
                ))
                .or_default();
            group.0 += 1;
            if let Some(properties) = row.get("properties").and_then(serde_json::Value::as_object) {
                for (name, value) in properties {
                    if value.is_null() {
                        continue;
                    }
                    let observed_type = schema_json_type(value);
                    let property = group
                        .1
                        .entry(name.clone())
                        .or_insert_with(|| (observed_type.clone(), 0));
                    if property.0 != observed_type {
                        property.0 = "mixed".to_string();
                    }
                    property.1 += 1;
                }
            }
        }

        Ok(groups
            .into_iter()
            .map(
                |((source_label, relationship_type, target_label), (count, properties))| {
                    rushdb_protocol::RelationshipDescription {
                        source_label,
                        target_label,
                        relationship_type,
                        count,
                        properties: properties
                            .into_iter()
                            .map(|(name, (value_type, relationships_count))| {
                                rushdb_protocol::RelationshipPropertyDescription {
                                    name,
                                    value_type,
                                    relationships_count,
                                }
                            })
                            .collect(),
                    }
                },
            )
            .collect())
    }

    #[cfg(test)]
    pub(crate) fn describe_relationships_scan(
        &self,
    ) -> Result<Vec<rushdb_protocol::RelationshipDescription>, String> {
        let view = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.describe_relationships_scan_in_view(&view)
    }
}

/// Every relationship header of the tenant, streamed in creation order.
fn for_each_relationship(
    snap: &rushdb_storage::ReadView,
    tenant: rushdb_types::ids::TenantId,
    visit: &mut dyn FnMut(rushdb_types::occurrence::RelationshipHeader) -> Result<(), String>,
) -> Result<(), String> {
    use rushdb_types::keys::{key, P_REL};
    let prefix = key(tenant, &[P_REL, b""]);
    let mut cursor = snap
        .scan(
            rushdb_storage::ScanBounds::prefix(&prefix),
            rushdb_storage::ScanProjection::KeyValue,
        )
        .map_err(|e| e.to_string())?;
    loop {
        let batch = cursor
            .next_batch(1_024, 4 << 20)
            .map_err(|e| e.to_string())?;
        for row in batch.rows {
            let raw = row.value.unwrap_or_default();
            visit(
                serde_json::from_slice(&raw)
                    .map_err(|e| format!("corrupt relationship header: {e}"))?,
            )?;
        }
        if batch.done {
            return Ok(());
        }
    }
}
