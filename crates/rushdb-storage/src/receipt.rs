//! Retry receipts: one row per commit key answers "did this
//! commit already happen, and what did it return?" for
//! `idempotency-window-seconds`, then a bounded sweep removes it.
//!
//! `RECEIPT(uuid) → status ‖ ci ‖ created_ms ‖ payload` is the single
//! source of the outcome (it replaces the separate status and payload rows,
//! which were kept forever and pruned by commit count respectively);
//! `RECEIPT_EXP(created_ms, uuid)` orders receipts by age for the sweep. A
//! receipt older than the window is treated as absent even before the
//! sweep reaches it, so a late retry executes as a new commit.

use uuid::Uuid;

use crate::{OutcomeStatus, StorageError};

/// Both receipt families (storage accounting ranges over this).
pub(crate) const RECEIPT_FAMILY: &[u8] = b"receipt";
/// Receipt rows, keyed by commit key.
pub(crate) const RECEIPT_PREFIX: &[u8] = b"receipt/";
/// Expiry index, keyed by creation time then commit key.
pub(crate) const RECEIPT_EXP_PREFIX: &[u8] = b"receipt-exp/";

const STATUS_ABORTED: u8 = 0;
const STATUS_COMMITTED: u8 = 1;
/// Payload flag: the response body follows as stored.
const PAYLOAD_RAW: u8 = 1;
/// Payload flag: the response body follows zstd-compressed (a 500-document ingest response shrinks several-fold).
const PAYLOAD_ZSTD: u8 = 2;
/// zstd level for receipt payloads: fast, written once per commit.
const PAYLOAD_ZSTD_LEVEL: i32 = 3;
/// `status(1) + ci(8) + created_ms(8)`; a payload follows for commits that
/// stored one.
const HEADER_LEN: usize = 17;

pub(crate) fn receipt_key(commit_key: Uuid) -> Vec<u8> {
    let mut key = RECEIPT_PREFIX.to_vec();
    key.extend_from_slice(commit_key.as_bytes());
    key
}

pub(crate) fn expiry_key(created_ms: u64, commit_key: Uuid) -> Vec<u8> {
    let mut key = RECEIPT_EXP_PREFIX.to_vec();
    key.extend_from_slice(&created_ms.to_be_bytes());
    key.extend_from_slice(commit_key.as_bytes());
    key
}

/// Commit key of an expiry-index row.
pub(crate) fn expiry_commit_key(key: &[u8]) -> Result<(u64, Uuid), StorageError> {
    let rest = key
        .strip_prefix(RECEIPT_EXP_PREFIX)
        .filter(|rest| rest.len() == 24)
        .ok_or(StorageError::CorruptLog)?;
    let created = u64::from_be_bytes(rest[..8].try_into().map_err(|_| StorageError::CorruptLog)?);
    let id = Uuid::from_slice(&rest[8..]).map_err(|_| StorageError::CorruptLog)?;
    Ok((created, id))
}

/// A decoded receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub(crate) status: OutcomeStatus,
    pub(crate) commit_index: u64,
    pub(crate) created_ms: u64,
    /// The stored response body; `None` for aborts and for commits applied
    /// without one (command replay, replication without a payload).
    pub(crate) payload: Option<Vec<u8>>,
}

impl Receipt {
    /// Whether the receipt is past the idempotency window at `now_ms`.
    pub(crate) fn expired(&self, now_ms: u64, window_ms: u64) -> bool {
        now_ms.saturating_sub(self.created_ms) > window_ms
    }
}

pub(crate) fn encode(
    status: OutcomeStatus,
    commit_index: u64,
    created_ms: u64,
    payload: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + 1 + payload.map_or(0, <[u8]>::len));
    out.push(match status {
        OutcomeStatus::Aborted => STATUS_ABORTED,
        OutcomeStatus::Committed => STATUS_COMMITTED,
    });
    out.extend_from_slice(&commit_index.to_be_bytes());
    out.extend_from_slice(&created_ms.to_be_bytes());
    // An explicit flag byte keeps an empty payload distinct from none; the
    // body is compressed when that actually saves bytes.
    if let Some(payload) = payload {
        match zstd::bulk::compress(payload, PAYLOAD_ZSTD_LEVEL) {
            Ok(compressed) if compressed.len() < payload.len() => {
                out.push(PAYLOAD_ZSTD);
                out.extend_from_slice(&compressed);
            }
            _ => {
                out.push(PAYLOAD_RAW);
                out.extend_from_slice(payload);
            }
        }
    }
    out
}

pub(crate) fn decode(raw: &[u8]) -> Result<Receipt, StorageError> {
    if raw.len() < HEADER_LEN {
        return Err(StorageError::CorruptLog);
    }
    let status = match raw[0] {
        STATUS_ABORTED => OutcomeStatus::Aborted,
        STATUS_COMMITTED => OutcomeStatus::Committed,
        _ => return Err(StorageError::CorruptLog),
    };
    let commit_index =
        u64::from_be_bytes(raw[1..9].try_into().map_err(|_| StorageError::CorruptLog)?);
    let created_ms = u64::from_be_bytes(
        raw[9..17]
            .try_into()
            .map_err(|_| StorageError::CorruptLog)?,
    );
    let payload = match raw.get(HEADER_LEN) {
        None => None,
        Some(&PAYLOAD_RAW) => Some(raw[HEADER_LEN + 1..].to_vec()),
        Some(&PAYLOAD_ZSTD) => Some(
            zstd::stream::decode_all(&raw[HEADER_LEN + 1..])
                .map_err(|_| StorageError::CorruptLog)?,
        ),
        Some(_) => return Err(StorageError::CorruptLog),
    };
    if status == OutcomeStatus::Aborted && payload.is_some() {
        return Err(StorageError::CorruptLog);
    }
    Ok(Receipt {
        status,
        commit_index,
        created_ms,
        payload,
    })
}

/// The two rows of one receipt.
pub(crate) fn rows(
    commit_key: Uuid,
    status: OutcomeStatus,
    commit_index: u64,
    created_ms: u64,
    payload: Option<&[u8]>,
) -> [(Vec<u8>, Vec<u8>); 2] {
    [
        (
            receipt_key(commit_key),
            encode(status, commit_index, created_ms, payload),
        ),
        (expiry_key(created_ms, commit_key), Vec::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response body shaped like an ingest acknowledgement: record ids.
    fn serde_json_like_ids(count: usize) -> Vec<u8> {
        let ids: Vec<String> = (0..count)
            .map(|i| {
                format!(
                    "{{\"id\":\"{}\",\"label\":\"User\"}}",
                    Uuid::from_u128(i as u128)
                )
            })
            .collect();
        format!("[{}]", ids.join(",")).into_bytes()
    }

    #[test]
    fn receipt_codec_roundtrips_and_rejects_malformed_rows() {
        let id = Uuid::now_v7();
        for (status, payload) in [
            (OutcomeStatus::Committed, Some(&b"{\"ok\":1}"[..])),
            (OutcomeStatus::Committed, Some(&b""[..])),
            (OutcomeStatus::Committed, None),
            (OutcomeStatus::Aborted, None),
        ] {
            let raw = encode(status, 42, 1_000, payload);
            assert_eq!(
                decode(&raw).unwrap(),
                Receipt {
                    status,
                    commit_index: 42,
                    created_ms: 1_000,
                    payload: payload.map(<[u8]>::to_vec),
                }
            );
        }
        // A repetitive response body is stored compressed and read back
        // byte for byte.
        let body = serde_json_like_ids(500);
        let raw = encode(OutcomeStatus::Committed, 7, 9, Some(&body));
        assert_eq!(raw[HEADER_LEN], PAYLOAD_ZSTD);
        assert!(
            raw.len() * 3 < body.len(),
            "{} vs {}",
            raw.len(),
            body.len()
        );
        assert_eq!(decode(&raw).unwrap().payload, Some(body));
        let mut corrupt = raw;
        corrupt.truncate(HEADER_LEN + 4);
        assert!(decode(&corrupt).is_err());
        assert!(decode(&[1; 5]).is_err());
        assert!(decode(&[9; 17]).is_err());
        let mut bad_presence = encode(OutcomeStatus::Committed, 1, 1, None);
        bad_presence.push(7);
        assert!(decode(&bad_presence).is_err());
        let mut aborted_with_payload = encode(OutcomeStatus::Aborted, 1, 1, None);
        aborted_with_payload.push(1);
        assert!(decode(&aborted_with_payload).is_err());
        assert_eq!(expiry_commit_key(&expiry_key(77, id)).unwrap(), (77, id));
        // Expiry order is creation order.
        assert!(expiry_key(5, Uuid::max()) < expiry_key(6, Uuid::nil()));
    }
}
