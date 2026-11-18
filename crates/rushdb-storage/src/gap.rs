//! Publish at WAL durability (PUB-1): commits published but not yet
//! materialized into a shard's redb file.
//!
//! The publisher makes a commit visible once its WAL record is durable and
//! pushes its ops (base and derived) into its shard's [`Gap`]. The shard's materializer applies them to redb
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
//!
//! A tenant's usage-bytes row is written by the apply itself, from the
//! values each op replaces. The gap computes it only when a reader asks
//! (the usage report is its only reader): the first read through a view
//! computes the rows of every older gap commit with exactly the apply's
//! rules ([`crate::usage::commit_rows`]) and caches them on each commit.

use std::borrow::Cow;
use std::collections::{BTreeMap, VecDeque};
use std::ops::Bound;
use std::sync::OnceLock;
use std::sync::{Arc, Condvar, Mutex, PoisonError, RwLock};

use crate::backend::StorageReadView;
use crate::read_view::{ReadLimits, ScanDirection, ScanProjection};
use crate::resources::{ByteBudget, ByteReservation};
use crate::{Op, StorageError};

/// `Version::op` of a tenant's usage row: computed on demand.
const USAGE: usize = usize::MAX;

/// One commit's effect on one key: the key's last op in the commit, read
/// in place (or, for a usage row, the commit's computed usage).
#[derive(Clone)]
pub(crate) struct Version {
    pub(crate) ci: u64,
    commit: Arc<GapCommit>,
    op: usize,
    /// The commit Set or Purged the key: redb drops the key's history then,
    /// so no snapshot older than this commit sees any version of it.
    pub(crate) erases: bool,
}

/// Versions of every key the gap's commits write, oldest first per key.
type GapIndex = im::OrdMap<GapKey, Arc<[Version]>>;

/// Key bytes a [`GapKey`] keeps inline.
const INLINE: usize = 64;

/// A gap index key that keeps its first [`INLINE`] bytes inside the index
/// node: a descent compares node memory and reads the heap copy only when
/// two keys share their first [`INLINE`] bytes (index keys are mostly
/// shorter). With every comparison behind a pointer, the check a merge
/// import's index lookups make against the gap cost as much as the apply
/// the gap takes off their path.
#[derive(Clone)]
pub(crate) struct GapKey {
    head: [u8; INLINE],
    full: Arc<[u8]>,
}

impl GapKey {
    fn new(full: Arc<[u8]>) -> Self {
        let mut head = [0u8; INLINE];
        let inline = full.len().min(INLINE);
        head[..inline].copy_from_slice(&full[..inline]);
        Self { head, full }
    }

    /// A key to look `key` up with.
    fn probe(key: &[u8]) -> Self {
        Self::new(Arc::from(key))
    }

    fn bytes(&self) -> &[u8] {
        &self.full
    }

    fn head(&self) -> &[u8] {
        &self.head[..self.full.len().min(INLINE)]
    }
}

impl Ord for GapKey {
    /// Byte order of the full keys.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.head().cmp(other.head()) {
            std::cmp::Ordering::Equal if self.full.len() > INLINE && other.full.len() > INLINE => {
                self.full[INLINE..].cmp(&other.full[INLINE..])
            }
            // Equal heads, one key no longer than them: the shorter first.
            std::cmp::Ordering::Equal => self.full.len().cmp(&other.full.len()),
            unequal => unequal,
        }
    }
}

impl PartialOrd for GapKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for GapKey {
    fn eq(&self, other: &Self) -> bool {
        self.full == other.full
    }
}

impl Eq for GapKey {}

/// Owned probe bounds for an index range.
fn probes(lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> (Bound<GapKey>, Bound<GapKey>) {
    (lower.map(GapKey::probe), upper.map(GapKey::probe))
}

/// What a reader sees of a gap: its versions and its commits, oldest first.
#[derive(Clone, Default)]
pub(crate) struct GapSnapshot {
    index: GapIndex,
    commits: im::Vector<Arc<GapCommit>>,
}

impl GapSnapshot {
    pub(crate) fn is_empty(&self) -> bool {
        self.commits.is_empty()
    }
}

/// A commit's usage rows after it: every usage-row key of the tenants it
/// touches, with its value (`None`: absent).
type UsageAfter = BTreeMap<Vec<u8>, Option<Vec<u8>>>;

/// A published commit waiting for its shard's materializer.
pub(crate) struct GapCommit {
    pub(crate) ci: u64,
    pub(crate) base: Arc<Vec<Op>>,
    pub(crate) derived: Vec<Op>,
    /// The usage rows after this commit, once a reader computed them.
    usage: OnceLock<Result<UsageAfter, StorageError>>,
    /// What the commit charges against the gap's budget.
    bytes: usize,
    /// Every key the commit writes, as the gap's index holds it.
    keys: Vec<GapKey>,
}

impl GapCommit {
    /// The usage rows this commit's tenants get.
    #[cfg(debug_assertions)]
    pub(crate) fn usage_keys(&self) -> impl Iterator<Item = &[u8]> {
        self.keys
            .iter()
            .map(GapKey::bytes)
            .filter(|key| crate::usage::is_usage_bytes_key(key))
    }

    fn ops(&self) -> impl Iterator<Item = &Op> {
        self.base.iter().chain(&self.derived)
    }

    /// The `index`-th op of [`Self::ops`].
    fn op(&self, index: usize) -> &Op {
        match index.checked_sub(self.base.len()) {
            None => &self.base[index],
            Some(at) => &self.derived[at],
        }
    }

    /// Per key the commit writes, in key order: its last op (or [`USAGE`]
    /// for the usage row of every tenant it touches) and whether any of its
    /// ops erased the key's history.
    fn effects(&self) -> BTreeMap<Cow<'_, [u8]>, (usize, bool)> {
        let mut out: BTreeMap<Cow<'_, [u8]>, (usize, bool)> = BTreeMap::new();
        let mut tenant = None;
        for (index, op) in self.ops().enumerate() {
            let key = op.key();
            let erases = matches!(op, Op::Set(..) | Op::Purge(_));
            let index = if crate::usage::is_usage_bytes_key(key) {
                USAGE
            } else {
                // Consecutive ops mostly share a tenant: one usage-row
                // entry per run.
                let owner = rushdb_types::keys::parse_tenant(key);
                if let Some(owner) = owner.filter(|_| owner != tenant) {
                    tenant = Some(owner);
                    out.entry(Cow::Owned(rushdb_types::keys::usage_bytes_key(owner)))
                        .or_insert((USAGE, false));
                }
                index
            };
            let key = Cow::Borrowed(key);
            let effect = out.entry(key).or_insert((index, false));
            effect.0 = index;
            effect.1 |= erases;
        }
        out
    }
}

/// Bytes a commit charges against the gap's budget.
pub(crate) fn commit_bytes(base: &[Op], derived: &[Op]) -> usize {
    // Each key is held twice (the op and the index entry) plus per-entry
    // bookkeeping.
    base.iter()
        .chain(derived)
        .map(|op| op.encoded_len() + op.key().len() + 64)
        .sum()
}

struct State {
    /// The writer's copy; `snapshot` is a clone of it.
    readers: GapSnapshot,
    /// Commits waiting for the materializer, with their charges (a
    /// reader's snapshot may keep a commit's ops alive, not its charge).
    queue: VecDeque<(Arc<GapCommit>, ByteReservation)>,
    /// Newest commit index pushed.
    top: u64,
    stopping: bool,
    paused: bool,
    failure: Option<String>,
}

/// One shard's published, not yet materialized commits.
pub(crate) struct Gap {
    state: Mutex<State>,
    snapshot: RwLock<GapSnapshot>,
    changed: Condvar,
    budget: ByteBudget,
    bound: usize,
}

impl Gap {
    pub(crate) fn new(budget: ByteBudget) -> Self {
        let bound = budget.snapshot().limit;
        Self {
            state: Mutex::new(State {
                readers: GapSnapshot::default(),
                queue: VecDeque::new(),
                top: 0,
                stopping: false,
                paused: false,
                failure: None,
            }),
            snapshot: RwLock::new(GapSnapshot::default()),
            changed: Condvar::new(),
            budget,
            bound,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What readers see now.
    pub(crate) fn snapshot(&self) -> GapSnapshot {
        self.snapshot
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn publish(&self, readers: &GapSnapshot) {
        *self
            .snapshot
            .write()
            .unwrap_or_else(PoisonError::into_inner) = readers.clone();
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
        charge: ByteReservation,
    ) {
        let mut commit = GapCommit {
            ci,
            base,
            derived,
            usage: OnceLock::new(),
            bytes: charge.bytes(),
            keys: Vec::new(),
        };
        let effects: Vec<(GapKey, usize, bool)> = commit
            .effects()
            .into_iter()
            .map(|(key, (op, erases))| (GapKey::new(Arc::from(&*key)), op, erases))
            .collect();
        commit.keys = effects.iter().map(|(key, ..)| key.clone()).collect();
        let commit = Arc::new(commit);
        let mut state = self.lock();
        debug_assert!(ci > state.top, "gap commits arrive in commit order");
        for (key, op, erases) in effects {
            let version = Version {
                ci,
                commit: Arc::clone(&commit),
                op,
                erases,
            };
            // One descent for a new key (the common case); a key with older
            // versions is inserted again with them.
            if let Some(older) = state
                .readers
                .index
                .insert(key.clone(), Arc::from([version.clone()]))
            {
                let versions: Arc<[Version]> = older.iter().cloned().chain([version]).collect();
                state.readers.index.insert(key, versions);
            }
        }
        state.top = ci;
        state.readers.commits.push_back(Arc::clone(&commit));
        state.queue.push_back((commit, charge));
        self.publish(&state.readers);
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
                for (commit, _) in &state.queue {
                    if !batch.is_empty() && bytes + commit.bytes > max_bytes {
                        break;
                    }
                    bytes += commit.bytes;
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
        let done: Vec<(Arc<GapCommit>, ByteReservation)> = state.queue.drain(..count).collect();
        let Some(last) = done.last().map(|(commit, _)| commit.ci) else {
            return;
        };
        for (commit, _) in &done {
            for key in &commit.keys {
                let Some(versions) = state.readers.index.get(key) else {
                    continue;
                };
                let kept: Arc<[Version]> =
                    versions.iter().filter(|v| v.ci > last).cloned().collect();
                if kept.is_empty() {
                    state.readers.index.remove(key);
                } else if kept.len() != versions.len() {
                    state.readers.index.insert(key.clone(), kept);
                }
            }
            state.readers.commits.pop_front();
        }
        self.publish(&state.readers);
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
            .map_or(u64::MAX, |(commit, _)| commit.ci - 1)
    }

    /// Wait until every commit at or below `ci` is materialized (or the
    /// materializer failed).
    pub(crate) fn wait_materialized(&self, ci: u64) -> Result<(), StorageError> {
        let mut state = self.lock();
        loop {
            if let Some(failure) = &state.failure {
                return Err(StorageError::Durability(failure.clone()));
            }
            if state.queue.front().is_none_or(|(commit, _)| commit.ci > ci) {
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
    commits: im::Vector<Arc<GapCommit>>,
    /// The base view's commit index: versions at or below it are in it.
    base_ci: u64,
}

impl GapView {
    /// `gap` must be snapshotted before `base` was opened.
    pub(crate) fn new(
        base: Box<dyn StorageReadView>,
        gap: GapSnapshot,
    ) -> Result<Self, StorageError> {
        let base_ci = base.visible_ci()?;
        Ok(Self {
            base,
            index: gap.index,
            commits: gap.commits,
            base_ci,
        })
    }

    /// `key`'s value after `version`'s commit (`None`: absent).
    fn value_of<'a>(
        &'a self,
        key: &[u8],
        version: &'a Version,
    ) -> Result<Option<&'a [u8]>, StorageError> {
        if version.op != USAGE {
            return Ok(match version.commit.op(version.op) {
                Op::Put(_, value) | Op::Set(_, value) => Some(value),
                Op::Del(_) | Op::Purge(_) => None,
            });
        }
        let after = self.usage_after(&version.commit)?;
        Ok(after.get(key).and_then(Option::as_deref))
    }

    /// The usage rows after `commit`, computing (and caching on each
    /// commit) those of every older commit of this view first: each one's
    /// rows depend on the rows before it.
    fn usage_after<'a>(&self, commit: &'a GapCommit) -> Result<&'a UsageAfter, StorageError> {
        if let Some(after) = commit.usage.get() {
            return after.as_ref().map_err(Clone::clone);
        }
        for older in &self.commits {
            if older.ci <= self.base_ci || older.usage.get().is_some() {
                continue;
            }
            if older.ci > commit.ci {
                break;
            }
            let computed = self.compute_usage(older);
            // A concurrent reader may have stored the same rows first.
            let _ = older.usage.set(computed);
        }
        commit
            .usage
            .get()
            .ok_or(StorageError::CorruptLog)?
            .as_ref()
            .map_err(Clone::clone)
    }

    /// What the apply writes to the usage rows of `commit`'s tenants.
    fn compute_usage(&self, commit: &GapCommit) -> Result<UsageAfter, StorageError> {
        let before = BeforeCommit {
            view: self,
            ci: commit.ci,
        };
        let rows: BTreeMap<Vec<u8>, Option<Vec<u8>>> =
            crate::usage::commit_rows(commit.ops(), &before)?
                .into_iter()
                .map(|op| match op {
                    Op::Put(key, value) | Op::Set(key, value) => (key, Some(value)),
                    Op::Del(key) | Op::Purge(key) => (key, None),
                })
                .collect();
        let mut after = UsageAfter::new();
        for key in &commit.keys {
            let key = key.bytes();
            if !crate::usage::is_usage_bytes_key(key) {
                continue;
            }
            let value = if let Some(value) = rows.get(key) {
                value.clone()
            } else {
                // No delta: the row keeps its value, or what the commit
                // wrote to it directly (a purge).
                match commit.ops().filter(|op| op.key() == key).last() {
                    Some(Op::Put(_, value) | Op::Set(_, value)) => Some(value.clone()),
                    Some(Op::Del(_) | Op::Purge(_)) => None,
                    None => crate::DeriveRead::get(&before, key)?,
                }
            };
            after.insert(key.to_vec(), value);
        }
        Ok(after)
    }

    /// `key`'s current value once commit `ci` is applied.
    #[cfg(debug_assertions)]
    pub(crate) fn current_at(&self, key: &[u8], ci: u64) -> Result<Option<Vec<u8>>, StorageError> {
        self.before(key, ci + 1)
    }

    /// `key`'s current value before commit `ci` in this view.
    fn before(&self, key: &[u8], ci: u64) -> Result<Option<Vec<u8>>, StorageError> {
        let older = self
            .versions(key)
            .and_then(|mut versions| versions.rfind(|v| v.ci < ci));
        match older {
            Some(version) => Ok(self.value_of(key, version)?.map(<[u8]>::to_vec)),
            None => self.base.read_value(
                key,
                0,
                true,
                ReadLimits {
                    max_item_bytes: usize::MAX,
                    ..ReadLimits::default()
                },
                ScanProjection::KeyValue,
            ),
        }
    }

    /// `key`'s versions newer than the base view, oldest first.
    fn versions(&self, key: &[u8]) -> Option<impl DoubleEndedIterator<Item = &Version>> {
        let base_ci = self.base_ci;
        self.index
            .get(&GapKey::probe(key))
            .map(move |versions| versions.iter().filter(move |v| v.ci > base_ci))
    }

    /// The newest gap version of `key`, if any.
    fn newest(&self, key: &[u8]) -> Option<&Version> {
        self.versions(key)
            .and_then(|mut versions| versions.next_back())
    }

    /// Whether a gap key lies within `(lower, upper)`. Most of a merge
    /// import's lookups scan small ranges the gap holds nothing in, so this
    /// runs before every overlaid scan: one descent of the index, no
    /// allocation. `im`'s `get_next` can miss a parent's key when a child
    /// holds only smaller ones (a false `None`, never a wrong key), so a
    /// `None` is confirmed with a range.
    fn may_hold(&self, lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> bool {
        let found = match lower {
            Bound::Unbounded => self.index.get_min().map(|(key, _)| key),
            Bound::Included(start) | Bound::Excluded(start) => {
                let probe = GapKey::probe(start);
                match self.index.get_next(&probe) {
                    // The bound itself when excluded: let the range decide.
                    Some((key, _)) if matches!(lower, Bound::Excluded(_)) && *key == probe => {
                        return true;
                    }
                    Some((key, _)) => Some(key),
                    None => self
                        .index
                        .range((lower.map(GapKey::probe), Bound::Unbounded))
                        .next()
                        .map(|(key, _)| key),
                }
            }
        };
        let Some(first) = found else {
            return false;
        };
        match upper {
            Bound::Unbounded => true,
            Bound::Included(end) => first.bytes() <= end,
            Bound::Excluded(end) => first.bytes() < end,
        }
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
            .map_or(GapRead::Base, |v| GapRead::Version(v))
    }
}

/// The state before one gap commit, as the apply sees it.
struct BeforeCommit<'a> {
    view: &'a GapView,
    ci: u64,
}

impl crate::DeriveRead for BeforeCommit<'_> {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.view.before(key, self.ci)
    }

    fn range_prefix(&self, _prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        Err(StorageError::InvalidRead(
            "usage rows read single keys".into(),
        ))
    }

    fn range(&self, _start: &[u8], _end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        Err(StorageError::InvalidRead(
            "usage rows read single keys".into(),
        ))
    }
}

/// The gap's answer to a point read.
enum GapRead<'a> {
    /// The value after this version's commit decides the read.
    Version(&'a Version),
    /// The gap decides the key is not visible.
    Absent,
    /// The gap says nothing: the base view answers.
    Base,
}

/// A base range with the gap's newest values merged in (the gap wins).
struct Overlaid<'a> {
    view: &'a GapView,
    base: std::iter::Peekable<
        Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>,
    >,
    gap: std::iter::Peekable<Box<dyn Iterator<Item = (&'a [u8], &'a Version)> + 'a>>,
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
            let (key, version) = self.gap.next()?;
            let value = match self.view.value_of(key, version) {
                Ok(Some(value)) => value,
                Ok(None) => continue,
                Err(error) => return Some(Err(error)),
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
            self.newest(key).map_or(GapRead::Base, GapRead::Version)
        } else {
            self.at(key, ci)
        };
        match decided {
            GapRead::Version(version) => {
                crate::read_accounting::record("get", key);
                let Some(value) = self.value_of(key, version)? else {
                    return Ok(false);
                };
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
        if !self.may_hold(as_slices(lower), as_slices(upper)) {
            return Ok(base);
        }
        // Any key a gap commit writes is a candidate: callers check each
        // candidate's visibility at their snapshot.
        let mut range = self.index.range(probes(as_slices(lower), as_slices(upper)));
        let gap = match direction {
            ScanDirection::Forward => range.next(),
            ScanDirection::Reverse => range.next_back(),
        }
        .map(|(key, _)| key.bytes().to_vec());
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
        let newest = match self.newest(crate::LAST_APPLIED_KEY) {
            Some(version) => self.value_of(crate::LAST_APPLIED_KEY, version)?,
            None => None,
        };
        match newest {
            Some(raw) => crate::wal::decode_last_applied(raw).map_err(|_| StorageError::CorruptLog),
            None => Ok(self.base_ci),
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
        if !self.may_hold(as_slices(lower), as_slices(upper)) {
            return base;
        }
        let base_ci = self.base_ci;
        let gap = self
            .index
            .range(probes(as_slices(lower), as_slices(upper)))
            .filter_map(move |(key, versions)| {
                let newest = versions.last().filter(|v| v.ci > base_ci)?;
                Some((key.bytes(), newest))
            });
        Box::new(Overlaid {
            view: self,
            base: base.peekable(),
            gap: (Box::new(gap) as Box<dyn Iterator<Item = _> + 'a>).peekable(),
            limits,
            projection,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::GapKey;

    #[test]
    fn gap_keys_order_as_their_bytes() {
        // Keys around the inline length over a tiny alphabet, so heads tie
        // and one key is often a prefix of another.
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let keys: Vec<Vec<u8>> = (0..600)
            .map(|_| {
                let len = usize::try_from(next() % 49).unwrap();
                (0..len)
                    .map(|_| u8::try_from(next() % 3).unwrap())
                    .collect()
            })
            .collect();
        for a in &keys {
            for b in &keys {
                assert_eq!(
                    GapKey::probe(a).cmp(&GapKey::probe(b)),
                    a.cmp(b),
                    "{a:?} vs {b:?}"
                );
            }
        }
    }
}
