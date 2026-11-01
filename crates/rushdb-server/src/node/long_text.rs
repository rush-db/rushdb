//! Long text values: properties whose values are too long for the value
//! dictionary and are stored as digests. Listing and searching them reads
//! the records that hold them, so these read paths are kept apart from the
//! dictionary-backed catalog reads in `catalog.rs`.

use super::catalog::ScannedText;
use super::working_set::{value_bytes, WorkingSet};
use super::{OwnerKind, RequestScope};
use rushdb_lmpg::reader::TryRead;
use rushdb_types::value_match::ValueMatcher;

/// Records one listing of long text values may read
/// (`long_text_values_in_view`) before it leaves the values out.
const LONG_VALUE_WALK: u64 = 65_536;

impl RequestScope<'_> {
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

    /// The values of a text `property` on `label` that `search` matches,
    /// with owner counts, as a facet over every record of the label counts
    /// them, for a property the dictionary cannot answer alone because some
    /// values are too long for it. Short values are matched in the
    /// dictionary; long values are read only from the owners the presence
    /// rows list and matched in place, so no record header or other column
    /// is read. `None` (the caller scans records) unless the label holds
    /// the name under one text definition and no arrays.
    ///
    /// # Errors
    ///
    /// Returns a message when a read fails, a row is corrupt, or the meter
    /// stops the read.
    pub(crate) fn search_label_texts(
        &self,
        view: &rushdb_storage::ReadView,
        label: &str,
        property: &str,
        search: &ValueMatcher,
        access: &rushdb_lmpg::CatalogAccess,
        meter: &mut WorkingSet<'_>,
    ) -> Result<Option<Vec<(serde_json::Value, u64)>>, String> {
        let t = self.tenant();
        let Some(pid) = self.sole_text_definition(view, label, property)? else {
            return Ok(None);
        };
        if !access.property(label, property) {
            return Ok(Some(Vec::new()));
        }
        let text_of = |entry: &[u8]| -> Result<String, String> {
            let payload = entry
                .get(1..)
                .and_then(|payload| payload.strip_suffix(&[0x00]))
                .ok_or("corrupt text value")?;
            std::str::from_utf8(payload)
                .map(str::to_owned)
                .map_err(|_| "corrupt text value".to_string())
        };
        let mut out: Vec<(serde_json::Value, u64)> = Vec::new();
        let mut outcome: Result<(), String> = Ok(());
        let scope = [label.to_string()];
        rushdb_lmpg::derived::for_each_dictionary_entry(
            view,
            t,
            OwnerKind::Record,
            pid,
            &scope,
            &mut |entry, count| {
                let step = text_of(entry).and_then(|text| {
                    if count > 0 && search.matches_text(&text) {
                        meter.charge(value_bytes(entry.len()))?;
                        out.push((serde_json::Value::String(text), count));
                    }
                    Ok(())
                });
                match step {
                    Ok(()) => true,
                    Err(error) => {
                        outcome = Err(error);
                        false
                    }
                }
            },
        )
        .map_err(|e| e.to_string())?;
        std::mem::replace(&mut outcome, Ok(()))?;
        self.match_long_texts(view, label, pid, search, meter, &mut out)?;
        Ok(Some(out))
    }

    /// The one text definition `property` holds on `label` with no arrays,
    /// or `None` when the label holds the name otherwise.
    fn sole_text_definition(
        &self,
        view: &rushdb_storage::ReadView,
        label: &str,
        property: &str,
    ) -> Result<Option<u64>, String> {
        use rushdb_types::keys as k;
        use rushdb_types::model::LogicalType;
        let t = self.tenant();
        let Some(definitions) = self.scalar_definitions(view, property)? else {
            return Ok(None);
        };
        let mut held_here = Vec::new();
        for (ty, pid) in definitions {
            if view
                .first_key(&k::presence_pid_prefix(t, OwnerKind::Record, label, pid))
                .map_err(|e| e.to_string())?
                .is_some()
            {
                held_here.push((ty, pid));
            }
        }
        let [(LogicalType::Utf8, pid)] = held_here[..] else {
            return Ok(None);
        };
        let (_, array_labels) =
            rushdb_lmpg::derived::array_markers(view, t, OwnerKind::Record, pid)
                .map_err(|e| e.to_string())?;
        if array_labels.iter().any(|held| held == label) {
            return Ok(None);
        }
        Ok(Some(pid))
    }

    /// Adds to `out` the long values of text `pid` on `label` that `search`
    /// matches, read from the owners its presence rows list, counted by
    /// digest so each distinct value is kept once.
    fn match_long_texts(
        &self,
        view: &rushdb_storage::ReadView,
        label: &str,
        pid: u64,
        search: &ValueMatcher,
        meter: &mut WorkingSet<'_>,
        out: &mut Vec<(serde_json::Value, u64)>,
    ) -> Result<(), String> {
        use rushdb_types::keys as k;
        use std::collections::HashMap;
        let t = self.tenant();
        let mut outcome: Result<(), String> = Ok(());
        // Long values, counted by digest: each distinct value is kept once.
        let mut long: HashMap<[u8; k::LONG_DIGEST_BYTES], usize> = HashMap::new();
        let prefix =
            k::presence_nullness_prefix(t, OwnerKind::Record, label, pid, k::NULLNESS_VALUE);
        let (start, end) = rushdb_storage::prefix_range(&prefix);
        let mut walked = 0u64;
        let mut visit = |key: &[u8]| -> Result<(), String> {
            walked += 1;
            if walked.is_multiple_of(1024) {
                meter.check()?;
            }
            let (_, _, _, _, owner) = k::parse_presence_key(key).ok_or("corrupt presence row")?;
            let (from, to) =
                rushdb_storage::prefix_range(&k::occ_pid_prefix(t, OwnerKind::Record, owner, pid));
            let mut step: Result<(), String> = Ok(());
            view.for_each_row(&from, &to, &mut |_, raw| {
                step = (|| {
                    let occ = rushdb_types::occurrence::decode_occ_value(raw)
                        .map_err(|_| "corrupt occurrence row".to_string())?;
                    if occ.entry.len() <= k::DICT_MAX_VALUE_BYTES {
                        return Ok(());
                    }
                    let text = occ
                        .payload
                        .strip_suffix(&[0x00])
                        .and_then(|payload| std::str::from_utf8(payload).ok())
                        .ok_or("corrupt text value")?;
                    if !search.matches_text(text) {
                        return Ok(());
                    }
                    let digest = k::long_value_digest(occ.entry);
                    if let Some(&at) = long.get(&digest) {
                        out[at].1 += 1;
                        return Ok(());
                    }
                    // Charged while the caller holds the values.
                    meter.charge(value_bytes(occ.entry.len()))?;
                    long.insert(digest, out.len());
                    out.push((serde_json::Value::String(text.to_owned()), 1));
                    Ok(())
                })();
                step.is_ok()
            })
            .map_err(|e| e.to_string())?;
            step?;
            Ok(())
        };
        view.for_each_row(&start, &end, &mut |key, _| match visit(key) {
            Ok(()) => true,
            Err(error) => {
                outcome = Err(error);
                false
            }
        })
        .map_err(|e| e.to_string())?;
        outcome
    }
}
