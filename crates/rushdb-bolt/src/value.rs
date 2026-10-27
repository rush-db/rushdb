//! Bolt packed-stream value codec (encode/decode of the Neo4j binary value
//! types used by the Bolt protocol).

use serde_json::{Number, Value};

/// Marker for a tiny string of up to 15 bytes; the low nibble is the length.
pub const MARKER_TINY_STRING: u8 = 0x80;
/// Marker for a tiny list of up to 15 elements; the low nibble is the count.
pub const MARKER_TINY_LIST: u8 = 0x90;
/// Marker for a tiny map of up to 15 entries; the low nibble is the count.
pub const MARKER_TINY_MAP: u8 = 0xA0;
/// Marker for a tiny struct of up to 15 fields; the low nibble is the count.
pub const MARKER_TINY_STRUCT: u8 = 0xB0;
/// Marker for a null value.
pub const MARKER_NULL: u8 = 0xC0;
/// Marker for a 64-bit IEEE-754 float.
pub const MARKER_FLOAT64: u8 = 0xC1;
/// Marker for boolean `false`.
pub const MARKER_FALSE: u8 = 0xC2;
/// Marker for boolean `true`.
pub const MARKER_TRUE: u8 = 0xC3;
/// Marker for an 8-bit signed integer.
pub const MARKER_INT8: u8 = 0xC8;
/// Marker for a 16-bit signed integer.
pub const MARKER_INT16: u8 = 0xC9;
/// Marker for a 32-bit signed integer.
pub const MARKER_INT32: u8 = 0xCA;
/// Marker for a 64-bit signed integer.
pub const MARKER_INT64: u8 = 0xCB;
/// Marker for a string of up to 255 bytes.
pub const MARKER_STRING8: u8 = 0xD0;
/// Marker for a string of up to 65 535 bytes.
pub const MARKER_STRING16: u8 = 0xD1;
/// Marker for a string of up to 4 294 967 295 bytes.
pub const MARKER_STRING32: u8 = 0xD2;
/// Marker for a list of up to 255 elements.
pub const MARKER_LIST8: u8 = 0xD4;
/// Marker for a list of up to 65 535 elements.
pub const MARKER_LIST16: u8 = 0xD5;
/// Marker for a list of up to 4 294 967 295 elements.
pub const MARKER_LIST32: u8 = 0xD6;
/// Marker for a map of up to 255 entries.
pub const MARKER_MAP8: u8 = 0xD8;
/// Marker for a map of up to 65 535 entries.
pub const MARKER_MAP16: u8 = 0xD9;
/// Marker for a map of up to 4 294 967 295 entries.
pub const MARKER_MAP32: u8 = 0xDA;

/// Maximum number of values in one decoded container. This is independent of
/// the wire byte cap because a tiny payload can declare a huge list/map.
pub const MAX_CONTAINER_ITEMS: usize = 1_000_000;
/// Maximum nesting depth accepted from an untrusted Bolt message.
pub const MAX_NESTING_DEPTH: usize = 64;

/// A decoded Bolt message: a struct tag plus its already-decoded fields.
pub struct Struct {
    /// The Bolt struct tag (for example `0x01` for HELLO).
    pub tag: u8,
    /// The decoded fields, in wire order.
    pub fields: Vec<Value>,
}

// ---- encoding -------------------------------------------------------------

/// Encode a Bolt struct with `tag` and `fields` into `out`.
///
/// # Panics
/// Panics if `fields` exceeds the protocol's 15-field limit. Bolt defines only
/// a 4-bit tiny-struct marker; silently truncating the count would OR into the
/// marker bits and emit a corrupt, undecodable frame.
pub fn encode_struct(tag: u8, fields: Vec<Value>, out: &mut Vec<u8>) {
    let count = u8::try_from(fields.len())
        .ok()
        .filter(|count| *count <= 0x0F)
        .expect("Bolt structs carry at most 15 fields");
    out.push(MARKER_TINY_STRUCT | count);
    out.push(tag);
    for field in fields {
        encode_value(&field, out);
    }
}

/// Encode a JSON object as a Bolt map into `out`.
///
/// # Panics
/// Panics if the map exceeds the protocol's `u32` length limit.
#[expect(
    clippy::cast_possible_truncation,
    reason = "Bolt map markers store the entry count in one byte"
)]
pub fn encode_map(map: &serde_json::Map<String, Value>, out: &mut Vec<u8>) {
    if map.len() <= 0x0F {
        out.push(MARKER_TINY_MAP | map.len() as u8);
    } else if let Ok(len) = u8::try_from(map.len()) {
        out.push(MARKER_MAP8);
        out.push(len);
    } else if let Ok(len) = u16::try_from(map.len()) {
        out.push(MARKER_MAP16);
        out.extend_from_slice(&len.to_be_bytes());
    } else {
        out.push(MARKER_MAP32);
        out.extend_from_slice(
            &u32::try_from(map.len())
                .expect("Bolt map length exceeds the protocol's u32 limit")
                .to_be_bytes(),
        );
    }
    for (key, value) in map {
        encode_string(key, out);
        encode_value(value, out);
    }
}

/// Encode a JSON array as a Bolt list into `out`.
///
/// # Panics
/// Panics if the list exceeds the protocol's `u32` length limit.
#[expect(
    clippy::cast_possible_truncation,
    reason = "Bolt list markers store the element count in one byte"
)]
pub fn encode_list(list: &[Value], out: &mut Vec<u8>) {
    if list.len() <= 0x0F {
        out.push(MARKER_TINY_LIST | list.len() as u8);
    } else if let Ok(len) = u8::try_from(list.len()) {
        out.push(MARKER_LIST8);
        out.push(len);
    } else if let Ok(len) = u16::try_from(list.len()) {
        out.push(MARKER_LIST16);
        out.extend_from_slice(&len.to_be_bytes());
    } else {
        out.push(MARKER_LIST32);
        out.extend_from_slice(
            &u32::try_from(list.len())
                .expect("Bolt list length exceeds the protocol's u32 limit")
                .to_be_bytes(),
        );
    }
    for value in list {
        encode_value(value, out);
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Bolt string markers store the byte length in 8 or 16 bits"
)]
fn encode_string(s: &str, out: &mut Vec<u8>) {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len <= 0x0F {
        out.push(MARKER_TINY_STRING | len as u8);
    } else if let Ok(short) = u8::try_from(len) {
        out.push(MARKER_STRING8);
        out.push(short);
    } else if let Ok(short) = u16::try_from(len) {
        out.push(MARKER_STRING16);
        out.extend_from_slice(&short.to_be_bytes());
    } else {
        out.push(MARKER_STRING32);
        out.extend_from_slice(
            &u32::try_from(len)
                .expect("Bolt string length exceeds the protocol's u32 limit")
                .to_be_bytes(),
        );
    }
    out.extend_from_slice(bytes);
}

fn encode_int(i: i64, out: &mut Vec<u8>) {
    if (0..=127).contains(&i) {
        out.push(u8::try_from(i).expect("0..=127 fits in a u8"));
    } else if (-16..0).contains(&i) {
        out.push(u8::try_from(i & 0xFF).expect("masked value is 0..=255"));
    } else if let Ok(value) = i8::try_from(i) {
        out.push(MARKER_INT8);
        out.push(value.to_ne_bytes()[0]);
    } else if let Ok(value) = i16::try_from(i) {
        out.push(MARKER_INT16);
        out.extend_from_slice(&value.to_be_bytes());
    } else if let Ok(value) = i32::try_from(i) {
        out.push(MARKER_INT32);
        out.extend_from_slice(&value.to_be_bytes());
    } else {
        out.push(MARKER_INT64);
        out.extend_from_slice(&i.to_be_bytes());
    }
}

/// Encode a single JSON value into `out` using Bolt packed-stream markers.
pub fn encode_value(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.push(MARKER_NULL),
        Value::Bool(b) => out.push(if *b { MARKER_TRUE } else { MARKER_FALSE }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                encode_int(i, out);
            } else if let Some(f) = n.as_f64() {
                out.push(MARKER_FLOAT64);
                out.extend_from_slice(&f.to_be_bytes());
            } else {
                out.push(MARKER_NULL);
            }
        }
        Value::String(s) => encode_string(s, out),
        Value::Array(list) => encode_list(list, out),
        Value::Object(map) => encode_map(map, out),
    }
}

// ---- decoding -------------------------------------------------------------

/// Decode a full Bolt packed-stream value from `buf`.
///
/// # Errors
///
/// Returns an error if the buffer is truncated or contains an unsupported
/// marker or invalid UTF-8.
pub fn decode(buf: &[u8]) -> Result<Value, String> {
    let mut cursor = 0;
    let value = decode_at(buf, &mut cursor)?;
    if cursor != buf.len() {
        return Err(format!(
            "bolt: trailing bytes after value ({} bytes)",
            buf.len() - cursor
        ));
    }
    Ok(value)
}

fn read_exact<'a>(buf: &'a [u8], cursor: &mut usize, n: usize) -> Result<&'a [u8], String> {
    let end = cursor
        .checked_add(n)
        .ok_or_else(|| "bolt: value length overflow".to_string())?;
    if end > buf.len() {
        return Err(format!(
            "bolt: truncated value (need {n} bytes at {})",
            *cursor
        ));
    }
    let slice = &buf[*cursor..end];
    *cursor = end;
    Ok(slice)
}

fn read_u8(buf: &[u8], cursor: &mut usize) -> Result<u8, String> {
    Ok(read_exact(buf, cursor, 1)?[0])
}

fn read_array<const N: usize>(buf: &[u8], cursor: &mut usize) -> Result<[u8; N], String> {
    let slice = read_exact(buf, cursor, N)?;
    let mut bytes = [0u8; N];
    bytes.copy_from_slice(slice);
    Ok(bytes)
}

fn read_u16(buf: &[u8], cursor: &mut usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes(read_array(buf, cursor)?))
}

fn read_u32(buf: &[u8], cursor: &mut usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(read_array(buf, cursor)?))
}

fn read_len32(buf: &[u8], cursor: &mut usize) -> Result<usize, String> {
    usize::try_from(read_u32(buf, cursor)?)
        .map_err(|_| "bolt: declared length exceeds address space".to_string())
}

fn decode_string(buf: &[u8], cursor: &mut usize, len: usize) -> Result<Value, String> {
    let bytes = read_exact(buf, cursor, len)?;
    String::from_utf8(bytes.to_vec())
        .map(Value::String)
        .map_err(|e| format!("bolt: invalid UTF-8 string: {e}"))
}

/// Decode one Bolt packed-stream value starting at `cursor`, advancing it.
///
/// # Errors
///
/// Returns an error if the value is truncated or uses an unsupported marker.
pub fn decode_at(buf: &[u8], cursor: &mut usize) -> Result<Value, String> {
    decode_at_depth(buf, cursor, 0)
}

fn decode_at_depth(buf: &[u8], cursor: &mut usize, depth: usize) -> Result<Value, String> {
    if depth > MAX_NESTING_DEPTH {
        return Err("bolt: value nesting depth exceeds limit".into());
    }
    let marker = read_u8(buf, cursor)?;
    decode_marker(marker, buf, cursor, depth)
}

fn decode_marker(
    marker: u8,
    buf: &[u8],
    cursor: &mut usize,
    depth: usize,
) -> Result<Value, String> {
    match marker {
        0x00..=0x7F => Ok(Value::Number(Number::from(i64::from(marker)))),
        0xF0..=0xFF => Ok(Value::Number(Number::from(i64::from(marker) - 256))),
        0x80..=0x8F => decode_string(buf, cursor, usize::from(marker & 0x0F)),
        MARKER_NULL => Ok(Value::Null),
        MARKER_FALSE => Ok(Value::Bool(false)),
        MARKER_TRUE => Ok(Value::Bool(true)),
        MARKER_FLOAT64 => {
            let float = f64::from_be_bytes(read_array(buf, cursor)?);
            Ok(Number::from_f64(float).map_or(Value::Null, Value::Number))
        }
        MARKER_INT8 => {
            let value = i8::from_be_bytes(read_array(buf, cursor)?);
            Ok(Value::Number(Number::from(i64::from(value))))
        }
        MARKER_INT16 => {
            let value = i16::from_be_bytes(read_array(buf, cursor)?);
            Ok(Value::Number(Number::from(i64::from(value))))
        }
        MARKER_INT32 => {
            let value = i32::from_be_bytes(read_array(buf, cursor)?);
            Ok(Value::Number(Number::from(i64::from(value))))
        }
        MARKER_INT64 => {
            let value = i64::from_be_bytes(read_array(buf, cursor)?);
            Ok(Value::Number(Number::from(value)))
        }
        MARKER_STRING8 => {
            let len = usize::from(read_u8(buf, cursor)?);
            decode_string(buf, cursor, len)
        }
        MARKER_STRING16 => {
            let len = usize::from(read_u16(buf, cursor)?);
            decode_string(buf, cursor, len)
        }
        MARKER_STRING32 => {
            let len = read_len32(buf, cursor)?;
            decode_string(buf, cursor, len)
        }
        0x90..=0x9F => decode_list(buf, cursor, usize::from(marker & 0x0F), depth),
        MARKER_LIST8 => {
            let len = usize::from(read_u8(buf, cursor)?);
            decode_list(buf, cursor, len, depth)
        }
        MARKER_LIST16 => {
            let len = usize::from(read_u16(buf, cursor)?);
            decode_list(buf, cursor, len, depth)
        }
        MARKER_LIST32 => {
            let len = read_len32(buf, cursor)?;
            decode_list(buf, cursor, len, depth)
        }
        0xA0..=0xAF => decode_map(buf, cursor, usize::from(marker & 0x0F), depth),
        MARKER_MAP8 => {
            let len = usize::from(read_u8(buf, cursor)?);
            decode_map(buf, cursor, len, depth)
        }
        MARKER_MAP16 => {
            let len = usize::from(read_u16(buf, cursor)?);
            decode_map(buf, cursor, len, depth)
        }
        MARKER_MAP32 => {
            let len = read_len32(buf, cursor)?;
            decode_map(buf, cursor, len, depth)
        }
        0xB0..=0xBF => decode_struct(buf, cursor, usize::from(marker & 0x0F), depth),
        other => Err(format!("bolt: unsupported marker 0x{other:02x}")),
    }
}

fn decode_struct(
    buf: &[u8],
    cursor: &mut usize,
    size: usize,
    depth: usize,
) -> Result<Value, String> {
    check_container_size(size)?;
    let tag = read_u8(buf, cursor)?;
    let mut fields = Vec::with_capacity(size);
    for _ in 0..size {
        fields.push(decode_at_depth(buf, cursor, depth + 1)?);
    }
    // Represent structs as a tagged object so callers can dispatch.
    let mut map = serde_json::Map::new();
    map.insert(
        "__bolt_tag".into(),
        Value::Number(Number::from(i64::from(tag))),
    );
    map.insert("__bolt_fields".into(), Value::Array(fields));
    Ok(Value::Object(map))
}

fn decode_list(buf: &[u8], cursor: &mut usize, len: usize, depth: usize) -> Result<Value, String> {
    check_container_size(len)?;
    let mut list = Vec::with_capacity(len);
    for _ in 0..len {
        list.push(decode_at_depth(buf, cursor, depth + 1)?);
    }
    Ok(Value::Array(list))
}

fn decode_map(buf: &[u8], cursor: &mut usize, len: usize, depth: usize) -> Result<Value, String> {
    check_container_size(len)?;
    let mut map = serde_json::Map::new();
    for _ in 0..len {
        let key = decode_at_depth(buf, cursor, depth + 1)?;
        let key = match key {
            Value::String(s) => s,
            other => format!("{other}"),
        };
        let value = decode_at_depth(buf, cursor, depth + 1)?;
        map.insert(key, value);
    }
    Ok(Value::Object(map))
}

fn check_container_size(size: usize) -> Result<(), String> {
    if size > MAX_CONTAINER_ITEMS {
        return Err("bolt: container item count exceeds limit".into());
    }
    Ok(())
}

/// Extract the struct tag from a decoded struct-object.
#[must_use]
pub fn struct_tag(value: &Value) -> Option<u8> {
    value
        .get("__bolt_tag")
        .and_then(serde_json::Value::as_i64)
        .and_then(|tag| u8::try_from(tag).ok())
}

/// Extract the struct fields from a decoded struct-object.
#[must_use]
pub fn struct_fields(value: &Value) -> Vec<Value> {
    value
        .get("__bolt_fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(value: &Value) {
        let mut out = Vec::new();
        encode_value(value, &mut out);
        let decoded = decode(&out).unwrap();
        assert_eq!(&decoded, value, "roundtrip failed for {value}");
    }

    #[test]
    fn roundtrips_scalars_and_containers() {
        roundtrip(&serde_json::json!(null));
        roundtrip(&serde_json::json!(true));
        roundtrip(&serde_json::json!(false));
        roundtrip(&serde_json::json!(0));
        roundtrip(&serde_json::json!(127));
        roundtrip(&serde_json::json!(-16));
        roundtrip(&serde_json::json!(-17));
        roundtrip(&serde_json::json!(300));
        roundtrip(&serde_json::json!(70_000));
        roundtrip(&serde_json::json!(-4_000_000_000_i64));
        roundtrip(&serde_json::json!(9_000_000_000_000_000_000_i64));
        roundtrip(&serde_json::json!(3.25));
        roundtrip(&serde_json::json!("hello"));
        roundtrip(&serde_json::json!("a".repeat(40)));
        roundtrip(&serde_json::json!("a".repeat(300)));
        roundtrip(&serde_json::json!([1, "two", 3.0, null, true]));
        roundtrip(&serde_json::json!({ "name": "ada", "age": 36, "tags": ["x", "y"] }));
    }

    #[test]
    fn encodes_tiny_markers() {
        let mut out = Vec::new();
        encode_string("hi", &mut out);
        assert_eq!(out[0], 0x82);
        let mut out = Vec::new();
        encode_int(5, &mut out);
        assert_eq!(out[0], 5);
        let mut out = Vec::new();
        encode_int(-5, &mut out);
        assert_eq!(out[0], 0xFB);
    }

    /// The byte values are `PackStream`'s, as Neo4j drivers read them; a codec
    /// that only round-trips through itself cannot catch a shifted marker.
    #[test]
    fn encodes_scalars_with_packstream_markers() {
        let encoded = |value: Value| {
            let mut out = Vec::new();
            encode_value(&value, &mut out);
            out
        };
        assert_eq!(encoded(Value::Null), [0xC0]);
        assert_eq!(encoded(Value::Bool(false)), [0xC2]);
        assert_eq!(encoded(Value::Bool(true)), [0xC3]);
        assert_eq!(
            encoded(serde_json::json!(1.5)),
            [0xC1, 0x3F, 0xF8, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(decode(&[0xC3]).unwrap(), Value::Bool(true));
        assert_eq!(decode(&[0xC2]).unwrap(), Value::Bool(false));
        assert_eq!(
            decode(&[0xC1, 0x3F, 0xF8, 0, 0, 0, 0, 0, 0]).unwrap(),
            serde_json::json!(1.5)
        );
    }

    #[test]
    fn decodes_struct() {
        let mut out = Vec::new();
        encode_struct(0x01, vec![serde_json::json!({"a": 1})], &mut out);
        let value = decode(&out).unwrap();
        assert_eq!(struct_tag(&value), Some(0x01));
        let fields = struct_fields(&value);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0]["a"], 1);
    }

    #[test]
    fn rejects_declared_huge_containers_and_deep_nesting() {
        let mut huge = vec![MARKER_LIST32];
        huge.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode(&huge).unwrap_err().contains("item count"));

        let mut nested = vec![0x91; MAX_NESTING_DEPTH + 2];
        nested.push(MARKER_NULL);
        assert!(decode(&nested).unwrap_err().contains("nesting depth"));
    }

    #[test]
    fn uses_wide_markers_at_length_boundaries_and_rejects_trailing_bytes() {
        let string_255 = "x".repeat(255);
        let string_256 = "x".repeat(256);
        let string_65536 = "x".repeat(65_536);
        let mut encoded = Vec::new();
        encode_value(&Value::String(string_255), &mut encoded);
        assert_eq!(encoded[0], MARKER_STRING8);
        encoded.clear();
        encode_value(&Value::String(string_256), &mut encoded);
        assert_eq!(encoded[0], MARKER_STRING16);
        encoded.clear();
        encode_value(&Value::String(string_65536), &mut encoded);
        assert_eq!(encoded[0], MARKER_STRING32);
        assert!(decode(&[MARKER_NULL, MARKER_NULL]).is_err());

        let list = vec![Value::Null; 256];
        encoded.clear();
        encode_list(&list, &mut encoded);
        assert_eq!(encoded[0], MARKER_LIST16);
        assert_eq!(decode(&encoded).unwrap().as_array().unwrap().len(), 256);
    }
}
