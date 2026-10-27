//! Bounded, append-friendly MVCC history blocks.
//!
//! A key's `HISTORY` row is the **head**: the newest `<= BLOCK_SIZE` versions.
//! When the head overflows, the oldest `BLOCK_SIZE` versions are sealed into a
//! `HISTORY_BLOCK` row linked by a `prev` seal sequence, so a hot-key append
//! rewrites only the bounded head (amortized O(1)) instead of the whole version
//! list. Block layout (little-endian):
//!
//! ```text
//! magic    : 4 bytes ("V2BL")
//! prev     : u64   (seal seq of the next-older block; 0 = end of chain)
//! count    : u32
//! entries[count]:
//!   from_ci  : u64
//!   to_ci    : u64
//!   value_len: u32   (0xFFFF_FFFF = null/deleted)
//!   value    : value_len bytes
//! ```
//!
//! This is the greenfield runtime format: there is no legacy JSON array or
//! older envelope to decode.

use crate::VersionEntry;

/// Sentinel for a null (deleted) historical value.
const NULL_VALUE_LEN: u32 = u32::MAX;

/// Marker for a bounded history block.
pub const BLOCK_MAGIC: &[u8; 4] = b"V2BL";

/// Maximum versions held in a head/block before the oldest are sealed. Bounds
/// the per-append rewrite to this many entries, so a hot key appends in
/// amortized O(1) instead of rewriting its whole version list.
pub const BLOCK_SIZE: usize = 128;

/// Maximum encoded bytes in a sealed block (excluding a single oversized
/// value, which remains readable as one entry). This bounds decode/copy work
/// for ordinary histories independently of version count.
pub const BLOCK_MAX_BYTES: usize = 1024 * 1024;

/// Hard decode ceiling matching the storage read-item envelope. The smaller
/// sealing target bounds ordinary blocks; this guard prevents corrupt or
/// hostile history rows from allocating an unbounded version vector.
pub const MAX_DECODE_BYTES: usize = 64 * 1024 * 1024;

/// Expiry-index key for one version: the superseding commit index (big-endian,
/// so byte order == expiry order) followed by the data key. A range bounded by
/// `to_ci <= watermark` therefore yields exactly the expired versions, letting
/// GC target them without scanning every history key.
#[must_use]
pub fn expiry_key(to_ci: u64, data_key: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + data_key.len());
    out.extend_from_slice(&to_ci.to_be_bytes());
    out.extend_from_slice(data_key);
    out
}

/// Data key of an expiry-index key (everything after the 8-byte `to_ci`).
#[must_use]
pub fn expiry_data_key(key: &[u8]) -> Option<&[u8]> {
    key.get(8..)
}

/// Exclusive upper bound selecting every expiry key with `to_ci <= watermark`.
/// `None` means unbounded (only when `watermark == u64::MAX`).
#[must_use]
pub fn expiry_upper_bound(watermark: u64) -> Option<Vec<u8>> {
    watermark
        .checked_add(1)
        .map(|next| next.to_be_bytes().to_vec())
}

fn encode_entries(out: &mut Vec<u8>, versions: &[VersionEntry]) {
    for v in versions {
        out.extend_from_slice(&v.from_ci.to_le_bytes());
        out.extend_from_slice(&v.to_ci.to_le_bytes());
        match &v.value {
            Some(value) => {
                out.extend_from_slice(
                    &u32::try_from(value.len())
                        .expect("version value length fits u32")
                        .to_le_bytes(),
                );
                out.extend_from_slice(value);
            }
            None => out.extend_from_slice(&NULL_VALUE_LEN.to_le_bytes()),
        }
    }
}

fn decode_entries(raw: &[u8], count: usize, mut pos: usize) -> Result<Vec<VersionEntry>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|error| format!("invalid history block: entry allocation rejected: {error}"))?;
    for _ in 0..count {
        if pos + 20 > raw.len() {
            return Err("invalid history block: truncated".into());
        }
        let from_ci = u64::from_le_bytes(
            raw[pos..pos + 8]
                .try_into()
                .map_err(|_| "invalid history block: corrupt entry")?,
        );
        let to_ci = u64::from_le_bytes(
            raw[pos + 8..pos + 16]
                .try_into()
                .map_err(|_| "invalid history block: corrupt entry")?,
        );
        let value_len = u32::from_le_bytes(
            raw[pos + 16..pos + 20]
                .try_into()
                .map_err(|_| "invalid history block: corrupt entry")?,
        );
        pos += 20;
        let value = if value_len == NULL_VALUE_LEN {
            None
        } else {
            let end = pos + value_len as usize;
            if end > raw.len() {
                return Err("invalid history block: truncated value".into());
            }
            let value = raw[pos..end].to_vec();
            pos = end;
            Some(value)
        };
        out.push(VersionEntry {
            from_ci,
            to_ci,
            value,
        });
    }
    if pos != raw.len() {
        return Err("invalid history block: trailing bytes".into());
    }
    Ok(out)
}

/// Encode a history block: `V2BL || prev:u64 || count:u32 || entries`. `prev`
/// is the seal sequence of the next-older block, or `0` for the end of chain.
///
/// # Panics
/// Panics if the version count or any value length exceeds `u32::MAX`.
#[must_use]
pub fn encode_block(prev: u64, versions: &[VersionEntry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + versions.len() * 20);
    out.extend_from_slice(BLOCK_MAGIC);
    out.extend_from_slice(&prev.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(versions.len())
            .expect("version count fits u32")
            .to_le_bytes(),
    );
    encode_entries(&mut out, versions);
    out
}

/// Decode a history block into `(prev, versions)`.
///
/// # Errors
/// Returns a description when the magic is wrong or the body is truncated.
///
/// # Panics
/// Panics only if an internal fixed-width slice conversion fails after its
/// length has been bounds-checked, which cannot happen.
pub fn decode_block(raw: &[u8]) -> Result<(u64, Vec<VersionEntry>), String> {
    if raw.len() > MAX_DECODE_BYTES {
        return Err("invalid history block: exceeds decode limit".into());
    }
    if raw.len() < 16 || &raw[0..4] != BLOCK_MAGIC {
        return Err("invalid history block: bad magic".into());
    }
    let prev = u64::from_le_bytes(
        raw[4..12]
            .try_into()
            .map_err(|_| "invalid history block: corrupt header")?,
    );
    let count = u32::from_le_bytes(
        raw[12..16]
            .try_into()
            .map_err(|_| "invalid history block: corrupt header")?,
    ) as usize;
    if count > (raw.len() - 16) / 20 {
        return Err("invalid history block: truncated entries".into());
    }
    let versions = decode_entries(raw, count, 16)?;
    Ok((prev, versions))
}

/// Key for a sealed history block of `data_key`, in the dedicated block table.
/// Length-prefixed so a data key that is a byte-prefix of another cannot alias.
#[must_use]
pub fn block_key(data_key: &[u8], seq: u64) -> Vec<u8> {
    let len = u64::try_from(data_key.len()).unwrap_or(u64::MAX);
    let mut out = Vec::with_capacity(16 + data_key.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data_key);
    out.extend_from_slice(&seq.to_be_bytes());
    out
}

/// Prefix covering every sealed block of `data_key`.
#[must_use]
pub fn block_prefix(data_key: &[u8]) -> Vec<u8> {
    let len = u64::try_from(data_key.len()).unwrap_or(u64::MAX);
    let mut out = Vec::with_capacity(8 + data_key.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data_key);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(from: u64, to: u64, value: Option<&[u8]>) -> VersionEntry {
        VersionEntry {
            from_ci: from,
            to_ci: to,
            value: value.map(<[u8]>::to_vec),
        }
    }

    #[test]
    fn expiry_keys_order_by_to_ci_then_data_key() {
        let a = expiry_key(2, b"k1");
        let b = expiry_key(2, b"k2");
        let c = expiry_key(3, b"k0");
        assert!(a < b, "same to_ci orders by data key");
        assert!(b < c, "smaller to_ci orders first");
        assert_eq!(expiry_data_key(&c), Some(b"k0".as_slice()));
        assert_eq!(expiry_data_key(b"short"), None);
    }

    #[test]
    fn expiry_upper_bound_excludes_unexpired_versions() {
        let bound = expiry_upper_bound(5).unwrap();
        // A version expiring at 5 is below the bound; at 6 is not.
        assert!(expiry_key(5, b"k") < bound);
        assert!(expiry_key(6, b"k") >= bound);
        assert_eq!(expiry_upper_bound(u64::MAX), None);
    }

    #[test]
    fn block_round_trips_with_prev_link() {
        let versions = vec![
            entry(1, 5, Some(b"a")),
            entry(5, 9, None),
            entry(9, 0, Some(b"longer value")),
        ];
        let raw = encode_block(42, &versions);
        let (prev, decoded) = decode_block(&raw).unwrap();
        assert_eq!(prev, 42);
        assert_eq!(decoded, versions);
        assert!(decode_block(b"nope").is_err());
        // A truncated block is rejected.
        assert!(decode_block(&raw[..raw.len() - 1]).is_err());
    }

    #[test]
    fn decode_rejects_oversized_history_before_allocating_entries() {
        let raw = vec![0u8; MAX_DECODE_BYTES + 1];
        assert!(decode_block(&raw).is_err());
    }

    #[test]
    fn corrupt_entry_count_is_rejected_before_reservation() {
        let mut raw = Vec::from(&BLOCK_MAGIC[..]);
        raw.extend_from_slice(&0u64.to_le_bytes());
        raw.extend_from_slice(&u32::MAX.to_le_bytes());
        let error = decode_block(&raw).unwrap_err();
        assert!(error.contains("truncated entries"), "{error}");
    }

    #[test]
    fn block_keys_are_length_prefixed_and_unambiguous() {
        // `a` is a byte prefix of `ab`; the length segment keeps them distinct.
        assert_ne!(block_key(b"a", 1), block_key(b"ab", 1));
        assert!(block_key(b"a", 1).starts_with(&block_prefix(b"a")));
        assert!(!block_key(b"ab", 1).starts_with(&block_prefix(b"a")));
    }
}
