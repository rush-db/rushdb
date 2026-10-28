//! Node catalog operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use super::working_set::{value_bytes, WorkingSet};
use super::{
    access, logical_type_name, paginate_entries, type_name_str, Arc, CachedMetadata, OwnerKind,
    PropertyMetadata, RequestScope, ValueDomainIndex,
};
use rushdb_lmpg::reader::TryRead;
use rushdb_types::value_match::ValueMatcher;

/// A text property's exact distinct count and, when few, its values with owner counts.
type ScannedText = (u64, Vec<(serde_json::Value, u64)>);
/// Records one listing of long text values may read
/// (`long_text_values_in_view`) before it leaves the values out.
const LONG_VALUE_WALK: u64 = 65_536;
impl RequestScope<'_> {
    // The tenant identity is collision-free and derived from the request
    // scope, so the keyspace prefix is the sole isolation boundary. Records
    // never carry a discriminator property; nothing is stamped on write,
    // injected into queries, or stripped from responses. The scope value is
    // only used to derive the tenant id and to match the protocol context.

    // ---- graph ----------------------------------------------------------

    /// Sorted labels present for the current tenant.
    ///
    /// # Errors
    ///
    /// Returns a message when the label catalog cannot be read.
    pub fn labels(&self) -> Result<Vec<String>, String> {
        Ok(self.metadata_cached()?.0.clone())
    }

    /// Read labels from a caller-owned snapshot. Protocol describe paths use
    /// this entry point so catalog metadata cannot switch to a newer commit
    /// after the request context has pinned its view.
    pub(crate) fn labels_in_view(
        &self,
        view: &rushdb_storage::ReadView,
    ) -> Result<Vec<String>, String> {
        let data = self.metadata_cached_in_view(view)?;
        Ok(data.0.clone())
    }

    /// Read exact per-label counts from a caller-owned snapshot.
    pub(crate) fn label_counts_in_view(
        &self,
        view: &rushdb_storage::ReadView,
    ) -> Result<Vec<(String, u64)>, String> {
        let data = self.metadata_cached_in_view(view)?;
        Ok(data.1.clone())
    }

    /// Bounded, commit-index-keyed cache of `(labels, per-label counts)`.
    /// Results are exact for the current commit index; a later commit misses.
    pub(super) fn metadata_cached(&self) -> Result<CachedMetadata, String> {
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        self.metadata_cached_in_view(&view)
    }

    pub(super) fn metadata_cached_in_view(
        &self,
        view: &rushdb_storage::ReadView,
    ) -> Result<CachedMetadata, String> {
        let tenant = self.tenant();
        let ci = view.commit_index();
        let policy_generation = rushdb_authz::load_active_policy_in_view(view, tenant)
            .map_err(|e| e.to_string())?
            .map_or(0, |active| active.generation);
        let cache_key = (tenant, policy_generation);
        {
            let mut cache = self
                .metadata_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(cache) = cache.as_mut() {
                if let Some((cached_ci, data)) = cache.get(&cache_key) {
                    if *cached_ci == ci {
                        return Ok(data.clone());
                    }
                }
            }
        }
        let labels =
            rushdb_lmpg::reader::try_access::labels(view, tenant).map_err(|e| e.to_string())?;
        let counts = rushdb_lmpg::reader::try_access::label_counts(view, tenant)
            .map_err(|e| e.to_string())?;
        let data = std::sync::Arc::new((labels, counts));
        let mut cache = self
            .metadata_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cache) = cache.as_mut() {
            let bytes = data
                .0
                .iter()
                .map(|label| label.len() + 16)
                .chain(data.1.iter().map(|(label, _)| label.len() + 24))
                .sum::<usize>()
                + 64;
            cache.insert(cache_key, (ci, data.clone()), bytes);
        }
        Ok(data)
    }

    /// Per-label live record counts from the maintained label catalog
    /// (`O(labels)`), used by unscoped schema description.
    ///
    /// # Errors
    ///
    /// Returns a message when the label catalog cannot be read.
    pub fn label_counts(&self) -> Result<Vec<(String, u64)>, String> {
        Ok(self.metadata_cached()?.1.clone())
    }

    /// Typed scalar definitions `(type, pid)` of an unscoped record property,
    /// or `None` when no record ever carried the name.
    /// The property's scalar definitions of `types` (all when empty).
    fn scalar_definitions_of(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        types: &[rushdb_types::model::LogicalType],
    ) -> Result<Option<Vec<(rushdb_types::model::LogicalType, u64)>>, String> {
        Ok(self
            .scalar_definitions(view, property)?
            .map(|mut definitions| {
                if !types.is_empty() {
                    definitions.retain(|(ty, _)| types.contains(ty));
                }
                definitions
            }))
    }

    fn scalar_definitions(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
    ) -> Result<Option<Vec<(rushdb_types::model::LogicalType, u64)>>, String> {
        use rushdb_lmpg::reader::keys_ext as kx;
        use rushdb_types::model::{LogicalType, TypeState};
        let t = self.tenant();
        let norm = rushdb_types::value::normalize_name(property);
        let sig = kx::family_sig_key(t, "default", OwnerKind::Record, &norm);
        let Some(raw) = view.get(&sig).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let fid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| "corrupt family signature".to_string())?,
        );
        let mut out = Vec::new();
        for ty in [
            LogicalType::Int64,
            LogicalType::Float64,
            LogicalType::Utf8,
            LogicalType::Boolean,
            LogicalType::DateTime,
        ] {
            let Some(pid_raw) = view
                .get(&kx::definition_key(t, fid, TypeState::Known(ty)))
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            out.push((
                ty,
                u64::from_be_bytes(
                    pid_raw
                        .as_slice()
                        .try_into()
                        .map_err(|_| "corrupt property definition".to_string())?,
                ),
            ));
        }
        Ok(Some(out))
    }

    /// Whether any record under `labels` (all labels when empty) holds an
    /// explicit null for `property`, under any of its definitions, the
    /// untyped one (a null seen before any value) included. Nulls are not
    /// dictionary entries, so a list read from the dictionary would miss them.
    ///
    /// # Errors
    ///
    /// Returns a message when a read fails or a row is corrupt.
    pub(crate) fn explicit_nulls_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        labels: &[String],
    ) -> Result<bool, String> {
        use rushdb_lmpg::reader::keys_ext as kx;
        use rushdb_types::model::{LogicalType, TypeState};
        let t = self.tenant();
        let norm = rushdb_types::value::normalize_name(property);
        let sig = kx::family_sig_key(t, "default", OwnerKind::Record, &norm);
        let Some(raw) = view.get(&sig).map_err(|e| e.to_string())? else {
            return Ok(false);
        };
        let fid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| "corrupt family signature".to_string())?,
        );
        let states = [
            TypeState::Unresolved,
            TypeState::Known(LogicalType::Int64),
            TypeState::Known(LogicalType::Float64),
            TypeState::Known(LogicalType::Utf8),
            TypeState::Known(LogicalType::Boolean),
            TypeState::Known(LogicalType::DateTime),
        ];
        for state in states {
            let Some(pid_raw) = view
                .get(&kx::definition_key(t, fid, state))
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            let pid = u64::from_be_bytes(
                pid_raw
                    .as_slice()
                    .try_into()
                    .map_err(|_| "corrupt property definition".to_string())?,
            );
            if rushdb_lmpg::derived::has_nulls(view, t, OwnerKind::Record, pid, labels)
                .map_err(|e| e.to_string())?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether the value dictionary alone answers `pid` exactly under
    /// `labels` (all labels when empty): no array occurrence (whole-array
    /// semantics differ from element values) and no value too long for the
    /// dictionary.
    fn dictionary_answers(
        &self,
        view: &rushdb_storage::ReadView,
        pid: u64,
        labels: &[String],
    ) -> Result<bool, String> {
        let t = self.tenant();
        let (_, array_labels) =
            rushdb_lmpg::derived::array_markers(view, t, OwnerKind::Record, pid)
                .map_err(|e| e.to_string())?;
        let has_array = array_labels
            .iter()
            .any(|label| labels.is_empty() || labels.contains(label));
        let long = rushdb_lmpg::derived::long_values(view, t, OwnerKind::Record, pid, labels)
            .map_err(|e| e.to_string())?;
        Ok(!has_array && long == 0)
    }

    /// O(1)-per-definition exact distinct count for an unscoped property from
    /// the on-disk counters, or `None` (caller scans) for an
    /// array property or a property with no scalar definition.
    pub(crate) fn distinct_count_from_index_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
    ) -> Result<Option<u64>, String> {
        let Some(definitions) = self.scalar_definitions(view, property)? else {
            return Ok(None);
        };
        let mut total = 0u64;
        for (_, pid) in &definitions {
            let (has_array, _) =
                rushdb_lmpg::derived::array_markers(view, self.tenant(), OwnerKind::Record, *pid)
                    .map_err(|e| e.to_string())?;
            if has_array {
                return Ok(None);
            }
            total = total.saturating_add(
                rushdb_lmpg::derived::distinct_count(view, self.tenant(), OwnerKind::Record, *pid)
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok((!definitions.is_empty()).then_some(total))
    }

    /// Value domain of a property under `labels` (all labels when empty) from
    /// the value dictionary: exact
    /// distinct count, explicit-null presence, and numeric min/max. `None`
    /// (caller scans) when a definition holds arrays or long values, or no
    /// scalar definition exists. `numeric_only` tells the caller whether
    /// `min`/`max` are meaningful. The distinct set is working set while it
    /// is counted (`meter`), released before returning.
    ///
    /// # Errors
    ///
    /// Returns a message when a read fails, a row is corrupt, or the meter
    /// stops the read.
    pub(crate) fn value_domain_from_index_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        labels: &[String],
        meter: &mut WorkingSet<'_>,
    ) -> Result<Option<ValueDomainIndex>, String> {
        self.value_domain_of_types_in_view(view, property, labels, &[], meter)
    }

    /// [`Self::value_domain_from_index_in_view`] over the property's
    /// definitions of `types` only (all when empty): one entry of a type
    /// split reads its own values, not the whole property's.
    #[expect(
        clippy::too_many_lines,
        reason = "one linear catalog scan; splitting would scatter the shared read-view borrows"
    )]
    pub(crate) fn value_domain_of_types_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        labels: &[String],
        types: &[rushdb_types::model::LogicalType],
        meter: &mut WorkingSet<'_>,
    ) -> Result<Option<ValueDomainIndex>, String> {
        use rushdb_types::model::LogicalType;
        use rushdb_types::value::{scalar_cmp, scalar_to_json, Scalar};
        use std::cmp::Ordering;
        let Some(definitions) = self.scalar_definitions_of(view, property, types)? else {
            return Ok(None);
        };
        if definitions.is_empty() {
            return Ok(None);
        }
        let t = self.tenant();
        if let [label] = labels {
            if let Some(domain) = self.label_domain_in_view(view, label, &definitions)? {
                return Ok(Some(domain));
            }
        }
        let mut distinct = std::collections::BTreeSet::<Vec<u8>>::new();
        let mut held = 0u64;
        let mut numeric_only = true;
        let mut any_numeric = false;
        let mut null_present = false;
        let mut min: Option<Scalar> = None;
        let mut max: Option<Scalar> = None;
        let mut outcome: Result<bool, String> = Ok(true);
        for (ty, pid) in definitions {
            // A definition with no owner under `labels` says nothing about
            // them: a name stored as text on one label must not hide the
            // numeric range of another.
            if !labels.is_empty()
                && !access::prop_support_label_counts(view, t, pid)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .any(|(label, count)| count > 0 && labels.contains(&label))
            {
                continue;
            }
            if rushdb_lmpg::derived::has_nulls(view, t, OwnerKind::Record, pid, labels)
                .map_err(|e| e.to_string())?
            {
                null_present = true;
            }
            if !self.dictionary_answers(view, pid, labels)? {
                outcome = Ok(false);
                break;
            }
            let numeric = matches!(ty, LogicalType::Int64 | LogicalType::Float64);
            if numeric {
                any_numeric = true;
            } else {
                numeric_only = false;
            }
            let read = rushdb_lmpg::derived::for_each_dictionary_entry(
                view,
                t,
                OwnerKind::Record,
                pid,
                labels,
                &mut |entry, _| {
                    let Some(payload) = entry.get(1..) else {
                        outcome = Err("corrupt value dictionary entry".into());
                        return false;
                    };
                    if !distinct.contains(payload) {
                        let bytes = value_bytes(payload.len());
                        if let Err(error) = meter.charge(bytes) {
                            outcome = Err(error);
                            return false;
                        }
                        held += bytes;
                        distinct.insert(payload.to_vec());
                    }
                    if numeric {
                        if let Ok(scalar) =
                            rushdb_types::encoding::decode_order_preserving(ty, payload)
                        {
                            // Incomparable values replace, as before.
                            let replaces = |held: Option<&Scalar>, beaten: Ordering| {
                                held.is_none_or(|m| {
                                    scalar_cmp(m, &scalar).is_none_or(|order| order == beaten)
                                })
                            };
                            if replaces(min.as_ref(), Ordering::Greater) {
                                min = Some(scalar.clone());
                            }
                            if replaces(max.as_ref(), Ordering::Less) {
                                max = Some(scalar);
                            }
                        }
                    }
                    true
                },
            );
            if let Err(error) = read {
                outcome = Err(error.to_string());
            }
            if outcome.is_err() {
                break;
            }
        }
        meter.release(held);
        if !outcome? {
            return Ok(None);
        }
        Ok(Some(ValueDomainIndex {
            distinct: distinct.len() as u64,
            null_present,
            min: min.map(|scalar| scalar_to_json(&scalar)),
            max: max.map(|scalar| scalar_to_json(&scalar)),
            numeric_only: numeric_only && any_numeric,
        }))
    }

    /// The value domain under one `label` when exactly one of `definitions`
    /// has owners there and the dictionary answers for it: the distinct
    /// count from its counter and a numeric range from the dictionary's
    /// first and last entries, so the read does not grow with the number
    /// of distinct values. `None` sends the caller to the dictionary walk.
    fn label_domain_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        label: &str,
        definitions: &[(rushdb_types::model::LogicalType, u64)],
    ) -> Result<Option<ValueDomainIndex>, String> {
        use rushdb_types::model::LogicalType;
        let t = self.tenant();
        let mut held = Vec::new();
        for &(ty, pid) in definitions {
            if access::prop_support_label_counts(view, t, pid)
                .map_err(|e| e.to_string())?
                .into_iter()
                .any(|(owner, count)| count > 0 && owner == label)
            {
                held.push((ty, pid));
            }
        }
        let [(ty, pid)] = held[..] else {
            return Ok(None);
        };
        let labels = [label.to_string()];
        if !self.dictionary_answers(view, pid, &labels)? {
            return Ok(None);
        }
        let derived = |e: rushdb_storage::StorageError| e.to_string();
        let null_present =
            rushdb_lmpg::derived::has_nulls(view, t, OwnerKind::Record, pid, &labels)
                .map_err(derived)?;
        let distinct = rushdb_lmpg::derived::label_distinct(view, t, OwnerKind::Record, label, pid)
            .map_err(derived)?;
        let numeric = matches!(ty, LogicalType::Int64 | LogicalType::Float64);
        let (mut min, mut max) = (None, None);
        if numeric {
            if let Some((first, last)) =
                rushdb_lmpg::derived::dictionary_bounds(view, t, OwnerKind::Record, label, pid)
                    .map_err(derived)?
            {
                let decode = |entry: &[u8]| -> Result<serde_json::Value, String> {
                    let payload = entry.get(1..).ok_or("corrupt value dictionary entry")?;
                    rushdb_types::encoding::decode_order_preserving(ty, payload)
                        .map(|scalar| rushdb_types::value::scalar_to_json(&scalar))
                        .map_err(|e| e.to_string())
                };
                min = Some(decode(&first)?);
                max = Some(decode(&last)?);
            }
        }
        Ok(Some(ValueDomainIndex {
            distinct,
            null_present,
            min,
            max,
            numeric_only: numeric,
        }))
    }

    /// Exact top-k `(value, owner count)` pairs under `labels` (all labels
    /// when empty) from the value dictionary's
    /// refcounts (C7 `TopK`, DC10). `None` (caller counts during the scan)
    /// when a definition holds arrays or long values, or no scalar definition
    /// exists.
    pub(crate) fn value_domain_top_k_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        k: usize,
        labels: &[String],
        meter: &mut WorkingSet<'_>,
    ) -> Result<Option<Vec<(serde_json::Value, u64)>>, String> {
        if let [label] = labels {
            if let Some(head) =
                self.label_dictionary_head_in_view(view, property, label, k, meter)?
            {
                return Ok(Some(head));
            }
        }
        Ok(self
            .value_dictionary_in_view(view, property, labels, None, meter)?
            .map(|mut out| {
                for (value, _) in out.drain(k.min(out.len())..) {
                    let encoded = serde_json::to_vec(&value).map_or(0, |bytes| bytes.len());
                    meter.release(value_bytes(encoded));
                }
                out
            }))
    }

    /// The best `k` `(value, owner count)` pairs of one label's dictionary,
    /// ranked as [`Self::value_dictionary_in_view`] ranks (descending count,
    /// ties by the value's JSON text), when exactly one definition of
    /// `property` has owners under `label` and the dictionary answers for
    /// it. Only `k` values are held while the dictionary streams, so a
    /// column of unique values fits the working set. `None` sends the caller
    /// to the full dictionary read.
    ///
    /// # Errors
    ///
    /// Returns a message when a read fails, a row is corrupt, or the meter
    /// stops the read.
    fn label_dictionary_head_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        label: &str,
        k: usize,
        meter: &mut WorkingSet<'_>,
    ) -> Result<Option<Vec<(serde_json::Value, u64)>>, String> {
        use std::cmp::{Ordering, Reverse};
        use std::collections::BinaryHeap;
        // Heap order: the greatest entry is the worst kept one.
        struct Ranked(u64, String, serde_json::Value);
        impl PartialEq for Ranked {
            fn eq(&self, other: &Self) -> bool {
                self.cmp(other) == Ordering::Equal
            }
        }
        impl Eq for Ranked {}
        impl PartialOrd for Ranked {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Ranked {
            fn cmp(&self, other: &Self) -> Ordering {
                Reverse(self.0)
                    .cmp(&Reverse(other.0))
                    .then_with(|| self.1.cmp(&other.1))
            }
        }
        let Some(definitions) = self.scalar_definitions_of(view, property, &[])? else {
            return Ok(None);
        };
        let t = self.tenant();
        let mut held_by_label = Vec::new();
        for (ty, pid) in definitions {
            if access::prop_support_label_counts(view, t, pid)
                .map_err(|e| e.to_string())?
                .into_iter()
                .any(|(owner, count)| count > 0 && owner == label)
            {
                held_by_label.push((ty, pid));
            }
        }
        let [(ty, pid)] = held_by_label[..] else {
            return Ok(None);
        };
        let labels = [label.to_string()];
        if !self.dictionary_answers(view, pid, &labels)? {
            return Ok(None);
        }
        let mut heap: BinaryHeap<Ranked> = BinaryHeap::with_capacity(k.saturating_add(1));
        let mut held = 0u64;
        let mut outcome: Result<(), String> = Ok(());
        let read = rushdb_lmpg::derived::for_each_dictionary_entry(
            view,
            t,
            OwnerKind::Record,
            pid,
            &labels,
            &mut |entry, owner_count| {
                if k == 0 {
                    return false;
                }
                // A count below the worst kept one cannot enter a full head.
                if heap.len() == k && heap.peek().is_some_and(|worst| owner_count < worst.0) {
                    return true;
                }
                let Some(value) = entry
                    .get(1..)
                    .and_then(|payload| dictionary_value(ty, payload))
                else {
                    return true;
                };
                let text = serde_json::to_string(&value).unwrap_or_default();
                let bytes = value_bytes(text.len());
                if let Err(error) = meter.charge(bytes) {
                    outcome = Err(error);
                    return false;
                }
                held += bytes;
                heap.push(Ranked(owner_count, text, value));
                if heap.len() > k {
                    if let Some(dropped) = heap.pop() {
                        let bytes = value_bytes(dropped.1.len());
                        meter.release(bytes);
                        held -= bytes;
                    }
                }
                true
            },
        );
        if let Err(error) = read.map_err(|e| e.to_string()).and(outcome) {
            meter.release(held);
            return Err(error);
        }
        // Kept values stay charged while the caller holds them, as the full
        // dictionary read leaves them.
        Ok(Some(
            heap.into_sorted_vec()
                .into_iter()
                .map(|Ranked(count, _, value)| (value, count))
                .collect(),
        ))
    }

    /// Every distinct `(value, owner count)` pair under `labels` (all labels
    /// when empty) from the value dictionary, keeping only the values that
    /// match `search`, ranked by descending owner count (ties by the
    /// value's JSON text). Values read back as the records response shows
    /// them. `None` (caller scans) when a definition holds arrays or long
    /// values, or no scalar definition exists.
    ///
    /// The dictionary is streamed, and each kept value is charged to `meter`
    /// as it is kept; the returned values stay charged while the caller
    /// holds them.
    ///
    /// # Errors
    ///
    /// Returns a message when a read fails, a row is corrupt, or the meter
    /// stops the read.
    pub(crate) fn value_dictionary_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        labels: &[String],
        search: Option<&ValueMatcher>,
        meter: &mut WorkingSet<'_>,
    ) -> Result<Option<Vec<(serde_json::Value, u64)>>, String> {
        self.value_dictionary_of_types_in_view(view, property, labels, &[], search, meter)
    }

    /// [`Self::value_dictionary_in_view`] over the property's definitions of
    /// `types` only (all when empty).
    pub(crate) fn value_dictionary_of_types_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        labels: &[String],
        types: &[rushdb_types::model::LogicalType],
        search: Option<&ValueMatcher>,
        meter: &mut WorkingSet<'_>,
    ) -> Result<Option<Vec<(serde_json::Value, u64)>>, String> {
        use std::collections::hash_map::Entry;
        use std::collections::HashMap;
        let Some(definitions) = self.scalar_definitions_of(view, property, types)? else {
            return Ok(None);
        };
        if definitions.is_empty() {
            return Ok(None);
        }
        let t = self.tenant();
        // Across definitions the higher count wins; within one, a value held
        // under several labels sums their counts.
        let mut counts: HashMap<Vec<u8>, (serde_json::Value, u64)> = HashMap::new();
        let mut held = 0u64;
        let mut outcome: Result<bool, String> = Ok(true);
        for (ty, pid) in definitions {
            if !self.dictionary_answers(view, pid, labels)? {
                outcome = Ok(false);
                break;
            }
            let mut definition: HashMap<Vec<u8>, (serde_json::Value, u64)> = HashMap::new();
            let read = rushdb_lmpg::derived::for_each_dictionary_entry(
                view,
                t,
                OwnerKind::Record,
                pid,
                labels,
                &mut |entry, owner_count| {
                    let Some(payload) = entry.get(1..) else {
                        outcome = Err("corrupt value dictionary entry".into());
                        return false;
                    };
                    let Some(value) = dictionary_value(ty, payload) else {
                        return true;
                    };
                    if search.is_some_and(|search| !search.matches(&value)) {
                        return true;
                    }
                    let fingerprint = match serde_json::to_vec(&value) {
                        Ok(fingerprint) => fingerprint,
                        Err(error) => {
                            outcome = Err(error.to_string());
                            return false;
                        }
                    };
                    match definition.entry(fingerprint) {
                        Entry::Occupied(mut kept) => kept.get_mut().1 += owner_count,
                        Entry::Vacant(slot) => {
                            let bytes = value_bytes(slot.key().len());
                            if let Err(error) = meter.charge(bytes) {
                                outcome = Err(error);
                                return false;
                            }
                            held += bytes;
                            slot.insert((value, owner_count));
                        }
                    }
                    true
                },
            );
            if let Err(error) = read {
                outcome = Err(error.to_string());
            }
            if outcome.is_err() {
                break;
            }
            for (fingerprint, (value, count)) in definition {
                match counts.entry(fingerprint) {
                    Entry::Occupied(mut kept) => {
                        let bytes = value_bytes(kept.key().len());
                        meter.release(bytes);
                        held -= bytes;
                        kept.get_mut().1 = kept.get().1.max(count);
                    }
                    Entry::Vacant(slot) => {
                        slot.insert((value, count));
                    }
                }
            }
        }
        if !matches!(outcome, Ok(true)) {
            meter.release(held);
            outcome?;
            return Ok(None);
        }
        let mut out: Vec<(serde_json::Value, u64)> = counts.into_values().collect();
        out.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| {
                let left = serde_json::to_string(&a.0).unwrap_or_default();
                let right = serde_json::to_string(&b.0).unwrap_or_default();
                left.cmp(&right)
            })
        });
        Ok(Some(out))
    }

    /// Distinct count and, when it is at most `keep`, the values with their
    /// owner counts (by descending count, ties by value) of a text property
    /// on `label` that holds values too long for the dictionary. The count
    /// is exact from the write-time counters: the dictionary's for short
    /// values, the digest refcounts for long ones. Long values are digests
    /// on disk, so listing them reads the label's records until every digest
    /// is matched, at most `LONG_VALUE_WALK` records; past that the values
    /// are left out and the count stays. `None` when the property has no
    /// text definition.
    #[expect(
        clippy::too_many_lines,
        reason = "one linear digest walk; splitting would scatter the shared read-view borrows"
    )]
    pub(crate) fn long_text_values_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        property: &str,
        label: &str,
        keep: u64,
        meter: &mut WorkingSet<'_>,
    ) -> Result<Option<ScannedText>, String> {
        use rushdb_lmpg::derived;
        use rushdb_types::keys as k;
        use rushdb_types::model::LogicalType;
        use std::collections::{BTreeMap, BTreeSet};
        let Some(definitions) = self.scalar_definitions_of(view, property, &[LogicalType::Utf8])?
        else {
            return Ok(None);
        };
        if definitions.is_empty() {
            return Ok(None);
        }
        let t = self.tenant();
        let scope = [label.to_string()];
        let mut short: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
        let mut long: BTreeMap<[u8; k::LONG_DIGEST_BYTES], u64> = BTreeMap::new();
        let distinct = if let [(_, pid)] = definitions.as_slice() {
            derived::label_distinct(view, t, OwnerKind::Record, label, *pid)
                .map_err(|e| e.to_string())?
        } else {
            let mut entries: BTreeSet<Vec<u8>> = BTreeSet::new();
            let mut digests = BTreeSet::new();
            for (_, pid) in &definitions {
                entries.extend(
                    derived::dictionary(view, t, OwnerKind::Record, *pid, &scope)
                        .map_err(|e| e.to_string())?
                        .into_keys(),
                );
                digests.extend(
                    derived::long_value_counts(view, t, OwnerKind::Record, label, *pid)
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .map(|(digest, _)| digest),
                );
            }
            (entries.len() + digests.len()) as u64
        };
        if distinct == 0 || distinct > keep {
            return Ok(Some((distinct, Vec::new())));
        }
        // At most `keep` values from here on.
        for (_, pid) in &definitions {
            for (entry, count) in derived::dictionary(view, t, OwnerKind::Record, *pid, &scope)
                .map_err(|e| e.to_string())?
            {
                *short.entry(entry).or_default() += count;
            }
            for (digest, count) in
                derived::long_value_counts(view, t, OwnerKind::Record, label, *pid)
                    .map_err(|e| e.to_string())?
            {
                *long.entry(digest).or_default() += count;
            }
        }
        let decode = |entry: &[u8]| -> Option<serde_json::Value> {
            let scalar =
                rushdb_types::encoding::decode_order_preserving(LogicalType::Utf8, entry.get(1..)?)
                    .ok()?;
            Some(rushdb_types::value::scalar_to_json(&scalar))
        };
        let mut found: BTreeMap<[u8; k::LONG_DIGEST_BYTES], serde_json::Value> = BTreeMap::new();
        let mut held = 0u64;
        let mut walked = 0u64;
        let mut outcome: Result<(), String> = Ok(());
        'walk: for (_, pid) in &definitions {
            if found.len() == long.len() {
                break;
            }
            let prefix =
                k::presence_nullness_prefix(t, OwnerKind::Record, label, *pid, k::NULLNESS_VALUE);
            let (start, end) = rushdb_storage::prefix_range(&prefix);
            let mut visit = |key: &[u8]| -> Result<bool, String> {
                walked += 1;
                if walked > LONG_VALUE_WALK {
                    return Ok(false);
                }
                if walked.is_multiple_of(1024) {
                    meter.check()?;
                }
                let (_, _, _, _, owner) =
                    k::parse_presence_key(key).ok_or("corrupt presence row")?;
                let rows = view
                    .range_prefix(&k::occ_pid_prefix(t, OwnerKind::Record, owner, *pid))
                    .map_err(|e| e.to_string())?;
                for (_, raw) in rows {
                    let occ = rushdb_types::occurrence::decode_occ_value(&raw)
                        .map_err(|_| "corrupt occurrence row".to_string())?;
                    if occ.entry.len() <= k::DICT_MAX_VALUE_BYTES {
                        continue;
                    }
                    let digest = k::long_value_digest(occ.entry);
                    if !long.contains_key(&digest) || found.contains_key(&digest) {
                        continue;
                    }
                    if let Some(value) = decode(occ.entry) {
                        let bytes = value_bytes(occ.entry.len());
                        meter.charge(bytes)?;
                        held += bytes;
                        found.insert(digest, value);
                    }
                }
                Ok(found.len() < long.len())
            };
            view.for_each_row(&start, &end, &mut |key, _| match visit(key) {
                Ok(more) => more,
                Err(error) => {
                    outcome = Err(error);
                    false
                }
            })
            .map_err(|e| e.to_string())?;
            if outcome.is_err() || walked > LONG_VALUE_WALK {
                break 'walk;
            }
        }
        meter.release(held);
        outcome?;
        if found.len() < long.len() {
            // Not every long value was reached within the walk.
            return Ok(Some((distinct, Vec::new())));
        }
        let mut values: Vec<(serde_json::Value, u64)> = short
            .into_iter()
            .filter_map(|(entry, count)| Some((decode(&entry)?, count)))
            .chain(
                found
                    .into_iter()
                    .map(|(digest, value)| (value, long[&digest])),
            )
            .collect();
        values.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| {
                let left = serde_json::to_string(&a.0).unwrap_or_default();
                let right = serde_json::to_string(&b.0).unwrap_or_default();
                left.cmp(&right)
            })
        });
        Ok(Some((distinct, values)))
    }

    /// Whether vector writes/searches must carry an explicit index name.
    #[must_use]
    pub fn require_vector_index_name(&self) -> bool {
        self.require_vector_index_name
    }

    /// See `resources.introspection-value-max-chars` (0 = no cap).
    #[must_use]
    pub fn introspection_value_max_chars(&self) -> usize {
        self.introspection_value_max_chars
    }
    /// Property-definition metadata keyed by normalized property name. A
    /// family may have more than one typed definition, and every definition
    /// gets its own public id.
    pub(super) fn property_meta_in_view(
        &self,
        view: &rushdb_storage::ReadView,
    ) -> Result<PropertyMetadata, String> {
        use rushdb_types::keys::{key, P_FAMILY};
        let tenant = self.tenant();
        // Include the separator: `property-family-signature` is a different
        // key family and must not be scanned as a PropertyFamily row.
        let pref = key(tenant, &[P_FAMILY, b""]);
        let mut families = std::collections::BTreeMap::new();
        let mut out = std::collections::BTreeMap::new();
        for (_, raw) in view.range_prefix(&pref).map_err(|e| e.to_string())? {
            let fam: rushdb_types::occurrence::PropertyFamily = serde_json::from_slice(&raw)
                .map_err(|e| format!("corrupt property family: {e}"))?;
            families.insert(fam.family_id, fam);
        }
        // Scan definitions once, not once per property family.
        for (_, raw) in view
            .range_prefix(&key(tenant, &[b"property-row", b""]))
            .map_err(|e| e.to_string())?
        {
            let def: rushdb_types::occurrence::PropertyDefinition = serde_json::from_slice(&raw)
                .map_err(|e| format!("corrupt property definition: {e}"))?;
            let fam = families
                .get(&def.family_id)
                .ok_or("corrupt property definition: missing family")?;
            if fam.namespace != "default" || fam.owner_scope != OwnerKind::Record {
                continue;
            }
            if let rushdb_types::model::TypeState::Known(ty) = def.type_state {
                out.entry(fam.normalized_name.clone())
                    .or_insert_with(Vec::new)
                    .push((
                        def.property_id,
                        fam.display_name.clone(),
                        type_name_str(ty),
                        ty,
                    ));
            }
        }
        Ok(out)
    }

    /// Property definitions actually present on records matched by a query.
    /// The id is a tenant-local ordinal reference: it is only ever resolved
    /// against the current request's tenant catalog (`resolve_property_name`),
    /// so it carries no tenant fingerprint or cross-tenant capability.
    pub(super) fn public_property_id(property_id: u64) -> String {
        format!("p{property_id:016x}")
    }

    /// The property id behind [`Self::public_property_id`].
    pub(super) fn private_property_id(public: &str) -> Option<u64> {
        let hex = public.strip_prefix('p')?;
        (hex.len() == 16)
            .then(|| u64::from_str_radix(hex, 16).ok())
            .flatten()
    }

    /// Exact `(min, max)` of `pid` on `label` from the label's value
    /// dictionary, whose entries sort in value order; `None` when the
    /// dictionary does not hold every value (arrays, long strings) or holds
    /// none. Datetimes render as records do.
    fn exact_range(
        &self,
        view: &rushdb_storage::ReadView,
        meta: &PropertyMetadata,
        label: &str,
        pid: u64,
    ) -> Result<Option<(serde_json::Value, serde_json::Value)>, String> {
        use rushdb_types::model::LogicalType;
        let Some(ty) = meta
            .values()
            .flatten()
            .find_map(|(id, _, _, logical)| (*id == pid).then_some(*logical))
        else {
            return Ok(None);
        };
        let scope = [label.to_string()];
        if !self.dictionary_answers(view, pid, &scope)? {
            return Ok(None);
        }
        let mut first: Option<Vec<u8>> = None;
        let mut last: Option<Vec<u8>> = None;
        rushdb_lmpg::derived::for_each_dictionary_entry(
            view,
            self.tenant(),
            OwnerKind::Record,
            pid,
            &scope,
            &mut |entry, count| {
                if count > 0 {
                    if first.is_none() {
                        first = Some(entry.to_vec());
                    }
                    last = Some(entry.to_vec());
                }
                true
            },
        )
        .map_err(|e| e.to_string())?;
        let decode = |entry: Option<Vec<u8>>| -> Result<Option<serde_json::Value>, String> {
            let Some(entry) = entry else {
                return Ok(None);
            };
            let payload = entry
                .get(1..)
                .ok_or_else(|| "corrupt value dictionary entry".to_string())?;
            let scalar = rushdb_types::encoding::decode_order_preserving(ty, payload)
                .map_err(|e| e.to_string())?;
            Ok(Some(match scalar {
                rushdb_types::value::Scalar::DateTime(ms) if ty == LogicalType::DateTime => {
                    serde_json::Value::String(rushdb_types::datetime::ms_to_datetime_string(ms))
                }
                other => rushdb_types::value::scalar_to_json(&other),
            }))
        };
        Ok(decode(first)?.zip(decode(last)?))
    }

    /// Resolve either a stable property identifier or a raw property name.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn resolve_property_name(&self, id: &str) -> Result<String, String> {
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        self.resolve_property_name_in_view(&view, id)
    }

    /// The untyped definition every explicit `null` of a record property
    /// lands on: `(pid, display name)` per family that has one. Described as
    /// the property's `null` entry, beside any typed entries.
    pub(super) fn null_properties_in_view(
        &self,
        view: &rushdb_storage::ReadView,
    ) -> Result<Vec<(u64, String)>, String> {
        use rushdb_types::keys::{key, P_FAMILY};
        let tenant = self.tenant();
        let mut families = std::collections::BTreeMap::new();
        for (_, raw) in view
            .range_prefix(&key(tenant, &[P_FAMILY, b""]))
            .map_err(|e| e.to_string())?
        {
            let fam: rushdb_types::occurrence::PropertyFamily = serde_json::from_slice(&raw)
                .map_err(|e| format!("corrupt property family: {e}"))?;
            families.insert(fam.family_id, fam);
        }
        let mut untyped = Vec::new();
        for (_, raw) in view
            .range_prefix(&key(tenant, &[b"property-row", b""]))
            .map_err(|e| e.to_string())?
        {
            let def: rushdb_types::occurrence::PropertyDefinition = serde_json::from_slice(&raw)
                .map_err(|e| format!("corrupt property definition: {e}"))?;
            if def.type_state == rushdb_types::model::TypeState::Unresolved {
                untyped.push((def.family_id, def.property_id));
            }
        }
        Ok(untyped
            .into_iter()
            .filter_map(|(family, pid)| {
                let fam = families.get(&family)?;
                (fam.namespace == "default" && fam.owner_scope == OwnerKind::Record)
                    .then(|| (pid, fam.display_name.clone()))
            })
            .collect())
    }

    /// Resolve a property identifier against a caller-owned pinned view.
    pub(crate) fn resolve_property_name_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        id: &str,
    ) -> Result<String, String> {
        if id.starts_with('p') {
            for (norm, defs) in self.property_meta_in_view(view)? {
                for (pid, _, _, _) in defs {
                    if Self::public_property_id(pid) == id {
                        return Ok(norm);
                    }
                }
            }
        }
        Ok(rushdb_types::value::normalize_name(id))
    }

    /// Describe property definitions, types, support counts, and value statistics for matching records.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn describe_properties(
        &self,
        body: &serde_json::Value,
    ) -> Result<Vec<rushdb_protocol::PropertyDescription>, String> {
        let view = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.describe_properties_in_view(&view, body, None, &|| false)
    }

    /// Describe properties from a caller-owned pinned snapshot.
    #[expect(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
    )]
    pub(crate) fn describe_properties_in_view(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        body: &serde_json::Value,
        matched: Option<&[u64]>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<rushdb_protocol::PropertyDescription>, String> {
        let meta = self.property_meta_in_view(view)?;
        let null_only = self.null_properties_in_view(view)?;
        // E8 type-split siblings: every definition sharing a normalized name
        // carries the other canonical types present for that name, so a
        // client cached on a single-type definition can detect partitioned
        // support mechanically instead of silently under-reading. Siblings
        // are logical type names (`int64` vs `float64`): the display
        // `value_type` collapses both to `number`, which is exactly the
        // false-certainty hazard this flag exists to dispel.
        let mut sibling_map: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for (norm, defs) in &meta {
            let mut types: Vec<String> = defs
                .iter()
                .map(|(_, _, _, logical)| logical_type_name(*logical))
                .collect();
            types.sort();
            types.dedup();
            if types.len() > 1 {
                sibling_map.insert(norm.clone(), types);
            }
        }
        let siblings_of = |norm: &str, logical: &rushdb_types::model::LogicalType| -> Vec<String> {
            let here = logical_type_name(*logical);
            sibling_map
                .get(norm)
                .map(|types| {
                    types
                        .iter()
                        .filter(|t| t.as_str() != here)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };
        // E2 entry pagination: slice of the returned entry list, independent
        // of the stats scan window below.
        let entry_skip = body
            .get("entry_skip")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        let entry_limit = body
            .get("entry_limit")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as usize);
        // E2 catalog detail: names, types, exact support counts, array shape
        // and sibling flags from catalog indexes alone (no record scan).
        // A filtered (`where`) catalog read is refused — filtering needs the
        // record scan, which is exactly what `Full` (the default) does.
        let catalog = body
            .get("detail")
            .and_then(|v| v.as_str())
            .is_some_and(|d| d == "catalog");
        let where_empty = match body.get("where") {
            None => true,
            Some(v) => v.is_null() || v.as_object().is_some_and(serde_json::Map::is_empty),
        };
        if catalog {
            if !where_empty {
                return Err(
                    "catalog detail does not support `where`; use full detail or a label filter"
                        .to_string(),
                );
            }
            let mut out = merge_number_definitions(
                self.describe_properties_catalog(view, &meta, &null_only, &siblings_of, body)?,
                &meta,
            );
            out.sort_by(|a, b| a.name.cmp(&b.name));
            return Ok(paginate_entries(out, entry_skip, entry_limit));
        }
        // Resolve the match set to ordinals through the shared search path,
        // then attribute statistics per (label, pid) from occurrence rows, so
        // one describe.schema call yields per-label properties without
        // building record JSON. Filtered support counts cover every match
        // (an oversized match set fails on the query working-set cap rather
        // than returning a partial count disguised as an exact one); the
        // unfiltered read samples the first 10k records and takes exact
        // counts from the catalog below.
        let document = serde_json::json!({
            "labels": body.get("labels").cloned().unwrap_or(serde_json::Value::Array(vec![])),
            "where": body.get("where").cloned().unwrap_or_else(|| serde_json::json!({})),
        });
        let parsed =
            rushdb_query::document::parse_document(&document).map_err(|e| e.to_string())?;
        let resolved;
        let ords: &[u64] = match matched {
            // The caller resolved this filtered query's match set already.
            Some(ords) if !where_empty => ords,
            _ => {
                resolved = self.facet_ordinals(view, &parsed.search, is_cancelled)?;
                &resolved
            }
        };
        let ords = if where_empty {
            &ords[..ords.len().min(10_000)]
        } else {
            ords
        };
        let described: std::collections::HashSet<u64> = meta
            .values()
            .flat_map(|defs| defs.iter().map(|(pid, _, _, _)| *pid))
            .chain(null_only.iter().map(|(pid, _)| *pid))
            .collect();
        let stats = self.facet_property_stats(view.as_ref(), ords, &described, is_cancelled)?;
        let catalog_view = where_empty.then(|| Arc::clone(view));
        // The types each name holds on each label within the match set.
        let present: std::collections::BTreeSet<(&str, &str, String)> = stats
            .keys()
            .filter_map(|(label, pid)| {
                meta.iter().find_map(|(norm, defs)| {
                    defs.iter()
                        .find(|(id, _, _, _)| id == pid)
                        .map(|(_, _, _, logical)| {
                            (label.as_str(), norm.as_str(), logical_type_name(*logical))
                        })
                })
            })
            .collect();
        let mut out: Vec<rushdb_protocol::PropertyDescription> = stats
            .iter()
            .filter_map(|((label, pid), s)| {
                let typed = meta
                    .values()
                    .flat_map(|defs| defs.iter())
                    .find(|(definition_pid, _, _, _)| definition_pid == pid);
                let (name, value_type, sibling_types) = if let Some((_, name, ty, logical)) = typed
                {
                    let norm = rushdb_types::value::normalize_name(name);
                    let siblings = siblings_of(&norm, logical)
                        .into_iter()
                        .filter(|sibling| {
                            present.contains(&(label.as_str(), norm.as_str(), sibling.clone()))
                        })
                        .collect();
                    (name.clone(), ty.clone(), siblings)
                } else {
                    let (_, name) = null_only.iter().find(|(null_pid, _)| null_pid == pid)?;
                    (name.clone(), "null".to_string(), Vec::new())
                };
                // Datetimes are stored as epoch milliseconds, which order
                // by instant; describe them as the records response renders
                // them.
                let render = |value: &serde_json::Value| match value.as_i64() {
                    Some(ms) if value_type == "datetime" => {
                        serde_json::Value::String(rushdb_types::datetime::ms_to_datetime_string(ms))
                    }
                    _ => value.clone(),
                };
                Some(rushdb_protocol::PropertyDescription {
                    id: Self::public_property_id(*pid),
                    records_count: s.records_count as u64,
                    label: Some(label.clone()),
                    is_array: s.is_array,
                    sibling_types,
                    sample_values: s.samples.iter().map(render).collect(),
                    min: s.min.as_ref().map(render),
                    max: s.max.as_ref().map(render),
                    name,
                    value_type,
                })
            })
            .collect();
        if where_empty {
            // The bounded scan above supplies samples and ranges. Replace its
            // support facts with the exact maintained `(label, pid)` catalog;
            // this also removes typed siblings that were not actually present
            // on a sampled label.
            let Some(catalog_view) = catalog_view.as_ref() else {
                unreachable!("unfiltered scan retains its catalog view")
            };
            let mut exact: std::collections::BTreeMap<
                (String, String),
                rushdb_protocol::PropertyDescription,
            > = self
                .describe_properties_catalog(catalog_view, &meta, &null_only, &siblings_of, body)?
                .into_iter()
                .filter_map(|entry| Some(((entry.label.clone()?, entry.id.clone()), entry)))
                .collect();
            out.retain_mut(|entry| {
                let Some(label) = entry.label.as_ref() else {
                    return false;
                };
                let Some(catalog_entry) = exact.remove(&(label.clone(), entry.id.clone())) else {
                    return false;
                };
                entry.records_count = catalog_entry.records_count;
                entry.is_array = catalog_entry.is_array;
                // Type splits on this label, not elsewhere in the tenant.
                entry.sibling_types = catalog_entry.sibling_types;
                true
            });
            // Catalog entries outside the bounded sample remain part of the
            // schema, with empty sample fields from the catalog row.
            out.extend(exact.into_values());
            // The sample's range would describe the first records only:
            // ranges come from the label's value dictionary instead, exact
            // over every record, and are left out where the dictionary does
            // not hold every value (arrays, long strings).
            for entry in &mut out {
                let range = match (entry.label.as_ref(), Self::private_property_id(&entry.id)) {
                    (Some(label), Some(pid)) => {
                        self.exact_range(catalog_view.as_ref(), &meta, label, pid)?
                    }
                    _ => None,
                };
                (entry.min, entry.max) = range.unzip();
            }
        }
        let mut out = merge_number_definitions(out, &meta);
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(paginate_entries(out, entry_skip, entry_limit))
    }

    /// E2 catalog detail: per-`(label, property)` live owner counts, array
    /// shape, and sibling flags from maintained catalog indexes alone — no
    /// record scan. Counts come from the label-scoped support rows
    /// (`P_PROP_SUPPORT`, sharded by owner ordinal); array shape from the
    /// sticky `P_PROP_ARRAY` marker. Cost is
    /// O(properties × labels-with-that-property), independent of record count.
    pub(super) fn describe_properties_catalog(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        meta: &PropertyMetadata,
        null_only: &[(u64, String)],
        siblings_of: &dyn Fn(&str, &rushdb_types::model::LogicalType) -> Vec<String>,
        body: &serde_json::Value,
    ) -> Result<Vec<rushdb_protocol::PropertyDescription>, String> {
        let tenant = self.tenant();
        // Absent or empty `labels` = all labels (matches the full-scan path).
        let label_filter: Option<std::collections::BTreeSet<String>> = match body.get("labels") {
            Some(serde_json::Value::Array(items)) if !items.is_empty() => Some(
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
            ),
            _ => None,
        };
        // (label, pid) -> description; ordered to match the member-scan oracle.
        let mut out: std::collections::BTreeMap<
            (String, u64),
            rushdb_protocol::PropertyDescription,
        > = std::collections::BTreeMap::new();
        let (supported, present) =
            label_supports(view.as_ref(), tenant, meta, label_filter.as_ref())?;
        for (norm, pid, name, ty, logical, label, records_count) in supported {
            let on_label = present.get(&(label.clone(), norm.to_string()));
            let sibling_types = siblings_of(norm, &logical)
                .into_iter()
                .filter(|sibling| on_label.is_some_and(|types| types.contains(sibling)))
                .collect();
            let is_array = view
                .get(&rushdb_types::keys::array_label_marker_key(
                    tenant,
                    OwnerKind::Record,
                    pid,
                    &label,
                ))
                .map_err(|e| e.to_string())?
                .is_some();
            out.insert(
                (label.clone(), pid),
                rushdb_protocol::PropertyDescription {
                    id: Self::public_property_id(pid),
                    name: name.clone(),
                    value_type: ty.clone(),
                    records_count,
                    label: Some(label),
                    is_array,
                    sibling_types,
                    sample_values: Vec::new(),
                    min: None,
                    max: None,
                },
            );
        }
        // A property only ever set to `null` has type `null`.
        for (pid, name) in null_only {
            for (label, records_count) in
                access::prop_support_label_counts(view.as_ref(), tenant, *pid)
                    .map_err(|e| e.to_string())?
            {
                if label_filter
                    .as_ref()
                    .is_some_and(|filter| !filter.contains(&label))
                {
                    continue;
                }
                out.insert(
                    (label.clone(), *pid),
                    rushdb_protocol::PropertyDescription {
                        id: Self::public_property_id(*pid),
                        name: name.clone(),
                        value_type: "null".to_string(),
                        records_count,
                        label: Some(label),
                        is_array: false,
                        sibling_types: Vec::new(),
                        sample_values: Vec::new(),
                        min: None,
                        max: None,
                    },
                );
            }
        }
        Ok(out.into_values().collect())
    }

    /// Differential oracle for [`Self::describe_properties_catalog`]: compute
    /// the same per-`(label, pid)` counts and array shape by scanning every
    /// label member and its occurrence-owner keys. Test-only; O(records) by
    /// construction, which is exactly why the production path no longer uses
    /// it.
    #[cfg(test)]
    pub(super) fn describe_properties_catalog_scan(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        meta: &PropertyMetadata,
        siblings_of: &dyn Fn(&str, &rushdb_types::model::LogicalType) -> Vec<String>,
        body: &serde_json::Value,
    ) -> Result<Vec<rushdb_protocol::PropertyDescription>, String> {
        let tenant = self.tenant();
        // pid -> (norm, display name, display type, logical type) over catalog.
        let mut by_pid: std::collections::BTreeMap<
            u64,
            (String, String, String, rushdb_types::model::LogicalType),
        > = std::collections::BTreeMap::new();
        for (norm, defs) in meta {
            for (pid, name, ty, logical) in defs {
                by_pid.insert(*pid, (norm.clone(), name.clone(), ty.clone(), *logical));
            }
        }
        let labels: Vec<String> = match body.get("labels") {
            Some(serde_json::Value::Array(items)) if !items.is_empty() => items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            // Absent or empty = all labels (matches the full-scan path,
            // which treats an empty label list as unscoped).
            _ => access::labels(view.as_ref(), tenant).map_err(|e| e.to_string())?,
        };
        // (label, pid) -> (distinct owners, array evidence).
        let mut counts: std::collections::BTreeMap<(String, u64), (u64, bool)> =
            std::collections::BTreeMap::new();
        for label in &labels {
            let members =
                access::label_members(view.as_ref(), tenant, label).map_err(|e| e.to_string())?;
            for ord in members {
                // pid -> array evidence for this owner, from one occurrence
                // prefix scan.
                let mut per_pid: std::collections::BTreeMap<u64, bool> =
                    std::collections::BTreeMap::new();
                for occ in
                    access::occurrences_of_owner(view.as_ref(), tenant, OwnerKind::Record, ord)
                        .map_err(|e| e.to_string())?
                {
                    *per_pid.entry(occ.pid).or_default() |= occ.array;
                }
                for (pid, is_array) in per_pid {
                    let slot = counts.entry((label.clone(), pid)).or_insert((0, false));
                    slot.0 += 1;
                    slot.1 |= is_array;
                }
            }
        }
        let mut out = Vec::new();
        for ((label, pid), (records, is_array)) in counts {
            let Some((norm, name, ty, logical)) = by_pid.get(&pid) else {
                continue;
            };
            out.push(rushdb_protocol::PropertyDescription {
                id: Self::public_property_id(pid),
                name: name.clone(),
                value_type: ty.clone(),
                records_count: records,
                label: Some(label),
                is_array,
                sibling_types: siblings_of(norm, logical),
                sample_values: Vec::new(),
                min: None,
                max: None,
            });
        }
        Ok(out)
    }
}

/// One `(label, property definition)` with owners on the label: normalized
/// name, pid, display name, display type, logical type, label, owners.
type LabelSupport<'a> = (
    &'a str,
    u64,
    &'a String,
    &'a String,
    rushdb_types::model::LogicalType,
    String,
    u64,
);

/// `(label, normalized name)` -> the logical types held there.
type TypesOnLabel =
    std::collections::BTreeMap<(String, String), std::collections::BTreeSet<String>>;

/// Every definition's owner count per label (under `label_filter`), and the
/// logical types each normalized name holds on each label, so a type split
/// is reported where it exists, not wherever the name is used.
fn label_supports<'a>(
    view: &rushdb_storage::ReadView,
    tenant: rushdb_types::ids::TenantId,
    meta: &'a PropertyMetadata,
    label_filter: Option<&std::collections::BTreeSet<String>>,
) -> Result<(Vec<LabelSupport<'a>>, TypesOnLabel), String> {
    let mut supported = Vec::new();
    let mut present = TypesOnLabel::new();
    for (norm, defs) in meta {
        for (pid, name, ty, logical) in defs {
            for (label, records_count) in
                access::prop_support_label_counts(view, tenant, *pid).map_err(|e| e.to_string())?
            {
                if label_filter.is_some_and(|filter| !filter.contains(&label)) {
                    continue;
                }
                if records_count > 0 {
                    present
                        .entry((label.clone(), norm.clone()))
                        .or_default()
                        .insert(logical_type_name(*logical));
                }
                supported.push((
                    norm.as_str(),
                    *pid,
                    name,
                    ty,
                    *logical,
                    label,
                    records_count,
                ));
            }
        }
    }
    Ok((supported, present))
}

/// Integers and floats are one `number` to clients, but a property holding
/// both is stored in two definitions (`Int64`, then `Float64` once a float
/// arrives). Describe them as one entry per label: the counts add up
/// exactly, since a record holds one of the two (a mixed array is stored as
/// floats); ranges and samples combine; the id is the property's first
/// numeric definition, the same on every label; and the int/float split is
/// not reported as a sibling type.
fn merge_number_definitions(
    entries: Vec<rushdb_protocol::PropertyDescription>,
    meta: &super::PropertyMetadata,
) -> Vec<rushdb_protocol::PropertyDescription> {
    use rushdb_types::model::LogicalType;
    // Display name -> id of its first numeric definition.
    let canonical: std::collections::HashMap<&str, String> = meta
        .values()
        .flat_map(|defs| defs.iter())
        .filter(|(_, _, _, logical)| matches!(logical, LogicalType::Int64 | LogicalType::Float64))
        .fold(
            std::collections::HashMap::new(),
            |mut first, (pid, name, _, _)| {
                first
                    .entry(name.as_str())
                    .and_modify(|lowest: &mut u64| *lowest = (*lowest).min(*pid))
                    .or_insert(*pid);
                first
            },
        )
        .into_iter()
        .map(|(name, pid)| (name, RequestScope::public_property_id(pid)))
        .collect();
    let mut merged: Vec<rushdb_protocol::PropertyDescription> = Vec::with_capacity(entries.len());
    let mut at: std::collections::HashMap<(Option<String>, String), usize> =
        std::collections::HashMap::new();
    for mut entry in entries {
        if entry.value_type != "number" {
            merged.push(entry);
            continue;
        }
        if let Some(id) = canonical.get(entry.name.as_str()) {
            entry.id.clone_from(id);
        }
        entry
            .sibling_types
            .retain(|ty| ty != "int64" && ty != "float64");
        let key = (entry.label.clone(), entry.name.clone());
        let Some(&index) = at.get(&key) else {
            at.insert(key, merged.len());
            merged.push(entry);
            continue;
        };
        let into = &mut merged[index];
        into.records_count += entry.records_count;
        into.is_array |= entry.is_array;
        for value in entry.sample_values {
            if into.sample_values.len() < super::SCHEMA_SAMPLE_CAP
                && !into.sample_values.contains(&value)
            {
                into.sample_values.push(value);
            }
        }
        let pick = |current: &mut Option<serde_json::Value>,
                    other: Option<serde_json::Value>,
                    keep: std::cmp::Ordering| {
            if let Some(other) = other {
                let replace = current.as_ref().is_none_or(|value| {
                    rushdb_types::value::json_scalar_cmp(&other, value) == Some(keep)
                });
                if replace {
                    *current = Some(other);
                }
            }
        };
        pick(&mut into.min, entry.min, std::cmp::Ordering::Less);
        pick(&mut into.max, entry.max, std::cmp::Ordering::Greater);
    }
    merged
}

/// A value-dictionary payload of type `ty` as the records response shows it
/// (datetimes in their stored rendering), or `None` when it does not decode.
fn dictionary_value(
    ty: rushdb_types::model::LogicalType,
    payload: &[u8],
) -> Option<serde_json::Value> {
    let scalar = rushdb_types::encoding::decode_order_preserving(ty, payload).ok()?;
    Some(match scalar {
        rushdb_types::value::Scalar::DateTime(ms) => {
            serde_json::Value::String(rushdb_types::datetime::ms_to_datetime_string(ms))
        }
        scalar => rushdb_types::value::scalar_to_json(&scalar),
    })
}
