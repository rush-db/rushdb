//! Candidate ordinals for a search, in ascending ordinal order.
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

/// One key scan yielding the ordinal that ends each key.
pub(crate) struct OrdinalScan<'v> {
    cursor: rushdb_storage::ScanCursor<'v>,
    parse: fn(&[u8], usize) -> Option<u64>,
    prefix_len: usize,
    buffered: std::collections::VecDeque<u64>,
    batch_rows: usize,
    done: bool,
}

impl<'v> OrdinalScan<'v> {
    fn new(
        snap: &'v QuerySnapshot,
        prefix: &[u8],
        parse: fn(&[u8], usize) -> Option<u64>,
    ) -> Result<Self, QueryError> {
        Ok(Self {
            cursor: snap.scan(
                rushdb_storage::ScanBounds::prefix(prefix),
                rushdb_storage::ScanProjection::Keys,
            )?,
            parse,
            prefix_len: prefix.len(),
            buffered: std::collections::VecDeque::new(),
            batch_rows: FIRST_BATCH_ROWS,
            done: false,
        })
    }

    fn next(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<Option<u64>, QueryError> {
        while self.buffered.is_empty() && !self.done {
            if is_cancelled() {
                return Err(QueryError::Cancelled);
            }
            let batch = self.cursor.next_batch(self.batch_rows, BATCH_BYTES)?;
            self.batch_rows = (self.batch_rows * 2).min(BATCH_ROWS);
            for row in batch.rows {
                self.buffered.push_back(
                    (self.parse)(&row.key, self.prefix_len)
                        .ok_or(rushdb_storage::StorageError::CorruptLog)?,
                );
            }
            self.done = batch.done;
        }
        Ok(self.buffered.pop_front())
    }
}

/// Where a search's candidates come from.
pub(crate) enum Candidates<'v> {
    /// The index plan's owner set.
    Set(Box<roaring::treemap::IntoIter>),
    /// Every record of the tenant, or the members of the requested labels
    /// (merged; a record has one primary label, so the streams are disjoint).
    Scans {
        scans: Vec<OrdinalScan<'v>>,
        heads: Vec<Option<u64>>,
        started: bool,
    },
}

impl<'v> Candidates<'v> {
    pub(crate) fn from_set(set: OwnerSet) -> Self {
        Self::Set(Box::new(set.into_iter()))
    }

    /// Every record of the tenant (`labels` empty) or of `labels`.
    pub(crate) fn walk(
        snap: &'v QuerySnapshot,
        tenant: rushdb_types::ids::TenantId,
        labels: &[String],
    ) -> Result<Self, QueryError> {
        let mut scans = Vec::new();
        if labels.is_empty() {
            scans.push(OrdinalScan::new(
                snap,
                &rushdb_types::keys::record_hdr_prefix(tenant),
                |key, _| rushdb_types::keys::parse_record_hdr_key(key),
            )?);
        } else {
            let distinct: std::collections::BTreeSet<&String> = labels.iter().collect();
            for label in distinct {
                // The label is length-prefixed, so the prefix matches only
                // this label's rows; the ordinal ends the key.
                scans.push(OrdinalScan::new(
                    snap,
                    &rushdb_types::keys::label_member_prefix(tenant, label),
                    rushdb_types::keys::read_ord_tail,
                )?);
            }
        }
        let heads = vec![None; scans.len()];
        Ok(Self::Scans {
            scans,
            heads,
            started: false,
        })
    }

    /// The next candidate, ascending.
    pub(crate) fn next(
        &mut self,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<u64>, QueryError> {
        match self {
            Self::Set(owners) => Ok(owners.next()),
            Self::Scans {
                scans,
                heads,
                started,
            } => {
                if !*started {
                    for (scan, head) in scans.iter_mut().zip(heads.iter_mut()) {
                        *head = scan.next(is_cancelled)?;
                    }
                    *started = true;
                }
                let Some(i) = heads
                    .iter()
                    .enumerate()
                    .filter_map(|(i, head)| head.map(|ord| (ord, i)))
                    .min()
                    .map(|(_, i)| i)
                else {
                    return Ok(None);
                };
                let ord = heads[i].take();
                heads[i] = scans[i].next(is_cancelled)?;
                Ok(ord)
            }
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
