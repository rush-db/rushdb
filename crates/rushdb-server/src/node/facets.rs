//! Facet reads over the occurrence layout. A filtered facet resolves its
//! match set to record ordinals once (no record JSON), then counts labels
//! from record headers and property statistics or values from each matched
//! owner's occurrence rows. Cost is linear in the match set; memory is the
//! ordinal list plus the facet tallies, both charged to the query budget.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::reconstruct::{decode_value, is_empty_array_marker};
use rushdb_query::SearchRequest;
use rushdb_types::ids::OwnerKind;

use super::{PropertyScanStats, RequestScope};

/// Distinct values of one property over a match set, in first-seen order,
/// each with the number of matched records carrying it.
pub(crate) type FacetValues = Vec<(serde_json::Value, u64)>;

impl RequestScope<'_> {
    /// Authorized match-set ordinals for `search` in `view`. The working set
    /// (candidate and matched ordinals) is charged to `budget`, or to the
    /// configured per-query cap when the caller has none.
    pub(crate) fn facet_ordinals(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        search: &SearchRequest,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u64>, String> {
        let search = self.authorize_search_in_view(view.as_ref(), search)?;
        rushdb_query::matching_ordinals(&self.lmpg(), &search, is_cancelled, Arc::clone(view))
            .map_err(|e| e.to_string())
    }

    /// Record count per label over `ords`: one header read per ordinal.
    pub(crate) fn facet_labels(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<BTreeMap<String, u64>, String> {
        let tenant = self.tenant();
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        for &ord in ords {
            if is_cancelled() {
                return Err("query cancelled".into());
            }
            if let Some(header) =
                access::record_by_ord(view, tenant, ord).map_err(|e| e.to_string())?
            {
                *counts.entry(header.label).or_default() += 1;
            }
        }
        Ok(counts)
    }

    /// Per-`(label, pid)` support, array shape, samples and range over
    /// `ords`, from occurrence rows. The pid fixes the definition, so typed
    /// siblings (`int64` vs `float64`) are attributed exactly.
    pub(super) fn facet_property_stats(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        known: &std::collections::HashSet<u64>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<BTreeMap<(String, u64), PropertyScanStats>, String> {
        let tenant = self.tenant();
        let mut stats: BTreeMap<(String, u64), PropertyScanStats> = BTreeMap::new();
        for &ord in ords {
            if is_cancelled() {
                return Err("query cancelled".into());
            }
            let Some(header) =
                access::record_by_ord(view, tenant, ord).map_err(|e| e.to_string())?
            else {
                continue;
            };
            let occurrences = access::occurrences_of_owner(view, tenant, OwnerKind::Record, ord)
                .map_err(|e| e.to_string())?;
            for rows in occurrences.chunk_by(|a, b| a.pid == b.pid) {
                let pid = rows[0].pid;
                if !known.contains(&pid) {
                    continue;
                }
                let entry = stats.entry((header.label.clone(), pid)).or_default();
                entry.records_count += 1;
                if is_empty_array_marker(rows) {
                    entry.is_array = true;
                    continue;
                }
                entry.is_array |= rows.iter().all(|row| row.array);
                for row in rows {
                    entry.observe(
                        &decode_value(row.value, &row.payload).map_err(|e| e.to_string())?,
                    );
                }
            }
        }
        Ok(stats)
    }

    /// Distinct values of `property` over `ords`, as the records response
    /// would show them: arrays are one value, datetimes render as strings,
    /// and a property `access` hides for a record's label is absent.
    /// `property` may be spelled as the display name or the normalized name;
    /// the family is found by its normalized signature and the policy is
    /// checked against the family's display name. Distinct values are
    /// charged to `meter`. With `search`, only matching values are kept, so
    /// the working set holds matches only.
    pub(crate) fn facet_property_values(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        property: &str,
        search: Option<&rushdb_types::value_match::ValueMatcher>,
        access: &rushdb_lmpg::CatalogAccess,
        meter: &mut super::working_set::WorkingSet<'_>,
    ) -> Result<FacetValues, String> {
        // The distinct-value map is query working set: `query-memory-max`
        // caps it, not the request's response budget, tightened
        // by the request. The values stay charged while the caller
        // holds them.
        let mut index: HashMap<Vec<u8>, usize> = HashMap::new();
        let mut values: FacetValues = Vec::new();
        self.for_each_facet_value(
            view,
            ords,
            property,
            access,
            meter,
            &mut |_, value, meter| {
                if search.is_some_and(|search| !search.matches(&value)) {
                    return Ok(());
                }
                let fingerprint = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
                if let Some(&at) = index.get(&fingerprint) {
                    values[at].1 += 1;
                    return Ok(());
                }
                // Key, stored value and slot: roughly twice the encoded size.
                meter.charge(super::working_set::value_bytes(fingerprint.len()))?;
                index.insert(fingerprint, values.len());
                values.push((value, 1));
                Ok(())
            },
        )?;
        Ok(values)
    }

    /// Owner counts of the distinct values of `property` over `ords`, as
    /// [`Self::facet_property_values`] counts them, holding a 32-byte
    /// SHA-256 digest per distinct value instead of the value: a column of
    /// long texts is counted in bounded memory. Returns `(digest, count,
    /// first owner)` in first-seen order; [`Self::facet_values_of`] reads
    /// back the values of the owners a caller keeps.
    pub(crate) fn facet_value_digests(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        property: &str,
        access: &rushdb_lmpg::CatalogAccess,
        meter: &mut super::working_set::WorkingSet<'_>,
    ) -> Result<Vec<([u8; 32], u64, u64)>, String> {
        use sha2::Digest as _;
        let mut index: HashMap<[u8; 32], usize> = HashMap::new();
        let mut counts: Vec<([u8; 32], u64, u64)> = Vec::new();
        self.for_each_facet_value(
            view,
            ords,
            property,
            access,
            meter,
            &mut |ord, value, meter| {
                let fingerprint = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
                let digest: [u8; 32] = sha2::Sha256::digest(&fingerprint).into();
                if let Some(&at) = index.get(&digest) {
                    counts[at].1 += 1;
                    return Ok(());
                }
                meter.charge(super::working_set::value_bytes(digest.len() + 16))?;
                index.insert(digest, counts.len());
                counts.push((digest, 1, ord));
                Ok(())
            },
        )?;
        Ok(counts)
    }

    /// The value of `property` on each of `ords`, in order, as
    /// [`Self::facet_property_values`] renders it (owners without one are
    /// skipped).
    pub(crate) fn facet_values_of(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        property: &str,
        access: &rushdb_lmpg::CatalogAccess,
        meter: &mut super::working_set::WorkingSet<'_>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let mut values = Vec::with_capacity(ords.len());
        self.for_each_facet_value(view, ords, property, access, meter, &mut |_, value, _| {
            values.push(value);
            Ok(())
        })?;
        Ok(values)
    }

    /// Calls `visit` with each owner in `ords` holding `property` and its
    /// value, rendered as the records response shows it.
    fn for_each_facet_value(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        property: &str,
        access: &rushdb_lmpg::CatalogAccess,
        meter: &mut super::working_set::WorkingSet<'_>,
        visit: &mut dyn FnMut(
            u64,
            serde_json::Value,
            &mut super::working_set::WorkingSet<'_>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        let tenant = self.tenant();
        let norm = rushdb_types::value::normalize_name(property);
        // Record reconstruction shows every definition of a family (typed or
        // null-only) under the family's display name; when one record holds
        // several, the highest pid wins.
        let sig = rushdb_lmpg::reader::keys_ext::family_sig_key(
            tenant,
            "default",
            OwnerKind::Record,
            &norm,
        );
        let mut pids: Vec<u64> = Vec::new();
        // A family can hold a datetime definition beside others (a string
        // `CreationDate` on one label, a datetime on another), so values
        // render by the definition they were read from, not by the
        // tenant-wide type of the name.
        let mut datetime_pids: Vec<u64> = Vec::new();
        let mut display = property.to_string();
        if let Some(raw) = view.get(&sig).map_err(|e| e.to_string())? {
            let fid = u64::from_be_bytes(
                raw.as_slice()
                    .try_into()
                    .map_err(|_| "corrupt property family signature".to_string())?,
            );
            if let Some(family) = access::family(view, tenant, fid).map_err(|e| e.to_string())? {
                display = family.display_name;
                for def in
                    access::definitions_of_family(view, tenant, fid).map_err(|e| e.to_string())?
                {
                    if def.type_state
                        == rushdb_types::model::TypeState::Known(
                            rushdb_types::model::LogicalType::DateTime,
                        )
                    {
                        datetime_pids.push(def.property_id);
                    }
                    pids.push(def.property_id);
                }
                pids.sort_unstable();
            }
        }
        for &ord in ords {
            meter.check()?;
            let mut found = None;
            for &pid in &pids {
                let rows = access::occurrences_of_pid(view, tenant, OwnerKind::Record, ord, pid)
                    .map_err(|e| e.to_string())?;
                if !rows.is_empty() {
                    found = Some((pid, rows));
                }
            }
            let Some((pid, rows)) = found else {
                continue;
            };
            let Some(header) =
                access::record_by_ord(view, tenant, ord).map_err(|e| e.to_string())?
            else {
                continue;
            };
            if !access.property(&header.label, &display) {
                continue;
            }
            let value = if is_empty_array_marker(&rows) {
                serde_json::Value::Array(Vec::new())
            } else if rows.iter().all(|row| row.array) {
                serde_json::Value::Array(
                    rows.iter()
                        .map(|row| decode_value(row.value, &row.payload))
                        .collect::<Result<_, _>>()
                        .map_err(|e| e.to_string())?,
                )
            } else {
                let value =
                    decode_value(rows[0].value, &rows[0].payload).map_err(|e| e.to_string())?;
                match value.as_i64() {
                    Some(ms) if datetime_pids.contains(&pid) => {
                        serde_json::Value::String(rushdb_types::datetime::ms_to_datetime_string(ms))
                    }
                    _ => value,
                }
            };
            visit(ord, value, meter)?;
        }
        Ok(())
    }
}

/// Endpoint labels resolved during one relationship facet. Endpoints repeat
/// (many records link to one company), so labels are cached up to a bound;
/// past it they are read again.
const ENDPOINT_LABEL_CACHE: usize = 100_000;

/// `(source label, type, target label)` → relationship count and per-pid
/// counts of relationships carrying a non-null value.
type RelationshipGroups = BTreeMap<(String, String, String), (u64, BTreeMap<u64, u64>)>;

impl RequestScope<'_> {
    /// Relationship shapes over a match set: every relationship with at
    /// least one endpoint in `ords`, counted once, grouped by its endpoint
    /// labels and type, with the properties those relationships carry.
    pub(crate) fn facet_relationships(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<rushdb_protocol::RelationshipDescription>, String> {
        let tenant = self.tenant();
        let matched: roaring::RoaringTreemap = ords.iter().copied().collect();
        let mut labels: HashMap<u64, String> = HashMap::new();
        let mut label_of = |ord: u64| -> Result<String, String> {
            if let Some(label) = labels.get(&ord) {
                return Ok(label.clone());
            }
            let label = access::record_by_ord(view, tenant, ord)
                .map_err(|e| e.to_string())?
                .map(|header| header.label)
                .unwrap_or_default();
            if labels.len() < ENDPOINT_LABEL_CACHE {
                labels.insert(ord, label.clone());
            }
            Ok(label)
        };
        let mut groups: RelationshipGroups = BTreeMap::new();
        for &ord in ords {
            if is_cancelled() {
                return Err("query cancelled".into());
            }
            let here = label_of(ord)?;
            let outgoing = access::adj_out(view, tenant, ord, None).map_err(|e| e.to_string())?;
            let incoming = access::adj_in(view, tenant, ord, None).map_err(|e| e.to_string())?;
            // An edge between two matched records is counted from its source.
            let edges = outgoing
                .into_iter()
                .map(|(rel_type, other, rel)| (rel_type, here.clone(), other, rel, true))
                .chain(
                    incoming
                        .into_iter()
                        .filter(|(_, other, _)| !matched.contains(*other))
                        .map(|(rel_type, other, rel)| (rel_type, here.clone(), other, rel, false)),
                );
            for (rel_type, here, other, rel, outgoing) in edges {
                let there = label_of(other)?;
                let key = if outgoing {
                    (here, rel_type, there)
                } else {
                    (there, rel_type, here)
                };
                let group = groups.entry(key).or_default();
                group.0 += 1;
                let mut carried: Vec<u64> =
                    access::occurrences_of_owner(view, tenant, OwnerKind::Relationship, rel)
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .filter(|occ| {
                            !matches!(
                                occ.value,
                                rushdb_types::occurrence::OccurrenceValue::ExplicitNull
                            )
                        })
                        .map(|occ| occ.pid)
                        .collect();
                carried.dedup();
                for pid in carried {
                    *group.1.entry(pid).or_default() += 1;
                }
            }
        }
        let meta = self.relationship_property_meta_in_view(view)?;
        Ok(groups
            .into_iter()
            .map(
                |((source_label, relationship_type, target_label), (count, pids))| {
                    let mut properties: Vec<rushdb_protocol::RelationshipPropertyDescription> =
                        pids.into_iter()
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
                    rushdb_protocol::RelationshipDescription {
                        source_label,
                        target_label,
                        relationship_type,
                        count,
                        properties,
                    }
                },
            )
            .collect())
    }
}
