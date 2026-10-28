//! Vector projection layer over canonical occurrences.
//!
//! [`VectorEngine`] manages immutable [`EmbeddingSpace`]s bound through stable
//! aliases, stores raw `f32` vector segments and bindings under canonical
//! tenant-prefixed keys, and exposes exact and approximate (ANN) similarity
//! search. Every mutation flows through the deterministic commit/replay path,
//! so vector state rebuilds identically after a restart.

pub mod ann;
pub mod search;
pub mod structural;

pub(crate) mod segment;

#[cfg(test)]
mod pinned_tests;

use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, Mutex};

use rushdb_lmpg::expand::meta_counter_key;
use rushdb_lmpg::reader::TryRead;
use rushdb_storage::{ReadView, StorageError};

fn uuid_v7() -> uuid::Uuid {
    uuid::Uuid::now_v7()
}
use rushdb_storage::CommitOutcome;
use rushdb_types::command::{CommitBatch, IsolationMode, Mutation};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{
    emb_alias_key, emb_binding_key, emb_bindings_prefix, emb_reclaim_prefix, emb_space_key,
    emb_vector_key, parse_emb_reclaim_value,
};
use rushdb_types::occurrence::{BindingStatus, EmbeddingBinding, EmbeddingSpaceInfo};

/// Immutable vector-space contract bound through stable aliases
/// (document 09). The metric is cosine through normalized vectors.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingSpace {
    /// Monotonic identifier allocated when the space is first created.
    pub space_id: u64,
    /// Alias this space was created under.
    pub name: String,
    /// Fixed vector dimensionality; other lengths are rejected.
    pub dims: u32,
    /// Similarity metric used when ranking scores in this space.
    pub similarity: rushdb_types::occurrence::Similarity,
}

/// Score a query against a stored vector under the space's similarity metric
/// (Neo4j semantics): cosine = dot/|a||b|, dot = raw dot product, euclidean =
/// `1 / (1 + L2 distance)`. Higher is more similar for all three.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "scores accumulate in f64 for numerical stability, then narrow to the f32 public API"
)]
pub fn score(query: &[f32], vec: &[f32], similarity: rushdb_types::occurrence::Similarity) -> f32 {
    match similarity {
        rushdb_types::occurrence::Similarity::Cosine => cosine(query, vec),
        rushdb_types::occurrence::Similarity::Dot => {
            let mut dot = 0f64;
            for i in 0..query.len() {
                dot += f64::from(query[i]) * f64::from(vec[i]);
            }
            dot as f32
        }
        rushdb_types::occurrence::Similarity::Euclidean => {
            let mut dist = 0f64;
            for i in 0..query.len() {
                let d = f64::from(query[i] - vec[i]);
                dist += d * d;
            }
            (1.0 / (1.0 + dist.sqrt())) as f32
        }
    }
}

/// Calibration (2026-10-06): a built `usearch` index carries a
/// large, roughly size-independent allocator floor — measured ~16-17 MiB
/// per generation regardless of vector count or dimension (1 vector/2 dims
/// through 5,000 vectors/128 dims all measured within 2% of the same
/// number; see `ann.rs`'s `MAX_CACHED_ANN_BYTES` doc). The old value (256) assumed a few-hundred-
/// byte formula estimate per generation and was never actually reachable:
/// `MAX_CACHED_ANN_BYTES` (256 MiB) always bound first, around the 15th
/// generation. 12 keeps both ceilings meaningful together (12 × ~20 MiB
/// safety-margined floor stays under the byte ceiling), instead of one
/// being silently dead code.
pub(crate) const MAX_CACHED_ANN_GENERATIONS: usize = 12;
/// Tenant-local immutable vector spaces are metadata, but each can retain
/// bindings and derived generations; bound their cardinality explicitly.
const MAX_VECTOR_SPACES: usize = 256;

/// Rows and bytes per batch when exact search streams an index's bindings.
const EXACT_SCAN_BATCH_ROWS: usize = 1024;
const EXACT_SCAN_BATCH_BYTES: usize = 4 * 1024 * 1024;

/// Total order over sources, the deterministic tie-break for equal scores.
fn source_order(source: SourceRef) -> (u8, u64, u64, u64) {
    (
        source.kind.as_byte(),
        source.owner_ord,
        source.pid,
        source.elem,
    )
}

/// One external vector to bind (see [`VectorEngine::bind_external_many`]).
struct BindRequest {
    source: SourceRef,
    vector: Vec<f32>,
    hash: u64,
    source_hash_tracked: bool,
}

/// Projection engine over canonical occurrences (plan document 09). Raw
/// f32 segments and bindings live under canonical tenant-prefixed keys and
/// participate in the same deterministic commit/replay path as graph data.
pub struct VectorEngine {
    /// Canonical graph and log engine the projection reads and commits through.
    pub lmpg: Arc<rushdb_lmpg::Lmpg>,
    /// Storage handle shared with `lmpg`, retained for direct view access.
    pub store: Arc<rushdb_storage::Store>,
    pub(crate) ann_cache: Mutex<HashMap<u64, Arc<ann::CachedAnn>>>,
    /// Per-space gate: one ANN assembly per space at a time.
    pub(crate) ann_builds: Mutex<HashMap<u64, Arc<Mutex<()>>>>,
    /// Background ANN builds started for `auto` searches (see
    /// [`search`]), so at most one runs per space and a graph the budget
    /// cannot admit is not rebuilt until its space changes.
    pub(crate) ann_warmups: Mutex<HashMap<u64, search::Warmup>>,
    /// Directory frozen ANN segments persist under (`<dir>/space-<id>/...`),
    /// when the store has one (`None` for a pure in-memory store, which has
    /// nowhere to persist — segments then stay resident-only, exactly as
    /// before this existed). Rebuildable cache, not canonical data: never
    /// included in backup/restore, and tenant purge removes it wholesale
    /// alongside dropping this engine's cache.
    pub(crate) segments_dir: Option<std::path::PathBuf>,
}

/// Errors surfaced by the vector projection engine.
#[derive(Debug, thiserror::Error)]
pub enum VectorError {
    /// Underlying canonical storage read or commit failed.
    #[error("vector storage read failed: {0}")]
    Storage(#[from] StorageError),
    /// A supplied vector or query is structurally invalid (bad shape or
    /// non-finite components).
    #[error("invalid vector: {0}")]
    InvalidVector(String),
    /// The operation conflicts with committed state, such as an unknown alias
    /// or an immutable-space mismatch.
    #[error("{0}")]
    Conflict(String),
    /// A binding names a source occurrence that no longer exists.
    #[error("source occurrence not found")]
    SourceMissing,
    /// Query and space dimensionality disagree.
    #[error("dimension mismatch: space has {0}, got {1}")]
    DimMismatch(u32, usize),
    /// The storage commit path itself failed.
    #[error("engine error: {0}")]
    Engine(String),
    /// The vector cache budget cannot admit an ANN build right now. Search
    /// falls back to the exact path instead of surfacing this.
    #[error("ANN build memory budget: {0}")]
    AnnBudget(String),
}

/// Canonical identity of one vector's source occurrence: the owning record or
/// relationship ordinal plus the property/element address that produced it.
#[derive(Debug, Clone, Copy)]
pub struct SourceRef {
    /// Whether the source is a record or a relationship occurrence.
    pub kind: OwnerKind,
    /// Ordinal of the owning record or relationship.
    pub owner_ord: u64,
    /// Property identifier of the source field.
    pub pid: u64,
    /// Element index within the source property.
    pub elem: u64,
}

/// One ranked search hit: the source it came from, its similarity score, and
/// the value hash recorded when the binding was written.
#[derive(Debug, Clone)]
pub struct ScoredSource {
    /// Canonical identity of the matched source.
    pub source: SourceRef,
    /// Similarity score under the space metric; higher is more similar.
    pub score: f32,
    /// Hash of the source value at bind time, used for staleness checks.
    pub value_hash: u64,
}

impl PartialEq for ScoredSource {
    fn eq(&self, other: &Self) -> bool {
        self.score.total_cmp(&other.score) == std::cmp::Ordering::Equal
    }
}
impl Eq for ScoredSource {}
impl PartialOrd for ScoredSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ScoredSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.total_cmp(&other.score)
    }
}

impl From<String> for VectorError {
    fn from(v: String) -> Self {
        VectorError::Conflict(v)
    }
}

fn empty_batch(t: TenantId) -> CommitBatch {
    CommitBatch {
        command_version: rushdb_types::command::COMMAND_VERSION,
        tenant: t,
        transaction_id: String::new(),
        commit_key: uuid::Uuid::nil(),
        start_index: 0,
        isolation: IsolationMode::Snapshot,
        preconditions: vec![],
        mutations: vec![],
    }
}

/// Delete what an interrupted segment write left under `dir/space-*/`:
/// staged files (`.tmp-`) and sidecars whose segment file is gone.
fn sweep_orphan_segments(dir: &std::path::Path) {
    let Ok(spaces) = std::fs::read_dir(dir) else {
        return;
    };
    for space in spaces.flatten() {
        if !space.file_name().to_string_lossy().starts_with("space-") {
            continue;
        }
        let Ok(files) = std::fs::read_dir(space.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if !name.starts_with("seg-") || segment::is_segment_file_name(&name) {
                continue;
            }
            let staged = name.contains(".tmp-");
            let segment = name.split('.').next().unwrap_or_default();
            if staged || !space.path().join(segment).exists() {
                if let Err(error) = std::fs::remove_file(file.path()) {
                    eprintln!("rushdb: cannot remove orphan vector segment {name}: {error}");
                }
            }
        }
    }
}

fn decode_vector(raw: Option<Vec<u8>>, dims: u32) -> Result<Vec<f32>, VectorError> {
    let b = raw.ok_or(StorageError::CorruptLog)?;
    if b.len() % 4 != 0 || b.len() / 4 != dims as usize {
        return Err(StorageError::CorruptLog.into());
    }
    let values: Vec<f32> = b
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect();
    if values.iter().any(|v| !v.is_finite()) {
        return Err(StorageError::CorruptLog.into());
    }
    Ok(values)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "cosine accumulates in f64 for numerical stability, then narrows to the f32 API type"
)]
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut na, mut nb) = (0f64, 0f64, 0f64);
    for i in 0..a.len() {
        dot += f64::from(a[i]) * f64::from(b[i]);
        na += f64::from(a[i]).powi(2);
        nb += f64::from(b[i]).powi(2);
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        (dot / (na.sqrt() * nb.sqrt())) as f32
    }
}

/// The hash of `source`'s current value: what a guarded bind
/// ([`VectorEngine::bind_if_source_hash`]) expects, so an embedding computed
/// from an older value is rejected. `None` when the source has no value.
///
/// # Errors
/// Returns [`StorageError`] when the occurrence cannot be read.
pub fn source_hash<R>(r: &R, t: TenantId, source: SourceRef) -> Result<Option<u64>, StorageError>
where
    R: TryRead,
{
    Ok(rushdb_lmpg::reader::try_access::occurrence(
        r,
        t,
        source.kind,
        source.owner_ord,
        source.pid,
        source.elem,
    )?
    .map(|occ| occ.value_hash()))
}

fn lookup_space<R: TryRead>(
    r: &R,
    t: TenantId,
    sid: u64,
) -> Result<Option<EmbeddingSpace>, StorageError> {
    let Some(raw) = r.get(&emb_space_key(t, sid))? else {
        return Ok(None);
    };
    let info: EmbeddingSpaceInfo =
        serde_json::from_slice(&raw).map_err(|_| StorageError::CorruptLog)?;
    if info.space_id != sid || info.dims == 0 {
        return Err(StorageError::CorruptLog);
    }
    Ok(Some(EmbeddingSpace {
        space_id: info.space_id,
        name: info.name,
        dims: info.dims,
        similarity: info.similarity,
    }))
}

fn read_counter_bytes(v: Option<&[u8]>) -> Result<Option<u64>, String> {
    v.map(|b| {
        b.try_into()
            .map(u64::from_be_bytes)
            .map_err(|_| "corrupt vector counter".to_string())
    })
    .transpose()
}

impl VectorEngine {
    /// Conservative resident-byte estimate for cached ANN generations.
    /// Canonical vector segments remain storage-owned and are excluded.
    #[must_use]
    pub fn resident_bytes(&self) -> usize {
        self.ann_cache.lock().map_or(0, |cache| {
            cache
                .values()
                .map(|cached| ann::cached_ann_bytes(cached))
                .sum()
        })
    }

    /// Borrow the storage handle backing this engine.
    pub fn store(&self) -> &Arc<rushdb_storage::Store> {
        &self.store
    }

    /// Tenant this engine projects.
    pub fn tenant(&self) -> TenantId {
        self.lmpg.tenant
    }

    /// Create an engine over an existing `Lmpg`, sharing its storage. The
    /// `Lmpg` expander already handles embedding mutations, so no secondary
    /// registration occurs here (single deterministic path).
    pub fn new(lmpg: Arc<rushdb_lmpg::Lmpg>) -> Self {
        let store = Arc::clone(&lmpg.store);
        VectorEngine {
            lmpg,
            store,
            ann_cache: Mutex::new(HashMap::new()),
            ann_builds: Mutex::new(HashMap::new()),
            ann_warmups: Mutex::new(HashMap::new()),
            segments_dir: None,
        }
    }

    /// Configures the directory frozen ANN segments persist under. The
    /// caller (`Node::vector_engine`) derives this from the store's own
    /// data directory, one subdirectory per tenant; omit it (the default)
    /// for a pure in-memory store.
    ///
    /// Opening sweeps staged writes an earlier engine left.
    /// Published segments stay: a cache miss reopens and reconciles them
    ///, deleting those without a live entry.
    #[must_use]
    pub fn with_segments_dir(mut self, dir: std::path::PathBuf) -> Self {
        sweep_orphan_segments(&dir);
        self.segments_dir = Some(dir);
        self
    }

    /// Capture a read view of the current committed state.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the read view cannot be opened.
    pub fn snapshot(&self) -> Result<ReadView, StorageError> {
        self.lmpg.store.read_view()
    }

    /// Resolve-or-create an immutable Space behind `alias`; the dims/model
    /// contract cannot change once the alias exists.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::InvalidVector`] when `dims` is zero,
    /// [`VectorError::Conflict`] when the alias already exists with different
    /// dims or similarity, and [`VectorError::Storage`] or
    /// [`VectorError::Engine`] when the commit fails.
    pub fn ensure_space(&self, alias: &str, dims: u32) -> Result<EmbeddingSpace, VectorError> {
        self.ensure_space_similarity(alias, dims, rushdb_types::occurrence::Similarity::Cosine)
    }

    /// Ensure a vector space with an explicit similarity metric. The engine
    /// carries vectors only — no embedding source/model concept.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::InvalidVector`] when `dims` is zero,
    /// [`VectorError::Conflict`] when the alias exists with a different dims
    /// or similarity contract, and [`VectorError::Storage`] or
    /// [`VectorError::Engine`] when the commit fails.
    pub fn ensure_space_similarity(
        &self,
        alias: &str,
        dims: u32,
        similarity: rushdb_types::occurrence::Similarity,
    ) -> Result<EmbeddingSpace, VectorError> {
        if dims == 0 {
            return Err(VectorError::InvalidVector(
                "dimensions must be positive".into(),
            ));
        }
        let t = self.lmpg.tenant;
        let store = self.lmpg.store.clone();
        let alias_owned = alias.to_string();

        match store.commit_for_tenant(&self.lmpg.tenant, uuid_v7(), move |_ci, txn| {
            let akey = emb_alias_key(t, &alias_owned);
            if let Some(raw) = txn.get(&akey).map_err(|e| e.to_string())? {
                let sid = read_counter_bytes(Some(&raw))?.ok_or("corrupt alias mapping")?;
                let raw = txn
                    .get(&emb_space_key(t, sid))
                    .map_err(|e| e.to_string())?
                    .ok_or("corrupt alias mapping")?;
                let info: EmbeddingSpaceInfo =
                    serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
                let sp = EmbeddingSpace {
                    space_id: info.space_id,
                    name: info.name,
                    dims: info.dims,
                    similarity: info.similarity,
                };
                if sp.dims != dims || sp.similarity != similarity {
                    return Err(format!(
                        "immutable space mismatch for alias `{alias_owned}`"
                    ));
                }
                return Ok((empty_batch(t), sp));
            }
            let space_prefix = rushdb_types::keys::key(t, &[rushdb_types::keys::P_EMB_SPACE]);
            if txn
                .range_prefix(&space_prefix)
                .map_err(|e| e.to_string())?
                .len()
                >= MAX_VECTOR_SPACES
            {
                return Err(format!(
                    "vector space limit reached ({MAX_VECTOR_SPACES} per tenant)"
                ));
            }
            let sid = read_counter_bytes(
                txn.get(&meta_counter_key(t, "space"))
                    .map_err(|e| e.to_string())?
                    .as_deref(),
            )?
            .unwrap_or(0);
            let info = EmbeddingSpaceInfo {
                space_id: sid,
                name: alias_owned.clone(),
                dims,
                similarity,
            };
            let mut batch = empty_batch(t);
            batch.mutations.push(Mutation::EnsureEmbeddingSpace {
                alias: alias_owned.clone(),
                info,
                new: true,
            });
            Ok((
                batch,
                EmbeddingSpace {
                    space_id: sid,
                    name: alias_owned,
                    dims,
                    similarity,
                },
            ))
        }) {
            Ok(CommitOutcome::Committed { payload, .. }) => Ok(payload),
            Ok(CommitOutcome::Conflicted(reason)) => Err(reason.into()),
            Err(e) => Err(VectorError::Engine(e.to_string())),
        }
    }

    /// Resolve `alias` to its immutable space, or `None` when unbound.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if the snapshot fails or the alias or
    /// space record is corrupt, and [`VectorError::InvalidVector`] indirectly
    /// when the stored space is malformed.
    pub fn lookup_space_by_alias(
        &self,
        alias: &str,
    ) -> Result<Option<EmbeddingSpace>, VectorError> {
        self.lookup_space_in_view(&self.snapshot()?, alias)
    }

    /// Resolve `alias` within a caller-owned view, or `None` when unbound.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if `snap` belongs to another store, if
    /// the alias is corrupt, or if the referenced space record is missing or
    /// malformed.
    pub fn lookup_space_in_view(
        &self,
        snap: &ReadView,
        alias: &str,
    ) -> Result<Option<EmbeddingSpace>, VectorError> {
        self.validate_view(snap)?;
        let Some(raw) = snap.get(&emb_alias_key(self.lmpg.tenant, alias))? else {
            return Ok(None);
        };
        let sid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| StorageError::CorruptLog)?,
        );
        Ok(Some(
            lookup_space(snap, self.lmpg.tenant, sid)?.ok_or(StorageError::CorruptLog)?,
        ))
    }

    /// Bind one canonical source into `alias` recording its exact value
    /// hash. Two immutable Spaces may bind one source independently
    /// (vertical slice item 7).
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::SourceMissing`] when the source occurrence does
    /// not exist, [`VectorError::Conflict`] for an unknown alias,
    /// [`VectorError::DimMismatch`] for a wrong-length vector, and
    /// [`VectorError::InvalidVector`] when a component is not finite.
    pub fn bind(&self, alias: &str, source: SourceRef, vector: &[f32]) -> Result<u64, VectorError> {
        let t = self.lmpg.tenant;
        let hash = source_hash(&self.snapshot()?, t, source)?.ok_or(VectorError::SourceMissing)?;
        self.bind_inner(alias, source, vector, hash, true)
    }

    /// Bind a managed embedding only when the source still has the hash that
    /// the caller captured before invoking its provider. This is the safe
    /// completion path for asynchronous embedding jobs: a source update while
    /// the provider was running rejects the stale vector instead of attaching
    /// it to the newer value.
    ///
    /// # Errors
    /// Returns [`VectorError::Conflict`] when the source hash changed,
    /// [`VectorError::SourceMissing`] when the source no longer exists, and
    /// the same validation errors as [`Self::bind`] otherwise.
    pub fn bind_if_source_hash(
        &self,
        alias: &str,
        source: SourceRef,
        expected_source_hash: u64,
        vector: &[f32],
    ) -> Result<u64, VectorError> {
        let t = self.lmpg.tenant;
        let current_hash =
            source_hash(&self.snapshot()?, t, source)?.ok_or(VectorError::SourceMissing)?;
        if current_hash != expected_source_hash {
            return Err(VectorError::Conflict(format!(
                "source changed while embedding was in flight (expected hash {expected_source_hash}, current {current_hash})"
            )));
        }
        self.bind_inner(alias, source, vector, expected_source_hash, true)
    }

    /// Bind an external vector that is NOT stored as a record occurrence
    /// (inline `$vectors`): no live-occurrence hash is required. The source
    /// hash is derived deterministically from the vector payload instead.
    /// Vectors are data; the DB never generates embeddings.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Conflict`] for an unknown alias,
    /// [`VectorError::DimMismatch`] for a wrong-length vector, and
    /// [`VectorError::InvalidVector`] when a component is not finite.
    pub fn bind_external(
        &self,
        alias: &str,
        source: SourceRef,
        vector: &[f32],
    ) -> Result<u64, VectorError> {
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        let hash = rushdb_types::hash::stable_hash64(&bytes);
        self.bind_inner(alias, source, vector, hash, false)
    }

    fn bind_inner(
        &self,
        alias: &str,
        source: SourceRef,
        vector: &[f32],
        hash: u64,
        source_hash_tracked: bool,
    ) -> Result<u64, VectorError> {
        let slots = self.bind_many_inner(
            alias,
            vec![BindRequest {
                source,
                vector: vector.to_vec(),
                hash,
                source_hash_tracked,
            }],
        )?;
        slots
            .into_iter()
            .next()
            .ok_or_else(|| VectorError::Engine("binding commit returned no slot".into()))
    }

    /// Bind external vectors to `alias` in **one** commit (one fsync) and
    /// return each item's slot, in order.
    ///
    /// Binding one commit per vector made an ingest batch of N inline vectors
    /// cost N durable commits (C9, cloud-capacity F10). Re-binding an
    /// identical vector stays an idempotent no-op that allocates no slot.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Conflict`] for an unknown alias,
    /// [`VectorError::DimMismatch`] for a wrong-length vector, and
    /// [`VectorError::InvalidVector`] when a component is not finite. No
    /// vector of the batch is bound when any item is invalid.
    pub fn bind_external_many(
        &self,
        alias: &str,
        items: Vec<(SourceRef, Vec<f32>)>,
    ) -> Result<Vec<u64>, VectorError> {
        let requests = items
            .into_iter()
            .map(|(source, vector)| {
                let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
                BindRequest {
                    source,
                    hash: rushdb_types::hash::stable_hash64(&bytes),
                    vector,
                    source_hash_tracked: false,
                }
            })
            .collect();
        self.bind_many_inner(alias, requests)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one linear validate/txn-build/commit/absorb pipeline; splitting it would scatter the shared commit-closure captures"
    )]
    fn bind_many_inner(
        &self,
        alias: &str,
        requests: Vec<BindRequest>,
    ) -> Result<Vec<u64>, VectorError> {
        let t = self.lmpg.tenant;
        let Some(sp) = self.lookup_space_by_alias(alias)? else {
            return Err(VectorError::Conflict(format!(
                "unknown embedding alias `{alias}`"
            )));
        };
        for request in &requests {
            if sp.dims as usize != request.vector.len() {
                return Err(VectorError::DimMismatch(sp.dims, request.vector.len()));
            }
            if request.vector.iter().any(|v| !v.is_finite()) {
                return Err(VectorError::InvalidVector(
                    "components must be finite".into(),
                ));
            }
        }
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        // Captured by value before `sp` moves into the commit closure below;
        // needed after the commit to keep this space's cached ANN generation
        // current (`incrementally_absorb_binds`).
        let space_id = sp.space_id;
        let dims = sp.dims;
        let similarity = sp.similarity;

        let store = self.lmpg.store.clone();
        let alias_owned = alias.to_string();
        let requests = Arc::new(requests);
        let staged = Arc::clone(&requests);
        let outcome = store.commit_for_tenant(&self.lmpg.tenant, uuid_v7(), move |_ci, txn| {
            let akey = emb_alias_key(t, &alias_owned);
            let raw = txn
                .get(&akey)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "alias vanished".to_string())?;
            let sid = read_counter_bytes(Some(&raw))?.ok_or("corrupt alias mapping")?;
            if sid != sp.space_id {
                return Err("embedding alias changed during binding".into());
            }
            // A fresh slot per binding is required for MVCC: a pinned read
            // view resolves the historical binding's `vector_slot`, so reusing
            // the slot would overwrite bytes that view still needs.
            // Reclaiming old slots needs the retention/GC machinery (see
            // embedding-versioning). One counter read covers the batch;
            // expansion advances the counter past the highest slot used.
            let slot_key = meta_counter_key(t, &format!("vecslot/{sid}"));
            let mut next_slot =
                read_counter_bytes(txn.get(&slot_key).map_err(|e| e.to_string())?.as_deref())?
                    .unwrap_or(0);
            let mut batch = empty_batch(t);
            let mut results = Vec::with_capacity(staged.len());
            for request in staged.iter() {
                let source = request.source;
                // Retries after a partial ingest must be idempotent. Reusing
                // the same source hash means the requested vector is already
                // bound; allocate no slot and emit no duplicate event.
                let binding_key = emb_binding_key(
                    t,
                    sid,
                    source.kind,
                    source.owner_ord,
                    source.pid,
                    source.elem,
                );
                if let Some(raw) = txn.get(&binding_key).map_err(|e| e.to_string())? {
                    let existing: EmbeddingBinding =
                        serde_json::from_slice(&raw).map_err(|_| "corrupt embedding binding")?;
                    if existing.source_hash == request.hash
                        && existing.source_hash_tracked == request.source_hash_tracked
                        && existing.status == BindingStatus::Ready
                    {
                        results.push(
                            serde_json::json!({ "slot": existing.vector_slot, "bound": false }),
                        );
                        continue;
                    }
                }
                let slot = next_slot;
                next_slot += 1;
                batch.mutations.push(Mutation::UpsertEmbeddingBinding {
                    binding: EmbeddingBinding {
                        space_id: sid,
                        owner_kind: source.kind,
                        owner_ord: source.owner_ord,
                        pid: source.pid,
                        elem: source.elem,
                        vector_slot: slot,
                        source_hash: request.hash,
                        source_hash_tracked: request.source_hash_tracked,
                        status: BindingStatus::Ready,
                    },
                    vector: Some(request.vector.clone()),
                    expect_live_hash: request.source_hash_tracked,
                });
                results.push(serde_json::json!({ "slot": slot, "bound": true }));
            }
            Ok((batch, serde_json::Value::Array(results)))
        });
        match outcome {
            Ok(CommitOutcome::Committed {
                payload,
                commit_index,
            }) => {
                let results = payload
                    .as_array()
                    .ok_or_else(|| VectorError::Engine("invalid binding commit payload".into()))?;
                let mut slots = Vec::with_capacity(results.len());
                let mut absorbed = Vec::new();
                for (result, request) in results.iter().zip(requests.iter()) {
                    let slot = result["slot"].as_u64().ok_or_else(|| {
                        VectorError::Engine("invalid binding commit payload".into())
                    })?;
                    if result["bound"] == serde_json::Value::Bool(true) {
                        absorbed.push((
                            ann::AnnEntry {
                                slot,
                                source: request.source,
                                pivot: 0,
                                value_hash: request.hash,
                            },
                            request.vector.clone(),
                        ));
                    }
                    slots.push(slot);
                }
                self.incrementally_absorb_binds(space_id, commit_index, absorbed, dims, similarity);
                Ok(slots)
            }
            Ok(CommitOutcome::Conflicted(reason)) => Err(reason.into()),
            Err(e) => Err(VectorError::Engine(e.to_string())),
        }
    }

    /// Reclaim embedding vector slots whose superseding binding version is at
    /// or below `through_ci` (the GC watermark).
    ///
    /// Re-binding allocates a fresh slot and records the superseded one in the
    /// reclamation index; its bytes stay readable by a pinned view until the
    /// history floor passes it, so deletion waits for `through_ci`. Commits
    /// bounded batches until no reclaimable row remains (or a cap is reached)
    /// and returns the number of slots reclaimed. A pinned view below the floor
    /// keeps the watermark from advancing, so its slots are never reclaimed.
    ///
    /// # Errors
    /// Returns [`VectorError::Storage`] on a read/commit failure.
    pub fn reclaim_slots(&self, through_ci: u64) -> Result<usize, VectorError> {
        const MAX_BATCHES: usize = 64;
        const BATCH: usize = 512;
        // Never reclaim a slot a pinned view may still need: clamp to the
        // durable history floor, which already accounts for active read pins.
        let through_ci = through_ci.min(self.lmpg.store.gc_watermark());
        let t = self.lmpg.tenant;
        let store = self.lmpg.store.clone();
        let prefix = emb_reclaim_prefix(t);
        let mut total = 0usize;
        for _ in 0..MAX_BATCHES {
            let pending = {
                let view = store.read_view()?;
                view.range_prefix(&prefix)?
                    .iter()
                    .filter(|(_, value)| {
                        parse_emb_reclaim_value(value)
                            .is_some_and(|(after_ci, _, _)| after_ci <= through_ci)
                    })
                    .count()
            };
            if pending == 0 {
                break;
            }
            match store.commit_for_tenant(&t, uuid_v7(), move |_ci, _txn| {
                let mut batch = empty_batch(t);
                batch
                    .mutations
                    .push(Mutation::ReclaimEmbeddingSlots { through_ci });
                Ok((batch, ()))
            }) {
                Ok(CommitOutcome::Committed { .. }) => total += pending.min(BATCH),
                Ok(CommitOutcome::Conflicted(_)) => break,
                Err(e) => return Err(VectorError::Engine(e.to_string())),
            }
        }
        Ok(total)
    }

    /// Fetch the binding of one source in `space`, if present.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if the snapshot fails or the stored
    /// binding record is corrupt.
    pub fn binding_of(
        &self,
        space: u64,
        source: SourceRef,
    ) -> Result<Option<EmbeddingBinding>, VectorError> {
        self.binding_of_in_view(&self.snapshot()?, space, source)
    }

    /// Fetch the binding of one source in `space` within a caller-owned view.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if `snap` belongs to another store or
    /// the stored binding record is corrupt.
    pub fn binding_of_in_view(
        &self,
        snap: &ReadView,
        space: u64,
        source: SourceRef,
    ) -> Result<Option<EmbeddingBinding>, VectorError> {
        self.validate_view(snap)?;
        let t = self.lmpg.tenant;
        let raw = snap.get(&emb_binding_key(
            t,
            space,
            source.kind,
            source.owner_ord,
            source.pid,
            source.elem,
        ))?;
        raw.map(|raw| serde_json::from_slice(&raw).map_err(|_| StorageError::CorruptLog.into()))
            .transpose()
    }

    /// All bindings of one Space regardless of status.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if the snapshot fails or a stored
    /// binding record is corrupt.
    pub fn bindings_of(&self, space: u64) -> Result<Vec<EmbeddingBinding>, VectorError> {
        self.bindings_of_in_view(&self.snapshot()?, space)
    }

    /// All bindings of one Space within a caller-owned view, regardless of
    /// status.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if `snap` belongs to another store or
    /// a stored binding record is corrupt.
    pub fn bindings_of_in_view(
        &self,
        snap: &ReadView,
        space: u64,
    ) -> Result<Vec<EmbeddingBinding>, VectorError> {
        self.validate_view(snap)?;
        let mut bindings = Vec::new();
        for_each_binding(snap, self.lmpg.tenant, space, &mut |binding| {
            bindings.push(binding);
            true
        })?;
        Ok(bindings)
    }

    /// Binding counts of one Space by status within a caller-owned view,
    /// streamed: no binding list is held.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if `snap` belongs to another store or
    /// a stored binding record is corrupt.
    pub fn binding_counts_in_view(
        &self,
        snap: &ReadView,
        space: u64,
    ) -> Result<BindingCounts, VectorError> {
        self.validate_view(snap)?;
        let mut counts = BindingCounts::default();
        for_each_binding(snap, self.lmpg.tenant, space, &mut |binding| {
            counts.total += 1;
            match binding.status {
                BindingStatus::Ready => counts.ready += 1,
                BindingStatus::Stale => counts.stale += 1,
                BindingStatus::Retired => counts.retired += 1,
                BindingStatus::Pending => {}
            }
            if binding.source_hash_tracked {
                counts.tracked += 1;
            }
            true
        })?;
        Ok(counts)
    }

    /// Revalidation derives every binding's eligibility from live truth:
    /// Ready when hashes match, Stale on mismatch, Retired when the source
    /// occurrence is gone. Status transitions flow through the same
    /// deterministic mutation path, so replay reproduces them exactly.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Conflict`] for an unknown alias, and
    /// [`VectorError::Storage`] or [`VectorError::Engine`] when reading or
    /// committing the new statuses fails.
    #[expect(
        clippy::too_many_lines,
        reason = "one linear derive-statuses/commit/absorb pipeline; splitting it would scatter the shared commit-closure captures"
    )]
    pub fn revalidate_bindings(&self, space_alias: &str) -> Result<(usize, usize), VectorError> {
        let t = self.lmpg.tenant;
        let Some(space) = self.lookup_space_by_alias(space_alias)? else {
            return Err(VectorError::Conflict("unknown alias".into()));
        };
        let store = self.lmpg.store.clone();
        match store.commit_for_tenant(&self.lmpg.tenant, uuid_v7(), move |_ci, txn| {
            let bindings: Vec<EmbeddingBinding> = txn
                .range_prefix(&emb_bindings_prefix(t, space.space_id))
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|(_, raw)| serde_json::from_slice(&raw).map_err(|e| e.to_string()))
                .collect::<Result<_, _>>()?;
            let mut stale = 0usize;
            let mut retired = 0usize;
            let mut newly_retired: Vec<SourceRef> = Vec::new();
            let mut batch = empty_batch(t);
            for b in &bindings {
                // Owner deletion retires every binding. Native occurrence
                // bindings additionally track their source value hash; external
                // vectors carry a payload hash and only track owner existence.
                let source_alive = match b.owner_kind {
                    OwnerKind::Record => {
                        rushdb_lmpg::reader::try_access::record_by_ord(txn, t, b.owner_ord)
                            .map_err(|e| e.to_string())?
                            .is_some()
                    }
                    OwnerKind::Relationship => {
                        rushdb_lmpg::reader::try_access::relationship_by_ord(txn, t, b.owner_ord)
                            .map_err(|e| e.to_string())?
                            .is_some()
                    }
                };
                let target_status = if !source_alive {
                    BindingStatus::Retired
                } else if b.source_hash_tracked {
                    match source_hash(
                        txn,
                        t,
                        SourceRef {
                            kind: b.owner_kind,
                            owner_ord: b.owner_ord,
                            pid: b.pid,
                            elem: b.elem,
                        },
                    )
                    .map_err(|e| e.to_string())?
                    {
                        Some(hash) if hash == b.source_hash => BindingStatus::Ready,
                        Some(_) => BindingStatus::Stale,
                        None => BindingStatus::Retired,
                    }
                } else {
                    BindingStatus::Ready
                };
                match target_status {
                    BindingStatus::Stale => stale += 1,
                    BindingStatus::Retired => retired += 1,
                    _ => {}
                }
                if target_status == BindingStatus::Retired && b.status != BindingStatus::Retired {
                    newly_retired.push(SourceRef {
                        kind: b.owner_kind,
                        owner_ord: b.owner_ord,
                        pid: b.pid,
                        elem: b.elem,
                    });
                }
                if target_status != b.status {
                    // Carry the NEW status into the persisted binding, not the
                    // stale read value.
                    let mut updated = b.clone();
                    updated.status = target_status;
                    // Persist the computed status as-is: the apply must not
                    // re-derive it from a hash (external bindings have no
                    // canonical occurrence hash to compare).
                    batch.mutations.push(Mutation::UpsertEmbeddingBinding {
                        binding: updated,
                        vector: None,
                        expect_live_hash: false,
                    });
                }
            }
            // Carried out via the payload (not applied here): a commit
            // closure must stay a pure function of its inputs, replayable
            // without side effects on this engine's own in-memory ANN
            // cache.
            let retired_sources: Vec<serde_json::Value> = newly_retired
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "kind": s.kind,
                        "ownerOrd": s.owner_ord,
                        "pid": s.pid,
                        "elem": s.elem,
                    })
                })
                .collect();
            let payload = serde_json::json!({
                "stale": stale,
                "retired": retired,
                "retiredSources": retired_sources,
            });
            Ok((batch, payload))
        }) {
            Ok(CommitOutcome::Committed {
                payload,
                commit_index,
            }) => {
                let stale = usize::try_from(
                    payload["stale"]
                        .as_u64()
                        .ok_or_else(|| VectorError::Engine("missing stale count".into()))?,
                )
                .map_err(|_| VectorError::Engine("stale count out of range".into()))?;
                let retired = usize::try_from(
                    payload["retired"]
                        .as_u64()
                        .ok_or_else(|| VectorError::Engine("missing retired count".into()))?,
                )
                .map_err(|_| VectorError::Engine("retired count out of range".into()))?;
                let retired_sources: Vec<SourceRef> = payload["retiredSources"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| {
                        Some(SourceRef {
                            kind: match v["kind"].as_str()? {
                                "Record" => OwnerKind::Record,
                                "Relationship" => OwnerKind::Relationship,
                                _ => return None,
                            },
                            owner_ord: v["ownerOrd"].as_u64()?,
                            pid: v["pid"].as_u64()?,
                            elem: v["elem"].as_u64()?,
                        })
                    })
                    .collect();
                self.incrementally_absorb_retirements(
                    space.space_id,
                    commit_index,
                    &retired_sources,
                    stale,
                );
                Ok((stale, retired))
            }
            Ok(CommitOutcome::Conflicted(reason)) => Err(reason.into()),
            Err(e) => Err(VectorError::Engine(e.to_string())),
        }
    }

    /// Remove every binding and vector segment of a space (index deletion).
    /// Alias resolution, enumeration, and deletion share writer admission.
    /// Immutable space metadata and allocation counters are retained.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Conflict`] for an unknown alias, and
    /// [`VectorError::Storage`] or [`VectorError::Engine`] when the delete
    /// commit fails.
    pub fn clear_space(&self, alias: &str) -> Result<usize, VectorError> {
        let t = self.lmpg.tenant;
        let alias = alias.to_owned();
        match self
            .store
            .commit_for_tenant(&t, uuid_v7(), move |_ci, txn| {
                let raw = txn
                    .get(&emb_alias_key(t, &alias))
                    .map_err(|e| e.to_string())?
                    .ok_or("unknown alias")?;
                let space_id = read_counter_bytes(Some(&raw))?.ok_or("corrupt alias mapping")?;
                let count = txn
                    .keys_prefix(&emb_bindings_prefix(t, space_id))
                    .map_err(|e| e.to_string())?
                    .len();
                let mut batch = empty_batch(t);
                batch
                    .mutations
                    .push(Mutation::ClearEmbeddingSpace { alias, space_id });
                Ok((batch, (space_id, count)))
            }) {
            Ok(CommitOutcome::Committed {
                payload: (space_id, count),
                commit_index,
            }) => {
                // Cache eviction is best-effort derived maintenance after a
                // successful durable delete. Do not evict a newer generation.
                if let Ok(mut cache) = self.ann_cache.lock() {
                    if cache
                        .get(&space_id)
                        .is_some_and(|entry| entry.space_version <= commit_index)
                    {
                        if let Some(old) = cache.remove(&space_id) {
                            ann::retire_cached(&old);
                        }
                    }
                }
                Ok(count)
            }
            Ok(CommitOutcome::Conflicted(r)) => Err(r.into()),
            Err(e) => Err(VectorError::Engine(e.to_string())),
        }
    }

    /// Exhaustively score every `Ready` binding of `alias` against `query` and
    /// return the top `k`, optionally filtered by `candidate`.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Conflict`] for an unknown alias,
    /// [`VectorError::DimMismatch`] when `query` length differs from the
    /// space dims, [`VectorError::InvalidVector`] for non-finite query
    /// components, and [`VectorError::Storage`] for corrupt segments.
    pub fn search_exact(
        &self,
        alias: &str,
        query: &[f32],
        k: usize,
        candidate: Option<&dyn Fn(SourceRef) -> bool>,
    ) -> Result<Vec<ScoredSource>, VectorError> {
        let snap = self.snapshot()?;
        let space = self
            .lookup_space_in_view(&snap, alias)?
            .ok_or_else(|| VectorError::Conflict("unknown alias".into()))?;
        self.search_exact_in_view(&snap, &space, query, k, candidate)
    }

    /// Reject a caller-owned view that belongs to a different store.
    fn validate_view(&self, snap: &ReadView) -> Result<(), VectorError> {
        if !snap.belongs_to(&self.lmpg.store) {
            return Err(
                StorageError::InvalidRead("vector view belongs to another Store".into()).into(),
            );
        }
        Ok(())
    }

    /// Exact top-`k` search within a caller-owned view, with optional
    /// filtering. The view must belong to this engine's store.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] when `snap` belongs to another store
    /// or a binding or segment is corrupt, [`VectorError::DimMismatch`] when
    /// `query` length differs from the space dims, and
    /// [`VectorError::InvalidVector`] for non-finite query components.
    pub fn search_exact_in_view(
        &self,
        snap: &ReadView,
        space: &EmbeddingSpace,
        query: &[f32],
        k: usize,
        candidate: Option<&dyn Fn(SourceRef) -> bool>,
    ) -> Result<Vec<ScoredSource>, VectorError> {
        self.exact_in_view(snap, space, query, k, candidate, None)
    }

    /// [`Self::search_exact_in_view`] keeping only hits scoring at least
    /// `min_score`; a lower hit never enters the top-`k` heap.
    pub(crate) fn exact_in_view(
        &self,
        snap: &ReadView,
        space: &EmbeddingSpace,
        query: &[f32],
        k: usize,
        candidate: Option<&dyn Fn(SourceRef) -> bool>,
        min_score: Option<f32>,
    ) -> Result<Vec<ScoredSource>, VectorError> {
        self.validate_view(snap)?;
        let t = self.lmpg.tenant;
        if query.len() != space.dims as usize {
            return Err(VectorError::DimMismatch(space.dims, query.len()));
        }
        if query.iter().any(|v| !v.is_finite()) {
            return Err(VectorError::InvalidVector(
                "query components must be finite".into(),
            ));
        }
        if k == 0 {
            return Ok(Vec::new());
        }

        // P14 bounded top-K heap: retain only the `k` best scores in a min-heap by
        // score (the heap's top is the current k-th best). Peak memory is
        // O(k), not O(bindings), so exact search on a large index is bounded.
        let mut heap: BinaryHeap<std::cmp::Reverse<ScoredSource>> = BinaryHeap::new();
        // Stream the bindings in bounded batches: materializing every binding
        // row first made exact search O(bindings) in memory, not O(k)
        // (C9, cloud-capacity F10).
        let mut cursor = snap.scan(
            rushdb_storage::ScanBounds::prefix(&emb_bindings_prefix(t, space.space_id)),
            rushdb_storage::ScanProjection::KeyValue,
        )?;
        loop {
            let batch = cursor.next_batch(EXACT_SCAN_BATCH_ROWS, EXACT_SCAN_BATCH_BYTES)?;
            for row in batch.rows {
                let raw = row.value.unwrap_or_default();
                self.score_exact_binding(
                    snap, space, query, k, candidate, min_score, &raw, &mut heap,
                )?;
            }
            if batch.done {
                break;
            }
        }
        let mut scored: Vec<ScoredSource> = heap.into_iter().map(|r| r.0).collect();
        // Deterministic: score descending, then canonical source.
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| source_order(a.source).cmp(&source_order(b.source)))
        });
        Ok(scored)
    }

    /// Score one binding row into the bounded top-`k` heap (exact search).
    #[expect(
        clippy::too_many_arguments,
        reason = "one streamed row folded into the caller's heap; bundling the arguments would only move them into a single-use struct"
    )]
    fn score_exact_binding(
        &self,
        snap: &ReadView,
        space: &EmbeddingSpace,
        query: &[f32],
        k: usize,
        candidate: Option<&dyn Fn(SourceRef) -> bool>,
        min_score: Option<f32>,
        raw: &[u8],
        heap: &mut BinaryHeap<std::cmp::Reverse<ScoredSource>>,
    ) -> Result<(), VectorError> {
        let t = self.lmpg.tenant;
        {
            let b: EmbeddingBinding =
                serde_json::from_slice(raw).map_err(|_| StorageError::CorruptLog)?;
            if b.space_id != space.space_id {
                return Err(StorageError::CorruptLog.into());
            }
            if b.status != BindingStatus::Ready {
                return Ok(());
            }
            let src = SourceRef {
                kind: b.owner_kind,
                owner_ord: b.owner_ord,
                pid: b.pid,
                elem: b.elem,
            };
            if candidate.is_some_and(|f| !f(src)) {
                return Ok(());
            }
            let vec = decode_vector(
                snap.get(&emb_vector_key(t, space.space_id, b.vector_slot))?,
                space.dims,
            )?;
            let score = score(query, &vec, space.similarity);
            if min_score.is_some_and(|min| score < min) {
                return Ok(());
            }
            heap.push(std::cmp::Reverse(ScoredSource {
                source: src,
                score,
                value_hash: b.source_hash,
            }));
            if heap.len() > k {
                heap.pop();
            }
        }
        Ok(())
    }

    /// Ranked sources projected through owner JSON reconstruction across
    /// Record and Relationship perspectives (slice item 9 surface).
    /// This convenience method captures a new view. Use the explicit in-view
    /// methods to score and reconstruct one statement at the same version.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if the snapshot or owner
    /// reconstruction fails.
    pub fn reconstruct_sources(
        &self,
        hits: &[ScoredSource],
    ) -> Result<Vec<serde_json::Value>, VectorError> {
        let snap = self.snapshot()?;
        self.reconstruct_sources_in_view(&snap, hits)
    }

    /// Reconstruct owner JSON for ranked `hits` within a caller-owned view.
    /// The view must belong to this engine's store.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] if `snap` belongs to another store or
    /// an owner record or relationship cannot be rebuilt.
    pub fn reconstruct_sources_in_view(
        &self,
        snap: &ReadView,
        hits: &[ScoredSource],
    ) -> Result<Vec<serde_json::Value>, VectorError> {
        self.validate_view(snap)?;
        hits.iter()
            .map(|h| match h.source.kind {
                OwnerKind::Record => Ok(rushdb_lmpg::reconstruct::reconstruct_record(
                    snap,
                    self.lmpg.tenant,
                    h.source.owner_ord,
                )?
                .unwrap_or(serde_json::Value::Null)),
                OwnerKind::Relationship => Ok(rushdb_lmpg::reconstruct::reconstruct_relationship(
                    snap,
                    self.lmpg.tenant,
                    h.source.owner_ord,
                )?
                .unwrap_or(serde_json::Value::Null)),
            })
            .collect()
    }
}

/// Binding counts of one Space by status ([`VectorEngine::binding_counts_in_view`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BindingCounts {
    /// Every binding.
    pub total: usize,
    /// Searchable bindings.
    pub ready: usize,
    /// Bindings whose source value changed.
    pub stale: usize,
    /// Bindings whose source is gone.
    pub retired: usize,
    /// Bindings tracking their source value's hash (native, not external).
    pub tracked: usize,
}

/// Stream every binding of `space` to `visit` (until it returns `false`)
/// without collecting the rows: a space past ~230K bindings overflowed the
/// 64 MiB scan ceiling and failed every search.
pub(crate) fn for_each_binding<R: rushdb_lmpg::reader::TryRead + ?Sized>(
    r: &R,
    t: TenantId,
    space: u64,
    visit: &mut dyn FnMut(EmbeddingBinding) -> bool,
) -> Result<(), VectorError> {
    let (start, end) = rushdb_storage::prefix_range(&emb_bindings_prefix(t, space));
    let mut corrupt = false;
    r.for_each_row(&start, &end, &mut |_, raw| {
        if let Ok(binding) = serde_json::from_slice(raw) {
            visit(binding)
        } else {
            corrupt = true;
            false
        }
    })?;
    if corrupt {
        return Err(StorageError::CorruptLog.into());
    }
    Ok(())
}
