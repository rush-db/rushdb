//! Record framing for the log-first storage substrate. This **is** the
//! write-ahead log: [`crate::log_store`] appends
//! these framed records to segment files, and that durable append is the
//! commit's ack boundary (not the buffered redb materialization). See the
//! manual's `docs/manual/architecture/storage-engine.md`.
//!
//! Every record (an expanded op-set, optionally paired with its originating
//! [`CommitBatch`] — [`encode_ops_record`]) is wrapped in a fixed
//! header with magic, format version, shard id, LSN, payload length, and a
//! CRC32C-style checksum. This lets replay/export/CDC distinguish valid old
//! data, valid new data, partial new data, and corrupted data instead of
//! hoping the filesystem write was atomic.
//!
//! Greenfield: only the framed format is read or written. There is no legacy
//! unframed format to migrate. An in-memory store (no WAL file) instead keeps
//! a row-based command journal — the one remaining user of the bare
//! `__log__/<lsn>` row format.

use crate::Op;
use crc32fast::Hasher as Crc32;
use rushdb_types::command::CommitBatch;
use std::io;

/// Magic bytes identifying a `RushDB` logical WAL record.
const WAL_MAGIC: [u8; 8] = *b"RDBWAL01";

/// Current logical WAL framing format version.
pub const WAL_FORMAT_VERSION: u32 = 2;

/// Header size: magic(8) + version(4) + `shard_id(4)` + lsn(8) + length(4) + crc(4).
const WAL_HEADER_LEN: usize = 8 + 4 + 4 + 8 + 4 + 4;

/// A decoded logical WAL record header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalHeader {
    /// Physical shard that produced the record.
    pub shard_id: u32,
    /// Log sequence number (commit index) of the record.
    pub lsn: u64,
    /// Length of the payload that follows the header.
    pub payload_len: u32,
}

/// Error returned when a logical WAL record fails to decode or verify.
#[derive(Debug, thiserror::Error)]
pub enum WalDecodeError {
    /// The record is shorter than its header or declared payload.
    #[error("short WAL record: expected {expected} bytes, got {actual}")]
    Truncated {
        /// Total length required by the header/payload.
        expected: usize,
        /// Actual bytes available.
        actual: usize,
    },
    /// The record does not begin with the expected magic.
    #[error("invalid WAL magic")]
    BadMagic,
    /// The record uses a framing version this build does not support.
    #[error("unsupported WAL format version {0}")]
    UnsupportedVersion(u32),
    /// The record's CRC does not match its contents.
    #[error("WAL record checksum mismatch")]
    ChecksumMismatch,
    /// The declared payload exceeds the 64 MiB record ceiling.
    #[error("payload length {0} exceeds the 64 MiB record ceiling")]
    OversizedPayload(u32),
}

/// Encode one logical record into a framed, checksummed byte vector.
///
/// # Panics
/// Panics if `payload` is larger than `u32::MAX` bytes, which the 64 MiB
/// record ceiling makes impossible in practice.
#[must_use]
pub fn encode_wal_record(shard_id: u32, lsn: u64, payload: &[u8]) -> Vec<u8> {
    let payload_len = u32::try_from(payload.len()).expect("payload length fits u32");
    let mut out = Vec::with_capacity(WAL_HEADER_LEN + payload.len());
    out.extend_from_slice(&WAL_MAGIC);
    out.extend_from_slice(&WAL_FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&shard_id.to_le_bytes());
    out.extend_from_slice(&lsn.to_le_bytes());
    out.extend_from_slice(&payload_len.to_le_bytes());
    // Checksum covers everything after the length field (length field itself is
    // integrity-checked by the outer redb row checksum; include header+payload
    // for a RustDB-level end-to-end guard).
    let mut hasher = Crc32::new();
    hasher.update(&WAL_MAGIC);
    hasher.update(&WAL_FORMAT_VERSION.to_le_bytes());
    hasher.update(&shard_id.to_le_bytes());
    hasher.update(&lsn.to_le_bytes());
    hasher.update(&payload_len.to_le_bytes());
    hasher.update(payload);
    out.extend_from_slice(&hasher.finalize().to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Decode and verify a framed record; returns the header and payload.
///
/// # Errors
/// Returns [`WalDecodeError`] when the record is truncated, has bad magic, an
/// unsupported version, an oversized payload, or a checksum mismatch.
///
/// # Panics
/// Panics only if an internal fixed-width slice conversion fails after its
/// length has been bounds-checked, which cannot happen.
pub fn decode_wal_record(raw: &[u8]) -> Result<(WalHeader, &[u8]), WalDecodeError> {
    if raw.len() < WAL_HEADER_LEN {
        return Err(WalDecodeError::Truncated {
            expected: WAL_HEADER_LEN,
            actual: raw.len(),
        });
    }
    if raw[0..8] != WAL_MAGIC {
        return Err(WalDecodeError::BadMagic);
    }
    let version = u32::from_le_bytes(raw[8..12].try_into().expect("slice"));
    if version != WAL_FORMAT_VERSION {
        return Err(WalDecodeError::UnsupportedVersion(version));
    }
    let shard_id = u32::from_le_bytes(raw[12..16].try_into().expect("slice"));
    let lsn = u64::from_le_bytes(raw[16..24].try_into().expect("slice"));
    let payload_len = u32::from_le_bytes(raw[24..28].try_into().expect("slice"));
    if payload_len > 64 * 1024 * 1024 {
        return Err(WalDecodeError::OversizedPayload(payload_len));
    }
    let expected = WAL_HEADER_LEN + payload_len as usize;
    if raw.len() < expected {
        return Err(WalDecodeError::Truncated {
            expected,
            actual: raw.len(),
        });
    }
    let stored_crc = u32::from_le_bytes(raw[28..32].try_into().expect("slice"));
    let mut hasher = Crc32::new();
    hasher.update(&raw[0..28]);
    hasher.update(&raw[WAL_HEADER_LEN..expected]);
    let computed = hasher.finalize();
    if stored_crc != computed {
        return Err(WalDecodeError::ChecksumMismatch);
    }
    Ok((
        WalHeader {
            shard_id,
            lsn,
            payload_len,
        },
        &raw[WAL_HEADER_LEN..expected],
    ))
}

/// Wall-clock now in milliseconds since the Unix epoch (used for retention
/// lease deadlines and checkpoint publication timestamps).
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Encode the `LAST_APPLIED` watermark row (a framed record with an empty
/// payload carries the LSN in the header).
#[must_use]
pub fn encode_last_applied(shard_id: u32, lsn: u64) -> Vec<u8> {
    encode_wal_record(shard_id, lsn, &[])
}

/// Decode the `LAST_APPLIED` watermark row.
///
/// # Errors
/// Returns a human-readable description if the frame is invalid or its
/// checksum does not verify.
pub fn decode_last_applied(raw: &[u8]) -> Result<u64, String> {
    let (header, _) = decode_wal_record(raw).map_err(|e| e.to_string())?;
    Ok(header.lsn)
}

/// Encode an expanded op-set as a compact binary WAL payload: an op count
/// followed by `(tag, key, [value])` entries. This is the log-first WAL's
/// record content (decision B): the log stores state transitions, so recovery
/// applies them directly and needs no expander.
///
/// # Panics
/// Panics if an op-set has more than `u32::MAX` ops or a key/value exceeds
/// `u32::MAX` bytes, which the 64 MiB record ceiling makes impossible.
#[must_use]
pub fn encode_op_set(ops: &[Op]) -> Vec<u8> {
    let mut out = Vec::with_capacity(op_set_encoded_len(ops));
    write_op_set(&mut out, ops);
    out
}

/// Append the op-set encoding of `ops` to `out`.
fn write_op_set(out: &mut Vec<u8>, ops: &[Op]) {
    let count = u32::try_from(ops.len()).expect("op count fits u32");
    out.extend_from_slice(&count.to_le_bytes());
    for op in ops {
        encode_one_op(out, op);
    }
}

/// Exact byte length [`encode_op_set`] would produce for `ops`, computed
/// without allocating — used for cheap pre-commit size checks (deciding
/// whether a commit needs chaining, or exceeds the configured transaction
/// ceiling entirely) without building the buffer just to measure it.
#[must_use]
pub fn op_set_encoded_len(ops: &[Op]) -> usize {
    let mut len = 4; // count prefix
    for op in ops {
        len += 1; // tag
        len += match op {
            Op::Put(key, value) | Op::Set(key, value) => 4 + key.len() + 4 + value.len(),
            Op::Del(key) | Op::Purge(key) => 4 + key.len(),
        };
    }
    len
}

fn put_len_prefixed(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).expect("field length fits u32");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

fn take_u32(cur: &mut &[u8]) -> Result<u32, String> {
    if cur.len() < 4 {
        return Err("truncated op-set length".into());
    }
    let (head, rest) = cur.split_at(4);
    *cur = rest;
    Ok(u32::from_le_bytes(head.try_into().expect("4-byte slice")))
}

fn take_bytes(cur: &mut &[u8]) -> Result<Vec<u8>, String> {
    let len = take_u32(cur)? as usize;
    if cur.len() < len {
        return Err("truncated op-set field".into());
    }
    let (head, rest) = cur.split_at(len);
    *cur = rest;
    Ok(head.to_vec())
}

/// Encode one op's tag + length-prefixed fields — the per-op body
/// [`encode_op_set`]'s loop emits once per op.
fn encode_one_op(out: &mut Vec<u8>, op: &Op) {
    match op {
        Op::Put(key, value) => {
            out.push(0);
            put_len_prefixed(out, key);
            put_len_prefixed(out, value);
        }
        Op::Del(key) => {
            out.push(1);
            put_len_prefixed(out, key);
        }
        Op::Set(key, value) => {
            out.push(2);
            put_len_prefixed(out, key);
            put_len_prefixed(out, value);
        }
        Op::Purge(key) => {
            out.push(3);
            put_len_prefixed(out, key);
        }
    }
}

/// Decode one op's tag + length-prefixed fields, advancing `cur` past it —
/// the per-op body [`decode_op_set`]'s loop consumes once per op.
fn decode_one_op(cur: &mut &[u8]) -> Result<Op, String> {
    let tag = *cur.first().ok_or("truncated op tag")?;
    *cur = &cur[1..];
    let key = take_bytes(cur)?;
    Ok(match tag {
        0 => Op::Put(key, take_bytes(cur)?),
        1 => Op::Del(key),
        2 => Op::Set(key, take_bytes(cur)?),
        3 => Op::Purge(key),
        other => return Err(format!("unknown op tag {other}")),
    })
}

/// Decode an op-set encoded by [`encode_op_set`].
///
/// # Errors
/// Returns a human-readable description if the payload is truncated, carries
/// an unknown op tag, or has trailing bytes.
pub fn decode_op_set(raw: &[u8]) -> Result<Vec<Op>, String> {
    let mut cur = raw;
    let count = take_u32(&mut cur)? as usize;
    let mut ops = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        ops.push(decode_one_op(&mut cur)?);
    }
    if !cur.is_empty() {
        return Err("trailing bytes in op set".into());
    }
    Ok(ops)
}

/// Record-kind tag distinguishing a bare op-set from one paired with its
/// originating command. A one-byte prefix inside the framed
/// payload, so a WAL (or an export stream) can carry a mix of both kinds and
/// decode each unambiguously — low-level/raw op application (tests, the
/// full-replay shadow-map path) never had a `CommitBatch` to begin with and
/// keeps using `None`.
const OPS_RECORD_KIND_BARE: u8 = 0;
const OPS_RECORD_KIND_WITH_COMMAND: u8 = 1;
/// A non-final fragment of a chained commit (a large
/// transaction): this LSN's commit did not fit in one 64 MiB framed record, so
/// its *logical body* — the same `[op_set_len][op_set][command?]` layout
/// [`encode_ops_record`]'s `WITH_COMMAND` kind uses — is split across
/// several records sharing one LSN, at arbitrary raw byte boundaries (never
/// aligned to op or command boundaries, since nothing is decoded until every
/// fragment is reassembled). Reconstructed only once the matching
/// [`OPS_RECORD_KIND_CHAIN_FINAL`] is seen (see [`ChainAssembler`]).
const OPS_RECORD_KIND_CHAIN_CONTINUE: u8 = 2;
/// The final fragment of a chained commit: like `CHAIN_CONTINUE`, but marks
/// the chain complete — this is the point at which the concatenation of
/// every fragment's bytes becomes a decodable logical body.
const OPS_RECORD_KIND_CHAIN_FINAL: u8 = 3;

/// Encode an expanded op-set as a framed WAL record at `(shard_id, lsn)`,
/// optionally paired with the serialized `CommitBatch` that produced it
/// (carrying the command in the WAL record is what makes the
/// command journal — `commit_key` per LSN — fully derivable from the WAL,
/// closing the `__log__` redb row that used to be the only source of it).
/// `command_payload` is the batch pre-serialized by the caller (already
/// computed once for the outcome-payload row), not re-serialized here.
///
/// # Panics
/// Panics if the op-set encodes to more than `u32::MAX` bytes, which the
/// 64 MiB record ceiling makes impossible.
#[must_use]
pub fn encode_ops_record(
    shard_id: u32,
    lsn: u64,
    ops: &[Op],
    command_payload: Option<&[u8]>,
) -> Vec<u8> {
    // Encoded once, into one exactly-sized buffer: the header is written
    // with a placeholder CRC, the body is appended in place, and the length
    // and checksum are patched at the end (SD-7.3).
    let op_set_len = op_set_encoded_len(ops);
    let body_len = 1 + command_payload.map_or(op_set_len, |command| 4 + op_set_len + command.len());
    let payload_len = u32::try_from(body_len).expect("payload length fits u32");
    let mut out = Vec::with_capacity(WAL_HEADER_LEN + body_len);
    out.extend_from_slice(&WAL_MAGIC);
    out.extend_from_slice(&WAL_FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&shard_id.to_le_bytes());
    out.extend_from_slice(&lsn.to_le_bytes());
    out.extend_from_slice(&payload_len.to_le_bytes());
    out.extend_from_slice(&[0; 4]); // CRC, patched below
    if let Some(command) = command_payload {
        out.push(OPS_RECORD_KIND_WITH_COMMAND);
        out.extend_from_slice(
            &u32::try_from(op_set_len)
                .expect("op-set fits u32")
                .to_le_bytes(),
        );
        write_op_set(&mut out, ops);
        out.extend_from_slice(command);
    } else {
        out.push(OPS_RECORD_KIND_BARE);
        write_op_set(&mut out, ops);
    }
    debug_assert_eq!(out.len(), WAL_HEADER_LEN + body_len);
    let crc_at = WAL_HEADER_LEN - 4;
    let mut hasher = Crc32::new();
    hasher.update(&out[..crc_at]);
    hasher.update(&out[WAL_HEADER_LEN..]);
    out[crc_at..WAL_HEADER_LEN].copy_from_slice(&hasher.finalize().to_le_bytes());
    out
}

/// Decode a framed op-set record, verifying the frame checksum. The returned
/// header carries the routing shard and LSN; the `CommitBatch` is `Some` only
/// for a record encoded with one (every real client commit through the
/// normal publish path — see [`encode_ops_record`]).
///
/// # Errors
/// Returns a human-readable description if the frame is invalid, the record
/// kind is unrecognized, or the payload is not a valid op-set/commit batch.
///
/// # Panics
/// Panics only if an internal fixed-width slice conversion fails after its
/// length has been bounds-checked, which cannot happen.
pub fn decode_ops_record(raw: &[u8]) -> Result<(WalHeader, Vec<Op>, Option<CommitBatch>), String> {
    let (header, payload) = decode_wal_record(raw).map_err(|e| e.to_string())?;
    let (&kind, rest) = payload.split_first().ok_or("empty WAL record payload")?;
    match kind {
        OPS_RECORD_KIND_BARE => Ok((header, decode_op_set(rest)?, None)),
        OPS_RECORD_KIND_WITH_COMMAND => {
            if rest.len() < 4 {
                return Err("truncated combined WAL record".into());
            }
            let op_set_len =
                u32::from_le_bytes(rest[0..4].try_into().expect("4-byte slice")) as usize;
            if rest.len() < 4 + op_set_len {
                return Err("truncated combined WAL record op-set".into());
            }
            let ops = decode_op_set(&rest[4..4 + op_set_len])?;
            let batch: CommitBatch = rushdb_types::command_codec::decode(&rest[4 + op_set_len..])
                .map_err(|e| format!("invalid commit batch: {e}"))?;
            Ok((header, ops, Some(batch)))
        }
        other => Err(format!("unknown WAL record kind {other}")),
    }
}

/// Split `ops` (optionally paired with `command_payload`) into one or more
/// framed WAL records at `(shard_id, lsn)`, each a complete, independently
/// checksummed frame whose total length never exceeds `max_record_bytes`.
/// All returned records share one LSN; only the last is tagged
/// [`OPS_RECORD_KIND_CHAIN_FINAL`] (or, when everything already fits in one
/// record — overwhelmingly the common case — the single returned record is
/// byte-identical to [`encode_ops_record`]'s ordinary `BARE`/`WITH_COMMAND`
/// output). When chaining is needed, the logical body (op-set plus optional
/// command, laid out exactly like `WITH_COMMAND`'s own format) is cut at
/// arbitrary raw byte boundaries — never aligned to op or command
/// boundaries — which is what lets a command payload of any size be chained
/// too, not just the op-set: nothing is decoded fragment-by-fragment, only
/// once every fragment of the chain is reassembled (see [`ChainAssembler`]).
///
/// # Panics
/// Panics if `max_record_bytes` is too small to fit even the frame header
/// and kind byte — a caller-side configuration error, not a data-dependent
/// runtime condition.
#[must_use]
pub fn encode_chained_ops_record(
    shard_id: u32,
    lsn: u64,
    ops: &[Op],
    command_payload: Option<&[u8]>,
    max_record_bytes: usize,
) -> Vec<Vec<u8>> {
    let whole_len = WAL_HEADER_LEN
        + 1
        + command_payload.map_or(op_set_encoded_len(ops), |command| {
            4 + op_set_encoded_len(ops) + command.len()
        });
    if whole_len <= max_record_bytes {
        return vec![encode_ops_record(shard_id, lsn, ops, command_payload)];
    }

    // The logical body to chain: identical layout to `WITH_COMMAND`'s own
    // body (op_set_len-prefixed op-set, then command bytes — possibly
    // empty, when there is no command). Reusing this exact layout means
    // reassembly can reuse `WITH_COMMAND`'s own parsing, not a new format.
    let op_set = encode_op_set(ops);
    let op_set_len = u32::try_from(op_set.len()).expect("op-set fits u32");
    let mut logical_body =
        Vec::with_capacity(4 + op_set.len() + command_payload.map_or(0, <[u8]>::len));
    logical_body.extend_from_slice(&op_set_len.to_le_bytes());
    logical_body.extend_from_slice(&op_set);
    if let Some(command) = command_payload {
        logical_body.extend_from_slice(command);
    }

    let chunk_budget = max_record_bytes
        .checked_sub(WAL_HEADER_LEN + 1)
        .expect("max_record_bytes covers at least the frame header and kind byte");
    assert!(
        chunk_budget > 0,
        "max_record_bytes ({max_record_bytes}) leaves no room for any payload"
    );

    let chunks: Vec<&[u8]> = logical_body.chunks(chunk_budget).collect();
    let last_index = chunks.len().saturating_sub(1);
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let kind = if i == last_index {
                OPS_RECORD_KIND_CHAIN_FINAL
            } else {
                OPS_RECORD_KIND_CHAIN_CONTINUE
            };
            let mut body = Vec::with_capacity(1 + chunk.len());
            body.push(kind);
            body.extend_from_slice(chunk);
            encode_wal_record(shard_id, lsn, &body)
        })
        .collect()
}

/// Reconstructs a (possibly chained) commit from consecutive framed WAL
/// records, fed one at a time in physical log order. The overwhelmingly
/// common case — a `BARE`/`WITH_COMMAND` record — reconstructs immediately.
/// A chained commit's `CHAIN_CONTINUE` fragments are buffered until the
/// matching `CHAIN_FINAL` is fed, at which point the whole op-set is
/// returned — in full, exactly once, never partially. This is the one
/// reconstruction implementation crash recovery and replication apply both
/// use, so there is never a second, potentially drifting copy of this logic.
#[derive(Default)]
pub struct ChainAssembler {
    open: Option<OpenChain>,
}

struct OpenChain {
    lsn: u64,
    shard_id: u32,
    /// Byte offset (in whatever stream the caller is scanning) where this
    /// chain's first fragment began — what recovery must truncate back to,
    /// and what retention must never truncate through, if the chain never
    /// completes.
    start_offset: u64,
    /// Concatenated raw logical-body bytes from every `CHAIN_CONTINUE` fed
    /// so far, in order.
    fragments: Vec<u8>,
}

/// Result of feeding one record to a [`ChainAssembler`].
pub enum ChainFeed {
    /// This LSN's commit's *framing* is fully reconstructed: `header` plus
    /// the logical body (`[op_set_len:4][op_set][command bytes, possibly
    /// empty]` — exactly `WITH_COMMAND`'s own body shape), ready to decode
    /// via [`decode_logical_body`]. Decoding is a separate, explicit step
    /// — deliberately never done inside [`ChainAssembler::feed`] — so a
    /// *content* corruption (e.g. an unparsable embedded `CommitBatch`,
    /// `metadata_retirement_rejects_a_corrupt_wal_command_envelope`'s exact
    /// case) is never mistaken for a torn write and silently truncated by a
    /// caller that only wants structural chain-completeness (`read_segment`'s
    /// recovery pass checks `is_open()`/`open_start_offset()` and never
    /// calls `decode_logical_body` at all).
    Complete(WalHeader, Vec<u8>),
    /// More fragments are still expected for this LSN; nothing to decode yet.
    Pending,
}

/// Decode a [`ChainFeed::Complete`] logical body into its op-set and
/// optional command. Kept separate from [`ChainAssembler::feed`] (see
/// [`ChainFeed::Complete`]'s doc) so framing/chain-structure completeness
/// and content validity are never conflated.
///
/// # Errors
/// Returns a human-readable description if the body is truncated, the
/// op-set carries an unknown tag, or a non-empty command fails to parse as
/// a `CommitBatch`.
///
/// # Panics
/// Panics only if an internal fixed-width slice conversion fails after its
/// length has been bounds-checked, which cannot happen.
pub fn decode_logical_body(
    header: WalHeader,
    body: &[u8],
) -> Result<(WalHeader, Vec<Op>, Option<CommitBatch>), String> {
    if body.len() < 4 {
        return Err("truncated reassembled chain body".into());
    }
    let op_set_len = u32::from_le_bytes(body[0..4].try_into().expect("4-byte slice")) as usize;
    if body.len() < 4 + op_set_len {
        return Err("truncated reassembled chain op-set".into());
    }
    let ops = decode_op_set(&body[4..4 + op_set_len])?;
    let command_bytes = &body[4 + op_set_len..];
    let batch = if command_bytes.is_empty() {
        None
    } else {
        Some(
            rushdb_types::command_codec::decode(command_bytes)
                .map_err(|e| format!("invalid commit batch: {e}"))?,
        )
    };
    Ok((header, ops, batch))
}

impl ChainAssembler {
    /// Feed one decoded, checksum-verified framed record, at `offset` in
    /// whatever byte stream the caller is scanning (the segment file for
    /// recovery, the replication page stream for a follower). Purely
    /// structural: never decodes the op-set or command (see
    /// [`ChainFeed::Complete`]'s doc for why).
    ///
    /// # Errors
    /// Returns an error if a `CHAIN_CONTINUE`/`CHAIN_FINAL` arrives for a
    /// different LSN than the currently-open chain, if a `BARE`/
    /// `WITH_COMMAND` record arrives while a chain is open, or if the frame
    /// itself fails to decode (bad magic/version/checksum) — all of which
    /// indicate a corrupt or reordered log, never a legitimate write
    /// pattern.
    pub fn feed(&mut self, raw: &[u8], offset: u64) -> Result<ChainFeed, String> {
        let (header, payload) = decode_wal_record(raw).map_err(|e| e.to_string())?;
        let (&kind, rest) = payload.split_first().ok_or("empty WAL record payload")?;
        match kind {
            OPS_RECORD_KIND_BARE => {
                if self.open.is_some() {
                    return Err(format!(
                        "non-chained record at lsn {} arrived while a chain was open",
                        header.lsn
                    ));
                }
                // Normalize to the `WITH_COMMAND`-shaped logical body (an
                // explicit op_set_len prefix, empty trailing command) so
                // every `Complete` result shares one decodable layout
                // regardless of which kind produced it.
                let op_set_len =
                    u32::try_from(rest.len()).map_err(|_| "op-set too large".to_string())?;
                let mut body = Vec::with_capacity(4 + rest.len());
                body.extend_from_slice(&op_set_len.to_le_bytes());
                body.extend_from_slice(rest);
                Ok(ChainFeed::Complete(header, body))
            }
            OPS_RECORD_KIND_WITH_COMMAND => {
                if self.open.is_some() {
                    return Err(format!(
                        "non-chained record at lsn {} arrived while a chain was open",
                        header.lsn
                    ));
                }
                // `rest` is already exactly the target logical-body shape.
                Ok(ChainFeed::Complete(header, rest.to_vec()))
            }
            OPS_RECORD_KIND_CHAIN_CONTINUE => {
                match &mut self.open {
                    Some(open) if open.lsn == header.lsn && open.shard_id == header.shard_id => {
                        open.fragments.extend_from_slice(rest);
                    }
                    Some(open) => {
                        return Err(format!(
                            "chain continuation at (lsn {}, shard {}) arrived while (lsn {}, shard {}) was open",
                            header.lsn, header.shard_id, open.lsn, open.shard_id
                        ));
                    }
                    None => {
                        self.open = Some(OpenChain {
                            lsn: header.lsn,
                            shard_id: header.shard_id,
                            start_offset: offset,
                            fragments: rest.to_vec(),
                        });
                    }
                }
                Ok(ChainFeed::Pending)
            }
            OPS_RECORD_KIND_CHAIN_FINAL => {
                if let Some(open) = &self.open {
                    if open.lsn != header.lsn || open.shard_id != header.shard_id {
                        return Err(format!(
                            "chain final at (lsn {}, shard {}) arrived while (lsn {}, shard {}) was open",
                            header.lsn, header.shard_id, open.lsn, open.shard_id
                        ));
                    }
                }
                let mut logical_body = self.open.take().map_or_else(Vec::new, |o| o.fragments);
                logical_body.extend_from_slice(rest);
                Ok(ChainFeed::Complete(header, logical_body))
            }
            other => Err(format!("unknown WAL record kind {other}")),
        }
    }

    /// True while a chain is open (fragments buffered, final not yet seen).
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The LSN of the currently-open chain, if any.
    #[must_use]
    pub fn open_lsn(&self) -> Option<u64> {
        self.open.as_ref().map(|o| o.lsn)
    }

    /// The byte offset where the currently-open chain's first fragment
    /// began, if any — what a caller reaching end-of-stream with an open
    /// chain must truncate back to (discarding the whole incomplete chain,
    /// never partially applying it).
    #[must_use]
    pub fn open_start_offset(&self) -> Option<u64> {
        self.open.as_ref().map(|o| o.start_offset)
    }
}

/// A published durability checkpoint: the durable LSN (header), the GC floor,
/// and the wall-clock time the checkpoint was published. The record's frame
/// checksum makes a torn/partial checkpoint fail verification on load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    /// Physical shard this checkpoint belongs to.
    pub shard_id: u32,
    /// Durable LSN covered by this checkpoint.
    pub durable_lsn: u64,
    /// GC floor that must remain readable.
    pub gc_watermark: u64,
    /// Wall-clock time the checkpoint was published, in ms since epoch.
    pub published_at_ms: u64,
}

/// Encode a checkpoint as a framed WAL record (LSN carried in the header,
/// floor + timestamp in the payload).
#[must_use]
pub fn encode_checkpoint(checkpoint: &Checkpoint) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&checkpoint.gc_watermark.to_le_bytes());
    body.extend_from_slice(&checkpoint.published_at_ms.to_le_bytes());
    encode_wal_record(checkpoint.shard_id, checkpoint.durable_lsn, &body)
}

/// Decode a checkpoint from a framed WAL record.
///
/// # Errors
/// Returns a human-readable description if the frame is invalid or its
/// payload is not exactly the expected 16 bytes.
pub fn decode_checkpoint(raw: &[u8]) -> Result<Checkpoint, String> {
    let (header, body) = decode_wal_record(raw).map_err(|e| e.to_string())?;
    if body.len() != 16 {
        return Err(format!("invalid checkpoint body length: {}", body.len()));
    }
    let mut gc = [0u8; 8];
    let mut ts = [0u8; 8];
    gc.copy_from_slice(&body[0..8]);
    ts.copy_from_slice(&body[8..16]);
    Ok(Checkpoint {
        shard_id: header.shard_id,
        durable_lsn: header.lsn,
        gc_watermark: u64::from_le_bytes(gc),
        published_at_ms: u64::from_le_bytes(ts),
    })
}

/// Framed-stream writer for export/backup: emits `{len}{crc}{bytes}` chunks so
/// a consumer can detect truncation/corruption without reading the whole
/// artifact. (P17 foundations.)
///
/// # Errors
/// Returns an I/O error if the chunk length exceeds `u32::MAX` or writing to
/// the sink fails.
pub fn write_framed_chunk<W: io::Write>(writer: &mut W, bytes: &[u8]) -> io::Result<()> {
    let mut hasher = Crc32::new();
    hasher.update(bytes);
    let len = u32::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "framed chunk exceeds u32 length",
        )
    })?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&hasher.finalize().to_le_bytes())?;
    writer.write_all(bytes)
}

/// Read one framed chunk written by [`write_framed_chunk`].
///
/// # Errors
/// Returns an I/O error if reading fails, the chunk exceeds the 64 MiB
/// ceiling, or its checksum does not match. A clean end of stream yields
/// `Ok(None)`.
pub fn read_framed_chunk<R: io::Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 64 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "framed chunk exceeds 64 MiB",
        ));
    }
    let mut crc_buf = [0u8; 4];
    reader.read_exact(&mut crc_buf)?;
    let stored_crc = u32::from_le_bytes(crc_buf);
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    let mut hasher = Crc32::new();
    hasher.update(&payload);
    if hasher.finalize() != stored_crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "framed chunk checksum mismatch",
        ));
    }
    Ok(Some(payload))
}

/// Manifest describing one exported WAL segment range. Written first in a
/// segment stream so a consumer can validate the range before reading records.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WalSegmentManifest {
    /// Segment magic identifying the artifact type.
    pub magic: String,
    /// Framing format version.
    pub format_version: u32,
    /// Physical shard the segment was exported from.
    pub shard_id: u32,
    /// First LSN included in the segment.
    pub from_lsn: u64,
    /// Last LSN included in the segment.
    pub to_lsn: u64,
    /// Number of framed records that follow.
    pub record_count: u64,
    /// CRC32 over the concatenated framed records that follow.
    pub records_crc: u32,
}

/// Magic identifying a WAL segment manifest.
pub const SEGMENT_MAGIC: &str = "RDBWALSEG";

/// Framed snapshot export magic: distinct from the WAL segment magic so a
/// restore can reject a mismatched artifact type immediately.
pub const SNAPSHOT_MAGIC: &str = "RDBWALSNAP";

/// Manifest header for a framed snapshot stream (W6, D9/P17). Written as the
/// first framed chunk; consumers validate `magic`/`format_version` before
/// reading tenant chunks. `tenants_crc` covers the concatenated tenant chunks,
/// so a torn/corrupted stream is rejected at restore time.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotManifest {
    /// Snapshot magic identifying the artifact type.
    pub magic: String,
    /// Framing format version.
    pub format_version: u32,
    /// Physical shard the snapshot was exported from.
    pub shard_id: u32,
    /// First LSN included in the snapshot.
    pub from_lsn: u64,
    /// Last LSN included in the snapshot.
    pub to_lsn: u64,
    /// Number of tenant chunks that follow.
    pub tenant_count: u64,
    /// CRC32 over the concatenated tenant chunks that follow.
    pub tenants_crc: u32,
}

/// One tenant's framed backup chunk: scope (`TenantId` bytes) + JSONL payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantChunk {
    /// Raw tenant id bytes this chunk belongs to.
    pub tenant: Vec<u8>,
    /// JSONL-encoded rows for the tenant.
    pub jsonl: Vec<u8>,
}

/// Encode a tenant chunk: 4-byte tenant length, tenant bytes, JSONL bytes.
/// The whole chunk is written as one framed chunk (CRC-verified on read).
///
/// # Panics
/// Panics if the tenant id is larger than `u32::MAX` bytes, which cannot
/// happen for the fixed-width tenant id in use.
#[must_use]
pub fn encode_tenant_chunk(chunk: &TenantChunk) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + chunk.tenant.len() + chunk.jsonl.len());
    let tenant_len = u32::try_from(chunk.tenant.len()).expect("tenant id fits u32");
    body.extend_from_slice(&tenant_len.to_le_bytes());
    body.extend_from_slice(&chunk.tenant);
    body.extend_from_slice(&chunk.jsonl);
    body
}

/// Decode a tenant chunk body (the inverse of [`encode_tenant_chunk`]).
///
/// # Errors
/// Returns a human-readable description if the chunk is shorter than its
/// declared tenant length or the length overflows the body.
///
/// # Panics
/// Panics only if an internal fixed-width slice conversion fails after its
/// length has been bounds-checked, which cannot happen.
pub fn decode_tenant_chunk(raw: &[u8]) -> Result<TenantChunk, String> {
    if raw.len() < 4 {
        return Err(format!("tenant chunk too short: {} bytes", raw.len()));
    }
    let tenant_len = u32::from_le_bytes(
        raw[0..4]
            .try_into()
            .map_err(|_| "tenant chunk header is corrupt")?,
    ) as usize;
    if 4 + tenant_len > raw.len() {
        return Err("tenant chunk length overflow".into());
    }
    Ok(TenantChunk {
        tenant: raw[4..4 + tenant_len].to_vec(),
        jsonl: raw[4 + tenant_len..].to_vec(),
    })
}

/// Write one framed snapshot stream: manifest chunk, then one framed chunk per
/// tenant (each independently checksummed; the manifest carries a CRC over all
/// tenant chunks). Returns the manifest.
///
/// # Errors
/// Returns an I/O error if writing the manifest or any tenant chunk fails.
pub fn write_snapshot<W: io::Write>(
    writer: &mut W,
    shard_id: u32,
    from_lsn: u64,
    to_lsn: u64,
    tenants: &[(Vec<u8>, Vec<u8>)],
) -> io::Result<SnapshotManifest> {
    let mut hasher = Crc32::new();
    for (tenant, jsonl) in tenants {
        let chunk = encode_tenant_chunk(&TenantChunk {
            tenant: tenant.clone(),
            jsonl: jsonl.clone(),
        });
        hasher.update(&chunk);
    }
    let manifest = SnapshotManifest {
        magic: SNAPSHOT_MAGIC.to_string(),
        format_version: WAL_FORMAT_VERSION,
        shard_id,
        from_lsn,
        to_lsn,
        tenant_count: tenants.len() as u64,
        tenants_crc: hasher.finalize(),
    };
    write_framed_chunk(writer, &encode_manifest(&manifest))?;
    for (tenant, jsonl) in tenants {
        write_framed_chunk(
            writer,
            &encode_tenant_chunk(&TenantChunk {
                tenant: tenant.clone(),
                jsonl: jsonl.clone(),
            }),
        )?;
    }
    Ok(manifest)
}

/// Read + validate only the snapshot manifest (magic/version), so a caller can
/// stream the tenant chunks one at a time instead of materializing them all.
///
/// # Errors
/// Returns [`io::ErrorKind::UnexpectedEof`] when the stream is empty, or
/// [`io::ErrorKind::InvalidData`] when the manifest is malformed, has the
/// wrong magic, or uses an unsupported format version.
pub fn read_snapshot_manifest<R: io::Read>(reader: &mut R) -> io::Result<SnapshotManifest> {
    let Some(manifest_chunk) = read_framed_chunk(reader)? else {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "snapshot stream empty: missing manifest",
        ));
    };
    let manifest: SnapshotManifest = serde_json::from_slice(&manifest_chunk).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid snapshot manifest: {e}"),
        )
    })?;
    if manifest.magic != SNAPSHOT_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot magic mismatch: expected {SNAPSHOT_MAGIC}, got {}",
                manifest.magic
            ),
        ));
    }
    if manifest.format_version != WAL_FORMAT_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot format version {} unsupported (expected {WAL_FORMAT_VERSION})",
                manifest.format_version
            ),
        ));
    }
    Ok(manifest)
}

/// Read one framed snapshot stream, verifying the manifest (magic + version)
/// and every tenant chunk CRC. Returns the manifest and decoded tenant chunks.
/// Truncated or corrupted input is rejected: if the stream ends before
/// `tenant_count` chunks are read, or a chunk fails its CRC, this errors.
///
/// # Errors
/// Returns [`io::ErrorKind::UnexpectedEof`] for a missing manifest or a
/// truncated chunk stream, or [`io::ErrorKind::InvalidData`] when the
/// manifest is malformed or a chunk fails verification.
pub fn read_snapshot<R: io::Read>(
    reader: &mut R,
) -> io::Result<(SnapshotManifest, Vec<TenantChunk>)> {
    let manifest = read_snapshot_manifest(reader)?;
    let tenant_capacity = usize::try_from(manifest.tenant_count).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot tenant_count does not fit in usize",
        )
    })?;
    let mut tenants = Vec::new();
    tenants
        .try_reserve_exact(tenant_capacity)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("snapshot tenant_count allocation rejected: {error}"),
            )
        })?;
    for _ in 0..manifest.tenant_count {
        let Some(chunk) = read_framed_chunk(reader)? else {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "snapshot truncated: {} of {} tenant chunks present",
                    tenants.len(),
                    manifest.tenant_count
                ),
            ));
        };
        tenants.push(
            decode_tenant_chunk(&chunk)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
        );
    }
    // Verify the aggregate CRC over all tenant chunks.
    let mut hasher = Crc32::new();
    for chunk in &tenants {
        hasher.update(&encode_tenant_chunk(chunk));
    }
    if hasher.finalize() != manifest.tenants_crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot tenant CRC mismatch",
        ));
    }
    Ok((manifest, tenants))
}

/// Encode a manifest as JSON (used as the first framed chunk of a stream).
///
/// # Panics
/// Panics if the manifest cannot be serialized, which cannot happen for the
/// derived `Serialize` implementations used here.
pub fn encode_manifest<M: serde::Serialize>(manifest: &M) -> Vec<u8> {
    serde_json::to_vec(manifest).expect("serializable manifest")
}

/// Write one full WAL segment: manifest chunk, then one framed chunk per
/// record (each chunk independently checksummed; the manifest also carries a
/// CRC over all records). Returns the manifest.
///
/// # Errors
/// Returns an I/O error if writing the manifest or any record fails.
pub fn write_wal_segment<W: io::Write>(
    writer: &mut W,
    shard_id: u32,
    records: &[(u64, &[u8])],
) -> io::Result<WalSegmentManifest> {
    let from_lsn = records.first().map_or(0, |(lsn, _)| *lsn);
    let to_lsn = records.last().map_or(0, |(lsn, _)| *lsn);
    let mut hasher = Crc32::new();
    for (_, record) in records {
        hasher.update(record);
    }
    let manifest = WalSegmentManifest {
        magic: SEGMENT_MAGIC.to_string(),
        format_version: WAL_FORMAT_VERSION,
        shard_id,
        from_lsn,
        to_lsn,
        record_count: records.len() as u64,
        records_crc: hasher.finalize(),
    };
    write_framed_chunk(writer, &encode_manifest(&manifest))?;
    for (_, record) in records {
        write_framed_chunk(writer, record)?;
    }
    Ok(manifest)
}

/// Read one WAL segment written by [`write_wal_segment`], verifying the
/// manifest and every record checksum. Returns `(manifest, records)`.
///
/// # Errors
/// Returns an I/O error if reading fails, the manifest is malformed or has the
/// wrong magic/version, the segment ends early, or a record CRC mismatches. A
/// clean end of stream yields `Ok(None)`.
pub fn read_wal_segment<R: io::Read>(
    reader: &mut R,
) -> io::Result<Option<(WalSegmentManifest, Vec<Vec<u8>>)>> {
    let Some(manifest_raw) = read_framed_chunk(reader)? else {
        return Ok(None);
    };
    let manifest: WalSegmentManifest = serde_json::from_slice(&manifest_raw).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad segment manifest: {e}"),
        )
    })?;
    if manifest.magic != SEGMENT_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad segment magic",
        ));
    }
    if manifest.format_version != WAL_FORMAT_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported segment version",
        ));
    }
    let record_capacity = usize::try_from(manifest.record_count).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "segment record_count does not fit in usize",
        )
    })?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(record_capacity)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("segment record_count allocation rejected: {error}"),
            )
        })?;
    let mut hasher = Crc32::new();
    for _ in 0..manifest.record_count {
        let Some(record) = read_framed_chunk(reader)? else {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "segment ended before record_count",
            ));
        };
        hasher.update(&record);
        records.push(record);
    }
    if hasher.finalize() != manifest.records_crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "segment records CRC mismatch",
        ));
    }
    Ok(Some((manifest, records)))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_batch() -> CommitBatch {
        CommitBatch {
            command_version: rushdb_types::command::COMMAND_VERSION,
            tenant: rushdb_types::ids::TenantId::single(),
            transaction_id: "t".into(),
            commit_key: uuid::Uuid::now_v7(),
            start_index: 0,
            isolation: rushdb_types::command::IsolationMode::Snapshot,
            preconditions: vec![],
            mutations: vec![],
        }
    }

    /// Feeds every record to a fresh [`ChainAssembler`] in order and returns
    /// the one `Complete` result, panicking if the chain never completes or
    /// completes more than once.
    fn assemble_all(records: &[Vec<u8>]) -> (WalHeader, Vec<Op>, Option<CommitBatch>) {
        let mut assembler = ChainAssembler::default();
        let mut result = None;
        for (i, record) in records.iter().enumerate() {
            match assembler.feed(record, i as u64).unwrap() {
                ChainFeed::Pending => {}
                ChainFeed::Complete(header, body) => {
                    assert!(result.is_none(), "chain completed more than once");
                    let (_, ops, batch) = decode_logical_body(header, &body).unwrap();
                    result = Some((header, ops, batch));
                }
            }
        }
        result.expect("chain never completed")
    }

    fn big_value(tag: u8, len: usize) -> Vec<u8> {
        vec![tag; len]
    }

    #[test]
    fn wal_record_encodes_once_and_roundtrips() {
        let ops = vec![
            Op::Put(b"k1".to_vec(), b"v1".to_vec()),
            Op::Set(b"k2".to_vec(), vec![7; 300]),
            Op::Del(b"k3".to_vec()),
            Op::Purge(b"k4".to_vec()),
        ];
        let command = rushdb_types::command_codec::encode(&sample_batch());
        for payload in [None, Some(command.as_slice())] {
            let record = encode_ops_record(3, 9, &ops, payload);
            // One exactly-sized allocation: nothing was re-encoded or grown.
            assert_eq!(record.capacity(), record.len());
            // Byte-identical to framing the separately encoded body.
            let mut body = Vec::new();
            if let Some(command) = payload {
                body.push(OPS_RECORD_KIND_WITH_COMMAND);
                body.extend_from_slice(
                    &u32::try_from(encode_op_set(&ops).len())
                        .unwrap()
                        .to_le_bytes(),
                );
                body.extend_from_slice(&encode_op_set(&ops));
                body.extend_from_slice(command);
            } else {
                body.push(OPS_RECORD_KIND_BARE);
                body.extend_from_slice(&encode_op_set(&ops));
            }
            assert_eq!(record, encode_wal_record(3, 9, &body));
            let (header, decoded, batch) = decode_ops_record(&record).unwrap();
            assert_eq!((header.shard_id, header.lsn), (3, 9));
            assert_eq!(decoded, ops);
            assert_eq!(batch.is_some(), payload.is_some());
        }
        // A flipped byte is caught by the patched checksum.
        let mut record = encode_ops_record(3, 9, &ops, Some(&command));
        let last = record.len() - 1;
        record[last] ^= 1;
        assert!(decode_ops_record(&record).is_err());
    }

    #[test]
    fn chained_record_fitting_in_one_frame_is_byte_identical_to_unchained() {
        let ops = vec![
            Op::Put(b"k1".to_vec(), b"v1".to_vec()),
            Op::Del(b"k2".to_vec()),
        ];
        let batch = sample_batch();
        let command_payload = rushdb_types::command_codec::encode(&batch);
        let plain = encode_ops_record(1, 5, &ops, Some(&command_payload));
        let chained =
            encode_chained_ops_record(1, 5, &ops, Some(&command_payload), 64 * 1024 * 1024);
        assert_eq!(chained.len(), 1, "small op-set must not be split");
        assert_eq!(
            chained[0], plain,
            "must be byte-identical to the unchained encoding"
        );
    }

    #[test]
    fn chain_splits_and_reassembles_many_fragments() {
        // Each op is ~1 KiB; a 2 KiB-per-record ceiling forces many fragments.
        let ops: Vec<Op> = (0u8..50)
            .map(|i| Op::Put(format!("key-{i}").into_bytes(), big_value(i, 1024)))
            .collect();
        let batch = sample_batch();
        let command_payload = rushdb_types::command_codec::encode(&batch);
        let max_record_bytes = 2048;
        let records =
            encode_chained_ops_record(7, 42, &ops, Some(&command_payload), max_record_bytes);
        assert!(
            records.len() > 10,
            "expected many fragments, got {}",
            records.len()
        );
        for record in &records {
            assert!(record.len() <= max_record_bytes, "fragment exceeds ceiling");
        }

        let (header, decoded_ops, decoded_batch) = assemble_all(&records);
        assert_eq!(header.shard_id, 7);
        assert_eq!(header.lsn, 42);
        assert_eq!(decoded_ops, ops);
        assert_eq!(decoded_batch.unwrap().commit_key, batch.commit_key);
    }

    #[test]
    fn chain_without_command_round_trips() {
        let ops: Vec<Op> = (0..20)
            .map(|i| Op::Set(format!("s-{i}").into_bytes(), big_value(1, 512)))
            .collect();
        let records = encode_chained_ops_record(0, 3, &ops, None, 1024);
        assert!(records.len() > 1);
        let (_, decoded_ops, decoded_batch) = assemble_all(&records);
        assert_eq!(decoded_ops, ops);
        assert!(decoded_batch.is_none());
    }

    #[test]
    fn chain_separates_ops_from_an_invalid_command_payload_only_at_completion() {
        // Raw byte chunking means a chunk boundary can fall anywhere,
        // including inside the command's own bytes — reconstruction must
        // still correctly separate the reassembled op-set from the
        // reassembled command bytes regardless of where the physical cuts
        // landed. `feed` itself is purely structural and must complete the
        // chain successfully even though the command is invalid JSON — only
        // the explicit `decode_logical_body` step, which a caller invokes
        // separately, should fail on it (this is the whole point of
        // splitting the two: a structural chain-completeness check, like
        // `log_store::read_segment`'s crash-recovery pass, must never
        // mistake this kind of content corruption for a torn write).
        let ops: Vec<Op> = (0..8)
            .map(|i| Op::Put(format!("p{i}").into_bytes(), big_value(2, 900)))
            .collect();
        let invalid_command = vec![b'{'; 100]; // deliberately not valid JSON
        let max_record_bytes = 1024;
        let records =
            encode_chained_ops_record(0, 9, &ops, Some(&invalid_command), max_record_bytes);
        assert!(records.len() > 1, "expected this to need chaining");
        for record in &records {
            assert!(record.len() <= max_record_bytes);
        }
        let mut assembler = ChainAssembler::default();
        let mut completed = None;
        for (i, record) in records.iter().enumerate() {
            match assembler.feed(record, i as u64).unwrap() {
                ChainFeed::Pending => {}
                ChainFeed::Complete(header, body) => {
                    assert!(completed.is_none(), "chain completed more than once");
                    completed = Some((header, body));
                }
            }
        }
        let (header, body) = completed.expect("feed must structurally complete the chain");
        // Only now, decoding explicitly, does the invalid command surface.
        assert!(
            decode_logical_body(header, &body).is_err(),
            "invalid JSON command must fail to decode"
        );
    }

    #[test]
    fn chain_assembler_rejects_interleaved_non_chained_record() {
        let ops: Vec<Op> = (0..10)
            .map(|i| Op::Put(format!("k{i}").into_bytes(), big_value(3, 300)))
            .collect();
        let records = encode_chained_ops_record(0, 1, &ops, None, 512);
        assert!(records.len() > 1);
        let mut assembler = ChainAssembler::default();
        assembler.feed(&records[0], 0).unwrap();
        assert!(assembler.is_open());
        let interloper = encode_ops_record(0, 99, &[Op::Del(b"x".to_vec())], None);
        assert!(assembler.feed(&interloper, 1).is_err());
    }

    #[test]
    fn chain_assembler_rejects_lsn_mismatch_mid_chain() {
        let ops: Vec<Op> = (0..10)
            .map(|i| Op::Put(format!("k{i}").into_bytes(), big_value(4, 300)))
            .collect();
        let records = encode_chained_ops_record(0, 1, &ops, None, 512);
        assert!(records.len() > 2);
        let mut assembler = ChainAssembler::default();
        assembler.feed(&records[0], 0).unwrap();
        // Re-frame the second record's body under a different LSN (with a
        // freshly computed checksum, so this is a decode-valid-but-wrong-chain
        // record — a reordered/corrupt log — not a checksum failure, which is
        // what's actually under test here).
        let (_, body) = decode_wal_record(&records[1]).unwrap();
        let retagged = encode_wal_record(0, 999, body);
        assert!(assembler.feed(&retagged, 1).is_err());
    }

    #[test]
    fn chain_assembler_open_tracks_lsn_and_start_offset() {
        let ops: Vec<Op> = (0..10)
            .map(|i| Op::Put(format!("k{i}").into_bytes(), big_value(5, 300)))
            .collect();
        let records = encode_chained_ops_record(0, 77, &ops, None, 512);
        assert!(records.len() > 1);
        let mut assembler = ChainAssembler::default();
        assert!(!assembler.is_open());
        assembler.feed(&records[0], 1234).unwrap();
        assert!(assembler.is_open());
        assert_eq!(assembler.open_lsn(), Some(77));
        assert_eq!(assembler.open_start_offset(), Some(1234));
        for (i, record) in records.iter().enumerate().skip(1) {
            assembler.feed(record, 1234 + i as u64).unwrap();
        }
        assert!(!assembler.is_open(), "chain must close on CHAIN_FINAL");
    }

    #[test]
    fn rejects_corrupted_payload() {
        let framed = encode_wal_record(0, 42, b"arbitrary payload bytes");
        let mut corrupted = framed;
        let last = corrupted.last_mut().unwrap();
        *last ^= 0xFF;
        let err = decode_wal_record(&corrupted).unwrap_err();
        assert!(
            matches!(err, WalDecodeError::ChecksumMismatch),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_truncated_record() {
        let framed = encode_wal_record(0, 42, b"arbitrary payload bytes");
        let err = decode_wal_record(&framed[..framed.len() - 3]).unwrap_err();
        assert!(
            matches!(err, WalDecodeError::Truncated { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_bad_magic_and_version() {
        let framed = encode_wal_record(0, 42, b"arbitrary payload bytes");
        let mut bad_magic = framed.clone();
        bad_magic[0] = b'X';
        assert!(matches!(
            decode_wal_record(&bad_magic).unwrap_err(),
            WalDecodeError::BadMagic
        ));
        let mut bad_version = framed;
        bad_version[8..12].copy_from_slice(&99u32.to_le_bytes());
        assert!(matches!(
            decode_wal_record(&bad_version).unwrap_err(),
            WalDecodeError::UnsupportedVersion(99)
        ));
    }

    #[test]
    fn last_applied_round_trip() {
        let raw = encode_last_applied(0, 1234);
        assert_eq!(decode_last_applied(&raw).unwrap(), 1234);
    }

    #[test]
    fn op_set_round_trips_all_variants() {
        let ops = vec![
            Op::Put(b"k1".to_vec(), b"v1".to_vec()),
            Op::Del(b"k2".to_vec()),
            Op::Set(b"k3".to_vec(), b"v3".to_vec()),
            Op::Purge(b"k4".to_vec()),
            Op::Put(vec![], vec![]),
        ];
        let raw = encode_ops_record(3, 7, &ops, None);
        let (header, decoded, batch) = decode_ops_record(&raw).unwrap();
        assert_eq!(header.shard_id, 3);
        assert_eq!(header.lsn, 7);
        assert_eq!(decoded, ops);
        assert!(batch.is_none(), "a bare op-set record carries no command");
    }

    /// A record encoded with its originating command round-trips
    /// both the op-set (for materialization replay, unchanged) and the
    /// `CommitBatch` (for command-journal purposes — this is what makes
    /// `commit_key` per LSN fully derivable from the WAL, closing the
    /// `__log__` redb row).
    #[test]
    fn op_set_with_command_round_trips_both() {
        let ops = vec![Op::Put(b"key".to_vec(), b"value".to_vec())];
        let batch = sample_batch();
        let command_payload = rushdb_types::command_codec::encode(&batch);
        let raw = encode_ops_record(2, 11, &ops, Some(&command_payload));
        let (header, decoded_ops, decoded_batch) = decode_ops_record(&raw).unwrap();
        assert_eq!(header.shard_id, 2);
        assert_eq!(header.lsn, 11);
        assert_eq!(decoded_ops, ops);
        assert_eq!(decoded_batch.unwrap().commit_key, batch.commit_key);
    }

    #[test]
    fn op_set_rejects_corruption_and_trailing_bytes() {
        let ops = vec![Op::Put(b"key".to_vec(), b"value".to_vec())];
        let mut raw = encode_ops_record(0, 1, &ops, None);
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        assert!(decode_ops_record(&raw).is_err());

        let mut trailing = encode_op_set(&ops);
        trailing.push(0);
        assert!(decode_op_set(&trailing).unwrap_err().contains("trailing"));

        assert!(decode_op_set(&[1, 0, 0, 0])
            .unwrap_err()
            .contains("truncated"));
    }

    #[test]
    fn framed_chunk_stream_round_trip_and_corruption() {
        let mut out = Vec::new();
        for chunk in &["alpha", "beta", "gamma"] {
            write_framed_chunk(&mut out, chunk.as_bytes()).unwrap();
        }
        let mut reader = std::io::Cursor::new(&out);
        let mut got = Vec::new();
        while let Some(chunk) = read_framed_chunk(&mut reader).unwrap() {
            got.push(String::from_utf8(chunk).unwrap());
        }
        assert_eq!(got, vec!["alpha", "beta", "gamma"]);

        let mut corrupted = out.clone();
        let mid = corrupted.len() / 2;
        corrupted[mid] ^= 0x40;
        let mut reader = std::io::Cursor::new(&corrupted);
        let res = (|| loop {
            if read_framed_chunk(&mut reader)?.is_none() {
                return Ok::<_, std::io::Error>(());
            }
        })();
        assert!(res.is_err(), "corruption must be detected");
    }

    #[test]
    fn wal_segment_round_trip_and_manifest() {
        let batch = sample_batch();
        let command_payload = rushdb_types::command_codec::encode(&batch);
        let records: Vec<(u64, Vec<u8>)> = vec![
            (10, encode_ops_record(0, 10, &[], Some(&command_payload))),
            (11, encode_ops_record(0, 11, &[], Some(&command_payload))),
            (12, encode_ops_record(0, 12, &[], Some(&command_payload))),
        ];
        let refs: Vec<(u64, &[u8])> = records
            .iter()
            .map(|(lsn, b)| (*lsn, b.as_slice()))
            .collect();
        let mut out = Vec::new();
        let manifest = write_wal_segment(&mut out, 0, &refs).unwrap();
        assert_eq!(manifest.from_lsn, 10);
        assert_eq!(manifest.to_lsn, 12);
        assert_eq!(manifest.record_count, 3);

        let mut reader = std::io::Cursor::new(&out);
        let (got, decoded) = read_wal_segment(&mut reader).unwrap().unwrap();
        assert_eq!(got, manifest);
        assert_eq!(decoded.len(), 3);
        let (_, _, decoded_batch) = decode_ops_record(&decoded[0]).unwrap();
        assert_eq!(decoded_batch.unwrap().commit_key, batch.commit_key);
    }

    #[test]
    fn wal_segment_detects_record_corruption() {
        let records: Vec<(u64, Vec<u8>)> = vec![
            (10, encode_ops_record(0, 10, &[], None)),
            (11, encode_ops_record(0, 11, &[], None)),
        ];
        let refs: Vec<(u64, &[u8])> = records
            .iter()
            .map(|(lsn, b)| (*lsn, b.as_slice()))
            .collect();
        let mut out = Vec::new();
        write_wal_segment(&mut out, 0, &refs).unwrap();
        let mid = out.len() / 2;
        out[mid] ^= 0x20;
        let mut reader = std::io::Cursor::new(&out);
        assert!(read_wal_segment(&mut reader).is_err());
    }

    #[test]
    fn checkpoint_round_trips_with_frame_checksum() {
        let checkpoint = Checkpoint {
            shard_id: 2,
            durable_lsn: 99,
            gc_watermark: 88,
            published_at_ms: 1_700_000_000_000,
        };
        let encoded = encode_checkpoint(&checkpoint);
        let decoded = decode_checkpoint(&encoded).unwrap();
        assert_eq!(decoded, checkpoint);

        // Torn frame: flipping a payload byte breaks the CRC and must reject.
        let mut torn = encoded;
        let last = torn.len() - 1;
        torn[last] ^= 0xFF;
        assert!(decode_checkpoint(&torn).is_err());

        // LSN lives in the header; a wrong body length is invalid too.
        let short = encode_wal_record(2, 99, &[0u8; 4]);
        assert!(decode_checkpoint(&short).is_err());
    }

    #[test]
    fn snapshot_round_trips_and_validates() {
        let tenants = vec![
            (
                b"tenant-a".to_vec(),
                b"record line 1\nrecord line 2\n".to_vec(),
            ),
            (b"tenant-b".to_vec(), b"record line 3\n".to_vec()),
        ];
        let mut out = Vec::new();
        let manifest = write_snapshot(&mut out, 3, 10, 20, &tenants).unwrap();
        assert_eq!(manifest.magic, SNAPSHOT_MAGIC);
        assert_eq!(manifest.tenant_count, 2);
        assert_eq!(manifest.from_lsn, 10);
        assert_eq!(manifest.to_lsn, 20);

        let mut reader = std::io::Cursor::new(&out);
        let (got_manifest, chunks) = read_snapshot(&mut reader).unwrap();
        assert_eq!(got_manifest, manifest);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].tenant, b"tenant-a");
        assert_eq!(chunks[0].jsonl, b"record line 1\nrecord line 2\n");
        assert_eq!(chunks[1].tenant, b"tenant-b");
    }

    #[test]
    fn snapshot_rejects_corruption_and_truncation() {
        let tenants = vec![(b"tenant-a".to_vec(), b"payload to be protected".to_vec())];
        let mut out = Vec::new();
        write_snapshot(&mut out, 0, 1, 5, &tenants).unwrap();

        // Corrupt a byte in the middle of the tenant JSONL chunk.
        let mut corrupt = out.clone();
        let mid = corrupt.len() / 2;
        corrupt[mid] ^= 0xFF;
        let mut reader = std::io::Cursor::new(&corrupt);
        assert!(
            read_snapshot(&mut reader).is_err(),
            "corrupted snapshot must be rejected"
        );

        // Truncate the stream mid-way (fewer tenant chunks than the manifest).
        let truncated = &out[..out.len() - 6];
        let mut reader = std::io::Cursor::new(truncated);
        assert!(
            read_snapshot(&mut reader).is_err(),
            "truncated snapshot must be rejected"
        );

        // Wrong magic (a WAL segment) must be rejected.
        let mut wal_segment = Vec::new();
        let refs: Vec<(u64, &[u8])> = vec![(1, &b"x"[..])];
        write_wal_segment(&mut wal_segment, 0, &refs).unwrap();
        let mut reader = std::io::Cursor::new(&wal_segment);
        assert!(read_snapshot(&mut reader).is_err());
    }

    #[test]
    fn oversized_manifest_counts_fail_before_allocation() {
        let snapshot_manifest = SnapshotManifest {
            magic: SNAPSHOT_MAGIC.into(),
            format_version: WAL_FORMAT_VERSION,
            shard_id: 0,
            from_lsn: 0,
            to_lsn: 0,
            tenant_count: u64::MAX,
            tenants_crc: 0,
        };
        let mut snapshot = Vec::new();
        write_framed_chunk(&mut snapshot, &encode_manifest(&snapshot_manifest)).unwrap();
        assert!(read_snapshot(&mut std::io::Cursor::new(snapshot)).is_err());

        let segment_manifest = WalSegmentManifest {
            magic: SEGMENT_MAGIC.into(),
            format_version: WAL_FORMAT_VERSION,
            shard_id: 0,
            from_lsn: 0,
            to_lsn: 0,
            record_count: u64::MAX,
            records_crc: 0,
        };
        let mut segment = Vec::new();
        write_framed_chunk(&mut segment, &encode_manifest(&segment_manifest)).unwrap();
        assert!(read_wal_segment(&mut std::io::Cursor::new(segment)).is_err());
    }
}
