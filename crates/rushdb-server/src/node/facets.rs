//! Facet reads over the occurrence layout. A filtered facet resolves its
//! match set to record ordinals once (no record JSON), then counts labels
//! from record headers and property statistics or values from each matched
//! owner's occurrence rows. Cost is linear in the match set; memory is the
//! ordinal list plus the facet tallies, both charged to the query budget.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::reader::TryRead;
use rushdb_lmpg::reconstruct::{decode_value, is_empty_array_marker};
use rushdb_query::SearchRequest;
use rushdb_types::ids::OwnerKind;

use super::{PropertyScanStats, RequestScope};

/// The label of every match when a query names exactly one (matching keeps
/// only records of the named labels).
pub(crate) fn only_label(labels: &[String]) -> Option<&str> {
    match labels {
        [label] => Some(label.as_str()),
        _ => None,
    }
}

/// Property stats of one part of a parallel facet scan, by `(label, pid)`.
type ScanStatsPart = Vec<((String, u64), PropertyScanStats)>;

/// Distinct values with their owner count and first owner, in first-seen
/// order.
pub(crate) type IndexedValues = Vec<(serde_json::Value, u64, u64)>;

/// Matched records one part of a parallel facet scan reads.
const FACET_PART_ORDS: usize = 4096;
/// Parts per round: cancellation is checked between rounds.
const FACET_ROUND_PARTS: usize = 64;

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

    /// The first `keep` authorized match-set ordinals, in match order: the
    /// prefix of [`Self::facet_ordinals`], read without resolving the rest.
    pub(crate) fn facet_ordinals_first(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        search: &SearchRequest,
        keep: usize,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u64>, String> {
        let search = self.authorize_search_in_view(view.as_ref(), search)?;
        rushdb_query::matching_ordinals_first(
            &self.lmpg(),
            &search,
            is_cancelled,
            Arc::clone(view),
            keep,
        )
        .map_err(|e| e.to_string())
    }

    /// Record count per label over `ords`: one header read per ordinal, or
    /// none when the query names one label (every match is of that label).
    pub(crate) fn facet_labels(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        only_label: Option<&str>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<BTreeMap<String, u64>, String> {
        if let Some(label) = only_label {
            let count = u64::try_from(ords.len()).unwrap_or(u64::MAX);
            return Ok((count > 0)
                .then(|| (label.to_owned(), count))
                .into_iter()
                .collect());
        }
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
    /// siblings (`int64` vs `float64`) are attributed exactly. With
    /// `only_label` (the query names one label) no record header is read.
    ///
    /// The scan runs in parts of consecutive ordinals on the shared thread
    /// budget; parts merge in ordinal order, so the result is the one scan's.
    pub(super) fn facet_property_stats(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        known: &std::collections::HashSet<u64>,
        only_label: Option<&str>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<BTreeMap<(String, u64), PropertyScanStats>, String> {
        let parts: Vec<&[u64]> = ords.chunks(FACET_PART_ORDS).collect();
        let mut stats: BTreeMap<(String, u64), PropertyScanStats> = BTreeMap::new();
        for round in parts.chunks(FACET_ROUND_PARTS) {
            if is_cancelled() {
                return Err("query cancelled".into());
            }
            let results = rushdb_storage::parallel::run_parts(round.len(), |part| {
                self.facet_property_stats_part(view, round[part], known, only_label)
            })?;
            for part in results {
                for (key, entry) in part {
                    match stats.entry(key) {
                        std::collections::btree_map::Entry::Vacant(slot) => {
                            slot.insert(entry);
                        }
                        std::collections::btree_map::Entry::Occupied(mut slot) => {
                            slot.get_mut().merge(entry);
                        }
                    }
                }
            }
        }
        Ok(stats)
    }

    fn facet_property_stats_part(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        known: &std::collections::HashSet<u64>,
        only_label: Option<&str>,
    ) -> Result<ScanStatsPart, String> {
        let tenant = self.tenant();
        // Keyed by (label slot, pid) while scanning: a match set of millions
        // of records would otherwise copy its label for every property.
        let mut labels: Vec<String> = only_label.map(str::to_owned).into_iter().collect();
        let mut stats: HashMap<(usize, u64), PropertyScanStats> = HashMap::new();
        for &ord in ords {
            let slot = if only_label.is_some() {
                0
            } else {
                let Some(header) =
                    access::record_by_ord(view, tenant, ord).map_err(|e| e.to_string())?
                else {
                    continue;
                };
                if let Some(slot) = labels.iter().position(|label| *label == header.label) {
                    slot
                } else {
                    labels.push(header.label);
                    labels.len() - 1
                }
            };
            let occurrences = access::occurrences_of_owner(view, tenant, OwnerKind::Record, ord)
                .map_err(|e| e.to_string())?;
            for rows in occurrences.chunk_by(|a, b| a.pid == b.pid) {
                let pid = rows[0].pid;
                if !known.contains(&pid) {
                    continue;
                }
                let entry = stats.entry((slot, pid)).or_default();
                entry.records_count += 1;
                if is_empty_array_marker(rows) {
                    entry.is_array = true;
                    continue;
                }
                entry.is_array |= rows.iter().all(|row| row.array);
                for row in rows {
                    entry.observe(&rushdb_types::types::doc_to_public(
                        decode_value(row.value, &row.payload, &row.suffix)
                            .map_err(|e| e.to_string())?,
                    ));
                }
            }
        }
        Ok(stats
            .into_iter()
            .map(|((slot, pid), entry)| ((labels[slot].clone(), pid), entry))
            .collect())
    }

    /// Samples of each wanted `(label, pid, target)` over the records of
    /// `samples[label]` (ascending ordinals), read through the property's
    /// presence rows instead of every sampled record: values are observed
    /// owner by owner in ordinal order, as a scan of the sample observes
    /// them, until `target` samples are held. An entry is returned when a
    /// sampled record holds the property (a value or only nulls); its
    /// support, shape and range are left for the caller to fill.
    pub(super) fn facet_property_samples(
        &self,
        view: &rushdb_storage::ReadView,
        samples: &BTreeMap<String, Vec<u64>>,
        wanted: &[(String, u64, usize)],
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<BTreeMap<(String, u64), PropertyScanStats>, String> {
        use rushdb_types::keys::{presence_key, NULLNESS_NULL, NULLNESS_VALUE};
        let tenant = self.tenant();
        let mut out = BTreeMap::new();
        for (label, pid, target) in wanted {
            if is_cancelled() {
                return Err("query cancelled".into());
            }
            let Some(ords) = samples.get(label) else {
                continue;
            };
            let (Some(&first), Some(&last)) = (ords.first(), ords.last()) else {
                continue;
            };
            let sampled_owners = |nullness: u8, visit: &mut dyn FnMut(u64) -> bool| {
                let start = presence_key(tenant, OwnerKind::Record, label, *pid, nullness, first);
                let end = presence_key(
                    tenant,
                    OwnerKind::Record,
                    label,
                    *pid,
                    nullness,
                    last.saturating_add(1),
                );
                view.for_each_key(&start, &end, &mut |key| {
                    match rushdb_types::keys::parse_presence_key(key) {
                        Some((_, _, _, _, owner)) if ords.binary_search(&owner).is_ok() => {
                            visit(owner)
                        }
                        _ => true,
                    }
                })
                .map_err(|e| e.to_string())
            };
            let mut entry = PropertyScanStats::default();
            let mut seen = false;
            let mut failed = None;
            sampled_owners(NULLNESS_VALUE, &mut |owner| {
                seen = true;
                let observed =
                    access::occurrences_of_pid(view, tenant, OwnerKind::Record, owner, *pid)
                        .map_err(|e| e.to_string())
                        .and_then(|rows| {
                            rows.iter().try_for_each(|row| {
                                decode_value(row.value, &row.payload, &row.suffix)
                                    .map(|value| {
                                        entry.observe(&rushdb_types::types::doc_to_public(value));
                                    })
                                    .map_err(|e| e.to_string())
                            })
                        });
                if let Err(error) = observed {
                    failed = Some(error);
                    return false;
                }
                entry.samples.len() < *target
            })?;
            if let Some(error) = failed {
                return Err(error);
            }
            if !seen {
                sampled_owners(NULLNESS_NULL, &mut |_| {
                    seen = true;
                    false
                })?;
            }
            if seen {
                entry.records_count = 1;
                out.insert((label.clone(), *pid), entry);
            }
        }
        Ok(out)
    }

    /// Distinct values of `property` over `ords`, as the records response
    /// would show them: an array counts each distinct element once, as the
    /// value dictionary does; datetimes render as strings,
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
                for (_, value) in counted_values(value) {
                    if search.is_some_and(|search| !search.matches(&value)) {
                        continue;
                    }
                    let fingerprint = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
                    if let Some(&at) = index.get(&fingerprint) {
                        values[at].1 += 1;
                        continue;
                    }
                    // Key, stored value and slot: roughly twice the encoded size.
                    meter.charge(super::working_set::value_bytes(fingerprint.len()))?;
                    index.insert(fingerprint, values.len());
                    values.push((value, 1));
                }
                Ok(())
            },
        )?;
        Ok(values)
    }

    /// Owner counts of the distinct values of `property` over `ords`, as
    /// [`Self::facet_property_values`] counts them, holding a 32-byte
    /// SHA-256 digest per distinct value instead of the value: a column of
    /// long texts is counted in bounded memory. Returns `(digest, count,
    /// first owner, element)` in first-seen order, `element` locating an
    /// array element in the owner's value; [`Self::facet_values_of`] reads
    /// back the values of the owners a caller keeps.
    pub(crate) fn facet_value_digests(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        property: &str,
        access: &rushdb_lmpg::CatalogAccess,
        meter: &mut super::working_set::WorkingSet<'_>,
    ) -> Result<Vec<ValueDigest>, String> {
        use sha2::Digest as _;
        let mut index: HashMap<[u8; 32], usize> = HashMap::new();
        let mut counts: Vec<ValueDigest> = Vec::new();
        self.for_each_facet_value(
            view,
            ords,
            property,
            access,
            meter,
            &mut |ord, value, meter| {
                for (element, value) in counted_values(value) {
                    let fingerprint = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
                    let digest: [u8; 32] = sha2::Sha256::digest(&fingerprint).into();
                    if let Some(&at) = index.get(&digest) {
                        counts[at].1 += 1;
                        continue;
                    }
                    meter.charge(super::working_set::value_bytes(digest.len() + 24))?;
                    index.insert(digest, counts.len());
                    counts.push((digest, 1, ord, element));
                }
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

    /// Distinct values of `property` over `ords`, each with its owner count
    /// and first owner, in first-seen order, read in one pass over the
    /// property's ordered indexes instead of each match's occurrence rows
    /// (READ-2). It answers exactly what [`Self::facet_property_values`]
    /// would, so it is used only when the index holds every counted value:
    /// the property has one typed, non-datetime definition (no nulls, no
    /// sibling types), no queried label holds it as an array, a Ready
    /// ordered index covers every queried label, the policy shows it on each,
    /// and `ords` is in ascending order. `None` otherwise, and when the match
    /// set is small next to the index (reading each match is then cheaper).
    pub(crate) fn facet_values_from_index(
        &self,
        view: &rushdb_storage::ReadView,
        ords: &[u64],
        property: &str,
        labels: &[String],
        access: &rushdb_lmpg::CatalogAccess,
        meter: &mut super::working_set::WorkingSet<'_>,
    ) -> Result<Option<IndexedValues>, String> {
        let tenant = self.tenant();
        if labels.is_empty() || !ords.is_sorted() {
            return Ok(None);
        }
        let Some((mut ids, indexed)) = self.facet_indexes(view, property, labels, access)? else {
            return Ok(None);
        };
        // One sequential index entry against two point reads per match.
        if u64::try_from(ords.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(4)
            < indexed
        {
            return Ok(None);
        }
        ids.sort_unstable();
        ids.dedup();
        let mut found: HashMap<Vec<u8>, (u64, u64)> = HashMap::new();
        let mut failed: Option<String> = None;
        for id in ids {
            rushdb_lmpg::index::for_each_entry(view, tenant, id, &[], &[], &mut |entry, owner| {
                if ords.binary_search(&owner).is_err() {
                    return true;
                }
                if let Some((count, first)) = found.get_mut(entry) {
                    *count += 1;
                    *first = (*first).min(owner);
                    return true;
                }
                if let Err(error) = meter.charge(super::working_set::value_bytes(entry.len())) {
                    failed = Some(error);
                    return false;
                }
                found.insert(entry.to_vec(), (1, owner));
                true
            })
            .map_err(|e| e.to_string())?;
            if let Some(error) = failed {
                return Err(error);
            }
        }
        let mut values = found
            .into_iter()
            .map(|(entry, (count, first))| {
                let (&tag, payload) = entry
                    .split_first()
                    .ok_or_else(|| "corrupt index entry".to_string())?;
                let ty =
                    rushdb_types::encoding::logical_from_tag(tag).map_err(|e| e.to_string())?;
                let scalar = rushdb_types::encoding::decode_order_preserving(ty, payload)
                    .map_err(|e| e.to_string())?;
                Ok((rushdb_types::value::scalar_to_json(&scalar), count, first))
            })
            .collect::<Result<Vec<_>, String>>()?;
        // A scan of the ascending match set meets each value first at its
        // lowest owner.
        values.sort_by_key(|(_, _, first)| *first);
        Ok(Some(values))
    }

    /// The ordered indexes that hold every value of `property` on `labels`
    /// (see [`Self::facet_values_from_index`]) and how many entries they
    /// hold, or `None` when they cannot answer it exactly.
    fn facet_indexes(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        labels: &[String],
        access: &rushdb_lmpg::CatalogAccess,
    ) -> Result<Option<(Vec<u32>, u64)>, String> {
        use rushdb_types::model::{LogicalType, TypeState};
        let tenant = self.tenant();
        let norm = rushdb_types::value::normalize_name(property);
        let sig = rushdb_lmpg::reader::keys_ext::family_sig_key(
            tenant,
            "default",
            OwnerKind::Record,
            &norm,
        );
        let Some(raw) = view.get(&sig).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let fid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| "corrupt property family signature".to_string())?,
        );
        let Some(family) = access::family(view, tenant, fid).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let definitions =
            access::definitions_of_family(view, tenant, fid).map_err(|e| e.to_string())?;
        let [definition] = definitions.as_slice() else {
            return Ok(None);
        };
        let TypeState::Known(ty) = definition.type_state else {
            return Ok(None);
        };
        // Index entries keep no datetime offset: a scan answers, so each
        // value shows the offset it was written with.
        if ty == LogicalType::DateTime {
            return Ok(None);
        }
        let pid = definition.property_id;
        let mut ids = Vec::new();
        let mut indexed: u64 = 0;
        for label in labels {
            if !access.property(label, &family.display_name) {
                return Ok(None);
            }
            let marker =
                rushdb_types::keys::array_label_marker_key(tenant, OwnerKind::Record, pid, label);
            let arrays = rushdb_types::keys::decode_counter(
                view.get(&marker).map_err(|e| e.to_string())?.as_deref(),
            )
            .unwrap_or(0);
            if arrays > 0 {
                return Ok(None);
            }
            let Some(def) = rushdb_lmpg::index::ready_ordered(view, tenant, label, &norm)
                .map_err(|e| e.to_string())?
            else {
                return Ok(None);
            };
            ids.push(def.id);
            indexed += rushdb_lmpg::derived::prop_support_label(
                view,
                tenant,
                OwnerKind::Record,
                pid,
                label,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(Some((ids, indexed)))
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
            let Some((_, rows)) = found else {
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
                        .map(|row| decode_value(row.value, &row.payload, &row.suffix))
                        .collect::<Result<_, _>>()
                        .map_err(|e| e.to_string())?,
                )
            } else {
                decode_value(rows[0].value, &rows[0].payload, &rows[0].suffix)
                    .map_err(|e| e.to_string())?
            };
            let value = rushdb_types::types::doc_to_public(value);
            visit(ord, value, meter)?;
        }
        Ok(())
    }
}

/// `(digest, owner count, first owner, array element)` of one distinct value.
pub(crate) type ValueDigest = ([u8; 32], u64, u64, Option<usize>);

/// The values an owner's `value` counts toward, as the value dictionary
/// counts them: a scalar is one value; an array counts each distinct element
/// once (with its position), and an empty array none.
fn counted_values(value: serde_json::Value) -> Vec<(Option<usize>, serde_json::Value)> {
    match value {
        serde_json::Value::Array(items) => {
            let mut seen = std::collections::HashSet::new();
            items
                .into_iter()
                .enumerate()
                .filter(|(_, item)| seen.insert(item.to_string()))
                .map(|(at, item)| (Some(at), item))
                .collect()
        }
        value => vec![(None, value)],
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
                .map_or_default(|header| header.label);
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
