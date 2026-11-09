//! The one parser of every vector a request carries (`query.vector`, the
//! hybrid vector leg, `vectors.upsert` items, inline `$vectors`).
//!
//! A vector is either a JSON array of numbers or, carrying the exact bits
//! in about a quarter of the text, `{"f32": "<base64>"}`: the standard
//! base64 (RFC 4648, padded) of its components as little-endian IEEE-754
//! `f32`s, back to back. A binary vector whose byte length is not a
//! multiple of 4, or that holds a NaN or an infinity, is refused.

use std::fmt;

use base64::Engine as _;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::Deserializer;

/// Decode the `f32` form's base64 text into components.
///
/// # Errors
/// Returns why the text is not a vector: not base64, a byte length that is
/// not a multiple of 4, or a non-finite component.
pub fn decode_f32_base64(text: &str) -> Result<Vec<f32>, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|error| format!("invalid vector: `f32` is not base64 ({error})"))?;
    if bytes.len() % 4 != 0 {
        return Err(format!(
            "invalid vector: `f32` holds {} bytes, not a whole number of 4-byte floats",
            bytes.len()
        ));
    }
    let values: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect();
    if let Some(at) = values.iter().position(|value| !value.is_finite()) {
        return Err(format!(
            "invalid vector: component {at} is not a finite number"
        ));
    }
    Ok(values)
}

/// Encode components as the `f32` form's base64 text (what a client sends).
#[must_use]
pub fn encode_f32_base64(values: &[f32]) -> String {
    let bytes: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// `#[serde(deserialize_with)]` for a vector field: a number array or
/// `{"f32": "<base64>"}`.
///
/// # Errors
/// Returns the deserializer's error for anything else, or for a binary
/// vector [`decode_f32_base64`] refuses.
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<f32>, D::Error> {
    deserializer.deserialize_any(WireVector)
}

struct WireVector;

impl<'de> Visitor<'de> for WireVector {
    type Value = Vec<f32>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of numbers or {\"f32\": \"<base64 of little-endian f32>\"}")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<f32>, A::Error> {
        let mut values = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(value) = seq.next_element::<f32>()? {
            values.push(value);
        }
        Ok(values)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Vec<f32>, A::Error> {
        let mut values = None;
        while let Some(key) = map.next_key::<String>()? {
            if key != "f32" {
                return Err(de::Error::custom(format!(
                    "invalid vector: unknown field `{key}`, expected `f32`"
                )));
            }
            if values.is_some() {
                return Err(de::Error::duplicate_field("f32"));
            }
            let text: String = map.next_value()?;
            values = Some(decode_f32_base64(&text).map_err(de::Error::custom)?);
        }
        values.ok_or_else(|| de::Error::missing_field("f32"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    struct Holder {
        #[serde(deserialize_with = "deserialize")]
        vector: Vec<f32>,
    }

    fn parse(value: &serde_json::Value) -> Result<Vec<f32>, String> {
        serde_json::from_value::<Holder>(serde_json::json!({ "vector": value }))
            .map(|holder| holder.vector)
            .map_err(|error| error.to_string())
    }

    #[test]
    fn the_binary_form_round_trips_every_bit() {
        // Values a decimal array cannot carry exactly through every client:
        // a subnormal, negative zero, the extremes, and a value whose
        // shortest decimal is long.
        let values = [
            f32::from_bits(1),
            -0.0,
            f32::MAX,
            f32::MIN_POSITIVE,
            0.1,
            -1.0e-30,
            std::f32::consts::PI,
        ];
        let text = encode_f32_base64(&values);
        let parsed = parse(&serde_json::json!({ "f32": text })).unwrap();
        let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&parsed), bits(&values));
        assert_eq!(
            parse(&serde_json::json!([1.0, -2.5])).unwrap(),
            vec![1.0, -2.5]
        );
        assert_eq!(
            parse(&serde_json::json!({ "f32": "" })).unwrap(),
            Vec::<f32>::new()
        );
    }

    #[test]
    fn a_malformed_binary_vector_is_refused() {
        let nan = encode_f32_base64(&[1.0, f32::NAN]);
        let inf = encode_f32_base64(&[f32::INFINITY]);
        for (value, expected) in [
            (serde_json::json!({ "f32": "AAAA_A==" }), "not base64"),
            (
                serde_json::json!({ "f32": "AAAAAAA=" }),
                "not a whole number",
            ),
            (
                serde_json::json!({ "f32": nan }),
                "component 1 is not a finite",
            ),
            (
                serde_json::json!({ "f32": inf }),
                "component 0 is not a finite",
            ),
            (
                serde_json::json!({ "f64": "AAAAAA==" }),
                "unknown field `f64`",
            ),
            (serde_json::json!({}), "missing field `f32`"),
            (serde_json::json!("AAAAAA=="), "an array of numbers"),
        ] {
            let error = parse(&value).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
        // Every refusal of the binary form reads as the caller's input.
        assert!(decode_f32_base64("AAAAAAA=")
            .unwrap_err()
            .starts_with("invalid vector"));
    }
}
