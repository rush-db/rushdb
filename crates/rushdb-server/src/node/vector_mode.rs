//! Which path answers a vector search. `auto` (the default) searches
//! exactly while the index is small enough that scoring every vector is
//! cheap, and on the ANN graph above `resources.vector-exact-max-pool`; a
//! caller can force either path. The response reports the path taken.

use std::sync::Arc;

use rushdb_protocol::VectorSearchMode;
use rushdb_types::occurrence::Similarity;
use rushdb_vector::search::{FallbackReason, SearchMode, SearchPath};
use rushdb_vector::{EmbeddingSpace, ScoredSource, SourceRef, VectorEngine};

use super::Node;

/// One vector search's inputs, shared by `query.vector` and the vector leg
/// of `query.hybrid`.
pub(super) struct VectorSearch<'a> {
    pub query: &'a [f32],
    /// `offset + limit`: how many ranked hits the page needs.
    pub k: usize,
    pub mode: Option<VectorSearchMode>,
    pub ef: Option<usize>,
    pub min_score: Option<f32>,
}

impl Node {
    /// Run one vector search over `space` and report the path taken. `auto`
    /// answers exactly up to `vector-exact-max-pool` vectors; above it, on
    /// the ANN graph once resident, and exactly while a background build
    /// makes it resident (a query never waits for a build).
    ///
    /// # Errors
    ///
    /// A forced `ann` on an index whose metric has no ANN graph, or a
    /// failed search.
    pub(super) fn run_vector_search(
        &self,
        vectors: &Arc<VectorEngine>,
        snap: &rushdb_storage::ReadView,
        space: &EmbeddingSpace,
        search: &VectorSearch<'_>,
        candidate: Option<&dyn Fn(SourceRef) -> bool>,
    ) -> Result<(Vec<ScoredSource>, serde_json::Value), String> {
        let pool = vectors
            .ready_vectors_in_view(snap, space)
            .map_err(|e| e.to_string())?;
        let (mut mode, auto) = self.vector_search_mode(search, space.similarity, pool)?;
        let mut warming = false;
        if auto
            && matches!(mode, SearchMode::Ann { .. })
            && !vectors
                .ann_resident_in_view(snap, space)
                .map_err(|e| e.to_string())?
        {
            vectors.warm_ann_in_background(snap, space);
            mode = SearchMode::Exact;
            warming = true;
        }
        let outcome = vectors
            .search_in_view(
                snap,
                space,
                search.query,
                search.k,
                mode,
                candidate,
                search.min_score,
            )
            .map_err(|e| e.to_string())?;
        let mut report = vector_search_report(mode, outcome.path, pool);
        if warming {
            report["fallback"] = "annWarming".into();
        }
        // VEC-3: an exact answer to an ANN-sized pool used to be silent.
        self.metrics.observe_vector_search(
            report["mode"].as_str().unwrap_or("exact"),
            report["fallback"].as_str().unwrap_or("none"),
        );
        Ok((outcome.hits, report))
    }

    /// The path a search takes before any fallback, and whether `auto`
    /// chose it.
    fn vector_search_mode(
        &self,
        search: &VectorSearch<'_>,
        similarity: Similarity,
        pool: u64,
    ) -> Result<(SearchMode, bool), String> {
        let ef = search.ef.filter(|ef| *ef > 0);
        let ann = SearchMode::Ann {
            ef: ef.unwrap_or(self.vector_default_ef).max(search.k),
        };
        let cosine = similarity == Similarity::Cosine;
        let requested = search.mode.unwrap_or(if ef.is_some() {
            VectorSearchMode::Ann
        } else {
            VectorSearchMode::Auto
        });
        match requested {
            VectorSearchMode::Exact => Ok((SearchMode::Exact, false)),
            VectorSearchMode::Ann if !cosine => Err(format!(
                "vector search mode `ann` requires a cosine index; this index uses `{}` \
                 (use `exact` or `auto`)",
                similarity.as_str()
            )),
            VectorSearchMode::Ann => Ok((ann, false)),
            VectorSearchMode::Auto if !cosine || pool <= self.vector_exact_max_pool => {
                Ok((SearchMode::Exact, true))
            }
            VectorSearchMode::Auto => Ok((ann, true)),
        }
    }
}

/// The `vectorSearch` report returned next to a search's rows.
pub(super) fn vector_search_report(
    mode: SearchMode,
    path: SearchPath,
    pool: u64,
) -> serde_json::Value {
    let (taken, ef, fallback) = match (path, mode) {
        (SearchPath::Ann, SearchMode::Ann { ef }) => ("ann", Some(ef), None),
        (SearchPath::ExactFallback(reason), _) => (
            "exact",
            None,
            Some(match reason {
                FallbackReason::Metric => "metric",
                FallbackReason::Budget => "budget",
                FallbackReason::LowConfidence => "lowConfidence",
            }),
        ),
        _ => ("exact", None, None),
    };
    let mut report = serde_json::json!({ "mode": taken, "pool": pool });
    if let Some(ef) = ef {
        report["ef"] = ef.into();
    }
    if let Some(fallback) = fallback {
        report["fallback"] = fallback.into();
    }
    report
}
