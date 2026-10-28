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
    use crate::model::LogicalType::{Boolean, DateTime, Float64, Int64, Utf8};
    match ty {
        Boolean => 1,
        Int64 => 2,
        Float64 => 3,
        Utf8 => 4,
        DateTime => 5,
    }
}

/// Decode a logical type from its one-byte tag.
///
/// # Errors
///
/// Returns [`TypesError::UnknownTypeTag`] if `tag` is not a known type tag.
pub fn logical_from_tag(tag: u8) -> Result<crate::model::LogicalType, TypesError> {
    use crate::model::LogicalType::{Boolean, DateTime, Float64, Int64, Utf8};
    Ok(match tag {
        1 => Boolean,
        2 => Int64,
        3 => Float64,
        4 => Utf8,
        5 => DateTime,
        other => return Err(TypesError::UnknownTypeTag(other)),
    })
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

/// Encode a scalar into bytes whose lexicographic order matches value order.
///
/// # Errors
///
/// Returns [`TypesError::Unencodable`] if `s` does not match `ty`.
pub fn encode_order_preserving(
    ty: crate::model::LogicalType,
    s: &Scalar,
) -> Result<Vec<u8>, TypesError> {
    use crate::model::LogicalType::{Boolean, DateTime, Float64, Int64, Utf8};
    let err = || TypesError::Unencodable("scalar/type mismatch");
    Ok(match ty {
        Boolean => match s {
            Scalar::Boolean(true) => vec![1],
            Scalar::Boolean(false) => vec![0],
            _ => return Err(err()),
        },
        Int64 => match s {
            Scalar::Int64(i) => int_encode(*i).to_vec(),
            _ => return Err(err()),
        },
        DateTime => match s {
            Scalar::DateTime(ms) => int_encode(*ms).to_vec(),
            _ => return Err(err()),
        },
        Float64 => match s {
            Scalar::Float64(f) if !f.is_nan() => float_encode(*f).to_vec(),
            _ => return Err(err()),
        },
        Utf8 => match s {
            Scalar::Utf8(st) => {
                let mut b = st.as_bytes().to_vec();
                b.push(0x00);
                b
            }
            _ => return Err(err()),
        },
    })
}

/// Decode bytes produced by [`encode_order_preserving`] back into a scalar.
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
    use crate::model::LogicalType::{Boolean, DateTime, Float64, Int64, Utf8};
    Ok(match ty {
        Boolean => match b {
            [0] => Scalar::Boolean(false),
            [1] => Scalar::Boolean(true),
            _ => return Err(TypesError::MalformedKey),
        },
        Int64 => {
            let raw: [u8; 8] = b.try_into().map_err(|_| TypesError::MalformedKey)?;
            Scalar::Int64(int_decode(&raw))
        }
        DateTime => {
            let raw: [u8; 8] = b.try_into().map_err(|_| TypesError::MalformedKey)?;
            Scalar::DateTime(int_decode(&raw))
        }
        Float64 => {
            let raw: [u8; 8] = b.try_into().map_err(|_| TypesError::MalformedKey)?;
            let value = float_decode(&raw);
            if value.is_nan() {
                return Err(TypesError::MalformedKey);
            }
            Scalar::Float64(value)
        }
        Utf8 => {
            let trimmed = b.strip_suffix(&[0x00]).ok_or(TypesError::MalformedKey)?;
            Scalar::Utf8(String::from_utf8(trimmed.to_vec()).map_err(|_| TypesError::MalformedKey)?)
        }
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
