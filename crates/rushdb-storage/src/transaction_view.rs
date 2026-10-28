//! Buffered transaction reads and dependency tracking over one storage view.

use super::{
    prefix_range, redb_backend, Arc, BTreeMap, DependencySet, Included, Op, OverlayView,
    ReadLimits, ReadView, RefCell, ScanBounds, ScanCursor, ScanProjection, ScanRow, StorageError,
    Unbounded,
};

/// A transaction's base read source, checked after its staged `buffer`.
/// Exactly one variant applies per transaction, chosen at construction —
/// there is no longer a "live data" field that sits unused (defaulted to a
/// static empty sentinel) whenever a snapshot/overlay/native base applies
/// instead.
pub(super) enum TxnBase<'a> {
    /// Committed-plus-pending overlay supplied by the sequencer: reads
    /// resolve through it (its own pending buffer over a committed
    /// snapshot) instead of any other base, since it is already
    /// authoritative.
    Overlay(Arc<OverlayView>),
    /// Snapshot pinned at BEGIN for a cross-request transaction: all base
    /// reads resolve at that commit index instead of current state, so the
    /// transaction sees a stable base plus its own overlay. Autocommit and
    /// recovery use a different variant and read current committed state
    /// under the writer admission lock.
    Snapshot(Arc<ReadView>),
    /// Native-backend merged view over current committed state.
    Native(redb_backend::MergedViews),
    /// The live committed-data map itself, borrowed for the transaction's
    /// duration (no native backend installed).
    Live(&'a BTreeMap<Vec<u8>, Vec<u8>>),
}

impl<'a> TxnBase<'a> {
    /// Choose a transaction's base in priority order: an overlay or pinned
    /// snapshot (callers pass at most one of the two — a transaction never
    /// carries both), else a native merged view, else the live
    /// committed-data map. `live` must be `Some` whenever none of the other
    /// three apply — callers only acquire the read-lock guard it derefs
    /// from when a cheaper base doesn't already apply.
    ///
    /// # Panics
    /// Panics if `live` is `None` while `overlay`, `snapshot`, and `native`
    /// are all also `None` — every call site arranges for one of the four
    /// to be available.
    pub(super) fn resolve(
        overlay: Option<Arc<OverlayView>>,
        snapshot: Option<Arc<ReadView>>,
        native: Option<redb_backend::MergedViews>,
        live: Option<&'a BTreeMap<Vec<u8>, Vec<u8>>>,
    ) -> Self {
        if let Some(overlay) = overlay {
            return TxnBase::Overlay(overlay);
        }
        if let Some(snapshot) = snapshot {
            return TxnBase::Snapshot(snapshot);
        }
        if let Some(native) = native {
            return TxnBase::Native(native);
        }
        TxnBase::Live(live.expect(
            "a transaction without an overlay, snapshot, or native view always has the live guard",
        ))
    }
}

/// Buffered view handed to admission/apply: reads see committed data merged
/// with staged writes; writes stay buffered until the closure succeeds.
pub struct Txn<'a> {
    pub(super) base: TxnBase<'a>,
    pub(super) buffer: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    /// Keys staged via [`Txn::purge`]: their drained op is [`Op::Purge`]
    /// (history-erasing) instead of [`Op::Del`] (history-retaining).
    pub(super) purge_keys: std::collections::BTreeSet<Vec<u8>>,
    /// Keys staged via [`Txn::set`]: their drained op is [`Op::Set`]
    /// (history-free overwrite) instead of [`Op::Put`] (history-retaining).
    pub(super) set_keys: std::collections::BTreeSet<Vec<u8>>,
    pub(super) deps: Option<&'a RefCell<DependencySet>>,
}

/// Materialize a bounded slice of a pinned snapshot's range. `max_rows == 0`
/// means unbounded. Used by transaction reads so their base view is the
/// transaction snapshot rather than current committed state.
fn snapshot_scan(
    snapshot: &ReadView,
    start: &[u8],
    end: &[u8],
    projection: ScanProjection,
    max_rows: usize,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
    let bounds = ScanBounds {
        start: Included(start.to_vec()),
        end: if end.is_empty() {
            Unbounded
        } else {
            std::ops::Bound::Excluded(end.to_vec())
        },
    };
    let mut cursor = snapshot.scan(bounds, projection)?;
    let mut rows: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    loop {
        let remaining = if max_rows == 0 {
            1024
        } else {
            max_rows.saturating_sub(rows.len()).clamp(1, 1024)
        };
        let batch = cursor.next_batch(remaining, 8 * 1024 * 1024)?;
        for row in batch.rows {
            rows.push((row.key, row.value.unwrap_or_default()));
        }
        if batch.done || (max_rows != 0 && rows.len() >= max_rows) {
            break;
        }
    }
    Ok(rows)
}

/// Lazy key-only iterator over a pinned snapshot's rows in `[start, end)`.
/// Unlike [`snapshot_scan`] it does not materialize, so a caller can stop after
/// a bounded number of live keys without walking (or buffering) a whole prefix.
struct SnapshotKeyIter<'view> {
    cursor: ScanCursor<'view>,
    pending: std::vec::IntoIter<ScanRow>,
    done: bool,
}

impl<'view> SnapshotKeyIter<'view> {
    fn new(snapshot: &'view ReadView, start: &[u8], end: &[u8]) -> Result<Self, StorageError> {
        let bounds = ScanBounds {
            start: Included(start.to_vec()),
            end: if end.is_empty() {
                Unbounded
            } else {
                std::ops::Bound::Excluded(end.to_vec())
            },
        };
        Ok(Self {
            cursor: snapshot.scan(bounds, ScanProjection::Keys)?,
            pending: Vec::new().into_iter(),
            done: false,
        })
    }
}

impl Iterator for SnapshotKeyIter<'_> {
    type Item = Result<Vec<u8>, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(row) = self.pending.next() {
                return Some(Ok(row.key));
            }
            if self.done {
                return None;
            }
            match self.cursor.next_batch(1024, 8 * 1024 * 1024) {
                Ok(batch) => {
                    self.done = batch.done;
                    self.pending = batch.rows.into_iter();
                }
                Err(error) => {
                    self.done = true;
                    return Some(Err(error));
                }
            }
        }
    }
}

/// Which side of the merged keys-only iteration a row is drawn from.
enum Take {
    /// The next key comes from the base (committed) iterator.
    Base,
    /// The next key comes from the staged overlay buffer.
    Overlay { key: Vec<u8>, live: bool },
}

impl Txn<'_> {
    fn track(&self, k: &[u8]) {
        if let Some(deps) = &self.deps {
            deps.borrow_mut().record(k.to_vec(), k.to_vec());
        }
    }

    /// Read the value staged in this transaction, falling back to the
    /// underlying committed view.
    ///
    /// # Errors
    /// Returns [`StorageError::ResourceExhausted`] when the key plus value
    /// exceeds the default read item limit.
    pub fn get(&self, k: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.track(k);
        if let Some(v) = self.buffer.get(k) {
            ReadLimits::default().check_item(k.len(), v.as_ref().map_or(0, Vec::len))?;
            return Ok(v.clone());
        }
        match &self.base {
            TxnBase::Overlay(overlay) => {
                let value = overlay.get(k)?;
                if let Some(v) = &value {
                    ReadLimits::default().check_item(k.len(), v.len())?;
                }
                Ok(value)
            }
            TxnBase::Snapshot(snapshot) => snapshot.get(k),
            TxnBase::Native(native) => {
                native.get_current(k, ReadLimits::default(), ScanProjection::KeyValue)
            }
            TxnBase::Live(data) => data
                .get(k)
                .map(|value| {
                    ReadLimits::default().check_item(k.len(), value.len())?;
                    Ok(value.clone())
                })
                .transpose(),
        }
    }

    /// Read-through helper that also records a point dependency.
    ///
    /// # Errors
    /// Returns [`StorageError::ResourceExhausted`] when the key plus value
    /// exceeds the default read item limit.
    pub fn get_tracked(&mut self, k: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.get(k)
    }

    /// Collect the committed and staged entries in the half-open byte range
    /// `[start, end)`; an empty `end` means unbounded above.
    ///
    /// # Errors
    /// Returns [`StorageError::ResourceExhausted`] if the materialized result
    /// would exceed the 64 MiB transaction-range ceiling.
    pub fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        self.range_projected(start, end, ScanProjection::KeyValue)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one linear merge of base, overlay and staged rows; splitting it obscures the precedence order"
    )]
    fn range_projected(
        &self,
        start: &[u8],
        end: &[u8],
        projection: ScanProjection,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        if let Some(deps) = &self.deps {
            deps.borrow_mut().record(start.to_vec(), end.to_vec());
        }
        if !end.is_empty() && start >= end {
            return Ok(Vec::new());
        }
        let bounds = (
            Included(start),
            if end.is_empty() {
                Unbounded
            } else {
                std::ops::Bound::Excluded(end)
            },
        );
        let source: Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + '_> =
            match &self.base {
                TxnBase::Overlay(overlay) => {
                    if projection == ScanProjection::Keys {
                        Box::new(
                            overlay
                                .range_keys(start, end)?
                                .into_iter()
                                .map(|key| Ok((key, Vec::new()))),
                        )
                    } else {
                        Box::new(overlay.range(start, end)?.into_iter().map(Ok))
                    }
                }
                TxnBase::Snapshot(snapshot) => {
                    let rows = snapshot_scan(snapshot, start, end, projection, 0)?;
                    Box::new(rows.into_iter().map(Ok::<_, StorageError>))
                }
                TxnBase::Native(native) => {
                    Box::new(native.current_range(start, end, ReadLimits::default(), projection)?)
                }
                TxnBase::Live(data) => Box::new(data.range::<[u8], _>(bounds).map(|(k, v)| {
                    let v = if projection == ScanProjection::Keys {
                        &[][..]
                    } else {
                        v.as_slice()
                    };
                    ReadLimits::default().check_item(k.len(), v.len())?;
                    Ok((k.clone(), v.to_vec()))
                })),
            };
        let mut base = source.peekable();
        let mut overlay = self.buffer.range::<[u8], _>(bounds).peekable();
        let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        let mut bytes = 0usize;
        while base.peek().is_some() || overlay.peek().is_some() {
            if base.peek().is_some_and(Result::is_err) {
                return Err(base.next().unwrap().unwrap_err());
            }
            let row = match (base.peek(), overlay.peek()) {
                (Some(Ok((bk, _))), Some((ok, _))) if bk < *ok => Some(base.next().unwrap()?),
                (Some(Ok((bk, _))), Some((ok, _))) if bk == *ok => {
                    base.next();
                    let (k, value) = overlay.next().unwrap();
                    value
                        .as_ref()
                        .map(|v| {
                            let v = if projection == ScanProjection::Keys {
                                &[][..]
                            } else {
                                v.as_slice()
                            };
                            ReadLimits::default().check_item(k.len(), v.len())?;
                            Ok((k.clone(), v.to_vec()))
                        })
                        .transpose()?
                }
                (_, Some(_)) => {
                    let (k, value) = overlay.next().unwrap();
                    value
                        .as_ref()
                        .map(|v| {
                            let v = if projection == ScanProjection::Keys {
                                &[][..]
                            } else {
                                v.as_slice()
                            };
                            ReadLimits::default().check_item(k.len(), v.len())?;
                            Ok((k.clone(), v.to_vec()))
                        })
                        .transpose()?
                }
                (Some(_), None) => Some(base.next().unwrap()?),
                (None, None) => break,
            };
            if let Some((k, v)) = row {
                ReadLimits::default().check_item(k.len(), v.len())?;
                bytes = bytes.checked_add(k.len() + v.len()).ok_or_else(|| {
                    StorageError::ResourceExhausted("transaction range size overflow".into())
                })?;
                if bytes > 64 * 1024 * 1024 {
                    return Err(StorageError::ResourceExhausted(
                        "materialized transaction range exceeds 64 MiB; use a bounded operation"
                            .into(),
                    ));
                }
                out.push((k, v));
            }
        }
        Ok(out)
    }

    /// Range-scan helper that records the scanned range dependency.
    ///
    /// # Errors
    /// Returns [`StorageError::ResourceExhausted`] if the materialized result
    /// would exceed the 64 MiB transaction-range ceiling.
    pub fn range_prefix_tracked(
        &mut self,
        prefix: &[u8],
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        self.range_prefix(prefix)
    }

    /// Collect the committed and staged entries under `prefix`.
    ///
    /// # Errors
    /// Returns [`StorageError::ResourceExhausted`] if the materialized result
    /// would exceed the 64 MiB transaction-range ceiling.
    pub fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let (s, e) = prefix_range(prefix);
        let out = self.range_projected(&s, &e, ScanProjection::KeyValue)?;
        if let Some(deps) = &self.deps {
            deps.borrow_mut().record_prefix(prefix);
        }
        Ok(out)
    }

    /// Key-only ordered scan merged with staged replacements/tombstones.
    /// Payloads are not copied or charged as result bytes. The transitional
    /// 64 MiB materialization ceiling still applies to copied key bytes.
    ///
    /// # Errors
    /// Returns [`StorageError::ResourceExhausted`] if the materialized result
    /// would exceed the 64 MiB transaction-range ceiling.
    pub fn keys_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>, StorageError> {
        let (start, end) = prefix_range(prefix);
        let out = self
            .range_projected(&start, &end, ScanProjection::Keys)?
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        if let Some(deps) = &self.deps {
            deps.borrow_mut().record_prefix(prefix);
        }
        Ok(out)
    }

    /// Bounded keys-only enumeration under a prefix, merged with the staged
    /// buffer (staged replacements are included, tombstones skipped): at most
    /// `limit` live keys in ascending order. Unlike [`Txn::keys_prefix`] it
    /// never applies the 64 MiB materialization ceiling — callers use it to
    /// make bounded progress over an arbitrarily large keyspace (chunked
    /// tenant purge) instead of materializing the whole range.
    ///
    /// # Errors
    /// Returns [`StorageError::ResourceExhausted`] when a single key exceeds
    /// the default read item limit.
    ///
    /// # Panics
    /// Panics only if an internal peeked iterator yields `None` immediately
    /// after `peek` reported an element, which cannot happen for the merged
    /// in-memory iterators used here.
    pub fn keys_prefix_limited(
        &self,
        prefix: &[u8],
        limit: usize,
    ) -> Result<Vec<Vec<u8>>, StorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let (start, end) = prefix_range(prefix);
        if !end.is_empty() && start >= end {
            return Ok(Vec::new());
        }
        let bounds = (
            Included(start.as_slice()),
            if end.is_empty() {
                Unbounded
            } else {
                std::ops::Bound::Excluded(end.as_slice())
            },
        );
        // Staged writes are this transaction's buffer plus, when the sequencer
        // supplies an overlay, the overlay's pending buffer.
        let merged;
        let staged: &BTreeMap<Vec<u8>, Option<Vec<u8>>> =
            if let TxnBase::Overlay(overlay) = &self.base {
                let mut map = overlay.buffer().clone();
                for (key, value) in &self.buffer {
                    map.insert(key.clone(), value.clone());
                }
                merged = map;
                &merged
            } else {
                &self.buffer
            };
        let source: Box<dyn Iterator<Item = Result<Vec<u8>, StorageError>> + '_> = match &self.base
        {
            TxnBase::Overlay(overlay) => {
                Box::new(SnapshotKeyIter::new(overlay.base(), &start, &end)?)
            }
            TxnBase::Snapshot(snapshot) => Box::new(
                snapshot_scan(snapshot, &start, &end, ScanProjection::Keys, limit)?
                    .into_iter()
                    .map(|(key, _)| key)
                    .map(Ok::<_, StorageError>),
            ),
            TxnBase::Native(native) => Box::new(
                native
                    .current_range(&start, &end, ReadLimits::default(), ScanProjection::Keys)?
                    .map(|row| row.map(|(key, _)| key)),
            ),
            TxnBase::Live(data) => Box::new(data.range::<[u8], _>(bounds).map(|(key, _)| {
                ReadLimits::default().check_item(key.len(), 0)?;
                Ok(key.clone())
            })),
        };
        let mut base = source.peekable();
        let mut staged_iter = staged.range::<[u8], _>(bounds).peekable();
        let mut out = Vec::with_capacity(limit.min(4096));
        while out.len() < limit && (base.peek().is_some() || staged_iter.peek().is_some()) {
            if base.peek().is_some_and(Result::is_err) {
                return Err(base.next().unwrap().unwrap_err());
            }
            let take = match (base.peek(), staged_iter.peek()) {
                (Some(Ok(bk)), Some((ok, _))) if bk < *ok => Take::Base,
                (Some(Ok(bk)), Some((ok, _))) if bk == *ok => {
                    base.next();
                    let entry = staged_iter.next().unwrap();
                    Take::Overlay {
                        key: entry.0.clone(),
                        live: entry.1.is_some(),
                    }
                }
                (_, Some(_)) => {
                    let entry = staged_iter.next().unwrap();
                    Take::Overlay {
                        key: entry.0.clone(),
                        live: entry.1.is_some(),
                    }
                }
                (Some(_), None) => Take::Base,
                (None, None) => break,
            };
            match take {
                Take::Base => {
                    let key = base.next().unwrap()?;
                    ReadLimits::default().check_item(key.len(), 0)?;
                    out.push(key);
                }
                Take::Overlay { key, live } => {
                    if live {
                        ReadLimits::default().check_item(key.len(), 0)?;
                        out.push(key);
                    }
                }
            }
        }
        if let Some(deps) = &self.deps {
            deps.borrow_mut().record_prefix(prefix);
        }
        Ok(out)
    }

    /// Stage an insert/overwrite of `k`; drains as [`Op::Put`].
    pub fn put(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.buffer.insert(k, Some(v));
    }

    /// Stage a deletion of `k`; drains as [`Op::Del`].
    pub fn del(&mut self, k: Vec<u8>) {
        self.buffer.insert(k, None);
    }

    /// Stage a history-erasing erase of `k` (tenant purge). Unlike [`Txn::del`]
    /// the drained op is [`Op::Purge`], so no superseded version is retained
    /// for a key whose value is intentionally destroyed.
    pub fn purge(&mut self, k: Vec<u8>) {
        self.purge_keys.insert(k.clone());
        self.buffer.insert(k, None);
    }

    /// Stage a history-free overwrite of `k` (internal bookkeeping keys).
    /// Unlike [`Txn::put`] the drained op is [`Op::Set`], so overwriting a hot
    /// counter does not append to its retained history.
    pub fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.set_keys.insert(k.clone());
        self.buffer.insert(k, Some(v));
    }

    /// Drain the staged base ops.
    pub(super) fn drain(self) -> Vec<Op> {
        let Txn {
            buffer,
            purge_keys,
            set_keys,
            ..
        } = self;
        buffer
            .into_iter()
            .map(|(k, v)| match v {
                Some(v) if set_keys.contains(&k) => Op::Set(k, v),
                Some(v) => Op::Put(k, v),
                None if purge_keys.contains(&k) => Op::Purge(k),
                None => Op::Del(k),
            })
            .collect()
    }
}
