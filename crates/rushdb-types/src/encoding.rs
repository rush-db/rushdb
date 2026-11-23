use crate::error::TypesError;
use crate::value::Scalar;

fn u64be(v: u64) -> [u8; 8] {
    v.to_be_bytes()
}

/// Order-preserving sign flip for signed integers.
fn int_encode(i: i64) -> [u8; 8] {
    u64be(u64::from_ne_bytes(i.to_ne_bytes()) ^ 0x8000_0000_0000_0000)
}

fn int_decode(b: &[u8]) -> i64 {
    let raw = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    i64::from_ne_bytes((raw ^ 0x8000_0000_0000_0000).to_ne_bytes())
}

/// Total order for f64: positive floats have the sign bit flipped, negatives
/// are bit-inverted. -0.0 and +0.0 are distinct keys.
fn float_encode(f: f64) -> [u8; 8] {
    let bits = f.to_bits();
    let transformed = if bits & 0x8000_0000_0000_0000 == 0 {
        bits | 0x8000_0000_0000_0000
    } else {
        !bits
    };
    u64be(transformed)
}

fn float_decode(b: &[u8]) -> f64 {
    let raw = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    let original = if raw & 0x8000_0000_0000_0000 == 0 {
        !raw
    } else {
        raw & !0x8000_0000_0000_0000
    };
    f64::from_bits(original)
}

/// Stable one-byte tag for a logical type used in encoded keys.
#[must_use]
pub fn logical_tag(ty: crate::model::LogicalType) -> u8 {
    ty.tag()
}

/// Decode a logical type from its one-byte tag.
///
/// # Errors
///
/// Returns [`TypesError::UnknownTypeTag`] if `tag` is not a known type tag.
pub fn logical_from_tag(tag: u8) -> Result<crate::model::LogicalType, TypesError> {
    crate::model::LogicalType::from_tag(tag).ok_or(TypesError::UnknownTypeTag(tag))
}

/// Encode a scalar with its logical type tag as a one-byte prefix.
///
/// # Panics
///
/// Panics if `s` does not match `ty`; use [`encode_order_preserving`] to handle
/// a mismatch instead.
#[must_use]
pub fn encode_value_with_prefix(ty: crate::model::LogicalType, s: &Scalar) -> Vec<u8> {
    let mut out = vec![logical_tag(ty)];
    out.extend_from_slice(&encode_order_preserving(ty, s).expect("encodable"));
    out
}

/// Order-preserving sign flip for a signed 32-bit integer.
fn int32_encode(i: i32) -> [u8; 4] {
    (u32::from_ne_bytes(i.to_ne_bytes()) ^ 0x8000_0000).to_be_bytes()
}

fn int32_decode(b: &[u8]) -> Option<i32> {
    let raw = u32::from_be_bytes(b.try_into().ok()?);
    Some(i32::from_ne_bytes((raw ^ 0x8000_0000).to_ne_bytes()))
}

fn int128_encode(i: i128) -> [u8; 16] {
    (u128::from_ne_bytes(i.to_ne_bytes()) ^ (1 << 127)).to_be_bytes()
}

/// Bytes of a stored instant after its comparable microseconds: the UTC
/// offset in seconds.
const INSTANT_SUFFIX: usize = 4;
/// Bytes of a datetime's comparable payload, and of the whole stored value
/// 3.0.13 wrote (UTC epoch milliseconds).
const INSTANT_COMPARABLE: usize = 8;

/// Encode a scalar into bytes whose lexicographic order matches value order
/// (for a point: its CRS and Hilbert cell first). Equal values have equal
/// bytes: a datetime's offset is not part of them (see [`encode_stored`]).
///
/// # Errors
///
/// Returns [`TypesError::Unencodable`] if `s` does not match `ty`.
pub fn encode_order_preserving(
    ty: crate::model::LogicalType,
    s: &Scalar,
) -> Result<Vec<u8>, TypesError> {
    use crate::model::LogicalType as T;
    let err = || TypesError::Unencodable("scalar/type mismatch");
    Ok(match (ty, s) {
        (T::Boolean, Scalar::Boolean(b)) => vec![u8::from(*b)],
        (T::Int64, Scalar::Int64(i))
        | (T::LocalTime, Scalar::LocalTime(i))
        | (T::LocalDateTime, Scalar::LocalDateTime(i)) => int_encode(*i).to_vec(),
        (T::DateTime, Scalar::DateTime(instant)) => int_encode(instant.micros).to_vec(),
        (T::Date, Scalar::Date(days)) => int32_encode(*days).to_vec(),
        (T::Float64, Scalar::Float64(f)) if !f.is_nan() => float_encode(*f).to_vec(),
        (T::Utf8, Scalar::Utf8(st)) => {
            let mut b = st.as_bytes().to_vec();
            b.push(0x00);
            b
        }
        (T::Duration, Scalar::Duration(d)) => {
            let mut b = int128_encode(d.normalized()).to_vec();
            b.extend_from_slice(&int32_encode(d.months));
            b.extend_from_slice(&int32_encode(d.days));
            b.extend_from_slice(&int_encode(d.micros));
            b
        }
        (T::Point, Scalar::Point(point)) => crate::types::point::encode(*point),
        _ => return Err(err()),
    })
}

/// The stored form of a scalar, as occurrence rows and the write-ahead log
/// keep it: the order-preserving payload, then what the comparison leaves
/// out (a datetime's UTC offset, in seconds).
///
/// # Errors
///
/// Returns [`TypesError::Unencodable`] if `s` does not match `ty`.
pub fn encode_stored(ty: crate::model::LogicalType, s: &Scalar) -> Result<Vec<u8>, TypesError> {
    let mut out = encode_order_preserving(ty, s)?;
    if let Scalar::DateTime(instant) = s {
        out.extend_from_slice(&instant.offset.to_be_bytes());
    }
    Ok(out)
}

/// The order-preserving payload and the suffix of a stored value. A
/// datetime stored by 3.0.13 (epoch milliseconds, no suffix) splits as all
/// payload; [`decode_stored`] reads it, the data migration rewrites it.
#[must_use]
pub fn split_stored(ty: crate::model::LogicalType, stored: &[u8]) -> (&[u8], &[u8]) {
    if ty == crate::model::LogicalType::DateTime
        && stored.len() == INSTANT_COMPARABLE + INSTANT_SUFFIX
    {
        stored.split_at(INSTANT_COMPARABLE)
    } else {
        (stored, &[])
    }
}

/// Whether a stored datetime is in the 3.0.13 form: UTC epoch
/// milliseconds without an offset.
#[must_use]
pub fn is_legacy_datetime(ty: crate::model::LogicalType, stored: &[u8]) -> bool {
    ty == crate::model::LogicalType::DateTime && stored.len() == INSTANT_COMPARABLE
}

/// Decode a stored value ([`encode_stored`]); a 3.0.13 datetime (epoch
/// milliseconds) reads as its instant in UTC.
///
/// # Errors
///
/// Returns [`TypesError::MalformedKey`] if the bytes are not a stored value
/// of `ty`.
pub fn decode_stored(ty: crate::model::LogicalType, stored: &[u8]) -> Result<Scalar, TypesError> {
    if is_legacy_datetime(ty, stored) {
        let raw: [u8; 8] = stored.try_into().map_err(|_| TypesError::MalformedKey)?;
        return crate::types::temporal::Instant::from_millis(int_decode(&raw))
            .map(Scalar::DateTime)
            .ok_or(TypesError::MalformedKey);
    }
    let (payload, suffix) = split_stored(ty, stored);
    let mut scalar = decode_order_preserving(ty, payload)?;
    if let Scalar::DateTime(instant) = &mut scalar {
        let offset = i32::from_be_bytes(suffix.try_into().map_err(|_| TypesError::MalformedKey)?);
        if offset.abs() > crate::types::temporal::MAX_OFFSET_SECONDS {
            return Err(TypesError::MalformedKey);
        }
        instant.offset = offset;
    }
    Ok(scalar)
}

/// Decode bytes produced by [`encode_order_preserving`] back into a scalar
/// (a datetime in UTC: the payload has no offset).
///
/// # Errors
///
/// Returns [`TypesError::MalformedKey`] if the bytes are not a valid encoding
/// of `ty` (for example a boolean byte other than `0`/`1`, or a string without
/// its trailing NUL terminator).
pub fn decode_order_preserving(
    ty: crate::model::LogicalType,
    b: &[u8],
) -> Result<Scalar, TypesError> {
    use crate::model::LogicalType as T;
    let malformed = || TypesError::MalformedKey;
    let int = || -> Result<i64, TypesError> {
        let raw: [u8; 8] = b.try_into().map_err(|_| malformed())?;
        Ok(int_decode(&raw))
    };
    Ok(match ty {
        T::Boolean => match b {
            [0] => Scalar::Boolean(false),
            [1] => Scalar::Boolean(true),
            _ => return Err(malformed()),
        },
        T::Int64 => Scalar::Int64(int()?),
        T::LocalDateTime => Scalar::LocalDateTime(int()?),
        T::LocalTime => {
            let micros = int()?;
            if !(0..crate::types::temporal::MICROS_PER_DAY).contains(&micros) {
                return Err(malformed());
            }
            Scalar::LocalTime(micros)
        }
        T::DateTime => Scalar::DateTime(crate::types::temporal::Instant::utc(int()?)),
        T::Date => Scalar::Date(int32_decode(b).ok_or_else(malformed)?),
        T::Float64 => {
            let raw: [u8; 8] = b.try_into().map_err(|_| malformed())?;
            let value = float_decode(&raw);
            if value.is_nan() {
                return Err(malformed());
            }
            Scalar::Float64(value)
        }
        T::Utf8 => {
            let trimmed = b.strip_suffix(&[0x00]).ok_or_else(malformed)?;
            Scalar::Utf8(String::from_utf8(trimmed.to_vec()).map_err(|_| malformed())?)
        }
        T::Duration => {
            if b.len() != 32 {
                return Err(malformed());
            }
            let duration = crate::types::duration::Duration {
                months: int32_decode(&b[16..20]).ok_or_else(malformed)?,
                days: int32_decode(&b[20..24]).ok_or_else(malformed)?,
                micros: int_decode(&<[u8; 8]>::try_from(&b[24..32]).map_err(|_| malformed())?),
            };
            if int128_encode(duration.normalized())[..] != b[..16] {
                return Err(malformed());
            }
            Scalar::Duration(duration)
        }
        T::Point => Scalar::Point(crate::types::point::decode(b).ok_or_else(malformed)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LogicalType;

    #[test]
    fn order_preserving_decoder_rejects_invalid_fixed_widths() {
        for ty in [
            LogicalType::Int64,
            LogicalType::DateTime,
            LogicalType::Float64,
        ] {
            assert!(matches!(
                decode_order_preserving(ty, &[]),
                Err(TypesError::MalformedKey)
            ));
            assert!(matches!(
                decode_order_preserving(ty, &[0; 9]),
                Err(TypesError::MalformedKey)
            ));
        }
        assert!(matches!(
            decode_order_preserving(LogicalType::Boolean, &[0, 0]),
            Err(TypesError::MalformedKey)
        ));
    }

    #[test]
    fn every_type_round_trips_and_keeps_its_order() {
        use crate::types::{duration::parse_duration, infer_typed};
        let samples: Vec<Scalar> = [
            "1969-12-31",
            "1970-01-01",
            "2024-05-01",
            "-0032-01-01",
            "00:00:00",
            "10:15:00.5",
            "23:59:59.999999",
            "1969-12-31T23:59:59.999999",
            "2024-05-01T10:15",
            "1969-12-31T23:00:00-02:00",
            "2024-05-01T10:15:00.000001Z",
            "P1M",
            "P30D",
            "PT-1S",
            "-P1Y",
            "PT0S",
        ]
        .iter()
        .map(|text| infer_typed(&serde_json::json!(text)).unwrap())
        .collect();
        for a in &samples {
            let ty = a.logical_type();
            let stored = encode_stored(ty, a).unwrap();
            assert_eq!(decode_stored(ty, &stored).as_ref(), Ok(a), "{a:?}");
            let payload = encode_order_preserving(ty, a).unwrap();
            assert_eq!(split_stored(ty, &stored).0, payload.as_slice());
            for b in samples.iter().filter(|b| b.logical_type() == ty) {
                let other = encode_order_preserving(ty, b).unwrap();
                assert_eq!(
                    payload.cmp(&other),
                    crate::value::scalar_cmp(a, b).unwrap(),
                    "{a:?} vs {b:?}"
                );
            }
        }
        assert!(parse_duration("P1M").is_some());
    }

    #[test]
    fn a_3_0_13_datetime_reads_as_its_instant() {
        let stored = int_encode(1_500).to_vec();
        assert!(is_legacy_datetime(LogicalType::DateTime, &stored));
        assert_eq!(
            decode_stored(LogicalType::DateTime, &stored),
            Ok(Scalar::DateTime(crate::types::temporal::Instant::utc(
                1_500_000
            )))
        );
    }

    #[test]
    fn float_order_encoding_rejects_nan() {
        assert!(matches!(
            encode_order_preserving(LogicalType::Float64, &Scalar::Float64(f64::NAN)),
            Err(TypesError::Unencodable(_))
        ));
        assert!(matches!(
            decode_order_preserving(LogicalType::Float64, &float_encode(f64::NAN)),
            Err(TypesError::MalformedKey)
        ));
    }
}
