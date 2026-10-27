//! Durable-log export: crash-log dumps and the streaming-replication
//! follower read path. Both derive the committed
//! command-batch sequence from the WAL's embedded-command envelope
//! rather than a separate journal.

use super::{prefix_range, wal, ReadLimits, ScanProjection, StorageError, Store, LOG_PREFIX};

impl Store {
    /// Durable-log export for tests/tools/backups.
    pub fn log_bytes(&self) -> Vec<Vec<u8>> {
        self.take_log_for_crash()
    }

    /// Export the decoded commit-batch payloads of the committed command log,
    /// ordered by LSN.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn take_log_for_crash(&self) -> Vec<Vec<u8>> {
        self.try_take_log_for_crash().unwrap_or_default()
    }

    /// Fallible crash-log export for recovery and archive callers. Unlike
    /// [`Store::take_log_for_crash`], this path never converts a scan,
    /// framing, or WAL decode failure into an empty/partial log.
    ///
    /// A durable store derives this from the WAL's embedded-command envelope
    ///; an in-memory store has no WAL file and keeps the row-based
    /// journal ([`log_key`], test mode only).
    ///
    /// # Errors
    /// Returns [`StorageError::CorruptLog`] for malformed WAL rows and the
    /// underlying storage error for failed backend scans.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn try_take_log_for_crash(&self) -> Result<Vec<Vec<u8>>, StorageError> {
        self.check_health()?;
        let _guard = self.health.guard(self.publish_lock.lock())?;
        if self.is_durable() {
            let records = {
                let guard = self.health.guard(self.log.lock())?;
                match guard.as_ref() {
                    Some(log) => log.read_from(1).map_err(|error| {
                        StorageError::Durability(format!("read WAL for crash log: {error}"))
                    })?,
                    None => Vec::new(),
                }
            };
            let mut out = Vec::with_capacity(records.len());
            // A chained commit (large-transaction support) spans
            // several physical records sharing one LSN; reassemble before
            // extracting its command, the same way WAL replay does.
            let mut assembler = wal::ChainAssembler::default();
            for (lsn, raw) in records {
                let (header, body) = match assembler
                    .feed(&raw, 0)
                    .map_err(|_| StorageError::CorruptLog)?
                {
                    wal::ChainFeed::Pending => continue,
                    wal::ChainFeed::Complete(header, body) => (header, body),
                };
                if header.lsn != lsn {
                    return Err(StorageError::CorruptLog);
                }
                let (_, _ops, command) = wal::decode_logical_body(header, &body)
                    .map_err(|_| StorageError::CorruptLog)?;
                if let Some(batch) = command {
                    out.push(rushdb_types::command_codec::encode(&batch));
                }
            }
            return Ok(out);
        }
        // Every store has at least one backend, so read the log rows
        // natively from each shard. Multi-shard: shards are scanned in shard
        // order, so the union must be re-sorted by the LSN carried in the WAL
        // frame's header.
        let mut out: Vec<(u64, Vec<u8>)> = Vec::new();
        for backend in self.all_backends() {
            let view = backend.read_view()?;
            let (ls, le) = prefix_range(LOG_PREFIX);
            let rows =
                view.current_range(&ls, &le, ReadLimits::default(), ScanProjection::KeyValue)?;
            for row in rows {
                let (_, value) = row?;
                let (header, payload) =
                    wal::decode_wal_record(&value).map_err(|_| StorageError::CorruptLog)?;
                out.push((header.lsn, payload.to_vec()));
            }
        }
        out.sort_by_key(|(lsn, _)| *lsn);
        Ok(out.into_iter().map(|(_, payload)| payload).collect())
    }

    /// Durable-log export of commit-batch payloads from `from_lsn` onward
    /// (each paired with its LSN), for a streaming-replication follower
    /// to replay via [`Store::apply_replicated_batch_with_payload`].
    /// Empty for a non-durable store. Takes the same `publish_lock` as
    /// [`Store::try_take_log_for_crash`] (briefly contends with commits —
    /// poll at a bounded interval, not a tight loop).
    ///
    /// # Errors
    /// Returns [`StorageError::CorruptLog`] for a malformed WAL record and
    /// the underlying storage error for a failed WAL read.
    pub fn try_log_commands_from(
        &self,
        from_lsn: u64,
    ) -> Result<Vec<(u64, Vec<u8>)>, StorageError> {
        self.check_health()?;
        if !self.is_durable() {
            return Ok(Vec::new());
        }
        let _guard = self.health.guard(self.publish_lock.lock())?;
        let records = {
            let guard = self.health.guard(self.log.lock())?;
            match guard.as_ref() {
                Some(log) => log.read_from(from_lsn).map_err(|error| {
                    StorageError::Durability(format!("read WAL for replication: {error}"))
                })?,
                None => Vec::new(),
            }
        };
        let mut out = Vec::with_capacity(records.len());
        // Same chain reassembly as `try_take_log_for_crash`: a replicated
        // commit too big for one WAL record arrives here as several
        // physical records sharing one LSN, and must be reconstructed
        // before its command is extracted and shipped to the follower —
        // which otherwise receives nothing for that LSN at all, or (worse)
        // a `CHAIN_CONTINUE` fragment's partial bytes misread as a complete
        // command.
        let mut assembler = wal::ChainAssembler::default();
        for (lsn, raw) in records {
            let (header, body) = match assembler
                .feed(&raw, 0)
                .map_err(|_| StorageError::CorruptLog)?
            {
                wal::ChainFeed::Pending => continue,
                wal::ChainFeed::Complete(header, body) => (header, body),
            };
            if header.lsn != lsn {
                return Err(StorageError::CorruptLog);
            }
            let (_, _ops, command) =
                wal::decode_logical_body(header, &body).map_err(|_| StorageError::CorruptLog)?;
            if let Some(batch) = command {
                out.push((lsn, rushdb_types::command_codec::encode(&batch)));
            }
        }
        Ok(out)
    }
}
