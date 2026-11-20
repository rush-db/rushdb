//! Which path answers a vector search. The decision is made on the vectors
//! the search could score: the candidate owners of a filtered search (at
//! most the index's ready vectors), or the whole index otherwise. `auto`
//! (the default) searches exactly while that pool is small enough
//! (`resources.vector-exact-max-pool`: by cost, [`exact_costs_less`], or a
//! fixed pool), and on the ANN graph above it; a caller can force either
//! path, and a forced `ann` over a filtered pool that small is still
//! answered exactly (`smallCandidateSet`). The response reports the path
//! taken, and `explain` reports it before searching.

use std::sync::Arc;

use rushdb_protocol::VectorSearchMode;

use crate::resource_config::ExactPool;
use rushdb_vector::search::{FallbackReason, SearchMode, SearchPath};
use rushdb_vector::{Candidates, EmbeddingSpace, ScoredSource, VectorEngine};

use super::{Node, RequestScope};

/// One vector search's inputs, shared by `query.vector` and the vector leg
/// of `query.hybrid`.
pub(super) struct VectorSearch<'a> {
    pub query: &'a [f32],
    /// `offset + limit`: how many ranked hits the page needs.
    pub k: usize,
    pub mode: Option<VectorSearchMode>,
    pub ef: Option<usize>,
    pub min_score: Option<f32>,
    /// The sources the search may return. An owner set (a predicate's
    /// matches) is what routing counts.
    pub candidates: Candidates<'a>,
}

/// The path a search takes before the vector engine runs it.
pub(super) struct VectorRoute {
    pub mode: SearchMode,
    /// The vectors the search could score: the candidate owners of a
    /// filtered search, at most the index's ready vectors; the ready
    /// vectors otherwise.
    pub pool: u64,
    /// Why a search that would use the ANN graph is answered exactly before
    /// it starts: `smallCandidateSet` (a forced `ann` over a filtered pool
    /// within `vector-exact-max-pool`) or `annWarming` (`auto` above it while
    /// the graph is not resident).
    pub fallback: Option<&'static str>,
}

impl Node {
    /// Run one vector search over `space` and report the path taken.
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
    ) -> Result<(Vec<ScoredSource>, serde_json::Value), String> {
        let route = self.route_vector_search(vectors, snap, space, search)?;
        // Exact search and ANN rerank score resident rows once the space's
        // arena is made (off the request, paid by this search's reads).
        vectors.rent_arena(snap, space);
        if route.fallback == Some("annWarming") {
            // A query never waits for a build: this one answers exactly and
            // later ones by ANN once the graph is resident.
            vectors.warm_ann_in_background(snap, space);
        }
        let outcome = vectors
            .search_in_view(
                snap,
                space,
                search.query,
                search.k,
                route.mode,
                search.candidates,
                search.min_score,
            )
            .map_err(|e| e.to_string())?;
        if outcome.path == SearchPath::ExactFallback(FallbackReason::Warming) {
            // The bindings no persisted segment covers are indexed off the
            // request; later searches answer by ANN once it is resident.
            vectors.warm_ann_in_background(snap, space);
        }
        let report = vector_search_report(&route, outcome.path);
        // VEC-3: an exact answer to an ANN-sized pool used to be silent.
        self.metrics.observe_vector_search(
            report["mode"].as_str().unwrap_or("exact"),
            report["fallback"].as_str().unwrap_or("none"),
        );
        Ok((outcome.hits, report))
    }

    /// The path `search` takes over `space` before any fallback the vector
    /// engine decides (`budget`, `lowConfidence`, `metric`). The one routing
    /// rule shared by the search and `explain`.
    ///
    /// # Errors
    ///
    /// A forced `ann` on an index whose metric has no ANN graph, or a failed
    /// read of the index's counts.
    pub(super) fn route_vector_search(
        &self,
        vectors: &Arc<VectorEngine>,
        snap: &rushdb_storage::ReadView,
        space: &EmbeddingSpace,
        search: &VectorSearch<'_>,
    ) -> Result<VectorRoute, String> {
        let ready = vectors
            .ready_vectors_in_view(snap, space)
            .map_err(|e| e.to_string())?;
        let candidates = search.candidates.owner_count();
        let pool = candidates.map_or(ready, |owners| owners.min(ready));
        let ef = search.ef.filter(|ef| *ef > 0);
        let walk_ef = ef.unwrap_or(self.vector_default_ef).max(search.k);
        let ann = SearchMode::Ann { ef: walk_ef };
        let small = match self.vector_exact_max_pool {
            ExactPool::Fixed(max) => pool <= max,
            ExactPool::Auto => exact_costs_less(
                pool,
                ready,
                u64::from(space.dims),
                walk_ef,
                candidates.is_some(),
            ),
        };
        let cosine = space.similarity == rushdb_types::occurrence::Similarity::Cosine;
        let requested = search.mode.unwrap_or(if ef.is_some() {
            VectorSearchMode::Ann
        } else {
            VectorSearchMode::Auto
        });
        let route = |mode, fallback| VectorRoute {
            mode,
            pool,
            fallback,
        };
        Ok(match requested {
            VectorSearchMode::Exact => route(SearchMode::Exact, None),
            VectorSearchMode::Ann if !cosine => {
                return Err(format!(
                    "vector search mode `ann` requires a cosine index; this index uses `{}` \
                     (use `exact` or `auto`)",
                    space.similarity.as_str()
                ))
            }
            // Scoring a small candidate set exactly is cheaper than the walk
            // and never less accurate.
            VectorSearchMode::Ann if candidates.is_some() && small => {
                route(SearchMode::Exact, Some("smallCandidateSet"))
            }
            VectorSearchMode::Auto if !cosine || small => route(SearchMode::Exact, None),
            VectorSearchMode::Auto
                if !vectors
                    .ann_resident_in_view(snap, space)
                    .map_err(|e| e.to_string())? =>
            {
                route(SearchMode::Exact, Some("annWarming"))
            }
            VectorSearchMode::Ann | VectorSearchMode::Auto => route(ann, None),
        })
    }
}

/// Work units an exact search over a whole index may take per unit of
/// `ef` and still cost no more than the walk: exact scores every ready
/// vector from the space's resident rows (`ready × dims`), the walk costs
/// about `ef`. Measured on 100,000 × 1,536-d cosine vectors (exact 11.7 ms,
/// walk at `ef` 128 1.4 ms).
const UNFILTERED_WORK_PER_EF: u128 = 144_000;

/// The same for a filtered search, in `pool² × dims / ready` units per
/// `ef`: exact reads and scores each candidate owner's vectors (`pool ×
/// dims`), a filtered walk admitting `pool / ready` of the graph costs
/// about `ef × ready / pool`. Calibrated on cosine vectors, top 10, where
/// exact and the walk cost the same: 100,000 × 1,536-d at pools of about
/// 13,000 (`ef` 128) and 27,000 (`ef` 512), 1,000,000 × 768-d at about
/// 75,000 and 150,000. This constant puts the bound within 13% of each.
const FILTERED_WORK_PER_EF: u128 = 26_000;

/// Whether scoring `pool` vectors of `dims` exactly is expected to cost less
/// than an ANN walk of breadth `ef` over an index of `ready` vectors that
/// admits only those `pool` (`filtered`) or all of them. Exact is never
/// less accurate, so a tie goes to exact.
fn exact_costs_less(pool: u64, ready: u64, dims: u64, ef: usize, filtered: bool) -> bool {
    let (pool, ready, dims) = (u128::from(pool), u128::from(ready), u128::from(dims));
    let ef = u128::try_from(ef).unwrap_or(u128::MAX);
    if filtered {
        pool.saturating_mul(pool).saturating_mul(dims)
            <= FILTERED_WORK_PER_EF
                .saturating_mul(ef)
                .saturating_mul(ready)
    } else {
        ready.saturating_mul(dims) <= UNFILTERED_WORK_PER_EF.saturating_mul(ef)
    }
}

impl RequestScope<'_> {
    /// Explain a `query.vector` at a caller-owned pinned view: its index,
    /// candidate owners and the path the search would take, decided by
    /// [`Self::route_vector_search`] like the search itself.
    ///
    /// # Errors
    ///
    /// As the search: a denied label, a missing index, a predicate that
    /// does not parse, or a forced `ann` on a non-cosine index.
    pub(crate) fn explain_vector_in_view(
        &self,
        query: &rushdb_protocol::VectorQuery,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<rushdb_protocol::ExplainVectorPlan, String> {
        let target = self.resolve_vector_target(query, snap)?;
        let vectors = self.vector_engine()?;
        let space = vectors
            .lookup_space_in_view(snap, &target.alias)
            .map_err(|e| e.to_string())?
            .ok_or("index space missing")?;
        let candidates = match &target.owners {
            Some(ords) => Candidates::Owners {
                kind: target.owner_kind,
                ords,
            },
            None => Candidates::All,
        };
        let k = usize::try_from(query.offset)
            .unwrap_or(usize::MAX)
            .saturating_add(usize::try_from(query.limit).unwrap_or(usize::MAX));
        let route = self.route_vector_search(
            &vectors,
            snap,
            &space,
            &VectorSearch {
                query: &query.vector,
                k,
                mode: query.mode,
                ef: query.ef,
                min_score: query.score_threshold,
                candidates,
            },
        )?;
        let planned = match route.mode {
            SearchMode::Exact => SearchPath::Exact,
            SearchMode::Ann { .. } => SearchPath::Ann,
        };
        let search = serde_json::from_value(vector_search_report(&route, planned))
            .map_err(|error| error.to_string())?;
        let candidate_count = match &target.owners {
            Some(ords) => ords.len(),
            None => vectors
                .ready_vectors_in_view(snap, &space)
                .map_err(|e| e.to_string())?,
        };
        Ok(rushdb_protocol::ExplainVectorPlan {
            access_path: "vector_index".to_string(),
            alias: target.alias,
            label: target.label,
            candidate_count,
            k,
            predicate: query.predicate.clone(),
            search,
        })
    }
}

/// The `vectorSearch` report returned next to a search's rows (and by
/// `explain`, for the route alone).
pub(super) fn vector_search_report(route: &VectorRoute, path: SearchPath) -> serde_json::Value {
    let (taken, ef, fallback) = match (path, route.mode) {
        (SearchPath::Ann, SearchMode::Ann { ef }) => ("ann", Some(ef), None),
        (SearchPath::ExactFallback(reason), _) => (
            "exact",
            None,
            Some(match reason {
                FallbackReason::Metric => "metric",
                FallbackReason::Budget => "budget",
                FallbackReason::LowConfidence => "lowConfidence",
                FallbackReason::Warming => "annWarming",
            }),
        ),
        _ => ("exact", None, route.fallback),
    };
    let mut report = serde_json::json!({ "mode": taken, "pool": route.pool });
    if let Some(ef) = ef {
        report["ef"] = ef.into();
    }
    if let Some(fallback) = fallback {
        report["fallback"] = fallback.into();
    }
    report
}

#[cfg(test)]
mod tests {
    use super::{exact_costs_less, FILTERED_WORK_PER_EF, UNFILTERED_WORK_PER_EF};

    /// The largest pool a filtered search over `ready` vectors of `dims`
    /// answers exactly at `ef`.
    fn filtered_bound(ready: u64, dims: u64, ef: usize) -> u64 {
        let mut pool = 0;
        while pool < ready && exact_costs_less(pool + 1, ready, dims, ef, true) {
            pool += 1;
        }
        pool
    }

    #[test]
    fn a_filtered_pool_is_exact_up_to_its_cost_bound() {
        let bound = filtered_bound(100_000, 1_536, 128);
        // pool² × dims ≤ K × ef × ready, and one more is past it.
        let work = |pool: u64| u128::from(pool).pow(2) * 1_536;
        assert!(work(bound) <= FILTERED_WORK_PER_EF * 128 * 100_000);
        assert!(work(bound + 1) > FILTERED_WORK_PER_EF * 128 * 100_000);
        assert!(exact_costs_less(bound, 100_000, 1_536, 128, true));
        assert!(!exact_costs_less(bound + 1, 100_000, 1_536, 128, true));
        // Wider walks, larger indexes and fewer dimensions move the bound up.
        assert!(filtered_bound(100_000, 1_536, 512) > bound);
        assert!(filtered_bound(400_000, 1_536, 128) > bound);
        assert!(filtered_bound(100_000, 768, 128) > bound);
        // A small index answers every filter exactly.
        assert_eq!(filtered_bound(1_000, 1_536, 128), 1_000);
    }

    #[test]
    fn an_unfiltered_index_is_exact_up_to_its_cost_bound() {
        let bound = UNFILTERED_WORK_PER_EF * 128 / 1_536;
        let bound = u64::try_from(bound).unwrap();
        assert!(exact_costs_less(bound, bound, 1_536, 128, false));
        assert!(!exact_costs_less(bound + 1, bound + 1, 1_536, 128, false));
        // Half the dimensions, twice the vectors.
        assert!(exact_costs_less(2 * bound, 2 * bound, 768, 128, false));
        assert!(!exact_costs_less(
            2 * bound + 2,
            2 * bound + 2,
            768,
            128,
            false
        ));
    }

    #[test]
    fn huge_inputs_saturate_instead_of_overflowing() {
        assert!(!exact_costs_less(u64::MAX, u64::MAX, u64::MAX, 128, true));
        assert!(!exact_costs_less(u64::MAX, u64::MAX, u64::MAX, 128, false));
        assert!(exact_costs_less(0, u64::MAX, 1_536, usize::MAX, true));
    }
}
