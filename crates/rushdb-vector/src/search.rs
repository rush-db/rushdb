//! One vector search: exact or approximate (ANN), an optional minimum
//! score, and the path that actually answered.

use std::sync::Arc;

use rushdb_storage::{ReadView, StorageError};

use crate::ann::space_version;
use crate::{EmbeddingSpace, ScoredSource, SourceRef, VectorEngine, VectorError};

/// A space's background ANN build.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Warmup {
    /// A build is running.
    Building,
    /// The graph of the space at `version` did not fit the vector cache
    /// budget; no build is started again until the space changes.
    NotAdmitted { version: u64 },
}

/// How a search ranks its candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Score every candidate.
    Exact,
    /// Gather candidates from the ANN graph and rerank them exactly.
    Ann {
        /// Gather breadth; at least `k` is gathered.
        ef: usize,
    },
}

/// Why an ANN search was answered by exact search instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    /// The space's metric has no ANN graph (only cosine does).
    Metric,
    /// The ANN graph does not fit the vector cache budget.
    Budget,
    /// The gather found too few candidates passing the filter.
    LowConfidence,
}

/// The path that answered a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchPath {
    /// Exact search, as requested.
    Exact,
    /// ANN gather with exact rerank.
    Ann,
    /// Exact search after an ANN request could not be answered by ANN.
    ExactFallback(FallbackReason),
}

/// Ranked hits, best first, and the path that produced them.
#[derive(Debug, Clone)]
pub struct SearchOutcome {
    /// Ranked hits, best first.
    pub hits: Vec<ScoredSource>,
    /// The path that produced them.
    pub path: SearchPath,
}

impl VectorEngine {
    /// Top-`k` search within a caller-owned view in `mode`, keeping only
    /// hits scoring at least `min_score` (scores are "higher is more
    /// similar" for every metric).
    ///
    /// # Errors
    ///
    /// As [`Self::search_exact_in_view`] and [`Self::search_ann_in_view`].
    #[allow(clippy::too_many_arguments)]
    pub fn search_in_view(
        &self,
        snap: &ReadView,
        space: &EmbeddingSpace,
        query: &[f32],
        k: usize,
        mode: SearchMode,
        candidate: Option<&dyn Fn(SourceRef) -> bool>,
        min_score: Option<f32>,
    ) -> Result<SearchOutcome, VectorError> {
        let (hits, path) = match mode {
            SearchMode::Exact => (
                self.exact_in_view(snap, space, query, k, candidate, min_score)?,
                SearchPath::Exact,
            ),
            SearchMode::Ann { ef } => {
                self.ann_in_view(snap, space, query, k, ef, candidate, min_score)?
            }
        };
        Ok(SearchOutcome { hits, path })
    }

    /// The number of `space`'s vectors a search ranks: its bindings in
    /// `Ready` status, as maintained by every commit that writes a binding.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] when the read fails or the count row
    /// is corrupt.
    pub fn ready_vectors_in_view(
        &self,
        snap: &ReadView,
        space: &EmbeddingSpace,
    ) -> Result<u64, VectorError> {
        match snap.get(&rushdb_types::keys::emb_ready_count_key(
            self.tenant(),
            space.space_id,
        ))? {
            None => Ok(0),
            Some(raw) => Ok(u64::from_be_bytes(
                raw.as_slice()
                    .try_into()
                    .map_err(|_| StorageError::CorruptLog)?,
            )),
        }
    }

    /// Whether `space`'s ANN graph is resident and current at `snap`, so an
    /// ANN search answers without building it.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Storage`] when the version row cannot be read
    /// and [`VectorError::Engine`] when the cache mutex is poisoned.
    pub fn ann_resident_in_view(
        &self,
        snap: &ReadView,
        space: &EmbeddingSpace,
    ) -> Result<bool, VectorError> {
        let (_, version) = space_version(snap, self.tenant(), space.space_id)?;
        Ok(self
            .ann_cache
            .lock()
            .map_err(|_| VectorError::Engine("ANN cache mutex poisoned".into()))?
            .get(&space.space_id)
            .is_some_and(|cached| cached.space_version == version))
    }

    /// Build `space`'s ANN graph off the request path, so later searches
    /// find it resident. At most one build runs per space; a graph the
    /// budget cannot admit is not built again until the space changes. The
    /// graph is a rebuildable cache, so a failed build only logs.
    pub fn warm_ann_in_background(self: &Arc<Self>, snap: &ReadView, space: &EmbeddingSpace) {
        let Ok((_, version)) = space_version(snap, self.tenant(), space.space_id) else {
            return;
        };
        {
            let Ok(mut warmups) = self.ann_warmups.lock() else {
                return;
            };
            match warmups.get(&space.space_id) {
                Some(Warmup::Building) => return,
                Some(Warmup::NotAdmitted { version: refused }) if *refused == version => return,
                _ => {}
            }
            warmups.insert(space.space_id, Warmup::Building);
        }
        let space_id = space.space_id;
        let engine = Arc::clone(self);
        let space = space.clone();
        let spawned = std::thread::Builder::new()
            .name("ann-warmup".into())
            .spawn(move || {
                let built = engine
                    .snapshot()
                    .map_err(VectorError::from)
                    .and_then(|snap| engine.current_ann(&snap, &space).map(|_| ()));
                let Ok(mut warmups) = engine.ann_warmups.lock() else {
                    return;
                };
                match built {
                    Err(VectorError::AnnBudget(_)) => {
                        warmups.insert(space.space_id, Warmup::NotAdmitted { version });
                    }
                    Err(error) => {
                        tracing::warn!(space = space.space_id, %error, "background ANN build failed");
                        warmups.remove(&space.space_id);
                    }
                    Ok(()) => {
                        warmups.remove(&space.space_id);
                    }
                }
            });
        if spawned.is_err() {
            if let Ok(mut warmups) = self.ann_warmups.lock() {
                warmups.remove(&space_id);
            }
        }
    }
}
