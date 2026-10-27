//! Segmented, append-only write-ahead log (log-first).
//!
//! The log-first substrate makes this log the **durable truth**: a commit is
//! acknowledged only after its framed record is appended and fsynced, and the
//! redb materialization is recovered by replaying log records beyond its
//! recorded checkpoint LSN. See the manual's
//! `docs/manual/architecture/storage-engine.md`.
//!
//! Layout: one directory of segment files named by their first LSN
//! (`00000000000000000001.seg`), each with a fixed header followed by
//! self-describing records produced by [`crate::wal::encode_wal_record`]. A
//! record's own 32-byte header carries its payload length and CRC, so a torn
//! trailing write (a crash mid-append) is detectable and truncatable, while a
//! torn or corrupt record in a sealed segment is a hard error.
//!
//! This module is additive: it is not yet wired into the publisher. Phase 1 of
//! the plan wires it as the durable acknowledgement and demotes redb to a
//! checkpointed materialization.

use crate::wal::{decode_wal_record, WAL_FORMAT_VERSION};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Magic identifying a live log segment file.
const LOG_SEGMENT_MAGIC: [u8; 8] = *b"RDBWSEG1";

/// Segment header: magic(8) + format version(4) + shard id(4) + first LSN(8).
const LOG_SEGMENT_HEADER_LEN: usize = 8 + 4 + 4 + 8;

/// A framed record's fixed header, as emitted by
/// [`crate::wal::encode_wal_record`]: magic(8) + version(4) + shard(4) +
/// lsn(8) + payload length(4) + crc(4).
const LOG_RECORD_HEADER_LEN: usize = 32;

/// Record ceiling, matching [`crate::wal`]. `pub(crate)` so `Store::append_log`
/// can size a chained commit's fragments against the same hard ceiling this
/// module enforces, rather than duplicating the literal.
pub(crate) const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;

/// Errors returned by the segmented log.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    /// An underlying file operation failed.
    #[error("log I/O error: {0}")]
    Io(#[from] io::Error),
    /// A segment failed structural validation.
    #[error("corrupt log segment {path}: {detail}")]
    Corrupt {
        /// Path of the offending segment.
        path: String,
        /// Why the segment is rejected.
        detail: String,
    },
    /// A record was appended with an LSN that breaks the contiguous sequence.
    #[error("log append out of order: expected LSN {expected}, got {actual}")]
    OutOfOrder {
        /// The LSN the log required next.
        expected: u64,
        /// The LSN the caller supplied.
        actual: u64,
    },
    /// A record exceeds the 64 MiB ceiling.
    #[error("log record {0} bytes exceeds the 64 MiB ceiling")]
    Oversized(usize),
}

/// One sealed or active log segment's in-memory descriptor.
#[derive(Debug, Clone)]
pub struct SegmentInfo {
    /// First LSN stored in the segment.
    pub first_lsn: u64,
    /// Last LSN stored in the segment (`first_lsn - 1` when empty).
    pub last_lsn: u64,
    /// Segment file path.
    pub path: PathBuf,
    /// Valid bytes in the segment (after any torn tail is truncated).
    pub bytes: u64,
}

/// The currently-open segment receiving appends.
#[derive(Debug)]
struct ActiveSegment {
    file: File,
    first_lsn: u64,
    bytes: u64,
}

/// A directory of append-only log segments with a contiguous LSN sequence.
#[derive(Debug)]
pub struct LogStore {
    dir: PathBuf,
    shard_id: u32,
    segment_bytes: u64,
    segments: Vec<SegmentInfo>,
    next_lsn: u64,
    active: Option<ActiveSegment>,
}

impl LogStore {
    /// Open (creating if absent) a log directory and recover its state.
    ///
    /// Existing segments are validated in order. A torn trailing record in the
    /// **last** segment is truncated away (an interrupted append); the same
    /// damage in a sealed segment is [`LogError::Corrupt`]. The next expected
    /// LSN is one past the highest recovered record (1 for an empty log).
    ///
    /// # Errors
    /// Returns [`LogError`] if the directory cannot be created or read, or a
    /// segment is corrupt.
    pub fn open(dir: &Path, shard_id: u32, segment_bytes: u64) -> Result<Self, LogError> {
        std::fs::create_dir_all(dir)?;
        let mut paths: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let first_lsn = name.strip_suffix(".seg")?.parse::<u64>().ok()?;
                Some((first_lsn, entry.path()))
            })
            .collect();
        paths.sort_by_key(|(first_lsn, _)| *first_lsn);

        let mut segments = Vec::with_capacity(paths.len());
        let mut next_lsn = 1u64;
        let last_index = paths.len().saturating_sub(1);
        for (index, (first_lsn, path)) in paths.iter().enumerate() {
            let is_active = index == last_index;
            let (records, valid_bytes, file_len) = read_segment(path, shard_id, is_active)?;
            if valid_bytes != file_len {
                if !is_active {
                    return Err(LogError::Corrupt {
                        path: path.display().to_string(),
                        detail: "sealed segment ends with a torn record".into(),
                    });
                }
                // Interrupted append in the active segment: drop the tail so
                // later appends resume on a clean record boundary.
                let file = OpenOptions::new().write(true).open(path)?;
                file.set_len(valid_bytes)?;
                file.sync_all()?;
            }
            let last_lsn = records
                .last()
                .map_or(first_lsn.saturating_sub(1), |(lsn, _)| *lsn);
            next_lsn = last_lsn.saturating_add(1);
            segments.push(SegmentInfo {
                first_lsn: *first_lsn,
                last_lsn,
                path: path.clone(),
                bytes: valid_bytes,
            });
        }

        // Reuse the last segment for appends when it still has room; otherwise
        // the next append starts a fresh segment.
        let active = match segments.last() {
            Some(last) if last.bytes < segment_bytes => {
                let file = OpenOptions::new().append(true).open(&last.path)?;
                Some(ActiveSegment {
                    file,
                    first_lsn: last.first_lsn,
                    bytes: last.bytes,
                })
            }
            _ => None,
        };

        Ok(Self {
            dir: dir.to_path_buf(),
            shard_id,
            segment_bytes,
            segments,
            next_lsn,
            active,
        })
    }

    /// Seed `next_lsn` from `floor` when this log has no existing segments
    /// (a freshly created, never-before-written directory) and `floor`
    /// exceeds the default start of 1. For opening the WAL beside a
    /// materialization whose checkpoint is already ahead of an empty WAL —
    /// a store opened from a restored base backup with no accompanying WAL
    /// directory, for example — so the next commit continues the sequence
    /// instead of colliding with LSNs the materialization already has. A
    /// no-op once any segment exists: a real recovered `next_lsn` is
    /// authoritative and must never be overridden.
    pub(crate) fn seed_next_lsn_if_empty(&mut self, floor: u64) {
        if self.segments.is_empty() && self.active.is_none() && floor > self.next_lsn {
            self.next_lsn = floor;
        }
    }

    /// The physical shard this log belongs to.
    #[must_use]
    pub fn shard_id(&self) -> u32 {
        self.shard_id
    }

    /// The LSN the next appended record must carry.
    #[must_use]
    pub fn next_lsn(&self) -> u64 {
        self.next_lsn
    }

    /// The highest LSN appended so far (`0` when empty). After [`Self::sync`],
    /// this is the durable LSN.
    #[must_use]
    pub fn durable_lsn(&self) -> u64 {
        self.segments.last().map_or(0, |segment| segment.last_lsn)
    }

    /// The segment descriptors in LSN order.
    #[must_use]
    pub fn segments(&self) -> &[SegmentInfo] {
        &self.segments
    }

    /// Append framed records (each produced by [`crate::wal::encode_wal_record`]
    /// or [`crate::wal::encode_chained_ops_record`]) with contiguous, ascending
    /// LSNs. **Chaining (large-transaction support):** several
    /// consecutive entries may share one LSN — a chained commit's fragments,
    /// which must all be present in this same call (never split across two
    /// `append_batch` calls: see the caller's own invariant in
    /// `Store::append_log`) — and are grouped and written as one atomic unit:
    /// `next_lsn` advances only once per group, and the whole group is kept
    /// in one segment file (rotating *before* its first fragment if it would
    /// not otherwise fit, rather than splitting it across two segments).
    ///
    /// Records are written to the active segment, rotating to a new segment
    /// once the configured size is reached. The write is buffered by the OS;
    /// call [`Self::sync`] to make it durable before acknowledging.
    ///
    /// # Errors
    /// Returns [`LogError::OutOfOrder`] if an LSN is not the expected next LSN,
    /// [`LogError::Oversized`] if a record exceeds the ceiling, or
    /// [`LogError::Io`] on write failure.
    pub fn append_batch(&mut self, records: &[(u64, Vec<u8>)]) -> Result<(), LogError> {
        let mut i = 0;
        while i < records.len() {
            let lsn = records[i].0;
            if lsn != self.next_lsn {
                return Err(LogError::OutOfOrder {
                    expected: self.next_lsn,
                    actual: lsn,
                });
            }
            let mut group_end = i + 1;
            while group_end < records.len() && records[group_end].0 == lsn {
                group_end += 1;
            }
            let group = &records[i..group_end];
            for (_, framed) in group {
                if framed.len() > MAX_RECORD_BYTES {
                    return Err(LogError::Oversized(framed.len()));
                }
            }
            let group_bytes: u64 = group.iter().map(|(_, framed)| framed.len() as u64).sum();
            let would_overflow = self.active.as_ref().is_some_and(|active| {
                active.bytes > 0 && active.bytes + group_bytes > self.segment_bytes
            });
            if would_overflow {
                self.seal_active()?;
            }
            if self.active.is_none() {
                self.create_active(lsn)?;
            }
            for (_, framed) in group {
                let active = self.active.as_mut().ok_or_else(|| LogError::Corrupt {
                    path: self.dir.display().to_string(),
                    detail: "active segment missing after creation".into(),
                })?;
                active.file.write_all(framed)?;
                active.bytes += framed.len() as u64;
                if let Some(last) = self
                    .segments
                    .last_mut()
                    .filter(|last| last.first_lsn == active.first_lsn)
                {
                    last.last_lsn = lsn;
                    last.bytes = active.bytes;
                }
            }
            self.next_lsn = lsn.saturating_add(1);
            i = group_end;
        }
        Ok(())
    }

    /// Make all appended records durable (fsync the active segment and the
    /// directory so a freshly created segment name survives a crash).
    ///
    /// # Errors
    /// Returns [`LogError::Io`] if the fsync fails.
    pub fn sync(&mut self) -> Result<(), LogError> {
        if let Some(active) = self.active.as_mut() {
            active.file.sync_data()?;
        }
        sync_dir(&self.dir)?;
        Ok(())
    }

    /// Read every record with LSN `>= from_lsn`, in order, as framed bytes.
    ///
    /// # Errors
    /// Returns [`LogError`] if a segment cannot be read or is corrupt.
    pub fn read_from(&self, from_lsn: u64) -> Result<Vec<(u64, Vec<u8>)>, LogError> {
        let last_index = self.segments.len().saturating_sub(1);
        let mut out = Vec::new();
        for (index, segment) in self.segments.iter().enumerate() {
            if segment.last_lsn < from_lsn {
                continue;
            }
            let (records, _, _) = read_segment(&segment.path, self.shard_id, index == last_index)?;
            for (lsn, framed) in records {
                if lsn >= from_lsn {
                    out.push((lsn, framed));
                }
            }
        }
        Ok(out)
    }

    /// Descriptors of the segments that may hold LSNs in `[from_lsn,
    /// until_lsn)`, each flagged when it is the active (last) segment. Callers
    /// read them with [`read_segment_records`] **after** releasing the log
    /// lock, so a retention sweep never blocks WAL appends while it reads.
    #[must_use]
    pub fn segments_in(&self, from_lsn: u64, until_lsn: u64) -> Vec<(SegmentInfo, bool)> {
        let last_index = self.segments.len().saturating_sub(1);
        self.segments
            .iter()
            .enumerate()
            .filter(|(_, segment)| segment.last_lsn >= from_lsn && segment.first_lsn < until_lsn)
            .map(|(index, segment)| (segment.clone(), index == last_index))
            .collect()
    }

    /// Delete sealed segments that end before `lsn`. The active segment is
    /// never removed, so the log always retains its live tail. Returns the
    /// number of segments deleted.
    ///
    /// # Errors
    /// Returns [`LogError::Io`] if a segment file cannot be removed.
    pub fn truncate_before(&mut self, lsn: u64) -> Result<usize, LogError> {
        let active_first = self.active.as_ref().map(|active| active.first_lsn);
        let mut removed = 0;
        let mut keep = Vec::with_capacity(self.segments.len());
        for segment in &self.segments {
            let is_active = active_first == Some(segment.first_lsn);
            if !is_active && segment.last_lsn < lsn {
                std::fs::remove_file(&segment.path)?;
                removed += 1;
            } else {
                keep.push(segment.clone());
            }
        }
        self.segments = keep;
        Ok(removed)
    }

    /// Seal the active segment: fsync it, then stop appending to it.
    fn seal_active(&mut self) -> Result<(), LogError> {
        if let Some(active) = self.active.take() {
            active.file.sync_all()?;
        }
        sync_dir(&self.dir)?;
        Ok(())
    }

    /// Create and open a new segment whose first record is at `first_lsn`.
    fn create_active(&mut self, first_lsn: u64) -> Result<(), LogError> {
        let path = self.dir.join(segment_file_name(first_lsn));
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        let mut header = Vec::with_capacity(LOG_SEGMENT_HEADER_LEN);
        header.extend_from_slice(&LOG_SEGMENT_MAGIC);
        header.extend_from_slice(&WAL_FORMAT_VERSION.to_le_bytes());
        header.extend_from_slice(&self.shard_id.to_le_bytes());
        header.extend_from_slice(&first_lsn.to_le_bytes());
        file.write_all(&header)?;
        self.segments.push(SegmentInfo {
            first_lsn,
            last_lsn: first_lsn.saturating_sub(1),
            path,
            bytes: LOG_SEGMENT_HEADER_LEN as u64,
        });
        self.active = Some(ActiveSegment {
            file,
            first_lsn,
            bytes: LOG_SEGMENT_HEADER_LEN as u64,
        });
        sync_dir(&self.dir)?;
        Ok(())
    }
}

/// Segment file name: zero-padded first LSN so lexical order is LSN order.
fn segment_file_name(first_lsn: u64) -> String {
    format!("{first_lsn:020}.seg")
}

/// fsync a directory so file creations/removals survive a crash.
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Outcome of trying to fill a buffer from a reader.
enum ReadFull {
    /// The buffer was filled.
    Full,
    /// The reader was already at end of stream.
    CleanEof,
    /// The stream ended partway through the buffer.
    Partial,
}

fn read_full<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<ReadFull> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => {
                return Ok(if filled == 0 {
                    ReadFull::CleanEof
                } else {
                    ReadFull::Partial
                });
            }
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(ReadFull::Full)
}

/// Read one segment: validate its header and records, returning the decoded
/// `(lsn, framed_bytes)` records and the byte offset after the last valid
/// record. When `allow_tail` is set, a torn trailing record is tolerated
/// (recovery of the active segment); otherwise it is corruption. The same
/// tolerance (truncate in the active segment, hard error in a sealed one)
/// applies to a **logically** incomplete trailing chain (a large
/// transaction) — a run of physically well-formed
/// `CHAIN_CONTINUE` records with no terminating `CHAIN_FINAL`, from a crash
/// mid-chain — even though each individual record in it passes its own
/// checksum. A sealed segment can never legitimately end mid-chain: writing
/// one never spans two `append_batch` calls, and appends and retention
/// truncation share one lock, so a chain is always either fully durable or
/// never started by the time any other segment could have been sealed.
/// Read one segment's framed records (see [`LogStore::segments_in`]).
///
/// # Errors
/// Returns [`LogError`] if the segment cannot be read or is corrupt.
pub fn read_segment_records(
    path: &Path,
    shard_id: u32,
    allow_tail: bool,
) -> Result<Vec<(u64, Vec<u8>)>, LogError> {
    read_segment(path, shard_id, allow_tail).map(|(records, _, _)| records)
}

fn read_segment(
    path: &Path,
    shard_id: u32,
    allow_tail: bool,
) -> Result<(Vec<(u64, Vec<u8>)>, u64, u64), LogError> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let corrupt = |detail: String| LogError::Corrupt {
        path: path.display().to_string(),
        detail,
    };

    let mut header = [0u8; LOG_SEGMENT_HEADER_LEN];
    if !matches!(read_full(&mut file, &mut header)?, ReadFull::Full) {
        return Err(corrupt("segment shorter than its header".into()));
    }
    if header[0..8] != LOG_SEGMENT_MAGIC {
        return Err(corrupt("invalid segment magic".into()));
    }
    let version = u32::from_le_bytes(header[8..12].try_into().expect("slice"));
    if version != WAL_FORMAT_VERSION {
        return Err(corrupt(format!("unsupported segment version {version}")));
    }
    let segment_shard = u32::from_le_bytes(header[12..16].try_into().expect("slice"));
    if segment_shard != shard_id {
        return Err(corrupt(format!(
            "segment shard {segment_shard} does not match {shard_id}"
        )));
    }

    let mut offset = LOG_SEGMENT_HEADER_LEN as u64;
    let mut records = Vec::new();
    let mut record_offsets = Vec::new();
    loop {
        let record_start = offset;
        let mut record_header = [0u8; LOG_RECORD_HEADER_LEN];
        match read_full(&mut file, &mut record_header)? {
            ReadFull::Full => {}
            ReadFull::CleanEof => break,
            ReadFull::Partial => {
                if allow_tail {
                    break;
                }
                return Err(corrupt("torn record header".into()));
            }
        }
        let payload_len =
            u32::from_le_bytes(record_header[24..28].try_into().expect("slice")) as usize;
        if payload_len > MAX_RECORD_BYTES {
            if allow_tail {
                break;
            }
            return Err(corrupt("record payload exceeds 64 MiB".into()));
        }
        let mut raw = Vec::with_capacity(LOG_RECORD_HEADER_LEN + payload_len);
        raw.extend_from_slice(&record_header);
        raw.resize(LOG_RECORD_HEADER_LEN + payload_len, 0);
        if !matches!(
            read_full(&mut file, &mut raw[LOG_RECORD_HEADER_LEN..])?,
            ReadFull::Full
        ) {
            if allow_tail {
                break;
            }
            return Err(corrupt("torn record payload".into()));
        }
        match decode_wal_record(&raw) {
            Ok((record, _)) => {
                records.push((record.lsn, raw));
                record_offsets.push(record_start);
                offset += (LOG_RECORD_HEADER_LEN + payload_len) as u64;
            }
            Err(error) => {
                if allow_tail {
                    break;
                }
                return Err(corrupt(format!("record checksum/format: {error}")));
            }
        }
    }

    // Chain completeness: every physically well-formed record read above
    // must also form logically complete chains. `valid_count` shrinks from
    // `records.len()` only if a chain-structure error is found (treated
    // exactly like a torn physical record) or the scan ends with a chain
    // still open (a crash mid-chain) — see this function's doc comment.
    let mut assembler = crate::wal::ChainAssembler::default();
    let mut valid_count = records.len();
    for (i, (_, raw)) in records.iter().enumerate() {
        if let Err(detail) = assembler.feed(raw, record_offsets[i]) {
            if !allow_tail {
                return Err(corrupt(detail));
            }
            valid_count = i;
            break;
        }
    }
    if valid_count == records.len() {
        if let Some(open_start) = assembler.open_start_offset() {
            if !allow_tail {
                return Err(corrupt(
                    "sealed segment ends with an incomplete chained commit".into(),
                ));
            }
            valid_count = record_offsets.partition_point(|&o| o < open_start);
        }
    }
    let valid_bytes = record_offsets.get(valid_count).copied().unwrap_or(offset);
    records.truncate(valid_count);

    Ok((records, valid_bytes, file_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wal::{encode_chained_ops_record, encode_wal_record};

    fn temp_dir() -> crate::TempPath {
        crate::TempPath::dir("logstore")
    }

    /// Fragments of one chained commit at `lsn`, as `append_batch` expects
    /// them: several entries sharing one LSN.
    fn chained_records(
        lsn: u64,
        ops: &[crate::Op],
        max_record_bytes: usize,
    ) -> Vec<(u64, Vec<u8>)> {
        encode_chained_ops_record(0, lsn, ops, None, max_record_bytes)
            .into_iter()
            .map(|framed| (lsn, framed))
            .collect()
    }

    fn big_ops(n: usize, value_len: usize) -> Vec<crate::Op> {
        (0..n)
            .map(|i| crate::Op::Put(format!("k{i}").into_bytes(), vec![b'x'; value_len]))
            .collect()
    }

    #[test]
    fn chained_commit_advances_next_lsn_once_not_per_fragment() {
        let dir = temp_dir();
        let mut log = LogStore::open(&dir, 0, 1 << 30).unwrap();
        let ops = big_ops(20, 200);
        let fragments = chained_records(1, &ops, 1024);
        assert!(fragments.len() > 1, "expected this commit to need chaining");
        log.append_batch(&fragments).unwrap();
        assert_eq!(
            log.next_lsn(),
            2,
            "a multi-fragment commit must advance next_lsn exactly once"
        );
        log.sync().unwrap();
        let all = log.read_from(1).unwrap();
        assert_eq!(
            all.len(),
            fragments.len(),
            "every physical fragment is durable"
        );

        // The next commit continues at lsn 2, unaffected by how many physical
        // records the chained commit took.
        log.append_batch(&[record(2, b"small")]).unwrap();
        assert_eq!(log.next_lsn(), 3);
    }

    #[test]
    fn chained_commit_never_splits_across_a_segment_boundary() {
        let dir = temp_dir();
        // A small rotation threshold that a chained commit's total size will
        // comfortably exceed on its own, to prove rotation happens *before*
        // the chain starts, not partway through it.
        let mut log = LogStore::open(&dir, 0, 512).unwrap();
        let ops = big_ops(30, 100);
        let fragments = chained_records(1, &ops, 1024);
        assert!(fragments.len() > 1);
        log.append_batch(&fragments).unwrap();
        log.sync().unwrap();

        // Every one of the chain's fragments must land in exactly one
        // segment (the active one at the time the chain started).
        let owning_segments: std::collections::HashSet<u64> = log
            .segments()
            .iter()
            .filter(|s| s.first_lsn <= 1 && s.last_lsn >= 1)
            .map(|s| s.first_lsn)
            .collect();
        assert_eq!(
            owning_segments.len(),
            1,
            "a chained commit must not be split across segment files"
        );
    }

    #[test]
    fn reopen_discards_an_incomplete_trailing_chain_and_retry_at_the_same_lsn_succeeds() {
        let dir = temp_dir();
        {
            let mut log = LogStore::open(&dir, 0, 1 << 30).unwrap();
            log.append_batch(&[record(1, b"first")]).unwrap();
            log.sync().unwrap();
        }
        // Simulate a crash mid-chain: append only the non-final fragments of
        // a chained commit at lsn 2 (drop the final fragment, as if the
        // process died before writing it).
        let ops = big_ops(10, 200);
        let fragments = encode_chained_ops_record(0, 2, &ops, None, 512);
        assert!(
            fragments.len() > 2,
            "need at least one continue + the final"
        );
        {
            let mut file = OpenOptions::new()
                .append(true)
                .open(dir.join(segment_file_name(1)))
                .unwrap();
            for framed in &fragments[..fragments.len() - 1] {
                file.write_all(framed).unwrap();
            }
            file.sync_all().unwrap();
        }
        let before_len = std::fs::metadata(dir.join(segment_file_name(1)))
            .unwrap()
            .len();

        let mut log = LogStore::open(&dir, 0, 1 << 30).unwrap();
        assert_eq!(
            log.next_lsn(),
            2,
            "the incomplete chain must not advance past its own lsn"
        );
        assert_eq!(
            log.read_from(1).unwrap().len(),
            1,
            "only the first, complete commit survives"
        );
        let after_len = std::fs::metadata(dir.join(segment_file_name(1)))
            .unwrap()
            .len();
        assert!(
            after_len < before_len,
            "the incomplete chain's bytes must be truncated away"
        );

        // A retry at the same, abandoned lsn must succeed cleanly — proving
        // the stale fragments were truly removed, not just ignored (which
        // would otherwise corrupt a later replay by gluing new fragments
        // onto old garbage sharing the same lsn).
        log.append_batch(&[record(2, b"retry")]).unwrap();
        log.sync().unwrap();
        assert_eq!(log.next_lsn(), 3);
        let all = log.read_from(1).unwrap();
        assert_eq!(all.len(), 2);
        let (_, ops, _) = crate::wal::decode_ops_record(&all[1].1).unwrap();
        assert_eq!(ops, vec![crate::Op::Put(Vec::new(), b"retry".to_vec())]);
    }

    /// Regression: a record whose *content* is corrupt (an embedded
    /// command that fails to parse as JSON) is structurally complete — a
    /// well-formed, checksummed, non-chained `WITH_COMMAND` frame — and
    /// must survive reopen untouched. The chain-completeness recovery pass
    /// must never fully decode a record's content just to decide whether a
    /// trailing chain is open; doing so would misclassify this as "looks
    /// like a torn write" and silently truncate a perfectly durable record
    /// whose only problem is a corrupt payload one layer up.
    #[test]
    fn reopen_never_truncates_a_structurally_complete_record_with_corrupt_content() {
        let dir = temp_dir();
        {
            let mut log = LogStore::open(&dir, 0, 1 << 30).unwrap();
            let framed = crate::wal::encode_ops_record(0, 1, &[], Some(b"not-json"));
            log.append_batch(&[(1, framed)]).unwrap();
            log.sync().unwrap();
        }
        let log = LogStore::open(&dir, 0, 1 << 30).unwrap();
        assert_eq!(
            log.next_lsn(),
            2,
            "a structurally complete record must not be discarded"
        );
        let all = log.read_from(1).unwrap();
        assert_eq!(
            all.len(),
            1,
            "the corrupt-content record must survive reopen"
        );
        // Decoding it explicitly (as a real caller needing its content
        // would) still correctly surfaces the content error — it is simply
        // never used as a signal to truncate during recovery.
        assert!(crate::wal::decode_ops_record(&all[0].1).is_err());
    }

    /// A real, decodable wal.rs-formatted record (one `Op::Put` carrying
    /// `payload` as its value) — not raw bytes — since `read_segment`'s
    /// chain-completeness pass now interprets every record's kind byte, the
    /// same as production (`Store::append_log` never writes anything else).
    fn record(lsn: u64, payload: &[u8]) -> (u64, Vec<u8>) {
        let ops = vec![crate::Op::Put(Vec::new(), payload.to_vec())];
        (lsn, crate::wal::encode_ops_record(0, lsn, &ops, None))
    }

    #[test]
    fn append_sync_read_round_trip() {
        let dir = temp_dir();
        let mut log = LogStore::open(&dir, 0, 1 << 20).unwrap();
        assert_eq!(log.next_lsn(), 1);
        assert_eq!(log.durable_lsn(), 0);
        log.append_batch(&[record(1, b"a"), record(2, b"bb"), record(3, b"ccc")])
            .unwrap();
        log.sync().unwrap();
        assert_eq!(log.durable_lsn(), 3);
        assert_eq!(log.next_lsn(), 4);

        let all = log.read_from(1).unwrap();
        assert_eq!(
            all.iter().map(|(lsn, _)| *lsn).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        let tail = log.read_from(2).unwrap();
        assert_eq!(tail.iter().map(|(lsn, _)| *lsn).collect::<Vec<_>>(), [2, 3]);
        assert!(log.read_from(4).unwrap().is_empty());
    }

    /// A fresh WAL (no segments) opened beside a
    /// materialization whose checkpoint is already ahead of it — e.g. a
    /// store opened from a restored base backup with no accompanying WAL
    /// directory — must continue the commit sequence from that checkpoint,
    /// not restart at LSN 1 and collide with LSNs the materialization
    /// already has.
    #[test]
    fn seed_next_lsn_if_empty_continues_past_a_restored_checkpoint() {
        let dir = temp_dir();
        let mut log = LogStore::open(&dir, 0, 1 << 20).unwrap();
        assert_eq!(log.next_lsn(), 1);

        log.seed_next_lsn_if_empty(51);
        assert_eq!(
            log.next_lsn(),
            51,
            "an empty log must adopt the materialization's checkpoint + 1"
        );
        log.append_batch(&[record(51, b"a")]).unwrap();
        assert_eq!(log.next_lsn(), 52);

        // Never seeds backwards past 1 for a genuinely fresh store (no
        // prior checkpoint at all).
        let fresh_dir = temp_dir();
        let mut fresh = LogStore::open(&fresh_dir, 0, 1 << 20).unwrap();
        fresh.seed_next_lsn_if_empty(1);
        assert_eq!(fresh.next_lsn(), 1);

        // A no-op once the log has real, recovered history — that `next_lsn`
        // is authoritative and must never be overridden by a floor.
        log.seed_next_lsn_if_empty(1_000);
        assert_eq!(
            log.next_lsn(),
            52,
            "seeding must not touch a log with real segments"
        );
    }

    #[test]
    fn rotates_segments_and_reads_across_them() {
        let dir = temp_dir();
        let mut log = LogStore::open(&dir, 0, 120).unwrap();
        for lsn in 1..=6 {
            log.append_batch(&[record(lsn, &[0xABu8; 40])]).unwrap();
        }
        log.sync().unwrap();
        assert!(
            log.segments().len() > 1,
            "expected rotation, got {} segment(s)",
            log.segments().len()
        );
        let all = log.read_from(1).unwrap();
        assert_eq!(
            all.iter().map(|(lsn, _)| *lsn).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn reopen_recovers_state() {
        let dir = temp_dir();
        {
            let mut log = LogStore::open(&dir, 0, 1 << 20).unwrap();
            log.append_batch(&[record(1, b"x"), record(2, b"y")])
                .unwrap();
            log.sync().unwrap();
        }
        let log = LogStore::open(&dir, 0, 1 << 20).unwrap();
        assert_eq!(log.next_lsn(), 3);
        assert_eq!(log.durable_lsn(), 2);
        assert_eq!(log.read_from(1).unwrap().len(), 2);
    }

    #[test]
    fn torn_tail_is_truncated_on_open() {
        let dir = temp_dir();
        {
            let mut log = LogStore::open(&dir, 0, 1 << 20).unwrap();
            log.append_batch(&[record(1, b"x"), record(2, b"y")])
                .unwrap();
            log.sync().unwrap();
        }
        // Simulate a crash mid-append: append a partial record to the active
        // segment (a full 32-byte header claiming a payload that is not there).
        let segment = dir.join(segment_file_name(1));
        {
            let mut file = OpenOptions::new().append(true).open(&segment).unwrap();
            let framed = encode_wal_record(0, 3, b"a much longer payload");
            file.write_all(&framed[..framed.len() - 5]).unwrap();
        }
        let before = std::fs::metadata(&segment).unwrap().len();
        let log = LogStore::open(&dir, 0, 1 << 20).unwrap();
        assert_eq!(log.next_lsn(), 3, "torn record must not advance the LSN");
        assert_eq!(log.read_from(1).unwrap().len(), 2);
        let after = std::fs::metadata(&segment).unwrap().len();
        assert!(after < before, "torn tail must be truncated");
    }

    #[test]
    fn sealed_torn_tail_is_corruption() {
        let dir = temp_dir();
        {
            let mut log = LogStore::open(&dir, 0, 120).unwrap();
            for lsn in 1..=4 {
                log.append_batch(&[record(lsn, &[0xABu8; 40])]).unwrap();
            }
            log.sync().unwrap();
        }
        // Damage the first (sealed) segment's last record.
        let first = dir.join(segment_file_name(1));
        let mut bytes = std::fs::read(&first).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        std::fs::write(&first, &bytes).unwrap();
        let error = LogStore::open(&dir, 0, 120).unwrap_err();
        assert!(matches!(error, LogError::Corrupt { .. }), "got: {error}");
    }

    #[test]
    fn out_of_order_append_is_rejected() {
        let dir = temp_dir();
        let mut log = LogStore::open(&dir, 0, 1 << 20).unwrap();
        log.append_batch(&[record(1, b"a")]).unwrap();
        let error = log.append_batch(&[record(3, b"c")]).unwrap_err();
        assert!(
            matches!(
                error,
                LogError::OutOfOrder {
                    expected: 2,
                    actual: 3
                }
            ),
            "got: {error}"
        );
    }

    #[test]
    fn truncate_before_removes_only_sealed_segments() {
        let dir = temp_dir();
        let mut log = LogStore::open(&dir, 0, 120).unwrap();
        for lsn in 1..=6 {
            log.append_batch(&[record(lsn, &[0xABu8; 40])]).unwrap();
        }
        log.sync().unwrap();
        let before = log.segments().len();
        assert!(before > 1);
        let active_first = log.segments().last().unwrap().first_lsn;
        let removed = log.truncate_before(u64::MAX).unwrap();
        assert_eq!(removed, before - 1, "all sealed segments must go");
        assert_eq!(log.segments().len(), 1);
        assert_eq!(log.segments()[0].first_lsn, active_first);
        // The active segment and its records survive.
        assert!(!log.read_from(active_first).unwrap().is_empty());
    }

    #[test]
    fn rejects_foreign_shard_segment() {
        let dir = temp_dir();
        {
            let mut log = LogStore::open(&dir, 7, 1 << 20).unwrap();
            log.append_batch(&[record(1, b"a")]).unwrap();
            log.sync().unwrap();
        }
        let error = LogStore::open(&dir, 0, 1 << 20).unwrap_err();
        assert!(matches!(error, LogError::Corrupt { .. }), "got: {error}");
    }
}
