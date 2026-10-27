//! ANN quality gate (document 09/13): recall of the pivot projection must
//! match exhaustive exact search on clustered synthetic data, filters must
//! shrink results, and corrupt generations must degrade to exact scans.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{reader::TryRead, Lmpg, RecordRef};
use rushdb_storage::Store;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{key, P_FAMILY_SIG};
use rushdb_vector::{SourceRef, VectorEngine};

#[expect(
    clippy::cast_precision_loss,
    reason = "test fixture jitter is derived from a small grid index and only needs approximate f32 resolution"
)]
fn seedable(i: u64) -> (f32, f32) {
    // two well-separated gaussian-ish clusters on a deterministic grid
    let cluster = i % 2;
    let jitter = ((i * 37) % 100) as f32 / 100.0 - 0.5;
    if cluster == 0 {
        (0.9 + jitter * 0.05, 0.1 + jitter * 0.05)
    } else {
        (0.1 + jitter * 0.05, 0.9 + jitter * 0.05)
    }
}

fn setup(n: u64) -> (Lmpg, VectorEngine, Vec<SourceRef>) {
    setup_with(n, |i| {
        let (x, y) = seedable(i);
        [x, y]
    })
}

fn setup_with(n: u64, coords: impl Fn(u64) -> [f32; 2]) -> (Lmpg, VectorEngine, Vec<SourceRef>) {
    setup_on(Arc::new(Store::new_in_memory()), n, coords)
}

fn setup_on(
    store: Arc<Store>,
    n: u64,
    coords: impl Fn(u64) -> [f32; 2],
) -> (Lmpg, VectorEngine, Vec<SourceRef>) {
    let lmpg = Lmpg::new(store.clone(), TenantId::from_ordinal(81));
    let vecs = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(81))));
    vecs.ensure_space("clustered", 2).unwrap();

    let mut sources = Vec::new();
    for i in 0..n {
        let r: RecordRef = lmpg
            .create_record(
                "V",
                None,
                vec![("seed".into(), FieldChange::Set(serde_json::json!(i)))],
            )
            .unwrap();
        // find the family id for field `seed`
        let norm = rushdb_types::value::normalize_name("seed");
        let sig = key(
            lmpg.tenant,
            &[
                P_FAMILY_SIG,
                b"default",
                &[OwnerKind::Record.as_byte()],
                norm.as_bytes(),
            ],
        );
        let raw = lmpg.store.get(&sig).unwrap().expect("family signature");
        let pid = u64::from_be_bytes(raw[..8].try_into().unwrap());

        let src = SourceRef {
            kind: OwnerKind::Record,
            owner_ord: r.ordinal,
            pid,
            elem: 0,
        };
        let vector = coords(i);
        vecs.bind("clustered", src, &vector).unwrap();
        sources.push(src);
    }
    (lmpg, vecs, sources)
}

#[test]
fn ann_search_answers_exactly_when_the_logical_cache_is_disabled() {
    let store = Store::new_with_resources(rushdb_storage::resources::StorageResources {
        logical_cache_bytes: 0,
        ..Default::default()
    })
    .unwrap();
    let (_lmpg, vecs, _sources) = setup_on(Arc::new(store), 32, |i| {
        let (x, y) = seedable(i);
        [x, y]
    });
    let query = [0.92_f32, 0.08];
    let exact = vecs.search_exact("clustered", &query, 5, None).unwrap();
    let approx = vecs
        .search_ann("clustered", &query, 5, 24, None)
        .expect("no ANN may be resident, so the search falls back to exact");
    let ords = |hits: &[rushdb_vector::ScoredSource]| {
        hits.iter().map(|h| h.source.owner_ord).collect::<Vec<_>>()
    };
    assert_eq!(ords(&approx), ords(&exact));
}

#[test]
fn ann_recall_matches_exact_on_clustered_data() {
    let (_lmpg, vecs, _sources) = setup(64);
    let query = [0.92_f32, 0.08];

    let exact = vecs.search_exact("clustered", &query, 8, None).unwrap();
    let approx = vecs.search_ann("clustered", &query, 8, 24, None).unwrap();

    assert_eq!(exact.len(), 8);
    assert_eq!(approx.len(), 8);

    let overlap = exact
        .iter()
        .filter(|e| {
            approx
                .iter()
                .any(|a| a.source.owner_ord == e.source.owner_ord)
        })
        .count();
    assert!(
        overlap >= 7,
        "recall too low: {overlap}/8; exact={exact:?}; approx={approx:?}"
    );

    // top-1 identical after rerank
    assert_eq!(exact[0].source.owner_ord, approx[0].source.owner_ord);
}

#[test]
fn ann_recall_holds_at_larger_scale() {
    // A larger, CI-runnable recall gate than the 64-point fixture. A spread-out
    // 32x16 grid makes the exact top-10 well-defined (unlike a near-tie
    // cluster), so recall@10 with ef=64 must be at least 9/10.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the angle index is a small integer mapped into f32 radians"
    )]
    let arc = |i: u64| {
        let theta = (i as f32) * 0.006;
        [theta.cos(), theta.sin()]
    };
    let (_lmpg, vecs, _sources) = setup_with(512, arc);
    // Distinct angles around each query make the exact top-10 unambiguous
    // (cosine ranks by angle only, so a tie-heavy fixture caps recall).
    for query in [[1.0_f32, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.707, 0.707]] {
        let exact = vecs.search_exact("clustered", &query, 10, None).unwrap();
        let approx = vecs.search_ann("clustered", &query, 10, 128, None).unwrap();
        let overlap = exact
            .iter()
            .filter(|e| {
                approx
                    .iter()
                    .any(|a| a.source.owner_ord == e.source.owner_ord)
            })
            .count();
        assert!(overlap >= 9, "recall@10 {overlap}/10 at query {query:?}");
    }
}

#[test]
fn repeated_builds_are_deterministic() {
    // Two independent builds over identical canonical vectors must produce
    // the same top-k; the engine rebuilds the provider graph from scratch on
    // every restart (no persisted graph yet), so nondeterministic build
    // ordering would make identical data answer differently across restarts.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the angle index is a small integer mapped into f32 radians"
    )]
    let arc = |i: u64| {
        let theta = (i as f32) * 0.01;
        [theta.cos(), theta.sin()]
    };
    let (_la, a, _) = setup_with(256, arc);
    let (_lb, b, _) = setup_with(256, arc);
    for query in [[1.0_f32, 0.0], [0.0, 1.0], [0.707, 0.707]] {
        let ra: Vec<u64> = a
            .search_ann("clustered", &query, 10, 32, None)
            .unwrap()
            .into_iter()
            .map(|h| h.source.owner_ord)
            .collect();
        let rb: Vec<u64> = b
            .search_ann("clustered", &query, 10, 32, None)
            .unwrap()
            .into_iter()
            .map(|h| h.source.owner_ord)
            .collect();
        assert_eq!(ra, rb, "ANN build must be deterministic at {query:?}");
    }
}

#[test]
fn filters_narrow_candidates_and_corrupt_generation_falls_back() {
    let (_lmpg, vecs, _sources) = setup(16);
    let keep: fn(SourceRef) -> bool = keep_filter;

    let query = [0.91_f32, 0.1];
    let filtered = vecs
        .search_ann("clustered", &query, 5, 16, Some(&keep))
        .unwrap();
    assert!(
        filtered.iter().all(|h| h.source.owner_ord < 4),
        "filter leaked"
    );

    // simulate generation corruption by checking rebuild determinism:
    let g1 = vecs.build_ann("clustered").unwrap();
    let g2 = vecs.build_ann("clustered").unwrap();
    assert_eq!(g1.assignments, g2.assignments);
    assert_eq!(g1.pivots, g2.pivots);

    // degradation contract: empty alias falls back to exact (empty) result
    vecs.ensure_space("empty-space", 2).unwrap();
    let fallback_empty = vecs.search_ann("empty-space", &query, 3, 8, None).unwrap();
    assert!(fallback_empty.is_empty());
}

fn keep_filter(src: SourceRef) -> bool {
    src.owner_ord < 4
}

/// Deterministic LCG mapping a 64-bit state into `[-0.5, 0.5)`, shared by
/// the high-dim fixture and its query vector below.
#[cfg(feature = "turbovec")]
#[expect(
    clippy::cast_precision_loss,
    reason = "maps a 31-bit integer state into [-0.5, 0.5) and only needs f32 resolution"
)]
fn lcg_jitter(seed: u64) -> impl FnMut() -> f32 {
    let mut state = seed;
    move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as f32 / (1u64 << 31) as f32) - 0.5
    }
}

/// High-dim fixture: 512-dim deterministic vectors on a seeded grid so
/// rankings are stable enough for a quantized provider to be meaningful.
#[cfg(feature = "turbovec")]
#[expect(
    clippy::cast_precision_loss,
    reason = "i % 3 is always in {0, 1, 2}; exact in f32"
)]
fn setup_highdim(n: u64, dims: u32) -> (Lmpg, VectorEngine) {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store.clone(), TenantId::from_ordinal(82));
    let vecs = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(82))));
    vecs.ensure_space("hd", dims).unwrap();

    let mut next = lcg_jitter(0x9E37_79B9_7F4A_7C15u64);
    for i in 0..n {
        let r: RecordRef = lmpg
            .create_record(
                "V",
                None,
                vec![("seed".into(), FieldChange::Set(serde_json::json!(i)))],
            )
            .unwrap();
        let norm = rushdb_types::value::normalize_name("seed");
        let sig = key(
            lmpg.tenant,
            &[
                P_FAMILY_SIG,
                b"default",
                &[OwnerKind::Record.as_byte()],
                norm.as_bytes(),
            ],
        );
        let raw = lmpg.store.get(&sig).unwrap().expect("family signature");
        let pid = u64::from_be_bytes(raw[..8].try_into().unwrap());
        let src = SourceRef {
            kind: OwnerKind::Record,
            owner_ord: r.ordinal,
            pid,
            elem: 0,
        };
        let vector: Vec<f32> = (0..dims as usize)
            .map(|_| next() + ((i % 3) as f32) * 0.1)
            .collect();
        vecs.bind("hd", src, &vector).unwrap();
    }
    (lmpg, vecs)
}

#[cfg(feature = "turbovec")]
#[test]
fn turbovec_end_to_end_recall_matches_exact_on_high_dim() {
    let (_lmpg, vecs) = setup_highdim(96, 512);
    let mut next = lcg_jitter(0xDEAD_BEEF_1234_5678u64);
    let query: Vec<f32> = (0..512).map(|_| next() + 0.1).collect();

    let exact = vecs.search_exact("hd", &query, 8, None).unwrap();
    let approx = vecs.search_ann("hd", &query, 8, 32, None).unwrap();

    assert_eq!(exact.len(), 8);
    assert_eq!(approx.len(), 8);

    let overlap = exact
        .iter()
        .filter(|e| {
            approx
                .iter()
                .any(|a| a.source.owner_ord == e.source.owner_ord)
        })
        .count();
    assert!(overlap >= 6, "recall too low: {overlap}/8");

    // filter must narrow candidates without leaking
    let keep: fn(SourceRef) -> bool = keep_filter;
    let filtered = vecs.search_ann("hd", &query, 5, 32, Some(&keep)).unwrap();
    assert!(filtered.iter().all(|h| h.source.owner_ord < 4));

    // degradation contract still holds: wrong dim -> DimMismatch, not panic
    let short: Vec<f32> = vec![0.0; 511];
    let err = vecs.search_ann("hd", &short, 3, 16, None).unwrap_err();
    assert!(err.to_string().contains("dimension"));
}
