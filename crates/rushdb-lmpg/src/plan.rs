use std::collections::{BTreeSet, HashMap};

use rushdb_storage::Txn;
use rushdb_types::command::{Mutation, Precondition};
use rushdb_types::encoding::{encode_order_preserving, logical_tag};
use rushdb_types::ids::{FamilyId, OwnerKind, PropertyId, TenantId};
use rushdb_types::keys::key;
use rushdb_types::model::{Cardinality, LogicalType, Nullability, TypeState};
use rushdb_types::occurrence::{
    OccurrenceRow, OccurrenceValue, PropertyDefinition, PropertyFamily,
};
use rushdb_types::value::{infer_scalar, normalize_name};
use serde_json::Value as Json;

/// Namespace used for all property families created by the planner.
pub const DEFAULT_NAMESPACE: &str = "default";

/// One planned write to a property: set, typed datetime, explicit null, or
/// delete.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FieldChange {
    /// Set the property to the supplied JSON value.
    Set(Json),
    /// Typed datetime write: epoch ms stored as `LogicalType::DateTime`
    /// (produced by ISO date/datetime inference under suggestTypes).
    SetDatetime(i64),
    /// Write an explicit null occurrence (distinct from absence).
    ExplicitNull,
    /// Remove all occurrences of the property for the owner.
    Delete,
}

/// Value payload at the mutation boundary: explicit Null (payload-free),
/// a single typed scalar, or an ordered array decomposed into element
/// occurrences.
#[derive(Debug, Clone)]
pub enum ValuePayload {
    /// A single explicit-null occurrence.
    Null,
    /// A single scalar or object value.
    Single(Json),
    /// An ordered array decomposed into element occurrences.
    Array(Vec<Json>),
    /// Typed datetime element payload (epoch ms).
    DateTime(i64),
}

fn single_element(v: &Json) -> Vec<Json> {
    vec![v.clone()]
}

/// Errors raised while planning a mutation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// A storage read or decode failed during planning.
    #[error("storage read failed: {0}")]
    Storage(String),
    /// A write conflicts with the definition's resolved type.
    #[error("type mismatch on `{field}`: expected {expected}, got {found}")]
    TypeMismatch {
        /// Field display name.
        field: String,
        /// Type required by the definition.
        expected: String,
        /// Type inferred from the incoming value.
        found: String,
    },
    /// An explicit null was rejected by a `NotNull` definition.
    #[error("explicit null rejected under NotNull definition `{0}`")]
    NullRejected(String),
    /// An array contains elements that cannot share one logical type.
    #[error("unsupported mixed-type array on `{0}`")]
    MixedArray(String),
    /// A referenced family or definition is missing.
    #[error("missing entity: {0}")]
    MissingEntity(String),
    /// A family or definition already exists where one was not expected.
    #[error("duplicate entity: {0}")]
    DuplicateEntity(String),
    /// A write names a property the engine reserves (`__*`).
    #[error("invalid property name `{0}`: names starting with `__` are reserved for the engine")]
    ReservedName(String),
}

impl From<rushdb_storage::StorageError> for PlanError {
    fn from(error: rushdb_storage::StorageError) -> Self {
        Self::Storage(error.to_string())
    }
}

fn decode_u64(raw: &[u8]) -> Result<u64, PlanError> {
    raw.try_into()
        .map(u64::from_be_bytes)
        .map_err(|_| PlanError::Storage("invalid u64 metadata".into()))
}

/// Tracks allocator positions across the mutations planned in one batch and
/// accumulates the deterministic mutation set for the `CommitBatch`.
pub struct Planner<'a> {
    /// Tenant whose keys are planned against.
    pub tenant: TenantId,
    pub(crate) txn: &'a Txn<'a>,
    allocs: HashMap<&'static str, u64>,
    /// Mutations staged so far, in planning order.
    pub mutations: Vec<Mutation>,
    /// Preconditions accumulated for the commit.
    pub preconditions: Vec<Precondition>,
    pending_families: HashMap<(String, u8, String), u64>,
    pending_defs: HashMap<(u64, &'static str, u8), u64>,
    /// Definition slots known to be absent in this batch, so the type-state
    /// probes of a property run once per batch, not once per write.
    absent_defs: std::collections::HashSet<(u64, &'static str, u8)>,
    /// Owners whose ordinal this planner allocated: they hold no committed
    /// occurrences, so writes to them skip the read-before-delete probes.
    new_owners: std::collections::HashSet<(OwnerKind, u64)>,
    /// Occurrence slots `(kind, owner, pid, elem)` already staged for
    /// deletion in this batch.
    pub deleted_occurrences: BTreeSet<(OwnerKind, u64, u64, u64)>,
}

impl<'a> Planner<'a> {
    /// Creates a planner over `txn` for `tenant` with empty pending state.
    #[must_use]
    pub fn new(tenant: TenantId, txn: &'a Txn<'a>) -> Self {
        Planner {
            tenant,
            txn,
            allocs: HashMap::new(),
            mutations: Vec::new(),
            preconditions: Vec::new(),
            pending_families: HashMap::new(),
            pending_defs: HashMap::new(),
            absent_defs: std::collections::HashSet::new(),
            new_owners: std::collections::HashSet::new(),
            deleted_occurrences: BTreeSet::new(),
        }
    }

    /// Allocates the next ordinal for the named counter, reading and caching
    /// the committed counter on first use within this batch.
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if the counter key cannot be read or
    /// decoded.
    pub fn next_alloc(&mut self, name: &'static str) -> Result<u64, PlanError> {
        let entry = self.allocs.entry(name);
        match entry {
            std::collections::hash_map::Entry::Occupied(mut o) => {
                let v = *o.get();
                *o.get_mut() += 1;
                Ok(v)
            }
            std::collections::hash_map::Entry::Vacant(vac) => {
                let ckey = crate::expand::meta_counter_key(self.tenant, name);
                let cur = self.txn.get(&ckey)?.map(|b| decode_u64(&b)).transpose()?;
                let next = cur.unwrap_or(0);
                vac.insert(next + 1);
                Ok(next)
            }
        }
    }

    /// Allocate a new record or relationship ordinal and remember it as
    /// created by this batch.
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if the ordinal counter cannot be read.
    pub fn next_owner(&mut self, kind: OwnerKind) -> Result<u64, PlanError> {
        let ordinal = self.next_alloc(match kind {
            OwnerKind::Record => "record",
            OwnerKind::Relationship => "relationship",
        })?;
        self.new_owners.insert((kind, ordinal));
        Ok(ordinal)
    }

    /// Resolve-or-create the Property Family for `(namespace, scope, name)`.
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if the family signature index cannot be
    /// read or decoded.
    pub fn resolve_family(
        &mut self,
        namespace: &str,
        scope: OwnerKind,
        display_name: &str,
    ) -> Result<(PropertyFamily, bool), PlanError> {
        let norm = normalize_name(display_name);
        if rushdb_types::value::is_reserved_name(&norm) {
            return Err(PlanError::ReservedName(display_name.to_owned()));
        }
        let pk = (namespace.to_string(), scope.as_byte(), norm.clone());
        if let Some(fid) = self.pending_families.get(&pk) {
            return Ok((
                PropertyFamily {
                    family_id: *fid,
                    namespace: namespace.to_string(),
                    owner_scope: scope,
                    normalized_name: norm,
                    display_name: display_name.to_string(),
                },
                true,
            ));
        }

        let sig = key(
            self.tenant,
            &[
                rushdb_types::keys::P_FAMILY_SIG,
                namespace.as_bytes(),
                &[scope.as_byte()],
                norm.as_bytes(),
            ],
        );
        if let Some(raw) = self.txn.get(&sig)? {
            let fid = FamilyId(decode_u64(&raw)?);
            self.pending_families.insert(pk, fid.0);
            return Ok((
                PropertyFamily {
                    family_id: fid.0,
                    namespace: namespace.into(),
                    owner_scope: scope,
                    normalized_name: norm,
                    display_name: display_name.to_string(),
                },
                false,
            ));
        }

        // CommitBatch recorded within the loop below, functionally identical
        // structure in either branch.
        let fid = FamilyId(self.next_alloc("family")?);
        self.mutations.push(Mutation::EnsureFamily {
            family_id: fid.0,
            namespace: namespace.into(),
            owner_scope: scope,
            normalized_name: norm.clone(),
            display_name: display_name.into(),
        });
        self.pending_families.insert(pk, fid.0);
        Ok((
            PropertyFamily {
                family_id: fid.0,
                namespace: namespace.into(),
                owner_scope: scope,
                normalized_name: norm,
                display_name: display_name.into(),
            },
            false,
        ))
    }

    fn family_def_slot(state: TypeState) -> (&'static str, u8) {
        match state {
            TypeState::Unresolved => ("U", 0),
            TypeState::Known(ty) => ("K", logical_tag(ty)),
        }
    }

    /// Looks up the definition for `(fid, state)` in pending state or committed
    /// storage, caching a hit for the rest of the batch.
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if the definition index cannot be read,
    /// decoded, or points at a missing row.
    pub fn definition_exists(
        &mut self,
        fid: u64,
        state: TypeState,
    ) -> Result<Option<PropertyDefinition>, PlanError> {
        let slot_key = Self::family_def_slot(state);
        let pk = (fid, slot_key.0, slot_key.1);
        if self.absent_defs.contains(&pk) {
            return Ok(None);
        }
        let is_pending = self.pending_defs.contains_key(&pk);
        if is_pending {
            let pid = self.pending_defs[&pk];
            return Ok(Some(PropertyDefinition {
                property_id: pid,
                family_id: fid,
                type_state: state,
                cardinality: Cardinality::Single,
                nullability: Nullability::Unspecified,
            }));
        }
        let dkey = key(
            self.tenant,
            &[
                rushdb_types::keys::P_DEFINITION,
                &state_to_bytes(fid, state),
            ],
        );
        if let Some(raw) = self.txn.get(&dkey)? {
            let pid = PropertyId(decode_u64(&raw)?);
            let row = self.definition_row(pid)?.ok_or_else(|| {
                PlanError::Storage("definition index references a missing row".into())
            })?;
            self.pending_defs.insert(pk, pid.0);
            return Ok(Some(row));
        }
        self.absent_defs.insert(pk);
        Ok(None)
    }

    fn definition_row(&self, pid: PropertyId) -> Result<Option<PropertyDefinition>, PlanError> {
        let pkey = key(
            self.tenant,
            &[b"property-row", &rushdb_types::keys::u64be(pid.0)],
        );
        self.txn
            .get(&pkey)?
            .map(|raw| serde_json::from_slice(&raw).map_err(|e| PlanError::Storage(e.to_string())))
            .transpose()
    }

    /// Returns the existing definition for `(family_id, state)`, or creates
    /// and stages one.
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if the definition index cannot be read
    /// or the new property id cannot be allocated.
    pub fn ensure_definition(
        &mut self,
        family_id: u64,
        state: TypeState,
    ) -> Result<PropertyDefinition, PlanError> {
        if let Some(existing) = self.definition_exists(family_id, state)? {
            return Ok(existing);
        }
        let slot_key = Self::family_def_slot(state);
        let pid = PropertyId(self.next_alloc("property-definition")?);
        let def = PropertyDefinition {
            property_id: pid.0,
            family_id,
            type_state: state,
            cardinality: Cardinality::Single,
            nullability: Nullability::Unspecified,
        };
        self.mutations.push(Mutation::EnsureDefinition {
            property_id: pid.0,
            family_id,
            type_state: state,
            cardinality: Cardinality::Single,
            nullability: Nullability::Unspecified,
        });
        let pk = (family_id, slot_key.0, slot_key.1);
        self.absent_defs.remove(&pk);
        self.pending_defs.insert(pk, pid.0);
        Ok(def)
    }

    /// Resolve the effective definition used for a field write under the
    /// document-22 policy: first-null stays Unresolved; typed evidence
    /// resolves a parallel Known(T) definition; null prefers any existing
    /// nullable Known sibling before the neutral Unresolved definition.
    ///
    /// # Errors
    /// Returns [`PlanError`] if the family or definition cannot be resolved,
    /// or an array mixes incompatible element types.
    pub fn resolve_definition_for_change(
        &mut self,
        scope: OwnerKind,
        display_name: &str,
        change: &FieldChange,
    ) -> Result<PropertyDefinition, PlanError> {
        let (family, _) = self.resolve_family(DEFAULT_NAMESPACE, scope, display_name)?;

        match change {
            FieldChange::Set(_v @ Json::Array(items)) => {
                let item_ty = unify_array_type(items, display_name)?;
                self.pick_typed_definition(family.family_id, item_ty, display_name)
            }
            // A JSON `null` scalar is an explicit null occurrence, not a
            // mixed-type array. Callers commonly write `Set(json!(null))`
            // instead of `ExplicitNull`; rejecting it with a misleading
            // `MixedArray` error (and failing the whole record) was a defect.
            // Every explicit null lands on the family's untyped definition, so
            // a typed definition's support counts values of its type only and
            // the catalog reports the nulls once, as the `null` entry. (Nulls
            // used to join whichever typed definition existed at write time:
            // per-type counts then included nulls, and nulls written before
            // the first value were not counted once a typed one existed.) A
            // NOT NULL typed definition still receives it, to reject it.
            FieldChange::Set(Json::Null) | FieldChange::ExplicitNull => {
                if let Some(def) = self.not_null_known(family.family_id)? {
                    return Ok(def);
                }
                self.ensure_definition(family.family_id, TypeState::Unresolved)
            }
            FieldChange::Set(v) => {
                let (ty, _) =
                    infer_scalar(v).ok_or_else(|| PlanError::MixedArray(display_name.into()))?;
                self.pick_typed_definition(family.family_id, ty, display_name)
            }
            FieldChange::SetDatetime(_) => {
                self.pick_typed_definition(family.family_id, LogicalType::DateTime, display_name)
            }
            FieldChange::Delete => {
                if let Some(unres) =
                    self.definition_exists(family.family_id, TypeState::Unresolved)?
                {
                    return Ok(unres);
                }
                Err(PlanError::MissingEntity(format!(
                    "family {}",
                    family.normalized_name
                )))
            }
        }
    }

    fn pick_typed_definition(
        &mut self,
        fid: u64,
        ty: LogicalType,
        _field: &str,
    ) -> Result<PropertyDefinition, PlanError> {
        if let Some(existing) = self.definition_exists(fid, TypeState::Known(ty))? {
            return Ok(existing);
        }

        // Schema-less family policy (product parity): a property may hold
        // different types across records (e.g. `value` is a string on one
        // row and a number on another), so a new typed definition is ADDED
        // alongside existing ones; the catalog reports a family's Int64 and
        // Float64 definitions as one number. An integer after a float gets
        // its own Int64 definition, as a float after an integer gets a
        // Float64 one: a definition's dictionary and ordered index hold
        // entries of its own type only, which their readers and the range
        // planner rely on (an Int64 entry under a Float64 definition read
        // back as its bits, and `$gte` skipped it). First typed observation
        // (or a new parallel type) resolves/extends the family; the neutral
        // null population stays on Unresolved.
        self.ensure_definition(fid, TypeState::Known(ty))
    }

    /// The family's first typed definition declared NOT NULL, if any.
    fn not_null_known(&mut self, fid: u64) -> Result<Option<PropertyDefinition>, PlanError> {
        for ty in [
            LogicalType::Boolean,
            LogicalType::Int64,
            LogicalType::Float64,
            LogicalType::Utf8,
            LogicalType::DateTime,
        ] {
            if let Some(def) = self.definition_exists(fid, TypeState::Known(ty))? {
                if matches!(def.nullability, Nullability::NotNull) {
                    return Ok(Some(def));
                }
            }
        }
        Ok(None)
    }

    /// Checks a proposed change against the definition's nullability and
    /// resolved type.
    ///
    /// # Errors
    /// Returns [`PlanError::NullRejected`] for an explicit null on a `NotNull`
    /// definition, or [`PlanError::TypeMismatch`] for an incompatible scalar.
    pub fn check_write_allowed(
        &self,
        def: &PropertyDefinition,
        field: &str,
        change: &FieldChange,
    ) -> Result<(), PlanError> {
        if let FieldChange::ExplicitNull = change {
            if matches!(def.nullability, Nullability::NotNull) {
                return Err(PlanError::NullRejected(field.to_string()));
            }
            return Ok(());
        }
        if let FieldChange::Set(v) = change {
            if v.is_null() {
                // A JSON `null` scalar is an explicit null, checked against
                // nullability rather than compared as a typed value.
                if matches!(def.nullability, Nullability::NotNull) {
                    return Err(PlanError::NullRejected(field.to_string()));
                }
                return Ok(());
            }
            if !v.is_array() {
                if let Some(TypeState::Known(expected)) = Some(def.type_state) {
                    let inferred = infer_scalar(v).map(|(t, _)| t);
                    match inferred {
                        Some(t) if t == expected => return Ok(()),
                        Some(LogicalType::Int64) if expected == LogicalType::Float64 => {
                            return Ok(())
                        }
                        other => {
                            return Err(PlanError::TypeMismatch {
                                field: field.to_string(),
                                expected: format!("{expected}"),
                                found: format!(
                                    "{:?}",
                                    other.map_or_else(|| "?".into(), |t| format!("{t}"))
                                ),
                            })
                        }
                    }
                }
            }
        }
        // A typed datetime write must still satisfy the definition's resolved
        // type; without this guard a `SetDatetime` could bypass the mismatch
        // check that a plain scalar `Set` is subject to.
        if let FieldChange::SetDatetime(_) = change {
            if let TypeState::Known(expected) = def.type_state {
                if expected != LogicalType::DateTime {
                    return Err(PlanError::TypeMismatch {
                        field: field.to_string(),
                        expected: format!("{expected}"),
                        found: "DateTime".into(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Delete every live occurrence this owner holds for the family's
    /// definitions, returning tombstone rows for embedding invalidation.
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if a definition or occurrence row cannot
    /// be read.
    pub fn stage_deletes_of_family(
        &mut self,
        kind: OwnerKind,
        owner_ord: u64,
        family: &PropertyFamily,
    ) -> Result<(), PlanError> {
        if self.new_owners.contains(&(kind, owner_ord)) {
            // Created by this batch: nothing committed to delete.
            return Ok(());
        }
        for ty_state in [
            TypeState::Unresolved,
            TypeState::Known(LogicalType::Boolean),
            TypeState::Known(LogicalType::Int64),
            TypeState::Known(LogicalType::Float64),
            TypeState::Known(LogicalType::Utf8),
            TypeState::Known(LogicalType::DateTime),
        ] {
            if let Some(def) = self.definition_exists(family.family_id, ty_state)? {
                self.stage_delete_owner_property(kind, owner_ord, def.property_id, None)?;
            }
        }
        Ok(())
    }

    /// Stage deletion of all occurrences of one (owner, property) pair,
    /// optionally restricting to one element slot. Skips duplicates already
    /// staged in this batch.
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if the owner index or an occurrence row
    /// cannot be read or decoded.
    pub fn stage_delete_owner_property(
        &mut self,
        kind: OwnerKind,
        owner_ord: u64,
        pid: u64,
        elem: Option<u64>,
    ) -> Result<(), PlanError> {
        let occurrences = crate::reader::try_access::occurrences_of_pid(
            self.txn,
            self.tenant,
            kind,
            owner_ord,
            pid,
        )?;
        let mut family_id = None;
        for occ in occurrences {
            if elem.is_some_and(|e| e != occ.elem) {
                continue;
            }
            self.stage_delete(&occ, &mut family_id)?;
        }
        Ok(())
    }

    /// Stage the tombstone of one occurrence (deduplicated within the batch).
    fn stage_delete(
        &mut self,
        occ: &crate::reader::try_access::Occurrence,
        family_id: &mut Option<u64>,
    ) -> Result<(), PlanError> {
        if !self
            .deleted_occurrences
            .insert((occ.kind, occ.owner, occ.pid, occ.elem))
        {
            return Ok(());
        }
        let fid = if let Some(fid) = *family_id {
            fid
        } else {
            let def = crate::reader::try_access::definition(self.txn, self.tenant, occ.pid)?
                .ok_or_else(|| PlanError::Storage("occurrence of an unknown property".into()))?;
            *family_id = Some(def.family_id);
            def.family_id
        };
        self.mutations.push(Mutation::DeleteOccurrence {
            row: occ.to_row(fid),
        });
        Ok(())
    }

    /// Stage tombstones for every occurrence of one owner (all properties).
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if the occurrences or a definition cannot
    /// be read.
    pub fn stage_delete_all_of_owner(
        &mut self,
        kind: OwnerKind,
        owner_ord: u64,
    ) -> Result<(), PlanError> {
        let occurrences = crate::reader::try_access::occurrences_of_owner(
            self.txn,
            self.tenant,
            kind,
            owner_ord,
        )?;
        let mut current: Option<(u64, Option<u64>)> = None;
        for occ in occurrences {
            let mut family_id = match current {
                Some((pid, fid)) if pid == occ.pid => fid,
                _ => None,
            };
            self.stage_delete(&occ, &mut family_id)?;
            current = Some((occ.pid, family_id));
        }
        Ok(())
    }

    /// Build and stage `PutOccurrence` mutations for a new field value.
    ///
    /// # Errors
    /// Returns [`PlanError::MixedArray`] if a value cannot be inferred as a
    /// scalar, or [`PlanError::Storage`] if an ordinal cannot be allocated.
    ///
    /// # Panics
    /// Panics if an inferred scalar cannot be order-preservingly encoded,
    /// which is unreachable for the scalar types produced by `infer_scalar`.
    pub fn stage_put_values(
        &mut self,
        kind: OwnerKind,
        owner_ord: u64,
        family: &PropertyFamily,
        def: &PropertyDefinition,
        field_display: &str,
        payload: &ValuePayload,
    ) -> Result<(), PlanError> {
        let elements: Vec<Json> = match payload {
            ValuePayload::Null => vec![Json::Null],
            ValuePayload::Single(v) => vec![v.clone()],
            ValuePayload::Array(items) => items.iter().flat_map(single_element).collect(),
            ValuePayload::DateTime(ms) => vec![serde_json::json!(ms)],
        };
        let is_array = matches!(payload, ValuePayload::Array(_));

        // Empty arrays are stored as a single payload-free marker occurrence so
        // reconstruction can emit a genuine `[]` (prod preserves empty arrays;
        // they are not collapsed to null).
        if is_array && elements.is_empty() {
            let row = OccurrenceRow {
                owner_kind: kind,
                owner_ord,
                property_id: def.property_id,
                family_id: family.family_id,
                elem_ordinal: Some(0),
                occurrence_value: OccurrenceValue::EmptyArray,
            };
            self.mutations.push(Mutation::PutOccurrence {
                row,
                canonical_value_bytes: None,
            });
            return Ok(());
        }

        for (idx, ev) in elements.iter().enumerate() {
            let elem_ord = if is_array { Some(idx as u64) } else { None };

            let (value_state, enc_opt) = if matches!(payload, ValuePayload::DateTime(_)) {
                let ty = LogicalType::DateTime;
                let s = rushdb_types::value::Scalar::DateTime(
                    ev.as_i64()
                        .ok_or_else(|| PlanError::MixedArray(field_display.into()))?,
                );
                let enc = encode_order_preserving(ty, &s).expect("encodable staged scalar");
                (OccurrenceValue::Stored { ty }, Some(enc))
            } else {
                match ev {
                    Json::Null => (OccurrenceValue::ExplicitNull, None),
                    scalar => {
                        let (ty, s) = infer_scalar(scalar)
                            .ok_or_else(|| PlanError::MixedArray(field_display.into()))?;
                        // An integer element of a mixed number array is stored
                        // in the array's Float64 definition as a float, so the
                        // definition's entries share one encoding.
                        let (ty, s) = match (def.type_state, s) {
                            (
                                TypeState::Known(LogicalType::Float64),
                                rushdb_types::value::Scalar::Int64(i),
                            ) => (
                                LogicalType::Float64,
                                rushdb_types::value::Scalar::Float64(int_as_float(i)),
                            ),
                            (_, s) => (ty, s),
                        };
                        let enc = encode_order_preserving(ty, &s).expect("encodable staged scalar");
                        (OccurrenceValue::Stored { ty }, Some(enc))
                    }
                }
            };

            let row = OccurrenceRow {
                owner_kind: kind,
                owner_ord,
                property_id: def.property_id,
                family_id: family.family_id,
                elem_ordinal: elem_ord,
                occurrence_value: value_state,
            };
            self.mutations.push(Mutation::PutOccurrence {
                row,
                canonical_value_bytes: enc_opt,
            });
        }
        Ok(())
    }

    /// Appends a precondition to be enforced by the commit.
    pub fn push_precondition(&mut self, pc: Precondition) {
        self.preconditions.push(pc);
    }

    /// Reads every value under `prefix` from the transaction overlay.
    ///
    /// # Errors
    /// Returns [`PlanError::Storage`] if the range scan fails.
    pub fn txn_values_under(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>, PlanError> {
        Ok(self
            .txn
            .range_prefix(prefix)?
            .into_iter()
            .map(|(_, v)| v)
            .collect())
    }

    /// Stages deletion of a relationship header found during cascade planning.
    pub fn txn_stage_rel_header_delete(
        &mut self,
        hdr: rushdb_types::occurrence::RelationshipHeader,
    ) {
        self.mutations.push(Mutation::DeleteRelationshipHeader {
            relationship_id: hdr.relationship_id,
            ordinal: hdr.ordinal,
            rel_type: hdr.rel_type,
            source_ord: hdr.source_ord,
            target_ord: hdr.target_ord,
        });
    }

    /// Sorts the staged mutations into deterministic commit order and returns
    /// them.
    #[must_use]
    pub fn finish(mut self) -> Vec<Mutation> {
        self.mutations.sort_by_key(sort_key);
        self.mutations
    }
}

fn sort_key(m: &Mutation) -> String {
    match m {
        Mutation::CreateRecordHeader { .. } | Mutation::CreateRelationshipHeader { .. } => {
            format!("{}|{}", 'a', header_ord(m))
        }
        Mutation::EnsureFamily { family_id, .. } => format!("{}|{}", 'f', family_id),
        Mutation::EnsureDefinition { property_id, .. } => format!("{}|{}", 'd', property_id),
        Mutation::DeleteFamily { family_id, .. } => format!("{}|{}", 'x', family_id),
        // A delete of an occurrence slot must apply before a put of the same
        // slot (an update rewrites `(owner, pid, elem)` in place, DC11).
        Mutation::DeleteOccurrence { row } => format!(
            "n|{}|{:020}|{:020}|{:020}",
            row.owner_kind.as_byte(),
            row.owner_ord,
            row.property_id,
            row.elem_ordinal.unwrap_or(0)
        ),
        Mutation::PutOccurrence { row, .. } => format!(
            "o|{}|{:020}|{:020}|{:020}",
            row.owner_kind.as_byte(),
            row.owner_ord,
            row.property_id,
            row.elem_ordinal.unwrap_or(0)
        ),
        Mutation::RetypeRecordHeader { ordinal, .. } => format!("{}|{}", 'r', ordinal),
        Mutation::DeleteRecordHeader { ordinal, .. }
        | Mutation::DeleteRelationshipHeader { ordinal, .. } => format!("{}|{}", 'z', ordinal),
        Mutation::EnsureEmbeddingSpace { info, .. } => format!("{}|{}", 'f', info.space_id),
        Mutation::SetMetaValue { .. }
        | Mutation::SetMetaBlob { .. }
        | Mutation::DeleteMetaBlob { .. } => format!("{}|{}", 'z', u64::MAX / 4),
        Mutation::ClearEmbeddingSpace { space_id, .. } => format!("z|clear-embedding|{space_id}"),
        Mutation::SetSecurityPolicy { .. } | Mutation::SetSecurityPolicyThreshold { .. } => {
            "z|security-policy".into()
        }
        Mutation::PurgeTenantChunk { .. } => {
            format!("{}|{}", 'z', u64::MAX / 2)
        }
        Mutation::RegisterTenant { .. } => "z|register-tenant".into(),
        Mutation::RetireTenant => "z|retire-tenant".into(),
        Mutation::UpsertEmbeddingBinding { binding, .. } => {
            format!("{}|{}", 'o', binding.vector_slot)
        }
        Mutation::ReclaimEmbeddingSlots { .. } => "z|reclaim-embedding".into(),
        Mutation::DefineIndex { name, .. } | Mutation::DropIndex { name } => {
            format!("z|index|{name}")
        }
        Mutation::IndexBackfill { id, .. } | Mutation::IndexDrop { id, .. } => {
            format!("z|index|{id:010}")
        }
        Mutation::IndexApplied { index, .. } => format!("z|index-applied|{index:010}"),
        Mutation::DeleteRawKey { key } => format!("z|repair-delete|{key:?}"),
        Mutation::PutRawKey { key, .. } => format!("z|repair-put|{key:?}"),
    }
}

fn header_ord(m: &Mutation) -> u64 {
    match m {
        Mutation::CreateRecordHeader { ordinal, .. }
        | Mutation::CreateRelationshipHeader { ordinal, .. } => *ordinal,
        _ => 0,
    }
}

fn state_to_bytes(fid: u64, state: TypeState) -> Vec<u8> {
    let mut out = rushdb_types::keys::u64be(fid).to_vec();
    out.extend_from_slice(b"/");
    match state {
        TypeState::Unresolved => out.extend_from_slice(b"U"),
        TypeState::Known(ty) => {
            out.extend_from_slice(b"K");
            out.push(logical_tag(ty));
        }
    }
    out
}

fn unify_array_type(items: &[Json], field: &str) -> Result<LogicalType, PlanError> {
    // An empty array has no element to type: it is stored as an empty-array
    // marker and read back as `[]`. Default to string, as for an all-null
    // array below.
    if items.is_empty() {
        return Ok(LogicalType::Utf8);
    }
    let mut seen: Option<LogicalType> = None;
    for it in items {
        // null elements are type-less explicit-null occurrences (document 22);
        // they never force a mixed-array error and don't pick the type
        if it.is_null() {
            continue;
        }
        let Some((ty, _)) = infer_scalar(it) else {
            return Err(PlanError::MixedArray(field.into()));
        };
        seen = Some(match seen {
            None => ty,
            Some(prev) if prev == ty => prev,
            Some(LogicalType::Int64 | LogicalType::Float64)
                if ty == LogicalType::Int64 || ty == LogicalType::Float64 =>
            {
                LogicalType::Float64
            }
            Some(other) => other,
        });
    }
    // all elements were null (or the array was empty): string-typed, matching
    // the reference `suggestPropertyType(null) === STRING`
    Ok(seen.unwrap_or(LogicalType::Utf8))
}

/// An integer as the float a Float64 definition stores (exact below 2^53).
#[expect(
    clippy::cast_precision_loss,
    reason = "a mixed number array is stored as floats"
)]
fn int_as_float(i: i64) -> f64 {
    i as f64
}
