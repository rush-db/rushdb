//! Fallible reconstruction for native pinned readers. Legacy reconstruction
//! remains only until its consumers migrate; do not adapt errors into nulls.
use crate::reader::{try_access as access, TryRead};
use rushdb_storage::StorageError;
use rushdb_types::encoding::decode_order_preserving;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::occurrence::{OccurrenceValue, PropertyFamily};
use rushdb_types::value::{ID_FIELD, LABEL_FIELD, SOURCE_FIELD, TARGET_FIELD, TYPE_FIELD};
use serde_json::{Map, Value as Json};
use std::collections::{BTreeMap, HashSet};

fn id_string(id: &[u8; 16]) -> String {
    uuid::Uuid::from_bytes(*id).simple().to_string()
}

/// Bounded family-definition cache for a single reconstruction session.
/// Within one pinned `ReadView`, property-family definitions are frozen, so a
/// session that reconstructs many records can resolve each distinct family
/// once instead of once per occurrence. The cache is capped (bounded resident
/// structure); beyond the cap, lookups fall back to direct reads. A session
/// must not outlive its view.
const MAX_CACHED_FAMILIES: usize = 4096;

/// Property families resolved by reconstruction sessions. A family and a
/// pid's family never change once written, so one cache serves every
/// session over views of the same tenant (a statement reconstructing many
/// records one at a time keeps it between sessions).
#[derive(Default)]
pub struct FamilyCache {
    family_cache: std::collections::HashMap<u64, Option<PropertyFamily>>,
    pid_family: std::collections::HashMap<u64, u64>,
}

/// Fallible reconstruction session over one pinned read view, with a bounded
/// property-family cache.
pub struct ReconstructSession<'a, R: TryRead> {
    snap: &'a R,
    tenant: TenantId,
    family_cache: std::collections::HashMap<u64, Option<PropertyFamily>>,
    pid_family: std::collections::HashMap<u64, u64>,
}

impl<'a, R: TryRead> ReconstructSession<'a, R> {
    /// Creates a session bound to `snap` for `tenant`.
    #[must_use]
    pub fn new(snap: &'a R, tenant: TenantId) -> Self {
        Self::with_cache(snap, tenant, FamilyCache::default())
    }

    /// A session that starts from (and can hand back) `cache`.
    #[must_use]
    pub fn with_cache(snap: &'a R, tenant: TenantId, cache: FamilyCache) -> Self {
        ReconstructSession {
            snap,
            tenant,
            family_cache: cache.family_cache,
            pid_family: cache.pid_family,
        }
    }

    /// The families this session resolved, for the next session.
    #[must_use]
    pub fn into_cache(self) -> FamilyCache {
        FamilyCache {
            family_cache: self.family_cache,
            pid_family: self.pid_family,
        }
    }

    /// Resolve a property family, caching the definition for the session.
    fn family(&mut self, fid: u64) -> Result<PropertyFamily, StorageError> {
        if let Some(cached) = self.family_cache.get(&fid) {
            return cached.clone().ok_or(StorageError::CorruptLog);
        }
        let value = access::family(self.snap, self.tenant, fid)?;
        if self.family_cache.len() < MAX_CACHED_FAMILIES {
            self.family_cache.insert(fid, value.clone());
        }
        value.ok_or(StorageError::CorruptLog)
    }

    /// Reconstructs a record document from its live occurrences.
    ///
    /// # Errors
    /// Returns [`StorageError`] if a read fails or stored state is corrupt.
    pub fn reconstruct_record(&mut self, ordinal: u64) -> Result<Option<Json>, StorageError> {
        let Some(header) = access::record_by_ord(self.snap, self.tenant, ordinal)? else {
            return Ok(None);
        };
        let mut map = Map::new();
        map.insert(ID_FIELD.into(), Json::String(id_string(&header.record_id)));
        map.insert(LABEL_FIELD.into(), Json::String(header.label));
        self.reconstruct_owner(OwnerKind::Record, ordinal, &mut map)?;
        Ok(Some(Json::Object(map)))
    }

    /// Reconstructs only the requested record properties plus identity metadata.
    /// This keeps occurrence value decoding proportional to the projected columns.
    ///
    /// # Errors
    /// Returns [`StorageError`] if a read fails or stored state is corrupt.
    pub fn reconstruct_record_selected(
        &mut self,
        ordinal: u64,
        selected: &HashSet<String>,
    ) -> Result<Option<Json>, StorageError> {
        let Some(header) = access::record_by_ord(self.snap, self.tenant, ordinal)? else {
            return Ok(None);
        };
        let mut map = Map::new();
        map.insert(ID_FIELD.into(), Json::String(id_string(&header.record_id)));
        map.insert(LABEL_FIELD.into(), Json::String(header.label));
        self.reconstruct_owner_selected(OwnerKind::Record, ordinal, &mut map, selected)?;
        Ok(Some(Json::Object(map)))
    }

    /// Reconstructs a relationship document from its live occurrences.
    ///
    /// # Errors
    /// Returns [`StorageError`] if a read fails or stored state is corrupt.
    pub fn reconstruct_relationship(&mut self, ordinal: u64) -> Result<Option<Json>, StorageError> {
        let Some(header) = access::relationship_by_ord(self.snap, self.tenant, ordinal)? else {
            return Ok(None);
        };
        let mut map = Map::new();
        map.insert(
            ID_FIELD.into(),
            Json::String(id_string(&header.relationship_id)),
        );
        map.insert(TYPE_FIELD.into(), Json::String(header.rel_type));
        map.insert(
            SOURCE_FIELD.into(),
            Json::String(id_string(&header.source_id)),
        );
        map.insert(
            TARGET_FIELD.into(),
            Json::String(id_string(&header.target_id)),
        );
        self.reconstruct_owner(OwnerKind::Relationship, ordinal, &mut map)?;
        Ok(Some(Json::Object(map)))
    }

    /// Family of a pid, through the definition row (both cached per session).
    fn family_of_pid(&mut self, pid: u64) -> Result<PropertyFamily, StorageError> {
        if let Some(fid) = self.pid_family.get(&pid) {
            return self.family(*fid);
        }
        let def =
            access::definition(self.snap, self.tenant, pid)?.ok_or(StorageError::CorruptLog)?;
        if self.pid_family.len() < MAX_CACHED_FAMILIES {
            self.pid_family.insert(pid, def.family_id);
        }
        self.family(def.family_id)
    }

    /// Fold one owner's occurrences (one prefix scan, `(pid, elem)` order)
    /// into JSON fields, keeping only `selected` names when given.
    fn fold_occurrences(
        &mut self,
        occurrences: Vec<access::Occurrence>,
        out: &mut Map<String, Json>,
        selected: Option<&HashSet<String>>,
    ) -> Result<(), StorageError> {
        let mut by_pid: BTreeMap<u64, Vec<access::Occurrence>> = BTreeMap::new();
        for occ in occurrences {
            by_pid.entry(occ.pid).or_default().push(occ);
        }
        for (pid, rows) in by_pid {
            let family = self.family_of_pid(pid)?;
            if selected.is_some_and(|names| !names.contains(&family.display_name)) {
                continue;
            }
            let is_array = rows.iter().all(|row| row.array);
            let empty_array = is_empty_array_marker(&rows);
            let values = rows
                .iter()
                .map(|row| decode_value(row.value, &row.payload))
                .collect::<Result<Vec<_>, _>>()?;
            out.insert(
                family.display_name,
                if empty_array {
                    Json::Array(vec![])
                } else if is_array {
                    Json::Array(values)
                } else {
                    values.into_iter().next().unwrap_or(Json::Null)
                },
            );
        }
        Ok(())
    }

    fn reconstruct_owner_selected(
        &mut self,
        kind: OwnerKind,
        ordinal: u64,
        out: &mut Map<String, Json>,
        selected: &HashSet<String>,
    ) -> Result<(), StorageError> {
        let occurrences = access::occurrences_of_owner(self.snap, self.tenant, kind, ordinal)?;
        self.fold_occurrences(occurrences, out, Some(selected))
    }

    fn reconstruct_owner(
        &mut self,
        kind: OwnerKind,
        ordinal: u64,
        out: &mut Map<String, Json>,
    ) -> Result<(), StorageError> {
        let occurrences = access::occurrences_of_owner(self.snap, self.tenant, kind, ordinal)?;
        self.fold_occurrences(occurrences, out, None)
    }

    /// Bounded record reconstruction: like [`Self::reconstruct_record`], but
    /// rejects owners exceeding `max_occurrences` live occurrences.
    ///
    /// # Errors
    /// Returns [`StorageError::ResourceExhausted`] if the owner has too many
    /// occurrences, or [`StorageError`] if a read fails or state is corrupt.
    pub fn reconstruct_record_limited(
        &mut self,
        ordinal: u64,
        max_occurrences: usize,
    ) -> Result<Option<Json>, StorageError> {
        let Some(header) = access::record_by_ord(self.snap, self.tenant, ordinal)? else {
            return Ok(None);
        };
        let occurrences =
            access::occurrences_of_owner(self.snap, self.tenant, OwnerKind::Record, ordinal)?;
        if occurrences.len() > max_occurrences {
            return Err(StorageError::ResourceExhausted(format!(
                "owner {:?}/{ordinal} exceeds {max_occurrences} occurrences",
                OwnerKind::Record
            )));
        }
        let mut map = Map::new();
        map.insert(ID_FIELD.into(), Json::String(id_string(&header.record_id)));
        map.insert(LABEL_FIELD.into(), Json::String(header.label));
        self.fold_occurrences(occurrences, &mut map, None)?;
        Ok(Some(Json::Object(map)))
    }
}

/// True when a property's occurrences are exactly the single payload-free
/// empty-array marker (stored at elem 0 for a zero-element array).
#[must_use]
pub fn is_empty_array_marker(rows: &[access::Occurrence]) -> bool {
    rows.len() == 1 && rows[0].elem == 0 && matches!(rows[0].value, OccurrenceValue::EmptyArray)
}

/// Reconstructs a record document from `snap` using a fresh session.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or stored state is corrupt.
pub fn reconstruct_record<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    ordinal: u64,
) -> Result<Option<Json>, StorageError> {
    let mut session = ReconstructSession::new(snap, tenant);
    session.reconstruct_record(ordinal)
}

/// Reconstructs a relationship document from `snap` using a fresh session.
///
/// # Errors
/// Returns [`StorageError`] if a read fails or stored state is corrupt.
pub fn reconstruct_relationship<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    ordinal: u64,
) -> Result<Option<Json>, StorageError> {
    let mut session = ReconstructSession::new(snap, tenant);
    session.reconstruct_relationship(ordinal)
}

/// Bounded record reconstruction: like [`reconstruct_record`], but caps
/// the number of occurrences an owner may contribute. A record with more than
/// `max_occurrences` live occurrences is rejected with
/// [`StorageError::ResourceExhausted`] instead of being materialized in full,
/// so a pathological fan-out cannot balloon query working memory.
///
/// # Errors
/// Returns [`StorageError::ResourceExhausted`] if the owner has too many
/// occurrences, or [`StorageError`] if a read fails or state is corrupt.
pub fn reconstruct_record_limited<R: TryRead>(
    snap: &R,
    tenant: TenantId,
    ordinal: u64,
    max_occurrences: usize,
) -> Result<Option<Json>, StorageError> {
    let mut session = ReconstructSession::new(snap, tenant);
    session.reconstruct_record_limited(ordinal, max_occurrences)
}

/// Decodes one occurrence value into JSON.
///
/// # Errors
/// Returns [`StorageError::CorruptLog`] if a stored payload cannot be decoded.
pub fn decode_value(value: OccurrenceValue, payload: &[u8]) -> Result<Json, StorageError> {
    use rushdb_types::value::Scalar;
    match value {
        OccurrenceValue::ExplicitNull | OccurrenceValue::EmptyArray => Ok(Json::Null),
        OccurrenceValue::Stored { ty } => {
            let scalar =
                decode_order_preserving(ty, payload).map_err(|_| StorageError::CorruptLog)?;
            Ok(match scalar {
                Scalar::Boolean(b) => Json::Bool(b),
                Scalar::Int64(i) | Scalar::DateTime(i) => Json::Number(i.into()),
                Scalar::Float64(f) => {
                    serde_json::Number::from_f64(f).map_or(Json::Null, Json::Number)
                }
                Scalar::Utf8(s) => Json::String(s),
            })
        }
    }
}
