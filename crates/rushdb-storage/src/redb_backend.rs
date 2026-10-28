use std::collections::BTreeMap;
use std::ops::Bound;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::history_v2;
use crate::{
    Op, ReadLimits, ScanProjection, StorageError, StorageStatistics, VersionEntry, LAST_APPLIED_KEY,
};

const DATA: TableDefinition<&[u8], &[u8]> = TableDefinition::new("rushdb.data.v1");
const HISTORY: TableDefinition<&[u8], &[u8]> = TableDefinition::new("rushdb.history.v1");
/// Sealed, append-only history blocks (P05 append-friendly segments). The
/// `HISTORY` row is the head (newest versions); older versions are sealed here
/// and linked by `prev`, so a hot-key append rewrites only the head.
const HISTORY_BLOCK: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("rushdb.history_block.v1");
/// Expiry-ordered secondary index over [`HISTORY`]: one row per retained
/// version, keyed `to_ci_be || data_key`, value empty. Lets GC range straight
/// to the expired versions instead of decoding every key's version list.
const HISTORY_EXPIRY: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("rushdb.history_expiry.v1");
/// Expiry rows carry no payload; the key alone identifies the version.
const EMPTY_VALUE: &[u8] = &[];
const META: TableDefinition<&str, u64> = TableDefinition::new("rushdb.meta.v1");
const CHECKPOINT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("rushdb.checkpoint.v1");
const GC_WATERMARK: &str = "gc_watermark";
const CHECKPOINT_KEY: &[u8] = b"checkpoint";

/// Data rows carry the commit index that wrote the current version as an
/// 8-byte big-endian header in front of the value. This was a separate
/// `created` table keyed by every data key, which duplicated every key on disk
/// (~0.9 GB plus as much B-tree slack at 200K records), cost a second B-tree
/// write per op, and a second lookup per snapshot read (C10, cloud-capacity
/// F1/F4).
const VERSION_HEADER: usize = 8;

fn encode_current(commit_index: u64, value: &[u8]) -> Vec<u8> {
    let mut row = Vec::with_capacity(VERSION_HEADER + value.len());
    row.extend_from_slice(&commit_index.to_be_bytes());
    row.extend_from_slice(value);
    row
}

/// Split a data row into `(commit index that wrote it, value)`.
fn decode_current(row: &[u8]) -> Result<(u64, &[u8]), StorageError> {
    let header = row
        .get(..VERSION_HEADER)
        .and_then(|bytes| <[u8; VERSION_HEADER]>::try_from(bytes).ok())
        .ok_or(StorageError::CorruptLog)?;
    Ok((u64::from_be_bytes(header), &row[VERSION_HEADER..]))
}

/// The value part of a data row.
fn current_value(row: &[u8]) -> Result<&[u8], StorageError> {
    decode_current(row).map(|(_, value)| value)
}

/// All sealed-block keys for `data_key`, ascending.
fn block_keys(
    blocks: &redb::Table<'_, &[u8], &[u8]>,
    data_key: &[u8],
) -> Result<Vec<Vec<u8>>, String> {
    let prefix = history_v2::block_prefix(data_key);
    let (start, end) = crate::prefix_range(&prefix);
    let mut out = Vec::new();
    for row in blocks
        .range::<&[u8]>((
            Bound::Included(start.as_slice()),
            Bound::Excluded(end.as_slice()),
        ))
        .map_err(|e| e.to_string())?
    {
        out.push(row.map_err(|e| e.to_string())?.0.value().to_vec());
    }
    Ok(out)
}

/// Append one version to a key's history. The newest `<= BLOCK_SIZE` versions
/// live in the `HISTORY` head; when it overflows, the oldest are sealed into an
/// immutable `HISTORY_BLOCK` and linked by `prev`. A hot-key append therefore
/// rewrites only the bounded head, not the whole version list.
fn append_history(
    history: &mut redb::Table<'_, &[u8], &[u8]>,
    blocks: &mut redb::Table<'_, &[u8], &[u8]>,
    data_key: &[u8],
    version: VersionEntry,
) -> Result<(), String> {
    let existing = history
        .get(data_key)
        .map_err(|e| e.to_string())?
        .map(|value| value.value().to_vec());
    let (mut prev, mut versions) = match existing {
        Some(raw) => {
            history_v2::decode_block(&raw).map_err(|e| format!("{e}: corrupt history head"))?
        }
        None => (0u64, Vec::new()),
    };
    versions.insert(0, version); // newest first
    if versions.len() > history_v2::BLOCK_SIZE
        || history_v2::encode_block(prev, &versions).len() > history_v2::BLOCK_MAX_BYTES
    {
        // Seal a full BLOCK_SIZE of the oldest versions, leaving the newest in
        // the head. This happens once per BLOCK_SIZE appends, so a hot-key
        // append is amortized O(1) rather than rewriting the whole list.
        let mut take = versions.len().min(history_v2::BLOCK_SIZE);
        while take > 1
            && history_v2::encode_block(prev, &versions[versions.len() - take..]).len()
                > history_v2::BLOCK_MAX_BYTES
        {
            take -= 1;
        }
        let sealed = versions.split_off(versions.len() - take);
        let seq = sealed[0].to_ci;
        blocks
            .insert(
                history_v2::block_key(data_key, seq).as_slice(),
                history_v2::encode_block(prev, &sealed).as_slice(),
            )
            .map_err(|e| e.to_string())?;
        prev = seq;
    }
    history
        .insert(
            data_key,
            history_v2::encode_block(prev, &versions).as_slice(),
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Replace a key's history with `kept` (newest first), deleting every sealed
/// block and rewriting the head/chain.
fn write_chain(
    history: &mut redb::Table<'_, &[u8], &[u8]>,
    blocks: &mut redb::Table<'_, &[u8], &[u8]>,
    data_key: &[u8],
    kept: Vec<VersionEntry>,
) -> Result<(), String> {
    for block in block_keys(blocks, data_key)? {
        blocks.remove(block.as_slice()).map_err(|e| e.to_string())?;
    }
    if kept.is_empty() {
        history.remove(data_key).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let mut remaining = kept;
    let mut chunks: Vec<Vec<VersionEntry>> = Vec::new();
    while !remaining.is_empty() {
        let mut take = remaining.len().min(history_v2::BLOCK_SIZE);
        while take > 1
            && history_v2::encode_block(0, &remaining[..take]).len() > history_v2::BLOCK_MAX_BYTES
        {
            take -= 1;
        }
        chunks.push(remaining.drain(..take).collect());
    }
    let head = chunks.remove(0);
    let mut prev = 0u64;
    for chunk in chunks.into_iter().rev() {
        let seq = chunk[0].to_ci;
        blocks
            .insert(
                history_v2::block_key(data_key, seq).as_slice(),
                history_v2::encode_block(prev, &chunk).as_slice(),
            )
            .map_err(|e| e.to_string())?;
        prev = seq;
    }
    history
        .insert(data_key, history_v2::encode_block(prev, &head).as_slice())
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Drop a key's retained history (head + sealed blocks) and every expiry-index
/// row that points at it. Used by `Set`/`Purge`, which overwrite or erase
/// without retaining a version.
fn drop_history_indexed(
    history: &mut redb::Table<'_, &[u8], &[u8]>,
    blocks: &mut redb::Table<'_, &[u8], &[u8]>,
    expiry: &mut redb::Table<'_, &[u8], &[u8]>,
    key: &[u8],
) -> Result<(), String> {
    let mut drop_expiry = |versions: &[VersionEntry]| -> Result<(), String> {
        for version in versions {
            if version.to_ci != 0 {
                expiry
                    .remove(history_v2::expiry_key(version.to_ci, key).as_slice())
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    };
    if let Some(raw) = history.get(key).map_err(|e| e.to_string())? {
        let (_prev, versions) =
            history_v2::decode_block(raw.value()).map_err(|e| format!("{e}: corrupt history"))?;
        drop_expiry(&versions)?;
    }
    history.remove(key).map_err(|e| e.to_string())?;
    for block in block_keys(blocks, key)? {
        if let Some(raw) = blocks.get(block.as_slice()).map_err(|e| e.to_string())? {
            let (_prev, versions) = history_v2::decode_block(raw.value())
                .map_err(|e| format!("{e}: corrupt history block"))?;
            drop_expiry(&versions)?;
        }
        blocks.remove(block.as_slice()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Reclaim versions with `to_ci <= watermark` for one key, removing their
/// expiry rows and rebuilding the chain from the survivors.
fn prune_history(
    history: &mut redb::Table<'_, &[u8], &[u8]>,
    blocks: &mut redb::Table<'_, &[u8], &[u8]>,
    expiry: &mut redb::Table<'_, &[u8], &[u8]>,
    data_key: &[u8],
    watermark: u64,
) -> Result<(), String> {
    let mut kept: Vec<VersionEntry> = Vec::new();
    let mut next: Option<u64> = None;
    loop {
        let raw = match next {
            None => history
                .get(data_key)
                .map_err(|e| e.to_string())?
                .map(|value| value.value().to_vec()),
            Some(seq) => blocks
                .get(history_v2::block_key(data_key, seq).as_slice())
                .map_err(|e| e.to_string())?
                .map(|value| value.value().to_vec()),
        };
        let Some(raw) = raw else {
            break;
        };
        let (prev, versions) =
            history_v2::decode_block(&raw).map_err(|e| format!("{e}: corrupt history block"))?;
        for version in versions {
            if version.to_ci == 0 || version.to_ci > watermark {
                kept.push(version);
            } else {
                expiry
                    .remove(history_v2::expiry_key(version.to_ci, data_key).as_slice())
                    .map_err(|e| e.to_string())?;
            }
        }
        if prev == 0 {
            break;
        }
        next = Some(prev);
    }
    write_chain(history, blocks, data_key, kept)
}

/// Shared per-op MVCC application against an open redb write transaction.
fn apply_in_txn(tx: &redb::WriteTransaction, ops: &[Op], commit_index: u64) -> Result<(), String> {
    let mut data = tx.open_table(DATA).map_err(|e| e.to_string())?;
    let mut history = tx.open_table(HISTORY).map_err(|e| e.to_string())?;
    let mut blocks = tx.open_table(HISTORY_BLOCK).map_err(|e| e.to_string())?;
    let mut expiry = tx.open_table(HISTORY_EXPIRY).map_err(|e| e.to_string())?;
    let mut tables = MvccTables {
        data: &mut data,
        history: &mut history,
        blocks: &mut blocks,
        expiry: &mut expiry,
    };
    let mut usage = crate::usage::UsageBytes::default();
    for op in ops {
        apply_op(&mut tables, op, commit_index, &mut usage)?;
    }
    let rows = usage.rows(|key| {
        tables
            .data
            .get(key)
            .map_err(|e| e.to_string())?
            .map(|row| {
                current_value(row.value())
                    .map(<[u8]>::to_vec)
                    .map_err(|e| e.to_string())
            })
            .transpose()
    })?;
    for (key, value) in rows {
        let op = match value {
            Some(value) => Op::Put(key, value),
            None => Op::Del(key),
        };
        apply_op(
            &mut tables,
            &op,
            commit_index,
            &mut crate::usage::UsageBytes::default(),
        )?;
    }
    Ok(())
}

/// The open MVCC tables of one redb write transaction.
struct MvccTables<'a, 't> {
    data: &'a mut redb::Table<'t, &'static [u8], &'static [u8]>,
    history: &'a mut redb::Table<'t, &'static [u8], &'static [u8]>,
    blocks: &'a mut redb::Table<'t, &'static [u8], &'static [u8]>,
    expiry: &'a mut redb::Table<'t, &'static [u8], &'static [u8]>,
}

/// Apply one op's MVCC transition, recording its byte change in `usage`.
fn apply_op(
    tables: &mut MvccTables<'_, '_>,
    op: &Op,
    commit_index: u64,
    usage: &mut crate::usage::UsageBytes,
) -> Result<(), String> {
    let MvccTables {
        data,
        history,
        blocks,
        expiry,
    } = tables;
    {
        let key = match op {
            Op::Put(key, _) | Op::Del(key) | Op::Set(key, _) | Op::Purge(key) => key,
        };
        // Purge is destructive: erase the key's current value, metadata and
        // retained history without appending a superseded version.
        if matches!(op, Op::Purge(_)) {
            let old = data.remove(key.as_slice()).map_err(|e| e.to_string())?;
            let old = old
                .map(|row| current_value(row.value()).map(<[u8]>::len))
                .transpose()
                .map_err(|e| e.to_string())?;
            usage.record(key, old, None);
            drop_history_indexed(history, blocks, expiry, key)?;
            return Ok(());
        }
        // Set overwrites internal bookkeeping without retaining a version:
        // insert the new value, advance the creation index, drop any history
        // already accumulated for the key (bounds hot-counter write cost).
        if let Op::Set(_, value) = op {
            let old = data
                .insert(
                    key.as_slice(),
                    encode_current(commit_index, value).as_slice(),
                )
                .map_err(|e| e.to_string())?;
            let old = old
                .map(|row| current_value(row.value()).map(<[u8]>::len))
                .transpose()
                .map_err(|e| e.to_string())?;
            usage.record(key, old, Some(value.len()));
            drop_history_indexed(history, blocks, expiry, key)?;
            return Ok(());
        }
        let previous = data
            .get(key.as_slice())
            .map_err(|e| e.to_string())?
            .map(|row| {
                decode_current(row.value())
                    .map(|(from_ci, value)| (from_ci, value.to_vec()))
                    .map_err(|e| e.to_string())
            })
            .transpose()?;

        usage.record(
            key,
            previous.as_ref().map(|(_, value)| value.len()),
            match op {
                Op::Put(_, value) => Some(value.len()),
                _ => None,
            },
        );
        if let Some((from_ci, previous_value)) = previous {
            append_history(
                history,
                blocks,
                key,
                VersionEntry {
                    from_ci,
                    to_ci: commit_index,
                    value: Some(previous_value),
                },
            )?;
            expiry
                .insert(
                    history_v2::expiry_key(commit_index, key).as_slice(),
                    EMPTY_VALUE,
                )
                .map_err(|e| e.to_string())?;
        }

        match op {
            Op::Put(key, value) => {
                data.insert(
                    key.as_slice(),
                    encode_current(commit_index, value).as_slice(),
                )
                .map_err(|e| e.to_string())?;
            }
            Op::Del(key) => {
                data.remove(key.as_slice()).map_err(|e| e.to_string())?;
            }
            Op::Purge(_) => unreachable!("purge is handled before version bookkeeping"),
            Op::Set(..) => unreachable!("set is handled before version bookkeeping"),
        }
    }
    Ok(())
}

pub(crate) struct RedbState {
    pub(crate) data: BTreeMap<Vec<u8>, Vec<u8>>,
    pub(crate) created: BTreeMap<Vec<u8>, u64>,
    pub(crate) history: BTreeMap<Vec<u8>, Vec<VersionEntry>>,
    pub(crate) gc_watermark: u64,
}

/// Crash-safe durable representation of the `RushDB` ordered keyspace.
///
/// redb owns page allocation, checksums, recovery, MVCC and fsync semantics.
/// `RushDB` only maps its logical key/value operations and historical metadata
/// onto redb tables in one ACID write transaction.
pub(crate) struct RedbBackend {
    /// redb is MVCC: read transactions never wait for the writer, and it
    /// serializes write transactions internally. Every read and write path
    /// therefore takes the **shared** guard; only physical compaction (which
    /// needs `&mut Database`) takes the exclusive one. A plain mutex here made
    /// every `read_view()` wait for the whole write transaction, page-miss I/O
    /// included (C1, cloud-capacity F3).
    db: RwLock<Database>,
    // Declared after db so the cache is destroyed before its charge releases.
    _cache_reservation: crate::resources::ByteReservation,
    shard_id: u32,
    /// Set at shutdown so long maintenance scans (e.g. byte accounting) stop
    /// within a batch instead of delaying a clean close.
    abort: Arc<AtomicBool>,
    /// Cumulative nanoseconds spent waiting to acquire the redb database lock
    /// (RM-7 backend-lock measurement).
    mutex_wait_ns: AtomicU64,
    /// Cumulative nanoseconds the redb database lock was held (any mode).
    mutex_hold_ns: AtomicU64,
    /// Maximum single hold time of the redb database lock, in nanoseconds.
    mutex_hold_max_ns: AtomicU64,
    /// Write-transaction holds by holder.
    writer_holds: WriterHoldCounters,
    /// RM-8 fault injection: fail the next `apply`/`apply_many`.
    #[cfg(test)]
    fail_next_apply: AtomicBool,
    /// RM-8 fault injection: fail the next `retire_before`.
    #[cfg(test)]
    fail_next_retire: AtomicBool,
    /// RM-8 fault injection: fail the next `compact`.
    #[cfg(test)]
    fail_next_compact: AtomicBool,
    live_reads: Arc<LiveReads>,
}

/// Longest work one history-GC batch does while holding the writer. Keys
/// with long version chains made a 512-key batch hold it for over a second
/// beside a bulk load, stalling every tenant's commits (SCALE-2, 2026-10-19:
/// 2 of 4,197 batches over 1 s, 14 over 0.25 s).
const HISTORY_GC_BATCH_MAX_HOLD: std::time::Duration = std::time::Duration::from_millis(100);

/// Per-holder writer-hold histograms: counts per bucket of
/// [`crate::WRITER_HOLD_BOUNDS`] (last slot: above every bound) and the
/// total nanoseconds held.
#[derive(Default)]
struct WriterHoldCounters {
    counts: [[AtomicU64; crate::WRITER_HOLD_BOUNDS.len() + 1]; crate::WriterHolder::ALL.len()],
    sum_ns: [AtomicU64; crate::WriterHolder::ALL.len()],
}

impl WriterHoldCounters {
    fn record(&self, holder: crate::WriterHolder, held: std::time::Duration) {
        let seconds = held.as_secs_f64();
        let bucket = crate::WRITER_HOLD_BOUNDS
            .iter()
            .position(|bound| seconds <= *bound)
            .unwrap_or(crate::WRITER_HOLD_BOUNDS.len());
        self.counts[holder.index()][bucket].fetch_add(1, Ordering::Relaxed);
        self.sum_ns[holder.index()].fetch_add(
            u64::try_from(held.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }

    fn snapshot(&self) -> Vec<(crate::WriterHolder, crate::WriterHolds)> {
        crate::WriterHolder::ALL
            .into_iter()
            .map(|holder| {
                let mut holds = crate::WriterHolds::default();
                for (count, counter) in holds.counts.iter_mut().zip(&self.counts[holder.index()]) {
                    *count = counter.load(Ordering::Relaxed);
                }
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a metric sum; nanosecond precision past 2^53 is not needed"
                )]
                let seconds = self.sum_ns[holder.index()].load(Ordering::Relaxed) as f64 / 1e9;
                holds.sum_seconds = seconds;
                (holder, holds)
            })
            .collect()
    }
}

/// Times one write transaction from before `begin_write` until it drops,
/// after the commit or abort.
struct WriterHold<'a> {
    counters: &'a WriterHoldCounters,
    holder: crate::WriterHolder,
    started: std::time::Instant,
}

impl Drop for WriterHold<'_> {
    fn drop(&mut self) {
        self.counters.record(self.holder, self.started.elapsed());
    }
}

/// A redb database-lock guard (shared or exclusive) that records wait/hold
/// durations on drop.
struct TimedDbGuard<'a, G> {
    inner: G,
    held_at: std::time::Instant,
    hold_ns: &'a AtomicU64,
    hold_max_ns: &'a AtomicU64,
}

impl<G: std::ops::Deref<Target = Database>> std::ops::Deref for TimedDbGuard<'_, G> {
    type Target = Database;
    fn deref(&self) -> &Database {
        &self.inner
    }
}

impl<G: std::ops::DerefMut<Target = Database>> std::ops::DerefMut for TimedDbGuard<'_, G> {
    fn deref_mut(&mut self) -> &mut Database {
        &mut self.inner
    }
}

impl<G> Drop for TimedDbGuard<'_, G> {
    fn drop(&mut self) {
        let held = u64::try_from(self.held_at.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.hold_ns.fetch_add(held, Ordering::Relaxed);
        self.hold_max_ns.fetch_max(held, Ordering::Relaxed);
    }
}

/// `ReadTransaction` owns its snapshot state (no borrowed Database lifetime in
/// redb 4.2). The database mutex is not retained while serving reads.
pub(crate) struct RedbReadView {
    tx: redb::ReadTransaction,
    /// The data table, opened once per view. Re-opening it on every point
    /// read was ~20% of a vector search's CPU (C9 follow-up).
    data: std::sync::OnceLock<redb::ReadOnlyTable<&'static [u8], &'static [u8]>>,
    /// Declared last so it is released after the transaction ends.
    _live: LiveRead,
}

/// Live redb read transactions of one backend. redb refuses to compact while
/// any read transaction is open ("A transaction is still in progress"), and a
/// read transaction outlives the database lock it was opened under, so the
/// exclusive lock alone does not keep readers out. Every `begin_read` is
/// counted (under the shared lock); under the exclusive lock no new one can
/// start, so a zero count there is final.
#[derive(Default)]
struct LiveReads(AtomicU64);

impl LiveReads {
    fn enter(self: &Arc<Self>) -> LiveRead {
        self.0.fetch_add(1, Ordering::AcqRel);
        LiveRead(Arc::clone(self))
    }

    fn is_drained(&self) -> bool {
        self.0.load(Ordering::Acquire) == 0
    }
}

/// One counted read transaction; decrements [`LiveReads`] on drop.
struct LiveRead(Arc<LiveReads>);

impl Drop for LiveRead {
    fn drop(&mut self) {
        self.0 .0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// A read transaction with its [`LiveRead`]; fields drop in declaration
/// order, so the transaction ends before the count is released.
struct CountedRead {
    tx: redb::ReadTransaction,
    live: LiveRead,
}

impl std::ops::Deref for CountedRead {
    type Target = redb::ReadTransaction;
    fn deref(&self) -> &redb::ReadTransaction {
        &self.tx
    }
}

/// Wrap a backend error as a durable-storage failure.
fn read_error(error: impl std::fmt::Display) -> StorageError {
    StorageError::Durability(error.to_string())
}

/// Derive the physical shard id from the redb file name: the unsuffixed file
/// is shard 0, `rushdb-3.redb` is shard 3.
fn shard_id_from_path(path: &Path) -> Result<u32, String> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "redb shard path has no valid UTF-8 file stem".to_string())?;
    match stem.rsplit_once('-') {
        Some((_, suffix)) => suffix.parse::<u32>().map_or_else(
            |_| {
                // Test and operator-created files may carry a random
                // hexadecimal suffix while still denoting the primary
                // unsuffixed shard. Only that explicit form is accepted;
                // arbitrary names must not silently become shard 0.
                if suffix.len() == 12 && suffix.bytes().all(|b| b.is_ascii_hexdigit()) {
                    Ok(0)
                } else {
                    Err(format!("invalid redb shard suffix `{suffix}`"))
                }
            },
            Ok,
        ),
        None => Ok(0),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl RedbReadView {
    fn data_table(
        &self,
    ) -> Result<&redb::ReadOnlyTable<&'static [u8], &'static [u8]>, StorageError> {
        if let Some(table) = self.data.get() {
            return Ok(table);
        }
        let table = self.tx.open_table(DATA).map_err(read_error)?;
        // A racing reader may have set it first; either table is equivalent.
        let _ = self.data.set(table);
        self.data.get().ok_or(StorageError::CorruptLog)
    }

    pub(crate) fn get_current_projected(
        &self,
        key: &[u8],
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let data = self.data_table()?;
        data.get(key)
            .map_err(read_error)?
            .map(|row| {
                if projection == ScanProjection::Keys {
                    return Ok(Vec::new());
                }
                let value = current_value(row.value())?;
                limits.check_item(key.len(), value.len())?;
                Ok(value.to_vec())
            })
            .transpose()
    }

    pub(crate) fn current_range(
        &self,
        start: &[u8],
        end: &[u8],
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<impl Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>>, StorageError> {
        let data = self.data_table()?;
        let bounds = (
            Bound::Included(start),
            if end.is_empty() {
                Bound::Unbounded
            } else {
                Bound::Excluded(end)
            },
        );
        let range = data.range::<&[u8]>(bounds).map_err(read_error)?;
        Ok(range.map(move |entry| {
            let (key, value) = entry.map_err(read_error)?;
            if projection == ScanProjection::Keys {
                limits.check_item(key.value().len(), 0)?;
                return Ok((key.value().to_vec(), Vec::new()));
            }
            let value = current_value(value.value())?;
            limits.check_item(key.value().len(), value.len())?;
            Ok((key.value().to_vec(), value.to_vec()))
        }))
    }

    /// `ScanCursor`'s v2 native-iterator fast path (B7 follow-up): a real
    /// redb range cursor over `(lower, upper)`, one seek for the whole
    /// range instead of one per row. Mirrors [`Self::next_key`]'s bounds
    /// construction (arbitrary `Bound` pairs, not [`Self::current_range`]'s
    /// simplified convention) so it accepts `ScanCursor`'s bounds directly.
    pub(crate) fn native_range(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<impl Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>>, StorageError> {
        let data = self.data_table()?;
        let bounds = (
            lower.as_ref().map(Vec::as_slice),
            upper.as_ref().map(Vec::as_slice),
        );
        let range = data.range::<&[u8]>(bounds).map_err(read_error)?;
        Ok(range.map(move |entry| {
            let (key, value) = entry.map_err(read_error)?;
            if projection == ScanProjection::Keys {
                limits.check_item(key.value().len(), 0)?;
                return Ok((key.value().to_vec(), Vec::new()));
            }
            let value = current_value(value.value())?;
            limits.check_item(key.value().len(), value.len())?;
            Ok((key.value().to_vec(), value.to_vec()))
        }))
    }

    pub(crate) fn visible_ci(&self) -> Result<u64, StorageError> {
        let table = self.data_table()?;
        let value = table.get(LAST_APPLIED_KEY).map_err(read_error)?;
        match value {
            None => Ok(0),
            Some(row) => crate::wal::decode_last_applied(current_value(row.value())?)
                .map_err(|_| StorageError::CorruptLog),
        }
    }

    pub(crate) fn history_floor(&self) -> Result<u64, StorageError> {
        let table = self.tx.open_table(META).map_err(read_error)?;
        Ok(table
            .get(GC_WATERMARK)
            .map_err(read_error)?
            .map_or(0, |value| value.value()))
    }

    pub(crate) fn get_at(
        &self,
        key: &[u8],
        ci: u64,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let data = self.data_table()?;
        if let Some(row) = data.get(key).map_err(read_error)? {
            let (from_ci, value) = decode_current(row.value())?;
            if from_ci <= ci {
                if projection == ScanProjection::Keys {
                    return Ok(Some(Vec::new()));
                }
                limits.check_item(key.len(), value.len())?;
                return Ok(Some(value.to_vec()));
            }
        }
        // Walk the head, then the sealed-block chain, for the version covering
        // `ci` (newest first; exactly one version can satisfy the range).
        let history = self.tx.open_table(HISTORY).map_err(read_error)?;
        let blocks = self.tx.open_table(HISTORY_BLOCK).map_err(read_error)?;
        let mut next: Option<u64> = None;
        loop {
            let raw = match next {
                None => history
                    .get(key)
                    .map_err(read_error)?
                    .map(|value| value.value().to_vec()),
                Some(seq) => blocks
                    .get(history_v2::block_key(key, seq).as_slice())
                    .map_err(read_error)?
                    .map(|value| value.value().to_vec()),
            };
            let Some(raw) = raw else {
                return Ok(None);
            };
            if raw.len() > limits.max_history_block_bytes {
                return Err(StorageError::ResourceExhausted(
                    "MVCC history block exceeds read limit".into(),
                ));
            }
            let (prev, versions) = history_v2::decode_block(&raw).map_err(read_error)?;
            if let Some(value) = versions
                .into_iter()
                .find(|v| v.from_ci <= ci && (v.to_ci == 0 || ci < v.to_ci))
                .and_then(|v| v.value)
            {
                return if projection == ScanProjection::Keys {
                    Ok(Some(Vec::new()))
                } else {
                    limits.check_item(key.len(), value.len())?;
                    Ok(Some(value))
                };
            }
            if prev == 0 {
                return Ok(None);
            }
            next = Some(prev);
        }
    }

    pub(crate) fn next_key(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        include_history: bool,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let data = self.data_table()?;
        let bounds = (
            lower.as_ref().map(Vec::as_slice),
            upper.as_ref().map(Vec::as_slice),
        );
        let current = data
            .range::<&[u8]>(bounds)
            .map_err(read_error)?
            .next()
            .transpose()
            .map_err(read_error)?;
        if !include_history {
            return current
                .map(|(key, _)| {
                    limits.check_item(key.value().len(), 0)?;
                    Ok(key.value().to_vec())
                })
                .transpose();
        }
        let history = self.tx.open_table(HISTORY).map_err(read_error)?;
        let old = history
            .range::<&[u8]>(bounds)
            .map_err(read_error)?
            .next()
            .transpose()
            .map_err(read_error)?;
        let key = match (&current, &old) {
            (Some((a, _)), Some((b, _))) => Some(a.value().min(b.value())),
            (Some((a, _)), None) => Some(a.value()),
            (None, Some((b, _))) => Some(b.value()),
            (None, None) => None,
        };
        key.map(|key| {
            limits.check_item(key.len(), 0)?;
            Ok(key.to_vec())
        })
        .transpose()
    }
}

impl crate::backend::StorageReadView for RedbReadView {
    fn read_value(
        &self,
        key: &[u8],
        ci: u64,
        current: bool,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        crate::read_accounting::record("get", key);
        if current {
            self.get_current_projected(key, limits, projection)
        } else {
            self.get_at(key, ci, limits, projection)
        }
    }

    fn next_key(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        include_history: bool,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        if let Bound::Included(key) | Bound::Excluded(key) = lower {
            crate::read_accounting::record("seek", key);
        }
        RedbReadView::next_key(self, lower, upper, limits, include_history)
    }

    fn current_range<'a>(
        &'a self,
        start: &[u8],
        end: &[u8],
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>, StorageError>
    {
        crate::read_accounting::record("scan", start);
        Ok(Box::new(RedbReadView::current_range(
            self, start, end, limits, projection,
        )?))
    }

    fn native_range<'a>(
        &'a self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<
        Option<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>>,
        StorageError,
    > {
        if let Bound::Included(key) | Bound::Excluded(key) = lower {
            crate::read_accounting::record("scan", key);
        }
        Ok(Some(Box::new(RedbReadView::native_range(
            self, lower, upper, limits, projection,
        )?)))
    }

    fn visible_ci(&self) -> Result<u64, StorageError> {
        RedbReadView::visible_ci(self)
    }

    fn history_floor(&self) -> Result<u64, StorageError> {
        RedbReadView::history_floor(self)
    }

    fn snapshot_pinned(&self) -> bool {
        true
    }
}

/// A native read view over every physical shard of one store.
///
/// Physical routing sends each key to exactly one shard, so point reads return
/// the first shard that holds the key and ordered reads merge the per-shard
/// iterators. This lets a multi-shard store serve reads straight from its
/// backend with no in-memory mirror. A single-shard store keeps the
/// optimized current-transaction read; a multi-shard store resolves each key at
/// the snapshot commit index.
pub(crate) struct MergedViews {
    views: Vec<Box<dyn crate::backend::StorageReadView>>,
}

impl MergedViews {
    pub(crate) fn new(views: Vec<Box<dyn crate::backend::StorageReadView>>) -> Self {
        Self { views }
    }

    pub(crate) fn is_single(&self) -> bool {
        self.views.len() == 1
    }

    /// True only when every shard's view is a physically pinned snapshot, so
    /// the `current` fast path is valid for the whole merged view.
    pub(crate) fn snapshot_pinned(&self) -> bool {
        self.views.iter().all(|view| view.snapshot_pinned())
    }

    /// Largest durable commit index across shards (the global watermark).
    pub(crate) fn visible_ci(&self) -> Result<u64, StorageError> {
        let mut visible = 0;
        for view in &self.views {
            visible = visible.max(view.visible_ci()?);
        }
        Ok(visible)
    }

    /// Highest GC floor across shards: a snapshot is serveable only where every
    /// shard still retains history.
    pub(crate) fn history_floor(&self) -> Result<u64, StorageError> {
        let mut floor = 0;
        for view in &self.views {
            floor = floor.max(view.history_floor()?);
        }
        Ok(floor)
    }

    /// Point read of the current value across shards.
    pub(crate) fn get_current(
        &self,
        key: &[u8],
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        for view in &self.views {
            if let Some(value) = view.read_value(key, 0, true, limits, projection)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    /// Point read at snapshot `ci`; `current` selects the single-shard
    /// current-table fast path when the snapshot is the latest commit.
    pub(crate) fn read_value(
        &self,
        key: &[u8],
        ci: u64,
        current: bool,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        if self.is_single() {
            return self.views[0].read_value(key, ci, current, limits, projection);
        }
        for view in &self.views {
            if let Some(value) = view.read_value(key, ci, false, limits, projection)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    /// Point read at snapshot `ci` (no current-transaction fast path).
    pub(crate) fn get_at(
        &self,
        key: &[u8],
        ci: u64,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        self.read_value(key, ci, false, limits, projection)
    }

    /// Smallest next key across shards within `(lower, upper)`.
    pub(crate) fn next_key(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        current: bool,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        if self.is_single() {
            return self.views[0].next_key(lower, upper, limits, !current);
        }
        let mut best: Option<Vec<u8>> = None;
        for view in &self.views {
            if let Some(key) = view.next_key(lower, upper, limits, true)? {
                if best.as_ref().is_none_or(|current| key < *current) {
                    best = Some(key);
                }
            }
        }
        Ok(best)
    }

    /// Ordered k-way merge of every shard's current range.
    pub(crate) fn current_range<'a>(
        &'a self,
        start: &[u8],
        end: &[u8],
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<MergeRanges<'a>, StorageError> {
        let mut sources = Vec::with_capacity(self.views.len());
        for view in &self.views {
            sources.push(view.current_range(start, end, limits, projection)?);
        }
        Ok(MergeRanges::new(sources))
    }

    /// `ScanCursor`'s v2 native-iterator fast path (B7 follow-up),
    /// `Bound`-based like [`StorageReadView::native_range`]: k-way merge of
    /// every shard's native cursor. `Ok(None)` when any shard lacks the
    /// fast path (falls back to seek-per-row for the whole merged view —
    /// a partial fast path would need a per-shard fallback inside the
    /// merge, not worth the complexity while every production backend
    /// either has it or is single-shard).
    pub(crate) fn native_range<'a>(
        &'a self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<MergeRanges<'a>>, StorageError> {
        let mut sources = Vec::with_capacity(self.views.len());
        for view in &self.views {
            match view.native_range(lower, upper, limits, projection)? {
                Some(source) => sources.push(source),
                None => return Ok(None),
            }
        }
        Ok(Some(MergeRanges::new(sources)))
    }
}

/// One pending head of a [`MergeRanges`] source.
enum MergeHead {
    Row(Vec<u8>, Vec<u8>),
    Error(StorageError),
    Done,
}

/// Lazy k-way merge of ordered per-shard ranges. Keys are unique to one shard,
/// so equal keys cannot occur: the merge emits the smallest pending key from
/// whichever shard owns it, one row at a time (no materialization).
pub(crate) struct MergeRanges<'a> {
    sources: Vec<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>>,
    heads: Vec<MergeHead>,
}

impl<'a> MergeRanges<'a> {
    fn new(
        sources: Vec<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>>,
    ) -> Self {
        let mut merge = Self {
            heads: (0..sources.len()).map(|_| MergeHead::Done).collect(),
            sources,
        };
        for index in 0..merge.sources.len() {
            merge.advance(index);
        }
        merge
    }

    fn advance(&mut self, index: usize) {
        self.heads[index] = match self.sources[index].next() {
            Some(Ok((key, value))) => MergeHead::Row(key, value),
            Some(Err(error)) => MergeHead::Error(error),
            None => MergeHead::Done,
        };
    }
}

impl Iterator for MergeRanges<'_> {
    type Item = Result<(Vec<u8>, Vec<u8>), StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self
            .heads
            .iter()
            .position(|head| matches!(head, MergeHead::Error(_)))
        {
            if let MergeHead::Error(error) =
                std::mem::replace(&mut self.heads[index], MergeHead::Done)
            {
                return Some(Err(error));
            }
        }
        let mut best: Option<usize> = None;
        for (index, head) in self.heads.iter().enumerate() {
            if let MergeHead::Row(key, _) = head {
                let replace = match best {
                    None => true,
                    Some(best_index) => match &self.heads[best_index] {
                        MergeHead::Row(best_key, _) => key < best_key,
                        _ => true,
                    },
                };
                if replace {
                    best = Some(index);
                }
            }
        }
        let index = best?;
        let head = std::mem::replace(&mut self.heads[index], MergeHead::Done);
        self.advance(index);
        match head {
            MergeHead::Row(key, value) => Some(Ok((key, value))),
            _ => unreachable!("best index always holds a row"),
        }
    }
}

impl crate::backend::StorageBackend for RedbBackend {
    fn load(&self) -> Result<crate::backend::LoadedState, String> {
        Ok(RedbBackend::load(self)?.into())
    }

    fn apply(&self, ops: &[Op], commit_index: u64) -> Result<(), String> {
        self.apply(ops, commit_index)
    }

    fn load_watermark(&self) -> Result<(u64, u64), String> {
        // Read only LAST_APPLIED + GC_WATERMARK: startup must not
        // materialize the dataset to restore the durable LSN and GC floor.
        let tx = self.begin_read()?;
        let data = tx.open_table(DATA).map_err(|e| e.to_string())?;
        let lsn = match data.get(LAST_APPLIED_KEY).map_err(|e| e.to_string())? {
            Some(row) => current_value(row.value())
                .map_err(|e| e.to_string())
                .and_then(crate::wal::decode_last_applied)
                .map_err(|error| format!("corrupt last-applied watermark: {error}"))?,
            None => 0,
        };
        drop(data);
        let meta = tx.open_table(META).map_err(|e| e.to_string())?;
        let floor = meta
            .get(GC_WATERMARK)
            .map_err(|e| e.to_string())?
            .map_or(0, |v| v.value());
        Ok((lsn, floor))
    }

    fn apply_many(&self, batches: &[(u64, &[Op])]) -> Result<(), String> {
        self.apply_many(batches)
    }

    fn retire_before(&self, watermark: u64) -> Result<(), String> {
        self.retire_before(watermark)
    }

    fn retire_before_batch(
        &self,
        watermark: u64,
        cursor: Option<Vec<u8>>,
        max_keys: usize,
    ) -> Result<Option<Vec<u8>>, String> {
        self.retire_before_batch(watermark, cursor, max_keys)
    }

    fn begin_shutdown(&self) {
        self.begin_shutdown();
    }

    fn checkpoint(&self, durable_lsn: u64) -> Result<(), String> {
        self.checkpoint(durable_lsn)
    }

    fn compact(&self) -> Result<(), String> {
        self.compact()
    }

    fn while_writes_paused(&self, run: &mut dyn FnMut()) -> Result<(), String> {
        // redb runs one write transaction at a time: holding one open (and
        // aborting it) keeps every other writer out while readers continue.
        let db = self.lock_db()?;
        let paused = db.begin_write().map_err(|e| e.to_string())?;
        let _hold = self.writer_hold(crate::WriterHolder::Pause);
        run();
        paused.abort().map_err(|e| e.to_string())
    }

    fn storage_stats(&self) -> Result<StorageStatistics, String> {
        self.storage_stats()
    }

    fn read_view(&self) -> Result<Box<dyn crate::backend::StorageReadView>, StorageError> {
        Ok(Box::new(self.read_view()?))
    }

    fn lock_times(&self) -> crate::BackendLockTimes {
        crate::BackendLockTimes {
            wait_ns: self.mutex_wait_ns.load(Ordering::Relaxed),
            hold_ns: self.mutex_hold_ns.load(Ordering::Relaxed),
            hold_max_ns: self.mutex_hold_max_ns.load(Ordering::Relaxed),
        }
    }

    fn writer_holds(&self) -> Vec<(crate::WriterHolder, crate::WriterHolds)> {
        self.writer_holds.snapshot()
    }
}

impl From<RedbState> for crate::backend::LoadedState {
    fn from(state: RedbState) -> Self {
        crate::backend::LoadedState {
            data: state.data,
            created: state.created,
            history: state.history,
            gc_watermark: state.gc_watermark,
        }
    }
}

impl RedbBackend {
    pub(crate) fn read_view(&self) -> Result<RedbReadView, StorageError> {
        let CountedRead { tx, live } = self.begin_read().map_err(read_error)?;
        Ok(RedbReadView {
            data: std::sync::OnceLock::new(),
            tx,
            _live: live,
        })
    }

    /// Open a counted read transaction (see [`LiveReads`]). The shared
    /// database lock is released on return; the transaction keeps its
    /// snapshot.
    fn begin_read(&self) -> Result<CountedRead, String> {
        let db = self.lock_db()?;
        let live = self.live_reads.enter();
        let tx = db.begin_read().map_err(|e| e.to_string())?;
        Ok(CountedRead { tx, live })
    }
    pub(crate) fn open(
        path: &Path,
        cache: crate::resources::ByteReservation,
    ) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Database::builder()
            .set_cache_size(cache.bytes())
            .create(path)
            .map_err(|e| e.to_string())?;
        let mut tx = db.begin_write().map_err(|e| e.to_string())?;
        // Quick repair on the first durable commit of this open: until
        // the first checkpoint it is the newest commit with allocator state,
        // and without it a crash between open and that checkpoint forced a
        // full repair of the whole file (10.9 s at 2M records / 7 GB).
        tx.set_quick_repair(true);
        tx.open_table(DATA).map_err(|e| e.to_string())?;
        tx.open_table(HISTORY).map_err(|e| e.to_string())?;
        tx.open_table(HISTORY_BLOCK).map_err(|e| e.to_string())?;
        tx.open_table(HISTORY_EXPIRY).map_err(|e| e.to_string())?;
        tx.open_table(META).map_err(|e| e.to_string())?;
        tx.open_table(CHECKPOINT_TABLE).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        let backend = Self {
            db: RwLock::new(db),
            _cache_reservation: cache,
            shard_id: shard_id_from_path(path)?,
            abort: Arc::new(AtomicBool::new(false)),
            mutex_wait_ns: AtomicU64::new(0),
            mutex_hold_ns: AtomicU64::new(0),
            mutex_hold_max_ns: AtomicU64::new(0),
            writer_holds: WriterHoldCounters::default(),
            #[cfg(test)]
            fail_next_apply: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_retire: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_compact: AtomicBool::new(false),
            live_reads: Arc::default(),
        };
        Ok(backend)
    }

    /// Acquire the redb database lock in **shared** mode (every read and write
    /// transaction), recording wait and hold durations for the RM-7
    /// measurement. Hold time is recorded when the returned guard drops.
    fn lock_db(&self) -> Result<TimedDbGuard<'_, RwLockReadGuard<'_, Database>>, String> {
        let wait_start = std::time::Instant::now();
        let inner = self
            .db
            .read()
            .map_err(|_| "redb lock poisoned".to_string())?;
        Ok(self.timed(inner, wait_start))
    }

    /// Acquire the redb database lock in **exclusive** mode. Only physical
    /// compaction needs `&mut Database`; nothing else may use this.
    fn lock_db_exclusive(
        &self,
    ) -> Result<TimedDbGuard<'_, RwLockWriteGuard<'_, Database>>, String> {
        let wait_start = std::time::Instant::now();
        let inner = self
            .db
            .write()
            .map_err(|_| "redb lock poisoned".to_string())?;
        Ok(self.timed(inner, wait_start))
    }

    /// Start timing one write transaction of `holder`. Declared right after
    /// `begin_write` returns, so it covers the work and the commit (the
    /// holder's own hold), not the wait for another writer.
    fn writer_hold(&self, holder: crate::WriterHolder) -> WriterHold<'_> {
        WriterHold {
            counters: &self.writer_holds,
            holder,
            started: std::time::Instant::now(),
        }
    }

    fn timed<G>(&self, inner: G, wait_start: std::time::Instant) -> TimedDbGuard<'_, G> {
        let waited = u64::try_from(wait_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.mutex_wait_ns.fetch_add(waited, Ordering::Relaxed);
        TimedDbGuard {
            inner,
            held_at: std::time::Instant::now(),
            hold_ns: &self.mutex_hold_ns,
            hold_max_ns: &self.mutex_hold_max_ns,
        }
    }

    pub(crate) fn load(&self) -> Result<RedbState, String> {
        // A redb read transaction owns its snapshot independently of the
        // `Database` handle. Release the backend mutex before reconstructing
        // the full in-memory state so startup/recovery scans do not block
        // unrelated readers or writers for their entire duration.
        let tx = self.begin_read()?;

        let mut data = BTreeMap::new();
        let mut created = BTreeMap::new();
        {
            let table = tx.open_table(DATA).map_err(|e| e.to_string())?;
            for entry in table.iter().map_err(|e| e.to_string())? {
                let (key, row) = entry.map_err(|e| e.to_string())?;
                let (from_ci, value) = decode_current(row.value()).map_err(|e| e.to_string())?;
                data.insert(key.value().to_vec(), value.to_vec());
                created.insert(key.value().to_vec(), from_ci);
            }
        }
        let history = {
            let table = tx.open_table(HISTORY).map_err(|e| e.to_string())?;
            let blocks = tx.open_table(HISTORY_BLOCK).map_err(|e| e.to_string())?;
            let heads: Vec<Vec<u8>> = table
                .iter()
                .map_err(|e| e.to_string())?
                .map(|entry| {
                    entry
                        .map(|(key, _)| key.value().to_vec())
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<_, _>>()?;
            let mut out = BTreeMap::new();
            for key in heads {
                let mut versions = Vec::new();
                let mut next: Option<u64> = None;
                loop {
                    let raw = match next {
                        None => table
                            .get(key.as_slice())
                            .map_err(|e| e.to_string())?
                            .map(|value| value.value().to_vec()),
                        Some(seq) => blocks
                            .get(history_v2::block_key(&key, seq).as_slice())
                            .map_err(|e| e.to_string())?
                            .map(|value| value.value().to_vec()),
                    };
                    let Some(raw) = raw else { break };
                    let (prev, block) = history_v2::decode_block(&raw)?;
                    versions.extend(block);
                    if prev == 0 {
                        break;
                    }
                    next = Some(prev);
                }
                out.insert(key, versions);
            }
            out
        };
        let gc_watermark = tx
            .open_table(META)
            .map_err(|e| e.to_string())?
            .get(GC_WATERMARK)
            .map_err(|e| e.to_string())?
            .map_or(0, |value| value.value());
        // A published checkpoint (framed, crc32-verified) carries the same
        // floor; take the max so a torn older row never rewinds the floor.
        let checkpoint_floor = tx
            .open_table(CHECKPOINT_TABLE)
            .map_err(|e| e.to_string())?
            .get(CHECKPOINT_KEY)
            .map_err(|e| e.to_string())?
            .map(|value| {
                crate::wal::decode_checkpoint(value.value())
                    .map(|checkpoint| checkpoint.gc_watermark)
                    .map_err(|error| format!("corrupt GC checkpoint: {error}"))
            })
            .transpose()?
            .unwrap_or(0);
        let gc_watermark = gc_watermark.max(checkpoint_floor);

        Ok(RedbState {
            data,
            created,
            history,
            gc_watermark,
        })
    }

    /// Apply a logical `RushDB` operation set and its MVCC metadata in one
    /// redb transaction. `commit()` is the durable acknowledgement boundary.
    pub(crate) fn apply(&self, ops: &[Op], commit_index: u64) -> Result<(), String> {
        #[cfg(test)]
        if self.fail_next_apply.swap(false, Ordering::SeqCst) {
            return Err("injected backend apply failure".into());
        }
        let db = self.lock_db()?;
        let mut tx = db.begin_write().map_err(|e| e.to_string())?;
        let _hold = self.writer_hold(crate::WriterHolder::Publish);
        // Log-first: the WAL is the durable truth, so a redb commit is a
        // buffered materialization write. Durability is forced by the periodic
        // `checkpoint()`; recovery replays any WAL tail beyond the last
        // checkpoint.
        tx.set_durability(redb::Durability::None)
            .map_err(|e| e.to_string())?;
        apply_in_txn(&tx, ops, commit_index)?;
        tx.commit().map_err(|e| e.to_string())
    }

    /// Apply many commits' op-sets in ONE redb write transaction. Commit
    /// indices must be supplied in ascending order; a single `commit()`
    /// (one fsync when durability is `Local`) durably acknowledges the whole
    /// batch. Used by the group-commit publisher so N concurrent commits
    /// coalesce into one fsync instead of N.
    pub(crate) fn apply_many(&self, batches: &[(u64, &[Op])]) -> Result<(), String> {
        #[cfg(test)]
        if self.fail_next_apply.swap(false, Ordering::SeqCst) {
            return Err("injected backend apply failure".into());
        }
        let db = self.lock_db()?;
        let mut tx = db.begin_write().map_err(|e| e.to_string())?;
        let _hold = self.writer_hold(crate::WriterHolder::Publish);
        // Log-first: the WAL is the durable truth, so a redb commit is a
        // buffered materialization write. Durability is forced by the periodic
        // `checkpoint()`; recovery replays any WAL tail beyond the last
        // checkpoint.
        tx.set_durability(redb::Durability::None)
            .map_err(|e| e.to_string())?;
        for (commit_index, ops) in batches {
            apply_in_txn(&tx, ops, *commit_index)?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub(crate) fn retire_before(&self, watermark: u64) -> Result<(), String> {
        #[cfg(test)]
        if self.fail_next_retire.swap(false, Ordering::SeqCst) {
            return Err("injected backend retire failure".into());
        }
        // Drive the bounded, resumable GC to completion. Each call processes a
        // bounded number of history keys in its own redb transaction, so the
        // durable reclamation is incremental rather than one monolithic txn.
        let mut cursor: Option<Vec<u8>> = None;
        loop {
            cursor = self.retire_before_batch(watermark, cursor, 128)?;
            if cursor.is_none() {
                return Ok(());
            }
        }
    }

    /// Reclaim a bounded batch of history superseded before `watermark`.
    ///
    /// Walks the expiry index, whose keys are `to_ci_be || data_key`, so
    /// the range bounded by `to_ci <= watermark` contains exactly the expired
    /// versions. Unexpired history keys are never visited, and each batch is one
    /// redb write transaction. `cursor` resumes after the last expiry key
    /// (exclusive). Returns the next key to resume from, or `None` when no
    /// expired version remains.
    pub(crate) fn retire_before_batch(
        &self,
        watermark: u64,
        cursor: Option<Vec<u8>>,
        max_keys: usize,
    ) -> Result<Option<Vec<u8>>, String> {
        self.retire_before_batch_within(watermark, cursor, max_keys, HISTORY_GC_BATCH_MAX_HOLD)
    }

    /// [`Self::retire_before_batch`], ending the batch after `max_hold` of
    /// work as well as after `max_keys` keys (at least one key per batch).
    fn retire_before_batch_within(
        &self,
        watermark: u64,
        cursor: Option<Vec<u8>>,
        max_keys: usize,
        max_hold: std::time::Duration,
    ) -> Result<Option<Vec<u8>>, String> {
        let started = std::time::Instant::now();
        let db = self.lock_db()?;
        let tx = db.begin_write().map_err(|e| e.to_string())?;
        let _hold = self.writer_hold(crate::WriterHolder::HistoryGc);
        {
            let mut history = tx.open_table(HISTORY).map_err(|e| e.to_string())?;
            let mut blocks = tx.open_table(HISTORY_BLOCK).map_err(|e| e.to_string())?;
            let mut expiry = tx.open_table(HISTORY_EXPIRY).map_err(|e| e.to_string())?;
            let upper_bound = history_v2::expiry_upper_bound(watermark);
            let mut resume = cursor;
            let mut processed = 0usize;
            let mut next: Option<Vec<u8>> = None;
            loop {
                let lower: Bound<&[u8]> = match resume.as_deref() {
                    Some(k) => Bound::Excluded(k),
                    None => Bound::Unbounded,
                };
                let upper: Bound<&[u8]> = match upper_bound.as_deref() {
                    Some(k) => Bound::Excluded(k),
                    None => Bound::Unbounded,
                };
                // Confine the range guards to this block so `expiry` is free to
                // mutate below (redb guards borrow the table).
                let next_key = {
                    let mut range = expiry
                        .range::<&[u8]>((lower, upper))
                        .map_err(|e| e.to_string())?;
                    range
                        .next()
                        .transpose()
                        .map_err(|e| e.to_string())?
                        .map(|(key, _value)| key.value().to_vec())
                };
                let Some(expiry_key) = next_key else {
                    break;
                };
                let data_key = history_v2::expiry_data_key(&expiry_key)
                    .ok_or("corrupt expiry key")?
                    .to_vec();
                // Reclaim every expired version for this data key at once (a
                // sibling version may expire in the same window), removing
                // each sibling's expiry row in one chain rebuild.
                prune_history(&mut history, &mut blocks, &mut expiry, &data_key, watermark)?;
                // Orphan expiry row (history already gone) or the current row:
                // removing is idempotent.
                expiry
                    .remove(expiry_key.as_slice())
                    .map_err(|e| e.to_string())?;
                processed += 1;
                resume = Some(expiry_key.clone());
                if processed >= max_keys || started.elapsed() >= max_hold {
                    next = Some(expiry_key);
                    break;
                }
            }
            let completed = next.is_none();
            drop(expiry);
            drop(blocks);
            drop(history);
            if completed {
                // Completed the full walk: advance the durable GC watermark and
                // publish a checkpoint from the SAME transaction (the LSN read
                // here is the shard's durable watermark, consistent with the
                // reclaimed history).
                let data = tx.open_table(DATA).map_err(|e| e.to_string())?;
                let durable_lsn = match data.get(LAST_APPLIED_KEY).map_err(|e| e.to_string())? {
                    Some(row) => current_value(row.value())
                        .map_err(|e| e.to_string())
                        .and_then(crate::wal::decode_last_applied)
                        .map_err(|error| format!("corrupt last-applied watermark: {error}"))?,
                    None => 0,
                };
                drop(data);
                let mut meta = tx.open_table(META).map_err(|e| e.to_string())?;
                meta.insert(GC_WATERMARK, &watermark)
                    .map_err(|e| e.to_string())?;
                let checkpoint = crate::wal::Checkpoint {
                    shard_id: self.shard_id,
                    durable_lsn,
                    gc_watermark: watermark,
                    published_at_ms: now_ms(),
                };
                let encoded = crate::wal::encode_checkpoint(&checkpoint);
                drop(meta);
                let mut cp = tx.open_table(CHECKPOINT_TABLE).map_err(|e| e.to_string())?;
                cp.insert(CHECKPOINT_KEY, encoded.as_slice())
                    .map_err(|e| e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())?;
            Ok(next)
        }
    }

    /// Exact per-family key+value bytes from one read view. Excludes redb's
    /// copy-on-write slack; compare with the file length.
    ///
    /// The read transaction owns its snapshot, so the shard mutex is released
    /// immediately after it is opened: holding it for the whole scan would
    /// block every read and write for the scan's duration (minutes on a
    /// multi-GB store).
    pub(crate) fn storage_stats(&self) -> Result<StorageStatistics, String> {
        let tx = self.begin_read()?;
        // Counts and bytes come from redb's own B-tree metadata (`len`,
        // `stats().stored_bytes()`), not a row-by-row walk of the keyspace: a
        // full scan of a multi-GB file evicted the page cache and competed
        // with the writer (C2, cloud-capacity F2). Only the small receipt
        // family is ranged, to split it out of the data table.
        let mut stats = StorageStatistics::default();
        {
            let table = tx.open_table(DATA).map_err(|e| e.to_string())?;
            let (start, end) = crate::prefix_range(crate::receipt::RECEIPT_FAMILY);
            for row in table
                .range::<&[u8]>(start.as_slice()..end.as_slice())
                .map_err(|e| e.to_string())?
            {
                if self.abort.load(Ordering::Relaxed) {
                    return Err("maintenance cancelled".into());
                }
                let (key, value) = row.map_err(|e| e.to_string())?;
                stats.receipt_keys += 1;
                stats.receipt_bytes += (key.value().len() + value.value().len()) as u64;
            }
            let (keys, bytes) = table_size(&table)?;
            stats.data_keys = keys.saturating_sub(stats.receipt_keys);
            stats.data_bytes = bytes.saturating_sub(stats.receipt_bytes);
        }
        // Sealed history blocks and the expiry index exist solely because of
        // retained history; fold them into the history family so the
        // accounting stays complete (file >= data + history + log + payload).
        for definition in [HISTORY, HISTORY_BLOCK, HISTORY_EXPIRY] {
            let table = tx.open_table(definition).map_err(|e| e.to_string())?;
            let (keys, bytes) = table_size(&table)?;
            stats.history_keys += keys;
            stats.history_bytes += bytes;
        }
        Ok(stats)
    }

    /// Signal in-flight maintenance scans to stop within a batch (shutdown).
    pub(crate) fn begin_shutdown(&self) {
        self.abort.store(true, Ordering::Relaxed);
    }

    /// Force an fsync of every buffered materialization write and durably
    /// record `durable_lsn` as this shard's checkpoint. Under log-first the WAL
    /// is the durable truth; this is what advances the recoverable
    /// materialization and allows WAL segments at or below `durable_lsn` to be
    /// truncated.
    pub(crate) fn checkpoint(&self, durable_lsn: u64) -> Result<(), String> {
        let db = self.lock_db()?;
        let mut tx = db.begin_write().map_err(|e| e.to_string())?;
        let _hold = self.writer_hold(crate::WriterHolder::Checkpoint);
        tx.set_durability(redb::Durability::Immediate)
            .map_err(|e| e.to_string())?;
        // Quick repair: persist the allocator state with every durable
        // checkpoint. Crash recovery rolls back to the last durable commit, so
        // reopening after a kill is O(1) instead of a full checksum and
        // allocator walk of the file (9 s at 9 GB measured; cloud-capacity
        // F5). The cost is paid once per checkpoint, not per commit.
        tx.set_quick_repair(true);
        {
            let meta = tx.open_table(META).map_err(|e| e.to_string())?;
            let gc_watermark = meta
                .get(GC_WATERMARK)
                .map_err(|e| e.to_string())?
                .map_or(0, |value| value.value());
            drop(meta);
            let checkpoint = crate::wal::Checkpoint {
                shard_id: self.shard_id,
                durable_lsn,
                gc_watermark,
                published_at_ms: now_ms(),
            };
            let encoded = crate::wal::encode_checkpoint(&checkpoint);
            let mut table = tx.open_table(CHECKPOINT_TABLE).map_err(|e| e.to_string())?;
            table
                .insert(CHECKPOINT_KEY, encoded.as_slice())
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub(crate) fn compact(&self) -> Result<(), String> {
        #[cfg(test)]
        if self.fail_next_compact.swap(false, Ordering::SeqCst) {
            return Err("injected backend compact failure".into());
        }
        let mut db = self.lock_db_exclusive()?;
        // Never wait here: a reader may need this lock (or the store's
        // publish lock, held by the caller) before it lets go, so waiting
        // would stall it (a 2.3 s ingest stall, measured). The store
        // retries with every lock released.
        if !self.live_reads.is_drained() {
            return Err(crate::backend::COMPACT_BUSY.into());
        }
        while db.compact().map_err(|e| e.to_string())? {}
        Ok(())
    }
}

/// Row count and stored key+value bytes of one table, from redb metadata.
fn table_size<T: ReadableTableMetadata>(table: &T) -> Result<(u64, u64), String> {
    let keys = table.len().map_err(|e| e.to_string())?;
    let bytes = table.stats().map_err(|e| e.to_string())?.stored_bytes();
    Ok((keys, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::StorageBackend;
    use crate::resources::ByteBudget;

    #[test]
    fn shard_path_parser_rejects_malformed_suffixes() {
        assert_eq!(
            shard_id_from_path(std::path::Path::new("rushdb.redb")).unwrap(),
            0
        );
        assert_eq!(
            shard_id_from_path(std::path::Path::new("rushdb-7.redb")).unwrap(),
            7
        );
        assert!(shard_id_from_path(std::path::Path::new("rushdb-main.redb")).is_err());
    }

    #[test]
    fn malformed_last_applied_watermark_is_not_treated_as_zero() {
        let (dir, backend) = open_backend("corrupt-watermark");
        {
            let db = backend.db.read().unwrap();
            let tx = db.begin_write().unwrap();
            {
                let mut data = tx.open_table(DATA).unwrap();
                data.insert(LAST_APPLIED_KEY, &[0xff, 0x00][..]).unwrap();
            }
            tx.commit().unwrap();
        }
        let error = backend.load_watermark().unwrap_err();
        assert!(error.contains("corrupt last-applied watermark"));
        drop(backend);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_gc_checkpoint_is_not_treated_as_zero() {
        let (dir, backend) = open_backend("corrupt-checkpoint");
        {
            let db = backend.db.read().unwrap();
            let tx = db.begin_write().unwrap();
            {
                let mut table = tx.open_table(CHECKPOINT_TABLE).unwrap();
                table.insert(CHECKPOINT_KEY, &[0xff, 0x00][..]).unwrap();
            }
            tx.commit().unwrap();
        }
        let Err(error) = RedbBackend::load(&backend) else {
            panic!("corrupt checkpoint must fail closed")
        };
        assert!(error.contains("corrupt GC checkpoint"));
        drop(backend);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn compaction_reports_busy_while_a_read_transaction_is_open() {
        // redb refuses to compact while any read transaction is open, and a
        // view outlives the database lock it was opened under. The backend
        // reports that as busy at once, never waiting (a reader may need the
        // lock again before it lets go: a 2.3 s ingest stall, measured); the
        // store retries with its locks released.
        let (dir, backend) = open_backend("compact-busy");
        backend
            .apply(&[Op::Put(b"k".to_vec(), vec![7; 4096])], 1)
            .unwrap();
        let view = backend.read_view().unwrap();
        let started = std::time::Instant::now();
        assert_eq!(backend.compact().unwrap_err(), crate::backend::COMPACT_BUSY);
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        // The reader can still write while compaction was refused.
        backend
            .apply(&[Op::Put(b"k2".to_vec(), b"v".to_vec())], 2)
            .unwrap();
        drop(view);
        backend.compact().unwrap();
        drop(backend);
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn open_backend(name: &str) -> (std::path::PathBuf, RedbBackend) {
        let dir = std::env::temp_dir().join(format!(
            "rushdb-expiry-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rushdb.redb");
        let budget = ByteBudget::new("test", 8 * 1024 * 1024).unwrap();
        let cache = budget.try_reserve(8 * 1024 * 1024).unwrap();
        let backend = RedbBackend::open(&path, cache).unwrap();
        (dir, backend)
    }

    #[test]
    fn crash_before_first_checkpoint_needs_no_full_repair() {
        // A process that opens the store and dies before its first
        // checkpoint must leave allocator state behind (quick repair), or the
        // next open repairs the whole file (10.9 s at 2M records).
        const CHILD: &str = "RUSHDB_REDB_CRASH_PATH";
        if let Ok(path) = std::env::var(CHILD) {
            let budget = ByteBudget::new("test", 8 * 1024 * 1024).unwrap();
            let backend = RedbBackend::open(
                Path::new(&path),
                budget.try_reserve(8 * 1024 * 1024).unwrap(),
            )
            .unwrap();
            std::mem::forget(backend);
            std::process::abort();
        }
        let dir = crate::TempPath::dir("redb-crash");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rushdb.redb");
        // A first, clean open creates the file (the state after a clean stop).
        {
            let budget = ByteBudget::new("test", 8 * 1024 * 1024).unwrap();
            let backend =
                RedbBackend::open(&path, budget.try_reserve(8 * 1024 * 1024).unwrap()).unwrap();
            backend
                .apply(&[Op::Put(b"k".to_vec(), b"v".to_vec())], 1)
                .unwrap();
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "redb_backend::tests::crash_before_first_checkpoint_needs_no_full_repair",
                "--nocapture",
            ])
            .env(CHILD, &path)
            .status()
            .unwrap();
        assert!(!status.success(), "the child must crash");
        let repaired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&repaired);
        let db = redb::Database::builder()
            .set_repair_callback(move |_| flag.store(true, Ordering::SeqCst))
            .open(&path)
            .unwrap();
        drop(db);
        assert!(
            !repaired.load(Ordering::SeqCst),
            "a crash between open and the first checkpoint must not force a full repair"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn backend_mutex_durations_are_measured() {
        // RM-7: wait/hold durations are recorded and surfaced.
        let (dir, backend) = open_backend("mutex-metrics");
        backend
            .apply(&[Op::Put(b"k".to_vec(), b"v".to_vec())], 1)
            .unwrap();
        assert!(backend.mutex_hold_ns.load(Ordering::Relaxed) > 0);
        assert!(backend.mutex_hold_max_ns.load(Ordering::Relaxed) > 0);
        let times = crate::backend::StorageBackend::lock_times(&backend);
        // Cumulative hold is always at least the single maximum hold.
        assert!(times.hold_ns >= times.hold_max_ns && times.hold_max_ns > 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_history_gc_batch_ends_at_its_hold_bound() {
        // SCALE-2: a batch stops after its time bound even when keys remain,
        // and resumes from where it stopped.
        let (dir, backend) = open_backend("gc-hold-bound");
        for ci in 1..=4u64 {
            let ops: Vec<Op> = (0..8u8)
                .map(|key| Op::Put(vec![key], ci.to_be_bytes().to_vec()))
                .collect();
            backend.apply(&ops, ci).unwrap();
        }
        let cursor = backend
            .retire_before_batch_within(4, None, 512, std::time::Duration::ZERO)
            .unwrap();
        assert!(
            cursor.is_some(),
            "a zero bound must end the batch after one key"
        );
        let mut batches = 1;
        let mut cursor = cursor;
        while cursor.is_some() {
            cursor = backend
                .retire_before_batch_within(4, cursor, 512, std::time::Duration::ZERO)
                .unwrap();
            batches += 1;
        }
        // One key per batch: the 8 keys' expired versions, plus the batch
        // that finds none left.
        assert_eq!(batches, 9);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_writer_hold_is_counted_under_its_holder() {
        // SCALE-2: each write transaction is attributed to what held the
        // writer, so the longest holds can be told apart.
        let (dir, backend) = open_backend("writer-holds");
        backend
            .apply(&[Op::Put(b"k".to_vec(), b"v".to_vec())], 1)
            .unwrap();
        backend
            .apply_many(&[(2, &[Op::Put(b"k".to_vec(), b"w".to_vec())][..])])
            .unwrap();
        backend.checkpoint(2).unwrap();
        backend.retire_before_batch(1, None, 16).unwrap();
        let holds: std::collections::HashMap<&str, crate::WriterHolds> =
            crate::backend::StorageBackend::writer_holds(&backend)
                .into_iter()
                .map(|(holder, holds)| (holder.label(), holds))
                .collect();
        let count = |label: &str| holds[label].counts.iter().sum::<u64>();
        assert_eq!(count("publish"), 2);
        assert_eq!(count("checkpoint"), 1);
        assert_eq!(count("history_gc"), 1);
        assert_eq!(count("pause"), 0);
        assert!(holds["checkpoint"].sum_seconds > 0.0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn read_view_does_not_wait_for_an_open_write_transaction() {
        // C1 / F3: the publisher holds the backend lock for its whole write
        // transaction (page-miss I/O included). Readers must not queue behind
        // it — redb serves them from MVCC.
        let (dir, backend) = open_backend("reader-vs-writer");
        backend
            .apply(&[Op::Put(b"k".to_vec(), b"before".to_vec())], 1)
            .unwrap();
        let backend = Arc::new(backend);
        let (opened_tx, opened_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let writer = {
            let backend = Arc::clone(&backend);
            std::thread::spawn(move || {
                let db = backend.lock_db().unwrap();
                let tx = db.begin_write().unwrap();
                opened_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                tx.commit().unwrap();
            })
        };
        opened_rx.recv().unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        {
            let backend = Arc::clone(&backend);
            std::thread::spawn(move || {
                let view = backend.read_view().unwrap();
                let value = view
                    .get_current_projected(b"k", ReadLimits::default(), ScanProjection::KeyValue)
                    .unwrap();
                done_tx.send(value).unwrap();
            });
        }
        let seen = done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("read_view blocked behind an open write transaction");
        assert_eq!(
            seen.as_deref(),
            Some(&b"before"[..]),
            "reader must observe the pre-transaction snapshot"
        );
        release_tx.send(()).unwrap();
        writer.join().unwrap();
        drop(backend);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn injected_backend_faults_surface_as_errors() {
        // RM-8: injected apply/retire/compact faults fail loudly and are
        // consumed (the following operation succeeds).
        let (dir, backend) = open_backend("fault-inject");
        backend.fail_next_apply.store(true, Ordering::SeqCst);
        assert!(backend
            .apply(&[Op::Put(b"k".to_vec(), b"v".to_vec())], 1)
            .is_err());
        backend
            .apply(&[Op::Put(b"k".to_vec(), b"v".to_vec())], 1)
            .unwrap();
        backend.fail_next_retire.store(true, Ordering::SeqCst);
        assert!(backend.retire_before(1).is_err());
        backend.fail_next_compact.store(true, Ordering::SeqCst);
        assert!(backend.compact().is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn expiry_rows(backend: &RedbBackend) -> usize {
        let db = backend.db.read().unwrap();
        let tx = db.begin_read().unwrap();
        let table = tx.open_table(HISTORY_EXPIRY).unwrap();
        table.iter().unwrap().count()
    }

    fn history_rows(backend: &RedbBackend) -> usize {
        let db = backend.db.read().unwrap();
        let tx = db.begin_read().unwrap();
        let table = tx.open_table(HISTORY).unwrap();
        table.iter().unwrap().count()
    }

    fn block_rows(backend: &RedbBackend) -> usize {
        let db = backend.db.read().unwrap();
        let tx = db.begin_read().unwrap();
        let table = tx.open_table(HISTORY_BLOCK).unwrap();
        table.iter().unwrap().count()
    }

    #[test]
    fn history_chain_appends_across_sealed_blocks() {
        let (dir, backend) = open_backend("chain");
        let n = history_v2::BLOCK_SIZE as u64 * 2 + 5;
        backend
            .apply(&[Op::Put(b"k".to_vec(), b"v0".to_vec())], 1)
            .unwrap();
        for i in 2..=n {
            backend
                .apply(&[Op::Put(b"k".to_vec(), format!("v{i}").into_bytes())], i)
                .unwrap();
        }
        assert_eq!(history_rows(&backend), 1, "one head row per key");
        assert!(block_rows(&backend) >= 1, "oldest versions are sealed");

        // Reads resolve versions across the head and the sealed chain.
        let view = backend.read_view().unwrap();
        let at = |ci| {
            view.get_at(b"k", ci, ReadLimits::default(), ScanProjection::KeyValue)
                .unwrap()
        };
        // Values are `v0` at ci 1, then `v{i}` at ci i.
        assert_eq!(at(n), Some(format!("v{n}").into_bytes()), "current");
        assert_eq!(at(1), Some(b"v0".to_vec()), "oldest (sealed)");
        assert_eq!(at(2), Some(b"v2".to_vec()), "in the head");
        assert_eq!(at(3), Some(b"v3".to_vec()));
        drop(view);

        // GC at the head watermark reclaims the whole chain.
        assert_eq!(backend.retire_before_batch(n, None, 100).unwrap(), None);
        assert_eq!(history_rows(&backend), 0);
        assert_eq!(block_rows(&backend), 0);
        assert_eq!(expiry_rows(&backend), 0);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn gc_skips_unexpired_history_entirely() {
        let (dir, backend) = open_backend("skip");
        for i in 0..5u64 {
            let key = format!("k{i}").into_bytes();
            backend
                .apply(&[Op::Put(key.clone(), b"v1".to_vec())], 2 * i + 1)
                .unwrap();
            backend
                .apply(&[Op::Put(key, b"v2".to_vec())], 2 * i + 2)
                .unwrap();
        }
        assert_eq!(history_rows(&backend), 5);

        // Nothing has expired at watermark 0: the expiry range is empty, so a
        // one-key batch finishes immediately instead of walking every history
        // key (the pre-index behavior).
        assert_eq!(backend.retire_before_batch(0, None, 1).unwrap(), None);
        assert_eq!(history_rows(&backend), 5, "nothing reclaimed");

        // A real watermark still reclaims everything, bounded by max_keys.
        let mut cursor = None;
        let mut batches = 0;
        while let Some(next) = backend.retire_before_batch(10, cursor, 1).unwrap() {
            cursor = Some(next);
            batches += 1;
            assert!(batches <= 10, "bounded batches must terminate");
        }
        assert!(batches >= 5, "one key per batch, got {batches}");
        assert_eq!(history_rows(&backend), 0);
        assert_eq!(expiry_rows(&backend), 0);
        std::fs::remove_dir_all(dir).ok();
    }
}
