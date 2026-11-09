//! Searches and builds on the shared thread budget
//! (`rushdb_storage::parallel`): an exact search split into parts returns
//! exactly what one thread returns — the same hits, scores and order, ties
//! included — and an ANN graph decoded by several threads is the graph one
//! thread builds.
//! Every test sets the budget itself; results must not depend on it.

use std::sync::Arc;

use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::occurrence::Similarity;
use rushdb_vector::search::{SearchMode, SearchPath};
use rushdb_vector::{Candidates, SourceRef, VectorEngine};

fn source(owner_ord: u64) -> SourceRef {
    SourceRef {
        kind: OwnerKind::Record,
        owner_ord,
        pid: 1,
        elem: 0,
    }
}

/// `n` vectors of `dims` drawn from `distinct` patterns, so most scores are
/// shared by many sources and ties cross every part boundary.
#[expect(clippy::cast_precision_loss, reason = "a test distribution")]
fn vectors(n: u64, dims: usize, distinct: u64, seed: u64) -> Vec<Vec<f32>> {
    let mut state = seed;
    let patterns: Vec<Vec<f32>> = (0..distinct)
        .map(|_| {
            (0..dims)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
                })
                .collect()
        })
        .collect();
    (0..n)
        .map(|i| patterns[usize::try_from(i % distinct).unwrap()].clone())
        .collect()
}

fn engine(alias: &str, similarity: Similarity, vectors: &[Vec<f32>]) -> VectorEngine {
    let engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::new(Store::new_in_memory()),
        TenantId::from_ordinal(64),
    )));
    let dims = u32::try_from(vectors[0].len()).unwrap();
    engine
        .ensure_space_similarity(alias, dims, similarity)
        .unwrap();
    for (start, chunk) in (0u64..).step_by(10_000).zip(vectors.chunks(10_000)) {
        let items = chunk
            .iter()
            .zip(start..)
            .map(|(vector, ord)| (source(ord), vector.clone()))
            .collect();
        engine.bind_external_many(alias, items).unwrap();
    }
    engine
}

/// Hits as comparable values: owner, exact score bits, value hash.
type Hits = Vec<(u64, u32, u64)>;

fn hits(found: &[rushdb_vector::ScoredSource]) -> Hits {
    found
        .iter()
        .map(|hit| (hit.source.owner_ord, hit.score.to_bits(), hit.value_hash))
        .collect()
}

/// Brute force: every admitted vector scored, ranked by score descending,
/// then by owner (the canonical source order here).
fn oracle(
    vectors: &[Vec<f32>],
    query: &[f32],
    similarity: Similarity,
    k: usize,
    admit: impl Fn(u64) -> bool,
    min_score: Option<f32>,
) -> Vec<(u64, u32)> {
    let mut scored: Vec<(f32, u64)> = (0u64..)
        .zip(vectors)
        .filter(|(ord, _)| admit(*ord))
        .map(|(ord, v)| (rushdb_vector::score(query, v, similarity), ord))
        .filter(|(score, _)| min_score.is_none_or(|min| *score >= min))
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(k)
        .map(|(score, ord)| (ord, score.to_bits()))
        .collect()
}

#[test]
fn a_parallel_exact_search_returns_exactly_the_single_thread_result() {
    // Past one scan chunk (65,536 ready bindings), so a full scan scores
    // several chunks of several parts each.
    const N: u64 = 70_000;
    let vectors = vectors(N, 8, 97, 0x5eed);
    let query = self::vectors(1, 8, 1, 77).remove(0);
    let few: roaring::RoaringTreemap = (0..N).step_by(233).collect();
    // More owners than ready vectors: answered by the full scan.
    let many: roaring::RoaringTreemap = (0..2 * N).filter(|ord| ord % 3 != 0).collect();
    for similarity in [Similarity::Cosine, Similarity::Euclidean] {
        let engine = engine("par", similarity, &vectors);
        let snap = engine.snapshot().unwrap();
        let space = engine.lookup_space_in_view(&snap, "par").unwrap().unwrap();
        let odd = |source: SourceRef| source.owner_ord % 2 == 1;
        let run = |k: usize, candidates: Candidates<'_>, min_score: Option<f32>| {
            let outcome = engine
                .search_in_view(
                    &snap,
                    &space,
                    &query,
                    k,
                    SearchMode::Exact,
                    candidates,
                    min_score,
                )
                .unwrap();
            assert_eq!(outcome.path, SearchPath::Exact);
            hits(&outcome.hits)
        };
        let threshold = rushdb_vector::score(&query, &vectors[5], similarity);
        let all = |threads: usize| -> Vec<Hits> {
            rushdb_storage::parallel::set_threads(threads);
            vec![
                run(10, Candidates::All, None),
                run(3_000, Candidates::All, None),
                run(25, Candidates::Check(&odd), None),
                run(
                    40,
                    Candidates::Owners {
                        kind: OwnerKind::Record,
                        ords: &few,
                    },
                    None,
                ),
                run(
                    40,
                    Candidates::Owners {
                        kind: OwnerKind::Record,
                        ords: &many,
                    },
                    None,
                ),
                run(
                    usize::try_from(N).unwrap(),
                    Candidates::All,
                    Some(threshold),
                ),
            ]
        };
        let single = all(1);
        for threads in [2, 8] {
            assert_eq!(
                all(threads),
                single,
                "{similarity:?} with {threads} threads"
            );
        }
        rushdb_storage::parallel::set_threads(1);
        // And the single-thread result is the brute-force ranking.
        let expected = [
            oracle(&vectors, &query, similarity, 10, |_| true, None),
            oracle(&vectors, &query, similarity, 3_000, |_| true, None),
            oracle(&vectors, &query, similarity, 25, |ord| ord % 2 == 1, None),
            oracle(
                &vectors,
                &query,
                similarity,
                40,
                |ord| few.contains(ord),
                None,
            ),
            oracle(
                &vectors,
                &query,
                similarity,
                40,
                |ord| many.contains(ord),
                None,
            ),
            oracle(
                &vectors,
                &query,
                similarity,
                usize::try_from(N).unwrap(),
                |_| true,
                Some(threshold),
            ),
        ];
        for (got, want) in single.iter().zip(expected) {
            let got: Vec<(u64, u32)> = got.iter().map(|(ord, bits, _)| (*ord, *bits)).collect();
            assert_eq!(got, want, "{similarity:?}");
        }
    }
}

#[test]
fn an_ann_graph_is_the_same_whatever_the_threads_decoding_it() {
    // Below the inline build limit, so the first ANN search builds it; one
    // segment of several decode parts and chunks.
    let vectors = vectors(10_000, 24, 10_000, 0xab5);
    let queries = self::vectors(40, 24, 40, 0x51);
    let answers = |threads: usize| -> Vec<Hits> {
        rushdb_storage::parallel::set_threads(threads);
        let engine = engine(&format!("ann{threads}"), Similarity::Cosine, &vectors);
        let snap = engine.snapshot().unwrap();
        let space = engine
            .lookup_space_in_view(&snap, &format!("ann{threads}"))
            .unwrap()
            .unwrap();
        queries
            .iter()
            .map(|query| {
                let ann = engine
                    .search_in_view(
                        &snap,
                        &space,
                        query,
                        10,
                        SearchMode::Ann { ef: 128 },
                        Candidates::All,
                        None,
                    )
                    .unwrap();
                assert_eq!(ann.path, SearchPath::Ann);
                hits(&ann.hits)
            })
            .collect()
    };
    let single = answers(1);
    assert_eq!(answers(8), single, "the graph does not depend on threads");
    // And the default breadth finds nearly all of the exact top 10.
    let found: usize = queries
        .iter()
        .zip(&single)
        .map(|(query, got)| {
            oracle(&vectors, query, Similarity::Cosine, 10, |_| true, None)
                .iter()
                .filter(|(ord, _)| got.iter().any(|(owner, _, _)| owner == ord))
                .count()
        })
        .sum();
    assert!(
        found * 100 >= 95 * queries.len() * 10,
        "recall@10 {found}/{}",
        queries.len() * 10
    );
    rushdb_storage::parallel::set_threads(1);
}
