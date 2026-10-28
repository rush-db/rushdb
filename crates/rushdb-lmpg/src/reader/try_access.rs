//! Fallible LMPG primitives over storage format v3. Corrupt rows are
//! errors, not missing records. Eager adjacency remains subject to the Txn
//! read cap.
use super::{
    keys_ext::{
        adj_pair_prefix, adj_prefix_typed, definition_row_key, family_key, rel_by_ord_key,
        rel_row_key,
    },
    TryRead,
};
use crate::derived;
use rushdb_storage::StorageError;
use rushdb_types::ids::{OwnerKind, RecordId, RelationshipId, TenantId};
use rushdb_types::keys::{self as k, key, u64be};
use rushdb_types::occurrence::{
    decode_occ_value, OccurrenceRow, OccurrenceValue, PropertyDefinition, PropertyFamily,
    RecordHeader, RelationshipHeader,
};

/// An adjacency edge: `(relationship type, other node ordinal, relationship ordinal)`.
pub type Edge = (String, u64, u64);

fn decode<T: serde::de::DeserializeOwned>(raw: Option<Vec<u8>>) -> Result<Option<T>, StorageError> {
    raw.map(|raw| serde_json::from_slice(&raw).map_err(|_| StorageError::CorruptLog))
        .transpose()
}

/// One canonical occurrence: identity `(kind, owner, pid, elem)` plus
/// its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// Owner kind.
    pub kind: OwnerKind,
    /// Owner ordinal.
    pub owner: u64,
    /// Property id.
    pub pid: u64,
    /// Element slot (`0` for scalars).
    pub elem: u64,
    /// Whether the occurrence is an array element.
    pub array: bool,
    /// Value state.
    pub value: OccurrenceValue,
    /// Order-preserving payload of a stored scalar (empty otherwise).
    pub payload: Vec<u8>,
}

impl Occurrence {
    fn decode(key: &[u8], raw: &[u8]) -> Result<Self, StorageError> {
        let (_, kind, owner, pid, elem) = k::parse_occ_key(key).ok_or(StorageError::CorruptLog)?;
        let value = decode_occ_value(raw).map_err(|_| StorageError::CorruptLog)?;
        Ok(Self {
            kind,
            owner,
            pid,
            elem,
            array: value.array,
            value: value.value,
            payload: value.payload.to_vec(),
        })
    }

    /// Canonical value hash.
    #[must_use]
    pub fn value_hash(&self) -> u64 {
        match self.value {
            OccurrenceValue::Stored { ty } => rushdb_types::hash::value_hash(
                rushdb_types::encoding::logical_tag(ty),
                &self.payload,
            ),
            OccurrenceValue::ExplicitNull | OccurrenceValue::EmptyArray => {
                rushdb_types::hash::null_value_hash()
            }
        }
    }

    /// The command-vocabulary row for this occurrence.
    #[must_use]
    pub fn to_row(&self, family_id: u64) -> OccurrenceRow {
        OccurrenceRow {
            owner_kind: self.kind,
            owner_ord: self.owner,
            property_id: self.pid,
            family_id,
            elem_ordinal: self.array.then_some(self.elem),
            occurrence_value: self.value,
        }
    }
}

fn collect_occurrences<R: TryRead + ?Sized>(
    r: &R,
    prefix: &[u8],
) -> Result<Vec<Occurrence>, StorageError> {
    r.range_prefix(prefix)?
        .into_iter()
        .map(|(key, raw)| Occurrence::decode(&key, &raw))
        .collect()
}

/// Every occurrence of one owner, in `(pid, elem)` order: one prefix scan.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a row is corrupt.
pub fn occurrences_of_owner<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    owner: u64,
) -> Result<Vec<Occurrence>, StorageError> {
    collect_occurrences(r, &k::occ_owner_prefix(t, kind, owner))
}

/// Every element of one owner's pid, in element order.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a row is corrupt.
pub fn occurrences_of_pid<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    owner: u64,
    pid: u64,
) -> Result<Vec<Occurrence>, StorageError> {
    collect_occurrences(r, &k::occ_pid_prefix(t, kind, owner, pid))
}

/// One occurrence slot.
///
/// # Errors
/// Returns [`StorageError`] if the read fails or the row is corrupt.
pub fn occurrence<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    kind: OwnerKind,
    owner: u64,
    pid: u64,
    elem: u64,
) -> Result<Option<Occurrence>, StorageError> {
    let occ_key = k::occ_key(t, kind, owner, pid, elem);
    r.get(&occ_key)?
        .map(|raw| Occurrence::decode(&occ_key, &raw))
        .transpose()
}

/// Looks up a record header by public id: `RECORD_ID` then `RECORD_HDR`.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is corrupt.
pub fn record_by_id<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    id: &RecordId,
) -> Result<Option<RecordHeader>, StorageError> {
    let Some(raw) = r.get(&k::record_id_key(t, &id.0))? else {
        return Ok(None);
    };
    let ordinal = u64::from_be_bytes(raw.try_into().map_err(|_| StorageError::CorruptLog)?);
    record_by_ord(r, t, ordinal)
}

/// Looks up a record header by storage ordinal (one point read).
///
/// # Errors
/// Returns [`StorageError`] if the read fails or the row is corrupt.
pub fn record_by_ord<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    ord: u64,
) -> Result<Option<RecordHeader>, StorageError> {
    let Some(raw) = r.get(&k::record_hdr_key(t, ord))? else {
        return Ok(None);
    };
    let (record_id, label) = k::decode_record_hdr(&raw).ok_or(StorageError::CorruptLog)?;
    Ok(Some(RecordHeader {
        record_id,
        ordinal: ord,
        label: label.to_owned(),
    }))
}

/// Looks up a property family by family id.
///
/// # Errors
/// Returns [`StorageError`] if the read fails or the row is corrupt.
pub fn family<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    id: u64,
) -> Result<Option<PropertyFamily>, StorageError> {
    decode(r.get(&family_key(t, id))?)
}

/// Looks up a property definition row by property id.
///
/// # Errors
/// Returns [`StorageError`] if the read fails or the row is corrupt.
pub fn definition<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    id: u64,
) -> Result<Option<PropertyDefinition>, StorageError> {
    decode(r.get(&definition_row_key(t, id))?)
}

/// Owner kind of a property id (from its family's owner scope), or `None`
/// when the pid is unknown.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or a row is corrupt.
pub fn kind_of_pid<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    pid: u64,
) -> Result<Option<OwnerKind>, StorageError> {
    let Some(def) = definition(r, t, pid)? else {
        return Ok(None);
    };
    Ok(family(r, t, def.family_id)?.map(|family| family.owner_scope))
}

/// Every property definition belonging to a family.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a definition row is corrupt.
pub fn definitions_of_family<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    id: u64,
) -> Result<Vec<PropertyDefinition>, StorageError> {
    let mut out = Vec::new();
    for (_, raw) in r.range_prefix(&key(t, &[b"property-row"]))? {
        let def: PropertyDefinition =
            serde_json::from_slice(&raw).map_err(|_| StorageError::CorruptLog)?;
        if def.family_id == id {
            out.push(def);
        }
    }
    Ok(out)
}

/// Record ordinals currently carrying `label`, ascending.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a membership key is corrupt.
pub fn label_members<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    label: &str,
) -> Result<Vec<u64>, StorageError> {
    let prefix = k::label_member_prefix(t, label);
    r.range_prefix(&prefix)?
        .into_iter()
        .map(|(key, _)| k::read_ord_tail(&key, prefix.len()).ok_or(StorageError::CorruptLog))
        .collect()
}

/// Live labels in first-appearance order (oldest live record first).
///
/// # Errors
/// Returns [`StorageError`] if the counters cannot be read or are malformed.
pub fn labels<R: TryRead + ?Sized>(r: &R, t: TenantId) -> Result<Vec<String>, StorageError> {
    Ok(derived::label_catalog(r, t)?
        .into_iter()
        .map(|(label, _)| label)
        .collect())
}

/// Per-label live record counts, in first-appearance order.
///
/// # Errors
/// Returns [`StorageError`] if the counters cannot be read or are malformed.
pub fn label_counts<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
) -> Result<Vec<(String, u64)>, StorageError> {
    derived::label_catalog(r, t)
}

/// Total live owner support of a property (its owner kind resolved from the
/// family).
///
/// # Errors
/// Returns a storage error when a counter is corrupt or cannot be read.
pub fn prop_support_total<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    pid: u64,
) -> Result<u64, StorageError> {
    match kind_of_pid(r, t, pid)? {
        Some(kind) => derived::prop_support_total(r, t, kind, pid),
        None => Ok(0),
    }
}

/// Per-label live owner counts of one property id, ordered by label.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a counter is corrupt.
pub fn prop_support_label_counts<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    pid: u64,
) -> Result<Vec<(String, u64)>, StorageError> {
    match kind_of_pid(r, t, pid)? {
        Some(kind) => derived::prop_support_by_label(r, t, kind, pid),
        None => Ok(Vec::new()),
    }
}

/// One label-scoped support count.
///
/// # Errors
/// Returns [`StorageError`] when the counter is malformed or the read fails.
pub fn prop_support_label<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    pid: u64,
    label: &str,
) -> Result<u64, StorageError> {
    match kind_of_pid(r, t, pid)? {
        Some(kind) => derived::prop_support_label(r, t, kind, pid, label),
        None => Ok(0),
    }
}

/// Relationship catalog rows `(type, source_label, target_label, count)`.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a counter is corrupt.
pub fn relationship_catalog<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
) -> Result<Vec<(String, String, String, u64)>, StorageError> {
    derived::rel_catalog(r, t)
}

/// Relationship-property owner counts for one shape `(type, source label,
/// target label)`: `(pid, count)`.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or a counter is corrupt.
pub fn rel_prop_counts<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    (rel_type, source, target): (&str, &str, &str),
) -> Result<Vec<(u64, u64)>, StorageError> {
    derived::rel_shape_prop_counts(r, t, rel_type, source, target)
}

/// Looks up a relationship header by public id.
///
/// # Errors
/// Returns [`StorageError`] if the read fails or the row is corrupt.
pub fn relationship_by_id<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    id: &RelationshipId,
) -> Result<Option<RelationshipHeader>, StorageError> {
    decode(r.get(&rel_row_key(t, &id.0))?)
}

/// Looks up a relationship header by storage ordinal.
///
/// # Errors
/// Returns [`StorageError`] if the read fails or an id row is corrupt.
pub fn relationship_by_ord<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    ord: u64,
) -> Result<Option<RelationshipHeader>, StorageError> {
    let Some(raw) = r.get(&rel_by_ord_key(t, ord))? else {
        return Ok(None);
    };
    let id = raw
        .as_slice()
        .try_into()
        .map_err(|_| StorageError::CorruptLog)?;
    relationship_by_id(r, t, &RelationshipId(id))
}

/// True when a directed edge of `rel_type` already runs from `node` to `other`.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails.
pub fn edge_exists<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    node: u64,
    rel_type: &str,
    other: u64,
) -> Result<bool, StorageError> {
    let prefix = adj_pair_prefix(t, b"out", node, rel_type, other);
    for (key, _) in r.range_prefix(&prefix)? {
        let Some(rest) = key.strip_prefix(prefix.as_slice()) else {
            continue;
        };
        // Stored edge keys append `/` and the 8-byte relationship ordinal.
        if rest.len() == 9 && rest[0] == b'/' {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Adjacency scan: outgoing edges of a node, optionally filtered by type.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or an adjacency key is corrupt.
pub fn adj_out<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    node: u64,
    rel_type: Option<&str>,
) -> Result<Vec<Edge>, StorageError> {
    adjacency(r, t, b"out", node, rel_type)
}

/// Adjacency scan: incoming edges of a node, optionally filtered by type.
///
/// # Errors
/// Returns [`StorageError`] if the scan fails or an adjacency key is corrupt.
pub fn adj_in<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    node: u64,
    rel_type: Option<&str>,
) -> Result<Vec<Edge>, StorageError> {
    adjacency(r, t, b"in", node, rel_type)
}

fn adjacency<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    dir: &[u8],
    node: u64,
    rel_type: Option<&str>,
) -> Result<Vec<Edge>, StorageError> {
    let base = match rel_type {
        Some(rt) => adj_prefix_typed(t, dir, node, rt),
        None => key(t, &[b"adj", dir, &u64be(node)]),
    };
    parse_adjacency(&base, r.range_prefix(&base)?, rel_type)
}

/// Every record adjacent to `node` (either direction, any type, ascending,
/// without repeats), or `None` when it has more than `limit` relationships.
///
/// # Errors
/// Returns [`StorageError`] if a scan fails or an adjacency key is corrupt.
pub fn neighbours_limited<R: TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    node: u64,
    limit: usize,
) -> Result<Option<Vec<u64>>, StorageError> {
    let mut out = Vec::new();
    for dir in [b"out".as_slice(), b"in".as_slice()] {
        let base = key(t, &[b"adj", dir, &u64be(node)]);
        let rows = r.range_prefix_limited(&base, limit.saturating_add(1))?;
        if rows.len() > limit {
            return Ok(None);
        }
        out.extend(
            parse_adjacency(&base, rows, None)?
                .into_iter()
                .map(|(_, other, _)| other),
        );
        if out.len() > limit {
            return Ok(None);
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(Some(out))
}

fn parse_adjacency(
    base: &[u8],
    rows: Vec<(Vec<u8>, Vec<u8>)>,
    rel_type: Option<&str>,
) -> Result<Vec<Edge>, StorageError> {
    let mut edges = Vec::new();
    for (key, _) in rows {
        let rest = key.strip_prefix(base).ok_or(StorageError::CorruptLog)?;
        let (rel_type, tail) = match rel_type {
            Some(rt) if rest.len() == 18 && rest[0] == b'/' && rest[9] == b'/' => {
                (rt.to_owned(), &rest[1..])
            }
            None if rest.len() >= 19
                && rest[0] == b'/'
                && rest[rest.len() - 18] == b'/'
                && rest[rest.len() - 9] == b'/' =>
            {
                let name = std::str::from_utf8(&rest[1..rest.len() - 18])
                    .map_err(|_| StorageError::CorruptLog)?;
                (name.to_owned(), &rest[rest.len() - 17..])
            }
            // An adjacent prefix (e.g. a longer type name) is not a row in the
            // requested exact type. Match the existing typed scan contract.
            _ => continue,
        };
        let other = u64::from_be_bytes(tail[..8].try_into().map_err(|_| StorageError::CorruptLog)?);
        let ordinal =
            u64::from_be_bytes(tail[9..].try_into().map_err(|_| StorageError::CorruptLog)?);
        edges.push((rel_type, other, ordinal));
    }
    edges.sort_by_key(|(_, _, ordinal)| *ordinal);
    Ok(edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Map(BTreeMap<Vec<u8>, Vec<u8>>);

    impl TryRead for Map {
        fn get(&self, k: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
            Ok(self.0.get(k).cloned())
        }
        fn range_prefix(&self, p: &[u8]) -> Result<super::super::KeyValueRows, StorageError> {
            Ok(self
                .0
                .iter()
                .filter(|(key, _)| key.starts_with(p))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect())
        }
        fn range(
            &self,
            start: &[u8],
            end: &[u8],
        ) -> Result<super::super::KeyValueRows, StorageError> {
            let end = if end.is_empty() {
                std::ops::Bound::Unbounded
            } else {
                std::ops::Bound::Excluded(end)
            };
            Ok(self
                .0
                .range::<[u8], _>((std::ops::Bound::Included(start), end))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect())
        }
    }

    #[test]
    fn record_by_ord_reads_the_header_row() {
        let t = TenantId::from_ordinal(5);
        let mut map = BTreeMap::new();
        map.insert(
            k::record_hdr_key(t, 3),
            k::encode_record_hdr(&[7u8; 16], "Doc"),
        );
        assert_eq!(
            record_by_ord(&Map(map), t, 3).unwrap(),
            Some(RecordHeader {
                record_id: [7u8; 16],
                ordinal: 3,
                label: "Doc".into(),
            })
        );
    }

    #[test]
    fn record_by_ord_rejects_a_malformed_header() {
        let t = TenantId::from_ordinal(5);
        let mut map = BTreeMap::new();
        map.insert(k::record_hdr_key(t, 3), vec![1, 2, 3]);
        assert!(matches!(
            record_by_ord(&Map(map), t, 3),
            Err(StorageError::CorruptLog)
        ));
    }

    #[test]
    fn occurrences_reject_malformed_values() {
        let t = TenantId::from_ordinal(5);
        let mut map = BTreeMap::new();
        map.insert(k::occ_key(t, OwnerKind::Record, 1, 9, 0), vec![0]);
        assert!(matches!(
            occurrences_of_owner(&Map(map), t, OwnerKind::Record, 1),
            Err(StorageError::CorruptLog)
        ));
    }

    #[test]
    fn relationship_catalog_rejects_malformed_counters() {
        let t = TenantId::from_ordinal(5);
        let mut map = BTreeMap::new();
        map.insert(
            k::rel_catalog_key(t, "KNOWS", "Person", "Person"),
            vec![1, 2, 3],
        );
        assert!(matches!(
            relationship_catalog(&Map(map), t),
            Err(StorageError::CorruptLog)
        ));
    }

    #[test]
    fn store_implements_the_fallible_reader_boundary() {
        let store = rushdb_storage::Store::new_in_memory();
        let t = TenantId::from_ordinal(5);
        assert_eq!(prop_support_label_counts(&store, t, 7).unwrap(), Vec::new());
    }
}
