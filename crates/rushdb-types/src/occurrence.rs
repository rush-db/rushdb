use serde::{Deserialize, Serialize};

use crate::ids::OwnerKind;
use crate::model::{Cardinality, LogicalType, Nullability, TypeState};

/// `ExplicitNull` is a payload-free occurrence state, never a domain type.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize, Deserialize)]
pub enum OccurrenceValue {
    /// A property explicitly set to null.
    ExplicitNull,
    /// Marker occurrence for an array property with zero elements. Arrays are
    /// stored element-wise, so an empty array has no elements to persist; this
    /// payload-free marker (stored at elem 0) lets reconstruction emit a
    /// genuine `[]` — matching prod, where empty arrays are preserved rather
    /// than collapsed to null.
    EmptyArray,
    /// A stored scalar value of a logical type; its order-preserving payload
    /// is stored inline in the occurrence row.
    Stored {
        /// Logical type of the stored scalar.
        ty: LogicalType,
    },
}

impl OccurrenceValue {
    /// Report whether this occurrence is an explicit null.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, OccurrenceValue::ExplicitNull)
    }
}

/// One property occurrence of a record or relationship. Its identity is
/// `(owner_kind, owner_ord, property_id, elem)`; the canonical row is keyed by
/// exactly that (`keys::occ_key`, DC11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OccurrenceRow {
    /// Whether the owning row is a record or a relationship.
    pub owner_kind: OwnerKind,
    /// Dense ordinal of the owning row.
    pub owner_ord: u64,
    /// Property id this occurrence belongs to.
    pub property_id: u64,
    /// Property family this occurrence belongs to.
    pub family_id: u64,
    /// Element index within an array property, or `None` for scalars.
    pub elem_ordinal: Option<u64>,
    /// The occurrence's value state. The value hash is not carried: it is a
    /// function of this state and the canonical bytes
    /// ([`occurrence_value_hash`]).
    pub occurrence_value: OccurrenceValue,
}

/// Canonical value hash of an occurrence (null and the empty-array marker
/// share the null hash, as the planner assigns them).
#[must_use]
pub fn occurrence_value_hash(value: OccurrenceValue, payload: &[u8]) -> u64 {
    match value {
        OccurrenceValue::Stored { ty } => {
            crate::hash::value_hash(crate::encoding::logical_tag(ty), payload)
        }
        OccurrenceValue::ExplicitNull | OccurrenceValue::EmptyArray => {
            crate::hash::null_value_hash()
        }
    }
}

/// Durable header for a record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordHeader {
    /// Stable public record id.
    pub record_id: [u8; 16],
    /// Dense ordinal of the record.
    pub ordinal: u64,
    /// Primary label of the record.
    pub label: String,
}

/// Durable header for a relationship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationshipHeader {
    /// Stable public relationship id.
    pub relationship_id: [u8; 16],
    /// Dense ordinal of the relationship.
    pub ordinal: u64,
    /// Relationship type.
    pub rel_type: String,
    /// Ordinal of the source record.
    pub source_ord: u64,
    /// Ordinal of the target record.
    pub target_ord: u64,
    /// Public id of the source record.
    pub source_id: [u8; 16],
    /// Public id of the target record.
    pub target_id: [u8; 16],
}

/// A normalized property family, scoped to records or relationships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyFamily {
    /// Dense id of the family.
    pub family_id: u64,
    /// Namespace the property belongs to.
    pub namespace: String,
    /// Whether the family is owned by records or relationships.
    pub owner_scope: OwnerKind,
    /// Normalized name used for lookup.
    pub normalized_name: String,
    /// Human-readable display name.
    pub display_name: String,
}

/// A typed property definition within a family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDefinition {
    /// Dense id of the property.
    pub property_id: u64,
    /// Family this definition belongs to.
    pub family_id: u64,
    /// Current type resolution state.
    pub type_state: TypeState,
    /// Allowed value cardinality.
    pub cardinality: Cardinality,
    /// Nullability constraint.
    pub nullability: Nullability,
}

/// Immutable vector-space contract bound through stable aliases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Similarity {
    /// Cosine similarity (dot product of L2-normalized vectors).
    #[default]
    Cosine,
    /// Dot product (raw; for best results use normalized vectors).
    Dot,
    /// Euclidean similarity = `1 / (1 + euclidean_distance)` (Neo4j-style).
    Euclidean,
}

impl Similarity {
    /// Lowercase name of the similarity metric.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Similarity::Cosine => "cosine",
            Similarity::Dot => "dot",
            Similarity::Euclidean => "euclidean",
        }
    }
}

/// Immutable metadata for an embedding space.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingSpaceInfo {
    /// Dense id of the space.
    pub space_id: u64,
    /// Stable, opaque index name. The engine carries vectors without any notion
    /// of embedding source (managed/external/model) — those are control-plane
    /// concepts. Legacy spaces (persisted without this field) default to empty.
    #[serde(default)]
    pub name: String,
    /// Dimensionality of vectors in the space.
    pub dims: u32,
    /// Similarity metric for the space. Older persisted spaces without this
    /// field default to cosine.
    #[serde(default)]
    pub similarity: Similarity,
}

/// Lifecycle status of an embedding binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingStatus {
    /// Binding created but not yet populated.
    Pending,
    /// Binding is populated and eligible for search.
    Ready,
    /// Source value changed, so the binding must be rebuilt.
    Stale,
    /// Binding permanently retired.
    Retired,
}

/// One canonical source -> Space association carrying the exact value hash
/// at bind time. Eligible only while that hash remains current.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingBinding {
    /// Space this binding targets.
    pub space_id: u64,
    /// Whether the owner is a record or a relationship.
    pub owner_kind: OwnerKind,
    /// Dense ordinal of the owning row.
    pub owner_ord: u64,
    /// Property id of the bound value.
    pub pid: u64,
    /// Element index within an array property.
    pub elem: u64,
    /// Slot of the stored vector within the space.
    pub vector_slot: u64,
    /// Value hash captured when the binding was created.
    pub source_hash: u64,
    /// `true` for a vector derived from a canonical occurrence. Such a binding
    /// becomes stale when the occurrence value changes. External vectors use a
    /// payload-derived hash and remain eligible while their owner exists.
    #[serde(default = "default_source_hash_tracked")]
    pub source_hash_tracked: bool,
    /// Current lifecycle status of the binding.
    pub status: BindingStatus,
}

fn default_source_hash_tracked() -> bool {
    true
}

/// Occurrence row flag: the occurrence is an element of an array property.
pub const OCC_FLAG_ARRAY: u8 = 0x01;
/// Occurrence row state: explicit null.
pub const OCC_STATE_NULL: u8 = 0x00;
/// Occurrence row state: the zero-element array marker.
pub const OCC_STATE_EMPTY_ARRAY: u8 = 0xFE;

/// Encode an occurrence row value: `flags || state || payload`, where state is
/// [`OCC_STATE_NULL`], [`OCC_STATE_EMPTY_ARRAY`], or the logical type tag of a
/// stored scalar followed by its order-preserving payload.
#[must_use]
pub fn encode_occ_value(array: bool, value: OccurrenceValue, payload: Option<&[u8]>) -> Vec<u8> {
    let payload = payload.unwrap_or_default();
    let mut out = Vec::with_capacity(2 + payload.len());
    out.push(if array { OCC_FLAG_ARRAY } else { 0 });
    match value {
        OccurrenceValue::ExplicitNull => out.push(OCC_STATE_NULL),
        OccurrenceValue::EmptyArray => out.push(OCC_STATE_EMPTY_ARRAY),
        OccurrenceValue::Stored { ty } => {
            out.push(crate::encoding::logical_tag(ty));
            out.extend_from_slice(payload);
        }
    }
    out
}

/// A decoded occurrence row value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccValue<'a> {
    /// Whether the occurrence is an array element.
    pub array: bool,
    /// Value state.
    pub value: OccurrenceValue,
    /// Order-preserving payload of a stored scalar (empty otherwise).
    pub payload: &'a [u8],
    /// `tag || payload` of a stored scalar: the value-dictionary entry.
    pub entry: &'a [u8],
}

impl OccValue<'_> {
    /// Canonical value hash ([`occurrence_value_hash`]).
    #[must_use]
    pub fn value_hash(&self) -> u64 {
        occurrence_value_hash(self.value, self.payload)
    }
}

/// Decode an occurrence row value written by [`encode_occ_value`].
///
/// # Errors
/// Returns [`crate::error::TypesError::MalformedKey`] for a malformed value.
pub fn decode_occ_value(raw: &[u8]) -> Result<OccValue<'_>, crate::error::TypesError> {
    let malformed = crate::error::TypesError::MalformedKey;
    let (&flags, rest) = raw.split_first().ok_or(malformed.clone())?;
    let (&state, payload) = rest.split_first().ok_or(malformed.clone())?;
    let value = match state {
        OCC_STATE_NULL => OccurrenceValue::ExplicitNull,
        OCC_STATE_EMPTY_ARRAY => OccurrenceValue::EmptyArray,
        tag => OccurrenceValue::Stored {
            ty: crate::encoding::logical_from_tag(tag)?,
        },
    };
    if !matches!(value, OccurrenceValue::Stored { .. }) && !payload.is_empty() {
        return Err(malformed);
    }
    Ok(OccValue {
        array: flags & OCC_FLAG_ARRAY != 0,
        value,
        payload,
        entry: rest,
    })
}

#[cfg(test)]
mod codec_tests {
    use super::*;
    use crate::encoding::{decode_order_preserving, encode_order_preserving, logical_tag};
    use crate::model::LogicalType;
    use crate::value::Scalar;

    /// Seeded xorshift: a reproducible property test without a dependency.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    fn scalar(rng: &mut Rng) -> (LogicalType, Scalar) {
        let bits = rng.next();
        match bits % 5 {
            0 => (LogicalType::Boolean, Scalar::Boolean(bits & 8 != 0)),
            1 => (LogicalType::Int64, Scalar::Int64(bits.cast_signed())),
            2 => {
                let f = f64::from_bits(rng.next());
                let f = if f.is_finite() { f } else { -0.0 };
                (LogicalType::Float64, Scalar::Float64(f))
            }
            3 => (
                LogicalType::DateTime,
                Scalar::DateTime((bits >> 2).cast_signed()),
            ),
            _ => {
                // Up to 1 MiB, including multi-byte UTF-8.
                let len = usize::try_from(rng.next() % (1 << 20)).unwrap();
                let text: String = "aé€".chars().cycle().take(len / 3).collect();
                (LogicalType::Utf8, Scalar::Utf8(text))
            }
        }
    }

    #[test]
    fn occurrence_values_round_trip_for_every_type_null_and_array_state() {
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        for _ in 0..200 {
            let array = rng.next() & 1 == 1;
            let (ty, value) = scalar(&mut rng);
            let payload = encode_order_preserving(ty, &value).unwrap();
            let raw = encode_occ_value(array, OccurrenceValue::Stored { ty }, Some(&payload));
            let decoded = decode_occ_value(&raw).unwrap();
            assert_eq!(decoded.array, array);
            assert_eq!(decoded.value, OccurrenceValue::Stored { ty });
            assert_eq!(decoded.payload, payload.as_slice());
            assert_eq!(decoded.entry[0], logical_tag(ty));
            assert_eq!(&decoded.entry[1..], payload.as_slice());
            let back = decode_order_preserving(ty, decoded.payload).unwrap();
            assert_eq!(
                encode_order_preserving(ty, &back).unwrap(),
                payload,
                "{ty:?}"
            );
        }
        for state in [OccurrenceValue::ExplicitNull, OccurrenceValue::EmptyArray] {
            for array in [false, true] {
                let raw = encode_occ_value(array, state, None);
                let decoded = decode_occ_value(&raw).unwrap();
                assert_eq!((decoded.array, decoded.value), (array, state));
                assert!(decoded.payload.is_empty());
            }
        }
    }

    #[test]
    fn malformed_occurrence_values_are_errors_never_panics() {
        assert!(decode_occ_value(&[]).is_err());
        assert!(decode_occ_value(&[0]).is_err());
        // A null or empty-array state never carries a payload.
        assert!(decode_occ_value(&[0, OCC_STATE_NULL, 1]).is_err());
        assert!(decode_occ_value(&[0, OCC_STATE_EMPTY_ARRAY, 1]).is_err());
        // Unknown type tags are rejected.
        assert!(decode_occ_value(&[0, 0x7F]).is_err());
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        for _ in 0..10_000 {
            let len = usize::try_from(rng.next() % 24).unwrap();
            let raw: Vec<u8> = (0..len).map(|_| rng.next().to_le_bytes()[0]).collect();
            // Must return, Ok or Err, for any input.
            let _ = decode_occ_value(&raw);
        }
    }
}
