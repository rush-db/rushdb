//! The record-set join behind `linkRecords`, `unlinkRecords` and the join
//! query: the `(source, target)` pairs of two selectors, either every pair
//! (cartesian) or the pairs whose key properties share a value.
//!
//! Both sides are resolved to ordinals, never to rebuilt records. For a key
//! join the smaller side's key values are held in a map (value → owners)
//! charged against `query-memory-max`, and the larger side streams past it
//! reading only its key property. Pairs reach the caller in chunks, so
//! memory follows the smaller side and one chunk, never the whole join, and
//! a writer can commit each chunk on its own.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::working_set::WorkingSet;
use super::RequestScope;

/// Pairs handed to the caller at a time (the batch link and unlink commit).
pub(super) const JOIN_CHUNK: usize = 10_000;

/// What a join hands each chunk of pairs to; `false` stops the join.
pub(super) type ChunkVisit<'v> = dyn FnMut(&[(u64, u64)]) -> Result<bool, String> + 'v;

/// Bytes one ordinal holds in a side's ordinal list.
const ORDINAL_BYTES: u64 = 8;

/// One side of a join: the records of `label` matching `predicate` (all
/// when empty), keyed by property `key` in a key join.
pub(super) struct JoinSide<'a> {
    pub(super) label: &'a str,
    pub(super) predicate: &'a serde_json::Value,
    pub(super) key: Option<&'a str>,
}

impl RequestScope<'_> {
    /// The ordinals of the records `label` holds that match `predicate` (all
    /// when it is empty or absent), ascending, at `snap`.
    pub(super) fn selector_ordinals(
        &self,
        label: &str,
        predicate: Option<&serde_json::Value>,
        snap: &Arc<rushdb_storage::ReadView>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u64>, String> {
        let mut document = serde_json::json!({ "labels": [label] });
        if let Some(predicate) = predicate.filter(|p| p.as_object().is_none_or(|o| !o.is_empty())) {
            document["where"] = predicate.clone();
        }
        let parsed =
            rushdb_query::document::parse_document(&document).map_err(|e| e.to_string())?;
        rushdb_query::matching_ordinals(
            &self.lmpg(),
            &parsed.search,
            is_cancelled,
            Arc::clone(snap),
        )
        .map_err(|e| e.to_string())
    }

    /// Visit the pairs of `source` × `target` at `snap` in chunks of up to
    /// [`JOIN_CHUNK`]: every pair (without keys; `exclude_self` drops a
    /// record paired with itself) or each pair whose key values overlap
    /// (type-strict; an array key contributes each element), each pair
    /// once. `visit` returns `false` to stop; `meter` holds the working set
    /// and the request's cancellation probe.
    pub(super) fn for_each_join_chunk(
        &self,
        snap: &Arc<rushdb_storage::ReadView>,
        source: &JoinSide<'_>,
        target: &JoinSide<'_>,
        exclude_self: bool,
        meter: &mut WorkingSet<'_>,
        visit: &mut ChunkVisit<'_>,
    ) -> Result<(), String> {
        let is_cancelled = meter.is_cancelled();
        let sources =
            self.selector_ordinals(source.label, Some(source.predicate), snap, is_cancelled)?;
        meter.charge(ORDINAL_BYTES.saturating_mul(sources.len() as u64))?;
        let targets =
            self.selector_ordinals(target.label, Some(target.predicate), snap, is_cancelled)?;
        meter.charge(ORDINAL_BYTES.saturating_mul(targets.len() as u64))?;
        let mut chunk = Chunk {
            pairs: Vec::new(),
            visit,
            stopped: false,
        };
        let (Some(source_key), Some(target_key)) = (source.key, target.key) else {
            for &s in &sources {
                meter.check()?;
                for &t in &targets {
                    if (!exclude_self || s != t) && !chunk.push(s, t)? {
                        return Ok(());
                    }
                }
            }
            return chunk.flush().map(|_| ());
        };
        // Hold the smaller side's keys; stream the larger side past them.
        let source_keyed = sources.len() <= targets.len();
        let (keyed, keyed_key, probe, probe_key) = if source_keyed {
            (&sources, source_key, &targets, target_key)
        } else {
            (&targets, target_key, &sources, source_key)
        };
        let mut reader = KeyReader::new(snap, self.tenant());
        let mut owners: HashMap<String, Vec<u64>> = HashMap::new();
        for &ord in keyed {
            for value in reader.values(ord, keyed_key)? {
                meter.charge(super::working_set::value_bytes(value.len()) + ORDINAL_BYTES)?;
                owners.entry(value).or_default().push(ord);
            }
        }
        let mut seen = HashSet::new();
        for &p in probe {
            meter.check()?;
            seen.clear();
            for value in reader.values(p, probe_key)? {
                for &k in owners.get(&value).map_or(&[][..], Vec::as_slice) {
                    if !seen.insert(k) || (exclude_self && k == p) {
                        continue;
                    }
                    let (s, t) = if source_keyed { (k, p) } else { (p, k) };
                    if !chunk.push(s, t)? {
                        return Ok(());
                    }
                }
            }
        }
        chunk.flush().map(|_| ())
    }
}

/// The pairs gathered for the next `visit`.
struct Chunk<'v> {
    pairs: Vec<(u64, u64)>,
    visit: &'v mut ChunkVisit<'v>,
    stopped: bool,
}

impl Chunk<'_> {
    /// Add a pair, handing a full chunk over; `false` once `visit` stopped.
    fn push(&mut self, source: u64, target: u64) -> Result<bool, String> {
        self.pairs.push((source, target));
        if self.pairs.len() >= JOIN_CHUNK {
            return self.flush();
        }
        Ok(true)
    }

    /// Hand the gathered pairs over; `false` once `visit` stopped.
    fn flush(&mut self) -> Result<bool, String> {
        if !self.stopped && !self.pairs.is_empty() {
            self.stopped = !(self.visit)(&self.pairs)?;
            self.pairs.clear();
        }
        Ok(!self.stopped)
    }
}

/// Reads one key property of a record: its values as JSON text (an array
/// gives each element), which is equal for two values exactly when the
/// values are (the same type and the same value).
struct KeyReader<'v> {
    session: rushdb_lmpg::reconstruct::ReconstructSession<'v, rushdb_storage::ReadView>,
    selected: HashMap<String, HashSet<String>>,
}

impl<'v> KeyReader<'v> {
    fn new(snap: &'v rushdb_storage::ReadView, tenant: rushdb_types::ids::TenantId) -> Self {
        Self {
            session: rushdb_lmpg::reconstruct::ReconstructSession::new(snap, tenant),
            selected: HashMap::new(),
        }
    }

    fn values(&mut self, ord: u64, key: &str) -> Result<Vec<String>, String> {
        let selected = self
            .selected
            .entry(key.to_owned())
            .or_insert_with(|| HashSet::from([key.to_owned()]));
        let Some(serde_json::Value::Object(record)) = self
            .session
            .reconstruct_record_selected(ord, selected)
            .map_err(|e| e.to_string())?
        else {
            return Ok(Vec::new());
        };
        Ok(match record.get(key) {
            Some(serde_json::Value::Array(items)) => {
                items.iter().map(ToString::to_string).collect()
            }
            Some(value) => vec![value.to_string()],
            None => Vec::new(),
        })
    }
}
