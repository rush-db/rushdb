//! No-drift quality tests for the vector similarity math and its calibration.
//!
//! The engine does not embed, so a score difference between two deployments can
//! only come from the model/provider — *provided* the engine math is stable.
//! These tests pin that:
//!
//! - the scoring functions match an independently written `f64` reference across
//!   dimensions and magnitudes (the formula cannot silently change),
//! - cosine/dot/euclidean invariants and scale invariance hold,
//! - the exact-search path returns exactly the pairwise score for every hit (no
//!   hidden normalization or ingest-time transform),
//! - the two search entry points agree bit-for-bit, and
//! - scores are bit-identical across in-memory and durable backends and across a
//!   reopen (no `f32` serialization drift).

use std::collections::BTreeMap;
use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{Lmpg, RecordRef};
use rushdb_storage::Store;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{key, P_FAMILY_SIG};
use rushdb_types::occurrence::Similarity;
use rushdb_vector::{score, ScoredSource, SourceRef, VectorEngine};

const TENANT: TenantId = TenantId::from_ordinal(91);

fn metric_name(metric: Similarity) -> &'static str {
    match metric {
        Similarity::Cosine => "cosine",
        Similarity::Dot => "dot",
        Similarity::Euclidean => "euclidean",
    }
}

/// xorshift64* mapped to `[-1, 1)`, times `scale`.
#[expect(
    clippy::cast_precision_loss,
    reason = "fixture PRNG: the low 24 bits are used as a fraction, which fits f32's significand exactly"
)]
fn next_unit(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    let unit = ((*state >> 40) as f32) / ((1u64 << 24) as f32);
    unit * 2.0 - 1.0
}

fn random_vector(state: &mut u64, dims: usize, scale: f32) -> Vec<f32> {
    (0..dims).map(|_| next_unit(state) * scale).collect()
}

/// Independent reference implementation of the three metrics, in `f64`.
fn reference_score(query: &[f32], vec: &[f32], metric: Similarity) -> f64 {
    match metric {
        Similarity::Cosine => {
            let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
            for (a, b) in query.iter().zip(vec) {
                dot += f64::from(*a) * f64::from(*b);
                na += f64::from(*a).powi(2);
                nb += f64::from(*b).powi(2);
            }
            if na == 0.0 || nb == 0.0 {
                0.0
            } else {
                dot / (na.sqrt() * nb.sqrt())
            }
        }
        Similarity::Dot => query
            .iter()
            .zip(vec)
            .map(|(a, b)| f64::from(*a) * f64::from(*b))
            .sum(),
        Similarity::Euclidean => {
            let mut dist = 0.0f64;
            for (a, b) in query.iter().zip(vec) {
                let d = f64::from(*a) - f64::from(*b);
                dist += d * d;
            }
            1.0 / (1.0 + dist.sqrt())
        }
    }
}

fn assert_close(got: f32, want: f64, ctx: &str) {
    let got = f64::from(got);
    let tol = 1e-5 * want.abs().max(1.0);
    assert!(
        (got - want).abs() <= tol,
        "{ctx}: engine {got} vs reference {want} (tol {tol})"
    );
}

fn bind_fixture(
    store: Arc<Store>,
    alias: &str,
    dims: u32,
    metric: Similarity,
    vectors: &[Vec<f32>],
) -> (VectorEngine, BTreeMap<u64, Vec<f32>>) {
    let lmpg = Lmpg::new(store.clone(), TENANT);
    let vecs = VectorEngine::new(Arc::new(Lmpg::new(store, TENANT)));
    vecs.ensure_space_similarity(alias, dims, metric).unwrap();
    let pref = key(
        TENANT,
        &[
            P_FAMILY_SIG,
            b"default",
            &[OwnerKind::Record.as_byte()],
            b"v",
        ],
    );
    let mut by_ord = BTreeMap::new();
    for v in vectors {
        let r: RecordRef = lmpg
            .create_record(
                "Doc",
                None,
                vec![("v".into(), FieldChange::Set(serde_json::json!("v")))],
            )
            .unwrap();
        let raw = lmpg.store.read_view().unwrap().get(&pref).unwrap().unwrap();
        let pid = u64::from_be_bytes(raw.as_slice().try_into().unwrap());
        let src = SourceRef {
            kind: OwnerKind::Record,
            owner_ord: r.ordinal,
            pid,
            elem: 0,
        };
        vecs.bind(alias, src, v).unwrap();
        by_ord.insert(r.ordinal, v.clone());
    }
    (vecs, by_ord)
}

fn signature(hits: &[ScoredSource]) -> Vec<(u64, u32)> {
    let mut v: Vec<(u64, u32)> = hits
        .iter()
        .map(|h| (h.source.owner_ord, h.score.to_bits()))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn score_matches_independent_reference_across_dims_and_magnitudes() {
    let mut state = 0x1234_5678_9abc_def0u64;
    for dims in [1usize, 2, 3, 8, 64, 1536] {
        for scale in [1.0f32, 1e-3, 1e3] {
            let q = random_vector(&mut state, dims, scale);
            let v = random_vector(&mut state, dims, scale);
            for metric in [Similarity::Cosine, Similarity::Dot, Similarity::Euclidean] {
                assert_close(
                    score(&q, &v, metric),
                    reference_score(&q, &v, metric),
                    &format!("dims={dims} scale={scale} {}", metric_name(metric)),
                );
            }
        }
    }
}

#[test]
#[expect(
    clippy::float_cmp,
    reason = "exact comparison is the assertion: the zero-norm fallback returns exactly 0.0"
)]
fn cosine_invariants_and_scale_invariance_hold() {
    let a = [3.0f32, 4.0];
    let b = [4.0f32, 3.0];
    let neg = [-3.0f32, -4.0];
    let ortho = [4.0f32, -3.0];
    let zero = [0.0f32, 0.0];
    assert!((score(&a, &a, Similarity::Cosine) - 1.0).abs() < 1e-6);
    assert!((score(&a, &neg, Similarity::Cosine) + 1.0).abs() < 1e-6);
    assert!(score(&a, &ortho, Similarity::Cosine).abs() < 1e-6);
    assert_eq!(score(&a, &zero, Similarity::Cosine), 0.0);

    // Cosine is scale invariant: cos(a, b) == cos(k*a, b) for k > 0.
    let k = 7.5f32;
    let scaled: Vec<f32> = a.iter().map(|x| x * k).collect();
    assert!(
        (score(&scaled, &b, Similarity::Cosine) - score(&a, &b, Similarity::Cosine)).abs() < 1e-6,
        "cosine must be scale invariant"
    );
}

#[test]
#[expect(
    clippy::float_cmp,
    reason = "exact comparison is the assertion: identical vectors score exactly 1.0"
)]
fn euclidean_and_dot_invariants_hold() {
    let a = [1.0f32, 2.0, 3.0];
    let b = [4.0f32, 6.0, 3.0];
    // L2 = sqrt(9 + 16 + 0) = 5 -> 1 / (1 + 5) = 1/6.
    assert!((score(&a, &b, Similarity::Euclidean) - 1.0 / 6.0).abs() < 1e-6);
    assert_eq!(score(&a, &a, Similarity::Euclidean), 1.0);
    // dot = 4 + 12 + 9 = 25; self = 1 + 4 + 9 = 14.
    assert!((score(&a, &b, Similarity::Dot) - 25.0).abs() < 1e-5);
    assert!((score(&a, &a, Similarity::Dot) - 14.0).abs() < 1e-5);
}

#[test]
#[expect(
    clippy::float_cmp,
    reason = "bit-identical equality is the assertion: same f32 inputs through the same formula"
)]
fn exact_search_returns_the_pairwise_score_for_every_hit() {
    // Calibration: a hit's score must equal the direct pairwise `score` against
    // the stored vector — no hidden normalization, no ingest-time transform.
    let mut state = 7u64;
    let vectors: Vec<Vec<f32>> = (0..24).map(|_| random_vector(&mut state, 8, 1.0)).collect();
    let (vecs, by_ord) = bind_fixture(
        Arc::new(Store::new_in_memory()),
        "cal",
        8,
        Similarity::Cosine,
        &vectors,
    );
    let q = random_vector(&mut state, 8, 1.0);
    let hits = vecs.search_exact("cal", &q, vectors.len(), None).unwrap();
    assert_eq!(hits.len(), vectors.len());
    for h in hits {
        let bound = by_ord.get(&h.source.owner_ord).expect("bound vector");
        assert_eq!(
            h.score,
            score(&q, bound, Similarity::Cosine),
            "hit score must be bit-identical to the pairwise score"
        );
    }
}

#[test]
fn high_magnitude_high_dim_cosine_matches_reference() {
    // f64 accumulation must not drift or overflow at large magnitudes.
    let mut state = 5u64;
    let dims = 512usize;
    let vectors: Vec<Vec<f32>> = (0..8)
        .map(|_| random_vector(&mut state, dims, 1e3))
        .collect();
    let q = random_vector(&mut state, dims, 1e3);
    let (vecs, by_ord) = bind_fixture(
        Arc::new(Store::new_in_memory()),
        "big",
        u32::try_from(dims).unwrap(),
        Similarity::Cosine,
        &vectors,
    );
    for h in vecs.search_exact("big", &q, vectors.len(), None).unwrap() {
        let bound = by_ord.get(&h.source.owner_ord).unwrap();
        assert_close(
            h.score,
            reference_score(&q, bound, Similarity::Cosine),
            "high-magnitude cosine",
        );
        assert!(h.score.is_finite());
    }
}

#[test]
fn exact_search_and_view_path_agree_bit_for_bit() {
    let mut state = 99u64;
    let vectors: Vec<Vec<f32>> = (0..16).map(|_| random_vector(&mut state, 4, 1.0)).collect();
    let (vecs, _) = bind_fixture(
        Arc::new(Store::new_in_memory()),
        "paths",
        4,
        Similarity::Dot,
        &vectors,
    );
    let q = random_vector(&mut state, 4, 1.0);
    let direct = vecs.search_exact("paths", &q, 16, None).unwrap();
    let snap = vecs.snapshot().unwrap();
    let space = vecs.lookup_space_in_view(&snap, "paths").unwrap().unwrap();
    let via_view = vecs
        .search_exact_in_view(&snap, &space, &q, 16, None)
        .unwrap();
    assert_eq!(signature(&direct), signature(&via_view));
}

#[test]
fn scores_are_bit_identical_across_memory_disk_and_reopen() {
    let mut state = 4242u64;
    let vectors: Vec<Vec<f32>> = (0..32)
        .map(|_| random_vector(&mut state, 16, 1.0))
        .collect();
    let q = random_vector(&mut state, 16, 1.0);

    let mem = bind_fixture(
        Arc::new(Store::new_in_memory()),
        "s",
        16,
        Similarity::Cosine,
        &vectors,
    )
    .0;

    let dir = rushdb_storage::TempPath::dir("vec-drift");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let disk_store = Arc::new(Store::open(&db).unwrap());
    let disk = bind_fixture(disk_store.clone(), "s", 16, Similarity::Cosine, &vectors).0;
    let disk_scores = signature(&disk.search_exact("s", &q, 32, None).unwrap());
    drop(disk);
    drop(disk_store);

    let reopened_store = Arc::new(Store::open(&db).unwrap());
    let reopened = VectorEngine::new(Arc::new(Lmpg::new(reopened_store, TENANT)));
    let reopened_scores = signature(&reopened.search_exact("s", &q, 32, None).unwrap());

    let mem_scores = signature(&mem.search_exact("s", &q, 32, None).unwrap());
    assert_eq!(mem_scores.len(), vectors.len());
    assert_eq!(
        disk_scores, mem_scores,
        "disk scores must match memory bit-for-bit"
    );
    assert_eq!(
        reopened_scores, mem_scores,
        "reopened scores must match memory bit-for-bit"
    );
    std::fs::remove_dir_all(&dir).ok();
}
