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
//! publisher adds and the materializer trims. A commit that writes many
//! keys (a bulk import's) goes in as one sorted run instead: inserting its
//! keys one by one cost the publisher more than the redb apply the gap
//! takes off its path, and the materializer drops a run whole. Each run
//! keeps a Bloom filter of its key prefixes, so a lookup the run holds
//! nothing for skips its search. A reader snapshots the gap
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
use std::cell::OnceCell;
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

/// Commits that write at least this many keys enter the gap as a [`Run`].
/// Few commits are that large, so a read checks few runs; tests use a small
/// threshold so both forms meet in every scenario.
const RUN_KEYS: usize = if cfg!(test) { 6 } else { 16_384 };

/// Versions of every key the gap's commits write.
#[derive(Clone, Default)]
struct GapIndex {
    /// Small commits' versions, oldest first per key.
    map: im::OrdMap<GapKey, Arc<[Version]>>,
    /// Large commits, one run each, oldest first.
    runs: im::Vector<Arc<Run>>,
}

/// One large commit's versions, sorted by key, one per key.
struct Run {
    ci: u64,
    entries: Box<[(GapKey, Version)]>,
    filter: PrefixFilter,
}

impl Run {
    fn new(ci: u64, entries: Box<[(GapKey, Version)]>) -> Self {
        let filter = PrefixFilter::from_sorted_keys(entries.iter().map(|(key, _)| key.bytes()));
        Self {
            ci,
            entries,
            filter,
        }
    }

    /// Whether a key starting with `prefix` may be in the run.
    fn may_start_with(&self, prefix: &[u8]) -> bool {
        self.filter.may_start_with(prefix)
    }

    fn get(&self, probe: &GapKey) -> Option<&Version> {
        self.entries
            .binary_search_by(|(key, _)| key.cmp(probe))
            .ok()
            .map(|at| &self.entries[at].1)
    }

    /// The entries with keys in `(lower, upper)`.
    fn range(&self, lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> &[(GapKey, Version)] {
        let start = match lower {
            Bound::Unbounded => 0,
            Bound::Included(start) => self.entries.partition_point(|(key, _)| key.bytes() < start),
            Bound::Excluded(start) => self
                .entries
                .partition_point(|(key, _)| key.bytes() <= start),
        };
        let end = match upper {
            Bound::Unbounded => self.entries.len(),
            Bound::Included(end) => self.entries.partition_point(|(key, _)| key.bytes() <= end),
            Bound::Excluded(end) => self.entries.partition_point(|(key, _)| key.bytes() < end),
        };
        self.entries.get(start..end.max(start)).unwrap_or_default()
    }
}

/// Key prefixes a run's filter holds: every distinct prefix of its keys up
/// to this many bytes. Tests use a short depth so their keys outgrow it.
const FILTER_DEPTH: usize = if cfg!(test) { 7 } else { 16 };

/// Bits a filter probes per prefix.
const FILTER_PROBES: u64 = 3;

/// A Bloom filter over the distinct key prefixes of one run, up to
/// [`FILTER_DEPTH`] bytes, at 10 to 20 bits a prefix. Every key in a range
/// starts with the bytes its bounds share, so a prefix the filter lacks
/// rules the run out without a search: most of a merge import's index
/// lookups find nothing in the gap, and binary searches of every run cost
/// them as much as the apply the gap takes off their path.
struct PrefixFilter {
    bits: Box<[u64]>,
    /// Bit count minus one (a power of two).
    mask: u64,
}

/// FNV-1a over each prefix of `key` up to [`FILTER_DEPTH`] bytes: the
/// `d`-th item hashes `key[..d + 1]`.
fn prefix_hashes(key: &[u8]) -> impl Iterator<Item = u64> + '_ {
    key.iter()
        .take(FILTER_DEPTH)
        .scan(0xCBF2_9CE4_8422_2325_u64, |hash, &byte| {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3);
            Some(*hash)
        })
}

/// The hash a filter checks for keys starting with `prefix` (that of its
/// first [`FILTER_DEPTH`] bytes; `None` for an empty prefix).
fn prefix_hash(prefix: &[u8]) -> Option<u64> {
    prefix_hashes(prefix).last()
}

/// The bit positions of one prefix hash: double hashing over a
/// splitmix64-finalized hash.
fn filter_bits(hash: u64, mask: u64) -> impl Iterator<Item = u64> {
    let mut mixed = hash;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^= mixed >> 31;
    let step = (mixed >> 32) | 1;
    (0..FILTER_PROBES).map(move |i| mixed.wrapping_add(i.wrapping_mul(step)) & mask)
}

/// How many leading bytes `a` and `b` share.
fn shared_len(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

impl PrefixFilter {
    /// The filter of `keys`, in ascending order: a key's prefixes longer
    /// than what it shares with the key before it are the new ones.
    fn from_sorted_keys<'k>(mut keys: impl Iterator<Item = &'k [u8]> + Clone) -> Self {
        let new_prefixes = |keys: &mut dyn Iterator<Item = &'k [u8]>,
                            visit: &mut dyn FnMut(&'k [u8], usize)| {
            let mut before: &[u8] = &[];
            for key in keys {
                visit(key, shared_len(before, key).min(FILTER_DEPTH));
                before = key;
            }
        };
        let mut count = 0usize;
        new_prefixes(&mut keys.clone(), &mut |key, shared| {
            count += key.len().min(FILTER_DEPTH) - shared;
        });
        let bit_count = (count.max(1) * 10).next_power_of_two().max(64);
        let mut bits = vec![0u64; bit_count / 64].into_boxed_slice();
        let mask = bit_count as u64 - 1;
        new_prefixes(&mut keys, &mut |key, shared| {
            for hash in prefix_hashes(key).skip(shared) {
                for bit in filter_bits(hash, mask) {
                    bits[(bit / 64) as usize] |= 1 << (bit % 64);
                }
            }
        });
        Self { bits, mask }
    }

    /// Whether a key starting with `prefix` may be in the filter's run
    /// (always for an empty prefix). Only the first [`FILTER_DEPTH`] bytes
    /// of `prefix` are checked.
    fn may_start_with(&self, prefix: &[u8]) -> bool {
        self.admits(prefix_hash(prefix))
    }

    /// [`Self::may_start_with`] for a [`prefix_hash`].
    fn admits(&self, hash: Option<u64>) -> bool {
        hash.is_none_or(|hash| {
            filter_bits(hash, self.mask)
                .all(|bit| self.bits[(bit / 64) as usize] & (1 << (bit % 64)) != 0)
        })
    }
}

/// The bytes every key in `(lower, upper)` starts with: the bounds' shared
/// prefix (none when either is unbounded).
fn range_prefix<'a>(lower: Bound<&'a [u8]>, upper: Bound<&[u8]>) -> &'a [u8] {
    match (lower, upper) {
        (
            Bound::Included(start) | Bound::Excluded(start),
            Bound::Included(end) | Bound::Excluded(end),
        ) => &start[..shared_len(start, end)],
        _ => &[],
    }
}

impl GapIndex {
    /// The runs of commits newer than `base_ci`, oldest first.
    fn runs_after(&self, base_ci: u64) -> impl DoubleEndedIterator<Item = &Run> {
        self.runs
            .iter()
            .map(|run| &**run)
            .filter(move |run| run.ci > base_ci)
    }

    /// The map's versions of `key` (none without a descent when the map is
    /// empty, as it mostly is during a bulk load).
    fn in_map(&self, probe: &OnceCell<GapKey>, key: &[u8]) -> Option<&Arc<[Version]>> {
        if self.map.is_empty() {
            return None;
        }
        self.map.get(probe.get_or_init(|| GapKey::probe(key)))
    }

    /// `key`'s versions newer than `base_ci`, oldest first.
    fn versions(&self, key: &[u8], base_ci: u64) -> Vec<&Version> {
        let probe = OnceCell::new();
        let mut out: Vec<&Version> = self.in_map(&probe, key).map_or_else(Vec::new, |versions| {
            versions.iter().filter(|v| v.ci > base_ci).collect()
        });
        let from_map = out.len();
        out.extend(
            self.runs_after(base_ci)
                .filter(|run| run.may_start_with(key))
                .filter_map(|run| run.get(probe.get_or_init(|| GapKey::probe(key)))),
        );
        if from_map > 0 && out.len() > from_map {
            out.sort_by_key(|v| v.ci);
        }
        out
    }

    /// `key`'s newest version newer than `base_ci`.
    fn newest(&self, key: &[u8], base_ci: u64) -> Option<&Version> {
        let probe = OnceCell::new();
        let in_map = self
            .in_map(&probe, key)
            .and_then(|versions| versions.last())
            .filter(|v| v.ci > base_ci);
        let floor = in_map.map_or(base_ci, |v| v.ci);
        self.runs
            .iter()
            .rev()
            .take_while(|run| run.ci > floor)
            .filter(|run| run.may_start_with(key))
            .find_map(|run| run.get(probe.get_or_init(|| GapKey::probe(key))))
            .or(in_map)
    }
}

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
    /// Every key the commit writes into the index's map (none for a
    /// commit in a run, which leaves the gap whole).
    keys: Vec<GapKey>,
    /// The usage rows of the tenants the commit touches.
    usage_rows: Vec<Vec<u8>>,
}

impl GapCommit {
    /// The usage rows this commit's tenants get.
    pub(crate) fn usage_keys(&self) -> impl Iterator<Item = &[u8]> {
        self.usage_rows.iter().map(Vec::as_slice)
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

/// How long a commit waits for the managed budget to free room when the
/// gap holds nothing it could wait for instead.
const ROOM_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

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
    /// empty gap and takes all of it. With nothing in the gap to wait for,
    /// the managed budget is short: request tokens and caches release bytes
    /// as requests end, so the commit waits up to [`ROOM_WAIT`] for them
    /// before it is refused.
    pub(crate) fn reserve(&self, bytes: usize) -> Result<ByteReservation, StorageError> {
        let bytes = bytes.min(self.bound);
        let mut state = self.lock();
        let mut short_since = None;
        loop {
            if let Some(failure) = &state.failure {
                return Err(StorageError::Durability(failure.clone()));
            }
            let error = match self.budget.try_reserve(bytes) {
                Ok(charge) => return Ok(charge),
                Err(error) => error,
            };
            if !state.queue.is_empty() {
                short_since = None;
                state = self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            }
            let since = *short_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= ROOM_WAIT {
                return Err(StorageError::ResourceExhausted(error.to_string()));
            }
            state = self
                .changed
                .wait_timeout(state, std::time::Duration::from_millis(5))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
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
            usage_rows: Vec::new(),
        };
        // In key order (the effects map's), as a run keeps them.
        let effects: Vec<(GapKey, usize, bool)> = commit
            .effects()
            .into_iter()
            .map(|(key, (op, erases))| (GapKey::new(Arc::from(&*key)), op, erases))
            .collect();
        commit.usage_rows = effects
            .iter()
            .map(|(key, ..)| key.bytes())
            .filter(|key| crate::usage::is_usage_bytes_key(key))
            .map(<[u8]>::to_vec)
            .collect();
        let run = effects.len() >= RUN_KEYS;
        if !run {
            commit.keys = effects.iter().map(|(key, ..)| key.clone()).collect();
        }
        let commit = Arc::new(commit);
        if run {
            let entries = effects
                .into_iter()
                .map(|(key, op, erases)| {
                    let commit = Arc::clone(&commit);
                    (
                        key,
                        Version {
                            ci,
                            commit,
                            op,
                            erases,
                        },
                    )
                })
                .collect();
            let run = Arc::new(Run::new(ci, entries));
            let mut state = self.lock();
            debug_assert!(ci > state.top, "gap commits arrive in commit order");
            state.readers.index.runs.push_back(run);
            self.enqueue(state, commit, charge);
            return;
        }
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
                .map
                .insert(key.clone(), Arc::from([version.clone()]))
            {
                let versions: Arc<[Version]> = older.iter().cloned().chain([version]).collect();
                state.readers.index.map.insert(key, versions);
            }
        }
        self.enqueue(state, commit, charge);
    }

    /// Queue `commit`, whose versions are in the index, and publish.
    fn enqueue(
        &self,
        mut state: std::sync::MutexGuard<'_, State>,
        commit: Arc<GapCommit>,
        charge: ByteReservation,
    ) {
        state.top = commit.ci;
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
                let Some(versions) = state.readers.index.map.get(key) else {
                    continue;
                };
                let kept: Arc<[Version]> =
                    versions.iter().filter(|v| v.ci > last).cloned().collect();
                if kept.is_empty() {
                    state.readers.index.map.remove(key);
                } else if kept.len() != versions.len() {
                    state.readers.index.map.insert(key.clone(), kept);
                }
            }
            state.readers.commits.pop_front();
        }
        let mut runs = Vec::new();
        while let Some(run) = state.readers.index.runs.pop_front() {
            if run.ci > last {
                state.readers.index.runs.push_front(run);
                break;
            }
            runs.push(run);
        }
        self.publish(&state.readers);
        // Release the charges before a waiter can see the queue shorter.
        drop(done);
        drop(state);
        // A run's last snapshot may be this one: free it off the lock.
        drop(runs);
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
        for key in commit.usage_keys() {
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
        let versions = self.index.versions(key, self.base_ci);
        match versions.into_iter().rfind(|v| v.ci < ci) {
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

    /// The newest gap version of `key`, if any.
    fn newest(&self, key: &[u8]) -> Option<&Version> {
        self.index.newest(key, self.base_ci)
    }

    /// Whether a gap key lies within `(lower, upper)`. Most of a merge
    /// import's lookups scan small ranges the gap holds nothing in, so this
    /// runs before every overlaid scan: one descent of the index, no
    /// allocation. `im`'s `get_next` can miss a parent's key when a child
    /// holds only smaller ones (a false `None`, never a wrong key), so a
    /// `None` is confirmed with a range. A run is searched only when its
    /// filter holds the range's prefix.
    fn may_hold(&self, lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> bool {
        self.map_may_hold(lower, upper)
            || self
                .runs_within(lower, upper)
                .any(|run| !run.range(lower, upper).is_empty())
    }

    /// The runs after the base whose filters admit keys in `(lower, upper)`.
    fn runs_within(
        &self,
        lower: Bound<&[u8]>,
        upper: Bound<&[u8]>,
    ) -> impl DoubleEndedIterator<Item = &Run> {
        let hash = prefix_hash(range_prefix(lower, upper));
        self.index
            .runs_after(self.base_ci)
            .filter(move |run| run.filter.admits(hash))
    }

    fn map_may_hold(&self, lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> bool {
        let map = &self.index.map;
        if map.is_empty() {
            return false;
        }
        let found = match lower {
            Bound::Unbounded => map.get_min().map(|(key, _)| key),
            Bound::Included(start) | Bound::Excluded(start) => {
                let probe = GapKey::probe(start);
                match map.get_next(&probe) {
                    // The bound itself when excluded: let the range decide.
                    Some((key, _)) if matches!(lower, Bound::Excluded(_)) && *key == probe => {
                        return true;
                    }
                    Some((key, _)) => Some(key),
                    None => map
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
        let versions = self.index.versions(key, self.base_ci);
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
        let (lower, upper) = (as_slices(lower), as_slices(upper));
        let mut range = self.index.map.range(probes(lower, upper));
        let in_map = match direction {
            ScanDirection::Forward => range.next(),
            ScanDirection::Reverse => range.next_back(),
        }
        .map(|(key, _)| key.bytes());
        let in_runs = self.runs_within(lower, upper).filter_map(|run| {
            let entries = run.range(lower, upper);
            match direction {
                ScanDirection::Forward => entries.first(),
                ScanDirection::Reverse => entries.last(),
            }
            .map(|(key, _)| key.bytes())
        });
        let gap = in_map
            .into_iter()
            .chain(in_runs)
            .reduce(|a, b| match direction {
                ScanDirection::Forward => a.min(b),
                ScanDirection::Reverse => a.max(b),
            })
            .map(<[u8]>::to_vec);
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
        let (lower, upper) = (as_slices(lower), as_slices(upper));
        let in_map =
            self.index
                .map
                .range(probes(lower, upper))
                .filter_map(move |(key, versions)| {
                    let newest = versions.last().filter(|v| v.ci > base_ci)?;
                    Some((key.bytes(), newest))
                });
        let mut sources: Vec<Source<'a>> =
            vec![(Box::new(in_map) as Box<dyn Iterator<Item = _> + 'a>).peekable()];
        for run in self.runs_within(lower, upper) {
            let entries = run.range(lower, upper);
            if !entries.is_empty() {
                let entries = entries.iter().map(|(key, version)| (key.bytes(), version));
                sources.push((Box::new(entries) as Box<dyn Iterator<Item = _> + 'a>).peekable());
            }
        }
        let gap: Box<dyn Iterator<Item = (&'a [u8], &'a Version)> + 'a> = if sources.len() == 1 {
            Box::new(sources.pop().into_iter().flatten())
        } else {
            Box::new(NewestOf { sources })
        };
        Box::new(Overlaid {
            view: self,
            base: base.peekable(),
            gap: gap.peekable(),
            limits,
            projection,
        })
    }
}

/// One key-ordered stream of gap versions.
type Source<'a> = std::iter::Peekable<Box<dyn Iterator<Item = (&'a [u8], &'a Version)> + 'a>>;

/// Key-ordered streams merged: each key once, with its newest version.
struct NewestOf<'a> {
    sources: Vec<Source<'a>>,
}

impl<'a> Iterator for NewestOf<'a> {
    type Item = (&'a [u8], &'a Version);

    fn next(&mut self) -> Option<Self::Item> {
        let key = self
            .sources
            .iter_mut()
            .filter_map(|source| source.peek().map(|(key, _)| *key))
            .min()?;
        self.sources
            .iter_mut()
            .filter_map(|source| source.next_if(|(at, _)| *at == key))
            .max_by_key(|(_, version)| version.ci)
    }
}

#[cfg(test)]
mod tests {
    use super::{range_prefix, GapKey, PrefixFilter, FILTER_DEPTH};
    use std::ops::Bound;

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn within(key: &[u8], lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> bool {
        let above = match lower {
            Bound::Included(start) => key >= start,
            Bound::Excluded(start) => key > start,
            Bound::Unbounded => true,
        };
        let below = match upper {
            Bound::Included(end) => key <= end,
            Bound::Excluded(end) => key < end,
            Bound::Unbounded => true,
        };
        above && below
    }

    #[test]
    fn a_run_filter_never_rules_out_a_range_that_holds_a_key() {
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let key = |state: &mut u64, max: u64| -> Vec<u8> {
            let len = usize::try_from(xorshift(state) % max).unwrap();
            (0..len)
                .map(|_| u8::try_from(xorshift(state) % 4).unwrap())
                .collect()
        };
        // Keys up to twice the filter's depth over a tiny alphabet: many
        // shared prefixes, some keys prefixes of others.
        let depth = u64::try_from(FILTER_DEPTH).unwrap();
        let mut keys: Vec<Vec<u8>> = (0..300).map(|_| key(&mut state, 2 * depth)).collect();
        keys.sort();
        keys.dedup();
        let filter = PrefixFilter::from_sorted_keys(keys.iter().map(Vec::as_slice));
        let (mut misses, mut ruled_out) = (0, 0);
        for _ in 0..20_000 {
            // Bounds that share a random part of their prefix.
            let a = key(&mut state, 2 * depth + 2);
            let shared = usize::try_from(xorshift(&mut state)).unwrap() % (a.len() + 1);
            let mut b = a[..shared].to_vec();
            b.extend(key(&mut state, depth));
            let bounds = [
                (Bound::Included(&a[..]), Bound::Included(&b[..])),
                (Bound::Excluded(&a[..]), Bound::Excluded(&b[..])),
                (Bound::Included(&a[..]), Bound::Excluded(&b[..])),
            ];
            for (lower, upper) in bounds {
                let holds = keys.iter().any(|key| within(key, lower, upper));
                let admitted = filter.may_start_with(range_prefix(lower, upper));
                assert!(admitted || !holds, "{lower:?}..{upper:?} holds a key");
                if !holds {
                    misses += 1;
                    ruled_out += usize::from(!admitted);
                }
            }
            assert!(
                filter.may_start_with(&a) || !keys.contains(&a),
                "{a:?} is a key"
            );
        }
        // Most narrow misses are ruled out; wide ranges share short
        // prefixes every key has, so they never are.
        assert!(ruled_out * 10 > misses, "{ruled_out} of {misses} ruled out");
    }

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
