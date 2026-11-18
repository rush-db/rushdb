//! Publish at WAL durability (PUB-1): commits published but not yet
//! materialized into a shard's redb file.
//!
//! The publisher makes a commit visible once its WAL record is durable and
//! pushes its ops (base, derived and the usage rows the apply would write)
//! into its shard's [`Gap`]. The shard's materializer applies them to redb
//! in commit order, off the publish path, and trims them from the gap.
//! Readers see redb at its materialized commit through a [`GapView`], which
//! answers from the gap's versions first with exactly the visibility rules
//! redb's apply produces (see [`GapView::visit_value`]).
//!
//! The gap's versions live in a persistent ordered map: a reader takes a
//! snapshot in O(1) and keeps it for the life of its view while the
//! publisher adds and the materializer trims. A reader snapshots the gap
//! before it opens the redb transaction, so everything the snapshot lacks
//! is already in that transaction; versions the transaction also holds are
//! skipped.

use std::collections::{BTreeMap, VecDeque};
use std::ops::Bound;
use std::sync::{Arc, Condvar, Mutex, PoisonError, RwLock};

use crate::backend::StorageReadView;
use crate::read_view::{ReadLimits, ScanDirection, ScanProjection};
use crate::resources::{ByteBudget, ByteReservation};
use crate::{Op, StorageError};

/// One commit's effect on one key.
#[derive(Clone, Debug)]
pub(crate) struct Version {
    pub(crate) ci: u64,
    /// The key's value after the commit (`None`: absent).
    pub(crate) value: Option<Arc<[u8]>>,
    /// The commit Set or Purged the key: redb drops the key's history then,
    /// so no snapshot older than this commit sees any version of it.
    pub(crate) erases: bool,
}

/// Versions of every key the gap's commits write, oldest first per key.
pub(crate) type GapIndex = im::OrdMap<Arc<[u8]>, Arc<[Version]>>;

/// A published commit waiting for its shard's materializer.
pub(crate) struct GapCommit {
    pub(crate) ci: u64,
    pub(crate) base: Arc<Vec<Op>>,
    pub(crate) derived: Vec<Op>,
    /// The usage rows the apply computes for this commit; the gap serves them
    /// until the commit is materialized (debug builds check they match).
    pub(crate) usage: Vec<Op>,
    charge: ByteReservation,
}

impl GapCommit {
    fn ops(&self) -> impl Iterator<Item = &Op> {
        self.base.iter().chain(&self.derived).chain(&self.usage)
    }

    /// The commit's net effect per key, in key order.
    fn versions(&self) -> BTreeMap<&[u8], Version> {
        let mut out: BTreeMap<&[u8], Version> = BTreeMap::new();
        for op in self.ops() {
            let (value, erases) = match op {
                Op::Put(_, value) => (Some(Arc::from(value.as_slice())), false),
                Op::Set(_, value) => (Some(Arc::from(value.as_slice())), true),
                Op::Del(_) => (None, false),
                Op::Purge(_) => (None, true),
            };
            let version = out.entry(op.key()).or_insert(Version {
                ci: self.ci,
                value: None,
                erases: false,
            });
            version.value = value;
            version.erases |= erases;
        }
        out
    }
}

/// Bytes a commit charges against the gap's budget.
pub(crate) fn commit_bytes(base: &[Op], derived: &[Op], usage: &[Op]) -> usize {
    // Each key is held twice (the op and the index entry) plus per-entry
    // bookkeeping.
    base.iter()
        .chain(derived)
        .chain(usage)
        .map(|op| op.encoded_len() + op.key().len() + 64)
        .sum()
}

struct State {
    /// The writer's copy; `snapshot` is a clone of it.
    index: GapIndex,
    queue: VecDeque<Arc<GapCommit>>,
    /// Newest commit index pushed.
    top: u64,
    stopping: bool,
    paused: bool,
    failure: Option<String>,
}

/// One shard's published, not yet materialized commits.
pub(crate) struct Gap {
    state: Mutex<State>,
    snapshot: RwLock<GapIndex>,
    changed: Condvar,
    budget: ByteBudget,
    bound: usize,
}

impl Gap {
    pub(crate) fn new(budget: ByteBudget) -> Self {
        let bound = budget.snapshot().limit;
        Self {
            state: Mutex::new(State {
                index: GapIndex::new(),
                queue: VecDeque::new(),
                top: 0,
                stopping: false,
                paused: false,
                failure: None,
            }),
            snapshot: RwLock::new(GapIndex::new()),
            changed: Condvar::new(),
            budget,
            bound,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The versions readers see now.
    pub(crate) fn snapshot(&self) -> GapIndex {
        self.snapshot
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn publish(&self, index: &GapIndex) {
        *self
            .snapshot
            .write()
            .unwrap_or_else(PoisonError::into_inner) = index.clone();
    }

    /// The materializer's failure, which fences the store.
    pub(crate) fn failure(&self) -> Option<String> {
        self.lock().failure.clone()
    }

    /// Wait until the gap has room for a commit of `bytes` (backpressure),
    /// then charge it. A commit larger than the whole bound waits for an
    /// empty gap and takes all of it.
    pub(crate) fn reserve(&self, bytes: usize) -> Result<ByteReservation, StorageError> {
        let bytes = bytes.min(self.bound);
        let mut state = self.lock();
        loop {
            if let Some(failure) = &state.failure {
                return Err(StorageError::Durability(failure.clone()));
            }
            if let Ok(charge) = self.budget.try_reserve(bytes) {
                return Ok(charge);
            }
            if state.queue.is_empty() {
                // Nothing to wait for: the budget's parent is exhausted.
                return self
                    .budget
                    .try_reserve(bytes)
                    .map_err(|error| StorageError::ResourceExhausted(error.to_string()));
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Make `commit` visible to readers that snapshot the gap from now on.
    pub(crate) fn push(
        &self,
        ci: u64,
        base: Arc<Vec<Op>>,
        derived: Vec<Op>,
        usage: Vec<Op>,
        charge: ByteReservation,
    ) {
        let commit = Arc::new(GapCommit {
            ci,
            base,
            derived,
            usage,
            charge,
        });
        let mut state = self.lock();
        debug_assert!(ci > state.top, "gap commits arrive in commit order");
        for (key, version) in commit.versions() {
            let key: Arc<[u8]> = Arc::from(key);
            let versions = match state.index.get(&key) {
                Some(older) => older.iter().cloned().chain([version]).collect(),
                None => Arc::from([version]),
            };
            state.index.insert(key, versions);
        }
        state.top = ci;
        state.queue.push_back(commit);
        self.publish(&state.index);
        drop(state);
        self.changed.notify_all();
    }

    /// The next commits to materialize, oldest first, at most `max_bytes`
    /// of them (at least one). Blocks while the gap is empty or paused;
    /// `None` once stopping and drained.
    pub(crate) fn next_batch(&self, max_bytes: usize) -> Option<Vec<Arc<GapCommit>>> {
        let mut state = self.lock();
        loop {
            if !state.paused && !state.queue.is_empty() {
                let mut bytes = 0usize;
                let mut batch = Vec::new();
                for commit in &state.queue {
                    let size = commit.charge.bytes();
                    if !batch.is_empty() && bytes + size > max_bytes {
                        break;
                    }
                    bytes += size;
                    batch.push(Arc::clone(commit));
                }
                return Some(batch);
            }
            if state.stopping && (state.queue.is_empty() || state.failure.is_some()) {
                return None;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The first `count` queued commits are in redb: drop them and their
    /// versions.
    pub(crate) fn materialized(&self, count: usize) {
        let mut state = self.lock();
        let done: Vec<Arc<GapCommit>> = state.queue.drain(..count).collect();
        let Some(last) = done.last().map(|commit| commit.ci) else {
            return;
        };
        for commit in &done {
            for key in commit.versions().into_keys() {
                let Some(versions) = state.index.get(key) else {
                    continue;
                };
                let kept: Arc<[Version]> =
                    versions.iter().filter(|v| v.ci > last).cloned().collect();
                if kept.is_empty() {
                    state.index.remove(key);
                } else if kept.len() != versions.len() {
                    state.index.insert(Arc::from(key), kept);
                }
            }
        }
        self.publish(&state.index);
        // Release the charges before a waiter can see the queue shorter.
        drop(done);
        drop(state);
        self.changed.notify_all();
    }

    /// The materializer failed: fail every waiter and fence publication.
    pub(crate) fn fail(&self, error: String) {
        self.lock().failure.get_or_insert(error);
        self.changed.notify_all();
    }

    /// The newest commit index `c` such that every commit of this shard at
    /// or below `c` is in redb (`u64::MAX`: nothing queued).
    pub(crate) fn materialized_through(&self) -> u64 {
        self.lock()
            .queue
            .front()
            .map_or(u64::MAX, |commit| commit.ci - 1)
    }

    /// Wait until every commit at or below `ci` is materialized (or the
    /// materializer failed).
    pub(crate) fn wait_materialized(&self, ci: u64) -> Result<(), StorageError> {
        let mut state = self.lock();
        loop {
            if let Some(failure) = &state.failure {
                return Err(StorageError::Durability(failure.clone()));
            }
            if state.queue.front().is_none_or(|commit| commit.ci > ci) {
                return Ok(());
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Bytes the gap holds.
    pub(crate) fn bytes(&self) -> usize {
        self.budget.snapshot().used
    }

    /// Ask the materializer to finish the queue and exit.
    pub(crate) fn stop(&self) {
        let mut state = self.lock();
        state.stopping = true;
        state.paused = false;
        drop(state);
        self.changed.notify_all();
    }

    /// Hold (or release) materialization, so tests can read across a gap.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_paused(&self, paused: bool) {
        self.lock().paused = paused;
        self.changed.notify_all();
    }
}

/// A shard's redb view overlaid with the gap's versions newer than it.
pub(crate) struct GapView {
    base: Box<dyn StorageReadView>,
    index: GapIndex,
    /// The base view's commit index: versions at or below it are in it.
    base_ci: u64,
}

impl GapView {
    /// `index` must be snapshotted before `base` was opened.
    pub(crate) fn new(
        base: Box<dyn StorageReadView>,
        index: GapIndex,
    ) -> Result<Self, StorageError> {
        let base_ci = base.visible_ci()?;
        Ok(Self {
            base,
            index,
            base_ci,
        })
    }

    /// `key`'s versions newer than the base view, oldest first.
    fn versions(&self, key: &[u8]) -> Option<impl DoubleEndedIterator<Item = &Version>> {
        let base_ci = self.base_ci;
        self.index
            .get(key)
            .map(move |versions| versions.iter().filter(move |v| v.ci > base_ci))
    }

    /// The newest gap version of `key`, if any.
    fn newest(&self, key: &[u8]) -> Option<&Version> {
        self.versions(key)
            .and_then(|mut versions| versions.next_back())
    }

    /// What the gap says about `key` at snapshot `ci`. redb's apply moves a superseded value into history, and a Set or
    /// Purge drops all of a key's history: so a gap commit newer than `ci`
    /// that erased the key hides every older version, and otherwise the
    /// newest gap version at or below `ci` is the answer.
    fn at(&self, key: &[u8], ci: u64) -> GapRead<'_> {
        let Some(versions) = self.versions(key) else {
            return GapRead::Base;
        };
        let versions: Vec<&Version> = versions.collect();
        if versions.iter().any(|v| v.ci > ci && v.erases) {
            return GapRead::Absent;
        }
        versions
            .iter()
            .rev()
            .find(|v| v.ci <= ci)
            .map_or(GapRead::Base, |v| GapRead::from(v.value.as_deref()))
    }
}

/// The gap's answer to a point read.
enum GapRead<'a> {
    /// The gap holds the visible value.
    Value(&'a [u8]),
    /// The gap decides the key is not visible.
    Absent,
    /// The gap says nothing: the base view answers.
    Base,
}

impl<'a> From<Option<&'a [u8]>> for GapRead<'a> {
    fn from(value: Option<&'a [u8]>) -> Self {
        value.map_or(Self::Absent, Self::Value)
    }
}

/// A base range with the gap's newest values merged in (the gap wins).
struct Overlaid<'a> {
    base: std::iter::Peekable<
        Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>,
    >,
    gap: std::iter::Peekable<Box<dyn Iterator<Item = (&'a [u8], Option<&'a [u8]>)> + 'a>>,
    limits: ReadLimits,
    projection: ScanProjection,
}

impl Iterator for Overlaid<'_> {
    type Item = Result<(Vec<u8>, Vec<u8>), StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let gap_first = match (self.base.peek(), self.gap.peek()) {
                (Some(Err(_)), _) => return self.base.next(),
                (None, None) => return None,
                (None, Some(_)) => true,
                (Some(_), None) => false,
                (Some(Ok((base_key, _))), Some((gap_key, _))) => {
                    if base_key.as_slice() == *gap_key {
                        // The gap's version replaces the base row.
                        self.base.next();
                        true
                    } else {
                        *gap_key < base_key.as_slice()
                    }
                }
            };
            if !gap_first {
                return self.base.next();
            }
            let (key, value) = self.gap.next()?;
            let Some(value) = value else {
                continue;
            };
            crate::read_accounting::record("row", key);
            if self.projection == ScanProjection::Keys {
                return Some(
                    self.limits
                        .check_item(key.len(), 0)
                        .map(|()| (key.to_vec(), Vec::new())),
                );
            }
            return Some(
                self.limits
                    .check_item(key.len(), value.len())
                    .map(|()| (key.to_vec(), value.to_vec())),
            );
        }
    }
}

fn visit_projected(
    key: &[u8],
    value: &[u8],
    limits: ReadLimits,
    projection: ScanProjection,
    visit: &mut dyn FnMut(&[u8]),
) -> Result<(), StorageError> {
    if projection == ScanProjection::Keys {
        visit(&[]);
    } else {
        limits.check_item(key.len(), value.len())?;
        visit(value);
    }
    Ok(())
}

fn as_slices(bound: &Bound<Vec<u8>>) -> Bound<&[u8]> {
    bound.as_ref().map(Vec::as_slice)
}

impl StorageReadView for GapView {
    fn visit_value(
        &self,
        key: &[u8],
        ci: u64,
        current: bool,
        limits: ReadLimits,
        projection: ScanProjection,
        visit: &mut dyn FnMut(&[u8]),
    ) -> Result<bool, StorageError> {
        let decided = if current {
            self.newest(key)
                .map_or(GapRead::Base, |v| GapRead::from(v.value.as_deref()))
        } else {
            self.at(key, ci)
        };
        match decided {
            GapRead::Value(value) => {
                crate::read_accounting::record("get", key);
                visit_projected(key, value, limits, projection, visit)?;
                Ok(true)
            }
            GapRead::Absent => {
                crate::read_accounting::record("get", key);
                Ok(false)
            }
            // Every base version is at or below `base_ci`, so a snapshot at
            // or above it reads the base's current rows.
            GapRead::Base => self.base.visit_value(
                key,
                ci,
                current || ci >= self.base_ci,
                limits,
                projection,
                visit,
            ),
        }
    }

    fn next_key(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        include_history: bool,
        direction: ScanDirection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let base = self
            .base
            .next_key(lower, upper, limits, include_history, direction)?;
        // Any key a gap commit writes is a candidate: callers check each
        // candidate's visibility at their snapshot.
        let mut range = self
            .index
            .range::<_, [u8]>((as_slices(lower), as_slices(upper)));
        let gap = match direction {
            ScanDirection::Forward => range.next(),
            ScanDirection::Reverse => range.next_back(),
        }
        .map(|(key, _)| key.to_vec());
        if let Some(key) = &gap {
            limits.check_item(key.len(), 0)?;
        }
        Ok(direction.first(base, gap))
    }

    fn current_range<'a>(
        &'a self,
        start: &[u8],
        end: &[u8],
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>, StorageError>
    {
        let base = self.base.current_range(start, end, limits, projection)?;
        let lower = Bound::Included(start.to_vec());
        let upper = if end.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(end.to_vec())
        };
        Ok(self.overlaid_owned(base, &lower, &upper, limits, projection))
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
        let Some(base) = self.base.native_range(lower, upper, limits, projection)? else {
            return Ok(None);
        };
        Ok(Some(
            self.overlaid_owned(base, lower, upper, limits, projection),
        ))
    }

    fn visible_ci(&self) -> Result<u64, StorageError> {
        match self.newest(crate::LAST_APPLIED_KEY) {
            Some(Version {
                value: Some(raw), ..
            }) => crate::wal::decode_last_applied(raw).map_err(|_| StorageError::CorruptLog),
            _ => Ok(self.base_ci),
        }
    }

    fn history_floor(&self) -> Result<u64, StorageError> {
        self.base.history_floor()
    }

    fn snapshot_pinned(&self) -> bool {
        self.base.snapshot_pinned()
    }
}

impl GapView {
    /// A base range with the gap's newest values in `(lower, upper)`
    /// merged in.
    fn overlaid_owned<'a>(
        &'a self,
        base: Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a> {
        if self.index.is_empty() {
            return base;
        }
        let base_ci = self.base_ci;
        let gap = self
            .index
            .range::<_, [u8]>((as_slices(lower), as_slices(upper)))
            .filter_map(move |(key, versions)| {
                let newest = versions.last().filter(|v| v.ci > base_ci)?;
                Some((&**key, newest.value.as_deref()))
            });
        Box::new(Overlaid {
            base: base.peekable(),
            gap: (Box::new(gap) as Box<dyn Iterator<Item = _> + 'a>).peekable(),
            limits,
            projection,
        })
    }
}
