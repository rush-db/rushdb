//! Online compaction of one redb file (#427): the file is rewritten while
//! reads and writes go on. A snapshot of it is copied into a new file
//! (copying writes only the live pages, so the copy is compact), the copy
//! catches up with the commits made since from the WAL, and then, under a
//! short exclusive hold of the database, takes the last few commits, is
//! made durable and replaces the file. Read views open on the old file
//! keep it until they end.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use super::{
    current_value, LiveReads, RedbBackend, CHECKPOINT_TABLE, DATA, HISTORY, HISTORY_BLOCK,
    HISTORY_EXPIRY, META,
};
use crate::backend::StorageBackend;
use crate::StorageError;

/// The copy's file beside the store file.
pub(crate) fn copy_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".compact");
    PathBuf::from(name)
}

/// The copy's page cache: it is written once and read by its catch-up only.
const COPY_CACHE_BYTES: usize = 16 * 1024 * 1024;

/// Rows copied per write transaction of the copy.
const COPY_BATCH_ROWS: usize = 50_000;

/// Commits the copy may lag behind when the swap starts: the swap applies
/// them while it holds the database.
const SWAP_LAG: u64 = 256;

/// Test hook: runs once the snapshot is copied, before the catch-up.
#[cfg(test)]
pub(crate) static AFTER_COPY: std::sync::Mutex<Option<Box<dyn FnMut() + Send>>> =
    std::sync::Mutex::new(None);

/// Catch-ups an online compaction ran (test observability).
#[cfg(test)]
pub(crate) static CATCH_UPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Catch the copy up with the WAL's commits of `shard` in `from..=to`.
pub(crate) type CatchUp<'a> =
    dyn FnMut(&dyn StorageBackend, u32, u64, u64) -> Result<(), StorageError> + 'a;

impl RedbBackend {
    /// Rewrite this backend's file compactly while it serves (see the
    /// module docs). `Ok(false)` when `abandon` asked to give the copy up
    /// (a write that bypasses the WAL happened, or a shutdown).
    pub(crate) fn compact_online(
        &self,
        catch_up: &mut CatchUp<'_>,
        abandon: &dyn Fn() -> bool,
    ) -> Result<bool, String> {
        let copy_path = copy_path(&self.path);
        remove_if_exists(&copy_path)?;
        let swapped = self.compact_into(&copy_path, catch_up, abandon);
        if !matches!(swapped, Ok(true)) {
            let _ = std::fs::remove_file(&copy_path);
        }
        swapped
    }

    fn compact_into(
        &self,
        copy_path: &Path,
        catch_up: &mut CatchUp<'_>,
        abandon: &dyn Fn() -> bool,
    ) -> Result<bool, String> {
        let cache = crate::resources::ByteBudget::new("compaction-copy", COPY_CACHE_BYTES)
            .and_then(|budget| budget.try_reserve(COPY_CACHE_BYTES))
            .map_err(|e| e.to_string())?;
        let copy = RedbBackend::open_as_shard(copy_path, cache, self.shard_id)?;
        let mut applied = {
            let read = self.begin_read()?;
            let applied = applied_lsn(&read.tx)?;
            let target = copy.lock_db()?;
            copy_tables(&read.tx, &target, &self.abort)?;
            applied
        };
        #[cfg(test)]
        if let Some(hook) = AFTER_COPY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
        {
            hook();
        }
        loop {
            if abandon() || self.abort.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let now = self.applied()?;
            if now.saturating_sub(applied) <= SWAP_LAG {
                break;
            }
            catch_up(&copy, self.shard_id, applied + 1, now).map_err(|e| e.to_string())?;
            #[cfg(test)]
            CATCH_UPS.fetch_add(1, Ordering::SeqCst);
            applied = now;
        }
        #[cfg(test)]
        crate::kill_point::hit("online-compact-before-swap");
        // The swap: no read or write of this backend starts until it ends.
        let mut db = self.lock_db_exclusive()?;
        let now = {
            let read = db.begin_read().map_err(|e| e.to_string())?;
            applied_lsn(&read)?
        };
        if now > applied {
            catch_up(&copy, self.shard_id, applied + 1, now).map_err(|e| e.to_string())?;
            #[cfg(test)]
            CATCH_UPS.fetch_add(1, Ordering::SeqCst);
        }
        if abandon() {
            return Ok(false);
        }
        copy.make_durable()?;
        drop(copy);
        std::fs::rename(copy_path, &self.path).map_err(|e| e.to_string())?;
        sync_parent(&self.path)?;
        #[cfg(test)]
        crate::kill_point::hit("online-compact-renamed");
        let (fresh, _) = Self::build_database(&self.path, self.cache_reservation.bytes())?;
        let old = std::mem::replace(&mut *db, fresh);
        let old_reads = std::mem::take(
            &mut *self
                .live_reads
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        self.after_file_swap();
        drop(db);
        retire(old, old_reads, Arc::clone(&self.abort));
        Ok(true)
    }

    /// The applied commit index of the current file.
    fn applied(&self) -> Result<u64, String> {
        let read = self.begin_read()?;
        applied_lsn(&read.tx)
    }

    /// Commit an empty durable write with allocator state, so the file is
    /// whole on disk and opens without a repair.
    /// After [`Self::compact_online`] swapped in a file that is durable as
    /// a whole: nothing is buffered, and the next checkpoint is written
    /// whatever it finds.
    fn after_file_swap(&self) {
        self.unsynced.store(false, Ordering::SeqCst);
        self.buffered_bytes.store(0, Ordering::SeqCst);
        self.gc_batches_buffered.store(0, Ordering::SeqCst);
        self.flush_claimed.store(false, Ordering::SeqCst);
        self.quick_repair_due.store(false, Ordering::SeqCst);
        self.checkpointed_lsn.store(u64::MAX, Ordering::SeqCst);
    }

    fn make_durable(&self) -> Result<(), String> {
        let db = self.lock_db()?;
        let mut tx = db.begin_write().map_err(|e| e.to_string())?;
        tx.set_durability(redb::Durability::Immediate)
            .map_err(|e| e.to_string())?;
        tx.set_quick_repair(true);
        tx.commit().map_err(|e| e.to_string())
    }
}

/// The applied commit index in `tx`'s snapshot.
fn applied_lsn(tx: &redb::ReadTransaction) -> Result<u64, String> {
    let data = tx.open_table(DATA).map_err(|e| e.to_string())?;
    Ok(
        match data
            .get(crate::LAST_APPLIED_KEY)
            .map_err(|e| e.to_string())?
        {
            Some(row) => current_value(row.value())
                .map_err(|e| e.to_string())
                .and_then(crate::wal::decode_last_applied)
                .map_err(|error| format!("corrupt last-applied watermark: {error}"))?,
            None => 0,
        },
    )
}

/// Copy every table of `from` into `to`, in batches; a shutdown stops it.
fn copy_tables(
    from: &redb::ReadTransaction,
    to: &Database,
    abort: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    for table in [
        DATA,
        HISTORY,
        HISTORY_BLOCK,
        HISTORY_EXPIRY,
        CHECKPOINT_TABLE,
    ] {
        copy_table(from, to, table, abort)?;
    }
    let source = from.open_table(META).map_err(|e| e.to_string())?;
    let tx = to.begin_write().map_err(|e| e.to_string())?;
    {
        let mut target = tx.open_table(META).map_err(|e| e.to_string())?;
        for row in source.iter().map_err(|e| e.to_string())? {
            let (key, value) = row.map_err(|e| e.to_string())?;
            target
                .insert(key.value(), value.value())
                .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

fn copy_table(
    from: &redb::ReadTransaction,
    to: &Database,
    table: TableDefinition<&[u8], &[u8]>,
    abort: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    let source = from.open_table(table).map_err(|e| e.to_string())?;
    let mut rows = source.iter().map_err(|e| e.to_string())?.peekable();
    while rows.peek().is_some() {
        if abort.load(Ordering::Relaxed) {
            return Err("shutting down".into());
        }
        let mut tx = to.begin_write().map_err(|e| e.to_string())?;
        // Buffered: the copy is made durable once, before the swap.
        tx.set_durability(redb::Durability::None)
            .map_err(|e| e.to_string())?;
        {
            let mut target = tx.open_table(table).map_err(|e| e.to_string())?;
            for row in rows.by_ref().take(COPY_BATCH_ROWS) {
                let (key, value) = row.map_err(|e| e.to_string())?;
                target
                    .insert(key.value(), value.value())
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Keep the replaced file's database until the read views on it end, then
/// close it (a close writes into the file, which no view may still read).
fn retire(old: Database, reads: Arc<LiveReads>, abort: Arc<std::sync::atomic::AtomicBool>) {
    let wait = move || {
        while !reads.is_drained() && !abort.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        drop(old);
    };
    if std::thread::Builder::new()
        .name("rushdb-compact-retire".into())
        .spawn(wait)
        .is_err()
    {
        tracing::warn!("could not start the thread that closes a compacted-away file");
    }
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Make a rename in `path`'s directory durable.
fn sync_parent(path: &Path) -> Result<(), String> {
    match path.parent() {
        #[cfg(unix)]
        Some(dir) => std::fs::File::open(dir)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| e.to_string()),
        _ => Ok(()),
    }
}
