//! Candidate ordinals for a search: by ordinal (either way), in record id
//! order, or a property's holders and the records missing it, for the
//! order plans of `order.rs`.
//!
//! A search without a usable index walks every record of the tenant or of
//! its labels. Those walks stream key batches from the snapshot instead of
//! collecting the population first, so a page can stop after its rows and
//! a count holds no ordinals at all.

use crate::planner::OwnerSet;
use crate::{QueryError, QuerySnapshot};

/// Rows fetched by a scan's first batch; each later batch doubles, up to
/// [`BATCH_ROWS`]. A page that stops after a few rows reads about as many
/// as it uses, and a full walk reaches full batches after six.
const FIRST_BATCH_ROWS: usize = 64;
/// Rows fetched per scan batch at most.
const BATCH_ROWS: usize = 4096;
/// Bytes fetched per scan batch.
const BATCH_BYTES: usize = 8 * 1024 * 1024;

/// A key and, when asked for, its value.
type KeyRow = (Vec<u8>, Option<Vec<u8>>);

/// One key scan of a prefix: ascending in batches (the first
/// [`FIRST_BATCH_ROWS`] rows, doubling up to [`BATCH_ROWS`]), or descending
/// one reverse seek per key. Every scan below reads its keys through one.
pub(crate) struct KeyWalk<'v> {
    snap: &'v QuerySnapshot,
    /// Ascending: a cursor over the keys; descending: `None`.
    cursor: Option<rushdb_storage::ScanCursor<'v>>,
    prefix: Vec<u8>,
    /// Descending: the exclusive end of the next seek.
    before: std::ops::Bound<Vec<u8>>,
    /// Whether each key comes with its value.
    values: bool,
    buffered: std::collections::VecDeque<KeyRow>,
    batch_rows: usize,
    done: bool,
}

impl<'v> KeyWalk<'v> {
    fn new(
        snap: &'v QuerySnapshot,
        prefix: Vec<u8>,
        descending: bool,
        values: bool,
    ) -> Result<Self, QueryError> {
        let bounds = rushdb_storage::ScanBounds::prefix(&prefix);
        let before = bounds.end.clone();
        let projection = if values {
            rushdb_storage::ScanProjection::KeyValue
        } else {
            rushdb_storage::ScanProjection::Keys
        };
        Ok(Self {
            snap,
            cursor: if descending {
                None
            } else {
                Some(snap.scan(bounds, projection)?)
            },
            prefix,
            before,
            values,
            buffered: std::collections::VecDeque::new(),
            batch_rows: FIRST_BATCH_ROWS,
            done: false,
        })
    }

    /// The next key (with its value, when asked for).
    fn next(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<Option<KeyRow>, QueryError> {
        while self.buffered.is_empty() && !self.done {
            if is_cancelled() {
                return Err(QueryError::Cancelled);
            }
            if let Some(cursor) = &mut self.cursor {
                let batch = cursor.next_batch(self.batch_rows, BATCH_BYTES)?;
                self.batch_rows = (self.batch_rows * 2).min(BATCH_ROWS);
                self.buffered
                    .extend(batch.rows.into_iter().map(|row| (row.key, row.value)));
                self.done = batch.done;
                continue;
            }
            let bounds = rushdb_storage::ScanBounds {
                start: std::ops::Bound::Included(self.prefix.clone()),
                end: self.before.clone(),
            };
            match self.snap.last_key(bounds)? {
                Some(key) => {
                    let value = if self.values {
                        Some(
                            self.snap
                                .get(&key)?
                                .ok_or(rushdb_storage::StorageError::CorruptLog)?,
                        )
                    } else {
                        None
                    };
                    self.before = std::ops::Bound::Excluded(key.clone());
                    self.buffered.push_back((key, value));
                }
                None => self.done = true,
            }
        }
        Ok(self.buffered.pop_front())
    }

    /// Descending: go on below `key`, passing over every key at or after it.
    fn continue_below(&mut self, key: Vec<u8>) {
        self.buffered.clear();
        self.before = std::ops::Bound::Excluded(key);
    }
}

/// One key scan yielding the ordinal that ends each key.
pub(crate) struct OrdinalScan<'v> {
    walk: KeyWalk<'v>,
    parse: fn(&[u8], usize) -> Option<u64>,
}

impl<'v> OrdinalScan<'v> {
    fn new(
        snap: &'v QuerySnapshot,
        prefix: &[u8],
        parse: fn(&[u8], usize) -> Option<u64>,
        descending: bool,
    ) -> Result<Self, QueryError> {
        Ok(Self {
            walk: KeyWalk::new(snap, prefix.to_vec(), descending, false)?,
            parse,
        })
    }

    fn next(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<Option<u64>, QueryError> {
        let Some((key, _)) = self.walk.next(is_cancelled)? else {
            return Ok(None);
        };
        Ok(Some(
            (self.parse)(&key, self.walk.prefix.len())
                .ok_or(rushdb_storage::StorageError::CorruptLog)?,
        ))
    }
}

/// Where a search's candidates come from.
pub(crate) enum Candidates<'v> {
    /// The index plan's owner set, ascending.
    Set(Box<roaring::treemap::IntoIter>),
    /// The index plan's owner set, descending.
    SetDescending(Box<std::iter::Rev<roaring::treemap::IntoIter>>),
    /// Merged key scans, ascending or descending by ordinal: the tenant's
    /// records, the members of labels, or presence rows. The streams are
    /// disjoint (a record has one primary label, and holds a property under
    /// one definition with one nullness).
    Scans(Box<Merged<'v>>),
    /// Every record of the tenant, or of the requested labels, in record id
    /// order.
    Ids(Box<IdScan<'v>>),
    /// The records of some labels that hold none of a property's
    /// definitions, ascending.
    Missing(Box<MissingScan<'v>>),
    /// The owners of ordered index entries, by value either way and by
    /// ascending ordinal among equal values.
    Index(Box<IndexMerge<'v>>),
}

/// One ordered index's entries `(value, owner)`.
pub(crate) struct EntryScan<'v> {
    walk: KeyWalk<'v>,
}

impl<'v> EntryScan<'v> {
    fn new(snap: &'v QuerySnapshot, prefix: Vec<u8>, descending: bool) -> Result<Self, QueryError> {
        Ok(Self {
            walk: KeyWalk::new(snap, prefix, descending, false)?,
        })
    }

    fn next(
        &mut self,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<(Vec<u8>, u64)>, QueryError> {
        let Some((key, _)) = self.walk.next(is_cancelled)? else {
            return Ok(None);
        };
        let (entry, owner) = rushdb_types::keys::parse_index_entry_key(&key, &self.walk.prefix)
            .ok_or(rushdb_storage::StorageError::CorruptLog)?;
        Ok(Some((entry.to_vec(), owner)))
    }

    /// The owners of `value`'s entries, ascending, and this scan moved on
    /// below them (descending).
    fn take_run(&mut self, value: &[u8]) -> Result<RunScan<'v>, QueryError> {
        let mut prefix = self.walk.prefix.clone();
        prefix.extend_from_slice(value);
        self.walk.continue_below(prefix.clone());
        Ok(RunScan {
            walk: KeyWalk::new(self.walk.snap, prefix, false, false)?,
        })
    }
}

/// The owners of one index value, ascending: the keys under
/// `index prefix ‖ value` that end right after an 8-byte owner. (A string
/// value ends in NUL, so a longer string sharing it as a prefix only shows
/// up here when it holds an interior NUL, and its keys are longer.)
pub(crate) struct RunScan<'v> {
    walk: KeyWalk<'v>,
}

impl RunScan<'_> {
    fn next(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<Option<u64>, QueryError> {
        while let Some((key, _)) = self.walk.next(is_cancelled)? {
            let Some(owner) = key.strip_prefix(self.walk.prefix.as_slice()) else {
                return Err(rushdb_storage::StorageError::CorruptLog.into());
            };
            if let Ok(owner) = <[u8; 8]>::try_from(owner) {
                return Ok(Some(u64::from_be_bytes(owner)));
            }
        }
        Ok(None)
    }
}

/// Ordered indexes of several labels merged by value: ascending by `(value,
/// owner)`, or descending by value with each value's owners ascending (so
/// equal values keep the ordinal order a sort keeps). A descending value's
/// owners stream from an ascending scan of that value, so a value held by
/// millions of records yields its first owner after one batch.
pub(crate) struct IndexMerge<'v> {
    scans: Vec<EntryScan<'v>>,
    heads: Vec<Option<(Vec<u8>, u64)>>,
    started: bool,
    descending: bool,
    /// Descending: the owners of the value being emitted, one ascending scan
    /// per index holding it, merged by owner.
    run: Vec<(RunScan<'v>, Option<u64>)>,
}

impl IndexMerge<'_> {
    fn next(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<Option<u64>, QueryError> {
        if !self.started {
            for (scan, head) in self.scans.iter_mut().zip(self.heads.iter_mut()) {
                *head = scan.next(is_cancelled)?;
            }
            self.started = true;
        }
        if !self.descending {
            let Some((_, i)) = self
                .heads
                .iter()
                .enumerate()
                .filter_map(|(i, head)| head.as_ref().map(|head| (head, i)))
                .min()
            else {
                return Ok(None);
            };
            let owner = self.heads[i].take().map(|(_, owner)| owner);
            self.heads[i] = self.scans[i].next(is_cancelled)?;
            return Ok(owner);
        }
        loop {
            if let Some(i) = self
                .run
                .iter()
                .enumerate()
                .filter_map(|(i, (_, head))| head.map(|owner| (owner, i)))
                .min()
                .map(|(_, i)| i)
            {
                let (scan, head) = &mut self.run[i];
                let owner = *head;
                *head = scan.next(is_cancelled)?;
                return Ok(owner);
            }
            self.run.clear();
            let Some(value) = self
                .heads
                .iter()
                .flatten()
                .map(|(value, _)| value)
                .max()
                .cloned()
            else {
                return Ok(None);
            };
            // Every index holding the largest value hands over its owners
            // and moves on below it.
            for (scan, head) in self.scans.iter_mut().zip(self.heads.iter_mut()) {
                if head.as_ref().is_some_and(|(entry, _)| *entry == value) {
                    let mut run = scan.take_run(&value)?;
                    let first = run.next(is_cancelled)?;
                    self.run.push((run, first));
                    *head = scan.next(is_cancelled)?;
                }
            }
        }
    }
}

/// A k-way merge of ordinal scans; each scan may carry the label its
/// records belong to.
pub(crate) struct Merged<'v> {
    scans: Vec<OrdinalScan<'v>>,
    labels: Vec<Option<String>>,
    heads: Vec<Option<u64>>,
    started: bool,
    descending: bool,
}

impl<'v> Merged<'v> {
    fn new(scans: Vec<(OrdinalScan<'v>, Option<String>)>, descending: bool) -> Self {
        let (scans, labels): (Vec<_>, Vec<_>) = scans.into_iter().unzip();
        let heads = vec![None; scans.len()];
        Self {
            scans,
            labels,
            heads,
            started: false,
            descending,
        }
    }

    /// The next ordinal and the label of the scan it came from.
    fn next(
        &mut self,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<(u64, Option<&str>)>, QueryError> {
        if !self.started {
            for (scan, head) in self.scans.iter_mut().zip(self.heads.iter_mut()) {
                *head = scan.next(is_cancelled)?;
            }
            self.started = true;
        }
        let heads = self
            .heads
            .iter()
            .enumerate()
            .filter_map(|(i, head)| head.map(|ord| (ord, i)));
        let best = if self.descending {
            heads.max()
        } else {
            heads.min()
        };
        let Some((_, i)) = best else {
            return Ok(None);
        };
        let ord = self.heads[i].take();
        self.heads[i] = self.scans[i].next(is_cancelled)?;
        Ok(ord.map(|ord| (ord, self.labels[i].as_deref())))
    }
}

/// The tenant's record identity rows (`RECORD_ID`: id → ordinal, in id byte
/// order, which is the order of `__id` strings), keeping the records of
/// `labels` (all when empty) by their header's label.
pub(crate) struct IdScan<'v> {
    walk: KeyWalk<'v>,
    tenant: rushdb_types::ids::TenantId,
    labels: std::collections::BTreeSet<String>,
    /// Records of other labels passed over, and how many may be.
    passed: u64,
    passing: u64,
}

impl<'v> IdScan<'v> {
    fn new(
        snap: &'v QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        labels: &[String],
        descending: bool,
    ) -> Result<Self, QueryError> {
        Ok(Self {
            walk: KeyWalk::new(
                snap,
                rushdb_types::keys::record_id_prefix(tenant),
                descending,
                true,
            )?,
            tenant,
            labels: labels.iter().cloned().collect(),
            passed: 0,
            passing: u64::MAX,
        })
    }

    /// Whether record `ord` is one of the scanned labels' (always, without
    /// labels).
    fn admits(&self, ord: u64) -> Result<bool, QueryError> {
        if self.labels.is_empty() {
            return Ok(true);
        }
        Ok(
            rushdb_lmpg::reader::try_access::record_by_ord(self.walk.snap, self.tenant, ord)?
                .is_some_and(|header| self.labels.contains(&header.label)),
        )
    }

    fn next(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<Option<u64>, QueryError> {
        while self.passed <= self.passing {
            let Some((_, value)) = self.walk.next(is_cancelled)? else {
                return Ok(None);
            };
            // The identity row's value is the ordinal.
            let ord = u64::from_be_bytes(
                value
                    .as_deref()
                    .unwrap_or_default()
                    .try_into()
                    .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
            );
            if self.admits(ord)? {
                return Ok(Some(ord));
            }
            self.passed += 1;
        }
        Ok(None)
    }
}

/// The records of some labels holding none of a property's definitions
/// (no presence row, value or null), ascending: the labels' members, less
/// the holders, checked with one presence lookup per definition and
/// nullness. A record of a label that holds no definition is missing it
/// without a lookup.
pub(crate) struct MissingScan<'v> {
    snap: &'v QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    walk: Merged<'v>,
    /// Labels holding a definition, and the definitions.
    holding: std::collections::BTreeSet<String>,
    pids: Vec<u64>,
    /// Holders passed over, and how many may be.
    passed: u64,
    passing: u64,
    /// Missing records not yet found (the counters know how many there
    /// are): the walk ends with the last one.
    remaining: u64,
}

impl MissingScan<'_> {
    fn next(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<Option<u64>, QueryError> {
        use rushdb_types::keys;
        if self.remaining == 0 {
            return Ok(None);
        }
        while let Some((ord, label)) = self.walk.next(is_cancelled)? {
            let label = label.unwrap_or_default();
            if !self.holding.contains(label) {
                self.remaining -= 1;
                return Ok(Some(ord));
            }
            let mut held = false;
            'pids: for &pid in &self.pids {
                for nullness in [keys::NULLNESS_VALUE, keys::NULLNESS_NULL] {
                    let key = keys::presence_key(
                        self.tenant,
                        rushdb_types::ids::OwnerKind::Record,
                        label,
                        pid,
                        nullness,
                        ord,
                    );
                    if rushdb_lmpg::reader::TryRead::get(self.snap.as_ref(), &key)?.is_some() {
                        held = true;
                        break 'pids;
                    }
                }
            }
            if !held {
                self.remaining -= 1;
                return Ok(Some(ord));
            }
            self.passed += 1;
            if self.passed > self.passing {
                return Ok(None);
            }
        }
        Ok(None)
    }
}

impl<'v> Candidates<'v> {
    pub(crate) fn from_set(set: OwnerSet) -> Self {
        Self::Set(Box::new(set.into_iter()))
    }

    /// The owner set walked from its largest ordinal down, as it is read.
    pub(crate) fn from_set_descending(set: OwnerSet) -> Self {
        Self::SetDescending(Box::new(set.into_iter().rev()))
    }

    /// Every record of the tenant (`labels` empty) or of `labels`, ascending
    /// by ordinal.
    pub(crate) fn walk(
        snap: &'v QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        labels: &[String],
    ) -> Result<Self, QueryError> {
        Self::walk_ordered(snap, tenant, labels, false)
    }

    /// [`Self::walk`], ascending or descending by ordinal.
    pub(crate) fn walk_ordered(
        snap: &'v QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        labels: &[String],
        descending: bool,
    ) -> Result<Self, QueryError> {
        let mut scans = Vec::new();
        if labels.is_empty() {
            scans.push((
                OrdinalScan::new(
                    snap,
                    &rushdb_types::keys::record_hdr_prefix(tenant),
                    |key, _| rushdb_types::keys::parse_record_hdr_key(key),
                    descending,
                )?,
                None,
            ));
        } else {
            scans = label_scans(snap, tenant, labels, descending)?;
        }
        Ok(Self::Scans(Box::new(Merged::new(scans, descending))))
    }

    /// Every record of the tenant (`labels` empty) or of `labels`, in
    /// record id order.
    pub(crate) fn by_id(
        snap: &'v QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        labels: &[String],
        descending: bool,
    ) -> Result<Self, QueryError> {
        Ok(Self::Ids(Box::new(IdScan::new(
            snap, tenant, labels, descending,
        )?)))
    }

    /// The owners of the presence rows under `prefixes` (disjoint, each
    /// ending in an ordinal), ascending.
    pub(crate) fn presence(
        snap: &'v QuerySnapshot,
        prefixes: &[Vec<u8>],
    ) -> Result<Self, QueryError> {
        let mut scans = Vec::new();
        for prefix in prefixes {
            scans.push((
                OrdinalScan::new(snap, prefix, rushdb_types::keys::read_ord_tail, false)?,
                None,
            ));
        }
        Ok(Self::Scans(Box::new(Merged::new(scans, false))))
    }

    /// The `missing` records of `labels` holding none of `pids` (each held
    /// under the labels in `holding`), ascending.
    pub(crate) fn missing(
        snap: &'v QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        labels: &[String],
        holding: std::collections::BTreeSet<String>,
        pids: Vec<u64>,
        missing: u64,
    ) -> Result<Self, QueryError> {
        Ok(Self::Missing(Box::new(MissingScan {
            snap,
            tenant,
            walk: Merged::new(label_scans(snap, tenant, labels, false)?, false),
            holding,
            pids,
            passed: 0,
            passing: u64::MAX,
            remaining: missing,
        })))
    }

    /// The owners of the ordered indexes `ids`, by value (descending: values
    /// down, equal values' owners up).
    pub(crate) fn index_order(
        snap: &'v QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        ids: &[u32],
        descending: bool,
    ) -> Result<Self, QueryError> {
        let mut scans = Vec::new();
        for &id in ids {
            scans.push(EntryScan::new(
                snap,
                rushdb_types::keys::index_entry_prefix(tenant, id),
                descending,
            )?);
        }
        let heads = vec![None; scans.len()];
        Ok(Self::Index(Box::new(IndexMerge {
            scans,
            heads,
            started: false,
            descending,
            run: Vec::new(),
        })))
    }

    /// The next candidate, in this source's order.
    pub(crate) fn next(
        &mut self,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<u64>, QueryError> {
        match self {
            Self::Set(owners) => Ok(owners.next()),
            Self::SetDescending(owners) => Ok(owners.next()),
            Self::Scans(merged) => Ok(merged.next(is_cancelled)?.map(|(ord, _)| ord)),
            Self::Ids(scan) => scan.next(is_cancelled),
            Self::Missing(scan) => scan.next(is_cancelled),
            Self::Index(merge) => merge.next(is_cancelled),
        }
    }

    /// Records the source read and passed over without yielding them (other
    /// labels' records, holders of the property).
    pub(crate) fn passed(&self) -> u64 {
        match self {
            Self::Ids(scan) => scan.passed,
            Self::Missing(scan) => scan.passed,
            _ => 0,
        }
    }

    /// Stop once the source has passed over more than `rows` records.
    pub(crate) fn give_up_after(&mut self, rows: u64) {
        match self {
            Self::Ids(scan) => scan.passing = rows,
            Self::Missing(scan) => scan.passing = rows,
            _ => {}
        }
    }

    /// Whether the source stopped at its [`Self::give_up_after`] bound:
    /// what it yielded is not all of it.
    pub(crate) fn gave_up(&self) -> bool {
        match self {
            Self::Ids(scan) => scan.passed > scan.passing,
            Self::Missing(scan) => scan.passed > scan.passing,
            _ => false,
        }
    }

    /// Every remaining candidate.
    pub(crate) fn collect(
        mut self,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u64>, QueryError> {
        let mut out = Vec::new();
        while let Some(ord) = self.next(is_cancelled)? {
            out.push(ord);
        }
        Ok(out)
    }
}

/// One member scan per distinct label, each carrying its label.
fn label_scans<'v>(
    snap: &'v QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    labels: &[String],
    descending: bool,
) -> Result<Vec<(OrdinalScan<'v>, Option<String>)>, QueryError> {
    let distinct: std::collections::BTreeSet<&String> = labels.iter().collect();
    let mut scans = Vec::new();
    for label in distinct {
        // The label is length-prefixed, so the prefix matches only this
        // label's rows; the ordinal ends the key.
        scans.push((
            OrdinalScan::new(
                snap,
                &rushdb_types::keys::label_member_prefix(tenant, label),
                rushdb_types::keys::read_ord_tail,
                descending,
            )?,
            Some(label.clone()),
        ));
    }
    Ok(scans)
}

/// Exclusive upper bound of the tenant's record ordinals at `snap` (the
/// record allocator's committed watermark); `None` when it was never set.
pub(crate) fn ordinal_end<R: rushdb_lmpg::reader::TryRead + ?Sized>(
    snap: &R,
    tenant: rushdb_types::ids::TenantId,
) -> Result<Option<u64>, QueryError> {
    let Some(raw) = snap.get(&rushdb_lmpg::expand::meta_counter_key(tenant, "record"))? else {
        return Ok(None);
    };
    Ok(Some(u64::from_be_bytes(
        raw.as_slice()
            .try_into()
            .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
    )))
}

/// The records of the tenant (`labels` empty) or of `labels` whose ordinal
/// is in `ords`, ascending: one key range per label (an ordinal's key
/// encoding sorts by value, so the range holds exactly those ordinals).
pub(crate) fn walk_range(
    snap: &QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    labels: &[String],
    ords: std::ops::Range<u64>,
) -> Result<Vec<u64>, QueryError> {
    use rushdb_types::keys;
    let mut out = Vec::new();
    let mut scan = |start: Vec<u8>, end: Vec<u8>, ord_at: usize| -> Result<(), QueryError> {
        let mut cursor = snap.scan(
            rushdb_storage::ScanBounds {
                start: std::ops::Bound::Included(start),
                end: std::ops::Bound::Excluded(end),
            },
            rushdb_storage::ScanProjection::Keys,
        )?;
        loop {
            let batch = cursor.next_batch(BATCH_ROWS, BATCH_BYTES)?;
            for row in batch.rows {
                out.push(
                    keys::read_ord_tail(&row.key, ord_at)
                        .ok_or(rushdb_storage::StorageError::CorruptLog)?,
                );
            }
            if batch.done {
                return Ok(());
            }
        }
    };
    if labels.is_empty() {
        let at = keys::record_hdr_prefix(tenant).len();
        scan(
            keys::record_hdr_key(tenant, ords.start),
            keys::record_hdr_key(tenant, ords.end),
            at,
        )?;
        return Ok(out);
    }
    let distinct: std::collections::BTreeSet<&String> = labels.iter().collect();
    for label in &distinct {
        let at = keys::label_member_prefix(tenant, label).len();
        scan(
            keys::label_member_key(tenant, label, ords.start),
            keys::label_member_key(tenant, label, ords.end),
            at,
        )?;
    }
    if distinct.len() > 1 {
        // A record has one primary label: the label ranges are disjoint.
        out.sort_unstable();
    }
    Ok(out)
}
