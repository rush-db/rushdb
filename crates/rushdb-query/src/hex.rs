//! Record-id hex decoding.

/// Decode exactly 16 bytes from a 32-character hex string (case-insensitive).
///
/// Returns `None` when the input is shorter than 32 characters or contains a
/// non-hex digit.
#[must_use]
pub fn hex_decode_16(hex: &str) -> Option<[u8; 16]> {
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        let hi = hex.as_bytes().get(i * 2)?.hex_val()?;
        let lo = hex.as_bytes().get(i * 2 + 1)?.hex_val()?;
        *byte = (hi << 4) | lo;
    }
    Some(out)
}

trait HexVal {
    fn hex_val(self) -> Option<u8>;
}

impl HexVal for u8 {
    fn hex_val(self) -> Option<u8> {
        match self {
            b'0'..=b'9' => Some(self - b'0'),
            b'a'..=b'f' => Some(self - b'a' + 10),
            b'A'..=b'F' => Some(self - b'A' + 10),
            _ => None,
        }
    }
}
