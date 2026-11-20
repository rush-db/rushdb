//! Resident state: the one framework for in-memory layers that answer
//! across requests (§B7 2026-11-16), with storage as the reference.
//!
//! A layer declares a key, a value, the storage areas ([`Dep`]) an entry
//! reflects, how to build an entry from a pinned view, and its size. The
//! framework owns the rest: versions, budgets and LRU, rent-or-buy and
//! background builds, metrics, the off switch and sampled verification.
//!
//! **Version rule.** The publisher (and every replay and apply path) raises
//! the version of each area a commit writes before the commit becomes
//! visible. An entry records `V`, the version of its deps when it was built
//! or offered at a snapshot `ci ≥ V`. A reader at snapshot `r` hits iff the
//! deps' version is still `V` and `r ≥ V`; anything else is a miss and the
//! caller reads storage.
//!
//! **Immutable.** An entry is an `Arc` of a value made at one snapshot;
//! nothing gets mutable access after insert. A write to a dep makes it a
//! miss at once, and it is dropped on the next touch or by LRU.
//!
//! **Off is the reference.** A budget of 0, `resident-caches: off` or
//! `RUSHDB_RESIDENT_CACHES=off` leave no entry: storage answers everything,
//! and every result must be the same bytes as with entries.

mod deps;
#[cfg(test)]
mod tests;

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};

pub use deps::{label_hash, Dep};

use crate::cache::LogicalCache;
use crate::resources::{ByteBudget, ResourceError, StorageResources};
use crate::{ReadView, StorageError, Store};

/// Environment switch that turns every resident layer off (`off`), for
/// diagnostics and for the reference run of the test suite.
pub const ENV_SWITCH: &str = "RUSHDB_RESIDENT_CACHES";

/// Whether the environment leaves resident layers on. Tests that assert a
/// hit (a build happened, fewer storage reads) assert it only when this is
/// true; their result assertions run in both modes.
#[must_use]
pub fn residents_enabled() -> bool {
    std::env::var(ENV_SWITCH).map_or(true, |value| !value.eq_ignore_ascii_case("off"))
}

/// The budget a layer is charged to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Budget {
    /// `owner-set-cache-bytes`, a child of the logical cache.
    OwnerSets,
    /// A share of `logical-cache-bytes`.
    Logical,
    /// `planner-stats-bytes`, a child of the logical cache when it exists.
    PlannerStats,
    /// `graph-cache-bytes`.
    Graph,
    /// A share of `vector-cache-bytes`.
    Vector,
    /// `column-cache-bytes`.
    Columns,
}

/// Why a build left no entry.
#[derive(Debug)]
pub enum BuildError {
    /// The value would not fit the layer's budget.
    Refused,
    /// Storage failed or the view was cancelled.
    Storage(StorageError),
}

impl From<StorageError> for BuildError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

/// What a build may ask of the framework while it runs.
pub struct BuildCx {
    limit: usize,
}

impl BuildCx {
    /// Refuse the build as soon as the value it is making exceeds what the
    /// layer could ever keep, so it stops early instead of finishing a value
    /// that will be thrown away.
    ///
    /// # Errors
    /// Returns [`BuildError::Refused`] when `bytes` exceeds the layer's budget.
    pub fn admit(&self, bytes: usize) -> Result<(), BuildError> {
        if bytes > self.limit {
            return Err(BuildError::Refused);
        }
        Ok(())
    }
}

/// One resident layer: what an entry is and what it reflects. Everything
/// else is the framework's.
pub trait Resident: Default + Send + Sync + 'static {
    /// What an entry is looked up by.
    type Key: Hash + Eq + Clone + Send + Sync + 'static;
    /// What an entry holds. `PartialEq` is for verification.
    type Value: PartialEq + Send + Sync + 'static;
    /// Metrics label and budget name.
    const NAME: &'static str;
    /// The budget entries are charged to.
    const BUDGET: Budget;

    /// The storage areas the entry for `key` reflects, read at `view` when
    /// they depend on stored definitions.
    ///
    /// # Errors
    /// Returns the storage error of a failed read.
    fn deps(&self, key: &Self::Key, view: &ReadView) -> Result<Vec<Dep>, StorageError>;

    /// The value for `key` at `view` (`None`: not representable, storage
    /// keeps answering).
    ///
    /// # Errors
    /// Returns [`BuildError::Refused`] when the value outgrows the budget, or
    /// the storage error of a failed read.
    fn build(
        &self,
        key: &Self::Key,
        view: &ReadView,
        cx: &BuildCx,
    ) -> Result<Option<Self::Value>, BuildError>;

    /// Resident bytes of `value`.
    fn bytes(value: &Self::Value) -> usize;
}

/// Counters and occupancy of one layer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerSnapshot {
    /// The layer's name.
    pub layer: &'static str,
    /// Resident bytes.
    pub bytes: u64,
    /// Live entries.
    pub entries: u64,
    /// Lookups answered from an entry.
    pub hits: u64,
    /// Lookups that read storage.
    pub misses: u64,
    /// Builds that finished.
    pub builds: u64,
    /// Values refused by the budget.
    pub build_refused: u64,
    /// Seconds spent building.
    pub build_seconds: f64,
    /// Entries dropped because a dep was written.
    pub invalidations: u64,
    /// Entries dropped by LRU.
    pub evictions: u64,
    /// Verifications run.
    pub verify: u64,
    /// Verifications that found an entry different from storage.
    pub verify_mismatch: u64,
}

#[derive(Default)]
struct Counters {
    hits: AtomicU64,
    misses: AtomicU64,
    builds: AtomicU64,
    refused: AtomicU64,
    build_nanos: AtomicU64,
    invalidations: AtomicU64,
    verify: AtomicU64,
    verify_mismatch: AtomicU64,
}

fn bump(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

/// State every layer shares.
struct Shared {
    table: deps::VersionTable,
    enabled: AtomicBool,
    /// Verify one hit in this many (0 = never).
    verify_period: u64,
    verifying: AtomicBool,
    store: OnceLock<Weak<Store>>,
    /// Background jobs (builds, verifications) still running.
    jobs: Mutex<usize>,
    idle: Condvar,
}

impl Shared {
    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn store(&self) -> Option<Arc<Store>> {
        self.store.get()?.upgrade()
    }

    fn begin_job(&self) {
        *self
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
    }

    fn end_job(&self) {
        let mut jobs = self
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *jobs = jobs.saturating_sub(1);
        if *jobs == 0 {
            self.idle.notify_all();
        }
    }
}

/// The deps an entry keeps: the layer's, plus each of their tenants'
/// unclassified area and the global one (so a write no family claims can
/// only cause misses).
fn full_deps(mut deps: Vec<Dep>) -> Box<[Dep]> {
    let tenants: HashSet<_> = deps.iter().filter_map(|dep| dep.tenant()).collect();
    deps.extend(tenants.into_iter().map(Dep::Tenant));
    deps.push(Dep::Global);
    deps.sort_unstable();
    deps.dedup();
    deps.into_boxed_slice()
}

struct Entry<V> {
    version: u64,
    deps: Box<[Dep]>,
    value: Arc<V>,
}

/// Rent paid toward building one key at one version of its deps.
struct Rent {
    version: u64,
    rows: u64,
    /// The build at this version left no entry (refused or not
    /// representable): no new build until a dep changes.
    declined: bool,
}

/// Bound on keys with rent outstanding per layer.
const RENT_KEYS: usize = 4_096;

/// One layer's entries, budget and counters.
pub struct Layer<R: Resident> {
    resident: R,
    shared: Arc<Shared>,
    limit: usize,
    cache: Mutex<Option<LogicalCache<R::Key, Entry<R::Value>>>>,
    rent: Mutex<HashMap<R::Key, Rent>>,
    building: Mutex<HashSet<R::Key>>,
    counters: Counters,
}

fn key_hash<K: Hash>(key: &K) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

impl<R: Resident> Layer<R> {
    fn cache(&self) -> std::sync::MutexGuard<'_, Option<LogicalCache<R::Key, Entry<R::Value>>>> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Whether entries can exist at all (budget above 0 and switched on).
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.shared.enabled() && self.limit > 0
    }

    /// The layer's declaration.
    #[must_use]
    pub fn resident(&self) -> &R {
        &self.resident
    }

    /// The entry for `key`, if it holds for a reader at `view`: no write to
    /// its deps since it was made, and the view includes its version.
    #[must_use]
    pub fn get(self: &Arc<Self>, key: &R::Key, view: &ReadView) -> Option<Arc<R::Value>> {
        let found = self
            .enabled()
            .then(|| self.lookup(key, view.commit_index()))
            .flatten();
        let Some((value, version)) = found else {
            bump(&self.counters.misses);
            return None;
        };
        let hits = self.counters.hits.fetch_add(1, Ordering::Relaxed) + 1;
        self.maybe_verify(key, &value, version, hits);
        Some(value)
    }

    /// The entry for `key`, if it holds for a reader of the latest published
    /// state: no write to its deps since it was made. (Published state
    /// includes the entry's version once no write to a dep is in flight,
    /// and an in-flight one has already raised the version.)
    #[must_use]
    pub fn get_latest(&self, key: &R::Key) -> Option<Arc<R::Value>> {
        let found = self.enabled().then(|| self.lookup(key, u64::MAX)).flatten();
        if let Some((value, _)) = found {
            bump(&self.counters.hits);
            Some(value)
        } else {
            bump(&self.counters.misses);
            None
        }
    }

    fn lookup(&self, key: &R::Key, ci: u64) -> Option<(Arc<R::Value>, u64)> {
        let mut guard = self.cache();
        let cache = guard.as_mut()?;
        let entry = cache.get(key)?;
        if self.shared.table.version(&entry.deps) != entry.version {
            cache.remove(key);
            bump(&self.counters.invalidations);
            return None;
        }
        (ci >= entry.version).then(|| (Arc::clone(&entry.value), entry.version))
    }

    /// Keep `value`, computed by a reader at `view`, without a rebuild. Not
    /// kept when a dep was written after the view's snapshot.
    pub fn offer(&self, key: R::Key, view: &ReadView, value: Arc<R::Value>) {
        if !self.enabled() {
            return;
        }
        let Ok(deps) = self.resident.deps(&key, view) else {
            return;
        };
        let deps = full_deps(deps);
        let version = self.shared.table.version(&deps);
        if view.commit_index() < version {
            return;
        }
        self.insert(key, version, deps, value);
    }

    fn insert(&self, key: R::Key, version: u64, deps: Box<[Dep]>, value: Arc<R::Value>) -> bool {
        let bytes = R::bytes(&value)
            .saturating_add(std::mem::size_of_val(&*deps))
            .saturating_add(std::mem::size_of::<Entry<R::Value>>());
        let kept = bytes <= self.limit
            && self.cache().as_mut().is_some_and(|cache| {
                cache.insert(
                    key,
                    Entry {
                        version,
                        deps,
                        value,
                    },
                    bytes,
                )
            });
        if !kept {
            bump(&self.counters.refused);
        }
        kept
    }

    /// Record `rows` read through storage for `key` by a reader at `view`.
    /// Once the rows read since the deps last changed reach `build_rows`
    /// (the cost of one build), the entry is built in the background at a
    /// fresh view; the reader keeps its storage answer.
    pub fn rent(self: &Arc<Self>, key: &R::Key, view: &ReadView, rows: u64, build_rows: u64) {
        if !self.enabled() || self.shared.store.get().is_none() {
            return;
        }
        // Already resident for this reader: nothing to pay for.
        if self.lookup(key, view.commit_index()).is_some() {
            return;
        }
        let Ok(deps) = self.resident.deps(key, view) else {
            return;
        };
        let version = self.shared.table.version(&full_deps(deps));
        if view.commit_index() < version {
            return;
        }
        let paid = {
            let mut rent = self
                .rent
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if rent.len() >= RENT_KEYS && !rent.contains_key(key) {
                rent.clear();
            }
            let spent = rent.entry(key.clone()).or_insert(Rent {
                version,
                rows: 0,
                declined: false,
            });
            if spent.version != version {
                *spent = Rent {
                    version,
                    rows: 0,
                    declined: false,
                };
            }
            spent.rows = spent.rows.saturating_add(rows);
            !spent.declined && spent.rows >= build_rows
        };
        if paid {
            self.spawn_build(key.clone());
        }
    }

    fn spawn_build(self: &Arc<Self>, key: R::Key) {
        let Some(store) = self.shared.store() else {
            return;
        };
        if !self
            .building
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key.clone())
        {
            return;
        }
        self.shared.begin_job();
        let layer = Arc::clone(self);
        let key_for_cleanup = key.clone();
        let spawned = std::thread::Builder::new()
            .name("resident-build".into())
            .spawn(move || {
                // Builds run on a helper of the shared query thread budget.
                let _lease = crate::parallel::background_lease();
                layer.build_now(&store, &key);
                layer.finish_build(&key);
                layer.shared.end_job();
            });
        if spawned.is_err() {
            self.finish_build(&key_for_cleanup);
            self.shared.end_job();
        }
    }

    fn finish_build(&self, key: &R::Key) {
        self.building
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
    }

    fn settle_rent(&self, key: &R::Key, version: u64, kept: bool) {
        let mut rent = self
            .rent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if kept {
            rent.remove(key);
        } else if let Some(spent) = rent.get_mut(key) {
            if spent.version == version {
                spent.declined = true;
            }
        }
    }

    /// Build `key` at a fresh view of `store` and keep it when it fits.
    fn build_now(&self, store: &Store, key: &R::Key) {
        let Ok(view) = store.read_view() else {
            return;
        };
        let Ok(deps) = self.resident.deps(key, &view) else {
            return;
        };
        let deps = full_deps(deps);
        let version = self.shared.table.version(&deps);
        if view.commit_index() < version || !self.enabled() {
            // A write to a dep is publishing; the next rent starts over.
            return;
        }
        let started = std::time::Instant::now();
        let built = self
            .resident
            .build(key, &view, &BuildCx { limit: self.limit });
        self.counters.build_nanos.fetch_add(
            u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        if self.shared.table.version(&deps) != version {
            // A dep was written while the build ran: the value may predate
            // it. Keep nothing; rent starts over at the new version.
            return;
        }
        match built {
            Ok(Some(value)) => {
                bump(&self.counters.builds);
                let kept = self.insert(key.clone(), version, deps, Arc::new(value));
                self.settle_rent(key, version, kept);
            }
            Ok(None) => {
                bump(&self.counters.builds);
                self.settle_rent(key, version, false);
            }
            Err(BuildError::Refused) => {
                bump(&self.counters.refused);
                self.settle_rent(key, version, false);
            }
            Err(BuildError::Storage(error)) => {
                tracing::debug!(layer = R::NAME, key = key_hash(key), %error, "resident build failed");
            }
        }
    }

    fn maybe_verify(
        self: &Arc<Self>,
        key: &R::Key,
        value: &Arc<R::Value>,
        version: u64,
        hits: u64,
    ) {
        let period = self.shared.verify_period;
        if period == 0 || !hits.is_multiple_of(period) {
            return;
        }
        let Some(store) = self.shared.store() else {
            return;
        };
        // One verification at a time, so a view is never held for long.
        if self.shared.verifying.swap(true, Ordering::SeqCst) {
            return;
        }
        self.shared.begin_job();
        let (layer, key, value) = (Arc::clone(self), key.clone(), Arc::clone(value));
        let spawned = std::thread::Builder::new()
            .name("resident-verify".into())
            .spawn(move || {
                layer.verify(&store, &key, &value, version);
                layer.shared.verifying.store(false, Ordering::SeqCst);
                layer.shared.end_job();
            });
        if spawned.is_err() {
            self.shared.verifying.store(false, Ordering::SeqCst);
            self.shared.end_job();
        }
    }

    /// Rebuild `key` at a view where its deps are still at `version` and
    /// compare with the entry served. A mismatch drops the entry.
    fn verify(&self, store: &Store, key: &R::Key, served: &Arc<R::Value>, version: u64) {
        let Ok(view) = store.read_view() else {
            return;
        };
        let Ok(deps) = self.resident.deps(key, &view) else {
            return;
        };
        if self.shared.table.version(&full_deps(deps)) != version || view.commit_index() < version {
            return;
        }
        let Ok(Some(rebuilt)) = self
            .resident
            .build(key, &view, &BuildCx { limit: usize::MAX })
        else {
            return;
        };
        bump(&self.counters.verify);
        if rebuilt != **served {
            bump(&self.counters.verify_mismatch);
            if let Some(cache) = self.cache().as_mut() {
                let same = cache
                    .get(key)
                    .is_some_and(|entry| Arc::ptr_eq(&entry.value, served));
                if same {
                    cache.remove(key);
                }
            }
            tracing::warn!(
                layer = R::NAME,
                key = key_hash(key),
                version,
                "resident entry differs from storage; dropped"
            );
        }
    }

    /// Drop every entry (budget changes and tests).
    pub fn clear(&self) {
        if let Some(cache) = self.cache().as_mut() {
            cache.clear();
        }
    }

    /// Test hook: replace the value an entry serves without touching its
    /// version, to prove that verification catches a corrupted entry.
    #[doc(hidden)]
    pub fn corrupt_for_test(&self, key: &R::Key, value: R::Value) {
        if let Some(cache) = self.cache().as_mut() {
            if let Some(entry) = cache.get_mut(key) {
                entry.value = Arc::new(value);
            }
        }
    }
}

trait LayerStats: Send + Sync {
    fn snapshot(&self) -> LayerSnapshot;
}

impl<R: Resident> LayerStats for Layer<R> {
    fn snapshot(&self) -> LayerSnapshot {
        let load = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        let occupancy = self.cache().as_ref().map_or_default(LogicalCache::stats);
        #[expect(
            clippy::cast_precision_loss,
            reason = "seconds for a metric; nanosecond precision is not needed"
        )]
        let build_seconds = load(&self.counters.build_nanos) as f64 / 1e9;
        LayerSnapshot {
            layer: R::NAME,
            bytes: occupancy.bytes,
            entries: occupancy.entries,
            hits: load(&self.counters.hits),
            misses: load(&self.counters.misses),
            builds: load(&self.counters.builds),
            build_refused: load(&self.counters.refused),
            build_seconds,
            invalidations: load(&self.counters.invalidations),
            evictions: occupancy.evictions,
            verify: load(&self.counters.verify),
            verify_mismatch: load(&self.counters.verify_mismatch),
        }
    }
}

/// The parent budgets layers are charged to.
struct Budgets {
    owner_set_bytes: usize,
    planner_stats_bytes: usize,
    logical: Option<ByteBudget>,
    graph: Option<ByteBudget>,
    vector: Option<ByteBudget>,
    columns: Option<ByteBudget>,
}

/// The store's resident layers and their shared version table.
pub struct Residents {
    shared: Arc<Shared>,
    budgets: Budgets,
    layers: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    stats: Mutex<Vec<Arc<dyn LayerStats>>>,
}

/// Slots of the version table: 2^20 × 8 bytes = 8 MiB, whatever the number
/// of tenants and properties.
const TABLE_BITS: u32 = 20;

/// Slots of the per-commit repeat filter of [`Residents::note_writes`]:
/// more than the distinct areas a wide record's rows cycle through.
const NOTE_FILTER_SLOTS: usize = 1_024;

impl Residents {
    /// The framework for a store configured by `config`, charging layers to
    /// the given parent budgets.
    pub(crate) fn new(
        config: &StorageResources,
        logical: Option<ByteBudget>,
        graph: Option<ByteBudget>,
        vector: Option<ByteBudget>,
        columns: Option<ByteBudget>,
    ) -> Self {
        Self::with_table_bits(config, TABLE_BITS, logical, graph, vector, columns)
    }

    pub(crate) fn with_table_bits(
        config: &StorageResources,
        bits: u32,
        logical: Option<ByteBudget>,
        graph: Option<ByteBudget>,
        vector: Option<ByteBudget>,
        columns: Option<ByteBudget>,
    ) -> Self {
        let rate = config.resident_verify_rate;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a rate in (0, 1] maps to a period of at least 1"
        )]
        let verify_period = if rate > 0.0 && rate.is_finite() {
            (1.0 / rate.min(1.0)).round().max(1.0) as u64
        } else {
            0
        };
        Self {
            shared: Arc::new(Shared {
                table: deps::VersionTable::new(bits),
                enabled: AtomicBool::new(config.resident_caches && residents_enabled()),
                verify_period,
                verifying: AtomicBool::new(false),
                store: OnceLock::new(),
                jobs: Mutex::new(0),
                idle: Condvar::new(),
            }),
            budgets: Budgets {
                owner_set_bytes: config.owner_set_cache_bytes,
                planner_stats_bytes: config.planner_stats_bytes,
                logical,
                graph,
                vector,
                columns,
            },
            layers: Mutex::new(HashMap::new()),
            stats: Mutex::new(Vec::new()),
        }
    }

    /// Let background builds and verifications open views of `store`.
    pub(crate) fn attach(&self, store: &Arc<Store>) {
        let _ = self.shared.store.set(Arc::downgrade(store));
    }

    fn budget(
        &self,
        budget: Budget,
        name: &'static str,
    ) -> Result<Option<ByteBudget>, ResourceError> {
        let share = |parent: &Option<ByteBudget>, bytes: Option<usize>| {
            parent
                .as_ref()
                .map(|parent| {
                    let limit = parent.snapshot().limit;
                    parent.child(name, bytes.map_or(limit, |bytes| bytes.min(limit)))
                })
                .transpose()
        };
        match budget {
            Budget::OwnerSets if self.budgets.owner_set_bytes == 0 => Ok(None),
            Budget::OwnerSets => share(&self.budgets.logical, Some(self.budgets.owner_set_bytes)),
            Budget::Logical => share(&self.budgets.logical, None),
            Budget::PlannerStats => match self.budgets.planner_stats_bytes {
                0 => Ok(None),
                bytes if self.budgets.logical.is_some() => {
                    share(&self.budgets.logical, Some(bytes))
                }
                bytes => ByteBudget::new(name, bytes).map(Some),
            },
            Budget::Graph => share(&self.budgets.graph, None),
            Budget::Vector => share(&self.budgets.vector, None),
            Budget::Columns => share(&self.budgets.columns, None),
        }
    }

    /// The layer `R`, made on first use with its budget.
    ///
    /// # Panics
    /// Never: a layer registered under `R`'s type id is always a `Layer<R>`.
    pub fn layer<R: Resident>(&self) -> Arc<Layer<R>> {
        let mut layers = self
            .layers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(layer) = layers.get(&TypeId::of::<R>()) {
            return Arc::clone(layer)
                .downcast::<Layer<R>>()
                .expect("a layer is registered under its own type id");
        }
        let budget = self.budget(R::BUDGET, R::NAME).unwrap_or_else(|error| {
            tracing::warn!(layer = R::NAME, %error, "resident layer has no budget");
            None
        });
        let limit = budget.as_ref().map_or(0, |budget| budget.snapshot().limit);
        let cache = budget.and_then(|budget| LogicalCache::new(budget).ok());
        let layer = Arc::new(Layer {
            resident: R::default(),
            shared: Arc::clone(&self.shared),
            limit,
            cache: Mutex::new(cache),
            rent: Mutex::new(HashMap::new()),
            building: Mutex::new(HashSet::new()),
            counters: Counters::default(),
        });
        layers.insert(
            TypeId::of::<R>(),
            Arc::clone(&layer) as Arc<dyn Any + Send + Sync>,
        );
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Arc::clone(&layer) as Arc<dyn LayerStats>);
        layer
    }

    /// Raise the version of every area `keys` (one commit's base and
    /// derived writes) change to `ci`. Runs before `ci` becomes visible.
    pub(crate) fn note_writes<'k>(&self, ci: u64, keys: impl IntoIterator<Item = &'k [u8]>) {
        // A commit writes many rows of few areas (a wide record writes a row
        // per property of each record): bump each distinct area once. The
        // filter drops most repeats before the sort.
        let mut recent = [None::<Dep>; NOTE_FILTER_SLOTS];
        let mut written = Vec::new();
        for key in keys {
            deps::deps_of_key(key, |dep| {
                let at = usize::try_from(dep.mix() % NOTE_FILTER_SLOTS as u64).unwrap_or_default();
                if recent[at] != Some(dep) {
                    recent[at] = Some(dep);
                    written.push(dep);
                }
            });
        }
        written.sort_unstable();
        written.dedup();
        for dep in written {
            self.shared.table.bump(dep, ci);
        }
    }

    /// The version of `deps` now (0 when unwritten since open).
    #[must_use]
    pub fn version(&self, deps: &[Dep]) -> u64 {
        self.shared.table.version(&full_deps(deps.to_vec()))
    }

    /// Whether layers keep entries (`resident-caches` and the environment).
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.shared.enabled()
    }

    /// Test hook: turn every layer on or off at runtime, so one request can
    /// be answered with and without entries at the same snapshot.
    #[doc(hidden)]
    pub fn set_enabled(&self, enabled: bool) {
        self.shared.enabled.store(enabled, Ordering::SeqCst);
    }

    /// Test hook: wait until no background build or verification runs.
    #[doc(hidden)]
    pub fn wait_idle(&self) {
        let mut jobs = self
            .shared
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *jobs > 0 {
            jobs = self
                .shared
                .idle
                .wait(jobs)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Counters and occupancy of every layer made so far.
    #[must_use]
    pub fn snapshots(&self) -> Vec<LayerSnapshot> {
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|layer| layer.snapshot())
            .collect()
    }

    /// Bytes of the version table.
    #[must_use]
    pub fn table_bytes(&self) -> usize {
        self.shared.table.bytes()
    }
}

impl Store {
    /// The store's resident layers.
    #[must_use]
    pub fn residents(&self) -> &Arc<Residents> {
        &self.residents
    }

    /// Let resident layers build and verify in the background at fresh
    /// views of this store. Called by whoever shares the store (`Lmpg`);
    /// without it layers still serve offered entries but never build.
    pub fn attach_residents(self: &Arc<Self>) {
        self.residents.attach(self);
    }
}
