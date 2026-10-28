//! Multi-segment ANN architecture (tenancy buckets 9a/9b): one
//! hot segment (brute-force, absorbs new writes) plus N immutable frozen
//! segments (`usearch`-backed, mmap-eligible). [`crate::ann`]'s
//! `AnnBacking::Segments` is the live default path for every space; see
//! `docs/manual/architecture/vector-internals.md` for the design this
//! implements.
//!
//! Corrected read path (the blueprint's §5.3, replacing the rejected
//! per-segment-concat design): gather candidates per segment, resolve each
//! candidate's canonical source to its **newest** segment (an update in a
//! later segment supersedes an older one — this is what makes the design
//! correct under multiple segments, where the single-generation path never
//! had to consider it), then exact-rerank the deduped set from canonical
//! vectors.
//!
//! **Bucket 9b, fully landed:** `HotSegment::push` absorbs a single bind
//! incrementally (`ann::VectorEngine::incrementally_absorb_bind`), without
//! rebuilding the whole space from canonical bindings; `SegmentSet`'s
//! tombstone set (below) removes a source from search results independent
//! of a rebind, without touching any segment's physical index; and
//! `FrozenSegment::build`/`open` persist/reopen a frozen segment via
//! `usearch`'s mmap `.view()` under the store's data directory when one is
//! configured (`ann::VectorEngine::with_segments_dir`), falling back to
//! `build_in_memory` for a pure in-memory store. A tombstoned source is
//! simply absent from `search`'s resolved candidates, never from a
//! segment's own entries.
//!
//! **Reopen:** every persisted segment carries its entries in a
//! sidecar, so a cache miss (restart, tenant eviction) reopens the space's
//! whole directory ([`open_space_dir`]) and [`SegmentSet::reconcile`]s it
//! against the current bindings: an entry stays live only if it is its
//! source's current binding, and only bindings no segment covers are
//! decoded again.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arc_swap::ArcSwap;
use rushdb_types::occurrence::Similarity;
#[cfg(test)]
use std::collections::HashMap;

use crate::{ScoredSource, SourceRef, VectorError};

/// On-disk schema version of a frozen segment's manifest. Bump and
/// rebuild-on-mismatch on any incompatible change, the same discipline
/// `ann::AnnManifest::format_version` already applies.
const SEGMENT_FORMAT_VERSION: u8 = 2;

/// One canonical vector admitted to a segment: its storage slot, the source
/// it projects, and the value hash recorded at bind time. Mirrors
/// `ann::AnnEntry`; kept separate because segments have their own identity
/// (position within ONE segment, not a single generation's slot order).
#[derive(Debug, Clone)]
pub(crate) struct SegmentEntry {
    pub slot: u64,
    pub source: SourceRef,
    pub value_hash: u64,
}

/// Durable description of a frozen segment's persisted provider file.
/// Reused only when `input_fingerprint` matches the canonical bindings it
/// was built from; otherwise the segment is a cache miss and must be
/// rebuilt, never treated as data loss (the canonical vectors are the
/// source of truth).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SegmentManifest {
    pub format_version: u8,
    pub input_fingerprint: String,
    pub entries: usize,
    pub dims: u32,
}

/// Fingerprints a segment's entries the same way as
/// `ann::generation_fingerprint`, so a frozen segment's reconciliation
/// check is defined identically to the single-generation path's.
pub(crate) fn segment_fingerprint(entries: &[SegmentEntry]) -> String {
    let mut hash = blake3::Hasher::new();
    for entry in entries {
        hash.update(&entry.slot.to_be_bytes());
        hash.update(&entry.value_hash.to_be_bytes());
        hash.update(&[entry.source.kind.as_byte()]);
        hash.update(&entry.source.owner_ord.to_be_bytes());
        hash.update(&entry.source.pid.to_be_bytes());
        hash.update(&entry.source.elem.to_be_bytes());
    }
    hash.finalize().to_hex().to_string()
}

/// Absorbs new writes. Exact brute-force scan, no graph — kept small by the
/// caller's freeze threshold, so a flat SIMD-friendly scan is cheap and a
/// provider graph would be pure overhead for few entries.
#[derive(Clone)]
pub(crate) struct HotSegment {
    entries: Vec<SegmentEntry>,
    vectors: Vec<Vec<f32>>,
}

impl HotSegment {
    pub(crate) fn new() -> Self {
        HotSegment {
            entries: Vec::new(),
            vectors: Vec::new(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Real owned bytes: entry metadata plus every vector's payload
    /// — exact, not a formula estimate.
    pub(crate) fn measured_bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<SegmentEntry>()
            + self
                .vectors
                .iter()
                .map(|v| v.len() * std::mem::size_of::<f32>())
                .sum::<usize>()
    }

    pub(crate) fn push(&mut self, entry: SegmentEntry, vector: Vec<f32>) {
        self.entries.push(entry);
        self.vectors.push(vector);
    }

    fn gather(&self, query: &[f32], k: usize, similarity: Similarity) -> Vec<usize> {
        let mut scored: Vec<(f32, usize)> = self
            .vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (crate::score(query, v, similarity), i))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(k).map(|(_, i)| i).collect()
    }
}

impl Default for HotSegment {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable, `usearch`-backed segment. Its graph is either built fresh in
/// memory (`build`) or reopened via `usearch`'s mmap `.view()` (`open`), so
/// a large frozen segment need not be fully resident — the OS page cache
/// bounds RSS, matching the blueprint's memory model (§3.2).
pub(crate) struct FrozenSegment {
    entries: Vec<SegmentEntry>,
    // Read by `manifest()`, used for lifecycle reporting
    // (`ann_lifecycle_in_view`'s `manifestState`/`providerState` shape) and
    // this module's own tests — always populated regardless, so a disk
    // round-trip can validate it.
    manifest: SegmentManifest,
    /// Always a *view*: of `buffer` for an in-memory segment, of
    /// `file` for a persisted one. Building allocates a growable arena (2 ×
    /// 8 MiB minimum, mostly reserved and unused: 16.9 MB for a 5 MB
    /// 3K×768 index); viewing the serialized form holds exactly its bytes.
    /// Declared before `buffer`/`file` so it is dropped first.
    index: usearch::Index,
    /// Serialized index the in-memory view reads (owned, stable address).
    buffer: Option<Box<[u8]>>,
    /// The persisted file the view maps; deleted once superseded.
    file: Option<SegmentFile>,
}

/// A persisted segment file. Removed (with its sidecars) when the last
/// segment holding it drops after [`FrozenSegment::supersede`], so a
/// merged-away or replaced segment never lingers on disk. A
/// segment merely evicted from the cache keeps its file for reuse.
struct SegmentFile {
    path: PathBuf,
    superseded: std::sync::atomic::AtomicBool,
    /// Identity of the file this segment maps; a different file at the same
    /// path is never deleted by this guard.
    identity: Option<(u64, u64)>,
}

fn file_identity(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path)
            .ok()
            .map(|meta| (meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

impl SegmentFile {
    fn new(path: &Path) -> Self {
        SegmentFile {
            path: path.to_path_buf(),
            superseded: std::sync::atomic::AtomicBool::new(false),
            identity: file_identity(path),
        }
    }
}

impl Drop for SegmentFile {
    fn drop(&mut self) {
        let doomed = self.superseded.load(std::sync::atomic::Ordering::SeqCst)
            && file_identity(&self.path) == self.identity;
        if doomed {
            // The segment file goes first: a reader of the directory never
            // sees a segment without its sidecars.
            for path in [
                self.path.clone(),
                manifest_path_for(&self.path),
                entries_path_for(&self.path),
            ] {
                if let Err(error) = std::fs::remove_file(&path) {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        eprintln!(
                            "rushdb: cannot remove superseded vector segment {}: {error}",
                            path.display()
                        );
                    }
                }
            }
        }
    }
}

/// Graph search breadth fixed at build/open time. Per-query `ef` is honored by
/// over-fetching `max(k, ef)` candidates instead of mutating the shared
/// index's expansion, which raced between concurrent queries (C9,
/// cloud-capacity F11).
const SEARCH_EXPANSION: usize = 128;

fn usearch_options(dims: u32, similarity: Similarity) -> usearch::IndexOptions {
    usearch::IndexOptions {
        dimensions: dims as usize,
        metric: match similarity {
            Similarity::Cosine => usearch::MetricKind::Cos,
            Similarity::Dot => usearch::MetricKind::IP,
            Similarity::Euclidean => usearch::MetricKind::L2sq,
        },
        // f16 halves the graph's resident vectors. Candidates are exactly
        // reranked against the canonical f32 vectors, so quantization can
        // only affect which candidates are gathered, never the returned
        // scores.
        quantization: usearch::ScalarKind::F16,
        connectivity: 0,
        expansion_add: 0,
        expansion_search: SEARCH_EXPANSION,
        multi: false,
    }
}

/// Builds a `usearch` index over `len` vectors, consuming them one at a time:
/// a caller decoding from storage never holds the f32 corpus, only the
/// index under construction.
fn build_index<V: AsRef<[f32]>>(
    len: usize,
    vectors: impl IntoIterator<Item = Result<V, VectorError>>,
    dims: u32,
    similarity: Similarity,
) -> Result<usearch::Index, VectorError> {
    let options = usearch_options(dims, similarity);
    let index = usearch::Index::new(&options).map_err(|e| VectorError::Engine(e.to_string()))?;
    index
        .reserve(len.max(1))
        .map_err(|e| VectorError::Engine(e.to_string()))?;
    let mut added = 0usize;
    for (key, vector) in vectors.into_iter().enumerate() {
        index
            .add(key as u64, vector?.as_ref())
            .map_err(|e| VectorError::Engine(e.to_string()))?;
        added += 1;
    }
    if added != len {
        return Err(VectorError::Engine(format!(
            "segment build: {added} vectors for {len} entries"
        )));
    }
    Ok(index)
}

/// Bytes [`build_index`] holds per vector of `dims` while building, and a
/// built segment's serialized size per vector: the f16 vector, its graph
/// links and the entry. Checked against real builds by
/// `build_bytes_estimate_covers_real_segments`.
pub(crate) fn build_bytes_per_vector(dims: u32) -> usize {
    (dims as usize) * 2 + 320
}

/// One persisted entry in the `.entries` sidecar: slot, owner kind, owner
/// ordinal, pid, element, value hash.
const ENTRY_RECORD_BYTES: usize = 8 + 1 + 8 + 8 + 8 + 8;
const ENTRIES_MAGIC: &[u8; 4] = b"RSE2";

fn encode_entries(entries: &[SegmentEntry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 8 + entries.len() * ENTRY_RECORD_BYTES);
    out.extend_from_slice(ENTRIES_MAGIC);
    out.extend_from_slice(&(entries.len() as u64).to_le_bytes());
    for entry in entries {
        out.extend_from_slice(&entry.slot.to_le_bytes());
        out.push(entry.source.kind.as_byte());
        out.extend_from_slice(&entry.source.owner_ord.to_le_bytes());
        out.extend_from_slice(&entry.source.pid.to_le_bytes());
        out.extend_from_slice(&entry.source.elem.to_le_bytes());
        out.extend_from_slice(&entry.value_hash.to_le_bytes());
    }
    out
}

fn decode_entries(bytes: &[u8]) -> Option<Vec<SegmentEntry>> {
    let u64_at = |at: usize| -> Option<u64> {
        Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
    };
    if bytes.get(..4)? != ENTRIES_MAGIC {
        return None;
    }
    let len = usize::try_from(u64_at(4)?).ok()?;
    if bytes.len() != 12 + len.checked_mul(ENTRY_RECORD_BYTES)? {
        return None;
    }
    (0..len)
        .map(|i| {
            let at = 12 + i * ENTRY_RECORD_BYTES;
            Some(SegmentEntry {
                slot: u64_at(at)?,
                source: SourceRef {
                    kind: rushdb_types::ids::OwnerKind::from_byte(bytes[at + 8])?,
                    owner_ord: u64_at(at + 9)?,
                    pid: u64_at(at + 17)?,
                    elem: u64_at(at + 25)?,
                },
                value_hash: u64_at(at + 33)?,
            })
        })
        .collect()
}

impl FrozenSegment {
    /// Real resident bytes: the underlying `usearch` index's own
    /// measured memory usage (near-zero for an mmap-reopened, `.view()`d
    /// index, since the OS page cache backs it rather than the heap) plus
    /// this segment's own entry metadata.
    pub(crate) fn measured_bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<SegmentEntry>()
            + self.index.memory_usage()
            + self.buffer.as_ref().map_or(0, |buffer| buffer.len())
    }

    /// Builds a fresh frozen segment from `entries`/`vectors` (same order),
    /// resident in memory only — no disk persistence. Used directly by a
    /// pure in-memory store (no configured segments directory). The built
    /// graph is serialized and viewed, releasing the build arena.
    ///
    /// # Errors
    /// Returns [`VectorError::Engine`] when the `usearch` index cannot be
    /// built or serialized, and whatever a vector of `vectors` yields.
    pub(crate) fn build_in_memory<V: AsRef<[f32]>>(
        entries: Vec<SegmentEntry>,
        vectors: impl IntoIterator<Item = Result<V, VectorError>>,
        dims: u32,
        similarity: Similarity,
    ) -> Result<Self, VectorError> {
        let built = build_index(entries.len(), vectors, dims, similarity)?;
        let mut buffer = vec![0u8; built.serialized_length()].into_boxed_slice();
        built
            .save_to_buffer(&mut buffer)
            .map_err(|e| VectorError::Engine(e.to_string()))?;
        drop(built);
        let index = usearch::Index::new(&usearch_options(dims, similarity))
            .map_err(|e| VectorError::Engine(e.to_string()))?;
        // SAFETY: `buffer` is owned by the returned segment, never mutated,
        // and declared after `index`, so it outlives every use of the view.
        unsafe { index.view_from_buffer(&buffer) }
            .map_err(|e| VectorError::Engine(e.to_string()))?;
        let manifest = SegmentManifest {
            format_version: SEGMENT_FORMAT_VERSION,
            input_fingerprint: segment_fingerprint(&entries),
            entries: entries.len(),
            dims,
        };
        Ok(FrozenSegment {
            entries,
            manifest,
            index,
            buffer: Some(buffer),
            file: None,
        })
    }

    /// Builds a fresh frozen segment from `entries`/`vectors` (same order),
    /// persists it under `path` as `usearch`'s own single-file format plus
    /// two sidecars (`<path>.manifest.json`, `<path>.entries`), and returns a
    /// view of that file (the build arena is released).
    ///
    /// # Errors
    /// Returns [`VectorError::Engine`] when the `usearch` index cannot be
    /// built, persisted or reopened, and whatever a vector of `vectors`
    /// yields.
    pub(crate) fn build<V: AsRef<[f32]>>(
        entries: Vec<SegmentEntry>,
        vectors: impl IntoIterator<Item = Result<V, VectorError>>,
        dims: u32,
        similarity: Similarity,
        path: &Path,
    ) -> Result<Self, VectorError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VectorError::Engine(format!("segment directory create: {e}")))?;
        }
        let built = build_index(entries.len(), vectors, dims, similarity)?;
        // Written aside and renamed into place, sidecars first: a reader of
        // the directory never sees a segment file without its sidecars.
        let staged = path.with_extension(format!("tmp-{}", uuid::Uuid::now_v7().simple()));
        built
            .save(&staged.to_string_lossy())
            .map_err(|e| VectorError::Engine(e.to_string()))?;
        drop(built);
        let manifest = SegmentManifest {
            format_version: SEGMENT_FORMAT_VERSION,
            input_fingerprint: segment_fingerprint(&entries),
            entries: entries.len(),
            dims,
        };
        let manifest_bytes = serde_json::to_vec(&manifest)
            .map_err(|e| VectorError::Engine(format!("segment manifest encode: {e}")))?;
        let staged_manifest = manifest_path_for(&staged);
        let staged_entries = entries_path_for(&staged);
        std::fs::write(&staged_manifest, manifest_bytes)
            .and_then(|()| std::fs::write(&staged_entries, encode_entries(&entries)))
            .map_err(|e| VectorError::Engine(format!("segment sidecar write: {e}")))?;
        std::fs::rename(&staged_entries, entries_path_for(path))
            .and_then(|()| std::fs::rename(&staged_manifest, manifest_path_for(path)))
            .and_then(|()| std::fs::rename(&staged, path))
            .map_err(|e| VectorError::Engine(format!("segment publish: {e}")))?;
        let index = usearch::Index::new(&usearch_options(dims, similarity))
            .map_err(|e| VectorError::Engine(e.to_string()))?;
        index
            .view(&path.to_string_lossy())
            .map_err(|e| VectorError::Engine(e.to_string()))?;
        Ok(FrozenSegment {
            entries,
            manifest,
            index,
            buffer: None,
            file: Some(SegmentFile::new(path)),
        })
    }

    /// Mark this segment's file for deletion once the last holder drops it
    /// (a merge absorbed it, a rebuild replaced it, or it holds no live
    /// entry). No-op for an in-memory segment.
    pub(crate) fn supersede(&self) {
        if let Some(file) = &self.file {
            file.superseded
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Reopens a persisted frozen segment via `usearch`'s mmap `.view()`,
    /// with its entries read back from the `.entries` sidecar and checked
    /// against the manifest's fingerprint. A torn file, missing or
    /// mismatched sidecar, or another dimension is `Ok(None)`: the
    /// canonical vectors remain the source of truth, so the caller simply
    /// decodes what no segment covers.
    ///
    /// # Errors
    /// Returns [`VectorError::Engine`] only for an unexpected I/O failure
    /// reading a sidecar; a missing/corrupt/mismatched one is `Ok(None)`.
    pub(crate) fn open(
        path: &Path,
        dims: u32,
        similarity: Similarity,
    ) -> Result<Option<Self>, VectorError> {
        let read = |path: PathBuf| match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(VectorError::Engine(format!(
                "segment sidecar read {}: {e}",
                path.display()
            ))),
        };
        let (Some(manifest_bytes), Some(entry_bytes)) = (
            read(manifest_path_for(path))?,
            read(entries_path_for(path))?,
        ) else {
            return Ok(None);
        };
        let Ok(manifest) = serde_json::from_slice::<SegmentManifest>(&manifest_bytes) else {
            return Ok(None);
        };
        let Some(entries) = decode_entries(&entry_bytes) else {
            return Ok(None);
        };
        if manifest.format_version != SEGMENT_FORMAT_VERSION
            || manifest.dims != dims
            || manifest.entries != entries.len()
            || manifest.input_fingerprint != segment_fingerprint(&entries)
        {
            return Ok(None);
        }
        let options = usearch_options(dims, similarity);
        let Ok(index) = usearch::Index::new(&options) else {
            return Ok(None);
        };
        if index.view(&path.to_string_lossy()).is_err() || index.size() != entries.len() {
            return Ok(None);
        }
        Ok(Some(FrozenSegment {
            entries,
            manifest,
            index,
            buffer: None,
            file: Some(SegmentFile::new(path)),
        }))
    }

    /// Raw candidate gather, bypassing dedup/rerank — exposed at
    /// `pub(crate)` for `ann.rs`'s own recall-regression tests, which
    /// exercise the live `usearch` graph directly rather than duplicating
    /// an index-construction harness.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "keys are vector positions assigned by build() as `key as u64` from a usize index, so they always round-trip back"
    )]
    pub(crate) fn gather(&self, query: &[f32], k: usize, ef: usize) -> Vec<usize> {
        self.index
            .search(query, k.max(ef))
            .map(|matches| matches.keys.into_iter().map(|key| key as usize).collect())
            .unwrap_or_default()
    }

    /// This segment's persisted manifest. Not yet surfaced by
    /// `ann_lifecycle_in_view` (which reports the space-level `AnnManifest`
    /// from the KV store, not per-segment provenance) — exercised today by
    /// this module's own tests; per-segment lifecycle reporting is a
    /// natural follow-on.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "per-segment lifecycle reporting not yet wired")
    )]
    pub(crate) fn manifest(&self) -> &SegmentManifest {
        &self.manifest
    }
}

fn manifest_path_for(path: &Path) -> PathBuf {
    sidecar_path(path, ".manifest.json")
}

fn entries_path_for(path: &Path) -> PathBuf {
    sidecar_path(path, ".entries")
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// Whether `name` is a published segment file (not a sidecar, not staged).
/// Segment names are `seg-<uuid v7>`, so their order is creation order.
pub(crate) fn is_segment_file_name(name: &str) -> bool {
    name.starts_with("seg-") && !name.contains('.')
}

/// Every persisted segment of one space, oldest first. A segment
/// file whose sidecars or view are invalid is derived garbage and is
/// deleted; the caller's reconcile decodes whatever it covered.
///
/// # Errors
/// Returns [`VectorError::Engine`] when the directory or a sidecar cannot
/// be read (other than not existing).
pub(crate) fn open_space_dir(
    dir: &Path,
    dims: u32,
    similarity: Similarity,
) -> Result<Vec<FrozenSegment>, VectorError> {
    let listing = match std::fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(VectorError::Engine(format!(
                "segment directory read {}: {e}",
                dir.display()
            )))
        }
    };
    let mut names: Vec<String> = listing
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_segment_file_name(name))
        .collect();
    names.sort();
    let mut segments = Vec::with_capacity(names.len());
    for name in names {
        let path = dir.join(&name);
        match FrozenSegment::open(&path, dims, similarity)? {
            Some(segment) => segments.push(segment),
            None => {
                for doomed in [
                    path.clone(),
                    manifest_path_for(&path),
                    entries_path_for(&path),
                ] {
                    if let Err(error) = std::fs::remove_file(&doomed) {
                        if error.kind() != std::io::ErrorKind::NotFound {
                            eprintln!(
                                "rushdb: cannot remove invalid vector segment {}: {error}",
                                doomed.display()
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(segments)
}

/// Either tier a segment can be in. Immutable once constructed; a
/// [`SegmentSet`] swap replaces the whole `Vec`, never mutates a segment in
/// place.
pub(crate) enum Segment {
    Hot(HotSegment),
    Frozen(FrozenSegment),
}

impl Segment {
    pub(crate) fn entries(&self) -> &[SegmentEntry] {
        match self {
            Segment::Hot(hot) => &hot.entries,
            Segment::Frozen(frozen) => &frozen.entries,
        }
    }

    pub(crate) fn measured_bytes(&self) -> usize {
        match self {
            Segment::Hot(hot) => hot.measured_bytes(),
            Segment::Frozen(frozen) => frozen.measured_bytes(),
        }
    }

    fn gather(&self, query: &[f32], k: usize, ef: usize, similarity: Similarity) -> Vec<usize> {
        match self {
            Segment::Hot(hot) => hot.gather(query, k, similarity),
            Segment::Frozen(frozen) => frozen.gather(query, k, ef),
        }
    }
}

pub(crate) type SourceKey = (u8, u64, u64, u64);

pub(crate) fn source_key(source: SourceRef) -> SourceKey {
    (
        source.kind.as_byte(),
        source.owner_ord,
        source.pid,
        source.elem,
    )
}

/// Size-tiered merge: segments whose entry counts share a power-of-`MERGE_FACTOR`
/// band (above the hot freeze size) form a tier, and `MERGE_FACTOR` frozen
/// segments of one tier merge into one of the next (Lucene's
/// `TieredMergePolicy` in its simplest form). Segment count is therefore
/// logarithmic in the space size, not linear in its write history.
pub(crate) const MERGE_FACTOR: usize = 10;

/// One immutable state of a [`SegmentSet`]: the segments (oldest first),
/// their stable ids, and the live map. Swapped as a unit, so a reader always
/// sees segments and liveness that agree.
#[derive(Clone)]
struct SetState {
    segments: Arc<Vec<Arc<Segment>>>,
    ids: Arc<Vec<u64>>,
    /// Canonical source → `(segment id, entry index)` of its one live entry,
    /// maintained at mutation time: a rebind moves it, a retirement
    /// removes it, a merge re-points it. A candidate any segment gathers
    /// counts only if the live map points at it, so newest-wins, retirement
    /// and dead (superseded) entries are one check, with no per-query index.
    live: im::HashMap<SourceKey, (u64, usize)>,
    /// Live entries per segment id, kept with `live` so the merge policy
    /// can find mostly-dead segments without a scan.
    live_counts: im::HashMap<u64, usize>,
}

impl SetState {
    /// Point `key` at `(id, index)`, keeping per-segment counts.
    fn set_live(&mut self, key: SourceKey, id: u64, index: usize) {
        if let Some((old, _)) = self.live.insert(key, (id, index)) {
            self.dec(old);
        }
        *self.live_counts.entry(id).or_insert(0) += 1;
    }

    fn remove_live(&mut self, key: &SourceKey) {
        if let Some((old, _)) = self.live.remove(key) {
            self.dec(old);
        }
    }

    fn dec(&mut self, id: u64) {
        if let Some(count) = self.live_counts.get_mut(&id) {
            *count = count.saturating_sub(1);
        }
    }
}

/// Ordered, immutable snapshot of a space's segments behind a lock-free
/// `ArcSwap` lease: a reader never blocks on a concurrent freeze or merge,
/// and a writer never blocks a concurrent read. Writers (the absorb path and
/// the maintenance merge) [`Self::fork`] the set and publish the fork, so an
/// in-flight reader keeps exactly the state it started with.
pub(crate) struct SegmentSet {
    state: ArcSwap<SetState>,
    /// Monotonic id for segments and their on-disk files (`seg-<n>`).
    next_segment_id: std::sync::atomic::AtomicU64,
}

impl SegmentSet {
    pub(crate) fn new() -> Self {
        SegmentSet {
            state: ArcSwap::new(Arc::new(SetState {
                segments: Arc::new(Vec::new()),
                ids: Arc::new(Vec::new()),
                live: im::HashMap::new(),
                live_counts: im::HashMap::new(),
            })),
            next_segment_id: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// An independent copy of this set (O(1): segments are shared `Arc`s and
    /// the live map is persistent). Mutating the fork never affects a reader
    /// of the original.
    pub(crate) fn fork(&self) -> Self {
        SegmentSet {
            state: ArcSwap::new(self.state.load_full()),
            next_segment_id: std::sync::atomic::AtomicU64::new(self.peek_segment_id()),
        }
    }

    /// The next segment id this set would allocate.
    pub(crate) fn peek_segment_id(&self) -> u64 {
        self.next_segment_id
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Read-only snapshot of this set's current segments.
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Arc<Vec<Arc<Segment>>> {
        Arc::clone(&self.state.load().segments)
    }

    /// Append a segment; every entry becomes the live one for its source
    /// (later entries win within the segment too).
    pub(crate) fn push(&self, segment: Arc<Segment>) {
        let id = self.alloc_segment_id();
        let mut state = (**self.state.load()).clone();
        for (index, entry) in segment.entries().iter().enumerate() {
            state.set_live(source_key(entry.source), id, index);
        }
        Arc::make_mut(&mut state.segments).push(segment);
        Arc::make_mut(&mut state.ids).push(id);
        self.state.store(Arc::new(state));
    }

    /// Appends one freshly bound entry to the newest hot segment (creating
    /// one if the newest segment is frozen or none exists) and makes it the
    /// source's live entry; the previous one becomes dead. The caller
    /// serializes writers (production holds the space's ANN-cache lock).
    pub(crate) fn push_to_hot(&self, entry: SegmentEntry, vector: Vec<f32>) {
        let mut state = (**self.state.load()).clone();
        let (mut hot, id) = match state.segments.last().map(Arc::as_ref) {
            Some(Segment::Hot(hot)) => {
                let hot = hot.clone();
                let id = *state.ids.last().expect("ids parallel segments");
                Arc::make_mut(&mut state.segments).pop();
                Arc::make_mut(&mut state.ids).pop();
                (hot, id)
            }
            _ => (HotSegment::new(), self.alloc_segment_id()),
        };
        state.set_live(source_key(entry.source), id, hot.len());
        hot.push(entry, vector);
        Arc::make_mut(&mut state.segments).push(Arc::new(Segment::Hot(hot)));
        Arc::make_mut(&mut state.ids).push(id);
        self.state.store(Arc::new(state));
    }

    /// Entry count of the newest segment when it is hot, else `0`.
    pub(crate) fn newest_hot_len(&self) -> usize {
        match self.state.load().segments.last().map(Arc::as_ref) {
            Some(Segment::Hot(hot)) => hot.len(),
            _ => 0,
        }
    }

    /// Next id for a segment or its on-disk file, advancing the counter.
    pub(crate) fn alloc_segment_id(&self) -> u64 {
        self.next_segment_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Removes `source` from every future [`Self::search`] result: it has no
    /// live entry any more. Its physical entries stay in their segments
    /// until a merge drops them.
    pub(crate) fn tombstone(&self, source: SourceRef) {
        let key = source_key(source);
        if !self.state.load().live.contains_key(&key) {
            return;
        }
        let mut state = (**self.state.load()).clone();
        state.remove_live(&key);
        self.state.store(Arc::new(state));
    }

    /// Segment count (hot + frozen).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.state.load().segments.len()
    }

    /// Live (searchable) source count.
    #[cfg(test)]
    pub(crate) fn live_len(&self) -> usize {
        self.state.load().live.len()
    }

    /// Real resident bytes: each segment's measured footprint plus the live
    /// map.
    pub(crate) fn measured_bytes(&self) -> usize {
        let state = self.state.load();
        state
            .segments
            .iter()
            .map(|segment| segment.measured_bytes())
            .sum::<usize>()
            + state.live.len()
                * (std::mem::size_of::<SourceKey>() + std::mem::size_of::<(u64, usize)>())
                * 2
    }

    /// Freeze path: if the newest segment is a hot segment with at least
    /// `min_len` entries, build it into a frozen segment — persisted under
    /// `path` when given, resident only otherwise — and swap it in under the
    /// same id (its live entries stay valid). `Ok(false)` when there is
    /// nothing to freeze.
    ///
    /// # Errors
    /// Returns whatever [`FrozenSegment::build`] returns; the hot segment is
    /// left untouched on error.
    pub(crate) fn freeze_newest_hot(
        &self,
        dims: u32,
        similarity: Similarity,
        path: Option<&Path>,
    ) -> Result<bool, VectorError> {
        self.freeze_hot_at_least(1, dims, similarity, path)
    }

    /// [`Self::freeze_newest_hot`] with a minimum size.
    ///
    /// # Errors
    /// See [`Self::freeze_newest_hot`].
    pub(crate) fn freeze_hot_at_least(
        &self,
        min_len: usize,
        dims: u32,
        similarity: Similarity,
        path: Option<&Path>,
    ) -> Result<bool, VectorError> {
        let current = self.state.load_full();
        let Some(Segment::Hot(hot)) = current.segments.last().map(Arc::as_ref) else {
            return Ok(false);
        };
        if hot.len() == 0 || hot.len() < min_len {
            return Ok(false);
        }
        let vectors = hot.vectors.iter().map(Ok);
        let frozen = match path {
            Some(path) => {
                FrozenSegment::build(hot.entries.clone(), vectors, dims, similarity, path)?
            }
            None => FrozenSegment::build_in_memory(hot.entries.clone(), vectors, dims, similarity)?,
        };
        let mut state = (*current).clone();
        let segments = Arc::make_mut(&mut state.segments);
        segments.pop();
        segments.push(Arc::new(Segment::Frozen(frozen)));
        self.state.store(Arc::new(state));
        Ok(true)
    }

    /// The frozen segments to merge next, oldest first, or `None`:
    /// segments that are at least half dead (rebound or retired entries) go
    /// first, rewritten with only their live entries (Lucene's
    /// deletes-aware merging); otherwise up to [`MERGE_FACTOR`] segments of
    /// the lowest full size tier. A merge never builds more than
    /// `max_entries` live entries (the build budget): a tier too
    /// large to merge stays as it is.
    pub(crate) fn merge_candidates(
        &self,
        base_size: usize,
        max_entries: usize,
    ) -> Option<Vec<u64>> {
        let state = self.state.load();
        let mut dead_heavy = Vec::new();
        let mut tiers: std::collections::BTreeMap<u32, Vec<u64>> =
            std::collections::BTreeMap::new();
        for (segment, id) in state.segments.iter().zip(state.ids.iter()) {
            if let Segment::Frozen(frozen) = segment.as_ref() {
                let size = frozen.entries.len().max(1);
                let live = state.live_counts.get(id).copied().unwrap_or(0);
                if live * 2 <= size {
                    dead_heavy.push(*id);
                    continue;
                }
                let mut tier = 0u32;
                let mut bound = base_size.max(1) * MERGE_FACTOR;
                while live >= bound {
                    tier += 1;
                    bound = bound.saturating_mul(MERGE_FACTOR);
                }
                tiers.entry(tier).or_default().push(*id);
            }
        }
        let live_of = |id: &u64| state.live_counts.get(id).copied().unwrap_or(0);
        let mut budgeted = |ids: Vec<u64>| -> Vec<u64> {
            let mut total = 0usize;
            ids.into_iter()
                .take(MERGE_FACTOR)
                .take_while(|id| {
                    total = total.saturating_add(live_of(id));
                    total <= max_entries
                })
                .collect()
        };
        if !dead_heavy.is_empty() {
            let picked = budgeted(dead_heavy);
            return (!picked.is_empty()).then_some(picked);
        }
        tiers
            .into_values()
            .filter(|ids| ids.len() >= MERGE_FACTOR)
            .map(&mut budgeted)
            .find(|picked| picked.len() >= MERGE_FACTOR)
    }

    /// The live entries held by the segments `ids`, in segment order.
    pub(crate) fn live_entries_in(&self, ids: &[u64]) -> Vec<SegmentEntry> {
        let state = self.state.load();
        let mut out = Vec::new();
        for (segment, id) in state.segments.iter().zip(state.ids.iter()) {
            if !ids.contains(id) {
                continue;
            }
            for (index, entry) in segment.entries().iter().enumerate() {
                if state.live.get(&source_key(entry.source)) == Some(&(*id, index)) {
                    out.push(entry.clone());
                }
            }
        }
        out
    }

    /// Replace the segments `ids` (all still present) by `merged` (`None`
    /// when they hold no live entry), which
    /// holds exactly their live entries ([`Self::live_entries_in`]) in that
    /// order. Entries rebound or retired since are left to the live map, so
    /// a concurrent absorb is never undone. Whoever publishes the result
    /// supersedes the replaced segments ([`Self::supersede_absent_from`]);
    /// a discarded result must not delete files its source still uses.
    /// Returns `false` (and changes nothing) when a segment is gone.
    pub(crate) fn replace_merged(&self, ids: &[u64], merged: Option<Segment>) -> bool {
        let current = self.state.load_full();
        if !ids.iter().all(|id| current.ids.contains(id)) {
            return false;
        }
        let merged_id = self.alloc_segment_id();
        let mut state = (*current).clone();
        for (index, entry) in merged.iter().flat_map(Segment::entries).enumerate() {
            let key = source_key(entry.source);
            if state.live.get(&key).is_some_and(|(id, _)| ids.contains(id)) {
                state.set_live(key, merged_id, index);
            }
        }
        let mut segments = Vec::with_capacity(current.segments.len());
        let mut new_ids = Vec::with_capacity(current.ids.len());
        // `None`: every entry was dead, the segments simply go.
        let mut merged = merged.map(Arc::new);
        for (segment, id) in current.segments.iter().zip(current.ids.iter()) {
            if ids.contains(id) {
                if let Some(merged) = merged.take() {
                    segments.push(merged);
                    new_ids.push(merged_id);
                }
                continue;
            }
            segments.push(Arc::clone(segment));
            new_ids.push(*id);
        }
        for id in ids {
            state.live_counts.remove(id);
        }
        state.segments = Arc::new(segments);
        state.ids = Arc::new(new_ids);
        self.state.store(Arc::new(state));
        true
    }

    /// Reconcile reopened segments with the space's current bindings
    ///: `current` maps each source to its bound `(slot, value
    /// hash)`. An entry stays live only if it is its source's newest entry
    /// and matches the binding; segments left without a live entry are
    /// dropped and their files superseded. Returns the bindings no segment
    /// covers, which the caller decodes.
    pub(crate) fn reconcile(&self, current: &[SegmentEntry]) -> Vec<SegmentEntry> {
        let wanted: std::collections::HashMap<SourceKey, (u64, u64)> = current
            .iter()
            .map(|entry| (source_key(entry.source), (entry.slot, entry.value_hash)))
            .collect();
        let mut state = (**self.state.load()).clone();
        let outdated: Vec<SourceKey> = state
            .live
            .iter()
            .filter(|(key, (id, index))| {
                let entry = state
                    .ids
                    .iter()
                    .position(|candidate| candidate == id)
                    .and_then(|at| state.segments[at].entries().get(*index));
                entry.is_none_or(|entry| wanted.get(*key) != Some(&(entry.slot, entry.value_hash)))
            })
            .map(|(key, _)| *key)
            .collect();
        for key in &outdated {
            state.remove_live(key);
        }
        let mut segments = Vec::with_capacity(state.segments.len());
        let mut ids = Vec::with_capacity(state.ids.len());
        for (segment, id) in state.segments.iter().zip(state.ids.iter()) {
            if state.live_counts.get(id).copied().unwrap_or(0) == 0 {
                if let Segment::Frozen(frozen) = segment.as_ref() {
                    frozen.supersede();
                }
                state.live_counts.remove(id);
                continue;
            }
            segments.push(Arc::clone(segment));
            ids.push(*id);
        }
        state.segments = Arc::new(segments);
        state.ids = Arc::new(ids);
        let missing = current
            .iter()
            .filter(|entry| !state.live.contains_key(&source_key(entry.source)))
            .cloned()
            .collect();
        self.state.store(Arc::new(state));
        missing
    }

    /// Supersede every segment of this set that `kept` no longer holds:
    /// the publisher of a merge (or the discarder of a lost one) releases
    /// exactly the files nobody newer uses.
    pub(crate) fn supersede_absent_from(&self, kept: &SegmentSet) {
        let kept = kept.state.load();
        for segment in self.state.load().segments.iter() {
            if let Segment::Frozen(frozen) = segment.as_ref() {
                if !kept
                    .segments
                    .iter()
                    .any(|other| Arc::ptr_eq(other, segment))
                {
                    frozen.supersede();
                }
            }
        }
    }

    /// Frozen segment count.
    pub(crate) fn frozen_count(&self) -> usize {
        self.state
            .load()
            .segments
            .iter()
            .filter(|segment| matches!(segment.as_ref(), Segment::Frozen(_)))
            .count()
    }

    /// Mark every persisted segment's file for deletion when its last holder
    /// drops (the whole set was replaced by a rebuild).
    pub(crate) fn supersede_all(&self) {
        for segment in self.state.load().segments.iter() {
            if let Segment::Frozen(frozen) = segment.as_ref() {
                frozen.supersede();
            }
        }
    }

    /// Per-segment gather, then a **global** merge across every segment
    /// (never a per-segment top-k concatenated). A gathered entry counts only
    /// if it is its source's live entry, so a superseded or retired vector
    /// can never surface, and reranking always uses the current one.
    ///
    /// Returns the deduped, filtered candidate count alongside the reranked
    /// top-`k` — the caller's degradation contract (fall back to an exact
    /// scan when ANN confidence is too low) is defined over that pre-rerank
    /// count, not the post-truncate result length.
    ///
    /// # Errors
    /// Propagates whatever `rerank` returns for a resolved candidate.
    pub(crate) fn search(
        &self,
        query: &[f32],
        k: usize,
        ef: usize,
        similarity: Similarity,
        candidate: Option<&dyn Fn(SourceRef) -> bool>,
        rerank: impl Fn(&SegmentEntry) -> Result<ScoredSource, VectorError>,
    ) -> Result<(usize, Vec<ScoredSource>), VectorError> {
        let state = self.state.load_full();
        if state.segments.is_empty() || k == 0 {
            return Ok((0, Vec::new()));
        }
        let gather_k = ef.max(k);
        // Phase timings, emitted only when the
        // `rushdb_vector::search` debug target is enabled.
        let profile = tracing::enabled!(target: "rushdb_vector::search", tracing::Level::DEBUG);
        let mut profile_gather = [(0usize, 0usize, std::time::Duration::ZERO); 2];
        let mut resolved: Vec<&SegmentEntry> = Vec::new();
        for (segment, id) in state.segments.iter().zip(state.ids.iter()) {
            let started = profile.then(std::time::Instant::now);
            let gathered = segment.gather(query, gather_k, gather_k, similarity);
            if let Some(started) = started {
                let kind = usize::from(matches!(**segment, Segment::Frozen(_)));
                profile_gather[kind].0 += 1;
                profile_gather[kind].1 += segment.entries().len();
                profile_gather[kind].2 += started.elapsed();
            }
            for index in gathered {
                let Some(entry) = segment.entries().get(index) else {
                    continue;
                };
                if state.live.get(&source_key(entry.source)) != Some(&(*id, index)) {
                    continue;
                }
                if candidate.is_none_or(|filter| filter(entry.source)) {
                    resolved.push(entry);
                }
            }
        }
        let candidate_count = resolved.len();
        let rerank_started = profile.then(std::time::Instant::now);
        let mut out: Vec<ScoredSource> = resolved
            .into_iter()
            .map(&rerank)
            .collect::<Result<_, _>>()?;
        if let Some(started) = rerank_started {
            let [hot, frozen] = profile_gather;
            tracing::debug!(
                target: "rushdb_vector::search",
                hot_segments = hot.0,
                hot_entries = hot.1,
                hot_gather_us = hot.2.as_micros(),
                frozen_segments = frozen.0,
                frozen_entries = frozen.1,
                frozen_gather_us = frozen.2.as_micros(),
                reranked = candidate_count,
                rerank_us = started.elapsed().as_micros(),
                "segment search"
            );
        }
        // Deterministic order: score descending, then the canonical source
        // key.
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| source_key(a.source).cmp(&source_key(b.source)))
        });
        out.truncate(k);
        Ok((candidate_count, out))
    }
}

impl Default for SegmentSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(vectors: &[Vec<f32>]) -> impl Iterator<Item = Result<&Vec<f32>, VectorError>> {
        vectors.iter().map(Ok)
    }

    impl FrozenSegment {
        fn entries_for_test(&self) -> &[SegmentEntry] {
            &self.entries
        }
    }
    use rushdb_types::ids::OwnerKind;

    /// Two well-separated clusters, matching `ann_recall.rs`'s fixture shape,
    /// so an exact top-k is unambiguous.
    #[expect(
        clippy::cast_precision_loss,
        reason = "test fixture jitter is derived from a small grid index and only needs approximate f32 resolution"
    )]
    fn clustered(n: u64) -> Vec<Vec<f32>> {
        (0..n)
            .map(|i| {
                let jitter = ((i * 37) % 100) as f32 / 100.0 - 0.5;
                if i % 2 == 0 {
                    vec![0.9 + jitter * 0.05, 0.1 + jitter * 0.05]
                } else {
                    vec![0.1 + jitter * 0.05, 0.9 + jitter * 0.05]
                }
            })
            .collect()
    }

    fn source_for(ord: u64) -> SourceRef {
        SourceRef {
            kind: OwnerKind::Record,
            owner_ord: ord,
            pid: 1,
            elem: 0,
        }
    }

    fn value_hash_of(vector: &[f32]) -> u64 {
        let mut hash = blake3::Hasher::new();
        for v in vector {
            hash.update(&v.to_le_bytes());
        }
        let digest = hash.finalize();
        u64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap())
    }

    fn entries_for(
        vectors: &[Vec<f32>],
        slot_offset: u64,
        source_offset: u64,
    ) -> Vec<SegmentEntry> {
        vectors
            .iter()
            .enumerate()
            .map(|(i, v)| SegmentEntry {
                slot: slot_offset + i as u64,
                source: source_for(source_offset + i as u64),
                value_hash: value_hash_of(v),
            })
            .collect()
    }

    /// Canonical-vector lookup by slot, standing in for the store read a
    /// production rerank closure would do (`emb_vector_key` + decode).
    struct CanonicalStore(HashMap<u64, Vec<f32>>);

    impl CanonicalStore {
        fn new() -> Self {
            CanonicalStore(HashMap::new())
        }

        fn insert_all(&mut self, entries: &[SegmentEntry], vectors: &[Vec<f32>]) {
            for (entry, vector) in entries.iter().zip(vectors) {
                self.0.insert(entry.slot, vector.clone());
            }
        }

        #[expect(
            clippy::unnecessary_wraps,
            reason = "matches SegmentSet::search's rerank callback signature, which must return Result for real production rerank closures that read the canonical store"
        )]
        fn rerank(
            &self,
            query: &[f32],
            similarity: Similarity,
            entry: &SegmentEntry,
        ) -> Result<ScoredSource, VectorError> {
            let vector = self
                .0
                .get(&entry.slot)
                .expect("canonical vector for slot (test store is populated for every entry)");
            Ok(ScoredSource {
                source: entry.source,
                score: crate::score(query, vector, similarity),
                value_hash: entry.value_hash,
            })
        }
    }

    fn exact_top_k(vectors: &[Vec<f32>], query: &[f32], k: usize) -> Vec<usize> {
        let mut scored: Vec<(f32, usize)> = vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (crate::score(query, v, Similarity::Cosine), i))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(k).map(|(_, i)| i).collect()
    }

    #[test]
    fn single_frozen_segment_matches_exact_oracle() {
        // The 1-segment case must behave like today's single-generation path
        // (no behavior change) — recall parity against the exact oracle,
        // same discipline `ann_recall.rs` applies to `HnswProvider`.
        let dir = tempfile::tempdir().unwrap();
        let vectors = clustered(64);
        let entries = entries_for(&vectors, 0, 0);
        let mut store = CanonicalStore::new();
        store.insert_all(&entries, &vectors);

        let frozen = FrozenSegment::build(
            entries,
            ok(&vectors),
            2,
            Similarity::Cosine,
            &dir.path().join("seg0"),
        )
        .unwrap();
        let set = SegmentSet::new();
        set.push(Arc::new(Segment::Frozen(frozen)));

        let query = [0.92_f32, 0.08];
        let exact = exact_top_k(&vectors, &query, 8);
        let (_, approx) = set
            .search(&query, 8, 24, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        assert_eq!(approx.len(), 8);
        let overlap = exact
            .iter()
            .filter(|&&i| approx.iter().any(|a| a.source.owner_ord == i as u64))
            .count();
        assert!(overlap >= 7, "recall too low: {overlap}/8");
        assert_eq!(approx[0].source.owner_ord, exact[0] as u64);
    }

    #[test]
    fn concurrent_searches_with_different_ef_do_not_interfere() {
        // C9 (cloud-capacity F11): `ef` used to be applied by mutating the
        // shared index's expansion per query — a race between concurrent
        // queries. A query's result must depend only on its own (query, k,
        // ef), never on what other threads search concurrently: every
        // concurrent result must equal the same search run alone.
        let dir = tempfile::tempdir().unwrap();
        let vectors = clustered(64);
        let entries = entries_for(&vectors, 0, 0);
        let mut store = CanonicalStore::new();
        store.insert_all(&entries, &vectors);
        let frozen = FrozenSegment::build(
            entries,
            ok(&vectors),
            2,
            Similarity::Cosine,
            &dir.path().join("seg0"),
        )
        .unwrap();
        let set = Arc::new(SegmentSet::new());
        set.push(Arc::new(Segment::Frozen(frozen)));
        let store = Arc::new(store);
        let search = |set: &SegmentSet, store: &CanonicalStore, worker: u8, ef: usize| {
            let query = [0.92_f32, 0.08 + f32::from(worker) * 0.01];
            let (_, hits) = set
                .search(&query, 4, ef, Similarity::Cosine, None, |entry| {
                    store.rerank(&query, Similarity::Cosine, entry)
                })
                .unwrap();
            hits.iter()
                .map(|hit| hit.source.owner_ord)
                .collect::<Vec<_>>()
        };
        let expected: Vec<(Vec<u64>, Vec<u64>)> = (0..8u8)
            .map(|worker| {
                (
                    search(&set, &store, worker, 4),
                    search(&set, &store, worker, 64),
                )
            })
            .collect();
        let expected = Arc::new(expected);
        let workers: Vec<_> = (0..8u8)
            .map(|worker| {
                let (set, store, expected) =
                    (Arc::clone(&set), Arc::clone(&store), Arc::clone(&expected));
                std::thread::spawn(move || {
                    for round in 0..50u8 {
                        let (narrow, wide) = &expected[usize::from(worker)];
                        if (worker + round) % 2 == 0 {
                            assert_eq!(&search(&set, &store, worker, 4), narrow);
                        } else {
                            assert_eq!(&search(&set, &store, worker, 64), wide);
                        }
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    }

    #[test]
    fn derived_segment_sets_keep_allocating_fresh_segment_ids() {
        // C9: a set derived from another (every incremental absorb) must
        // continue its id sequence; restarting at 0 made each freeze
        // overwrite the same `seg-0` file.
        let set = SegmentSet::new();
        for _ in 0..3 {
            set.alloc_segment_id();
        }
        let derived = set.fork();
        assert_eq!(derived.alloc_segment_id(), 3);
        assert_eq!(derived.alloc_segment_id(), 4);
        // The fork is independent of the original.
        assert_eq!(set.peek_segment_id(), 3);
    }

    #[test]
    fn in_memory_frozen_segment_matches_exact_oracle_with_no_disk_io() {
        // The in-memory store's shape: no path, no filesystem
        // interaction at all — the provider graph is resident only.
        let vectors = clustered(64);
        let entries = entries_for(&vectors, 0, 0);
        let mut store = CanonicalStore::new();
        store.insert_all(&entries, &vectors);

        let frozen =
            FrozenSegment::build_in_memory(entries, ok(&vectors), 2, Similarity::Cosine).unwrap();
        assert_eq!(frozen.manifest().entries, 64);
        let set = SegmentSet::new();
        set.push(Arc::new(Segment::Frozen(frozen)));

        let query = [0.92_f32, 0.08];
        let exact = exact_top_k(&vectors, &query, 8);
        let (_, approx) = set
            .search(&query, 8, 24, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        let overlap = exact
            .iter()
            .filter(|&&i| approx.iter().any(|a| a.source.owner_ord == i as u64))
            .count();
        assert!(overlap >= 7, "recall too low: {overlap}/8");
    }

    #[test]
    fn hot_plus_frozen_segments_merge_globally_against_full_corpus() {
        // Split one corpus across a frozen and a hot segment; recall must
        // hold against the oracle over the FULL corpus, not per segment —
        // this is exactly the "global merge, not per-segment concat" fix
        // the blueprint requires (rev. 1's rejected `flat_map().collect()`).
        let dir = tempfile::tempdir().unwrap();
        let vectors = clustered(64);
        let (frozen_vectors, hot_vectors) = vectors.split_at(32);
        let frozen_entries = entries_for(frozen_vectors, 0, 0);
        let hot_entries = entries_for(hot_vectors, 32, 32);

        let mut store = CanonicalStore::new();
        store.insert_all(&frozen_entries, frozen_vectors);
        store.insert_all(&hot_entries, hot_vectors);

        let frozen = FrozenSegment::build(
            frozen_entries,
            ok(frozen_vectors),
            2,
            Similarity::Cosine,
            &dir.path().join("seg0"),
        )
        .unwrap();
        let mut hot = HotSegment::new();
        for (entry, vector) in hot_entries.into_iter().zip(hot_vectors) {
            hot.push(entry, vector.clone());
        }

        let set = SegmentSet::new();
        set.push(Arc::new(Segment::Frozen(frozen)));
        set.push(Arc::new(Segment::Hot(hot)));

        let query = [0.92_f32, 0.08];
        let exact = exact_top_k(&vectors, &query, 8);
        let (_, approx) = set
            .search(&query, 8, 24, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        assert_eq!(approx.len(), 8);
        let overlap = exact
            .iter()
            .filter(|&&i| approx.iter().any(|a| a.source.owner_ord == i as u64))
            .count();
        assert!(overlap >= 7, "cross-segment recall too low: {overlap}/8");
    }

    #[test]
    fn rebind_in_newer_segment_supersedes_older_segment() {
        // Same canonical source bound in an older (frozen) segment far from
        // the query, then rebound in a newer (hot) segment close to the
        // query. The newer vector must win — this is the dedup-by-source
        // correctness property the blueprint calls out: a per-segment
        // dense `id` cannot express this, identity must be the canonical
        // `SourceRef`.
        let dir = tempfile::tempdir().unwrap();
        let source = source_for(99);
        let far_vector = vec![-1.0_f32, -1.0];
        let near_vector = vec![1.0_f32, 0.0];

        // Filler so each segment has more than one entry, all far from the
        // query, so `source` is unambiguously the best candidate once (and
        // only once) its near vector is the one considered.
        let filler: Vec<Vec<f32>> = (0..8u8).map(|i| vec![-1.0 - f32::from(i), -1.0]).collect();

        let mut old_vectors = filler.clone();
        old_vectors.push(far_vector.clone());
        let old_entries_filler = entries_for(&filler, 0, 1000);
        let old_entry_source = SegmentEntry {
            slot: 100,
            source,
            value_hash: value_hash_of(&far_vector),
        };
        let mut old_entries = old_entries_filler;
        old_entries.push(old_entry_source);

        let mut new_vectors = filler.clone();
        new_vectors.push(near_vector.clone());
        let new_entries_filler = entries_for(&filler, 2000, 2000);
        let new_entry_source = SegmentEntry {
            slot: 101,
            source,
            value_hash: value_hash_of(&near_vector),
        };
        let mut new_entries = new_entries_filler;
        new_entries.push(new_entry_source);

        let mut store = CanonicalStore::new();
        store.insert_all(&old_entries, &old_vectors);
        store.insert_all(&new_entries, &new_vectors);

        let frozen = FrozenSegment::build(
            old_entries,
            ok(&old_vectors),
            2,
            Similarity::Cosine,
            &dir.path().join("old"),
        )
        .unwrap();
        let mut hot = HotSegment::new();
        for (entry, vector) in new_entries.into_iter().zip(new_vectors) {
            hot.push(entry, vector);
        }

        let set = SegmentSet::new();
        set.push(Arc::new(Segment::Frozen(frozen)));
        set.push(Arc::new(Segment::Hot(hot)));

        let query = [1.0_f32, 0.0];
        let (_, results) = set
            .search(&query, 1, 16, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source.owner_ord, 99);
        assert!(
            results[0].score > 0.9,
            "expected the rebound (near) vector's score, got {}",
            results[0].score
        );
    }

    #[test]
    fn frozen_segment_persists_and_reopens_via_mmap_view() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("seg0");
        let vectors = clustered(32);
        let entries = entries_for(&vectors, 0, 0);

        let built =
            FrozenSegment::build(entries.clone(), ok(&vectors), 2, Similarity::Cosine, &path)
                .unwrap();
        let built_hit = built.gather(&[0.92, 0.08], 4, 16);
        assert!(!built_hit.is_empty());

        let reopened = FrozenSegment::open(&path, 2, Similarity::Cosine)
            .unwrap()
            .expect("a complete segment must reopen via mmap view");
        assert_eq!(
            segment_fingerprint(reopened.entries_for_test()),
            segment_fingerprint(&entries),
            "entries come back from the sidecar"
        );
        let reopened_hit = reopened.gather(&[0.92, 0.08], 4, 16);
        assert!(!reopened_hit.is_empty());
    }

    #[test]
    fn frozen_segment_is_a_cache_miss_not_an_error_on_a_tampered_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("seg0");
        let vectors = clustered(32);
        let entries = entries_for(&vectors, 0, 0);
        FrozenSegment::build(entries.clone(), ok(&vectors), 2, Similarity::Cosine, &path).unwrap();

        // Entries that no longer match the manifest's fingerprint: the
        // segment is reused only when its sidecars agree.
        let sidecar = entries_path_for(&path);
        let mut bytes = std::fs::read(&sidecar).unwrap();
        bytes[12] ^= 0xff;
        std::fs::write(&sidecar, bytes).unwrap();
        assert!(FrozenSegment::open(&path, 2, Similarity::Cosine)
            .unwrap()
            .is_none());
        // Another dimension is a miss too.
        std::fs::write(&sidecar, encode_entries(&entries)).unwrap();
        assert!(FrozenSegment::open(&path, 3, Similarity::Cosine)
            .unwrap()
            .is_none());
        assert!(FrozenSegment::open(&path, 2, Similarity::Cosine)
            .unwrap()
            .is_some());
    }

    #[test]
    fn frozen_segment_open_of_missing_file_is_a_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("never-written");
        let reopened = FrozenSegment::open(&path, 2, Similarity::Cosine).unwrap();
        assert!(reopened.is_none());
    }

    #[test]
    fn freeze_newest_hot_persists_it_and_recall_survives_the_swap() {
        let dir = tempfile::tempdir().unwrap();
        let vectors = clustered(32);
        let entries = entries_for(&vectors, 0, 0);
        let mut store = CanonicalStore::new();
        store.insert_all(&entries, &vectors);

        let set = SegmentSet::new();
        // No hot segment yet: a no-op, not an error.
        assert!(!set
            .freeze_newest_hot(2, Similarity::Cosine, Some(&dir.path().join("unused")))
            .unwrap());

        let mut hot = HotSegment::new();
        for (entry, vector) in entries.into_iter().zip(vectors.clone()) {
            hot.push(entry, vector);
        }
        set.push(Arc::new(Segment::Hot(hot)));
        assert_eq!(set.len(), 1);

        let froze = set
            .freeze_newest_hot(2, Similarity::Cosine, Some(&dir.path().join("seg0")))
            .unwrap();
        assert!(froze);
        // The hot segment is replaced in place by its frozen form; the next
        // absorbed bind starts a new hot segment.
        assert_eq!(set.len(), 1);

        // Freezing a set whose newest segment is frozen is a no-op.
        assert!(!set
            .freeze_newest_hot(2, Similarity::Cosine, Some(&dir.path().join("seg1")))
            .unwrap());
        assert_eq!(set.len(), 1);

        let query = [0.92_f32, 0.08];
        let exact = exact_top_k(&vectors, &query, 8);
        let (_, approx) = set
            .search(&query, 8, 24, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        let overlap = exact
            .iter()
            .filter(|&&i| approx.iter().any(|a| a.source.owner_ord == i as u64))
            .count();
        assert!(overlap >= 7, "recall too low after freeze: {overlap}/8");
    }

    #[test]
    fn frozen_segment_manifest_reflects_its_entries() {
        let dir = tempfile::tempdir().unwrap();
        let vectors = clustered(16);
        let entries = entries_for(&vectors, 0, 0);
        let frozen = FrozenSegment::build(
            entries,
            ok(&vectors),
            2,
            Similarity::Cosine,
            &dir.path().join("seg0"),
        )
        .unwrap();
        assert_eq!(frozen.manifest().entries, 16);
        assert_eq!(frozen.manifest().dims, 2);
        assert_eq!(frozen.manifest().format_version, SEGMENT_FORMAT_VERSION);
    }

    #[test]
    fn push_to_hot_absorbs_incremental_binds_without_a_rebuild() {
        // The incremental write path: push one entry at a time (as a real
        // bind would), never constructing a HotSegment/FrozenSegment
        // directly. Recall must match a from-scratch build over the same
        // corpus.
        let vectors = clustered(32);
        let entries = entries_for(&vectors, 0, 0);
        let mut store = CanonicalStore::new();
        store.insert_all(&entries, &vectors);

        let set = SegmentSet::new();
        assert_eq!(set.newest_hot_len(), 0);
        for (entry, vector) in entries.into_iter().zip(vectors.clone()) {
            set.push_to_hot(entry, vector);
        }
        assert_eq!(set.newest_hot_len(), 32);
        // No frozen segment yet: everything is still in one hot segment.
        assert_eq!(set.len(), 1);

        let query = [0.92_f32, 0.08];
        let exact = exact_top_k(&vectors, &query, 8);
        let (_, approx) = set
            .search(&query, 8, 24, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        let overlap = exact
            .iter()
            .filter(|&&i| approx.iter().any(|a| a.source.owner_ord == i as u64))
            .count();
        assert!(overlap >= 7, "recall too low: {overlap}/8");
    }

    #[test]
    fn push_to_hot_rebind_resolves_to_the_newest_vector() {
        // Two incremental pushes for the SAME source (a rebind, absorbed
        // one bind at a time): the newer vector must win, exactly like
        // `rebind_in_newer_segment_supersedes_older_segment` but within a
        // single growing hot segment instead of across a frozen/hot pair.
        let source = source_for(7);
        let far_vector = vec![-1.0_f32, -1.0];
        let near_vector = vec![1.0_f32, 0.0];
        let filler: Vec<Vec<f32>> = (0..8u8).map(|i| vec![-1.0 - f32::from(i), -1.0]).collect();
        let mut store = CanonicalStore::new();

        let set = SegmentSet::new();
        for (entry, vector) in entries_for(&filler, 0, 100).into_iter().zip(&filler) {
            store.insert_all(std::slice::from_ref(&entry), std::slice::from_ref(vector));
            set.push_to_hot(entry, vector.clone());
        }
        let old_entry = SegmentEntry {
            slot: 900,
            source,
            value_hash: value_hash_of(&far_vector),
        };
        store.insert_all(
            std::slice::from_ref(&old_entry),
            std::slice::from_ref(&far_vector),
        );
        set.push_to_hot(old_entry, far_vector);

        let new_entry = SegmentEntry {
            slot: 901,
            source,
            value_hash: value_hash_of(&near_vector),
        };
        store.insert_all(
            std::slice::from_ref(&new_entry),
            std::slice::from_ref(&near_vector),
        );
        set.push_to_hot(new_entry, near_vector);

        let query = [1.0_f32, 0.0];
        let (_, results) = set
            .search(&query, 1, 16, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source.owner_ord, 7);
        assert!(
            results[0].score > 0.9,
            "expected the rebound (near) vector's score, got {}",
            results[0].score
        );
    }

    #[test]
    fn tombstoned_source_never_surfaces_independent_of_a_rebind() {
        let vectors = clustered(32);
        let entries = entries_for(&vectors, 0, 0);
        let mut store = CanonicalStore::new();
        store.insert_all(&entries, &vectors);
        let removed_source = entries[0].source;

        let frozen =
            FrozenSegment::build_in_memory(entries, ok(&vectors), 2, Similarity::Cosine).unwrap();
        let set = SegmentSet::new();
        set.push(Arc::new(Segment::Frozen(frozen)));

        let query = [0.92_f32, 0.08];
        let (before_count, before) = set
            .search(&query, 32, 32, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        assert!(before
            .iter()
            .any(|r| r.source.owner_ord == removed_source.owner_ord));

        set.tombstone(removed_source);
        let (after_count, after) = set
            .search(&query, 32, 32, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        assert!(
            !after
                .iter()
                .any(|r| r.source.owner_ord == removed_source.owner_ord),
            "tombstoned source must never surface again"
        );
        assert_eq!(
            after_count + 1,
            before_count,
            "the tombstoned source must drop out of the resolved candidate count too"
        );
    }

    #[test]
    fn freeze_newest_hot_without_a_path_builds_in_memory() {
        // A pure in-memory store has nowhere to persist a frozen segment:
        // `path: None` must still freeze (build_in_memory), not error or
        // silently skip.
        let vectors = clustered(32);
        let entries = entries_for(&vectors, 0, 0);
        let mut store = CanonicalStore::new();
        store.insert_all(&entries, &vectors);

        let set = SegmentSet::new();
        for (entry, vector) in entries.into_iter().zip(vectors.clone()) {
            set.push_to_hot(entry, vector);
        }
        let froze = set.freeze_newest_hot(2, Similarity::Cosine, None).unwrap();
        assert!(froze);
        assert_eq!(set.len(), 1);

        let query = [0.92_f32, 0.08];
        let exact = exact_top_k(&vectors, &query, 8);
        let (_, approx) = set
            .search(&query, 8, 24, Similarity::Cosine, None, |entry| {
                store.rerank(&query, Similarity::Cosine, entry)
            })
            .unwrap();
        let overlap = exact
            .iter()
            .filter(|&&i| approx.iter().any(|a| a.source.owner_ord == i as u64))
            .count();
        assert!(
            overlap >= 7,
            "recall too low after in-memory freeze: {overlap}/8"
        );
    }

    #[test]
    fn merge_preserves_newest_wins_and_tombstones() {
        // Property test against an exact oracle: random binds,
        // rebinds and retirements spread over many small frozen segments,
        // then merges; after every merge the set must return exactly the
        // live sources an exhaustive scan of the canonical state returns.
        let mut rng = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        #[expect(clippy::cast_precision_loss, reason = "test vectors")]
        let vector = |next: &mut dyn FnMut() -> u64| -> Vec<f32> {
            (0..4)
                .map(|_| (next() % 1000) as f32 / 500.0 - 1.0)
                .collect()
        };
        let mut store = CanonicalStore::new();
        // Oracle: source ord → (slot, vector) of its live binding.
        let mut oracle: std::collections::BTreeMap<u64, (u64, Vec<f32>)> =
            std::collections::BTreeMap::new();
        let set = SegmentSet::new();
        let mut slot = 0u64;
        for round in 0..12 {
            // One frozen segment per round of 20 writes.
            for _ in 0..20 {
                let ord = next() % 60;
                if !oracle.is_empty() && next() % 7 == 0 {
                    let victim = *oracle
                        .keys()
                        .nth(usize::try_from(next()).unwrap() % oracle.len())
                        .unwrap();
                    oracle.remove(&victim);
                    set.tombstone(source_for(victim));
                    continue;
                }
                let v = vector(&mut next);
                let entry = SegmentEntry {
                    slot,
                    source: source_for(ord),
                    value_hash: value_hash_of(&v),
                };
                store.0.insert(slot, v.clone());
                oracle.insert(ord, (slot, v.clone()));
                set.push_to_hot(entry, v);
                slot += 1;
            }
            set.freeze_newest_hot(4, Similarity::Cosine, None).unwrap();
            if round % 3 == 2 {
                // Merge every frozen segment's live entries into one.
                let ids: Vec<u64> = set.state.load().ids.iter().copied().collect();
                let entries = set.live_entries_in(&ids);
                let vectors: Vec<Vec<f32>> =
                    entries.iter().map(|e| store.0[&e.slot].clone()).collect();
                let merged =
                    FrozenSegment::build_in_memory(entries, ok(&vectors), 4, Similarity::Cosine)
                        .unwrap();
                assert!(set.replace_merged(&ids, Some(Segment::Frozen(merged))));
                assert_eq!(set.len(), 1);
            }
            assert_eq!(set.live_len(), oracle.len(), "round {round}");
            for _ in 0..5 {
                let query = vector(&mut next);
                // ef covers everything, so the ANN gather is exhaustive and
                // any difference is a liveness bug, not recall.
                let (_, got) = set
                    .search(&query, 10, 10_000, Similarity::Cosine, None, |entry| {
                        store.rerank(&query, Similarity::Cosine, entry)
                    })
                    .unwrap();
                let mut want: Vec<(f32, u64)> = oracle
                    .iter()
                    .map(|(ord, (_, v))| (crate::score(&query, v, Similarity::Cosine), *ord))
                    .collect();
                want.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
                want.truncate(10);
                let got: Vec<u64> = got.iter().map(|hit| hit.source.owner_ord).collect();
                let want: Vec<u64> = want.into_iter().map(|(_, ord)| ord).collect();
                assert_eq!(got, want, "round {round}");
            }
        }
    }
}
